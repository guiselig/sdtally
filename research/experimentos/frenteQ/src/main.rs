//! frenteq (spike): as 15 categorias do Ajustes > Armazenamento em Rust, com as regras da Frente P,
//! e Dados do Sistema = (usado do Data − purgeable) − Σ categorias. Só leitura.
//! uso: frenteq [id...]   (sem argumento: todas + a conta; ids na tabela CATS)
//!      frenteq simbolos | tam:<caminho>   (diagnóstico)
//! Saída: JSON em stdout; tempo por categoria em stderr.

mod cats;
mod ffi;

use cats::M;
use serde_json::{Value, json};
use std::sync::mpsc;
use std::time::{Duration, Instant};

type F = fn() -> cats::R;

/// (id da extensão, nome curto, fonte, função, precisa de FDA, timeout em s)
const CATS: &[(&str, &str, &str, F, bool, u64)] = &[
    ("com.apple.STMExtension.Applications", "apps", "Spotlight (MDQuery) + STMSize (StorageManagement, privado)", cats::apps, false, 600),
    ("com.apple.STMExtension.Documents", "documents", "STMSize(~) com excluídos (StorageManagement, privado)", cats::documents, false, 600),
    ("com.apple.Music.MusicStorageExtension", "music", "Library.musicdb (AES+zlib), FileSize das faixas locais", || cats::midia(false), false, 120),
    ("com.apple.Photos.StorageManagementExtension", "photos", "PLFileUtilities (PhotoLibraryServicesCore, privado): r2 − r4", cats::fotos, true, 300),
    ("com.apple.TV.TVStorageExtension", "tv", "Library.tvdb (AES+zlib), FileSize dos itens locais", || cats::midia(true), false, 120),
    ("com.apple.iBooksX.DiskSpaceEfficiency", "books", "BKLibrary (sqlite, só leitura) + isUbiquitousItemAtURL + MDItem", cats::livros, true, 120),
    ("com.apple.podcasts.MacPodcastsStorageExtension", "podcasts", "MTLibrary (sqlite, só leitura) + alocado dos episódios", cats::podcasts, true, 120),
    ("com.apple.STMExtension.Trash", "trash", "STMSize dos filhos de ~/.Trash e /.Trashes/<uid>", cats::trash, false, 300),
    ("com.apple.STMExtension.GarageBand", "garageband", "STMSize de 10 caminhos fixos", cats::garageband, false, 300),
    ("com.apple.STMExtension.iOSFiles", "ios", "STMSize de backups do MobileSync e .ipsw", cats::ios, false, 300),
    ("com.apple.STMExtension.Developer", "developer", "STMSize de caminhos fixos do Xcode/CLT", cats::developer, false, 300),
    ("com.apple.CloudDocsDaemon.StorageManagement", "icloud", "banco FPFS (sqlite, só leitura) via fileproviderctl; bird no detalhe", cats::icloud, true, 300),
    ("com.apple.STMExtension.Mail", "mail", "STMSize(~/Library/Mail) − CacheDelete MailCacheDelete (privado, só consulta)", cats::mail, true, 300),
    ("com.apple.messages.StorageManagementExtension", "messages", "chat.db (sqlite, só leitura): anexos baixados e visíveis", cats::messages, true, 120),
    ("com.apple.STMExtension.OtherUsers", "otherusers", "STMSize de /Users/Shared e /Users/Deleted Users", cats::otherusers, false, 300),
];

/// Roda numa thread com timeout; a thread que passar do tempo fica para trás.
fn com_timeout(f: F, secs: u64) -> cats::R {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(secs)).unwrap_or_else(|_| Err(format!("timeout de {secs} s")))
}

/// Síntese §4.8: FDA detectado abrindo ~/Library/Safari.
fn tem_fda() -> bool {
    std::fs::read_dir(format!("{}/Library/Safari", std::env::var("HOME").unwrap_or_default())).is_ok()
}

fn main() {
    if let Err(e) = ffi::politicas().and_then(|_| ffi::init()) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    let pedidas: Vec<String> = std::env::args().skip(1).collect();
    // Diagnóstico: `frenteq tam:<caminho>` imprime o tamanho como as extensões medem.
    if let Some(p) = pedidas.first().and_then(|a| a.strip_prefix("tam:")) {
        println!("{}", ffi::tam(p, &[]).map_or("ausente".into(), |v| v.to_string()));
        return;
    }
    // `frenteq simbolos`: o que existe de privado nesta versão (para rodar no CI 15/26/27).
    if pedidas.first().is_some_and(|a| a == "simbolos") {
        let pf = "/System/Library/PrivateFrameworks";
        let r = json!({
            "macos": ffi::sysctl_str("kern.osproductversion"),
            "STMSizeDirectoryWithExcludedChildURLs": ffi::stm().is_ok(),
            "CacheDeleteCopyItemizedPurgeableSpaceWithInfo": ffi::dlopen(&format!("{pf}/CacheDelete.framework/Versions/A/CacheDelete"))
                .and_then(|h| ffi::dlsym(h, "CacheDeleteCopyItemizedPurgeableSpaceWithInfo")).is_some(),
            "PLFileUtilities": ffi::dlopen(&format!("{pf}/PhotoLibraryServicesCore.framework/Versions/A/PhotoLibraryServicesCore")).is_some()
                && !ffi::cls("PLFileUtilities").is_null(),
            "fileproviderctl": ffi::existe("/usr/bin/fileproviderctl"),
            // No 27: v40@0:8@16B24B28@?32 (void; path, BOOL, BOOL, bloco). Outra coisa = não chamar.
            "PLFileUtilities_encoding": ffi::enc_classe("PLFileUtilities", "calculateTotalSizeOfFilesAtPath:calculatePurgeable:allocatedSize:result:"),
        });
        println!("{r}");
        return;
    }
    let todas = pedidas.is_empty();
    let fda = tem_fda();
    let t0 = Instant::now();
    let mut saida = Vec::new();
    let mut soma = Some(0i64);
    let mut parcial = Vec::new();
    for &(id, nome, fonte, f, precisa_fda, to) in CATS {
        if !todas && !pedidas.iter().any(|p| p == nome) {
            continue;
        }
        let t = Instant::now();
        let (m, det) = if precisa_fda && !fda {
            (M::NaoMedida("terminal sem Acesso Total ao Disco".into()), Value::Null)
        } else {
            com_timeout(f, to).unwrap_or_else(|e| (M::NaoMedida(e), Value::Null))
        };
        let s = t.elapsed().as_secs_f64();
        eprintln!("[{nome} {s:.2}s] {}", m.bytes().map_or("-".into(), |v| v.to_string()));
        soma = soma.zip(m.bytes()).map(|(a, b)| a + b);
        if let M::Parcial(..) = m {
            parcial.push(nome);
        }
        saida.push(json!({"id": id, "nome": nome, "bytes": m.bytes(), "estado": m.estado(), "fonte": fonte,
                          "tempo_s": (s * 100.0).round() / 100.0, "detalhe": det}));
    }
    let mut raiz = json!({"macos": ffi::sysctl_str("kern.osproductversion"), "fda": fda, "categorias": saida});
    if todas {
        let usado = cats::usado_data();
        let purg = cats::purgeable();
        let sd = match (&usado, purg, soma) {
            (Ok(u), Some(p), Some(s)) if parcial.is_empty() => json!({"bytes": u - p - s, "estado": "medida"}),
            // Σ com categoria parcial é piso, então Dados do Sistema é teto.
            (Ok(u), Some(p), Some(s)) => json!({"bytes": u - p - s, "estado": format!("parcial (teto: {} parcial)", parcial.join(", "))}),
            _ => json!({"bytes": null, "estado": "não medida (falta usado, purgeable ou alguma categoria)"}),
        };
        raiz["soma_categorias"] = json!(soma);
        raiz["usado_data"] = json!({"bytes": usado.as_ref().ok(), "fonte": "diskutil apfs list -plist, CapacityInUse", "erro": usado.as_ref().err()});
        raiz["purgeable"] = json!({"bytes": purg, "fonte": "NSURL ImportantUsage − AvailableCapacity"});
        raiz["dados_do_sistema"] = sd;
        raiz["dados_do_sistema"]["fonte"] = json!("(usado do Data − purgeable) − Σ categorias");
    }
    raiz["tempo_total_s"] = json!((t0.elapsed().as_secs_f64() * 100.0).round() / 100.0);
    println!("{}", serde_json::to_string_pretty(&raiz).unwrap());
}

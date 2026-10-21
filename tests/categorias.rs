//! Categorias do Ajustes reproduzidas (Fase 2, Frente Q §6): a Σ por linha do `top` e a reprodução
//! das 15 categorias contra um oráculo. O oráculo da home falsa é o do `STMSize` (Frente Q §4): Σ
//! `st_blocks × 512` dos arquivos de cada categoria, hardlink uma vez e clone completo de novo,
//! calculado aqui mesmo a partir dos arquivos criados, nunca um número chutado.

use sdtally::sources::categorias::{self, Fatos, fatos, regras};
use sdtally::{anon::Ctx, catalogo, ledger::Medida, sys, walk};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn bytes(p: &Path) -> i64 {
    std::fs::symlink_metadata(p).expect("stat").blocks() as i64 * 512
}

fn escreve(raiz: &Path, rel: &str, n: usize) -> PathBuf {
    let p = raiz.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).expect("mkdir");
    // Conteúdo incompressível e não esparso: o alocado vem de blocos reais.
    let mut x = (rel.len() as u64 + n as u64) | 1;
    let v: Vec<u8> = (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect();
    std::fs::write(&p, v).expect("write");
    p
}

struct Tmp(PathBuf);
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn home_falsa_bate_com_o_oraculo_ao_byte() {
    sys::politicas_seguranca().expect("setiopolicy_np");
    let t = Tmp(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("sdtcat-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&t.0);
    let raiz = t.0.canonicalize().unwrap_or_else(|_| {
        std::fs::create_dir_all(&t.0).expect("mkdir");
        t.0.canonicalize().expect("canonicalize")
    });
    let (kb, h) = (1024usize, "Users/exemplo");
    let f = |rel: &str, n: usize| escreve(&raiz, &format!("{h}/{rel}"), n * kb);
    // Cada lista é o conteúdo de uma categoria (ou de Dados do Sistema).
    let apps = vec![
        escreve(&raiz, "Applications/Editor.app/Contents/MacOS/editor", 300 * kb),
        f("Library/Application Support/Bar/dados.bin", 200),
        f("Library/Containers/com.exemplo.Editor/Data/c.bin", 150),
        // Container com nome de UUID: o bundle id vem do plist do container (aqui, dos fatos).
        f("Library/Containers/00000000-0000-0000-0000-00000000000A/Data/c.bin", 120),
    ];
    // Dados do Sistema não entra no oráculo (é o resto); a lista só prova que o resto não vaza para as categorias.
    let _sistema = vec![
        f("Library/Containers/com.exemplo.Outro/Data/c.bin", 90),
        f("Library/Containers/com.apple.mail/Data/c.bin", 80),
        f("Library/Caches/com.exemplo/cache.bin", 110),
        f("Library/Application Support/MobileSync/Outro/g.bin", 30),
        f("Library/Developer/CoreSimulator/Devices/d.bin", 70),
        f("Library/Mobile Documents/com~apple~CloudDocs/nuvem.bin", 160),
        f("Library/Messages/chat.db", 130),
        f("Music/Music/Music Library.musiclibrary/Library.musicdb", 60),
        escreve(&raiz, "Library/Caches/sistema.bin", 100 * kb),
        escreve(&raiz, "private/var/db/x.bin", 50 * kb),
    ];
    let ios = vec![f("Library/Application Support/MobileSync/Backup/b1/Manifest.db", 140)];
    let mail = vec![f("Library/Mail/V10/caixa.bin", 180)];
    let dev = vec![f("Library/Developer/Xcode/DerivedData/Proj/o.bin", 220), escreve(&raiz, "Library/Developer/CommandLineTools/clt.bin", 260 * kb)];
    let lixo = vec![f(".Trash/velho.bin", 95)];
    let fotos = vec![f("Pictures/Photos Library.photoslibrary/originals/p.jpg", 400)];
    let outros = vec![escreve(&raiz, "Users/Shared/s.bin", 75 * kb)];
    let musica = vec![f("Music/Music/Media.localized/Music/a.m4a", 500)];
    // Documentos: pasta oculta, Documents (Mesa e Documentos desligado), solto em Music, clone e hardlink.
    let orig = f("dev/projeto/orig.bin", 600);
    let clone = raiz.join(format!("{h}/dev/projeto/clone.bin"));
    assert!(std::process::Command::new("/bin/cp").args(["-c", orig.to_str().unwrap(), clone.to_str().unwrap()]).status().expect("cp").success());
    let duro = f("dev/projeto/duro.bin", 210);
    std::fs::hard_link(&duro, raiz.join(format!("{h}/dev/projeto/duro2.bin"))).expect("ln");
    let docs = vec![orig, clone, duro, f(".config/x/conf.bin", 85), f("Documents/d.bin", 115), f("Music/solto.mp3", 65), f("Downloads/baixado.bin", 100)];
    std::process::Command::new("/bin/sync").status().expect("sync");

    // ---- o oráculo: Σ st_blocks por categoria (hardlink uma vez; o clone entra de novo) ----
    let soma = |v: &[PathBuf]| v.iter().map(|p| bytes(p)).sum::<i64>();
    let oraculo = [
        (regras::APPS, soma(&apps)),
        (regras::DOCS, soma(&docs)),
        (regras::MUSICA, soma(&musica)),
        (regras::FOTOS, soma(&fotos)),
        (regras::IOS, soma(&ios)),
        (regras::LIXO, soma(&lixo)),
        (regras::DEV, soma(&dev)),
        (regras::MAIL, soma(&mail)),
        (regras::OUTROS, soma(&outros)),
    ];

    // ---- a travessia de verdade, com os mesmos baldes do sdtally ----
    walk::MEASURE_XATTR.store(false, std::sync::atomic::Ordering::Relaxed);
    let r = walk::walk(raiz.to_str().unwrap(), 2, Arc::new(Ctx::real("exemplo"))).expect("walk");
    assert_eq!(r.clone_dup_files, 1, "o clone completo foi deduplicado pela travessia");
    assert_eq!(r.total.dup_clone, bytes(&docs[1]), "e a cópia ficou registrada no balde dela");
    assert_eq!(r.link_dup_files, 1, "hardlink contado uma vez, como no STMSize");
    let fa = Fatos {
        apps: Some(BTreeSet::from(["com.exemplo.Editor".to_string(), "com.exemplo.Uuid".to_string()])),
        container_ids: [("00000000-0000-0000-0000-00000000000A".to_string(), "com.exemplo.Uuid".to_string())].into(),
        mesa: Some(false),
        documentos: Some(false),
        // Música vem do banco; aqui, o que o banco diria: o tamanho do arquivo de faixa.
        musica: Some(Medida::Valor(soma(&musica))),
        tv: Some(Medida::Valor(0)),
        livros: Some(Medida::Valor(0)),
        podcasts: Some(Medida::Valor(0)),
        mensagens: Some(Medida::Valor(0)),
        mail_purgeable: Some(0),
        outras_homes: Some(0),
        arvore: Some(fatos::arvore(&r)),
    };
    let mut v = json!({
        "macos_version": "27.0", "root": false, "incomplete_reasons": [], "top": [],
        "settings_screen": {"screen": {"categories": oraculo.iter().map(|(id, b)| json!({"id": id, "bytes": b})).collect::<Vec<_>>()}},
    });
    categorias::anexar(&mut v, &fa);
    let rep = &v["system_data"]["reproduced"];
    for (id, esperado) in oraculo {
        let c = rep["categories"].as_array().unwrap().iter().find(|c| c["id"] == id).unwrap();
        assert_eq!(c["bytes"], esperado, "{id}: {c}");
        assert_eq!(c["error_bytes"], 0, "{id}");
    }
    // Só o iCloud Drive (banco do File Provider) fica sem reprodução, e o oráculo desta home não o tem.
    assert_eq!(rep["total"]["not_reproduced"], 1, "{}", rep["total"]);
    assert_eq!(rep["unplaced"]["bytes"], 0);
    assert_eq!(rep["total"]["error_bytes"], 0);
}

#[test]
fn por_linha_confere_com_o_top_nas_fixtures_do_app() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("app/Tests/SDTallyTests/Fixtures");
    let mut n = 0;
    for e in std::fs::read_dir(&dir).expect("fixtures").flatten() {
        let nome = e.file_name().to_string_lossy().into_owned();
        if !(nome.starts_with("completo") && nome.ends_with(".json")) {
            continue;
        }
        let mut v: Value = serde_json::from_slice(&std::fs::read(e.path()).expect("ler")).expect("json");
        catalogo::anexar(&mut v);
        let linhas = v["system_data"]["by_line"].as_array().unwrap_or_else(|| panic!("{nome}: sem by_line"));
        let do_top: i64 = v["top"].as_array().unwrap().iter().filter(|x| x["settings_line"].is_string()).map(|x| x["bytes"].as_i64().unwrap()).sum();
        let soma: i64 = linhas.iter().map(|l| l["bytes"].as_i64().unwrap()).sum();
        assert_eq!(soma, do_top, "{nome}: Σ by_line = Σ top com linha");
        assert!(linhas.windows(2).all(|w| w[0]["bytes"].as_i64() >= w[1]["bytes"].as_i64()), "{nome}: do maior para o menor");
        let nomes: BTreeSet<_> = linhas.iter().map(|l| l["line"].as_str().unwrap()).collect();
        assert_eq!(nomes.len(), linhas.len(), "{nome}: uma entrada por linha");
        assert!(linhas.iter().all(|l| l["grade_b_bytes"].as_i64().unwrap() <= l["bytes"].as_i64().unwrap()));
        // Idempotente.
        let antes = v.clone();
        catalogo::anexar(&mut v);
        assert_eq!(v, antes, "{nome}");
        n += 1;
    }
    assert!(n >= 4, "fixtures completo*.json: {n}");
}

/// Os fatos da fixture real (`facts`), no formato do `Fatos`; `null` é "não lido".
fn fatos_da_fixture(v: &Value) -> Fatos {
    let f = &v["facts"];
    let med = |k: &str| f[k].as_i64().map(Medida::Valor).or_else(|| Some(Medida::NaoMedida("fixture sem o dado".into())));
    Fatos {
        apps: Some(f["apps"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()),
        mesa: f["desktop_in_icloud"].as_bool(),
        documentos: f["documents_in_icloud"].as_bool(),
        musica: med("music"),
        tv: med("tv"),
        livros: med("books"),
        podcasts: med("podcasts"),
        mensagens: med("messages"),
        mail_purgeable: f["mail_purgeable"].as_i64(),
        outras_homes: f["other_accounts"].as_u64().map(|n| n as usize),
        arvore: Some(categorias::Arvore {
            baldes: f["tree"]["buckets"].as_array().unwrap().iter().map(|b| (b[0].as_str().unwrap().to_string(), b[1].as_i64().unwrap())).collect(),
            clones: f["tree"]["clones"].as_i64().unwrap(),
        }),
        ..Fatos::default()
    }
}

/// Mac M4 com macOS 27 medido ao vivo (oráculo: as funções da Apple, `frenteq`): a Σ das categorias
/// reproduzidas cai a ±1 % da Σ do oráculo e cada categoria dentro da tolerância do grau dela.
/// Tolerância por categoria: A = 1 % da categoria (mínimo 100 MB, para as pequenas); B = 2 %
/// (mínimo 100 MB), a faixa do Fotos (lógico menos purgeable não lido).
#[test]
fn mac27_bate_com_o_oraculo_a_um_por_cento() {
    let mut v: Value = serde_json::from_slice(&std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/completo-mac27.json")).unwrap()).unwrap();
    let f = fatos_da_fixture(&v);
    categorias::anexar(&mut v, &f);
    let rep = &v["system_data"]["reproduced"];
    let t = &rep["total"];
    // A fonte de toda categoria, mesmo a sem reprodução, está no catálogo.
    for c in rep["categories"].as_array().unwrap() {
        assert!(catalogo::FONTES.iter().any(|f| f.0 == c["source"]), "source {} sem entrada em FONTES", c["source"]);
    }
    let pct = t["error_percent"].as_f64().expect("erro % da Σ");
    assert!(pct.abs() <= 1.0 && t["within_tolerance"] == true, "Σ fora de ±1 %: {t}");
    for c in rep["categories"].as_array().unwrap() {
        let id = c["id"].as_str().unwrap();
        let Some(erro) = c["error_bytes"].as_i64() else {
            // Sem reprodução: só o iCloud Drive (banco do File Provider) e Livros (por livro).
            assert!(["com.apple.CloudDocsDaemon.StorageManagement", regras::LIVROS].contains(&id), "{id} sem reprodução: {c}");
            continue;
        };
        let tela = c["screen_bytes"].as_i64().unwrap();
        let limite = (tela / if c["grade"] == "B" { 50 } else { 100 }).max(100_000_000);
        eprintln!("{id}: grau {} erro {erro} B ({}) limite {limite} B", c["grade"], c["error_percent"]);
        assert!(erro.abs() <= limite, "{id}: erro {erro} B > {limite} B ({c})");
    }
}

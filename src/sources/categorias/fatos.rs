//! Leitura dos fatos que `regras` e `reproduzir` precisam e que a travessia não traz. Só leitura:
//! Spotlight (`mdfind`), plist de container (`plutil -extract` de uma chave), xattr das pastas,
//! bibliotecas de Música e TV, e uma soma no `chat.db` (`sqlite3`, URI `immutable=1`). Cada falha vira
//! "não lido" (`None` ou `NaoMedida`) e uma linha em `err`; nada aqui vira 0.

use super::{Arvore, Fatos, midia};
use crate::anon;
use crate::ledger::Medida;
use crate::{sys, walk};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// Mesma consulta da extensão de Aplicativos: bundles de aplicativo nas três raízes.
const CONSULTA_APPS: &str = "kMDItemContentType == 'com.apple.application-bundle'";

/// `mdfind -attr kMDItemCFBundleIdentifier`: uma linha "<caminho>   kMDItemCFBundleIdentifier = <id>"
/// por app. Fora: apps dentro de ~/Library e do lixo (a extensão os ignora) e id vazio ou "(null)".
pub fn ler_mdfind(saida: &str, home: &str) -> BTreeSet<String> {
    const CHAVE: &str = "kMDItemCFBundleIdentifier = ";
    saida
        .lines()
        .filter_map(|l| {
            let (caminho, id) = l.split_once(CHAVE)?;
            let caminho = caminho.trim_end();
            let id = id.trim().trim_matches('"');
            let fora = caminho.starts_with(&format!("{home}/Library/")) || caminho.starts_with(&format!("{home}/.Trash/")) || caminho.starts_with("/.Trashes/");
            (!fora && !id.is_empty() && id != "(null)").then(|| id.to_string())
        })
        .collect()
}

fn apps(home: &str) -> Result<BTreeSet<String>, String> {
    let s = sys::run("/usr/bin/mdfind", &["-onlyin", "/Applications", "-onlyin", "/System/Applications", "-onlyin", home, "-attr", "kMDItemCFBundleIdentifier", CONSULTA_APPS], 60)?;
    let ids = ler_mdfind(&s, home);
    // Spotlight desligado ou sem índice responde vazio: não é "nenhum app".
    if ids.is_empty() { Err("Spotlight listed no apps".into()) } else { Ok(ids) }
}

/// Container com nome de UUID → bundle id, pelo plist do `containermanagerd` (só essa chave).
fn ids_de_container(home: &str) -> BTreeMap<String, String> {
    let dir = format!("{home}/Library/Containers");
    std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| anon::is_uuid(n))
        .filter_map(|n| {
            let plist = format!("{dir}/{n}/.com.apple.containermanagerd.metadata.plist");
            let id = sys::run("/usr/bin/plutil", &["-extract", "MCMMetadataIdentifier", "raw", "-o", "-", &plist], 10).ok()?;
            Some((n, id.trim().to_string()))
        })
        .collect()
}

/// A pasta é gerida pelo iCloud Drive ("Mesa e Documentos" ligado): xattr do File Provider na própria
/// pasta. Pasta ausente é desligado; erro de leitura (TCC), não lido.
fn no_icloud(pasta: &Path) -> Option<bool> {
    match std::fs::symlink_metadata(pasta) {
        Ok(_) => Some(sys::apfs::xattr_valor(pasta, "com.apple.file-provider-domain-id").is_some_and(|v| v.starts_with(b"com.apple.CloudDocs."))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(false),
        Err(_) => None,
    }
}

/// Existência sem confundir "negado" com "ausente".
fn existe(p: &Path) -> Result<bool, String> {
    match std::fs::symlink_metadata(p) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("not readable ({e})")),
    }
}

/// Uma soma no banco, em modo só leitura. Banco ausente é `Valor(0)` (o app nunca foi usado);
/// esquema que não bate, `NaoMedida`.
fn soma_sql(banco: &Path, consulta: &str) -> Medida {
    match existe(banco) {
        Ok(false) => return Medida::Valor(0),
        Err(e) => return Medida::NaoMedida(e),
        Ok(true) => {}
    }
    let Some(b) = banco.to_str() else { return Medida::NaoMedida("database path is not UTF-8".into()) };
    // immutable=1: sem trava nem -shm, zero escrita; o custo é não ver o que ainda está só no -wal.
    let uri = format!("file:{}?mode=ro&immutable=1", b.replace('%', "%25").replace('?', "%3F").replace('#', "%23"));
    match sys::run("/usr/bin/sqlite3", &["-batch", &uri, consulta], 30) {
        Ok(s) => s.trim().parse::<i64>().map_or_else(|_| Medida::NaoMedida("database answered with something other than a number".into()), Medida::Valor),
        Err(_) => Medida::NaoMedida("database not readable or schema not recognised".into()),
    }
}

/// Livros: sem a pasta da biblioteca é 0; com ela, o tamanho depende de `isUbiquitousItemAtURL` e do
/// Spotlight por livro, que esta versão não lê.
fn livros(home: &str) -> Medida {
    match existe(&Path::new(home).join("Library/Containers/com.apple.iBooksX/Data/Documents/BKLibrary")) {
        Ok(false) => Medida::Valor(0),
        Ok(true) => Medida::NaoMedida("Books library present; per-book size not read by this version".into()),
        Err(e) => Medida::NaoMedida(e),
    }
}

/// Podcasts: nenhum episódio com arquivo local é 0; com algum, falta somar o alocado deles.
fn podcasts(home: &str) -> Medida {
    let banco = Path::new(home).join("Library/Group Containers/243LU875E5.groups.com.apple.podcasts/Documents/MTLibrary.sqlite");
    let n = soma_sql(
        &banco,
        "SELECT COUNT(*) FROM ZMTEPISODE e JOIN ZMTMEDIAENCLOSURE m ON m.Z_PK = e.ZCURRENTMEDIAENCLOSURE WHERE m.ZLOCALURL IS NOT NULL AND m.ZLOCALURL > ''",
    );
    match n {
        Medida::Valor(0) => Medida::Valor(0),
        Medida::Valor(_) => Medida::NaoMedida("downloaded episodes present; their size is not read by this version".into()),
        outra => outra,
    }
}

/// Outras contas em /Users (fora Shared, Deleted Users e Guest).
fn outras_contas(home: &str) -> Option<usize> {
    let meu = Path::new(home).file_name()?.to_str()?.to_string();
    Some(
        std::fs::read_dir("/Users")
            .ok()?
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| !n.starts_with('.') && *n != meu && !["Shared", "Deleted Users", "Guest"].contains(&n.as_str()))
            .count(),
    )
}

/// Os baldes da travessia sem dobrar, com as cópias de clone que o Ajustes conta a mais.
pub fn arvore(r: &walk::Result) -> Arvore {
    Arvore { baldes: r.buckets.iter().map(|(k, s)| (k.clone(), s.alloc + s.dup_clone)).collect(), clones: r.total.dup_clone }
}

/// Os fatos desta máquina. `purgeable`: o JSON do CacheDelete (`Purgeable::json`), de onde sai o
/// purgeable do Mail quando o serviço aparece. `fda`: false pula o que exige Acesso Total ao Disco. `arvore`: a travessia, de onde saem os baldes sem dobrar e as cópias de clone.
pub fn coletar(home: &str, fda: Option<bool>, purgeable: Option<&Value>, arvore: Option<&walk::Result>, err: &mut Vec<Value>) -> Fatos {
    let mut falha = |quem: &str, e: String| err.push(json!(format!("{quem}: {e}")));
    let apps = apps(home).map_err(|e| falha("apps", e)).ok();
    let sem_fda = || Medida::NaoMedida("terminal without Full Disk Access".into());
    let dev = std::fs::metadata(home).map(|m| m.dev());
    let biblioteca = |rel: &str| match &dev {
        Ok(d) => midia::total(&Path::new(home).join(rel), *d),
        Err(e) => Medida::NaoMedida(format!("home not readable ({e})")),
    };
    let (musica, tv) = (biblioteca("Music/Music/Music Library.musiclibrary/Library.musicdb"), biblioteca("Movies/TV/TV Library.tvlibrary/Library.tvdb"));
    let com_fda = |f: &dyn Fn() -> Medida| if fda == Some(false) { sem_fda() } else { f() };
    let h = Path::new(home);
    Fatos {
        apps,
        container_ids: ids_de_container(home),
        mesa: no_icloud(&h.join("Desktop")),
        documentos: no_icloud(&h.join("Documents")),
        musica: Some(musica),
        tv: Some(tv),
        livros: Some(com_fda(&|| livros(home))),
        podcasts: Some(com_fda(&|| podcasts(home))),
        mensagens: Some(com_fda(&|| soma_sql(&h.join("Library/Messages/chat.db"), "SELECT COALESCE(SUM(total_bytes), 0) FROM attachment WHERE hide_attachment = 0 AND transfer_state = 5"))),
        mail_purgeable: purgeable.and_then(|p| p["por_servico"]["com.apple.MailCacheDelete"].as_i64()),
        outras_homes: outras_contas(home),
        arvore: arvore.map(self::arvore),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mdfind() {
        let s = "/Applications/Safari.app   kMDItemCFBundleIdentifier = com.apple.Safari\n\
                 /Applications/Meu App.app   kMDItemCFBundleIdentifier = \"com.exemplo.App\"\n\
                 /Users/exemplo/Library/Foo.app   kMDItemCFBundleIdentifier = com.exemplo.Dentro\n\
                 /Users/exemplo/.Trash/Velho.app   kMDItemCFBundleIdentifier = com.exemplo.Velho\n\
                 /Users/exemplo/dev/X.app   kMDItemCFBundleIdentifier = (null)\n\
                 /Users/exemplo/dev/Y.app   kMDItemCFBundleIdentifier = com.exemplo.Y\n\
                 linha sem a chave\n";
        let ids = ler_mdfind(s, "/Users/exemplo");
        assert_eq!(ids.into_iter().collect::<Vec<_>>(), ["com.apple.Safari", "com.exemplo.App", "com.exemplo.Y"]);
    }

    #[test]
    fn banco_ausente_e_zero() {
        let p = std::env::temp_dir().join("sdtally-sem-banco-xyz.db");
        assert_eq!(soma_sql(&p, "SELECT 1"), Medida::Valor(0));
    }

    #[test]
    fn soma_em_banco_real() {
        // sqlite3 e o esquema mínimo do chat.db, num banco criado aqui.
        let dir = std::env::temp_dir().join(format!("sdtally-chat-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("chat.db");
        let q = "CREATE TABLE attachment(total_bytes INTEGER, hide_attachment INTEGER, transfer_state INTEGER);\
                 INSERT INTO attachment VALUES (100,0,5),(50,0,5),(7,1,5),(9,0,4);";
        sys::run("/usr/bin/sqlite3", &[db.to_str().unwrap(), q], 10).unwrap();
        let soma = "SELECT COALESCE(SUM(total_bytes), 0) FROM attachment WHERE hide_attachment = 0 AND transfer_state = 5";
        assert_eq!(soma_sql(&db, soma), Medida::Valor(150));
        // Esquema diferente: não medida, nunca 0.
        assert!(matches!(soma_sql(&db, "SELECT SUM(nao_existe) FROM attachment"), Medida::NaoMedida(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

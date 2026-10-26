//! Guarda "sem rede": o app, o sdtally (CLI) e o helper root não têm código de rede. Vale para
//! src/, app/Sources/SDTally, app/Sources/SDTallyHelper e app/Sources/SDTallyXPC, com as mesmas
//! regras do nunca_apaga (fora o que vem depois de `#[cfg(test)]` e as linhas inteiras de
//! comentário). Só o atualizador opcional, app/Sources/SDTallyUpdater, fala com o github.com.
//! Abrir uma página no navegador (NSWorkspace.open) não é rede do app e não casa aqui.

use std::fs;
use std::path::{Path, PathBuf};

/// APIs de rede (sensível a maiúsculas). Casam só no começo de um identificador.
const PROIBIDOS: [&str; 14] = [
    "URLSession", "NSURLConnection", "NSURLSession", "import Network", "CFNetwork", "CFSocket", "CFStream", "nw_", "socket(",
    "getaddrinfo", "WKWebView", "std::net", "TcpStream", "UdpSocket",
];
/// Crates e bibliotecas de rede: só a palavra inteira ("hyperlink" não conta).
const PALAVRAS: [&str; 4] = ["curl", "reqwest", "ureq", "hyper"];

fn casa(l: &str, p: &str, inteira: bool) -> bool {
    let id = |c: char| c.is_alphanumeric() || c == '_';
    l.match_indices(p)
        .any(|(i, _)| !l[..i].chars().next_back().is_some_and(id) && !(inteira && l[i + p.len()..].chars().next().is_some_and(id)))
}

/// Pedaços de rede no código de produção de `src`: cada um, uma vez por linha. Ler arquivo com
/// `contentsOf:` só conta com URL remota na mesma linha.
fn achados(src: &str, rust: bool) -> Vec<&'static str> {
    let mut v = Vec::new();
    for l in src.lines() {
        let t = l.trim_start();
        if rust && t.starts_with("#[cfg(test)]") {
            break;
        }
        if t.starts_with("//") {
            continue;
        }
        v.extend(PROIBIDOS.iter().copied().filter(|p| casa(l, p, false)));
        v.extend(PALAVRAS.iter().copied().filter(|p| casa(l, p, true)));
        if l.contains("contentsOf:") && (l.contains("URL(string:") || l.contains("http")) {
            v.push("contentsOf: remoto");
        }
    }
    v
}

fn arquivos(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())).flatten() {
        let p = e.path();
        if p.is_dir() {
            arquivos(&p, ext, out);
        } else if p.extension().is_some_and(|x| x == ext) {
            out.push(p);
        }
    }
}

#[test]
fn producao_sem_rede() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (mut rs, mut sw) = (Vec::new(), Vec::new());
    arquivos(&raiz.join("src"), "rs", &mut rs);
    for d in ["app/Sources/SDTally", "app/Sources/SDTallyHelper", "app/Sources/SDTallyXPC"] {
        arquivos(&raiz.join(d), "swift", &mut sw);
    }
    assert!(rs.len() > 20 && sw.len() > 20, "pastas não encontradas: {} rs, {} swift", rs.len(), sw.len());
    let mut fora = Vec::new();
    for (lista, rust) in [(rs, true), (sw, false)] {
        for p in lista {
            let rel = p.strip_prefix(raiz).expect("dentro da raiz").to_string_lossy().into_owned();
            for k in achados(&fs::read_to_string(&p).unwrap_or_else(|e| panic!("{rel}: {e}")), rust) {
                fora.push((rel.clone(), k));
            }
        }
    }
    assert!(fora.is_empty(), "rede fora do atualizador (arquivo, pedaço): {fora:#?}");
}

/// Nenhuma dependência Rust de rede, nem indireta.
#[test]
fn cargo_sem_crate_de_rede() {
    let lock = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock")).expect("Cargo.lock");
    let crates: Vec<&str> = lock.lines().filter_map(|l| l.strip_prefix("name = \"")?.strip_suffix('"')).collect();
    assert!(crates.contains(&"serde_json"), "Cargo.lock sem os crates: {crates:?}");
    let rede = ["reqwest", "ureq", "hyper", "curl", "curl-sys", "native-tls", "rustls", "openssl", "tokio", "socket2", "mio"];
    let achou: Vec<_> = crates.iter().filter(|c| rede.contains(c)).collect();
    assert!(achou.is_empty(), "crate de rede no Cargo.lock: {achou:?}");
}

#[test]
fn guarda_enxerga_o_que_deve() {
    assert_eq!(achados("let s = URLSession.shared\n// URLSession\n", false), ["URLSession"]);
    assert_eq!(achados("import Network\nlet c = nw_connection_create(e, p)\n", false), ["import Network", "nw_"]);
    assert_eq!(achados("let d = try Data(contentsOf: URL(string: \"https://x\")!)\n", false), ["contentsOf: remoto"]);
    assert!(achados("let d = try Data(contentsOf: pasta.appending(path: \"a.json\"))\n", false).is_empty());
    assert!(achados("NSWorkspace.shared.open(urlVersoes)\nlet unw_x = curly + hyperlink\n", false).is_empty());
    let rust = "use std::net::TcpStream;\nfn f() {}\n#[cfg(test)]\nmod t { use std::net::UdpSocket; }\n";
    assert_eq!(achados(rust, true), ["std::net", "TcpStream"]);
    assert_eq!(achados("let x = libc::socket(2, 1, 0);\nlet c = hyper::Client::new();\nCommand::new(\"curl\")\n", true), ["socket(", "hyper", "curl"]);
    // O atualizador tem rede, e a guarda a vê: é por isso que ele fica fora das pastas acima.
    let mut sw = Vec::new();
    arquivos(&Path::new(env!("CARGO_MANIFEST_DIR")).join("app/Sources/SDTallyUpdater"), "swift", &mut sw);
    assert!(sw.iter().any(|p| achados(&fs::read_to_string(p).unwrap(), false).contains(&"URLSession")));
}

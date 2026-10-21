//! Atribuição a apps (Fase 2, Frente C): home e raiz do sistema falsas, montadas aqui, e a fixture
//! `tests/fixtures/donos-top.json` (caminhos de /Users/exemplo). Cada linha do `top` ganha dono só
//! com fonte no disco; as outras ficam sem a chave `owner`.

use sdtally::owners::{self, Donos};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Tmp(PathBuf);
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn app(raiz: &Path, rel: &str, plist: &str, binario: bool) {
    let c = raiz.join(rel).join("Contents");
    std::fs::create_dir_all(&c).unwrap();
    let p = c.join("Info.plist");
    std::fs::write(&p, format!("<?xml version=\"1.0\"?><plist version=\"1.0\"><dict>{plist}</dict></plist>")).unwrap();
    if binario {
        assert!(Command::new("/usr/bin/plutil").args(["-convert", "binary1"]).arg(&p).status().unwrap().success());
    }
}

fn info(id: &str, extra: &str) -> String {
    format!("<key>CFBundleIdentifier</key><string>{id}</string>{extra}")
}

fn container(home: &Path, rel: &str, id: &str) {
    let d = home.join(rel);
    std::fs::create_dir_all(&d).unwrap();
    assert!(Command::new("/usr/bin/xattr").args(["-w", "com.apple.containermanager.identifier", id]).arg(&d).status().unwrap().success());
}

/// Raiz falsa: `raiz/Applications`, `raiz/home/Library/...`.
fn mundo(nome: &str) -> (Tmp, PathBuf, PathBuf) {
    let r = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("owners-{nome}"));
    let _ = std::fs::remove_dir_all(&r);
    let home = r.join("home");
    std::fs::create_dir_all(&home).unwrap();
    app(&r, "Applications/Exemplo.app", &info("com.exemplo.App", "<key>CFBundleName</key><string>Exemplo</string>"), false);
    // Plist binário e nome só por CFBundleName/DisplayName.
    app(&r, "Applications/Pasta Solta/Aninhado.app", &info("com.exemplo.Aninhado", ""), true);
    app(&r, "Applications/Binario.app", &info("com.exemplo.Binario", "<key>CFBundleDisplayName</key><string>Binario</string>"), true);
    // Dois apps (ids diferentes) com o mesmo nome: a pasta "Repetido" não tem dono.
    app(&r, "Applications/Repetido.app", &info("com.exemplo.Repetido1", ""), false);
    app(&r, "Applications/Outro Repetido.app", &info("com.exemplo.Repetido2", "<key>CFBundleName</key><string>Repetido</string>"), false);
    // Fabricante + produto: Application Support/Fabrica/Produto → "Fabrica Produto".
    app(&r, "Applications/Fabrica Produto.app", &info("com.fabrica.Produto", ""), false);
    std::fs::create_dir_all(r.join("Applications/Sem Plist.app/Contents")).unwrap();
    container(&home, "Library/Containers/com.exemplo.App", "com.exemplo.App");
    container(&home, "Library/Containers/com.terceiro.Removido", "com.terceiro.Removido");
    container(&home, "Library/Containers/com.apple.mediaanalysisd", "com.apple.mediaanalysisd");
    std::fs::create_dir_all(home.join("Library/Containers/sem-xattr")).unwrap();
    (Tmp(r.clone()), r, home)
}

fn fixture(home: &Path) -> Value {
    let mut v: Value = serde_json::from_slice(&std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/donos-top.json")).unwrap()).unwrap();
    v["home"] = home.to_str().unwrap().into();
    v
}

fn dono(v: &Value, path: &str) -> Value {
    v["top"].as_array().unwrap().iter().find(|x| x["path"] == path).unwrap_or_else(|| panic!("{path} fora da fixture"))["owner"].clone()
}

#[test]
fn fixture_ganha_dono_so_com_fonte() {
    let (_t, raiz, home) = mundo("fixture");
    let mut v = fixture(&home);
    owners::anexar_em(&mut v, &Donos::em(&raiz, home.to_str().unwrap()));
    let o = |p: &str| dono(&v, p);
    let esperado = |id: &str, nome: Option<&str>, fonte: &str| serde_json::json!({"bundle_id": id, "name": nome, "source": fonte});
    assert_eq!(o("~/Library/Containers/com.exemplo.App/Data"), esperado("com.exemplo.App", Some("Exemplo"), "container"));
    // App que o container conhece e as pastas de apps não: id sem nome.
    assert_eq!(o("~/Library/Containers/com.terceiro.Removido/Data"), esperado("com.terceiro.Removido", None, "container"));
    assert_eq!(o("~/Library/Containers/com.apple.mediaanalysisd/Data"), Value::Null);
    assert_eq!(o("~/Library/Containers/sem-xattr/Data"), Value::Null);
    assert_eq!(o("~/Library/Application Support/com.exemplo.App/Cache"), esperado("com.exemplo.App", Some("Exemplo"), "folder_bundle_id"));
    assert_eq!(o("~/Library/Application Support/exemplo/Dados"), esperado("com.exemplo.App", Some("Exemplo"), "folder_app_name"));
    assert_eq!(o("~/Library/Application Support/Repetido"), Value::Null);
    assert_eq!(o("~/Library/Application Support/Binario"), esperado("com.exemplo.Binario", Some("Binario"), "folder_app_name"));
    assert_eq!(o("~/Library/Application Support/Fabrica/Produto"), esperado("com.fabrica.Produto", Some("Fabrica Produto"), "folder_app_name"));
    assert_eq!(o("~/Library/Caches/Homebrew/downloads"), Value::Null);
    assert_eq!(o("~/Library/Caches/com.apple.bird"), Value::Null);
    assert_eq!(o("/Applications/Exemplo.app/Contents/Frameworks"), esperado("com.exemplo.App", Some("Exemplo"), "app_bundle"));
    assert_eq!(o("/Applications/Pasta Solta/Aninhado.app/Contents"), esperado("com.exemplo.Aninhado", Some("Aninhado"), "app_bundle"));
    assert_eq!(o("/Applications/Sem Plist.app/Contents"), Value::Null);
    assert_eq!(o("~/Library/Mobile Documents/iCloud~com~exemplo~App"), esperado("com.exemplo.App", Some("Exemplo"), "icloud_container"));
    assert_eq!(o("~/Library/Mobile Documents/57T9237FN3~com~exemplo~App"), esperado("com.exemplo.App", Some("Exemplo"), "icloud_container"));
    assert_eq!(o("~/Library/Mobile Documents/com~apple~CloudDocs"), Value::Null);
    assert_eq!(o("~/dev"), Value::Null);
    // Linha dobrada (`folded`) e linha de termo sem caminho: nunca.
    for x in v["top"].as_array().unwrap().iter().filter(|x| x["folded"] == true || x["path"].is_null()) {
        assert!(x.get("owner").is_none(), "{x}");
    }
    let n = v["top"].as_array().unwrap().iter().filter(|x| x.get("owner").is_some()).count();
    assert_eq!(n, 10);
}

#[test]
fn sem_home_nao_atribui_nada() {
    let (_t, _raiz, home) = mundo("semhome");
    let mut v = fixture(&home);
    v.as_object_mut().unwrap().remove("home");
    owners::anexar(&mut v);
    assert!(v["top"].as_array().unwrap().iter().all(|x| x.get("owner").is_none()));
}

#[test]
fn nao_escreve_nada_no_mundo_falso() {
    fn arvore(d: &Path, out: &mut Vec<(PathBuf, u64, std::time::SystemTime)>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let m = e.metadata().unwrap();
            out.push((e.path(), m.len(), m.modified().unwrap()));
            if m.is_dir() {
                arvore(&e.path(), out);
            }
        }
    }
    let (_t, raiz, home) = mundo("leitura");
    let (mut antes, mut depois) = (Vec::new(), Vec::new());
    arvore(&raiz, &mut antes);
    let mut v = fixture(&home);
    owners::anexar_em(&mut v, &Donos::em(&raiz, home.to_str().unwrap()));
    arvore(&raiz, &mut depois);
    antes.sort();
    depois.sort();
    assert_eq!(antes, depois);
}

/// O JSON do `catalogo::anexar` (o caminho do binário) também leva `owner`, e a redação troca o
/// que é pessoal sem mexer na atribuição.
#[test]
fn anexar_do_catalogo_e_redacao() {
    let (_t, _raiz, home) = mundo("catalogo");
    // `anexar` usa a raiz real: só o que não depende dela (xattr de container e pasta de home).
    container(&home, "Library/Containers/com.terceiro.Removido", "com.terceiro.Removido");
    let mut v = fixture(&home);
    sdtally::catalogo::anexar(&mut v);
    assert_eq!(dono(&v, "~/Library/Containers/com.terceiro.Removido/Data")["bundle_id"], "com.terceiro.Removido");
    let ctx = sdtally::anon::Ctx::new("exemplo", "Exemplo Silva", &[]);
    let mut v2 = v.clone();
    v2["top"][0]["owner"] = serde_json::json!({"bundle_id": "com.exemplo.silva.App", "name": "App do Silva", "source": "container"});
    sdtally::redigir::redigir(&mut v2, &ctx, home.to_str().unwrap());
    assert_eq!(v2["top"][0]["owner"]["bundle_id"], "<privado>");
    assert_eq!(v2["top"][0]["owner"]["name"], "<privado>");
    assert_eq!(v2["top"][0]["owner"]["source"], "container");
}

//! Guarda "nunca apaga" (PLANO Fase 3): o código de produção, Rust e Swift, não tem API nem
//! comando que apague. Vale para src/ e app/Sources/ (tudo, inclusive o helper); ficam de fora o
//! que vem depois de `#[cfg(test)]` e as linhas inteiras de comentário. Comando de apagar só como
//! texto sugerido ao usuário e só os da lista fixa de `conselho::COMANDOS`, que não casam aqui.
//! O que o programa apaga dos próprios arquivos fica em `PERMITIDOS`, com a contagem exata.

use std::fs;
use std::path::{Path, PathBuf};

/// Pedaços de código que apagam, mover para o Lixo ou removem atributo (sensível a maiúsculas).
const PROIBIDOS: [&str; 15] = [
    "remove_file", "remove_dir", "unlink", "rmdir", "removefile", "libc::remove(", "removexattr", "removeItem", "trashItem",
    "recycle(", ".moveItem(", "deletelocalsnapshots", "empty trash", "emptyTrash", "to delete",
];

/// (arquivo, pedaço, vezes): apaga só arquivo do próprio programa, nunca dado do usuário.
const PERMITIDOS: [(&str, &str, usize); 3] = [
    // Dir::remover: o temporário que o sdc-coleta criou ao gravar.
    ("src/sys/mod.rs", "unlink", 1),
    // Historico.podar: medições antigas do próprio SDTally (Medicoes/AAAA-MM-DD-HHMM.json comum).
    ("app/Sources/SDTally/Historico.swift", "removeItem", 1),
    // A cópia temporária do sdtally que o helper executa, na pasta 0700 criada por ele.
    ("app/Sources/SDTallyHelper/main.swift", "removeItem", 1),
];

fn palavra(l: &str, p: &str) -> bool {
    let id = |c: char| c.is_alphanumeric() || c == '_';
    l.match_indices(p).any(|(i, _)| !l[..i].chars().next_back().is_some_and(id) && !l[i + p.len()..].chars().next().is_some_and(id))
}

/// Pedaços proibidos que aparecem no código de produção de `src`: cada um, uma vez por linha.
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
        v.extend(PROIBIDOS.iter().copied().filter(|p| l.contains(p)));
        if palavra(l, "rm") {
            v.push("rm");
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
fn producao_nao_apaga() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (mut rs, mut sw) = (Vec::new(), Vec::new());
    arquivos(&raiz.join("src"), "rs", &mut rs);
    arquivos(&raiz.join("app/Sources"), "swift", &mut sw);
    assert!(rs.len() > 20 && sw.len() > 20, "pastas não encontradas: {} rs, {} swift", rs.len(), sw.len());
    let mut vistos: Vec<(String, &str, usize)> = Vec::new();
    for (lista, rust) in [(rs, true), (sw, false)] {
        for p in lista {
            let rel = p.strip_prefix(raiz).expect("dentro da raiz").to_string_lossy().into_owned();
            let a = achados(&fs::read_to_string(&p).unwrap_or_else(|e| panic!("{rel}: {e}")), rust);
            for k in PROIBIDOS.iter().copied().chain(["rm"]) {
                let n = a.iter().filter(|x| **x == k).count();
                if n > 0 {
                    vistos.push((rel.clone(), k, n));
                }
            }
        }
    }
    let esperado: Vec<(String, &str, usize)> = PERMITIDOS.iter().map(|(f, p, n)| (f.to_string(), *p, *n)).collect();
    let fora: Vec<_> = vistos.iter().filter(|v| !esperado.contains(v)).collect();
    let sumiu: Vec<_> = esperado.iter().filter(|e| !vistos.contains(e)).collect();
    assert!(fora.is_empty(), "apaga no código de produção (arquivo, pedaço, vezes): {fora:#?}");
    assert!(sumiu.is_empty(), "PERMITIDOS desatualizado, tire ou ajuste: {sumiu:#?}");
}

#[test]
fn guarda_enxerga_o_que_deve() {
    let rust = "fn f() {\n    std::fs::remove_file(p);\n}\n#[cfg(test)]\nmod t { fn g() { std::fs::remove_dir(p); } }\n";
    assert_eq!(achados(rust, true), ["remove_file"]);
    assert_eq!(achados("// remove_file\n/// removeItem\nlet x = 1;\n", true), Vec::<&str>::new());
    assert_eq!(achados("try fm.removeItem(at: u)\nNSWorkspace.shared.recycle([u])\n", false), ["removeItem", "recycle("]);
    // Swift não tem #[cfg(test)]: o resto do arquivo conta.
    assert_eq!(achados("#[cfg(test)]\ntrashItem\n", false), ["trashItem"]);
    assert_eq!(achados("Command::new(\"/bin/rm\")\nlet a = \"rm -rf ~/x\"\ntmutil deletelocalsnapshots 2026\n", true), ["rm", "rm", "deletelocalsnapshots"]);
    assert!(achados("let platform = \"form \";\nlet x = format!(\"{rm_x}\");\n", true).is_empty());
}

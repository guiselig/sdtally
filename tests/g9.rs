//! Carta anti-scareware (spec da fase 6, §7 G9) nos fontes Rust que têm texto para o usuário.

use std::process::Command;

const PROIBIDAS: &str = include_str!("../scripts/g9-proibidas.txt");
const EXCECOES: &str = include_str!("../scripts/g9-excecoes.txt");
const FONTES: [(&str, &str); 4] = [
    ("src/conselho.rs", include_str!("../src/conselho.rs")),
    ("src/catalogo.rs", include_str!("../src/catalogo.rs")),
    ("src/voltou.rs", include_str!("../src/voltou.rs")),
    ("src/report/i18n.rs", include_str!("../src/report/i18n.rs")),
];

#[test]
fn g9_sem_proibidas() {
    let proibidas: Vec<&str> = PROIBIDAS.lines().filter(|l| !l.is_empty()).collect();
    assert!(proibidas.contains(&"lixo") && proibidas.contains(&"junk") && !proibidas.contains(&"Lixo"), "lista do §7");
    let excecoes: Vec<&str> = EXCECOES.lines().collect();
    let mut achados = Vec::new();
    for (arq, src) in FONTES {
        for (n, l) in src.lines().enumerate().filter(|(_, l)| !excecoes.contains(l)) {
            achados.extend(proibidas.iter().filter(|p| l.contains(*p)).map(|p| format!("{arq}:{}: {p:?}", n + 1)));
        }
    }
    assert!(achados.is_empty(), "{achados:#?}");
}

#[test]
fn g9_script_so_rust() {
    let raiz = env!("CARGO_MANIFEST_DIR");
    let o = Command::new("/bin/sh").arg(format!("{raiz}/scripts/g9.sh")).arg("--so-rust").output().expect("sh");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let o = Command::new("/bin/sh").arg(format!("{raiz}/scripts/g9.sh")).arg("--outro").output().expect("sh");
    assert_eq!(o.status.code(), Some(2));
}

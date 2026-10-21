//! Dry-run conferido por regra (PLANO Fase 3): todo comando que o sdtally sugere tem um comando
//! que só mostra o que aconteceria (ou só lista), com a fonte. Comando novo sem isso reprova.

use sdtally::catalogo::{self, Acao};
use sdtally::conselho::{self, COMANDOS, Conselho, DETECTORES, REGRAS, SIMULACOES, simulacao};

/// Palavras de comando que mudam o disco. `clean` e `cleanup` só passam com `-n` ou `--dry-run`.
const MUDA: [&str; 8] = ["purge", "delete", "remove", "rm", "uninstall", "prune", "--force", "-f"];

fn so_le(c: &str) -> bool {
    let t: Vec<&str> = c.split_whitespace().collect();
    let simula = t.iter().any(|x| matches!(*x, "-n" | "--dry-run"));
    !t.iter().any(|x| MUDA.contains(x)) && (simula || !t.iter().any(|x| matches!(*x, "clean" | "cleanup")))
}

#[test]
fn todo_comando_tem_leitura_e_fonte() {
    assert_eq!(SIMULACOES.len(), COMANDOS.len());
    for c in COMANDOS {
        assert_eq!(SIMULACOES.iter().filter(|s| s.comando == c).count(), 1, "{c}");
        let s = simulacao(c).expect("linha");
        assert!(s.fonte.len() > 20 && s.fonte.contains(':'), "{c}: fonte curta demais: {:?}", s.fonte);
        assert!(so_le(s.leitura), "{c}: a leitura {:?} muda o disco", s.leitura);
        // Comando que muda o disco nunca é a própria leitura.
        assert_eq!(s.leitura == c, so_le(c), "{c}: ou o comando só lê e a leitura é ele, ou a leitura é outra");
    }
}

#[test]
fn toda_regra_ficha_e_detector_com_comando_tem_simulacao() {
    let mut vistos = 0;
    let de_regras = REGRAS.iter().filter_map(|r| r.comando);
    let de_fichas = catalogo::CATALOGO.iter().flat_map(|f| f.agir).filter_map(|a| if let Acao::Comando(c) = a { Some(*c) } else { None });
    for c in de_regras.chain(de_fichas) {
        assert!(simulacao(c).is_some(), "{c}: comando sem dry-run/fonte em conselho::SIMULACOES");
        vistos += 1;
    }
    assert!(vistos >= 10, "{vistos}");
    for d in DETECTORES {
        let f = catalogo::ficha(d.id).expect("ficha do detector");
        assert!(f.agir.iter().all(|a| !matches!(a, Acao::Comando(c) if simulacao(c).is_none())), "{}", d.id);
    }
}

#[test]
fn json_leva_o_dry_run() {
    let com = |comando| Conselho { id: "x", nivel: "safe", comando, ..Default::default() };
    let v = conselho::json(&[com(Some("go clean -cache")), com(Some("brew cleanup --dry-run")), com(None)]);
    assert_eq!(v[0]["dry_run"]["command"], "go clean -cache -n");
    assert!(v[0]["dry_run"]["doc"].as_str().is_some_and(|s| s.contains("go help clean")));
    assert_eq!(v[1]["dry_run"]["command"], "brew cleanup --dry-run");
    assert!(v[2]["dry_run"].is_null());
}

#[test]
fn so_le_distingue() {
    assert!(so_le("brew cleanup --dry-run") && so_le("go clean -cache -n") && so_le("npm cache ls") && so_le("tmutil listlocalsnapshots /"));
    assert!(!so_le("npm cache clean --force") && !so_le("go clean -cache") && !so_le("pip cache purge") && !so_le("xcrun simctl delete unavailable") && !so_le("brew cleanup"));
}

#[test]
fn fixture_do_app_usa_as_simulacoes_do_rust() {
    let f: serde_json::Value = serde_json::from_str(include_str!("../app/Tests/SDTallyTests/Fixtures/completo.json")).expect("json");
    let mut com = 0;
    for a in f["advice"].as_array().expect("advice") {
        let esperado = a["command"].as_str().and_then(simulacao).map(|s| (s.leitura, s.fonte));
        assert_eq!(a["dry_run"]["command"].as_str().zip(a["dry_run"]["doc"].as_str()), esperado, "{}", a["id"]);
        com += usize::from(esperado.is_some());
    }
    assert!(com >= 3, "{com}");
}

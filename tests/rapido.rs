//! Leitura rápida incremental (`--quick --since`): argumentos e o que a CLI pula com o painel do
//! Ajustes fechado.

use serde_json::Value;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_sdtally");

#[test]
fn since_exige_quick_e_numero() {
    for args in [&["--since", "100"][..], &["--json", "--since", "100"], &["--quick", "--json", "--since"], &["--quick", "--json", "--since", "abc"], &["--quick", "--json", "--since", "0"]] {
        assert_eq!(Command::new(BIN).args(args).output().unwrap().status.code(), Some(2), "{args:?}");
    }
}

fn agora() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

fn quick(args: &[&str]) -> Value {
    let o = Command::new(BIN).args(["--quick", "--json"]).args(args).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    serde_json::from_slice(&o.stdout).unwrap()
}

#[test]
fn com_since_e_painel_fechado_nao_le_o_log_mas_mantem_o_resto() {
    // Se quem roda o teste está com Ajustes > Armazenamento aberto, o log é lido: nada a conferir.
    if sdtally::sys::painel_do_ajustes_aberto() {
        return;
    }
    let v = quick(&["--since", &(agora() - 30).to_string()]);
    let tela = &v["settings_screen"]["screen"];
    assert_eq!(tela["state"], "not_measured");
    assert_eq!(tela["reason"], "Settings Storage pane closed; screen log skipped (--since)");
    assert!(!tela["reason"].as_str().unwrap().contains("not read"), "o app lê \"not read\" como log ilegível");
    assert!(v["stage_seconds"].get("tela_log").is_none(), "{}", v["stage_seconds"]);
    // O resto da leitura segue inteiro, com as mesmas fontes.
    assert_eq!(v["settings_screen"]["macos"]["source"], "diskutil_capacity_in_use");
    assert_eq!(v["data_volume"]["used"]["source"], "attr_vol_spaceused");
    assert_eq!(v["capacity"]["source"], "statfs");
    assert!(v["containers"].as_array().is_some_and(|c| c.iter().any(|x| x["holds_data"] == true && x["internal"] == true)));
    assert!(v["stage_errors"].as_array().unwrap().is_empty(), "{}", v["stage_errors"]);
}

#[test]
fn sem_since_le_a_hora_inteira() {
    let v = quick(&[]);
    assert!(v["stage_seconds"].get("tela_log").is_some());
    assert!(!v["settings_screen"]["screen"]["reason"].as_str().unwrap_or("").contains("skipped"));
}

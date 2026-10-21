//! `--verify` (spec da fase 6, §3.5, §4.6 e marco R8): parsers sobre amostras sintéticas,
//! resolução, sanduíche e recusa como root.

use sdtally::catalogo::{COMANDOS_VERIFICAR, FONTES};
use sdtally::conselho::COMANDOS;
use sdtally::report::i18n::Lang;
use sdtally::verify::{self, datas_tm, df_usado_kib, resolucao, sanduiche, sanduiche_igual, swap_total_cmib};
use serde_json::json;

#[test]
fn parse_df_k() {
    let s = "Filesystem   1024-blocks      Used Available Capacity iused     ifree %iused  Mounted on\n\
             /dev/disk3s5   482797652 414820440  23031172    95% 2632791 230311720    1%   /System/Volumes/Data\n";
    assert_eq!(df_usado_kib(s), Some(414_820_440));
    assert_eq!(df_usado_kib("Filesystem 1024-blocks Used\n"), None);
    assert_eq!(df_usado_kib("df: /x: No such file or directory\n"), None);
}

#[test]
fn parse_sysctl_swapusage() {
    assert_eq!(swap_total_cmib("vm.swapusage: total = 11264.00M  used = 9333.44M  free = 1930.56M  (encrypted)\n"), Some(1_126_400));
    assert_eq!(swap_total_cmib("vm.swapusage: total = 0.00M  used = 0.00M  free = 0.00M\n"), Some(0));
    assert_eq!(swap_total_cmib("vm.swapusage: total = 1024.5M  used = 0.00M\n"), None);
    assert_eq!(swap_total_cmib("vm.swapusage: total = +1.00M\n"), None);
    assert_eq!(swap_total_cmib("sysctl: unknown oid\n"), None);
}

#[test]
fn parse_tmutil_listlocalsnapshots() {
    let s = "Snapshots for disk /:\ncom.apple.TimeMachine.2026-10-03-080500.local\ncom.apple.TimeMachine.2026-10-01-181508.local\ncom.apple.os.update-ABC\n";
    assert_eq!(datas_tm(s.lines()), [Some("2026-10-01 18:15:08".into()), Some("2026-10-03 08:05:00".into())]);
    // A lista do fs_snapshot_list passa pela mesma função: mesmas datas, mesma ordem.
    let fs = ["com.apple.os.update-ABC", "com.apple.TimeMachine.2026-10-03-080500.local", "com.apple.TimeMachine.2026-10-01-181508.local"];
    assert_eq!(datas_tm(fs), datas_tm(s.lines()));
    assert!(datas_tm("Snapshots for disk /:\n".lines()).is_empty());
}

#[test]
fn resolucao_kib_mib() {
    assert_eq!(resolucao(424_776_130_560, 1024), (414_820_440, 414_820_440));
    assert_eq!(resolucao(1025, 1024), (1, 2));
    // 11264 MiB exatos em centésimos de MiB; 1 byte a mais já cai entre dois centésimos.
    assert_eq!(resolucao(11_264 * 1_048_576 * 100, 1_048_576), (1_126_400, 1_126_400));
    assert_eq!(resolucao((11_264 * 1_048_576 + 1) * 100, 1_048_576), (1_126_400, 1_126_401));
}

#[test]
fn sanduiche_dentro_fora() {
    assert_eq!(sanduiche((10, 10), 10, 10), ("equal", 10));
    // Arredondamento para cima ou para baixo: os dois valem.
    assert_eq!(sanduiche((9, 10), 10, 10), ("equal", 10));
    assert_eq!(sanduiche((10, 11), 12, 9), ("within", 10));
    assert_eq!(sanduiche((13, 13), 12, 9), ("outside", 13));
    assert_eq!(sanduiche((8, 8), 10, 10), ("outside", 8));
    let (a, b) = (vec![1, 2], vec![1, 2, 3]);
    assert_eq!(sanduiche_igual(&a, &a, &a), "equal");
    assert_eq!(sanduiche_igual(&b, &a, &b), "within");
    assert_eq!(sanduiche_igual(&a, &b, &b), "outside");
    assert_eq!(sanduiche_igual(&vec![9], &a, &b), "outside");
}

#[test]
fn mesma_fonte_not_checkable() {
    let v = verify::verificar(false, "/Users/exemplo-inexistente").expect("sem root");
    assert_eq!((v["schema_version"].as_i64(), v["kind"].as_str()), (Some(1), Some("verify")));
    assert!(v["file_generated_at"].is_null(), "sem arquivo salvo");
    let checks = v["checks"].as_array().unwrap();
    let ids: Vec<&str> = checks.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["data_used", "swap", "snapshots_tm", "capacity", "macos", "screen", "purgeable", "tree", "dir_stats", "apple_intelligence"]);
    for c in checks {
        let chaves: Vec<&String> = c.as_object().unwrap().keys().collect();
        assert_eq!(chaves.len(), 8, "{c}");
        assert!(["equal", "within", "outside", "not_checkable"].contains(&c["result"].as_str().unwrap()), "{c}");
        assert!(["B", "KiB", "MiB/100", "count"].contains(&c["unit"].as_str().unwrap()), "{c}");
        if let Some(cmd) = c["apple_command"].as_str() {
            assert!(COMANDOS_VERIFICAR.contains(&cmd) || COMANDOS.contains(&cmd), "{cmd}: fora das listas fixas");
        }
    }
    for c in &checks[3..6] {
        assert_eq!((c["result"].as_str(), c["reason"].as_str(), &c["apple_command"]), (Some("not_checkable"), Some("same source"), &json!(null)));
    }
    for c in &checks[6..] {
        assert_eq!(c["result"], "not_checkable");
        let r = c["reason"].as_str().unwrap();
        assert!(FONTES.iter().any(|f| r.ends_with(&format!("catalog checked: {} ({})", f.1.checked, f.0))), "{r}");
    }
    let t = verify::texto(&v, Lang::Pt);
    assert!(t.starts_with("Conferência com os comandos da Apple (lado a lado, agora)\n") && t.ends_with("Só leitura. Nada foi gravado.\n"), "{t}");
    assert!(t.contains("capacity · — · — · mesma fonte, não dá para conferir") && t.contains("tree · — · — · sem comando da Apple equivalente"), "{t}");
}

#[test]
fn recusa_como_root() {
    assert_eq!(verify::verificar(true, "/Users/exemplo").err(), Some("arg_root"));
}

#[test]
fn algum_fora_e_texto_mib() {
    let v = json!({"checks": [{"id": "swap", "sdtally": 1_126_400, "apple": [1_126_400, 1_126_401], "unit": "MiB/100", "result": "within", "apple_command": "sysctl vm.swapusage", "reason": null}]});
    assert!(!verify::algum_fora(&v));
    assert!(verify::texto(&v, Lang::Pt).contains("swap · 11.264,00 MiB · 11.264,00 MiB / 11.264,01 MiB · dentro (o disco mudou entre as leituras)"));
    assert!(verify::texto(&v, Lang::En).contains("11,264.00 MiB"));
    assert!(verify::algum_fora(&json!({"checks": [{"result": "outside"}]})));
}

#[test]
fn in_file_da_medicao_salva() {
    let h = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("verify-home");
    let d = sdtally::historico::pasta(h.to_str().unwrap());
    let _ = std::fs::remove_dir_all(&h);
    std::fs::create_dir_all(&d).unwrap();
    let m = json!({
        "schema_version": 1, "home": "/Users/exemplo", "generated_at": "2026-10-03 10:00",
        "data_volume": {"used": {"bytes": 2048 * 1024 + 5}},
        "swap": {"total": 3 * 1_048_576 + 1_048_576 / 2, "used": 0, "source": "sysctl_vm_swapusage"},
        "informational": {"snapshots": [{"kind": "time_machine"}, {"kind": "other"}, {"kind": "time_machine"}]},
    });
    std::fs::write(d.join("2026-10-03-1000.json"), m.to_string()).unwrap();
    let v = verify::verificar(false, h.to_str().unwrap()).expect("sem root");
    assert_eq!(v["file_generated_at"], "2026-10-03 10:00");
    let c = v["checks"].as_array().unwrap();
    assert_eq!((&c[0]["in_file"], &c[1]["in_file"], &c[2]["in_file"]), (&json!(2048), &json!(350), &json!(2)));
}

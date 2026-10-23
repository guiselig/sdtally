//! "O que fazer" (spec da fase 5, §4.4 e §6) a partir de contas fixas, com /Users/exemplo.

use sdtally::catalogo;
use sdtally::conselho::{self, COMANDOS, Conselho, REGRAS};
use sdtally::ledger::Medida;
use sdtally::report::{self, Chave, Conta, Item, Processo, Tela};
use sdtally::sources::snapshots::SnapMais;
use serde_json::{Value, json};

const GB: i64 = 1_000_000_000;

fn linha(nome: &str, termo: Chave, bytes: i64, privado: Option<i64>) -> Item {
    Item { nome: nome.into(), termo, bytes, privado, parcial: false, quantidade: None, processo: None }
}

fn arvore(nome: &str, bytes: i64, privado: i64) -> Item {
    linha(nome, Chave::Arvore, bytes, Some(privado))
}

/// Conta sem nada a recomendar: root, Acesso Total, tudo medido e pequeno.
fn base(ranking: Vec<Item>) -> Conta {
    Conta {
        ferramenta: "sdtally 0.1.0-teste".into(),
        data_hora: "2026-10-03 10:00".into(),
        macos: Some("26.0".into()),
        boot_time: None,
        root: true,
        fda: Some(true),
        negadas: 0,
        interrompida: false,
        tela: Tela { macos: Medida::Valor(40 * GB), ai: Medida::NaoDisponivel, ai_fisico: Medida::Valor(GB), dados_do_sistema: Ok(None), log_tela: None },
        usado: Medida::Valor(400 * GB),
        termos: vec![
            (Chave::Arvore, Medida::Valor(300 * GB)),
            (Chave::Xattr, Medida::Valor(0)),
            (Chave::Snapshots, Medida::Valor(0)),
            (Chave::Apagados, Medida::Valor(0)),
            (Chave::Negadas, Medida::Valor(0)),
            (Chave::Metadados, Medida::Valor(0)),
        ],
        ranking,
        resto: Some(GB),
        purgeable: Medida::Valor(GB),
        purgeable_top: vec![],
        volumes: Value::Null,
        swap: Some((GB as u64, 0)),
        snapshots: Some(vec![]),
        time_machine: Value::Null,
        processos: Some(vec![]),
        repetidos: None,
        erros: vec![],
        tempos: Default::default(),
        lixo: None,
        apfs_meta: None,
        negadas_sem_tamanho: None,
        maiores: None,
        duplicados: None,
    }
}

fn termo(c: &mut Conta, k: Chave, m: Medida) {
    c.termos.iter_mut().find(|t| t.0 == k).expect("termo").1 = m;
}

fn ids(v: &[Conselho]) -> Vec<&'static str> {
    v.iter().map(|x| x.id).collect()
}

/// Uma linha de 10 GB para cada prefixo de regra, mais todas as condições fora da árvore.
fn tudo() -> Conta {
    let mut r: Vec<Item> = REGRAS.iter().flat_map(|g| g.prefixos.iter().map(|p| arvore(&p.replace('*', "x"), 10 * GB, 10 * GB))).collect();
    r.push(linha("/System/Library/AssetsV2/com_apple_x", Chave::Negadas, 20 * GB, None));
    r.push(Item { processo: Some(("Exemplo".into(), 2 * GB)), ..linha("", Chave::Apagados, 2 * GB, None) });
    let mut c = base(r);
    c.root = false;
    c.fda = Some(false);
    c.resto = Some(40 * GB);
    c.purgeable = Medida::Valor(10 * GB);
    c.swap = Some((10 * GB as u64, 9 * GB as u64));
    c.tela.ai_fisico = Medida::Valor(6 * GB);
    let d = SnapMais::default;
    c.snapshots = Some(vec![("time_machine", Some("2026-10-03 08:00:00".into()), Some(3 * GB), d()), ("other", None, Some(2 * GB), d()), ("apple_other", None, Some(9 * GB), d())]);
    termo(&mut c, Chave::Apagados, Medida::Valor(2 * GB));
    termo(&mut c, Chave::Snapshots, Medida::Valor(14 * GB));
    c
}

#[test]
fn todas_as_regras_disparam() {
    let v = conselho::conselhos(&tudo());
    // swap_large e deleted_open juntas viram restart_worth (fase 6, §4.2).
    let especiais = ["grant_full_disk_access", "measure_as_admin", "time_machine_snapshots", "other_snapshots", "purgeable_auto", "restart_worth", "apple_intelligence_models"];
    for id in especiais.iter().copied().chain(REGRAS.iter().map(|r| r.id)) {
        assert_eq!(v.iter().filter(|x| x.id == id).count(), 1, "{id} em {:?}", ids(&v));
    }
    assert_eq!(v.len(), especiais.len() + REGRAS.len());
    let d = v.iter().find(|x| x.id == "restart_worth").expect("restart_worth");
    assert_eq!(d.processo.as_deref(), Some("Exemplo"));
    // snapshots apple_other não viram recomendação.
    assert_eq!(v.iter().find(|x| x.id == "time_machine_snapshots").map(|x| x.bytes), Some(3 * GB));
    assert!(conselho::conselhos(&base(vec![])).is_empty());
}

#[test]
fn so_de_medido() {
    let r = vec![arvore("~/.Trash", 3 * GB, 3 * GB), linha("~/Library/Mobile Documents/com~apple~CloudDocs", Chave::Negadas, 9 * GB, None)];
    let mut c = base(r.clone());
    termo(&mut c, Chave::Arvore, Medida::NaoMedida("interrompido".into()));
    termo(&mut c, Chave::Negadas, Medida::NaoMedida("0 pastas".into()));
    termo(&mut c, Chave::Apagados, Medida::NaoMedida("sem lsof".into()));
    c.ranking.push(Item { processo: Some(("Exemplo".into(), 5 * GB)), ..linha("", Chave::Apagados, 5 * GB, None) });
    c.purgeable = Medida::NaoMedida("cachedelete".into());
    c.tela.ai_fisico = Medida::NaoDisponivel;
    assert!(conselho::conselhos(&c).is_empty(), "{:?}", ids(&conselho::conselhos(&c)));
    // Parcial entra (é piso).
    let mut c = base(r);
    termo(&mut c, Chave::Arvore, Medida::Parcial(300 * GB, "1 pasta".into()));
    assert_eq!(ids(&conselho::conselhos(&c)), ["trash", "icloud_drive"]);
}

#[test]
fn dobrado_dentro_conta() {
    let v = conselho::conselhos(&base(vec![arvore("~/Library/Logs/(demais)", 2 * GB, 2 * GB), arvore("~/Library/Logs/DiagnosticReports/(demais)", GB / 2, GB / 2)]));
    assert_eq!(ids(&v), ["logs"]);
    assert_eq!(v[0].caminhos, ["~/Library/Logs", "~/Library/Logs/DiagnosticReports"]);
    assert_eq!((v[0].bytes, v[0].privado), (5 * GB / 2, Some(5 * GB / 2)));
}

#[test]
fn dobrado_ancestral_nao() {
    let v = conselho::conselhos(&base(vec![
        arvore("~/Library/Caches/(demais)", 50 * GB, 50 * GB),
        arvore("~/Library/Developer/(demais)", 50 * GB, 50 * GB),
        arvore("~/Library/Developer/Xcode", 50 * GB, 50 * GB),
        arvore("~/(demais)", 50 * GB, 50 * GB),
        arvore("~/Library/Mobile Documents/(demais)", 50 * GB, 50 * GB),
    ]));
    assert!(v.is_empty(), "{:?}", ids(&v));
}

#[test]
fn limite_usa_privado() {
    // 5 GB alocados e 0,2 GB privados (clones) não passam.
    assert!(conselho::conselhos(&base(vec![arvore("~/Library/Caches/Homebrew", 5 * GB, GB / 5)])).is_empty());
    // Soma das linhas da mesma regra.
    let v = conselho::conselhos(&base(vec![arvore("~/Library/Caches/Homebrew/a", GB, 6 * GB / 10), arvore("~/Library/Caches/Homebrew/b", GB, 6 * GB / 10)]));
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].privado, Some(12 * GB / 10));
    // Privado null em alguma linha: usa bytes e sai null.
    let v = conselho::conselhos(&base(vec![linha("~/Library/Caches/Homebrew", Chave::Negadas, 3 * GB / 2, None)]));
    assert_eq!((v.len(), v[0].privado, v[0].bytes), (1, None, 3 * GB / 2));
    // do_not_touch pede 5 GB.
    assert!(conselho::conselhos(&base(vec![arvore("~/Library/CloudStorage/Exemplo", 4 * GB, 4 * GB)])).is_empty());
}

#[test]
fn comandos_na_lista() {
    let v = conselho::conselhos(&tudo());
    let mut vistos = Vec::new();
    for x in &v {
        let Some(c) = x.comando else { continue };
        assert_eq!(COMANDOS.iter().filter(|k| **k == c).count(), 1, "{c}");
        for p in ["rm ", "sudo", "tmutil delete", "deletelocalsnapshots", "-rf", "|", ";", "&&", "$(", "`", ">"] {
            assert!(!c.contains(p), "{c:?} contém {p:?}");
        }
        assert!(!c.contains('~') && !c.contains("/Users"), "caminho medido em {c:?}");
        assert!(x.texto.ends_with("Ask the user before running."), "{}", x.texto);
        vistos.push(c);
    }
    vistos.sort();
    vistos.dedup();
    let mut todos = COMANDOS.to_vec();
    todos.sort();
    assert_eq!(vistos, todos, "todo comando da lista é usado por alguma regra");
    // JSON: as chaves do §4.4 mais kind e card (fase 6, §3.2), nada além; evidence só em signature.
    let j = conselho::json(&v);
    for x in j.as_array().expect("lista") {
        let mut k: Vec<&str> = x.as_object().expect("objeto").keys().map(String::as_str).filter(|k| !["evidence", "bytes_logical", "bytes_floor"].contains(k)).collect();
        k.sort();
        assert_eq!(x["evidence"].is_array(), x["kind"] == "signature", "{x}");
        assert_eq!(k, ["bytes", "card", "command", "dry_run", "id", "kind", "level", "paths", "private_bytes", "process", "term", "text"]);
        assert!(x["command"].is_null() || COMANDOS.contains(&x["command"].as_str().unwrap_or("")));
    }
}

#[test]
fn sem_texto_perigoso() {
    for x in conselho::conselhos(&tudo()) {
        for p in ["rm ", "sudo", "tmutil delete", "deletelocalsnapshots", "-rf"] {
            assert!(!x.texto.contains(p), "{}: {:?} contém {p:?}", x.id, x.texto);
        }
    }
}

#[test]
fn componente_inteiro() {
    let v = conselho::conselhos(&base(vec![
        arvore("~/.Trashcan", 9 * GB, 9 * GB),
        arvore("~/Library/Caches/HomebrewX", 9 * GB, 9 * GB),
        arvore("~/Library/LogsAntigos", 9 * GB, 9 * GB),
        arvore("~/Library/Caches/pipx", 9 * GB, 9 * GB),
        arvore("/Systemx", 9 * GB, 9 * GB),
    ]));
    assert!(v.is_empty(), "{:?}", ids(&v));
    let v = conselho::conselhos(&base(vec![arvore("~/.Trash", 9 * GB, 9 * GB), arvore("~/Library/Caches/pip/http", 9 * GB, 9 * GB)]));
    assert_eq!(ids(&v), ["trash", "pip_cache"]);
}

#[test]
fn system_folders_sem_library() {
    let v = conselho::conselhos(&base(vec![
        arvore("/Library/Caches", 20 * GB, 20 * GB),
        arvore("/Library/Application Support/Exemplo", 20 * GB, 20 * GB),
        arvore("/private/var/dbx", 20 * GB, 20 * GB),
        arvore("/private/var/folders", 20 * GB, 20 * GB),
    ]));
    assert!(v.is_empty(), "{:?}", ids(&v));
    let v = conselho::conselhos(&base(vec![
        arvore("/System/Library/AssetsV2", 3 * GB, 3 * GB),
        arvore("/private/var/vm", 2 * GB, 2 * GB),
        linha("/Library/Exemplo protegido", Chave::Negadas, GB, None),
    ]));
    assert_eq!(ids(&v), ["system_folders"]);
    assert_eq!((v[0].bytes, v[0].privado, v[0].termo), (6 * GB, None, None));
}

// ---- fase 6 (§6.2) ----

/// O JSON completo como o bin monta: relatório, `advice` das regras e os detectores.
fn completo(c: &Conta) -> Value {
    let mut v = report::json(c);
    v["advice"] = conselho::json(&conselho::conselhos(c));
    conselho::detectar(&mut v);
    v
}

fn ids_json(v: &Value) -> Vec<String> {
    v["advice"].as_array().expect("advice").iter().map(|x| x["id"].as_str().unwrap_or("").to_string()).collect()
}

fn achou<'a>(v: &'a Value, id: &str) -> Option<&'a Value> {
    v["advice"].as_array()?.iter().find(|x| x["id"] == id)
}

/// Referência do TM de 2026-10-01 18:15:08 (40 h antes de `generated_at`), com `privado` bytes.
fn tm(running: Value, tentativas: u64, privado: i64) -> Conta {
    let mut c = base(vec![]);
    termo(&mut c, Chave::Snapshots, Medida::Valor(privado));
    let m = SnapMais { purgeable: Some(true), criador: Some("com.apple.TimeMachine".into()), papel: Some("reference"), ..SnapMais::default() };
    c.snapshots = Some(vec![("time_machine", Some("2026-10-01 18:15:08".into()), Some(privado), m)]);
    c.time_machine = json!({"state": "measured", "reason": null, "source": "tm_preferences_plist", "running": running,
        "destinations": [{"reference_at": "2026-10-01 18:15:08", "stable_at": "2026-10-03 08:05:00", "last_completed_at": "2026-10-01 20:29:20",
                          "last_attempt_at": "2026-10-03 09:12:55", "attempts_since_completed": tentativas, "result_raw": 18}]});
    c
}

fn chronod(sem_alocado: u64, logico: i64, dv: u64) -> Conta {
    let mut c = base(vec![]);
    termo(&mut c, Chave::Apagados, Medida::Parcial(GB / 10, "sem alocado".into()));
    c.processos = Some(vec![
        Processo { nome: "chronod".into(), bytes: Some(749_568), arquivos: 172, sem_alocado, logico_sem_alocado: logico, dv_launchservices: dv },
        Processo { nome: "Exemplo".into(), bytes: Some(GB / 10), arquivos: 2, sem_alocado: 0, logico_sem_alocado: 0, dv_launchservices: 0 },
    ]);
    c
}

#[test]
fn regras_novas_no_limite() {
    for id in ["asset_voices", "asset_xcode", "asset_sim_runtime", "asset_os_update", "spotlight_index_large", "failed_smb_mount"] {
        let r = REGRAS.iter().find(|r| r.id == id).expect(id);
        let p = r.prefixos[0].replace('*', "x");
        let privado = if r.por_ocupa { 0 } else { r.limite };
        let v = conselho::conselhos(&base(vec![arvore(&p, r.limite, privado)]));
        assert_eq!(ids(&v), [id], "{id} no limite");
        let v = conselho::conselhos(&base(vec![arvore(&p, r.limite - 1, privado.min(r.limite - 1))]));
        assert!(v.is_empty(), "{id} 1 byte abaixo: {:?}", ids(&v));
    }
    // Assinatura: evidence genérica com Σ, limite e a fonte da 1ª linha.
    let v = conselho::json(&conselho::conselhos(&base(vec![arvore("~/Library/Metadata/CoreSpotlight/Priority", 30 * GB, 21 * GB)])));
    assert_eq!(v[0]["kind"], "signature");
    assert_eq!(v[0]["evidence"], json!([{"key": "private_bytes", "value": 21 * GB, "limit": 20 * GB, "source": "getattrlistbulk"}]));
    let v = conselho::json(&conselho::conselhos(&base(vec![linha("/Volumes", Chave::Negadas, 2 * GB, None)])));
    assert_eq!((v[0]["id"].as_str(), v[0]["level"].as_str()), (Some("failed_smb_mount"), Some("anomaly")));
    assert_eq!(v[0]["evidence"], json!([{"key": "bytes", "value": 2 * GB, "limit": GB, "source": "fsctl_dir_stats"}]));
    // Regra comum: sem evidence.
    let v = conselho::json(&conselho::conselhos(&base(vec![arvore("~/.Trash", 2 * GB, 2 * GB)])));
    assert!(v[0].get("evidence").is_none() && v[0]["kind"] == "rule");
}

#[test]
fn assets_por_ocupa() {
    let a = "/System/Library/AssetsV2/com_apple_MobileAsset_";
    let v = conselho::conselhos(&base(vec![arvore(&format!("{a}MetalToolchain"), 2 * GB, 0)]));
    assert_eq!(ids(&v), ["asset_xcode"]);
    assert_eq!((v[0].bytes, v[0].privado), (2 * GB, Some(0)));
    let v = conselho::conselhos(&base(vec![arvore(&format!("{a}iOSSimulatorRuntime"), 3 * GB, 0)]));
    assert_eq!((ids(&v), v[0].comando), (vec!["asset_sim_runtime"], Some("xcrun simctl runtime list")));
    assert!(v[0].texto.ends_with("Ask the user before running."));
}

#[test]
fn assets_saem_de_system_folders() {
    let a = "/System/Library/AssetsV2/com_apple_MobileAsset_";
    let v = conselho::conselhos(&base(vec![
        arvore(&format!("{a}MacSoftwareUpdate"), 8 * GB, 8 * GB),
        arvore(&format!("{a}VoiceServices_Exemplo"), 2 * GB, 0),
        arvore("/System/Library/Exemplo", 6 * GB, 6 * GB),
    ]));
    assert_eq!(ids(&v), ["asset_voices", "asset_os_update", "system_folders"]);
    let s = v.iter().find(|x| x.id == "system_folders").unwrap();
    assert_eq!((s.bytes, s.caminhos.clone()), (6 * GB, vec!["/System/Library/Exemplo".to_string()]));
}

#[test]
fn assets_componente_inteiro() {
    let a = "/System/Library/AssetsV2/com_apple_MobileAsset_";
    let v = conselho::conselhos(&base(vec![arvore(&format!("{a}MetalToolchainX"), 3 * GB, 0), arvore(&format!("{a}AppleDeveloperDocumentationX/y"), 3 * GB, 0)]));
    assert!(!ids(&v).contains(&"asset_xcode"), "{:?}", ids(&v));
    let v = conselho::conselhos(&base(vec![arvore(&format!("{a}MetalToolchain/x"), 3 * GB, 0)]));
    assert_eq!(ids(&v), ["asset_xcode"]);
}

#[test]
fn system_selado_nao_alerta() {
    // Snapshot de atualização (com.apple.os), não removível: nenhum alerta de atualização.
    let mut c = base(vec![]);
    termo(&mut c, Chave::Snapshots, Medida::Valor(20 * GB));
    let m = SnapMais { purgeable: Some(false), criador: Some("com.apple.os".into()), ..SnapMais::default() };
    c.snapshots = Some(vec![("apple_other", None, Some(20 * GB), m)]);
    let v = completo(&c);
    assert!(achou(&v, "os_update_snapshot").is_none(), "{:?}", ids_json(&v));
    assert!(ids_json(&v).is_empty(), "{:?}", ids_json(&v));
}

#[test]
fn tm_backup_stuck_suprime() {
    let v = completo(&tm(json!(false), 6, 30 * GB));
    let ids = ids_json(&v);
    assert!(ids.contains(&"tm_backup_stuck".into()) && !ids.contains(&"time_machine_snapshots".into()), "{ids:?}");
    let a = achou(&v, "tm_backup_stuck").unwrap();
    assert_eq!((a["kind"].as_str(), a["level"].as_str(), a["term"].as_str()), (Some("rule"), Some("check_first"), Some("snapshots")));
    assert_eq!((a["bytes"].as_i64(), a["private_bytes"].as_i64(), a["command"].as_str()), (Some(30 * GB), Some(30 * GB), Some("tmutil listlocalsnapshots /")));
    assert!(a.get("evidence").is_none());
    let t = a["text"].as_str().unwrap();
    assert!(t.contains("from 01/10 at 20:29") && t.contains("6 times") && t.contains("30.0 GB"), "{t}");
    assert!(t.contains("If space runs low") && t.ends_with("Ask the user before running."), "{t}");
    assert_eq!(v["advice_unchecked"], json!([]));
    // Sem purgeable medido, sem a frase do "pode apagá-lo".
    let mut c = tm(json!(false), 6, 30 * GB);
    c.snapshots.as_mut().unwrap()[0].3.purgeable = None;
    assert!(!achou(&completo(&c), "tm_backup_stuck").unwrap()["text"].as_str().unwrap().contains("If space runs low"));
}

#[test]
fn tm_backup_stuck_nao_com_running() {
    let v = completo(&tm(json!(true), 6, 30 * GB));
    assert_eq!(ids_json(&v), ["time_machine_snapshots"]);
    assert_eq!(v["advice_unchecked"], json!([]));
}

#[test]
fn tm_backup_stuck_uma_tentativa_nao() {
    assert!(achou(&completo(&tm(json!(false), 1, 30 * GB)), "tm_backup_stuck").is_none());
    assert!(achou(&completo(&tm(json!(false), 2, 30 * GB)), "tm_backup_stuck").is_some());
    // Limites: 1 GB no termo e mais de 24 h.
    assert!(achou(&completo(&tm(json!(false), 2, GB)), "tm_backup_stuck").is_some());
    assert!(achou(&completo(&tm(json!(false), 2, GB - 1)), "tm_backup_stuck").is_none());
    let mut c = tm(json!(false), 6, 30 * GB);
    c.data_hora = "2026-10-02 18:15".into();
    assert!(achou(&completo(&c), "tm_backup_stuck").is_none(), "24 h não passa");
    c.data_hora = "2026-10-02 19:16".into();
    assert!(achou(&completo(&c), "tm_backup_stuck").is_some());
    // Snapshot que não é a referência do destino.
    let mut c = tm(json!(false), 6, 30 * GB);
    c.snapshots.as_mut().unwrap()[0].3.papel = Some("stable");
    assert!(achou(&completo(&c), "tm_backup_stuck").is_none());
}

#[test]
fn tm_parado_referencia_sem_privado() {
    // A referência não segura nada só dela; os snapshots juntos seguram 49,6 GB.
    let mut c = tm(json!(false), 11, 0);
    termo(&mut c, Chave::Snapshots, Medida::Valor(49_600_000_000));
    let m = SnapMais { purgeable: Some(true), criador: Some("com.apple.TimeMachine".into()), ..SnapMais::default() };
    c.snapshots.as_mut().unwrap().push(("time_machine", Some("2026-10-03 08:00:00".into()), Some(2 * GB), m));
    assert!(ids(&conselho::conselhos(&c)).contains(&"time_machine_snapshots"));
    let v = completo(&c);
    assert!(!ids_json(&v).contains(&"time_machine_snapshots".into()), "{:?}", ids_json(&v));
    let a = achou(&v, "tm_backup_stuck").expect("tm_backup_stuck");
    assert_eq!((a["bytes"].as_i64(), a["private_bytes"].as_i64()), (Some(49_600_000_000), Some(0)));
    let t = a["text"].as_str().unwrap();
    assert!(t.contains("11 times") && t.contains("Local Time Machine snapshots hold 49.6 GB.") && t.contains("may delete them"), "{t}");
    assert!(a.get("bytes_floor").is_none());
}

#[test]
fn tm_parado_nao_com_snapshot_de_outro_app() {
    // O termo é a faixa de todos os snapshots: os 30 GB são do CCC, não do TM.
    let mut c = tm(json!(false), 5, 0);
    termo(&mut c, Chave::Snapshots, Medida::Valor(30 * GB));
    let m = SnapMais { purgeable: Some(false), criador: Some("com.bombich.ccc".into()), ..SnapMais::default() };
    c.snapshots.as_mut().unwrap().push(("other", None, Some(30 * GB), m));
    let v = completo(&c);
    assert!(achou(&v, "tm_backup_stuck").is_none(), "{:?}", ids_json(&v));
    // Com privado do TM de 1 GB+, dispara com a soma dos privados do TM, não com o termo.
    c.snapshots.as_mut().unwrap()[0].2 = Some(2 * GB);
    let a = achou(&completo(&c), "tm_backup_stuck").cloned().expect("tm_backup_stuck");
    assert_eq!(a["bytes"].as_i64(), Some(2 * GB));
    // A soma dos privados do TM é um piso: o JSON marca e o texto diz "at least".
    assert_eq!(a["bytes_floor"], json!(true));
    assert!(a["text"].as_str().unwrap().contains("snapshots hold at least 2.0 GB."), "{a}");
}

#[test]
fn tm_parado_nao_sem_snapshots() {
    let mut c = tm(json!(false), 11, 0);
    termo(&mut c, Chave::Snapshots, Medida::Valor(GB - 1));
    assert!(achou(&completo(&c), "tm_backup_stuck").is_none());
    // Abaixo de 1 GB nem precisa do plist.
    c.time_machine = Value::Null;
    assert_eq!(completo(&c)["advice_unchecked"], json!([]));
}

#[test]
fn chronod_datavault_dispara() {
    let v = completo(&chronod(50, GB, 50));
    let a = achou(&v, "chronod_datavault").expect("chronod");
    assert_eq!((a["kind"].as_str(), a["level"].as_str(), a["bytes_logical"].as_bool()), (Some("signature"), Some("safe"), Some(true)));
    assert_eq!((a["bytes"].as_i64(), a["process"].as_str(), a["term"].as_str()), (Some(GB), Some("chronod"), Some("deleted_open")));
    assert!(a["private_bytes"].is_null() && a["command"].is_null());
    let ev: Vec<(&str, i64)> = a["evidence"].as_array().unwrap().iter().map(|e| (e["key"].as_str().unwrap(), e["limit"].as_i64().unwrap())).collect();
    assert_eq!(ev, [("files_without_allocated", 50), ("logical_bytes_without_allocated", GB), ("launchservices_dv_files", 50)]);
    assert!(a["evidence"].as_array().unwrap().iter().all(|e| e["source"] == "lsof_proc_pidfdinfo"));
    for c in [chronod(49, GB, 50), chronod(50, GB - 1, 50), chronod(50, GB, 49)] {
        assert!(achou(&completo(&c), "chronod_datavault").is_none());
    }
    // Só bytes lógicos: não vira deleted_open nem restart_worth.
    assert_eq!(ids_json(&v), ["chronod_datavault"]);
}

#[test]
fn restart_worth_funde() {
    let apagado = || Item { processo: Some(("Exemplo".into(), 2 * GB)), ..linha("", Chave::Apagados, 2 * GB, None) };
    let mut c = base(vec![apagado()]);
    termo(&mut c, Chave::Apagados, Medida::Valor(2 * GB));
    c.swap = Some((10 * GB as u64, 9 * GB as u64));
    let v = conselho::conselhos(&c);
    assert_eq!(ids(&v), ["restart_worth"]);
    assert_eq!((v[0].nivel, v[0].bytes, v[0].privado, v[0].processo.as_deref()), ("safe", 12 * GB, None, Some("Exemplo")));
    assert!(v[0].texto.contains("10.0 GB of swap") && v[0].texto.contains("2.0 GB of deleted files"), "{}", v[0].texto);
    // Uma só: a de sempre.
    let mut s = c.clone();
    termo(&mut s, Chave::Apagados, Medida::Valor(GB - 1));
    assert_eq!(ids(&conselho::conselhos(&s)), ["swap_large"]);
    c.swap = Some((8 * GB as u64 - 1, 0));
    assert_eq!(ids(&conselho::conselhos(&c)), ["deleted_open"]);
}

#[test]
fn faixa_de_versao() {
    let f = ("15.1", "15.2");
    assert!(conselho::na_faixa("15.1", f) && conselho::na_faixa("15.1.1", f));
    assert!(!conselho::na_faixa("15.2", f) && !conselho::na_faixa("15.0.9", f) && !conselho::na_faixa("26.0", f) && !conselho::na_faixa("15.10", f));
}

#[test]
fn semdado_vai_para_unchecked() {
    let un = |c: &Conta| completo(c)["advice_unchecked"].clone();
    let r = |id: &str, reason: &str| json!({"id": id, "card": id, "reason": reason});
    // Sem snapshot do TM antigo e grande, o plist nem é preciso: "não", e não "não deu".
    assert_eq!(un(&base(vec![])), json!([]));
    assert_eq!(un(&tm(Value::Null, 6, 30 * GB)), json!([r("tm_backup_stuck", "tm status unreadable")]));
    let mut c = tm(json!(false), 6, 30 * GB);
    c.time_machine = json!({"state": "not_measured", "reason": "plist unreadable", "source": "tm_preferences_plist", "running": false, "destinations": null});
    assert_eq!(un(&c), json!([r("tm_backup_stuck", "plist unreadable")]));
    c.time_machine = Value::Null;
    assert_eq!(un(&c), json!([r("tm_backup_stuck", "plist unreadable")]));
    termo(&mut c, Chave::Snapshots, Medida::NaoMedida("sem snapshots".into()));
    assert_eq!(un(&c), json!([r("tm_backup_stuck", "term not measured")]));
    let mut c = chronod(50, GB, 50);
    c.processos = None;
    assert_eq!(un(&c), json!([r("chronod_datavault", "no lsof")]));
    termo(&mut c, Chave::Apagados, Medida::NaoMedida("sem lsof".into()));
    assert_eq!(un(&c), json!([r("chronod_datavault", "term not measured")]));
    // O que não deu para olhar não dispara.
    assert!(ids_json(&completo(&c)).is_empty());
    // A ficha da assinatura entra em `cards`.
    let mut v = completo(&c);
    catalogo::anexar(&mut v);
    assert!(v["cards"]["chronod_datavault"].is_object());
}

#[test]
fn time_machine_sem_url() {
    let mut v = completo(&tm(json!(false), 6, 30 * GB));
    catalogo::anexar(&mut v);
    let s = serde_json::to_string(&v).unwrap();
    for p in ["NetworkURL", "smb://", "afp://", "LastKnownVolumeName", "DestinationID"] {
        assert!(!s.contains(p), "{p}");
    }
    assert!(s.contains("tm_backup_stuck"));
}

/// `duplicate_files`: só com 1 GB+ de cópias a mais, `check_first`, sem comando, sem o nome do
/// arquivo no texto e com as cópias em `paths`; parcial vira piso.
#[test]
fn copias_identicas() {
    use sdtally::duplicados::{Duplicados, Grupo};
    use std::path::PathBuf;
    let g = |b: i64, ps: &[&str]| Grupo { bytes_each: b, caminhos: ps.iter().map(PathBuf::from).collect() };
    let mut c = base(vec![]);
    c.duplicados = Some(Duplicados { grupos: vec![g(400_000_000, &["/Users/exemplo/a.mov", "/Users/exemplo/b.mov"])], candidatos: 3, ..Default::default() });
    assert!(!ids(&conselho::conselhos(&c)).contains(&"duplicate_files"), "abaixo de 1 GB");
    let um = g(3 * GB / 2, &["/System/Volumes/Data/Users/exemplo/Filme Secreto.mov", "/Users/exemplo/x/Filme Secreto.mov", "/Users/exemplo/y/Filme Secreto.mov"]);
    c.duplicados = Some(Duplicados { grupos: vec![um.clone()], candidatos: 3, ..Default::default() });
    let v = conselho::conselhos(&c);
    let a = v.iter().find(|x| x.id == "duplicate_files").expect("conselho");
    assert_eq!((a.nivel, a.bytes, a.comando, a.piso), ("check_first", 3 * GB, None, false));
    assert_eq!(a.caminhos, ["/Users/exemplo/Filme Secreto.mov", "/Users/exemplo/x/Filme Secreto.mov", "/Users/exemplo/y/Filme Secreto.mov"]);
    assert_eq!(a.texto, "3 identical copies of a 1.5 GB file take 3.0 GB more. Keep one of each; before removing the others, check which copy your apps use.");
    c.duplicados = Some(Duplicados { grupos: vec![um, g(GB, &["/a", "/b"])], motivos: vec!["limite de 60 s na comparação"], ..Default::default() });
    let v = conselho::conselhos(&c);
    let a = v.iter().find(|x| x.id == "duplicate_files").expect("conselho");
    assert!(a.piso && a.bytes == 4 * GB && a.texto.starts_with("3 files are identical copies of others and take 4.0 GB more."), "{a:?}");
    assert!(!a.texto.contains("Secreto"));
}

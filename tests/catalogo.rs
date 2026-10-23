//! Catálogo (spec da fase 6, §1.2, §4.1 e §4.6): fichas, fontes e `anexar`, com /Users/exemplo.

use sdtally::catalogo::{self, Acao, CATALOGO, COMANDOS_VERIFICAR, Casa, FONTES, Ref, Sozinho, Txt};
use sdtally::conselho::{self, COMANDOS, Conselho, DETECTORES, REGRAS};
use sdtally::historico;
use sdtally::linhas;
use sdtally::ledger::Medida;
use sdtally::report::{Chave, Conta, Fonte, Item, Tela};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

const GB: i64 = 1_000_000_000;
const BIN: &str = env!("CARGO_BIN_EXE_sdtally");
const NIVEIS: [&str; 7] = ["measurement", "system_handles", "safe", "cost", "check_first", "anomaly", "do_not_touch"];
/// As 9 regras montadas no código (fora de `REGRAS`).
const ESPECIAIS: [&str; 9] = ["grant_full_disk_access", "measure_as_admin", "time_machine_snapshots", "other_snapshots", "purgeable_auto", "deleted_open", "swap_large", "apple_intelligence_models", "duplicate_files"];

fn caminhos() -> impl Iterator<Item = &'static str> {
    CATALOGO.iter().flat_map(|f| f.casa.iter()).filter_map(|c| match c {
        Casa::Caminho(p) => Some(*p),
        _ => None,
    })
}

fn textos(f: &catalogo::Ficha) -> Vec<(&'static str, Txt)> {
    let mut v = vec![("o_que_e", f.o_que_e), ("dono", f.dono), ("perde", f.perde), ("passo", f.passo)];
    if let Sozinho::Sim(t) | Sozinho::Quando(t) = f.sozinho {
        v.push(("sozinho", t));
    }
    v
}

/// Todo `card` (fora de `cards`) do JSON.
fn cards_citados(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m.iter().filter(|(k, _)| *k != "cards") {
                if k == "card" {
                    if let Some(s) = x.as_str() {
                        out.insert(s.into());
                    }
                } else {
                    cards_citados(x, out);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| cards_citados(x, out)),
        _ => {}
    }
}

/// Um conselho de cada id possível (assinatura com `evidence`).
fn advice() -> Value {
    let assinatura = |id: &str| REGRAS.iter().any(|r| r.id == id && r.assinatura) || DETECTORES.iter().any(|d| d.id == id && d.tipo == "signature");
    let ids = ESPECIAIS.iter().copied().chain(["restart_worth"]).chain(DETECTORES.iter().map(|d| d.id)).chain(REGRAS.iter().map(|r| r.id));
    conselho::json(&ids.map(|id| Conselho { id, nivel: "safe", bytes: GB, evidencia: assinatura(id).then(Vec::new), ..Default::default() }).collect::<Vec<_>>())
}

/// JSON completo sintético com todos os 6 lugares de `card` preenchíveis.
fn completo() -> Value {
    let mut top: Vec<Value> = caminhos().map(|p| json!({"path": p.replace('*', "x"), "folded": false, "term": "tree", "bytes": GB, "private_bytes": GB})).collect();
    top.push(json!({"path": "~/Library/Logs", "folded": true, "term": "tree", "bytes": GB, "private_bytes": GB}));
    top.push(json!({"path": null, "folded": false, "term": "deleted_open", "bytes": GB, "top_process": {"name": "Exemplo", "bytes": GB}}));
    top.push(json!({"path": "/opt/exemplo", "folded": false, "term": "tree", "bytes": GB}));
    json!({
        "schema_version": 1,
        "home": "/Users/exemplo",
        "containers": [{"volumes": [{"roles": ["System"], "capacity_in_use": GB}, {"roles": ["VM"]}, {"roles": ["Data"]}, {"roles": []}]}],
        "data_volume": {"used": {"bytes": 400 * GB}, "terms": (["tree", "large_xattrs", "snapshots", "deleted_open", "protected_folders"].map(|k| json!({"key": k, "bytes": GB})))},
        "top": top,
        "informational": {
            "purgeable": {"bytes": 9 * GB, "top_services": (["com.apple.TMCacheDelete", "com.apple.fspurgeable_document", "com.apple.PODCAST"].map(|n| json!({"name": n, "bytes": GB})))},
            "snapshots": (["time_machine", "apple_other", "other"].map(|k| json!({"kind": k, "date": null, "private_bytes": GB}))),
            "deleted_open_processes": [{"name": "chronod", "bytes": GB}, {"name": "Exemplo", "bytes": 1}],
        },
        "advice": advice(),
    })
}

fn fixtures() -> Vec<(String, Value)> {
    fn ler(d: &Path, out: &mut Vec<(String, Value)>) {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                ler(&p, out);
            } else if p.extension().is_some_and(|x| x == "json") {
                let v = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap_or_else(|x| panic!("{}: {x}", p.display()));
                out.push((p.display().to_string(), v));
            }
        }
    }
    let mut v = Vec::new();
    ler(&Path::new(env!("CARGO_MANIFEST_DIR")).join("app/Tests/SDTallyTests/Fixtures"), &mut v);
    assert!(!v.is_empty(), "sem fixtures do app");
    v
}

#[test]
fn todo_card_existe() {
    let mut j = completo();
    catalogo::anexar(&mut j);
    let mut saidas = vec![("completo".to_string(), j)];
    for (nome, mut v) in fixtures() {
        catalogo::anexar(&mut v);
        saidas.push((nome, v));
    }
    let mut ids = BTreeSet::new();
    for (nome, v) in &saidas {
        let mut c = BTreeSet::new();
        cards_citados(v, &mut c);
        for id in &c {
            assert!(catalogo::ficha(id).is_some(), "{nome}: card {id} fora do CATALOGO");
        }
        let chaves: BTreeSet<String> = v["cards"].as_object().unwrap_or_else(|| panic!("{nome}: sem cards")).keys().cloned().collect();
        assert_eq!(chaves, c, "{nome}: cards = só as fichas citadas");
        ids.extend(c);
    }
    // Todo conselho tem a própria ficha, com o mesmo id; kind "signature" só com evidence.
    for a in advice().as_array().unwrap() {
        assert_eq!(a["card"], a["id"], "{a}");
        assert_eq!(a["kind"].as_str(), Some(if a["evidence"].is_array() { "signature" } else { "rule" }), "{a}");
    }
    let j = &saidas[0].1;
    let top = j["top"].as_array().unwrap();
    assert_eq!(top[top.len() - 3]["card"], "logs", "dobrada dentro da regra");
    assert_eq!(top[top.len() - 2]["card"], "deleted_open", "sem caminho: pelo termo");
    assert!(top[top.len() - 1]["card"].is_null());
    let i = &j["informational"];
    assert_eq!(i["purgeable"]["top_services"].as_array().unwrap().iter().map(|x| x["card"].clone()).collect::<Vec<_>>(), [json!("svc_time_machine"), json!("svc_fspurgeable"), Value::Null]);
    assert_eq!(i["snapshots"].as_array().unwrap().iter().map(|x| x["card"].clone()).collect::<Vec<_>>(), [json!("time_machine_snapshots"), Value::Null, json!("other_snapshots")]);
    assert_eq!(i["deleted_open_processes"][0]["card"], "chronod_datavault");
    assert!(i["deleted_open_processes"][1]["card"].is_null());
    let vols: Vec<Value> = j["containers"][0]["volumes"].as_array().unwrap().iter().map(|x| x["card"].clone()).collect();
    assert_eq!(vols, [json!("vol_system"), json!("vol_vm"), Value::Null, Value::Null]);
    let termos: Vec<Value> = j["data_volume"]["terms"].as_array().unwrap().iter().map(|x| x["card"].clone()).collect();
    assert_eq!(termos, [Value::Null, json!("term_large_xattrs"), json!("term_snapshots"), json!("deleted_open"), Value::Null]);
}

#[test]
fn prioridade_e_casamento() {
    let a = "/System/Library/AssetsV2/com_apple_MobileAsset_";
    assert_eq!(catalogo::card_de_caminho(&format!("{a}UAF_Siri_Understanding")), Some("asset_siri_speech"));
    assert_eq!(catalogo::card_de_caminho(&format!("{a}UAF_FM_Visual/x")), Some("apple_intelligence_models"));
    assert_eq!(catalogo::card_de_caminho(&format!("{a}LinguisticDataX")), Some("system_folders"), "componente inteiro");
    assert_eq!(catalogo::card_de_caminho("/System/Library/(demais)"), Some("system_folders"));
    assert_eq!(catalogo::card_de_caminho("~/Library/Preferences/Maxon/Cinebench_2024"), Some("cinebench_assets"));
    assert_eq!(catalogo::card_de_caminho("~/Library/Preferences/com.exemplo.plist"), Some("preferences_anomaly"));
    assert_eq!(catalogo::card_de_caminho("~/Library"), None, "ancestral nunca casa");
    // Fichas do Início (spec-inicio §9.3).
    for (p, id) in [
        ("/Users/x", Some("other_users")),
        ("/Users/Shared", Some("other_users")),
        ("/Users", None),
        ("/Users/(demais)", None),
        ("~/Music", Some("user_folder")),
        ("~/Music/Music/Media.localized", Some("user_folder")),
        ("~/Downloads", Some("user_folder")),
        ("~/Public", None),
        ("~/.colima/_lima", Some("container_vm")),
        ("~/.lima/default", Some("container_vm")),
        ("~/Library/Containers/com.docker.docker/Data", Some("container_vm")),
        ("~/Library/Group Containers/HUAQ24HBR6.dev.orbstack/data", Some("container_vm")),
        ("~/.orbstack", None),
        ("~/Library/Group Containers/group.net.whatsapp.WhatsApp.shared/Message", Some("whatsapp_data")),
        ("~/Library/Mobile Documents/57T9237FN3~net~whatsapp~WhatsApp", Some("icloud_drive")),
        ("~/Library/Caches/Google/Chrome/Default", Some("browser_cache")),
        ("~/Library/Caches/com.apple.Safari", Some("browser_cache")),
        ("~/Library/Caches/Firefox/Profiles", Some("browser_cache")),
        ("~/Library/Caches/Google", None),
    ] {
        assert_eq!(catalogo::card_de_caminho(p), id, "{p}");
    }
    // why: pelo display.
    let mut w = json!({"schema_version": 1, "kind": "item", "path": "/Users/exemplo/.Trash/a", "display": "~/.Trash/a"});
    catalogo::anexar(&mut w);
    assert_eq!(w["card"], "trash");
    assert!(w["cards"]["trash"]["step"]["pt"].is_string());
}

#[test]
fn ids_unicos_e_caminho_sem_repeticao() {
    let ids: BTreeSet<&str> = CATALOGO.iter().map(|f| f.id).collect();
    assert_eq!(ids.len(), CATALOGO.len(), "id repetido");
    let c: Vec<&str> = caminhos().collect();
    assert_eq!(c.iter().collect::<BTreeSet<_>>().len(), c.len(), "Casa::Caminho repetida");
}

#[test]
fn fichas_completas() {
    for f in CATALOGO {
        for (campo, t) in textos(f) {
            assert!(!t.pt.trim().is_empty() && !t.en.trim().is_empty(), "{}.{campo} vazio", f.id);
        }
        assert!(!f.fonte.is_empty(), "{} sem Ref", f.id);
        assert!(!f.agir.is_empty(), "{} sem ação", f.id);
        assert!(!f.conferido.is_empty(), "{} sem conferido", f.id);
        assert!(NIVEIS.contains(&f.nivel), "{}: nível {}", f.id, f.nivel);
        assert!(!(f.id.starts_with("asset_") && f.nivel == "do_not_touch"), "{}: asset não é do_not_touch", f.id);
        // R6: relato só entra com o link conferido.
        for r in f.fonte {
            if let Ref::Relato(s) = r {
                assert!(s.starts_with("https://"), "{}: relato sem link", f.id);
            }
        }
    }
    // Cada regra tem a ficha dela, com o mesmo nível.
    for r in REGRAS {
        assert_eq!(catalogo::ficha(r.ficha).map(|f| (f.id, f.nivel)), Some((r.id, r.nivel)), "{}", r.id);
    }
    for id in ESPECIAIS {
        assert!(catalogo::ficha(id).is_some(), "{id}");
    }
}

#[test]
fn acao_comando_na_lista() {
    for f in CATALOGO {
        for a in f.agir {
            if let Acao::Comando(c) = a {
                assert!(COMANDOS.contains(c), "{}: {c}", f.id);
            }
        }
    }
    // O comando da regra é a ação da ficha.
    for r in REGRAS {
        if let Some(c) = r.comando {
            assert!(catalogo::ficha(r.ficha).unwrap().agir.iter().any(|a| matches!(a, Acao::Comando(x) if *x == c)), "{}", r.id);
        }
    }
}

#[test]
fn textos_sem_proibidas() {
    let g9 = include_str!("../scripts/g9-proibidas.txt");
    // ⌘6 e ⌘7: atalhos que saíram com a barra lateral de 5 itens (spec-inicio §10).
    let proibidas: Vec<&str> = ["rm ", "sudo", "tmutil delete", "deletelocalsnapshots", "-rf", "⌘6", "⌘7"].into_iter().chain(g9.lines().filter(|l| !l.is_empty())).collect();
    assert!(proibidas.len() > 30, "g9-proibidas.txt lido");
    for f in CATALOGO {
        for (campo, t) in textos(f) {
            for p in &proibidas {
                assert!(!t.pt.contains(p) && !t.en.contains(p), "{}.{campo} contém {p:?}", f.id);
            }
        }
    }
}

#[test]
fn toda_fonte_tem_conferencia() {
    fn sources(v: &Value, out: &mut BTreeSet<String>) {
        match v {
            Value::Object(m) => m.iter().for_each(|(k, x)| match (k.as_str(), x.as_str()) {
                ("source", Some(s)) => {
                    out.insert(s.into());
                }
                // `owner.source` é como se achou o dono (docs/json-schema.md), não fonte de medida.
                ("owner", _) => {}
                _ => sources(x, out),
            }),
            Value::Array(a) => a.iter().for_each(|x| sources(x, out)),
            _ => {}
        }
    }
    let mut s = BTreeSet::new();
    for (_, v) in fixtures() {
        sources(&v, &mut s);
    }
    // Todas as `Fonte` (o match sem `_` quebra a compilação quando entrar uma nova).
    for f in [Fonte::Travessia, Fonte::Xattr, Fonte::Snapshots, Fonte::Lsof, Fonte::DirStats, Fonte::VolSpaceUsed, Fonte::Diferenca, Fonte::CapacityInUse, Fonte::DirStatsStm, Fonte::LogPainel, Fonte::CacheDelete, Fonte::AddedTime, Fonte::ApfsUserClient] {
        match f {
            Fonte::Travessia | Fonte::Xattr | Fonte::Snapshots | Fonte::Lsof | Fonte::DirStats | Fonte::VolSpaceUsed | Fonte::Diferenca | Fonte::CapacityInUse | Fonte::DirStatsStm | Fonte::LogPainel | Fonte::CacheDelete | Fonte::AddedTime | Fonte::ApfsUserClient => s.insert(f.chave().into()),
        };
    }
    // Literais `"source": "…"` do relatório (statfs, swap, log do Ajustes e as da fase 6).
    let src = include_str!("../src/report/mod.rs");
    for (i, _) in src.match_indices("\"source\": \"") {
        let r = &src[i + 11..];
        s.insert(r[..r.find('"').unwrap()].into());
    }
    let fontes: Vec<&str> = FONTES.iter().map(|x| x.0).collect();
    assert_eq!(fontes.iter().collect::<BTreeSet<_>>().len(), fontes.len(), "source repetida em FONTES");
    for x in &s {
        assert!(fontes.contains(&x.as_str()), "source {x} sem entrada em FONTES");
    }
    for k in ["sysctl_kern_boottime", "tm_preferences_plist", "diskutil_list_snapshots", "statfs", "settings_ui_log", "sysctl_vm_swapusage"] {
        assert!(fontes.contains(&k), "§3.6: {k}");
    }
    for (n, c) in FONTES {
        assert!(["byte", "margin", "no", "n/a"].contains(&c.checked), "{n}: {}", c.checked);
        assert_eq!(c.on.is_empty(), matches!(c.checked, "no" | "n/a"), "{n}: on");
    }
}

#[test]
fn comandos_verificar_na_lista() {
    let ac: Vec<&str> = FONTES.iter().filter_map(|x| x.1.apple_command).collect();
    assert_eq!(ac.iter().collect::<BTreeSet<_>>(), COMANDOS_VERIFICAR.iter().collect::<BTreeSet<_>>(), "lista = os apple_command de FONTES");
    assert_eq!(COMANDOS_VERIFICAR.len(), ac.len());
    for c in COMANDOS_VERIFICAR {
        for p in ["rm ", "sudo", "tmutil delete", "deletelocalsnapshots", "-rf", "|", ";", "&&", "$(", "`", ">"] {
            assert!(!c.contains(p), "{c:?} contém {p:?}");
        }
        assert!(!COMANDOS.contains(c), "{c}: separada de conselho::COMANDOS");
    }
}

#[test]
fn prefixos_iguais_a_casa() {
    for r in REGRAS {
        let f = catalogo::ficha(r.ficha).unwrap();
        let casa: Vec<&str> = f.casa.iter().filter_map(|c| if let Casa::Caminho(p) = c { Some(*p) } else { None }).collect();
        assert_eq!(casa, r.prefixos, "{}", r.id);
    }
}

fn conta(ranking: Vec<Item>) -> Conta {
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
        termos: [Chave::Arvore, Chave::Xattr, Chave::Snapshots, Chave::Apagados, Chave::Negadas, Chave::Metadados].into_iter().map(|k| (k, Medida::Valor(0))).collect(),
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

#[test]
fn text_igual_fase5() {
    // Os `text` da fase 5 (src/conselho.rs em main fc26830), menos system_folders (§6.1).
    let fase5 = [
        ("trash", "Empty the Trash."),
        ("logs", "Move to the Trash in Finder; apps recreate them. Space returns when the Trash is emptied."),
        ("homebrew_cache", "Run brew cleanup --dry-run to see what it would remove. Without --dry-run it also removes old versions of installed formulae. Ask the user before running."),
        ("npm_cache", "Run npm cache clean --force; npm downloads packages again when needed. Ask the user before running."),
        ("go_build_cache", "Run go clean -cache; the next build recompiles. Ask the user before running."),
        ("pip_cache", "Run pip cache purge; pip downloads packages again when needed. Ask the user before running."),
        ("updater_caches", "Installers already applied: move to the Trash in Finder."),
        ("xcode_derived_data", "Delete in Finder; the next build recompiles everything."),
        ("simulators", "Run xcrun simctl delete unavailable; manage the rest in Xcode > Devices and Simulators. Ask the user before running."),
        ("simulator_runtimes", "See them with xcrun simctl runtime list; remove them in Xcode > Settings > Components. Ask the user before running."),
        ("playwright_browsers", "Only if you do not use Playwright: move to the Trash in Finder."),
        ("ollama_models", "See them with ollama list and remove them through Ollama itself. Ask the user before running."),
        ("cinebench_assets", "Benchmark assets; they download again when it runs."),
        ("cloudkit_cache", "Do not delete while iCloud is syncing; usually leave it."),
        ("claude_vm_bundles", "Quit the Claude app first; this is its virtual machine."),
        ("preferences_anomaly", "Anomaly: Preferences should hold kilobytes; look for the owning app."),
        ("icloud_drive", "Do not delete: this is iCloud Drive and it would disappear from every device. Use Optimize Mac Storage."),
        ("cloud_storage", "Real data; free space through the provider's app."),
    ];
    let r: Vec<Item> = REGRAS
        .iter()
        .flat_map(|g| g.prefixos.iter().map(|p| Item { nome: p.replace('*', "x"), termo: Chave::Arvore, bytes: 10 * GB, privado: Some(10 * GB), parcial: false, quantidade: None, processo: None }))
        .collect();
    let v = conselho::conselhos(&conta(r));
    assert_eq!(v.len(), REGRAS.len());
    for (id, texto) in fase5 {
        assert_eq!(v.iter().find(|x| x.id == id).map(|x| x.texto.as_str()), Some(texto), "{id}");
    }
    // 19 da fase 5 mais as 6 linhas novas do §6.2.
    assert_eq!(fase5.len() + 1 + 6, REGRAS.len());
    let s = v.iter().find(|x| x.id == "system_folders").unwrap();
    assert_eq!(s.texto, "Do not touch: it belongs to the system; macOS needs these folders.");
}

#[test]
fn anexar_idempotente() {
    let original = completo();
    let mut a = original.clone();
    catalogo::anexar(&mut a);
    let mut b = a.clone();
    catalogo::anexar(&mut b);
    assert_eq!(a, b);
    // Sem `card`/`cards` e a linha do Ajustes, volta ao original: nenhum número muda.
    fn tirar(v: &mut Value) {
        match v {
            Value::Object(m) => {
                m.remove("cards");
                m.remove("system_data");
                m.remove("settings_line");
                m.remove("settings_line_grade");
                m.remove("owner");
                // advice já sai com card do conselho::json; só os de anexar saem.
                if !m.contains_key("level") {
                    m.remove("card");
                }
                m.values_mut().for_each(tirar);
            }
            Value::Array(x) => x.iter_mut().for_each(tirar),
            _ => {}
        }
    }
    tirar(&mut b);
    assert_eq!(b, original);
    // Fora de objeto e sem as chaves: ignorado.
    let mut n = json!([1, 2]);
    catalogo::anexar(&mut n);
    assert_eq!(n, json!([1, 2]));
    let mut q = json!({"kind": "quick", "purgeable": {"top_services": null}});
    catalogo::anexar(&mut q);
    assert_eq!(q, json!({"kind": "quick", "purgeable": {"top_services": null}, "cards": {}}));
}

#[test]
fn last_sai_cru() {
    let h = Path::new(env!("CARGO_TARGET_TMPDIR")).join("catalogo-last");
    let _ = std::fs::remove_dir_all(&h);
    let d = historico::pasta(h.to_str().unwrap());
    std::fs::create_dir_all(&d).unwrap();
    let m = json!({
        "schema_version": 1, "home": "/Users/exemplo", "tool": "sdtally 0.1.0", "generated_at": "2026-10-01 09:00",
        "complete": true, "root": true, "full_disk_access": true,
        "data_volume": {"used": {"bytes": 400_000_000_000_i64}, "terms": [{"key": "snapshots", "bytes": 1}]},
        "top": [{"path": "~/.Trash", "folded": false, "term": "tree", "bytes": 1}],
    });
    std::fs::write(d.join("2026-10-01-0900.json"), serde_json::to_vec_pretty(&m).unwrap()).unwrap();
    let o = Command::new(BIN).args(["--last", "--json"]).env("HOME", &h).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v, m, "--last sem card nem cards");
}

#[test]
fn catalog_pelo_binario() {
    let o = Command::new(BIN).args(["--catalog", "--json"]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!((v["schema_version"].as_i64(), v["kind"].as_str()), (Some(1), Some("catalog")));
    assert_eq!(v["cards"].as_object().map(|m| m.len()), Some(CATALOGO.len()));
    assert_eq!(v["sources"].as_object().map(|m| m.len()), Some(FONTES.len()));
    let f = &v["cards"]["homebrew_cache"];
    for k in ["what", "owner", "self_cleans", "self_cleans_text", "level", "loses", "step", "actions", "refs", "verified_on"] {
        assert!(f.get(k).is_some(), "ficha sem {k}");
    }
    assert_eq!(f["actions"][0], json!({"kind": "command", "target": null, "command": "brew cleanup --dry-run"}));
    assert_eq!(v["cards"]["icloud_drive"]["actions"][0], json!({"kind": "settings", "target": "storage", "command": null}));
    assert_eq!(v["sources"]["attr_vol_spaceused"], json!({"checked": "byte", "checked_on": ["15", "26", "27"], "checked_at": "2026-10-01", "apple_command": "df -k /System/Volumes/Data"}));
    assert_eq!(v["sources"]["difference"]["checked_at"], Value::Null);
    for args in [&["--catalog"][..], &["--catalog", "--json", "--quick"], &["--catalog", "--json", "/tmp"], &["--catalog", "--json", "--last"]] {
        assert_eq!(Command::new(BIN).args(args).output().unwrap().status.code(), Some(2), "{args:?}");
    }
}

#[test]
fn linha_ajustes_tabela() {
    const DOC: &str = "com.apple.STMExtension.Documents";
    const APPS: &str = "com.apple.STMExtension.Applications";
    const DEV: &str = "com.apple.STMExtension.Developer";
    const OUTROS: &str = "com.apple.STMExtension.OtherUsers";
    let casos = [
        (None, "snapshots", Some("system_data"), Some("B")),
        (None, "deleted_open", Some("system_data"), Some("A")),
        (None, "large_xattrs", None, None),
        (Some("~"), "tree", Some("mixed"), Some("A")),
        (Some("~/Library"), "tree", Some("mixed"), Some("A")),
        (Some("~/Library/Developer"), "tree", Some("mixed"), Some("A")),
        (Some("~/Library/Application Support/MobileSync"), "tree", Some("mixed"), Some("A")),
        (Some("/Users"), "tree", Some("mixed"), Some("A")),
        (Some("~/.Trash"), "tree", Some("com.apple.STMExtension.Trash"), Some("A")),
        (Some("~/Library/Application Support/MobileSync/Backup"), "tree", Some("com.apple.STMExtension.iOSFiles"), Some("A")),
        (Some("~/Library/Application Support/MobileSync/Logs"), "tree", Some("system_data"), Some("A")),
        (Some("~/Library/Application Support/Claude/vm_bundles"), "tree", Some(APPS), Some("A")),
        (Some("~/Library/Mail/V10"), "protected_folders", Some("com.apple.STMExtension.Mail"), Some("A")),
        (Some("~/Library/Messages/Attachments"), "tree", Some("mixed"), Some("A")),
        (Some("~/Library/Developer/Xcode/DerivedData"), "tree", Some(DEV), Some("A")),
        (Some("~/Library/Developer/Xcode/iOS DeviceSupport"), "tree", Some(DEV), Some("A")),
        (Some("~/Library/Developer/CoreSimulator/Caches"), "tree", Some(DEV), Some("A")),
        (Some("~/Library/Developer/CoreSimulator/Devices"), "tree", Some("system_data"), Some("A")),
        (Some("~/Library/Containers/com.apple.Safari/Data"), "tree", Some("mixed"), Some("B")),
        (Some("~/Library/Mobile Documents/com~apple~CloudDocs"), "tree", Some("mixed"), Some("B")),
        (Some("~/Library/Caches/CloudKit/com.apple.bird"), "tree", Some("system_data"), Some("A")),
        (Some("~/Library/CloudStorage/GoogleDrive"), "tree", Some("system_data"), Some("A")),
        (Some("~/Music/Music/Media.localized/Music"), "tree", Some("com.apple.Music.MusicStorageExtension"), Some("B")),
        (Some("~/Pictures/Photos Library.photoslibrary/originals"), "tree", Some("com.apple.Photos.StorageManagementExtension"), Some("B")),
        (Some("~/Music"), "tree", Some(DOC), Some("A")),
        (Some("~/Music/Music/Music Library.musiclibrary"), "tree", Some("system_data"), Some("A")),
        (Some("~/Movies/TV/Media.localized"), "tree", Some("com.apple.TV.TVStorageExtension"), Some("B")),
        (Some("~/Movies/TV/TV Library.tvlibrary"), "tree", Some("system_data"), Some("A")),
        (Some("~/Movies/Exemplo"), "tree", Some(DOC), Some("A")),
        (Some("~/Pictures"), "tree", Some(DOC), Some("A")),
        (Some("~/Desktop"), "tree", Some("mixed"), Some("B")),
        (Some("~/Documents"), "tree", Some("mixed"), Some("B")),
        (Some("~/Applications"), "tree", Some(APPS), Some("A")),
        (Some("/Applications/Xcode.app/Contents/Developer"), "tree", Some(APPS), Some("A")),
        (Some("~/.colima/_lima"), "tree", Some(DOC), Some("A")),
        (Some("~/Downloads"), "tree", Some(DOC), Some("A")),
        (Some("/Users/Shared"), "tree", Some(OUTROS), Some("A")),
        (Some("/Users/x"), "protected_folders", Some(OUTROS), Some("B")),
        (Some("/Library/Developer/CommandLineTools"), "protected_folders", Some(DEV), Some("A")),
        (Some("~/Library/iTunes/iPhone Software Updates"), "tree", Some("com.apple.STMExtension.iOSFiles"), Some("A")),
        (Some("/Library/Audio/Apple Loops/Apple"), "tree", Some("com.apple.STMExtension.GarageBand"), Some("A")),
        (Some("/Library/Application Support/Logic/Alchemy Samples"), "tree", Some("com.apple.STMExtension.GarageBand"), Some("A")),
        (Some("/Library/Application Support/Logic/Exemplo"), "tree", Some("system_data"), Some("A")),
        (Some("/System/Library/AssetsV2/com_apple_MobileAsset_UAF_FM_GenerativeModels"), "protected_folders", Some("system_data"), Some("A")),
    ];
    for (p, t, l, g) in casos {
        assert_eq!(linhas::linha(p, t), (l, g), "{p:?} {t}");
    }
    // O resto é Dados do Sistema.
    for p in ["/private/tmp", "/opt/homebrew/Cellar", "/Library/Application Support", "/.DocumentRevisions-V100", "/"] {
        assert_eq!(linhas::linha(Some(p), "tree"), (Some("system_data"), Some("A")), "{p}");
    }
    // Linha `folded` traz o pai: a de `~` e a de `~/Library` são mistas; a de `/Applications`, Aplicativos.
    let mut v = json!({"top": [
        {"path": "~", "folded": true, "term": "tree", "bytes": GB},
        {"path": "~/Library", "folded": true, "term": "tree", "bytes": GB},
        {"path": "/Applications", "folded": true, "term": "tree", "bytes": GB},
    ]});
    catalogo::anexar(&mut v);
    let l: Vec<(&Value, &Value)> = v["top"].as_array().unwrap().iter().map(|x| (&x["settings_line"], &x["settings_line_grade"])).collect();
    assert_eq!(l, [(&json!("mixed"), &json!("A")), (&json!("mixed"), &json!("A")), (&json!(APPS), &json!("A"))]);
}

#[test]
fn json_forma() {
    let mut saidas = vec![("completo".to_string(), completo())];
    saidas.extend(fixtures());
    let mut n = 0;
    for (nome, mut v) in saidas {
        catalogo::anexar(&mut v);
        for x in v["top"].as_array().into_iter().flatten() {
            for k in ["settings_line", "settings_line_grade"] {
                assert!(x.get(k).is_some_and(|y| y.is_null() || y.is_string()), "{nome}: {k} em {x}");
            }
            assert_eq!(x["settings_line"].is_null(), x["settings_line_grade"].is_null(), "{nome}: grau sem linha em {x}");
            n += 1;
        }
    }
    assert!(n > 0, "nenhuma linha de top");
}

#[test]
fn rotulo_file_provider() {
    for (v, n) in [
        ("com.apple.CloudDocs.iCloudDriveFileProvider/11111111-2222-4333-8444-555555555555", "iCloud Drive"),
        ("com.google.drivefs.fpext/gdrive-0", "Google Drive"),
        ("com.microsoft.OneDrive.FileProvider/OneDrive", "OneDrive"),
        ("com.microsoft.OneDrive-mac.FileProvider/x", "OneDrive"),
        ("com.getdropbox.dropbox.fileprovider/x", "Dropbox"),
        ("com.exemplo.nuvem/conta", "File Provider"),
        ("", "File Provider"),
    ] {
        assert_eq!(linhas::provedor(v.as_bytes()), n, "{v}");
    }
    // Pasta real com o xattr; a linha `folded` e o que não é <uuid> ficam sem rótulo.
    let home = std::env::temp_dir().join(format!("sdtally-fp-{}", std::process::id()));
    let u = "66666666-7777-4888-9999-AAAAAAAAAAAA";
    let d = home.join("Library/Application Support/FileProvider").join(u);
    std::fs::create_dir_all(&d).unwrap();
    let ok = Command::new("/usr/bin/xattr").args(["-w", "com.apple.file-provider-domain-id", "com.apple.CloudDocs.iCloudDriveFileProvider/x"]).arg(&d).status().unwrap();
    assert!(ok.success());
    let p = format!("~/Library/Application Support/FileProvider/{u}");
    let mut v = json!({"home": home.to_str().unwrap(), "top": [
        {"path": p, "folded": false, "term": "tree", "bytes": GB},
        {"path": p, "folded": true, "term": "tree", "bytes": GB},
        {"path": "~/Library/Application Support/FileProvider/com.google.drivefs.fpext", "folded": false, "term": "tree", "bytes": GB},
        {"path": "~/Library/Application Support/FileProvider/BBBBBBBB-CCCC-4DDD-8EEE-FFFFFFFFFFFF", "folded": false, "term": "tree", "bytes": GB},
    ]});
    catalogo::anexar(&mut v);
    std::fs::remove_dir_all(&home).unwrap();
    let l: Vec<&Value> = v["top"].as_array().unwrap().iter().map(|x| &x["label"]).collect();
    assert_eq!(l, [&json!("iCloud Drive"), &Value::Null, &Value::Null, &json!("File Provider")]);
}

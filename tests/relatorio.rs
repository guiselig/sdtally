//! Relatório a partir de uma conta fixa: texto (golden pt e en, com parcial e não medido),
//! forma do JSON e cancelamento. SDT_GOLDEN=1 regrava os arquivos de tests/golden.

use sdtally::coleta::{self, Opcoes};
use sdtally::ledger::Medida;
use sdtally::report::i18n::Lang;
use sdtally::report::texto::{self, Estilo};
use sdtally::report::{self, Chave, Conta, Item, Processo, Repetidos, Tela};
use sdtally::sources::{self, apfs_meta::ApfsMeta, dirstats::Negadas, lixo::Lixo, snapshots::SnapMais, snapshots::Snapshots};
use sdtally::sys::iokit::EstatVolume;
use sdtally::walk::{self, Controle};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

const GB: i64 = 1_000_000_000;

fn item(nome: &str, termo: Chave, bytes: i64) -> Item {
    Item { nome: nome.into(), termo, bytes, privado: None, parcial: false, quantidade: None, processo: None }
}

/// JSON bruto de `sources::volumes` (forma do diskutil): ISC, container de recuperação, o do
/// Data e um externo. Os marcados no macOS somam 39_823_978_496, o `macos` de `conta()`.
fn volumes_brutos() -> Value {
    let vol = |roles: &[&str], usado: i64, macos: bool, data: bool| json!({"roles": roles, "capacity_in_use": usado, "no_macos_da_tela": macos, "e_o_data_de_boot": data});
    json!([
        {"container": "disk1", "disco_fisico": "disk0", "isc_fora_do_macos": true, "tipo": "interno", "tamanho": 524_288_000, "livre": 502_755_328,
         "volumes": [vol(&["xART"], 6_311_936, false, false), vol(&["Hardware"], 3_948_544, false, false)]},
        {"container": "disk2", "disco_fisico": "disk0", "isc_fora_do_macos": false, "tipo": "interno", "tamanho": 5_368_664_064i64, "livre": 2_782_986_240i64,
         "volumes": [vol(&["Recovery"], 2_500_000_000, true, false), vol(&["Update"], 3_000_000, true, false)]},
        {"container": "disk3", "disco_fisico": "disk0", "isc_fora_do_macos": false, "tipo": "interno", "tamanho": 494_384_795_648i64, "livre": 34_459_226_112i64,
         "volumes": [vol(&["System"], 12 * GB, true, false), vol(&["Preboot"], 10 * GB, true, false), vol(&["Recovery"], 2_900_000_000, true, false),
                     vol(&["Update"], 80_000_000, true, false), vol(&["Data"], 421 * GB, false, true), vol(&["VM"], 12_340_978_496, true, false)]},
        {"container": "disk5", "disco_fisico": "disk4", "isc_fora_do_macos": false, "tipo": "externo", "tamanho": 2_000 * GB, "livre": 1_000 * GB,
         "volumes": [vol(&[], 900 * GB, false, false)]},
    ])
}

fn conta() -> Conta {
    let mut ranking = vec![
        Item { privado: Some(38 * GB), ..item("~/Music", Chave::Arvore, 38_280_000_000) },
        Item { privado: Some(0), ..item("~/Library/Mobile Documents/com~apple~CloudDocs", Chave::Arvore, 36_120_000_000) },
        Item { quantidade: Some(8), ..item("", Chave::Snapshots, 32_180_000_000) },
        item("/System/Library/AssetsV2/com_apple_MobileAsset_UAF_FM_GenerativeModels", Chave::Negadas, 17_400_000_000),
        Item { privado: Some(1), ..item("/private/tmp", Chave::Arvore, 4_130_000_000) },
        Item { parcial: true, processo: Some(("chronod".into(), 190_000_000)), ..item("", Chave::Apagados, 210_000_000) },
        Item { privado: Some(2), ..item("/Library/(demais)", Chave::Arvore, 150_000_000) },
        item("", Chave::Metadados, 1_654_865_920),
    ];
    ranking.sort_by_key(|i| std::cmp::Reverse(i.bytes));
    let arvore: i64 = ranking.iter().filter(|i| i.termo == Chave::Arvore).map(|i| i.bytes).sum();
    let termos = vec![
        (Chave::Arvore, Medida::Valor(arvore)),
        (Chave::Xattr, Medida::NaoMedida("--sem-xattr".into())),
        (Chave::Snapshots, Medida::Valor(32_180_000_000)),
        (Chave::Apagados, Medida::Parcial(210_000_000, "3 de 20 arquivos sem tamanho alocado legível".into())),
        (Chave::Negadas, Medida::Parcial(17_400_000_000, "2 de 30 pastas sem dir-stats".into())),
        (Chave::Metadados, Medida::Valor(1_654_865_920)),
    ];
    let usado = 150 * GB;
    let soma: i64 = termos.iter().filter_map(|t| t.1.valor()).sum();
    Conta {
        ferramenta: "sdtally 0.1.0-teste".into(),
        data_hora: "2026-09-29 14:00".into(),
        macos: Some("27.0".into()),
        boot_time: Some("2026-09-13 09:12".into()),
        root: false,
        fda: Some(true),
        negadas: 30,
        interrompida: false,
        tela: Tela {
            macos: Medida::Valor(39_823_978_496),
            ai: Medida::NaoMedida("nenhum asset no log 'AssetGC determine' das últimas 24 h".into()),
            ai_fisico: Medida::Valor(17_500_000_000),
            dados_do_sistema: Ok(None),
            log_tela: None,
        },
        usado: Medida::Valor(usado),
        termos,
        ranking,
        resto: Some(usado - soma),
        purgeable: Medida::Valor(36_710_000_000),
        purgeable_top: vec![("com.apple.TMCacheDelete".into(), 31_750_000_000), ("com.apple.fspurgeable_data".into(), 4_390_000_000), ("a".into(), 1), ("b".into(), 1)],
        volumes: volumes_brutos(),
        swap: Some((10_737_418_240, 9_463_070_720)),
        snapshots: Some(vec![
            (
                "time_machine",
                Some("2026-09-27 18:15:08".into()),
                Some(30 * GB),
                SnapMais { purgeable: Some(true), cumulativo: Some(31 * GB), criador: Some("com.apple.TimeMachine".into()), papel: Some("reference") },
            ),
            ("time_machine", Some("2026-09-29 08:05:00".into()), Some(GB), SnapMais { purgeable: Some(true), criador: Some("com.apple.TimeMachine".into()), ..Default::default() }),
            ("apple_other", None, Some(2 * GB), SnapMais { purgeable: Some(false), criador: Some("com.apple.os".into()), ..Default::default() }),
            ("other", None, None, SnapMais::default()),
        ]),
        time_machine: json!({"state": "measured", "reason": null, "source": "tm_preferences_plist", "running": false,
            "destinations": [{"reference_at": "2026-09-27 18:15:08", "stable_at": null, "last_completed_at": "2026-09-27 20:29:20",
                              "last_attempt_at": "2026-09-29 13:10:00", "attempts_since_completed": 6, "result_raw": 18}]}),
        processos: Some(vec![Processo { nome: "chronod".into(), bytes: Some(190_000_000), arquivos: 4, sem_alocado: 1, logico_sem_alocado: 5_000_000, dv_launchservices: 3 }]),
        repetidos: Some(Repetidos { clones: (81_998, 26_210_000_000), hardlinks: (28_934, 12_020_000_000), compartilhados: (72_698, 525_000_000), enxertos: 7, dataless: (2_802, 55_620_000_000, 140), parcial: false }),
        erros: vec![],
        tempos: [("arvore".to_string(), json!(1.25)), ("meta".to_string(), json!(0.01))].into_iter().collect(),
        lixo: Some(Ok(Lixo { itens: 12, bytes: 4_560_000_000, velhos_itens: 3, velhos_bytes: 1_230_000_000, ..Lixo::default() })),
        apfs_meta: Some(ApfsMeta {
            medida: Medida::Valor(1_654_865_920),
            detalhe: Some(EstatVolume { bloco: 4096, usado: 421_787_705_344, arquivos: 2_404_065, pastas: 459_473, nos_omap: 6_835, nos_fsroot: 391_006, nos_extentref: 6_179 }),
        }),
        negadas_sem_tamanho: Some(vec![("/Library/Exemplo".into(), None), ("~/Library/Exemplo".into(), Some(1))]),
    }
}

fn estilo(lang: Lang) -> Estilo {
    Estilo { lang, binario: false, cor: false, top: 6 }
}

fn golden(nome: &str, obtido: &str) {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(nome);
    if std::env::var_os("SDT_GOLDEN").is_some() {
        std::fs::write(&p, obtido).expect("gravar golden");
    }
    let esperado = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e} (rode com SDT_GOLDEN=1)", p.display()));
    assert_eq!(obtido, esperado, "{nome} mudou (SDT_GOLDEN=1 regrava depois de conferir)");
}

#[test]
fn texto_golden_pt_en() {
    let c = conta();
    for (lang, nome) in [(Lang::Pt, "conta-pt.txt"), (Lang::En, "conta-en.txt")] {
        let t = texto::conta(&c, &estilo(lang));
        assert!(!t.contains('⟦'), "chave de texto faltando:\n{t}");
        assert!(!t.contains('\x1b'), "cor sem TTY");
        let largas: Vec<&str> = t.lines().filter(|l| l.chars().count() > 80).collect();
        assert!(largas.is_empty(), "linhas com mais de 80 colunas: {largas:#?}");
        golden(nome, &t);
    }
    let pt = texto::conta(&c, &estilo(Lang::Pt));
    // Estados visíveis em texto, não só em cor.
    for s in ["INCOMPLETO (sem sudo)", "xattrs grandes: não medido (--sem-xattr)", "≥0,21", "não medido", "sem registro", "Geral > Armazenamento"] {
        assert!(pt.contains(s), "falta {s:?} em:\n{pt}");
    }
    // Dados do Sistema com registro no log: valor e hora do log.
    let mut com_log = conta();
    com_log.tela.dados_do_sistema = Ok(Some((166_140_000_000, "2026-09-28 14:11".into())));
    let t = texto::conta(&com_log, &estilo(Lang::Pt));
    assert!(t.lines().any(|l| l.contains("Dados do Sistema (log do serviço, 2026-09-28 14:11)") && l.contains("166,14")), "{t}");
    assert_eq!(report::json(&com_log)["settings_screen"]["system_data"]["bytes"], 166_140_000_000i64);
    // Motivos (escritos em pt pelas fontes) traduzidos no en.
    let en = texto::conta(&c, &estilo(Lang::En));
    for s in ["arquivos", "pastas", "nenhum", " de ", "(demais)"] {
        assert!(!en.contains(s), "pt no en: {s:?}\n{en}");
    }
    let gib = texto::conta(&c, &Estilo { binario: true, ..estilo(Lang::En) });
    assert!(gib.contains("GiB = 2^30 bytes") && gib.contains("35.65"), "{gib}");
}

/// Dado ausente some da frase: nada de "?" no lugar do macOS, da contagem de snapshots ou do percentual.
#[test]
fn texto_sem_dado_nao_poe_interrogacao() {
    let mut c = conta();
    c.macos = None;
    c.ranking.iter_mut().filter(|i| i.termo == Chave::Snapshots).for_each(|i| i.quantidade = None);
    for lang in [Lang::Pt, Lang::En] {
        let t = texto::conta(&c, &estilo(lang));
        assert!(!t.contains('?'), "{t}");
        assert!(!t.lines().next().unwrap_or("").contains("macOS"), "cabeçalho sem macOS: {t}");
    }
}

/// Etapa da coleta que falhou (erro registrado ou panic) torna a medição incompleta: antes
/// `complete` só olhava interrupção, sudo e Acesso Total, e saía true com etapa falha.
#[test]
fn etapa_falha_torna_incompleta() {
    let mut c = conta();
    (c.root, c.fda) = (true, Some(true));
    assert_eq!(c.incompleto(), Vec::<&str>::new());
    let j = report::json(&c);
    assert_eq!((&j["complete"], &j["incomplete_reasons"]), (&json!(true), &json!([])));

    c.erros = vec![("snapshots".into(), "fs_snapshot_list errno 1".into())];
    assert_eq!(c.incompleto(), ["stage_failed"]);
    let j = report::json(&c);
    assert_eq!((&j["complete"], &j["incomplete_reasons"]), (&json!(false), &json!(["stage_failed"])));
    assert_eq!(j["stage_errors"][0]["stage"], "snapshots");
    for lang in [Lang::Pt, Lang::En] {
        let t = texto::conta(&c, &estilo(lang));
        assert!(t.contains("INCOMPLETO") || t.contains("INCOMPLETE"), "{t}");
        assert!(!t.contains('⟦'), "chave de texto faltando:\n{t}");
    }
}

#[test]
fn json_forma() {
    let c = conta();
    let j = report::json(&c);
    assert_eq!(j["schema_version"], 1);
    assert_eq!(j["complete"], false);
    assert_eq!(j["incomplete_reasons"], serde_json::json!(["no_root"]));
    let u = &j["data_volume"]["used"];
    assert!(u["bytes"].is_i64() && u["state"] == "measured" && u["source"] == "attr_vol_spaceused");
    let termos = j["data_volume"]["terms"].as_array().expect("terms");
    assert_eq!(termos.iter().map(|t| t["key"].as_str().unwrap_or("")).collect::<Vec<_>>(), ["tree", "large_xattrs", "snapshots", "deleted_open", "protected_folders", "apfs_metadata"]);
    let estados = ["measured", "partial", "not_measured", "not_available"];
    for t in termos {
        assert!(estados.contains(&t["state"].as_str().unwrap_or("")), "{t}");
        assert!(t["bytes"].is_i64() || t["bytes"].is_null(), "bytes inteiro ou null: {t}");
        assert!(t["source"].is_string());
    }
    assert!(termos[1]["bytes"].is_null() && termos[1]["reason"] == "--sem-xattr");
    assert_eq!(termos[3]["state"], "partial");
    // Metadados do APFS: termo da conta (não fica mais dentro de unexplained).
    assert_eq!((termos[5]["bytes"].as_i64(), termos[5]["state"].as_str(), termos[5]["source"].as_str()), (Some(1_654_865_920), Some("measured"), Some("apfs_user_client")));
    let r = &j["data_volume"]["unexplained"];
    assert_eq!((r["state"].as_str(), r["source"].as_str()), (Some("partial"), Some("difference")));
    let soma: i64 = termos.iter().filter_map(|t| t["bytes"].as_i64()).sum();
    assert_eq!(soma + r["bytes"].as_i64().unwrap_or(0), u["bytes"].as_i64().unwrap_or(-1), "conta fecha no JSON");
    assert_eq!(j["top"].as_array().map(|v| v.len()), Some(8), "top tem todas as linhas");
    let top: i64 = j["top"].as_array().into_iter().flatten().filter_map(|t| t["bytes"].as_i64()).sum();
    assert_eq!(top, soma, "Σ top = Σ termos medidos (docs/json-schema.md)");
    assert!(j["top"][0]["bytes"].is_i64() && j["top"][0]["path"] == "~/Music");
    assert_eq!(j["settings_screen"]["system_data"]["state"], "not_measured");
    assert_eq!(j["settings_screen"]["apple_intelligence"]["physical"]["bytes"], 17_500_000_000i64);
    assert_eq!(j["informational"]["purgeable"]["top_services"].as_array().map(|v| v.len()), Some(4), "todos os serviços");
    assert_eq!(j["swap"], json!({"total": 10_737_418_240u64, "used": 9_463_070_720u64, "source": "sysctl_vm_swapusage"}));
    assert_eq!(j["informational"]["repeated_shared_blocks"], json!({"files": 72_698, "bytes": 525_000_000}));
    let p = &j["informational"]["deleted_open_processes"][0];
    assert_eq!((p["name"].as_str(), p["bytes"].as_i64(), p["files"].as_u64()), (Some("chronod"), Some(190_000_000), Some(4)));
    assert_eq!(p["logical_bytes_without_allocated"], 5_000_000);
    assert_eq!(p["launchservices_dv_files"], 3);
    assert_eq!(j["boot_time"], "2026-09-13 09:12");
    assert_eq!(j["stage_seconds"], json!({"arvore": 1.25, "meta": 0.01}), "tempo por etapa vai no JSON");
    let tm = &j["informational"]["time_machine"];
    assert_eq!((tm["state"].as_str(), tm["running"].as_bool(), tm["source"].as_str()), (Some("measured"), Some(false), Some("tm_preferences_plist")));
    assert_eq!(tm["destinations"][0]["attempts_since_completed"], 6);

    // Lixo por idade e metadados do APFS (informativos, com a fonte).
    let lx = &j["informational"]["trash"];
    assert_eq!((lx["threshold_days"].as_i64(), lx["source"].as_str(), lx["scope"].as_str()), (Some(30), Some("getattrlist_addedtime"), Some("~/.Trash")));
    assert_eq!((lx["older"]["bytes"].as_i64(), lx["older"]["items"].as_u64(), lx["older"]["state"].as_str()), (Some(1_230_000_000), Some(3), Some("measured")));
    assert_eq!((lx["total"]["bytes"].as_i64(), lx["total"]["items"].as_u64()), (Some(4_560_000_000), Some(12)));
    assert_eq!(lx["undated"], json!({"items": 0, "bytes": 0}));
    let am = &j["informational"]["apfs_metadata"];
    assert_eq!((am["bytes"].as_i64(), am["state"].as_str(), am["source"].as_str(), am["block_size"].as_u64()), (Some(1_654_865_920), Some("measured"), Some("apfs_user_client"), Some(4096)));
    assert_eq!(am["nodes"], json!({"omap": 6_835, "fsroot": 391_006, "extentref": 6_179}));
    // Não medido: bytes null com o motivo em inglês, nunca 0.
    let mut nm = c.clone();
    nm.lixo = Some(Err("~/.Trash sem permissão (errno 1); falta Acesso Total ao Disco".into()));
    nm.apfs_meta = Some(ApfsMeta { medida: Medida::NaoMedida("seletor 0x1e do AppleAPFSUserClient falhou (0xe00002c2)".into()), detalhe: None });
    let jn = report::json(&nm);
    let (t, a) = (&jn["informational"]["trash"], &jn["informational"]["apfs_metadata"]);
    assert!(t["older"]["bytes"].is_null() && t["older"]["state"] == "not_measured" && t["older"]["items"].is_null() && t["undated"].is_null());
    assert_eq!(t["older"]["reason"], "~/.Trash: no permission (errno 1); Full Disk Access is missing");
    assert!(a["bytes"].is_null() && a["nodes"].is_null() && a["block_size"].is_null());
    assert_eq!(a["reason"], "selector 0x1e of the AppleAPFSUserClient failed (0xe00002c2)");
    let txt = texto::conta(&nm, &estilo(Lang::Pt));
    assert!(txt.contains("Lixo: itens há mais de 30 dias") && txt.contains("não medido") && txt.contains("falta Acesso Total ao Disco"), "{txt}");
    // Etapa que não rodou: null; o termo vira não medido e o resto é um teto.
    nm.lixo = None;
    nm.apfs_meta = None;
    let jn = report::json(&nm);
    assert!(jn["informational"]["trash"].is_null() && jn["informational"]["apfs_metadata"].is_null());

    // Pastas protegidas sem tamanho: count total, folders limitado a 200, errno null quando dir-stats deu 0.
    let ps = &j["informational"]["protected_without_size"];
    assert_eq!(ps, &json!({"count": 2, "folders": [{"path": "/Library/Exemplo", "errno": null}, {"path": "~/Library/Exemplo", "errno": 1}]}));
    let mut muitas = c.clone();
    muitas.negadas_sem_tamanho = Some((0..250).map(|i| (format!("/Library/P{i:03}"), Some(1))).collect());
    let ps = &report::json(&muitas)["informational"]["protected_without_size"];
    assert_eq!((ps["count"].as_u64(), ps["folders"].as_array().map(Vec::len)), (Some(250), Some(200)));
    assert_eq!(ps["folders"][199]["path"], "/Library/P199");
    muitas.negadas_sem_tamanho = None;
    assert!(report::json(&muitas)["informational"]["protected_without_size"].is_null());

    // containers: sem nome de volume nem de disco; internal, isc e holds_data.
    let cs = j["containers"].as_array().expect("containers");
    assert_eq!(cs.len(), 4);
    let cs_txt = serde_json::to_string(cs).unwrap_or_default();
    assert!(!cs_txt.contains("disk") && !cs_txt.contains("\"name\""), "{cs_txt}");
    assert_eq!(cs.iter().map(|c| c["internal"].clone()).collect::<Vec<_>>(), [json!(true), json!(true), json!(true), json!(false)]);
    assert_eq!(cs.iter().map(|c| (c["isc"] == true, c["holds_data"] == true)).collect::<Vec<_>>(), [(true, false), (false, false), (false, true), (false, false)]);
    assert_eq!(cs[2]["ceiling"], 494_384_795_648i64);
    // Invariante do macOS da tela: Σ capacity_in_use dos marcados = settings_screen.macos.
    let soma_macos: i64 = cs.iter().flat_map(|c| c["volumes"].as_array().into_iter().flatten()).filter(|v| v["in_settings_macos"] == true).filter_map(|v| v["capacity_in_use"].as_i64()).sum();
    assert_eq!(Some(soma_macos), j["settings_screen"]["macos"]["bytes"].as_i64());

    // Snapshots: kind nos 3 valores, data só no TM, nenhum nome.
    let sn = j["informational"]["snapshots"].as_array().expect("snapshots");
    assert_eq!(sn.iter().map(|x| x["kind"].as_str().unwrap_or("")).collect::<Vec<_>>(), ["time_machine", "time_machine", "apple_other", "other"]);
    assert!(sn[0]["date"].is_string() && sn[2]["date"].is_null() && sn[3]["date"].is_null() && sn[3]["private_bytes"].is_null());
    let chaves = ["kind", "date", "private_bytes", "purgeable", "cumulative_bytes", "creator", "tm_role", "verdict", "card"];
    for x in sn {
        let mut k: Vec<&str> = x.as_object().map(|o| o.keys().map(String::as_str).collect()).unwrap_or_default();
        k.sort_unstable();
        let mut e = chaves.to_vec();
        e.sort_unstable();
        assert_eq!(k, e, "chaves do snapshot: {x}");
    }
    // Veredito: referência, TM de 5 h (generated_at 2026-09-29 14:00), com.apple fora do TM, terceiro.
    assert_eq!(sn.iter().map(|x| x["verdict"].as_str()).collect::<Vec<_>>(), [Some("tm_reference"), Some("tm_recent"), Some("apple_other"), Some("third_party")]);
    assert_eq!((sn[0]["purgeable"].as_bool(), sn[0]["cumulative_bytes"].as_i64(), sn[0]["tm_role"].as_str()), (Some(true), Some(31 * GB), Some("reference")));
    assert_eq!((sn[2]["creator"].as_str(), sn[2]["purgeable"].as_bool()), (Some("com.apple.os"), Some(false)));
    // Fonte falhou: null, não 0.
    assert!(sn[3]["purgeable"].is_null() && sn[3]["cumulative_bytes"].is_null() && sn[3]["creator"].is_null() && sn[3]["tm_role"].is_null());
    let mut sem = c.clone();
    (sem.snapshots, sem.time_machine, sem.boot_time) = (None, Value::Null, None);
    let js = report::json(&sem);
    assert!(js["informational"]["snapshots"].is_null() && js["informational"]["time_machine"].is_null() && js["boot_time"].is_null());
    // Chaves em inglês: nenhuma com acento ou das palavras em pt usadas no texto.
    let s = serde_json::to_string(&j).unwrap_or_default();
    for k in ["\"bytes_", "\"estado\"", "\"fonte\"", "\"usado\""] {
        assert!(!s.contains(k), "chave em pt: {k}");
    }
}

/// Medição sem Acesso Total (app, 13/10): o dir-stats de ~/Library/Mobile Documents (61,92 GB)
/// conta bytes que a travessia já contou por outro caminho (clones do cache do iCloud), e Σ termos
/// passou do usado. Resto negativo nunca sai como número: vira não medido, com a sobreposição.
#[test]
fn soma_acima_do_usado_nao_vira_resto_negativo() {
    let mut c = conta();
    c.fda = Some(false);
    c.resto = Some(-14_680_000_000);
    let j = report::json(&c);
    let r = &j["data_volume"]["unexplained"];
    assert!(r["bytes"].is_null() && r["percent_of_used"].is_null(), "{r}");
    assert_eq!((r["state"].as_str(), r["source"].as_str()), (Some("not_measured"), Some("difference")));
    assert_eq!(r["overlap_bytes"], 14_680_000_000i64);
    assert!(r["reason"].as_str().is_some_and(|s| s.contains("more than the used space")), "{r}");
    // Resto normal não carrega os campos novos.
    let ok = report::json(&conta())["data_volume"]["unexplained"].clone();
    assert!(ok["overlap_bytes"].is_null() && ok["reason"].is_null() && ok["bytes"].is_i64(), "{ok}");

    for lang in [Lang::Pt, Lang::En] {
        let t = texto::conta(&c, &estilo(lang));
        for neg in ["-14,68", "-14.68", "-9,79", "-9.79"] {
            assert!(!t.contains(neg), "negativo ({neg}) no texto:\n{t}");
        }
    }
    let pt = texto::conta(&c, &estilo(Lang::Pt));
    assert!(pt.contains("14,68") && pt.contains("a mais que o usado"), "falta a sobreposição:\n{pt}");
    assert!(sdtally::conselho::conselhos(&c).iter().all(|a| a.bytes >= 0), "conselho com bytes negativos");
}

#[test]
fn cancelada_antes_da_travessia_sai_incompleta() {
    static JA: AtomicBool = AtomicBool::new(true);
    let (user, home) = coleta::identidade(false);
    let ctl = Arc::new(Controle::new(&JA));
    let c = coleta::coletar(&Opcoes { threads: Some(1), xattr: true, root: false }, &home, Arc::new(sdtally::anon::Ctx::real(&user)), ctl);
    assert!(c.interrompida && c.arvore.is_none());
    let conta = report::montar(&c, "sdtally teste", "2026-09-29 14:00");
    assert_eq!(conta.incompleto()[0], "interrupted");
    assert_eq!(conta.medida(Chave::Arvore), Some(&Medida::NaoMedida("interrompido".into())));
    let t = texto::conta(&conta, &estilo(Lang::Pt));
    assert!(t.contains("INCOMPLETO (interrompido, sem sudo"), "{t}");
    assert_eq!(report::json(&conta)["complete"], false);
}

/// `montar` sobre uma Coleta fabricada (travessia, negadas, snapshots, apagados), não sobre uma
/// Conta feita à mão: Σ top = Σ termos medidos e os parciais chegam ao ranking.
#[test]
fn montar_com_coleta_sintetica() {
    static JA: AtomicBool = AtomicBool::new(true);
    let (user, home) = coleta::identidade(false);
    let ctx = Arc::new(sdtally::anon::Ctx::real(&user));
    let mut c = coleta::coletar(&Opcoes { threads: Some(1), xattr: true, root: true }, &home, ctx, Arc::new(Controle::new(&JA)));
    c.interrompida = false;
    let soma = |alloc: i64| walk::Sum { files: 1, alloc, private: alloc / 2, ..Default::default() };
    let mut r = walk::Result { xattr_erros: 2, clone_dup_files: 3, clone_dup_bytes: 30, graft_roots: 1, ..Default::default() };
    // /private/tmp abaixo de 100 MB: dobra no balde "(demais)" do pai.
    for (k, b) in [("~/Music", 5 * GB), ("~/Library/Caches", 2 * GB), ("/private/tmp", 50_000_000)] {
        r.total.add(&soma(b));
        r.buckets.insert(k.into(), soma(b));
    }
    r.total.xattr_stream = 7_000_000;
    c.arvore = Some(r);
    let neg = Medida::Parcial(GB, "1 de 2 pastas sem dir-stats".into());
    c.negadas = Some(Negadas { json: Value::Null, total: neg.clone(), baldes: vec![("/System/Library/AssetsV2/x".into(), GB)], sem_tamanho: vec![] });
    c.negadas_total = neg;
    c.snap_total = Medida::Valor(3 * GB);
    c.snaps = Some(Snapshots {
        json: json!({"quantidade": 2}),
        total: c.snap_total.clone(),
        itens: vec![("time_machine", Some("2026-09-30 09:00:00".into()), Some(GB), SnapMais::default())],
        tm: json!({"state": "not_measured"}),
    });
    c.apagados = Medida::Parcial(400_000_000, "1 de 3 arquivos sem tamanho alocado legível".into());
    let mut pp = vec![json!({"processo": "chronod", "bytes": 300_000_000, "arquivos": 2, "sem_alocado": 1, "logico_sem_alocado": 7})];
    pp.extend((0..11).map(|i| json!({"processo": format!("p{i}"), "bytes": 1000 - i, "arquivos": 1, "sem_alocado": 0, "logico_sem_alocado": 0})));
    c.lsof = json!({"por_processo": pp});
    c.usado = Medida::Valor(20 * GB);

    // Etapa dos metadados sem resultado: termo não medido, fora da soma (o resto fica um teto).
    assert!(c.apfs_meta.is_none());
    let sem_meta = report::montar(&c, "sdtally teste", "2026-09-30 10:00");
    assert_eq!(sem_meta.termos.len(), 6);
    assert_eq!(sem_meta.medida(Chave::Metadados), Some(&Medida::NaoMedida("falha interna".into())));
    assert!(sem_meta.ranking.iter().all(|i| i.termo != Chave::Metadados));
    let meta = 2 * GB;
    c.apfs_meta = Some(ApfsMeta { medida: Medida::Valor(meta), detalhe: None });
    let conta = report::montar(&c, "sdtally teste", "2026-09-30 10:00");
    assert_eq!(conta.medida(Chave::Metadados), Some(&Medida::Valor(meta)));
    assert_eq!(conta.resto, sem_meta.resto.map(|r| r - meta), "unexplained = used − Σ termos medidos, com os metadados");
    assert_eq!(conta.termos.last().map(|t| t.0), Some(Chave::Metadados));
    assert_eq!(conta.medida(Chave::Arvore), Some(&Medida::Valor(7_050_000_000)));
    assert_eq!(conta.medida(Chave::Xattr), Some(&Medida::Parcial(7_000_000, "2 objetos com xattr ilegível".into())));
    let medidos: i64 = conta.termos.iter().filter_map(|t| t.1.valor()).sum();
    assert_eq!(conta.ranking.iter().map(|i| i.bytes).sum::<i64>(), medidos, "Σ top = Σ termos medidos");
    assert_eq!(conta.resto, Some(20 * GB - medidos));
    let item = |k: Chave| conta.ranking.iter().find(|i| i.termo == k).expect("item");
    assert!(item(Chave::Apagados).parcial && item(Chave::Xattr).parcial && !item(Chave::Snapshots).parcial);
    assert_eq!(item(Chave::Snapshots).quantidade, Some(2));
    assert_eq!(item(Chave::Apagados).processo, Some(("chronod".into(), 300_000_000)));
    assert!(conta.ranking.iter().any(|i| i.nome.ends_with("(demais)")), "{:?}", conta.ranking);
    let procs = conta.processos.as_ref().expect("processos");
    assert_eq!(procs.len(), 10, "os 10 maiores");
    assert_eq!(procs[0], Processo { nome: "chronod".into(), bytes: Some(300_000_000), arquivos: 2, sem_alocado: 1, logico_sem_alocado: 7, dv_launchservices: 0 });
    assert_eq!(conta.snapshots.as_ref().map(Vec::len), Some(1));
    assert_eq!(conta.time_machine["state"], "not_measured");
    let rep = conta.repetidos.as_ref().expect("repetidos");
    assert_eq!((rep.clones, rep.enxertos, rep.parcial), ((3, 30), 1, false));

    // Travessia interrompida: árvore e xattr viram piso com motivo, e o informativo também.
    let r = c.arvore.as_mut().expect("arvore");
    (r.interrompida, r.xattr_erros) = (true, 0);
    let conta = report::montar(&c, "sdtally teste", "2026-09-30 10:00");
    assert_eq!(conta.medida(Chave::Arvore), Some(&Medida::Parcial(7_050_000_000, "interrompido".into())));
    assert_eq!(conta.medida(Chave::Xattr), Some(&Medida::Parcial(7_000_000, "interrompido".into())));
    assert!(conta.repetidos.expect("repetidos").parcial);
    // Pasta ou entrada não lida (ENAMETOOLONG, panic) deixa a árvore parcial; EPERM/EACCES não
    // (vão para o dir-stats).
    let r = c.arvore.as_mut().expect("arvore");
    r.interrompida = false;
    r.errors.extend([(63, 2), (sdtally::walk::ERRO_INTERNO, 1), (1, 5), (13, 4), (2, 7), (20, 1)]);
    let conta = report::montar(&c, "sdtally teste", "2026-09-30 10:00");
    assert_eq!(conta.medida(Chave::Arvore), Some(&Medida::Parcial(7_050_000_000, "3 pastas/entradas não lidas".into())));
    // EPERM/EACCES (dir-stats) e ENOENT/ENOTDIR (sumiu entre a listagem e o open): árvore medida.
    c.arvore.as_mut().expect("arvore").errors.retain(|k, _| [1, 13, 2, 20].contains(k));
    assert_eq!(report::montar(&c, "t", "t").medida(Chave::Arvore), Some(&Medida::Valor(7_050_000_000)));
    c.xattr = false;
    assert_eq!(report::montar(&c, "t", "t").medida(Chave::Xattr), Some(&Medida::NaoMedida("--sem-xattr".into())));
}

/// O `top_process` do termo deleted_open é o de maior alocado medido, não o primeiro da lista
/// (que vem por max(alocado, lógico)).
#[test]
fn top_process_maior_alocado() {
    static JA: AtomicBool = AtomicBool::new(true);
    let (user, home) = coleta::identidade(false);
    let mut c = coleta::coletar(&Opcoes { threads: Some(1), xattr: false, root: false }, &home, Arc::new(sdtally::anon::Ctx::real(&user)), Arc::new(Controle::new(&JA)));
    c.interrompida = false;
    c.apagados = Medida::Parcial(113_270_000, "1 de 3 arquivos sem tamanho alocado legível".into());
    c.lsof = json!({"por_processo": [
        {"processo": "linkd", "bytes": 3_770_000, "arquivos": 2, "sem_alocado": 1, "logico_sem_alocado": 1_260_000_000},
        {"processo": "chronod", "bytes": null, "arquivos": 1, "sem_alocado": 1, "logico_sem_alocado": 900_000_000},
        {"processo": "Preview", "bytes": 109_500_000, "arquivos": 1, "sem_alocado": 0, "logico_sem_alocado": 0},
    ]});
    let conta = report::montar(&c, "t", "2026-10-03 10:00");
    let item = conta.ranking.iter().find(|i| i.termo == Chave::Apagados).expect("item");
    assert_eq!(item.processo, Some(("Preview".into(), 109_500_000)));
    let j = report::json(&conta);
    let t = j["top"].as_array().into_iter().flatten().find(|t| t["term"] == "deleted_open").expect("top");
    assert_eq!(t["top_process"], json!({"name": "Preview", "bytes": 109_500_000}));
    // A lista mantém a ordem da fonte.
    assert_eq!(j["informational"]["deleted_open_processes"][0]["name"], "linkd");
    // Nenhum medido: sem top_process.
    c.lsof = json!({"por_processo": [{"processo": "chronod", "bytes": null, "arquivos": 1, "sem_alocado": 1, "logico_sem_alocado": 9}]});
    assert_eq!(report::montar(&c, "t", "t").ranking.iter().find(|i| i.termo == Chave::Apagados).and_then(|i| i.processo.clone()), None);
}

#[test]
fn tipo_do_snapshot_pelo_prefixo() {
    use sources::snapshots::tipo;
    assert_eq!(tipo("com.apple.TimeMachine.2026-10-03-080500.local"), ("time_machine", Some("2026-10-03 08:05:00".into())));
    assert_eq!(tipo("com.apple.TimeMachine.estranho.local"), ("time_machine", None));
    assert_eq!(tipo("com.apple.os.update-ABCDEF0123456789"), ("apple_other", None));
    assert_eq!(tipo("meu-backup-pessoal"), ("other", None));
}

/// Neste Mac (e na VM do CI): a marca `in_settings_macos` soma exatamente o macOS da tela.
#[test]
fn macos_da_tela_igual_soma_dos_containers() {
    let v = sources::volumes::coletar(&mut Vec::new());
    let Medida::Valor(macos) = v.macos else { panic!("macOS não medido: {:?}", v.macos) };
    let cs = report::containers_json(&v.json);
    let soma: i64 = cs.as_array().into_iter().flatten().flat_map(|c| c["volumes"].as_array().into_iter().flatten()).filter(|v| v["in_settings_macos"] == true).filter_map(|v| v["capacity_in_use"].as_i64()).sum();
    assert_eq!(soma, macos);
    assert!(cs.as_array().is_some_and(|c| c.iter().any(|c| c["holds_data"] == true && c["internal"] == true)), "{cs}");
}

/// Ramo com entrada não lida: piso, não completo (antes saía `complete: true`).
#[test]
fn ramo_com_erro_de_leitura_e_piso() {
    let mut r = sdtally::detalhe::Ramo { raiz: "/tmp/x".into(), ..Default::default() };
    let j = report::json_ramo(&r, "/Users/x", "t");
    assert_eq!((j["complete"].as_bool(), j["total"]["lower_bound"].as_bool()), (Some(true), Some(false)));
    // Só entradas que sumiram: contam em read_errors, mas o total continua medido.
    r.erros = 3;
    let j = report::json_ramo(&r, "/Users/x", "t");
    assert_eq!((j["complete"].as_bool(), j["total"]["lower_bound"].as_bool(), j["read_errors"].as_u64()), (Some(true), Some(false), Some(3)));
    r.nao_lidos = 1;
    let j = report::json_ramo(&r, "/Users/x", "t");
    assert_eq!((j["complete"].as_bool(), j["total"]["lower_bound"].as_bool()), (Some(false), Some(true)));
}

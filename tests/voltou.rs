//! "Por que o espaço não voltou" (PLANO Fase 3): só causa com número medido; o resto some.

use sdtally::ledger::Medida;
use sdtally::report::{Chave, Conta, Item, Repetidos, Tela};
use sdtally::voltou;
use serde_json::Value;

const GB: i64 = 1_000_000_000;

fn item(nome: &str, termo: Chave, bytes: i64, quantidade: Option<usize>) -> Item {
    Item { nome: nome.into(), termo, bytes, privado: Some(bytes), parcial: false, quantidade, processo: None }
}

fn base() -> Conta {
    Conta {
        ferramenta: "sdtally 0.1.0-teste".into(),
        data_hora: "2026-10-18 10:00".into(),
        macos: Some("27.0".into()),
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
        ranking: vec![],
        resto: Some(GB),
        purgeable: Medida::Valor(0),
        purgeable_top: vec![],
        volumes: Value::Null,
        swap: None,
        snapshots: None,
        time_machine: Value::Null,
        processos: None,
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

fn ids(c: &Conta) -> Vec<&'static str> {
    voltou::causas(c).iter().map(|x| x.id).collect()
}

fn termo(c: &mut Conta, k: Chave, m: Medida) {
    c.termos.iter_mut().find(|t| t.0 == k).expect("termo").1 = m;
}

#[test]
fn sem_medida_nao_aparece() {
    assert!(ids(&base()).is_empty());
    // Abaixo de 1 GB também some.
    let mut c = base();
    termo(&mut c, Chave::Snapshots, Medida::Valor(GB - 1));
    c.repetidos = Some(Repetidos { clones: (3, 900_000_000), dataless: (9, 100, 1), ..Default::default() });
    assert!(ids(&c).is_empty());
}

#[test]
fn nao_medido_nunca_vira_causa() {
    let mut c = base();
    termo(&mut c, Chave::Snapshots, Medida::NaoMedida("sem fsctl".into()));
    termo(&mut c, Chave::Apagados, Medida::NaoMedida("sem lsof".into()));
    termo(&mut c, Chave::Arvore, Medida::NaoMedida("interrompido".into()));
    c.purgeable = Medida::NaoMedida("cachedelete".into());
    c.ranking = vec![item("~/.Trash", Chave::Arvore, 50 * GB, None)];
    assert!(ids(&c).is_empty());
}

#[test]
fn todas_as_causas_ordenadas() {
    let mut c = base();
    termo(&mut c, Chave::Snapshots, Medida::Valor(30 * GB));
    termo(&mut c, Chave::Apagados, Medida::Parcial(2 * GB, "sem lsof como usuário".into()));
    c.purgeable = Medida::Valor(5 * GB);
    c.repetidos = Some(Repetidos { clones: (10, 26 * GB), dataless: (7, 55 * GB, 2), parcial: true, ..Default::default() });
    c.ranking = vec![item("~/.Trash", Chave::Arvore, 3 * GB, None), item("~/.Trash/…", Chave::Arvore, 1, None), item("", Chave::Snapshots, 30 * GB, Some(4))];
    let v = voltou::causas(&c);
    let got: Vec<_> = v.iter().map(|x| (x.id, x.bytes, x.piso)).collect();
    assert_eq!(
        got,
        [("cloud_only", 55 * GB, true), ("local_snapshot", 30 * GB, false), ("shared_clone", 26 * GB, true), ("purgeable", 5 * GB, false), ("trash", 3 * GB + 1, false), ("deleted_open", 2 * GB, true)]
    );
    assert_eq!(v.iter().find(|x| x.id == "local_snapshot").and_then(|x| x.quantidade), Some(4));
    let j = voltou::json(&v);
    assert_eq!(j[0]["id"], "cloud_only");
    assert_eq!(j[0]["source"], "getattrlistbulk");
    assert_eq!(j[1]["bytes_floor"], false);
    for x in j.as_array().unwrap() {
        assert!(x["text"]["pt"].as_str().is_some_and(|s| !s.is_empty()) && x["title"]["en"].as_str().is_some_and(|s| !s.is_empty()));
    }
}

#[test]
fn fixture_do_app_usa_os_textos_do_rust() {
    let f: Value = serde_json::from_str(include_str!("../app/Tests/SDTallyTests/Fixtures/completo.json")).expect("json");
    let lista = f["not_freed"].as_array().expect("not_freed");
    assert!(!lista.is_empty());
    for x in lista {
        let id = x["id"].as_str().expect("id");
        let [(tp, te), (dp, de)] = voltou::texto(id).unwrap_or_else(|| panic!("{id}"));
        assert_eq!((x["title"]["pt"].as_str(), x["title"]["en"].as_str(), x["text"]["pt"].as_str(), x["text"]["en"].as_str()), (Some(tp), Some(te), Some(dp), Some(de)), "{id}");
    }
}

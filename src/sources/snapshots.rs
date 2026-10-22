//! Snapshots locais do volume de boot: lista (fs_snapshot_list) e tamanho (fsctl 0xC0184A13).
//! Só o Data de boot. Nunca /Volumes: o destino do backup travou no fsctl (síntese §4.4).

use super::DATA;
use crate::ledger::Medida;
use crate::sys;
use serde_json::{Value, json};

pub struct Snapshots {
    pub json: Value,
    /// Blocos só em snapshot, faixa do mais antigo ao mais novo.
    pub total: Medida,
    /// Cada snapshot, do mais antigo ao mais novo.
    pub itens: Vec<Snap>,
    /// Bloco `informational.time_machine` (spec fase 6 §3.3), já com as chaves do JSON.
    pub tm: Value,
}

/// (tipo, data, privado, resto) de um snapshot.
pub type Snap = (&'static str, Option<String>, Option<i64>, SnapMais);

/// Campos da fase 6 de cada snapshot (`null` no JSON quando a fonte falha).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapMais {
    /// Do diskutil apfs listSnapshots, junção por SnapshotXID.
    pub purgeable: Option<bool>,
    pub cumulativo: Option<i64>,
    /// Até 3 componentes reverse-DNS do nome (`criador`).
    pub criador: Option<String>,
    /// "reference" ou "stable": a data do snapshot TM bate com a do destino no plist.
    pub papel: Option<&'static str>,
}

const PLIST_TM: &str = "/Library/Preferences/com.apple.TimeMachine.plist";

/// Até os 3 primeiros componentes de um nome reverse-DNS (`com.apple.TimeMachine`,
/// `com.apple.os`, `com.bombich.ccc`), cada um `[A-Za-z0-9-]`; senão None.
pub fn criador(nome: &str) -> Option<String> {
    let p: Vec<&str> = nome.split('.').take(3).collect();
    let ok = p.len() >= 2 && p.iter().all(|c| !c.is_empty() && c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
    ok.then(|| p.join("."))
}

/// "AAAA-MM-DD[T ]HH:MM[:SS][Z]" → segundos do calendário civil (com Z, é a época Unix). Sem
/// fuso: serve para diferença entre duas datas locais.
pub(crate) fn segundos_civis(s: &str) -> Option<i64> {
    let s = s.strip_suffix('Z').unwrap_or(s);
    let n = |a: usize, b: usize| s.get(a..b).filter(|x| x.bytes().all(|c| c.is_ascii_digit())).and_then(|x| x.parse::<i64>().ok());
    let sep = |i: usize, c: &[u8]| s.as_bytes().get(i).is_some_and(|x| c.contains(x));
    if !(sep(4, b"-") && sep(7, b"-") && sep(10, b"T ") && sep(13, b":") && matches!(s.len(), 16 | 19) && (s.len() == 16 || sep(16, b":"))) {
        return None;
    }
    let (y, m, d, h, mi) = (n(0, 4)?, n(5, 7)?, n(8, 10)?, n(11, 13)?, n(14, 16)?);
    let seg = if s.len() == 19 { n(17, 19)? } else { 0 };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || h > 23 || mi > 59 || seg > 60 {
        return None;
    }
    // days_from_civil (H. Hinnant).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let dias = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    Some(dias * 86_400 + h * 3600 + mi * 60 + seg)
}

/// Idade em horas inteiras entre duas datas locais (`generated_at` − `date`).
/// ponytail: aritmética civil, erra 1 h na semana da troca de horário de verão; mktime se importar.
pub fn idade_h(agora: &str, data: &str) -> Option<i64> {
    Some((segundos_civis(agora)? - segundos_civis(data)?).div_euclid(3600))
}

/// Situação do snapshot (spec fase 6 §4.4), por prioridade. TM sem data: None.
pub fn veredito(kind: &str, papel: Option<&str>, idade_h: Option<i64>) -> Option<&'static str> {
    match (kind, papel, idade_h) {
        ("time_machine", Some("reference"), _) => Some("tm_reference"),
        ("time_machine", Some("stable"), _) => Some("tm_stable"),
        ("time_machine", _, Some(h)) if h < 24 => Some("tm_recent"),
        ("time_machine", _, Some(_)) => Some("tm_aged"),
        ("time_machine", ..) => None,
        ("apple_other", ..) => Some("apple_other"),
        _ => Some("third_party"),
    }
}

/// Valor de `k = v;` na saída do `tmutil status`.
pub fn campo(st: &str, k: &str) -> Option<String> {
    st.lines().find_map(|l| {
        let l = l.trim();
        l.strip_prefix(k).and_then(|r| r.trim_start().strip_prefix('=')).map(|v| v.trim().trim_end_matches(';').trim_matches('"').to_string())
    })
}

/// `Running = 1` → true, `0` → false; ausente ou falha do tmutil → None.
pub fn running(st: Result<&str, &str>) -> Option<bool> {
    match campo(st.ok()?, "Running")?.as_str() {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

/// As `<date>` de um `plutil -extract … xml1` (UTC) → segundos Unix.
pub(crate) fn datas_xml(x: &str) -> Vec<i64> {
    x.lines().filter_map(|l| l.trim().strip_prefix("<date>")?.strip_suffix("</date>")).filter_map(segundos_civis).collect()
}

/// Bloco `time_machine` pelo plist, chave por chave (`sys::plist_chave`). Datas em hora local.
fn time_machine(running: Option<bool>) -> Value {
    let base = |state: &str, reason: Option<&str>, dest: Value| json!({"state": state, "reason": reason, "source": "tm_preferences_plist", "running": running, "destinations": dest});
    if let Err(e) = std::fs::File::open(PLIST_TM) {
        let r = match e.kind() {
            std::io::ErrorKind::NotFound => "Time Machine preferences not found",
            _ => "plist unreadable",
        };
        return base("not_measured", Some(r), Value::Null);
    }
    let raw = |k: &str| sys::plist_chave(PLIST_TM, k, "raw").ok().map(|s| s.trim().to_string());
    let datas = |k: &str| sys::plist_chave(PLIST_TM, k, "xml1").ok().map(|x| datas_xml(&x));
    // Sem a chave: nenhum destino configurado. Teto de 8 para não rodar plutil sem fim.
    let n = raw("Destinations").and_then(|s| s.parse::<usize>().ok()).unwrap_or(0).min(8);
    let dest: Vec<Value> = (0..n)
        .map(|i| {
            let k = |c: &str| format!("Destinations.{i}.{c}");
            let data = |c: &str| raw(&k(c)).as_deref().and_then(segundos_civis).map(sys::local);
            let feito = datas(&k("SnapshotDates")).and_then(|v| v.into_iter().max());
            let tentativas = datas(&k("AttemptDates"));
            json!({
                "reference_at": data("ReferenceLocalSnapshotDate"),
                "stable_at": data("StableLocalSnapshotDate"),
                "last_completed_at": feito.map(sys::local),
                "last_attempt_at": tentativas.as_ref().and_then(|v| v.iter().max()).map(|t| sys::local(*t)),
                "attempts_since_completed": feito.zip(tentativas.as_ref()).map(|(f, v)| v.iter().filter(|t| **t > f).count()),
                "result_raw": raw(&k("RESULT")).and_then(|s| s.parse::<i64>().ok()),
            })
        })
        .collect();
    base("measured", None, json!(dest))
}

/// SnapshotXID → Purgeable, pelo diskutil. Nome e UUID ficam no Value descartado aqui.
fn purgeaveis(err: &mut Vec<Value>) -> std::collections::HashMap<u64, bool> {
    match sys::plist_json("/usr/sbin/diskutil", &["apfs", "listSnapshots", DATA, "-plist"], 30) {
        Ok(v) => v["Snapshots"].as_array().into_iter().flatten().filter_map(|s| Some((s["SnapshotXID"].as_u64()?, s["Purgeable"].as_bool()?))).collect(),
        Err(e) => {
            err.push(json!(format!("diskutil apfs listSnapshots: {e}")));
            Default::default()
        }
    }
}

/// Tipo pelo prefixo do nome: time_machine (com a data do nome, hora local), apple_other
/// (com.apple.*, ex.: atualização do sistema) ou other. O nome em si não sai daqui.
pub fn tipo(nome: &str) -> (&'static str, Option<String>) {
    match nome.strip_prefix("com.apple.TimeMachine.") {
        Some(r) => {
            // 2026-10-03-080500.local → 2026-10-03 08:05:00
            let d = r.split('.').next().unwrap_or("");
            let b = d.as_bytes();
            let ok = b.len() == 17 && b.iter().enumerate().all(|(i, c)| if matches!(i, 4 | 7 | 10) { *c == b'-' } else { c.is_ascii_digit() });
            ("time_machine", ok.then(|| format!("{} {}:{}:{}", &d[..10], &d[11..13], &d[13..15], &d[15..])))
        }
        None if nome.starts_with("com.apple.") => ("apple_other", None),
        None => ("other", None),
    }
}

pub fn coletar(err: &mut Vec<Value>) -> Snapshots {
    let st = sys::run("/usr/bin/tmutil", &["status"], 30);
    let campo = |k: &str| campo(st.as_ref().ok()?, k);
    let tm = time_machine(running(st.as_deref().map_err(|e| e.as_str())));
    // (data local, papel) de cada destino; a data do snapshot TM vem do nome, também local.
    let papeis: Vec<(String, &'static str)> = tm["destinations"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|d| [("reference_at", "reference"), ("stable_at", "stable")].map(|(k, p)| d[k].as_str().map(|s| (s.to_string(), p))))
        .flatten()
        .collect();
    let papel = |data: &Option<String>| ["reference", "stable"].into_iter().find(|p| papeis.iter().any(|(d, q)| q == p && Some(d) == data.as_ref()));
    let destinos: Value = match sys::plist_json("/usr/bin/tmutil", &["destinationinfo", "-X"], 30) {
        Ok(v) => {
            let kinds: Vec<Value> = v["Destinations"]
                .as_array()
                .map(|d| {
                    d.iter()
                        .map(|x| json!(match x["Kind"].as_str() { Some("Local") => "local", Some("Network") => "rede", Some(_) => "outro", None => "desconhecido" }))
                        .collect()
                })
                .unwrap_or_default();
            json!({"configurado": !kinds.is_empty(), "tipos": kinds})
        }
        Err(e) => json!({"configurado": Value::Null, "erro": e}),
    };
    let (mut lista, lista_erro) = match sys::apfs::snapshots(DATA) {
        Ok(l) => (l, None),
        Err(e) => {
            err.push(json!(format!("fs_snapshot_list errno {e}")));
            (Vec::new(), Some(e))
        }
    };
    lista.sort_by_key(|x| x.1);
    // O fsctl leva segundos a frio e a faixa de um snapshot só é o próprio {x,x}: mesma pergunta,
    // uma chamada.
    let mut feitos: std::collections::HashMap<(u64, u64), Result<u64, i32>> = std::collections::HashMap::new();
    let mut privado = |de: u64, ate: u64| *feitos.entry((de, ate)).or_insert_with(|| sys::apfs::snap_private(DATA, de, ate));
    let purg = if lista.is_empty() { Default::default() } else { purgeaveis(err) };
    let mut resumo = Vec::new();
    let mut itens = Vec::new();
    for (nome, xid) in &lista {
        let (t, data) = tipo(nome);
        let mut mais = SnapMais { criador: criador(nome), papel: if t == "time_machine" { papel(&data) } else { None }, ..Default::default() };
        let Some(xid) = *xid else {
            err.push(json!("snapshot sem XID (OBJID não devolvido)"));
            resumo.push((t, data.clone(), None, mais));
            itens.push(json!({"tipo": t, "data": data, "privado": null, "privado_erro": "sem XID", "cumulativo_0_ate_xid": null, "cumulativo_erro": "sem XID"}));
            continue;
        };
        let p = privado(xid, xid);
        let c = privado(0, xid);
        mais.purgeable = purg.get(&xid).copied();
        mais.cumulativo = c.as_ref().ok().map(|v| *v as i64);
        resumo.push((t, data.clone(), p.as_ref().ok().map(|v| *v as i64), mais));
        itens.push(json!({
            "tipo": t, "data": data,
            "privado": p.as_ref().ok(), "privado_erro": p.err(),
            "cumulativo_0_ate_xid": c.as_ref().ok(), "cumulativo_erro": c.err(),
        }));
    }
    // Lista ilegível não é "zero snapshots".
    let (faixa, total) = match (lista_erro, lista.first(), lista.last()) {
        (Some(e), ..) => (json!({"erro_errno": e}), Medida::NaoMedida(format!("fs_snapshot_list errno {e}"))),
        (None, Some((_, Some(lo))), Some((_, Some(hi)))) => match privado(*lo, *hi) {
            Ok(v) => (json!(v), Medida::Valor(v as i64)),
            Err(e) => (json!({"erro_errno": e}), Medida::NaoMedida(format!("fsctl errno {e}"))),
        },
        (None, None, _) => (json!(0), Medida::Valor(0)),
        // Ordenado por XID: None vem primeiro.
        _ => (json!({"erro": "snapshot sem XID"}), Medida::NaoMedida("snapshot sem XID".into())),
    };
    let j = json!({
        "quantidade": lista.len(),
        "lista": itens,
        "faixa_total": faixa,
        "fonte": "fs_snapshot_list + fsctl 0xC0184A13 {from_xid,to_xid}: privado {x,x}, cumulativo {0,x}, faixa {mais_antigo, mais_novo}",
        "time_machine": {
            "running": campo("Running"),
            "backup_phase": campo("BackupPhase"),
            "destino": destinos,
            "status_erro": st.err(),
        },
    });
    Snapshots { json: j, total, itens: resumo, tm }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn veredito_referencia_estavel_idade_terceiro() {
        assert_eq!(veredito("time_machine", Some("reference"), Some(40)), Some("tm_reference"));
        assert_eq!(veredito("time_machine", Some("stable"), Some(2)), Some("tm_stable"));
        assert_eq!(veredito("time_machine", None, Some(23)), Some("tm_recent"));
        assert_eq!(veredito("time_machine", None, Some(24)), Some("tm_aged"));
        assert_eq!(veredito("apple_other", None, None), Some("apple_other"));
        assert_eq!(veredito("other", None, None), Some("third_party"));
    }

    #[test]
    fn veredito_tm_sem_data() {
        assert_eq!(veredito("time_machine", None, None), None);
        assert_eq!(idade_h("2026-10-03 10:00", "estranho"), None);
        assert_eq!(idade_h("2026-10-03 10:00", "2026-10-01 18:15:08"), Some(39));
        assert_eq!(idade_h("2026-03-01 00:00", "2026-02-28 00:00:00"), Some(24));
    }

    #[test]
    fn datas_xml1_para_local() {
        let x = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\">\n<array>\n\t<date>2026-09-19T04:11:07Z</date>\n\t<date>2026-10-01T23:29:20Z</date>\n</array>\n</plist>\n";
        let v = datas_xml(x);
        assert_eq!(v, [1_789_791_067, 1_790_897_360]);
        assert_eq!(segundos_civis("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(segundos_civis("2000-02-29T12:00:00Z"), Some(951_825_600));
        assert_eq!(segundos_civis("2026-13-01T00:00:00Z"), None);
        // Hora local: mesmo formato e mesmos segundos (todo fuso é múltiplo de minuto).
        let l = sys::local(v[1]);
        assert_eq!((l.len(), &l[4..5], &l[10..11], &l[17..]), (19, "-", " ", "20"), "{l}");
        assert!(segundos_civis(&l).is_some_and(|c| (c - v[1]).abs() <= 14 * 3600), "{l}");
    }

    #[test]
    fn running_do_tmutil_status() {
        let st = |r: &str| format!("Backup session status:\n{{\n    ClientID = \"com.apple.backupd\";\n    Percent = \"-1\";\n    Running = {r};\n}}\n");
        assert_eq!(running(Ok(&st("0"))), Some(false));
        assert_eq!(running(Ok(&st("1"))), Some(true));
        assert_eq!(running(Ok(&st("x"))), None);
        assert_eq!(running(Ok("Backup session status:\n{\n}\n")), None);
        assert_eq!(running(Err("/usr/bin/tmutil: saiu com erro")), None);
        assert_eq!(campo(&st("1"), "Percent").as_deref(), Some("-1"));
    }

    #[test]
    fn criador_reverse_dns() {
        assert_eq!(criador("com.apple.TimeMachine.2026-10-01-181508.local").as_deref(), Some("com.apple.TimeMachine"));
        assert_eq!(criador("com.apple.os.update-ABCDEF0123456789").as_deref(), Some("com.apple.os"));
        assert_eq!(criador("com.bombich.ccc.2026").as_deref(), Some("com.bombich.ccc"));
        for n in ["meu-backup-pessoal", "com..x", "com.ap ple.x", ".com.x", "com.é.x"] {
            assert_eq!(criador(n), None, "{n}");
        }
    }
}

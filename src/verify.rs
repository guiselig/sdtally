//! `sdtally --verify` (spec fase 6 §3.5 e §4.6): confere, lado a lado e agora, os números que um
//! comando da Apple mostra igual. Cada conferência é um sanduíche: Apple, sdtally, Apple de novo.
//! Só leitura, sem root, nada gravado.

use crate::catalogo::FONTES;
use crate::report::i18n::Lang;
use crate::sources::{DATA, snapshots};
use crate::{historico, sys};
use serde_json::{Value, json};

const MIB: u64 = 1_048_576;

/// `Used` (KiB) da 2ª linha do `df -k <volume>`.
pub fn df_usado_kib(s: &str) -> Option<u64> {
    s.lines().nth(1)?.split_whitespace().nth(2)?.parse().ok()
}

/// `total` do `sysctl vm.swapusage` em centésimos de MiB ("10240.00M" → 1024000).
pub fn swap_total_cmib(s: &str) -> Option<u64> {
    let v = s.split_once("total = ")?.1.split('M').next()?;
    let (i, f) = v.split_once('.')?;
    let dig = |x: &str| !x.is_empty() && x.bytes().all(|c| c.is_ascii_digit());
    if !(dig(i) && f.len() == 2 && dig(f)) {
        return None;
    }
    Some(i.parse::<u64>().ok()? * 100 + f.parse::<u64>().ok()?)
}

/// Datas (hora local, do nome) dos snapshots do Time Machine, ordenadas. Serve para a saída do
/// `tmutil listlocalsnapshots /` (um nome por linha) e para a lista do fs_snapshot_list.
pub fn datas_tm<'a>(nomes: impl IntoIterator<Item = &'a str>) -> Vec<Option<String>> {
    let mut v: Vec<_> = nomes.into_iter().map(|n| snapshots::tipo(n.trim())).filter(|t| t.0 == "time_machine").map(|t| t.1).collect();
    v.sort();
    v
}

/// (piso, teto) de `x / div`: o arredondamento do comando não é documentado, então vale qualquer
/// um dos dois. É a resolução da fonte, não tolerância.
pub fn resolucao(x: u64, div: u64) -> (u64, u64) {
    (x / div, x.div_ceil(div))
}

/// Resultado e valor mostrado do sdtally contra as leituras da Apple antes (`a`) e depois (`b`).
pub fn sanduiche((piso, teto): (u64, u64), a: u64, b: u64) -> (&'static str, u64) {
    let faixa = a.min(b)..=a.max(b);
    match [piso, teto].into_iter().find(|v| faixa.contains(v)) {
        Some(v) if a == b => ("equal", v),
        Some(v) => ("within", v),
        None => ("outside", piso),
    }
}

/// Sanduíche para o que não é número (lista de datas): igual às duas, ou a uma delas quando mudou.
pub fn sanduiche_igual<T: PartialEq>(s: &T, a: &T, b: &T) -> &'static str {
    if s == a && s == b {
        "equal"
    } else if a != b && (s == a || s == b) {
        "within"
    } else {
        "outside"
    }
}

fn linha(id: &str, unit: &str, apple_command: Option<&str>, reason: Option<String>) -> Value {
    json!({"id": id, "sdtally": null, "apple": null, "unit": unit, "result": "not_checkable",
           "apple_command": apple_command, "in_file": null, "reason": reason})
}

/// Número contra o comando: `s` já vem como (piso, teto) na unidade dele.
fn comparar(id: &str, cmd: &str, unit: &str, a: Option<u64>, s: Result<(u64, u64), String>, b: Option<u64>) -> Value {
    let mut v = linha(id, unit, Some(cmd), None);
    match (a, s, b) {
        (Some(a), Ok(s), Some(b)) => {
            let (r, x) = sanduiche(s, a, b);
            (v["sdtally"], v["apple"], v["result"]) = (json!(x), json!([a, b]), json!(r));
        }
        (_, Err(e), _) => v["reason"] = json!(e),
        _ => v["reason"] = json!(format!("{cmd}: unreadable output")),
    }
    v
}

fn checked(fonte: &str) -> &'static str {
    FONTES.iter().find(|f| f.0 == fonte).map_or("no", |f| f.1.checked)
}

/// O JSON do §3.5. Com euid 0 recusa (a chave do i18n, como `--last`). `home` só serve para
/// achar a medição que o app salvou (`in_file`, informativo).
pub fn verificar(root: bool, home: &str) -> Result<Value, &'static str> {
    historico::guarda(root)?;
    let arquivo = historico::ultima(home).ok();
    let no_arquivo = |p: &str| arquivo.as_ref().and_then(|v| v.pointer(p)).and_then(Value::as_u64);
    let mut checks = Vec::new();

    let cmd = "df -k /System/Volumes/Data";
    let df = || sys::run("/bin/df", &["-k", DATA], 10).ok().as_deref().and_then(df_usado_kib);
    let a = df();
    let s = sys::apfs::vol_space_used(DATA).map(|x| resolucao(x, 1024)).map_err(|e| format!("ATTR_VOL_SPACEUSED errno {e}"));
    let mut v = comparar("data_used", cmd, "KiB", a, s, df());
    v["in_file"] = json!(no_arquivo("/data_volume/used/bytes").map(|x| x / 1024));
    checks.push(v);

    let cmd = "sysctl vm.swapusage";
    let sc = || sys::run("/usr/sbin/sysctl", &["vm.swapusage"], 10).ok().as_deref().and_then(swap_total_cmib);
    let a = sc();
    let s = sys::swap_usage().map(|(t, _)| resolucao(t * 100, MIB)).ok_or_else(|| "sysctlbyname vm.swapusage failed".to_string());
    let mut v = comparar("swap", cmd, "MiB/100", a, s, sc());
    v["in_file"] = json!(no_arquivo("/swap/total").map(|x| x * 100 / MIB));
    if v["result"] != "not_checkable" {
        v["reason"] = json!("same kernel counter: checks the reading and the unit conversion");
    }
    checks.push(v);

    let cmd = "tmutil listlocalsnapshots /";
    let tm = || sys::run("/usr/bin/tmutil", &["listlocalsnapshots", "/"], 30).ok().map(|o| datas_tm(o.lines()));
    let a = tm();
    let s = sys::apfs::snapshots(DATA).map(|l| datas_tm(l.iter().map(|x| x.0.as_str())));
    let b = tm();
    let mut v = linha("snapshots_tm", "count", Some(cmd), None);
    match (a, s, b) {
        (Some(a), Ok(s), Some(b)) => {
            let r = sanduiche_igual(&s, &a, &b);
            (v["sdtally"], v["apple"], v["result"]) = (json!(s.len()), json!([a.len(), b.len()]), json!(r));
            if r == "outside" && [a.len(), b.len()].contains(&s.len()) {
                v["reason"] = json!("same count, different dates");
            }
        }
        (_, Err(e), _) => v["reason"] = json!(format!("fs_snapshot_list errno {e}")),
        _ => v["reason"] = json!(format!("{cmd}: failed")),
    }
    let snaps = arquivo.as_ref().and_then(|v| v.pointer("/informational/snapshots")).and_then(Value::as_array);
    v["in_file"] = json!(snaps.map(|l| l.iter().filter(|x| x["kind"] == "time_machine").count()));
    checks.push(v);

    // O sdtally lê a mesma fonte que o comando mostraria: comparar não confere nada.
    for id in ["capacity", "macos", "screen"] {
        checks.push(linha(id, "B", None, Some("same source".into())));
    }
    for (id, fonte) in [("purgeable", "cachedelete"), ("tree", "getattrlistbulk"), ("dir_stats", "fsctl_dir_stats"), ("apple_intelligence", "dir_stats_like_settings")] {
        checks.push(linha(id, "B", None, Some(format!("no equivalent Apple command; catalog checked: {} ({fonte})", checked(fonte)))));
    }
    Ok(json!({
        "schema_version": 1, "kind": "verify",
        "file_generated_at": arquivo.as_ref().map_or(Value::Null, |v| v["generated_at"].clone()),
        "checks": checks,
    }))
}

/// Algum `outside`: o bin sai com 1.
pub fn algum_fora(v: &Value) -> bool {
    v["checks"].as_array().is_some_and(|c| c.iter().any(|x| x["result"] == "outside"))
}

fn numero(x: &Value, unit: &str, lang: Lang) -> String {
    let Some(n) = x.as_u64() else { return "—".into() };
    match unit {
        "MiB/100" => format!("{}{}{:02} MiB", lang.int(n / 100), if lang == Lang::Pt { ',' } else { '.' }, n % 100),
        "count" => lang.int(n),
        u => format!("{} {u}", lang.int(n)),
    }
}

/// Texto do §5.9.
pub fn texto(v: &Value, lang: Lang) -> String {
    let mut s = format!("{}\n{}\n", lang.t("verify_titulo"), lang.t("verify_colunas"));
    for c in v["checks"].as_array().into_iter().flatten() {
        let unit = c["unit"].as_str().unwrap_or("");
        let apple = match c["apple"].as_array() {
            Some(a) if a.len() == 2 => format!("{} / {}", numero(&a[0], unit, lang), numero(&a[1], unit, lang)),
            _ => "—".into(),
        };
        let reason = c["reason"].as_str().unwrap_or("");
        let r = match c["result"].as_str().unwrap_or("") {
            "equal" => lang.t("verify_equal"),
            "within" => lang.t("verify_within"),
            "outside" => lang.t("verify_outside"),
            _ if reason == "same source" => lang.t("verify_mesma_fonte"),
            _ if c["apple_command"].is_null() => lang.t("verify_sem_comando"),
            _ => reason.to_string(),
        };
        s += &format!("{} · {} · {apple} · {r}\n", c["id"].as_str().unwrap_or(""), numero(&c["sdtally"], unit, lang));
    }
    s + &lang.t("verify_rodape") + "\n"
}

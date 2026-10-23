//! Medições completas que o app salvou (spec da fase 5, §4.5): `--last`, `--history` e `--changes`. Só lê;
//! nada aqui grava. O erro nunca traz o conteúdo de um arquivo, só o nome e o motivo.

use crate::anon::Ctx;
use crate::redigir;
use serde_json::{Value, json};
use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// Arquivo maior que isto é recusado sem ler.
pub const LIMITE: u64 = 20_000_000;

/// `--last`, `--history` e `--mcp` nunca precisam de root: com euid 0, recusam. O erro é a chave
/// da mensagem no i18n.
pub fn guarda(root: bool) -> Result<(), &'static str> {
    if root { Err("arg_root") } else { Ok(()) }
}

/// `<home>/Library/Application Support/SDTally/Medicoes`, onde o app grava o stdout cru.
pub fn pasta(home: &str) -> PathBuf {
    Path::new(home).join("Library/Application Support/SDTally/Medicoes")
}

/// Um arquivo de medição: sem symlink, até 20 MB, JSON com `schema_version == 1` e sem `kind`
/// (o rápido e o histórico têm `kind`; a medição completa não).
pub fn ler(p: &Path) -> Result<Value, &'static str> {
    let m = p.symlink_metadata().map_err(|_| "unreadable")?;
    if m.file_type().is_symlink() {
        return Err("symlink");
    }
    if !m.is_file() {
        return Err("not a regular file");
    }
    if m.len() > LIMITE {
        return Err("larger than 20 MB");
    }
    // O_NOFOLLOW fecha a troca por symlink entre o lstat e o open; o take, o arquivo que cresceu.
    let f = OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(p).map_err(|_| "unreadable")?;
    let mut buf = Vec::new();
    f.take(LIMITE + 1).read_to_end(&mut buf).map_err(|_| "unreadable")?;
    if buf.len() as u64 > LIMITE {
        return Err("larger than 20 MB");
    }
    let v: Value = serde_json::from_slice(&buf).map_err(|_| "invalid JSON")?;
    if v.get("schema_version").and_then(Value::as_i64) != Some(1) {
        return Err("schema_version is not 1");
    }
    if v.get("kind").is_some() {
        return Err("not a complete measurement");
    }
    Ok(v)
}

/// As medições válidas de `dir`, da mais antiga para a mais nova (não pelo nome): pela
/// `generated_at_epoch`; arquivo de antes dela, pelo mtime; empate, pelo `generated_at`. A hora
/// local do `generated_at` não tem fuso: volta para trás no fim do horário de verão ou numa viagem
/// para o oeste. Arquivo recusado é pulado com aviso em stderr. Pasta ausente = nenhuma medição.
pub fn todas(dir: &Path) -> Result<Vec<Value>, String> {
    Ok(todas_com_nome(dir)?.into_iter().map(|x| x.2).collect())
}

/// Como `todas`, com a chave de ordem (época ou mtime) e o nome do arquivo.
fn todas_com_nome(dir: &Path) -> Result<Vec<(i64, String, Value)>, String> {
    let entradas = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", dir.display())),
    };
    let mut v = Vec::new();
    for e in entradas.flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "json") {
            continue;
        }
        match ler(&p) {
            Ok(m) => {
                let mtime = p.symlink_metadata().ok().and_then(|x| x.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64);
                v.push((m["generated_at_epoch"].as_i64().unwrap_or(mtime), e.file_name().to_string_lossy().into_owned(), m));
            }
            Err(motivo) => eprintln!("sdtally: skipped {}: {motivo}", e.file_name().to_string_lossy()),
        }
    }
    let texto = |m: &Value| m["generated_at"].as_str().unwrap_or("").to_string();
    v.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| texto(&a.2).cmp(&texto(&b.2))));
    Ok(v)
}

/// A mais nova, para `--last`.
pub fn ultima(home: &str) -> Result<Value, String> {
    let dir = pasta(home);
    todas(&dir)?.pop().ok_or_else(|| format!("no saved measurement in {}", dir.display()))
}

/// `{"schema_version":1,"kind":"history","measurements":[..]}`, da mais antiga para a mais nova.
pub fn historico(home: &str) -> Result<Value, String> {
    let m: Vec<Value> = todas(&pasta(home))?
        .iter()
        .map(|v| {
            let dv = &v["data_volume"];
            json!({
                "generated_at": v["generated_at"], "tool": v["tool"], "root": v["root"],
                "full_disk_access": v["full_disk_access"], "complete": v["complete"],
                "used": dv["used"]["bytes"], "unexplained": dv["unexplained"]["bytes"],
                "percent_of_used": dv["unexplained"]["percent_of_used"],
            })
        })
        .collect();
    Ok(json!({"schema_version": 1, "kind": "history", "measurements": m}))
}

/// Variação de 100 MB ou mais entra no "O que mudou" (`Historico.limite` no app).
pub const LIMITE_MUDANCA: i64 = 100_000_000;
/// No máximo tantas pastas no `--changes`.
pub const MAX_PASTAS: usize = 30;

/// Por que duas medições não se comparam (`Historico.motivo` no app); `None` = comparáveis.
fn motivo(a: &Value, b: &Value) -> Option<&'static str> {
    let interrompida = |m: &Value| m["incomplete_reasons"].as_array().is_some_and(|r| r.iter().any(|x| x == "interrupted"));
    if interrompida(a) || interrompida(b) {
        return Some("interrupted");
    }
    if ["root", "full_disk_access", "complete"].iter().any(|k| a[k] != b[k]) {
        return Some("different_modes");
    }
    (a["tool"] != b["tool"]).then_some("different_versions")
}

/// `{from, to, delta}`; lado ausente = null, e o delta também.
fn par(de: Option<i64>, para: Option<i64>) -> Value {
    json!({"from": de, "to": para, "delta": de.zip(para).map(|(x, y)| y - x)})
}

/// `{"kind":"changes"}` entre as duas medições mais novas, como o "O que mudou" do app
/// (`Historico.mudancas`): termos presentes nas duas e linhas do `top` pela chave path|term|folded
/// (a primeira de cada chave vale), |Δ| ≥ 100 MB, do maior para o menor (empate: chave maior
/// primeiro). Linha só de um lado conta pelo valor dela. Com `red`, os caminhos e os nomes de
/// arquivo saem redigidos, linha por linha, sem juntar linhas.
pub fn mudancas(home: &str, red: Option<(&Ctx, &str)>) -> Result<Value, String> {
    let dir = pasta(home);
    let mut v = todas_com_nome(&dir)?;
    if v.len() < 2 {
        return Err(format!("fewer than 2 saved measurements in {}", dir.display()));
    }
    let (eb, nb, b) = v.pop().unwrap_or_default();
    let (ea, na, a) = v.pop().unwrap_or_default();
    let ponta = |m: &Value, nome: &str| json!({"generated_at": m["generated_at"], "generated_at_epoch": m["generated_at_epoch"], "file": nome});
    let usado = |m: &Value| m.pointer("/data_volume/used/bytes").and_then(Value::as_i64);
    let mut r = json!({
        "schema_version": 1, "kind": "changes", "from": ponta(&a, &na), "to": ponta(&b, &nb), "span_seconds": eb - ea,
        "comparable": motivo(&a, &b).is_none(), "reason": motivo(&a, &b), "used": par(usado(&a), usado(&b)),
        "terms": [], "folders": [],
    });
    let sd = |m: &Value| m.pointer("/settings_screen/screen/system_data").and_then(Value::as_i64);
    if let (Some(x), Some(y)) = (sd(&a), sd(&b)) {
        r["system_data"] = par(Some(x), Some(y));
    }
    if motivo(&a, &b).is_none() {
        let lista = |m: &Value, k: &str| m.pointer(k).and_then(Value::as_array).cloned().unwrap_or_default();
        // (chave de ordem, |Δ|, linha)
        let mut termos: Vec<(String, i64, Value)> = Vec::new();
        let ta = lista(&a, "/data_volume/terms");
        for t in lista(&b, "/data_volume/terms") {
            let k = t["key"].as_str().unwrap_or("");
            let de = ta.iter().find(|x| x["key"] == k).and_then(|x| x["bytes"].as_i64());
            if let (Some(x), Some(y)) = (de, t["bytes"].as_i64()) {
                termos.push((format!("termo|{k}"), (y - x).abs(), json!({"key": k, "from": x, "to": y, "delta": y - x})));
            }
        }
        let chave = |l: &Value| format!("{}|{}|{}", l["path"].as_str().unwrap_or("-"), l["term"].as_str().unwrap_or(""), l["folded"] == true);
        let mut linhas: Vec<(String, Option<i64>, Option<i64>, Value)> = Vec::new();
        for (i, m) in [&a, &b].into_iter().enumerate() {
            for l in lista(m, "/top") {
                let k = chave(&l);
                let j = linhas.iter().position(|x| x.0 == k).unwrap_or_else(|| {
                    linhas.push((k, None, None, l.clone()));
                    linhas.len() - 1
                });
                let lado = if i == 0 { &mut linhas[j].1 } else { &mut linhas[j].2 };
                // A primeira de cada chave vale, como o Dictionary(uniquingKeysWith:) do app.
                if lado.is_none() {
                    *lado = l["bytes"].as_i64();
                }
            }
        }
        let mut pastas: Vec<(String, i64, Value)> = linhas
            .into_iter()
            .map(|(k, de, para, l)| {
                let d = para.unwrap_or(0) - de.unwrap_or(0);
                (k, d.abs(), json!({"path": l["path"], "term": l["term"], "folded": l["folded"] == true, "from": de, "to": para, "delta": d}))
            })
            .collect();
        for (xs, max, campo) in [(&mut termos, usize::MAX, "terms"), (&mut pastas, MAX_PASTAS, "folders")] {
            xs.retain(|x| x.1 >= LIMITE_MUDANCA);
            xs.sort_by(|x, y| (y.1, &y.0).cmp(&(x.1, &x.0)));
            r[campo] = xs.drain(..).take(max).map(|x| x.2).collect();
        }
    }
    if let Some((c, h)) = red {
        // As pastas passam pelo mesmo caminho do `top` de uma medição.
        let mut t = json!({"top": r["folders"].take()});
        redigir::redigir(&mut t, c, h);
        r["folders"] = t["top"].take();
        for p in ["from", "to"] {
            let f = c.clean(r[p]["file"].as_str().unwrap_or(""));
            r[p]["file"] = f.into();
        }
    }
    for p in ["from", "to"] {
        let f = redigir::escapar(r[p]["file"].as_str().unwrap_or(""));
        r[p]["file"] = f.into();
    }
    Ok(r)
}

//! Medições completas que o app salvou (spec da fase 5, §4.5): `--last` e `--history`. Só lê;
//! nada aqui grava. O erro nunca traz o conteúdo de um arquivo, só o nome e o motivo.

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
                v.push((m["generated_at_epoch"].as_i64().unwrap_or(mtime), m));
            }
            Err(motivo) => eprintln!("sdtally: skipped {}: {motivo}", e.file_name().to_string_lossy()),
        }
    }
    let texto = |m: &Value| m["generated_at"].as_str().unwrap_or("").to_string();
    v.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| texto(&a.1).cmp(&texto(&b.1))));
    Ok(v.into_iter().map(|x| x.1).collect())
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

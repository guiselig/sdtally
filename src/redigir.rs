//! Redação para a fronteira (spec da fase 6, §4.5): o MCP e `--redact names`. Troca usuário e
//! pastas pessoais pelos mesmos baldes anônimos do sdc-coleta, sem fundir linhas nem mudar número,
//! e escapa os invisíveis das strings que vêm do disco. O histórico local continua cru.

use crate::anon::Ctx;
use crate::sys;
use serde_json::Value;
use std::io::Read;

/// Strings que vêm do disco ou de processos (também na frase das descrições do MCP).
pub const DE_DISCO: [&str; 7] = ["path", "paths", "name", "process", "creator", "reason", "error"];
/// Chaves cuja string passa por `ctx.clean()` (os caminhos passam por `ctx.bucket()`; o nome de
/// processo, por `ctx.processo()`).
const NOMES: [&str; 5] = ["name", "creator", "reason", "error", "bundle_id"];
/// Caracteres que sobram de cada string escapada.
pub const LIMITE_TEXTO: usize = 256;
/// `--stdin` lê no máximo isto, como o `--last`.
pub const LIMITE_STDIN: u64 = crate::historico::LIMITE;

/// O `Ctx` anônimo do usuário: nome completo e nomes do host viram tokens proibidos.
// ponytail: duplicado de sdc-coleta.rs:100-111; unificar quando o sdc-coleta mudar
pub fn ctx_anonimo(user: &str) -> Ctx {
    let full = sys::run("/usr/bin/id", &["-F", user], 10).unwrap_or_default();
    let mut hosts: Vec<String> = ["ComputerName", "LocalHostName", "HostName"]
        .iter()
        .filter_map(|k| sys::run("/usr/sbin/scutil", &["--get", k], 10).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if let Some(h) = sys::sysctl_str("kern.hostname") {
        hosts.push(h.trim_end_matches(".local").to_string());
    }
    Ctx::new(user, full.trim(), &hosts)
}

/// O nome do processo também entra no `text` do conselho (`deleted_open`): troca lá o mesmo que no campo.
fn no_texto(v: &mut Value, f: impl Fn(&str) -> String) {
    for a in v.get_mut("advice").and_then(Value::as_array_mut).into_iter().flatten() {
        if let (Some(p), Some(t)) = (a["process"].as_str(), a["text"].as_str())
            && !p.is_empty()
        {
            let novo = t.replace(p, &f(p));
            a["text"] = novo.into();
        }
    }
}

fn cada_string(v: &mut Value, chaves: &[&str], f: &dyn Fn(&str) -> String) {
    match v {
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                match x {
                    Value::String(s) if chaves.contains(&k.as_str()) => *s = f(s),
                    Value::Array(a) if chaves.contains(&k.as_str()) && a.iter().all(Value::is_string) => {
                        for s in a.iter_mut() {
                            if let Value::String(s) = s {
                                *s = f(s);
                            }
                        }
                    }
                    _ => cada_string(x, chaves, f),
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| cada_string(x, chaves, f)),
        _ => {}
    }
}

/// Redige uma medição (completa ou rápida) no lugar. `home` é a pasta real do usuário.
pub fn redigir(v: &mut Value, ctx: &Ctx, home: &str) {
    // `~` relativo ao usuário do `ctx`, para o `bucket` reconhecer a home mesmo fora de /Users.
    let casa = format!("/Users/{}", ctx.user);
    let balde = |p: &str| match p.strip_prefix('~') {
        Some(r) if r.is_empty() || r.starts_with('/') => ctx.bucket(&format!("{casa}{r}")),
        _ => ctx.bucket(p),
    };
    for x in v.get_mut("top").and_then(Value::as_array_mut).into_iter().flatten() {
        if let Some(p) = x["path"].as_str() {
            x["path"] = balde(p).into();
        }
    }
    for x in v.pointer_mut("/informational/protected_without_size/folders").and_then(Value::as_array_mut).into_iter().flatten() {
        if let Some(p) = x["path"].as_str() {
            x["path"] = balde(p).into();
        }
    }
    for a in v.get_mut("advice").and_then(Value::as_array_mut).into_iter().flatten() {
        for p in a.get_mut("paths").and_then(Value::as_array_mut).into_iter().flatten() {
            if let Some(s) = p.as_str() {
                *p = balde(s).into();
            }
        }
    }
    let nome = |s: &str| ctx.clean(&if home.is_empty() { s.to_string() } else { s.replace(home, "~") });
    // Nome de processo cortado em 31 bytes pode terminar num pedaço de nome: `processo` corta o fim.
    let processo = |s: &str| ctx.processo(s);
    no_texto(v, processo);
    cada_string(v, &["process"], &processo);
    for p in v.pointer_mut("/informational/deleted_open_processes").and_then(Value::as_array_mut).into_iter().flatten() {
        if let Some(n) = p["name"].as_str() {
            p["name"] = ctx.processo(n).into();
        }
    }
    for x in v.get_mut("top").and_then(Value::as_array_mut).into_iter().flatten() {
        if let Some(n) = x.pointer("/top_process/name").and_then(Value::as_str) {
            x["top_process"]["name"] = ctx.processo(n).into();
        }
    }
    cada_string(v, &NOMES, &nome);
    if v.get("home").is_some() {
        v["home"] = "/Users/<usuário>".into();
    }
}

fn invisivel(c: char) -> bool {
    matches!(c as u32,
        0x00..=0x1F | 0x7F..=0x9F                                   // controle
        | 0x061C | 0x200E | 0x200F | 0x202A..=0x202E | 0x2066..=0x2069 // bidi
        | 0x200B..=0x200D | 0x2060..=0x2064 | 0xFEFF                  // largura zero
        | 0x2028 | 0x2029                                           // separadores de linha e parágrafo
        | 0xE0000..=0xE007F)                                        // tag characters
}

/// Invisíveis viram `\u{…}` visível; corta em 256 caracteres sem partir um escape.
pub fn escapar(s: &str) -> String {
    let (mut o, mut n) = (String::new(), 0);
    for c in s.chars() {
        let e = if invisivel(c) { format!("\\u{{{:04X}}}", c as u32) } else { c.to_string() };
        let k = e.chars().count();
        if n + k > LIMITE_TEXTO {
            break;
        }
        o.push_str(&e);
        n += k;
    }
    o
}

/// Escapa as strings de disco de qualquer JSON de saída (MCP sempre, `--redact names`).
pub fn escapar_json(v: &mut Value) {
    no_texto(v, escapar);
    cada_string(v, &DE_DISCO, &escapar);
}

/// `--redact names --stdin`: até 20 MB, `schema_version` 1, sem `kind` ou `kind: "quick"`.
/// O erro diz só o motivo, nunca o conteúdo.
pub fn ler_entrada(r: impl Read) -> Result<Value, &'static str> {
    let mut buf = Vec::new();
    r.take(LIMITE_STDIN + 1).read_to_end(&mut buf).map_err(|_| "unreadable input")?;
    if buf.len() as u64 > LIMITE_STDIN {
        return Err("input larger than 20 MB");
    }
    let v: Value = serde_json::from_slice(&buf).map_err(|_| "input is not valid JSON")?;
    if v.get("schema_version").and_then(Value::as_i64) != Some(1) {
        return Err("schema_version is not 1");
    }
    match v.get("kind") {
        None => Ok(v),
        Some(k) if k == "quick" => Ok(v),
        _ => Err("not a complete or quick measurement"),
    }
}

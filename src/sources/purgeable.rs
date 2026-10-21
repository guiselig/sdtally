//! Purgeable por serviço (CacheDelete itemizado, urgência 3; nunca purge).

use super::DATA;
use crate::anon::Ctx;
use crate::ledger::Medida;
use crate::sys;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub struct Purgeable {
    pub json: Value,
    pub total: Medida,
    /// (serviço, bytes) > 0, do maior para o menor.
    pub top: Vec<(String, i64)>,
}

pub fn coletar(ctx: &Ctx, err: &mut Vec<Value>) -> Purgeable {
    let kv = match sys::with_timeout(120, || sys::cachedelete::cachedelete_itemized(DATA, 3)) {
        None => Err("CacheDelete: timeout de 120 s".to_string()),
        Some(r) => r,
    };
    let kv = match kv {
        Ok(kv) => kv,
        Err(e) => {
            err.push(json!(e));
            return Purgeable { json: Value::Null, total: Medida::NaoMedida(e), top: Vec::new() };
        }
    };
    let mut tot = BTreeMap::new();
    let mut serv = BTreeMap::new();
    let mut top = Vec::new();
    let mut soma = 0i64;
    for (k, v) in kv {
        if k.starts_with("CACHE_DELETE_") {
            tot.insert(k, v);
        } else {
            soma += v;
            if v > 0 {
                top.push((ctx.clean(&k), v));
            }
            serv.insert(ctx.clean(&k), v);
        }
    }
    top.sort_by_key(|x| -x.1);
    // Resposta sem nenhum serviço não é "0 purgeable": o CacheDelete não itemizou nada.
    let total = if serv.is_empty() {
        err.push(json!("CacheDelete respondeu sem serviços"));
        Medida::NaoMedida("CacheDelete sem serviços".into())
    } else {
        Medida::Valor(soma)
    };
    let j = json!({
        "urgencia": 3,
        "soma_servicos": total.valor(),
        "totais": tot,
        "por_servico": serv,
        "fonte": "CacheDeleteCopyItemizedPurgeableSpaceWithInfo (dlsym, só consulta)",
        "nsurl_important": "pulado: exige Foundation",
    });
    Purgeable { json: j, total, top }
}

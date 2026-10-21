//! dir-stats das pastas que a travessia não pôde abrir (EPERM/EACCES), sem abrir a pasta.

use super::{DATA, LIMITE_BALDE};
use crate::anon::{self, Ctx};
use crate::ledger::Medida;
use crate::sys;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

pub struct Negadas {
    pub json: Value,
    pub total: Medida,
    /// (balde, bytes) já dobrados pelo limite.
    pub baldes: Vec<(String, i64)>,
    /// Pastas sem dir-stats (balde, errno; None = dir-stats devolveu 0), por caminho.
    pub sem_tamanho: Vec<(String, Option<i32>)>,
}

pub fn negadas(denied: &[(PathBuf, i32)], ctx: &Ctx) -> Negadas {
    let mut b: HashMap<String, i64> = HashMap::new();
    let (mut com, mut zero, mut soma) = (0u64, 0u64, 0i64);
    let mut falhas: BTreeMap<String, u64> = BTreeMap::new();
    let mut sem_tamanho: Vec<(String, Option<i32>)> = Vec::new();
    for (p, _) in denied {
        let s = p.to_string_lossy();
        match sys::apfs::dirstats(&s) {
            Ok(0) => {
                zero += 1;
                sem_tamanho.push((ctx.bucket(s.strip_prefix(DATA).unwrap_or(&s)), None));
            }
            Ok(v) => {
                com += 1;
                soma += v as i64;
                *b.entry(ctx.bucket(s.strip_prefix(DATA).unwrap_or(&s))).or_default() += v as i64;
            }
            Err(e) => {
                *falhas.entry(e.to_string()).or_default() += 1;
                sem_tamanho.push((ctx.bucket(s.strip_prefix(DATA).unwrap_or(&s)), Some(e)));
            }
        }
    }
    sem_tamanho.sort();
    let b = anon::fold(b, LIMITE_BALDE, |x| *x, |x, y| *x += *y);
    let mut v: Vec<_> = b.into_iter().collect();
    v.sort_by_key(|x| -x.1);
    // Pasta sem dir-stats (errno ou 0, que não dá para distinguir de "não mantido") não vira 0
    // medido: o termo sai parcial e o que faltou fica no não atribuído.
    let sem = zero + falhas.values().sum::<u64>();
    let total = match (sem, com) {
        (0, _) => Medida::Valor(soma),
        (_, 0) => Medida::NaoMedida(format!("{sem} pastas sem dir-stats")),
        _ => Medida::Parcial(soma, format!("{sem} de {} pastas sem dir-stats", denied.len())),
    };
    let j = json!({
        "fonte": "fsctl(pasta, 0xC1104A71, buffer zerado) = bytes físicos que o APFS mantém por pasta; sem abrir a pasta",
        "pastas": denied.len(),
        "com_dirstats": com,
        "sem_dirstats_valor_zero": zero,
        "falhas_por_errno": falhas,
        "bytes": total.valor(),
        "estado": total.estado(),
        "baldes": v.iter().map(|(k, s)| json!({"balde": k, "bytes": s})).collect::<Vec<_>>(),
        "ressalvas": "dir-stats conta clone em dobro e não conta comprimido nem xattr grande; pasta criada fora e movida para dentro pode ter 0 (Frente M)",
    });
    Negadas { json: j, total, baldes: v, sem_tamanho }
}

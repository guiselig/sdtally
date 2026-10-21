//! "Por que o espaço não voltou" (PLANO Fase 3, ideias B3): as causas que o relatório mediu e que
//! fazem apagar algo devolver menos do que o esperado. Pura: só lê a `Conta`. Causa sem número
//! medido (ou abaixo de 1 GB) não existe aqui; o texto não promete nada além do que a fonte mostra.

use crate::anon::DOBRADO;
use crate::conselho::casa;
use crate::ledger::Medida;
use crate::report::{Chave, Conta, Fonte};
use serde_json::{Value, json};

const GB: i64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Causa {
    pub id: &'static str,
    pub bytes: i64,
    /// Snapshots ou arquivos, quando a fonte conta.
    pub quantidade: Option<u64>,
    /// `bytes` é um piso (medição parcial ou interrompida).
    pub piso: bool,
    pub fonte: Fonte,
}

/// Ids de `texto`, na ordem do `match`.
pub const IDS: [&str; 6] = ["local_snapshot", "deleted_open", "shared_clone", "purgeable", "trash", "cloud_only"];

/// (título, explicação), cada um em (pt, en). Ordem do `match` em `texto`.
pub fn texto(id: &str) -> Option<[(&'static str, &'static str); 2]> {
    Some(match id {
        "local_snapshot" => [
            ("Snapshots locais", "Local snapshots"),
            ("Um snapshot local ainda segura os blocos dos arquivos que você apagou: o espaço só volta quando o snapshot sai. O macOS remove os do Time Machine sozinho se faltar espaço.", "A local snapshot still holds the blocks of files you deleted: the space only comes back when the snapshot is gone. macOS removes Time Machine ones on its own when space runs low."),
        ],
        "deleted_open" => [
            ("Apagados ainda abertos", "Deleted but still open"),
            ("Um arquivo apagado que um app ainda mantém aberto continua ocupando espaço até o app soltá-lo. Fechar o app ou reiniciar o Mac devolve.", "A deleted file an app still holds open keeps using space until the app lets go. Quitting the app or restarting the Mac gives it back."),
        ],
        "shared_clone" => [
            ("Clones APFS", "APFS clones"),
            ("Um clone APFS divide os blocos com o arquivo original: apagar só um dos dois devolve pouco ou nada. O espaço volta quando o último sai.", "An APFS clone shares blocks with the original file: deleting only one of them gives back little or nothing. The space comes back when the last one is gone."),
        ],
        "purgeable" => [
            ("Purgeable", "Purgeable"),
            ("O Disponível do Ajustes é o livre mais o purgeable. O purgeable só vira livre quando o macOS o libera; quem olha o livre (df, Terminal) não o vê como livre.", "Available in Settings is free plus purgeable. Purgeable only becomes free when macOS releases it; tools that show free space (df, Terminal) do not count it as free."),
        ],
        "trash" => [
            ("Lixo não esvaziado", "Trash not emptied"),
            ("O que está no Lixo continua ocupando espaço. O espaço volta quando o Lixo é esvaziado.", "What is in the Trash still uses space. The space comes back when the Trash is emptied."),
        ],
        "cloud_only" => [
            ("Arquivos só na nuvem", "Cloud-only files"),
            ("Arquivos só na nuvem (iCloud Drive e outros provedores) não ocupam espaço no disco: apagá-los aqui não devolve nada. O número é o tamanho aparente, não o ocupado.", "Cloud-only files (iCloud Drive and other providers) do not use disk space: deleting them here gives nothing back. The number is the apparent size, not the space used."),
        ],
        _ => return None,
    })
}

/// Do maior para o menor (empate: id). Só entra o que tem número medido de 1 GB ou mais.
pub fn causas(c: &Conta) -> Vec<Causa> {
    let mut v = Vec::new();
    let mut poe = |id, bytes: i64, quantidade, piso, fonte| {
        if bytes >= GB {
            v.push(Causa { id, bytes, quantidade, piso, fonte });
        }
    };
    let termo = |k: Chave| c.medida(k).and_then(|m| m.valor().map(|b| (b, matches!(m, Medida::Parcial(..)))));
    if let Some((b, p)) = termo(Chave::Snapshots) {
        let n = c.ranking.iter().find(|i| i.termo == Chave::Snapshots).and_then(|i| i.quantidade);
        poe("local_snapshot", b, n.map(|n| n as u64), p, Fonte::Snapshots);
    }
    if let Some((b, p)) = termo(Chave::Apagados) {
        poe("deleted_open", b, None, p, Fonte::Lsof);
    }
    if let Some(r) = &c.repetidos {
        poe("shared_clone", r.clones.1, Some(r.clones.0), r.parcial, Fonte::Travessia);
        poe("cloud_only", r.dataless.1, Some(r.dataless.0), r.parcial, Fonte::Travessia);
    }
    if let Some(b) = c.purgeable.valor() {
        poe("purgeable", b, None, matches!(c.purgeable, Medida::Parcial(..)), Fonte::CacheDelete);
    }
    if termo(Chave::Arvore).is_some() {
        let na_lixeira: Vec<_> = c.ranking.iter().filter(|i| casa("~/.Trash", i.nome.strip_suffix(DOBRADO).unwrap_or(&i.nome))).collect();
        poe("trash", na_lixeira.iter().map(|i| i.bytes).sum(), None, na_lixeira.iter().any(|i| i.parcial), Fonte::Travessia);
    }
    v.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.id.cmp(b.id)));
    v
}

/// `not_freed[]` do JSON completo: `text` e `title` em {pt, en, outras línguas}; `source` é a chave de
/// `sources`.
pub fn json(v: &[Causa]) -> Value {
    json!(v.iter().filter_map(|x| {
        let [(tp, te), (dp, de)] = texto(x.id)?;
        Some(json!({
            "id": x.id,
            "title": crate::traducao::json(tp, te),
            "text": crate::traducao::json(dp, de),
            "bytes": x.bytes,
            "bytes_floor": x.piso,
            "count": x.quantidade,
            "source": x.fonte.chave(),
        }))
    }).collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_id_tem_texto() {
        for id in ["local_snapshot", "deleted_open", "shared_clone", "purgeable", "trash", "cloud_only"] {
            assert!(texto(id).is_some(), "{id}");
        }
    }
}

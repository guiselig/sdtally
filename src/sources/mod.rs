//! Fontes de medição fora da travessia. Cada uma devolve o número da conta como `Medida`
//! (nunca 0 no lugar de "não medido") e o detalhe em JSON no formato do sdc-coleta.
//! Erros vão para `err` e aparecem no relatório; nenhuma fonte aborta a coleta.

pub mod apagados;
pub mod apfs_meta;
pub mod categorias;
pub mod dirstats;
pub mod lixo;
pub mod painel;
pub mod purgeable;
pub mod snapshots;
pub mod volumes;

pub const DATA: &str = "/System/Volumes/Data";
/// Baldes menores que isto somam em "(demais)" do pai.
pub const LIMITE_BALDE: i64 = 100_000_000;

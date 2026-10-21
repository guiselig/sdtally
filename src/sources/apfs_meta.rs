//! Metadados do APFS do volume Data: nós das árvores vivas (omap, fsroot, extentref) × bloco, pelo
//! AppleAPFSUserClient (Frente G). Sem root, só leitura. Não cobre a árvore extentref de cada
//! snapshot nem as árvores auxiliares (snap_meta, fext): é um piso do metadado, e é o termo
//! `apfs_metadata` da conta (a travessia só soma arquivos, então ele não se sobrepõe a ela).

use super::DATA;
use crate::ledger::Medida;
use crate::sys::{self, iokit::EstatVolume};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApfsMeta {
    pub medida: Medida,
    pub detalhe: Option<EstatVolume>,
}

/// O índice do volume vem do nome do dispositivo; `fs_used` do seletor 7 precisa bater com o
/// ATTR_VOL_SPACEUSED do Data (1 %), senão o índice aponta para outro volume e nada é dito. O motivo
/// de não medido fica na `Medida`; não vira erro de etapa (é informativo, não deixa a coleta INCOMPLETA).
pub fn coletar() -> ApfsMeta {
    let falha = |m: String| ApfsMeta { medida: Medida::NaoMedida(m), detalhe: None };
    let dev = match sys::dispositivo_de(DATA) {
        Ok(d) => d,
        Err(e) => return falha(format!("statfs do volume Data errno {e}")),
    };
    let est = match sys::iokit::estatisticas_volume(&dev) {
        Ok(e) => e,
        Err(m) => return falha(m),
    };
    let attr = match sys::apfs::vol_space_used(DATA) {
        Ok(a) => a,
        Err(e) => return falha(format!("ATTR_VOL_SPACEUSED errno {e}")),
    };
    conferir(est, attr)
}

/// Pura: o volume confere e os contadores são coerentes.
pub fn conferir(est: EstatVolume, attr_usado: u64) -> ApfsMeta {
    let dif = est.usado.abs_diff(attr_usado);
    let nao = |m: &str| ApfsMeta { medida: Medida::NaoMedida(m.into()), detalhe: None };
    if dif * 100 > attr_usado {
        return nao("o volume do AppleAPFSUserClient não confere com o ATTR_VOL_SPACEUSED do Data");
    }
    if est.bloco == 0 || est.metadados() == 0 || est.metadados() > attr_usado {
        return nao("contadores de nós do AppleAPFSUserClient incoerentes");
    }
    ApfsMeta { medida: Medida::Valor(est.metadados() as i64), detalhe: Some(est) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn est(usado: u64) -> EstatVolume {
        EstatVolume { bloco: 4096, usado, arquivos: 1, pastas: 1, nos_omap: 10, nos_fsroot: 100, nos_extentref: 5 }
    }

    #[test]
    fn confere_o_volume() {
        let m = conferir(est(1_000_000_000), 1_005_000_000);
        assert_eq!(m.medida, Medida::Valor(115 * 4096));
        let m = conferir(est(1_000_000_000), 2_000_000_000);
        assert!(matches!(m.medida, Medida::NaoMedida(_)) && m.detalhe.is_none());
        // Mais metadado que volume usado: contador de outra versão do macOS.
        let m = conferir(EstatVolume { nos_fsroot: u64::MAX / 8192, ..est(1_000_000) }, 1_000_000);
        assert!(matches!(m.medida, Medida::NaoMedida(_)));
    }
}

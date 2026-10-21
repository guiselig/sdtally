//! Versão do macOS e capacidades do volume, lidas uma vez no início. Cada fonte pergunta aqui
//! se pode medir, em vez de testar a versão por conta própria (Frente O §5).

use crate::ledger::Medida;
use crate::sys;
use serde_json::{Value, json};

/// VOL_CAP_FMT_CLONE_MAPPING (sys/attr.h). Só registrado: no Data do macOS 27 (M4) o bit vem
/// desligado e mesmo assim dir-stats e enxertos funcionam (29/09), então não serve de trava.
const VOL_CAP_FMT_CLONE_MAPPING: u32 = 0x0400_0000;

pub struct Caps {
    /// `kern.osproductversion` (ex.: "27.0").
    pub macos: Option<String>,
    /// VOL_CAPABILITIES_FORMAT do volume: (válidos, ativos) ou errno. Vai para o relatório para
    /// montar a matriz versão × capacidade com as coletas do CI e dos amigos.
    pub vol_formato: Result<(u32, u32), i32>,
}

impl Caps {
    pub fn ler(volume: &str) -> Caps {
        Caps { macos: sys::sysctl_str("kern.osproductversion"), vol_formato: sys::apfs::vol_cap_formato(volume) }
    }

    /// Apple Intelligence existe a partir do 15.1 (requisito oficial). Versão ilegível conta
    /// como disponível: a fonte tenta medir e o erro fala.
    pub fn apple_intelligence(&self) -> bool {
        self.macos.as_deref().and_then(versao).is_none_or(|v| v >= (15, 1))
    }

    pub fn json(&self) -> Value {
        json!({
            "macos": self.macos,
            "apple_intelligence": if self.apple_intelligence() { "disponível".into() } else { Medida::NaoDisponivel.estado() },
            "vol_formato": match self.vol_formato {
                Ok((v, a)) => json!({"validos": v, "ativos": a, "clone_mapping": v & a & VOL_CAP_FMT_CLONE_MAPPING != 0}),
                Err(e) => json!({"erro_errno": e}),
            },
            "fonte": "sysctl kern.osproductversion; getattrlist ATTR_VOL_CAPABILITIES do volume Data",
        })
    }
}

/// "15.1.2" → (15, 1); "27" → (27, 0).
fn versao(s: &str) -> Option<(u32, u32)> {
    let mut p = s.trim().split('.');
    let maior = p.next()?.parse().ok()?;
    let menor = p.next().map_or(Some(0), |x| x.parse().ok())?;
    Some((maior, menor))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(v: &str, fmt: Result<(u32, u32), i32>) -> Caps {
        Caps { macos: Some(v.into()), vol_formato: fmt }
    }

    #[test]
    fn versoes() {
        assert_eq!(versao("15.1.2"), Some((15, 1)));
        assert_eq!(versao("27"), Some((27, 0)));
        assert_eq!(versao("x"), None);
        assert!(!caps("15.0", Err(0)).apple_intelligence());
        assert!(caps("15.1", Err(0)).apple_intelligence());
        assert!(caps("26.0", Err(0)).apple_intelligence());
        assert!(Caps { macos: None, vol_formato: Err(0) }.apple_intelligence());
    }
}

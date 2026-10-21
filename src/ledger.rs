//! A conta do volume: cada linha com um de três estados e o resto por diferença.
//! Aritmética pura. O que não foi medido nunca vira 0 (PLANO §2.1).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Medida {
    Valor(i64),
    /// A fonte não existe nesta versão do macOS (ex.: Apple Intelligence no 15.0).
    NaoDisponivel,
    /// Existe, mas não deu para medir; o texto diz por quê.
    NaoMedida(String),
    /// Soma só do que foi medido; o texto diz o que ficou de fora (e fica no resto). É um piso.
    Parcial(i64, String),
}

impl Medida {
    pub fn valor(&self) -> Option<i64> {
        match self {
            Medida::Valor(v) | Medida::Parcial(v, _) => Some(*v),
            _ => None,
        }
    }

    pub fn estado(&self) -> String {
        match self {
            Medida::Valor(_) => "medida".into(),
            Medida::NaoDisponivel => "não disponível nesta versão".into(),
            Medida::NaoMedida(m) => format!("não medida ({m})"),
            Medida::Parcial(_, m) => format!("parcial ({m})"),
        }
    }

    /// Estado para o JSON (enum em inglês) e o motivo, se houver.
    pub fn estado_json(&self) -> (&'static str, Option<&str>) {
        match self {
            Medida::Valor(_) => ("measured", None),
            Medida::Parcial(_, m) => ("partial", Some(m)),
            Medida::NaoMedida(m) => ("not_measured", Some(m)),
            Medida::NaoDisponivel => ("not_available", None),
        }
    }

    /// Para o texto: GB com vírgula, ou o estado no lugar do número.
    pub fn gb(&self) -> String {
        match self {
            Medida::Valor(v) => gb(*v),
            Medida::NaoDisponivel => "não disp.".into(),
            Medida::NaoMedida(_) => "não medida".into(),
            Medida::Parcial(v, _) => format!("≥{}", gb(*v)),
        }
    }
}

pub fn gb(b: i64) -> String {
    format!("{:.2}", b as f64 / 1e9).replace('.', ",")
}

pub struct Termo {
    pub chave: &'static str,
    pub medida: Medida,
    pub fonte: &'static str,
}

/// Usado − Σ termos medidos. Termo não medido fica dentro do resto (e aparece como não medido
/// na própria linha); sem o usado não há resto.
pub fn resto<'a>(usado: Option<i64>, termos: impl IntoIterator<Item = &'a Medida>) -> Option<i64> {
    usado.map(|u| u - termos.into_iter().filter_map(Medida::valor).sum::<i64>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conta() {
        let termos = [Medida::Valor(60), Medida::NaoMedida("sem root".into()), Medida::Parcial(30, "1 pasta".into()), Medida::NaoDisponivel];
        // A parte medida de um termo parcial sai do resto; a que faltou fica nele.
        assert_eq!(resto(Some(100), &termos), Some(10));
        assert_eq!(resto(None, &termos), None);
        assert_eq!(resto(Some(5), &[]), Some(5));
        // Negativo fica visível: é sinal de contagem dupla, não é zerado.
        assert_eq!(resto(Some(50), &termos), Some(-40));
    }

    #[test]
    fn estados() {
        assert_eq!(Medida::Valor(0).estado(), "medida");
        assert_eq!(Medida::NaoDisponivel.estado(), "não disponível nesta versão");
        assert_eq!(Medida::NaoMedida("errno 1".into()).estado(), "não medida (errno 1)");
        assert_eq!(Medida::Valor(1_234_560_000).gb(), "1,23");
        assert_eq!(Medida::NaoMedida("x".into()).gb(), "não medida");
        assert_eq!(Medida::Parcial(1_234_560_000, "x".into()).estado(), "parcial (x)");
        assert_eq!(Medida::Parcial(1_234_560_000, "x".into()).gb(), "≥1,23");
    }
}

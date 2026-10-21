//! Textos do JSON em várias línguas (fichas do catálogo e "por que não voltou"). A fonte é o par
//! pt/en no código; as outras línguas ficam em `src/traducoes/<código>.json`, um mapa do texto em
//! inglês para o texto traduzido. Texto em inglês que mudou e ainda não foi traduzido sai sem a
//! língua (o app cai no inglês); os testes apontam o que falta.

use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

/// (código BCP 47, conteúdo). Os códigos seguem as localizações do macOS; "pt" (pt-BR) e "en"
/// vêm do código.
const LINGUAS: &[(&str, &str)] = &[
    ("ar", include_str!("traducoes/ar.json")),
    ("ca", include_str!("traducoes/ca.json")),
    ("cs", include_str!("traducoes/cs.json")),
    ("da", include_str!("traducoes/da.json")),
    ("de", include_str!("traducoes/de.json")),
    ("el", include_str!("traducoes/el.json")),
    ("es", include_str!("traducoes/es.json")),
    ("es-419", include_str!("traducoes/es-419.json")),
    ("fi", include_str!("traducoes/fi.json")),
    ("fr", include_str!("traducoes/fr.json")),
    ("he", include_str!("traducoes/he.json")),
    ("hi", include_str!("traducoes/hi.json")),
    ("hr", include_str!("traducoes/hr.json")),
    ("hu", include_str!("traducoes/hu.json")),
    ("id", include_str!("traducoes/id.json")),
    ("it", include_str!("traducoes/it.json")),
    ("ja", include_str!("traducoes/ja.json")),
    ("ko", include_str!("traducoes/ko.json")),
    ("ms", include_str!("traducoes/ms.json")),
    ("nb", include_str!("traducoes/nb.json")),
    ("nl", include_str!("traducoes/nl.json")),
    ("pl", include_str!("traducoes/pl.json")),
    ("pt-PT", include_str!("traducoes/pt-PT.json")),
    ("ro", include_str!("traducoes/ro.json")),
    ("ru", include_str!("traducoes/ru.json")),
    ("sk", include_str!("traducoes/sk.json")),
    ("sl", include_str!("traducoes/sl.json")),
    ("sv", include_str!("traducoes/sv.json")),
    ("th", include_str!("traducoes/th.json")),
    ("tr", include_str!("traducoes/tr.json")),
    ("uk", include_str!("traducoes/uk.json")),
    ("vi", include_str!("traducoes/vi.json")),
    ("zh-Hans", include_str!("traducoes/zh-Hans.json")),
    ("zh-Hant", include_str!("traducoes/zh-Hant.json")),
];

fn mapas() -> &'static Vec<(&'static str, HashMap<String, String>)> {
    static M: OnceLock<Vec<(&'static str, HashMap<String, String>)>> = OnceLock::new();
    M.get_or_init(|| LINGUAS.iter().map(|(c, s)| (*c, serde_json::from_str(s).unwrap_or_default())).collect())
}

/// `{"pt": ..., "en": ..., "<código>": ...}`: só as línguas que têm tradução, nunca texto vazio.
pub fn json(pt: &str, en: &str) -> Value {
    let mut o = Map::new();
    o.insert("pt".into(), Value::from(pt));
    o.insert("en".into(), Value::from(en));
    for (c, m) in mapas() {
        if let Some(t) = m.get(en).filter(|t| !t.trim().is_empty()) {
            o.insert((*c).into(), Value::from(t.as_str()));
        }
    }
    Value::Object(o)
}

/// Todo texto traduzível: (onde, pt, en). Fonte para os tradutores e para os testes.
pub fn todos() -> Vec<(String, &'static str, &'static str)> {
    use crate::catalogo::{CATALOGO, Sozinho};
    let mut v = Vec::new();
    for f in CATALOGO {
        let mut poe = |campo: &str, t: &crate::catalogo::Txt| v.push((format!("card {} {campo}", f.id), t.pt, t.en));
        poe("what", &f.o_que_e);
        poe("owner", &f.dono);
        if let Sozinho::Sim(t) | Sozinho::Quando(t) = &f.sozinho {
            poe("self_cleans_text", t);
        }
        poe("loses", &f.perde);
        poe("step", &f.passo);
    }
    for id in crate::voltou::IDS {
        if let Some([(tp, te), (dp, de)]) = crate::voltou::texto(id) {
            v.push((format!("not_freed {id} title"), tp, te));
            v.push((format!("not_freed {id} text"), dp, de));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn traducoes_validas_e_em_dia() {
        let ens: HashSet<&str> = todos().iter().map(|x| x.2).filter(|e| !e.is_empty()).collect();
        for (c, s) in LINGUAS {
            let m: HashMap<String, String> = serde_json::from_str(s).unwrap_or_else(|e| panic!("{c}.json: {e}"));
            let velhas: Vec<_> = m.keys().filter(|k| !ens.contains(k.as_str())).collect();
            assert!(velhas.is_empty(), "{c}.json traduz texto que não existe mais: {velhas:#?}");
            let faltam: Vec<_> = ens.iter().filter(|e| m.get(**e).is_none_or(|t| t.trim().is_empty())).collect();
            assert!(faltam.is_empty(), "{c}.json sem tradução de {} textos: {faltam:#?}", faltam.len());
        }
    }

    #[test]
    fn todo_id_do_nao_voltou_tem_texto() {
        assert_eq!(todos().iter().filter(|x| x.0.starts_with("not_freed")).count(), crate::voltou::IDS.len() * 2);
    }

    /// Fonte para os tradutores: `SDTALLY_TEXTOS=arquivo.json cargo test despejar -- --ignored`.
    #[test]
    #[ignore]
    fn despejar() {
        let v: Vec<_> = todos().iter().map(|(onde, pt, en)| serde_json::json!({"where": onde, "pt": pt, "en": en})).collect();
        std::fs::write(std::env::var("SDTALLY_TEXTOS").expect("SDTALLY_TEXTOS"), serde_json::to_string_pretty(&v).unwrap()).unwrap();
    }

    #[test]
    fn json_so_com_linguas_traduzidas() {
        let v = json("Olá", "texto que ninguém traduziu");
        assert_eq!(v.as_object().map(|o| o.len()), Some(2));
        assert_eq!(v["pt"], "Olá");
    }
}

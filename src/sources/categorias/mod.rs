//! As 15 categorias do Ajustes > Armazenamento reproduzidas a partir do que o sdtally já mede
//! (Frente Q §6): as linhas do `top` caem numa linha do Ajustes por `regras`; Música, TV e
//! Mensagens vêm do banco da biblioteca (`fatos`). Só leitura; nada é chamado em framework privado.
//! A saída vai em `system_data.reproduced` do JSON; `system_data.by_line` é só a soma do `top`.

pub mod fatos;
pub mod midia;
pub mod regras;

use crate::ledger::Medida;
use regras::{Aberto, DEV, DOCS, Destino, FOTOS, Grau, GARAGEBAND, IDS, IOS, LIVROS, LIXO, Linha, MAIL, MENSAGENS, MUSICA, OUTROS, PODCASTS, TV};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// Σ reproduzida contra Σ do log da tela: o critério da fase 2 (PLANO §5).
pub const TOLERANCIA_PCT: f64 = 1.0;

/// O que só a máquina sabe. `None` é "não lido": a categoria que depende dele sai parcial ou não
/// medida, nunca zero.
#[derive(Debug, Clone, Default)]
pub struct Fatos {
    /// Bundle ids dos apps que o Spotlight lista (a consulta da extensão de Aplicativos).
    pub apps: Option<BTreeSet<String>>,
    /// Pasta de container com nome de UUID → bundle id (`MCMMetadataIdentifier`).
    pub container_ids: BTreeMap<String, String>,
    /// "Mesa e Documentos" do iCloud ligado para cada pasta.
    pub mesa: Option<bool>,
    pub documentos: Option<bool>,
    pub musica: Option<Medida>,
    pub tv: Option<Medida>,
    pub livros: Option<Medida>,
    pub podcasts: Option<Medida>,
    pub mensagens: Option<Medida>,
    /// Purgeable do `com.apple.MailCacheDelete`.
    pub mail_purgeable: Option<i64>,
    /// Outras contas em /Users (fora Shared, Deleted Users e Guest).
    pub outras_homes: Option<usize>,
    /// Os baldes da travessia sem dobrar, no lugar das linhas `tree` do `top` (que dobram o que passa
    /// de 100 MB para baixo e escondem categorias pequenas).
    pub arvore: Option<Arvore>,
}

/// Baldes da travessia como o Ajustes os soma: (caminho com `~`, alocado + cópias de clone).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Arvore {
    pub baldes: Vec<(String, i64)>,
    /// Parte dos bytes acima que são cópias de clone completo: a travessia conta uma cópia por
    /// grupo, o `STMSize` do Ajustes conta todas (Frente Q §4).
    pub clones: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Categoria {
    pub id: &'static str,
    pub medida: Medida,
    pub grau: Grau,
    pub fonte: &'static str,
    pub nota: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resultado {
    pub categorias: Vec<Categoria>,
    /// Linhas que não couberam em nenhuma linha (fora de todas as categorias).
    pub nao_colocado: i64,
    pub motivos: Vec<Aberto>,
}

/// Linha da árvore → categoria. `macos` é a versão maior; `root` e `interrompida` vêm da raiz do JSON.
pub fn reproduzir(top: &[Linha], f: &Fatos, macos: Option<u32>, root: bool, interrompida: bool) -> Resultado {
    // id → (bytes, alguma linha de grau B)
    let mut soma: BTreeMap<&str, (i64, bool)> = BTreeMap::new();
    let (mut aberto, mut motivos) = (0i64, BTreeSet::new());
    for l in top {
        match regras::destino(l, f, macos) {
            Destino::Ignora => {}
            Destino::Em(id, g) => {
                let e = soma.entry(id).or_default();
                e.0 += l.bytes;
                e.1 |= g == Grau::B;
            }
            Destino::Aberto(a) => {
                aberto += l.bytes;
                motivos.insert(a);
            }
        }
    }
    let tem = |a: Aberto| motivos.contains(&a);
    let da_arvore = |id: &'static str, nota: Option<&'static str>, fonte: &'static str| {
        let (b, b_) = soma.get(id).copied().unwrap_or_default();
        let medida = if interrompida { Medida::Parcial(b, "scan interrupted".into()) } else { Medida::Valor(b) };
        Categoria { id, medida, grau: if b_ { Grau::B } else { Grau::A }, fonte, nota }
    };
    let do_banco = |id: &'static str, m: &Option<Medida>, fonte: &'static str| Categoria {
        id,
        medida: m.clone().unwrap_or_else(|| Medida::NaoMedida("not read".into())),
        grau: Grau::A,
        fonte,
        nota: None,
    };
    const ARV: &str = "tree_buckets";
    let mut cats = Vec::with_capacity(15);
    for id in IDS {
        let c = match id {
            regras::APPS => {
                let mut c = da_arvore(id, Some("containers of apps found by Spotlight; bundles in .app folders outside /Applications and ~/Applications are not separated"), ARV);
                if tem(Aberto::Containers) {
                    let b = c.medida.valor().unwrap_or(0);
                    c.medida = Medida::Parcial(b, "app containers not classified (no Spotlight app list)".into());
                }
                c
            }
            DOCS => {
                let mut c = da_arvore(id, Some("apps inside folders of the home are not subtracted"), ARV);
                if tem(Aberto::MesaDocumentos) {
                    let b = c.medida.valor().unwrap_or(0);
                    c.medida = Medida::Parcial(b, "Desktop/Documents not classified (iCloud Desktop & Documents state unknown)".into());
                }
                c
            }
            MUSICA => do_banco(id, &f.musica, "music_library_db"),
            FOTOS => da_arvore(id, Some("allocated size of the library folder; the screen counts logical size minus purgeable"), ARV),
            TV => do_banco(id, &f.tv, "tv_library_db"),
            LIVROS => do_banco(id, &f.livros, "library_presence"),
            PODCASTS => do_banco(id, &f.podcasts, "library_presence"),
            regras::LIXO => da_arvore(LIXO, None, ARV),
            GARAGEBAND => da_arvore(id, None, ARV),
            IOS => da_arvore(id, None, ARV),
            DEV => da_arvore(id, None, ARV),
            regras::ICLOUD => Categoria {
                id,
                medida: Medida::NaoMedida("needs the File Provider database, not read by this version".into()),
                grau: Grau::B,
                fonte: "not_reproduced",
                nota: None,
            },
            MAIL => {
                let mut c = da_arvore(id, None, ARV);
                let b = c.medida.valor().unwrap_or(0);
                match f.mail_purgeable {
                    Some(p) if b - p >= 0 => {
                        if let Medida::Valor(_) = c.medida {
                            c.medida = Medida::Valor(b - p);
                        }
                    }
                    Some(_) => c.medida = Medida::NaoMedida("negative after the purgeable: unknown, as in the Apple extension".into()),
                    None => {
                        c.grau = Grau::B;
                        c.nota = Some("ceiling: the MailCacheDelete purgeable was not read");
                    }
                }
                c
            }
            MENSAGENS => do_banco(id, &f.mensagens, "messages_db"),
            OUTROS => {
                let mut c = da_arvore(id, None, ARV);
                if f.outras_homes.is_some_and(|n| n > 0) && !root {
                    let b = c.medida.valor().unwrap_or(0);
                    c.medida = Medida::Parcial(b, "another account on this Mac needs administrator rights".into());
                    c.grau = Grau::B;
                }
                c
            }
            _ => unreachable!(),
        };
        cats.push(c);
    }
    Resultado { categorias: cats, nao_colocado: aberto, motivos: motivos.into_iter().collect() }
}

/// As linhas do `top` do JSON como a regra as vê. Com `arvore`, as linhas `tree` saem do JSON e
/// entram os baldes sem dobrar.
fn linhas_do_json(v: &Value, arvore: Option<&Arvore>) -> Option<Vec<Linha>> {
    let mut linhas: Vec<Linha> = v
        .get("top")?
        .as_array()?
        .iter()
        .filter(|x| arvore.is_none() || x["term"] != "tree")
        .filter_map(|x| Some(Linha { path: x["path"].as_str().map(String::from), term: x["term"].as_str()?.to_string(), bytes: x["bytes"].as_i64()?, folded: x["folded"] == true }))
        .collect();
    for (p, b) in arvore.map(|a| a.baldes.as_slice()).unwrap_or_default() {
        linhas.push(Linha { path: Some(p.clone()), term: "tree".into(), bytes: *b, folded: false });
    }
    Some(linhas)
}

/// `system_data.by_line`: Σ de `top[].bytes` por `settings_line`, do maior para o menor. Só soma o
/// que o `top` já diz; `grade_b_bytes` é a parte que veio de regra de grau B.
pub fn por_linha(v: &Value) -> Option<Value> {
    let mut m: BTreeMap<&str, (i64, i64, u64)> = BTreeMap::new();
    for x in v.get("top")?.as_array()? {
        let (Some(l), Some(b)) = (x["settings_line"].as_str(), x["bytes"].as_i64()) else { continue };
        let e = m.entry(l).or_default();
        e.0 += b;
        e.1 += if x["settings_line_grade"] == "B" { b } else { 0 };
        e.2 += 1;
    }
    let mut v: Vec<_> = m.into_iter().collect();
    v.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(b.0)));
    Some(json!(v.into_iter().map(|(l, (b, gb, n))| json!({"line": l, "bytes": b, "grade_b_bytes": gb, "rows": n})).collect::<Vec<_>>()))
}

/// Erro em % de `a` contra `b`, 2 casas; None sem referência.
fn erro_pct(a: i64, b: i64) -> Option<f64> {
    if b > 0 { Some(crate::coleta::round2((a - b) as f64 * 100.0 / b as f64)) } else if a == 0 { Some(0.0) } else { None }
}

/// Põe `system_data.reproduced` no JSON completo; sem `top` não faz nada. Compara com as categorias
/// do log da tela quando o painel foi aberto na última hora.
pub fn anexar(v: &mut Value, f: &Fatos) {
    let Some(top) = linhas_do_json(v, f.arvore.as_ref()) else { return };
    let macos = v["macos_version"].as_str().and_then(|s| s.split('.').next()?.parse().ok());
    let interrompida = v["incomplete_reasons"].as_array().is_some_and(|a| a.iter().any(|x| x == "interrupted"));
    let r = reproduzir(&top, f, macos, v["root"] == true, interrompida);
    let tela: Option<BTreeMap<String, i64>> = v["settings_screen"]["screen"]["categories"]
        .as_array()
        .filter(|a| !a.is_empty())
        .map(|a| a.iter().filter_map(|c| Some((c["id"].as_str()?.to_string(), c["bytes"].as_i64()?))).collect());
    let mut nossa = 0i64;
    let mut sem_reproduzir = 0i64;
    let mut faltam = 0;
    let mut todas_medidas = true;
    let cats: Vec<Value> = r
        .categorias
        .iter()
        .map(|c| {
            let (estado, motivo) = c.medida.estado_json();
            let b = c.medida.valor();
            let na_tela = tela.as_ref().map(|t| t.get(c.id).copied().unwrap_or(0));
            nossa += b.unwrap_or(0);
            if b.is_none() {
                faltam += 1;
                sem_reproduzir += na_tela.unwrap_or(0);
            }
            todas_medidas &= estado == "measured";
            json!({
                "id": c.id, "bytes": b, "state": estado, "reason": motivo, "grade": c.grau.letra(), "source": c.fonte, "note": c.nota,
                "screen_bytes": na_tela,
                "error_bytes": b.zip(na_tela).map(|(a, t)| a - t),
                "error_percent": b.zip(na_tela).and_then(|(a, t)| erro_pct(a, t)),
            })
        })
        .collect();
    let total_tela: Option<i64> = tela.as_ref().map(|t| t.values().sum());
    let erro = total_tela.map(|t| nossa - t);
    let pct = total_tela.and_then(|t| erro_pct(nossa, t));
    let bloco = json!({
        "state": if todas_medidas { "measured" } else { "partial" },
        "categories": cats,
        "total": {
            "bytes": nossa,
            "screen_bytes": total_tela,
            "error_bytes": erro,
            "error_percent": pct,
            "tolerance_percent": TOLERANCIA_PCT,
            "within_tolerance": pct.map(|p| p.abs() <= TOLERANCIA_PCT),
            "not_reproduced": faltam,
            "not_reproduced_screen_bytes": tela.as_ref().map(|_| sem_reproduzir),
        },
        "unplaced": {
            "bytes": r.nao_colocado,
            "reasons": r.motivos.iter().map(|m| match m {
                Aberto::Containers => "app_containers",
                Aberto::MesaDocumentos => "desktop_documents",
                Aberto::Outro => "unexpected_path",
            }).collect::<Vec<_>>(),
        },
        "inputs": {
            "apps_listed": f.apps.as_ref().map(BTreeSet::len),
            "desktop_in_icloud": f.mesa,
            "documents_in_icloud": f.documentos,
            "other_accounts": f.outras_homes,
            "mail_purgeable_read": f.mail_purgeable.is_some(),
            "tree_rows": if f.arvore.is_some() { "unfolded_buckets" } else { "top_rows" },
            "clone_copies_bytes_added": f.arvore.as_ref().map(|a| a.clones),
        },
    });
    if !v["system_data"].is_object() {
        v["system_data"] = json!({});
    }
    v["system_data"]["reproduced"] = bloco;
}

/// O que o `sdtally --json` faz depois do `catalogo::anexar`: lê os fatos desta máquina e anexa
/// `system_data.reproduced`. O tempo vai em `stage_seconds.categories`; falha de fato, em
/// `stage_errors` (etapa `categories`). Interrompido: sem fatos, as categorias que dependem deles
/// saem parciais ou não medidas.
pub fn anexar_coletando(v: &mut Value, home: &str, fda: Option<bool>, purgeable: Option<&Value>, arvore: Option<&crate::walk::Result>, interrompida: bool) {
    let t = std::time::Instant::now();
    let mut erros = Vec::new();
    let f = if interrompida { Fatos::default() } else { fatos::coletar(home, fda, purgeable, arvore, &mut erros) };
    anexar(v, &f);
    if v["stage_seconds"].is_object() {
        v["stage_seconds"]["categories"] = json!(crate::coleta::round2(t.elapsed().as_secs_f64()));
    }
    if let Some(l) = v["stage_errors"].as_array_mut() {
        l.extend(erros.into_iter().map(|e| json!({"stage": "categories", "error": e})));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(p: &str, b: i64) -> Linha {
        Linha { path: Some(p.into()), term: "tree".into(), bytes: b, folded: false }
    }

    fn por_id<'a>(r: &'a Resultado, id: &str) -> &'a Categoria {
        r.categorias.iter().find(|c| c.id == id).unwrap()
    }

    #[test]
    fn soma_por_categoria() {
        let f = Fatos {
            apps: Some(BTreeSet::from(["com.exemplo.A".to_string()])),
            mesa: Some(false),
            documentos: Some(false),
            musica: Some(Medida::Valor(900)),
            mensagens: Some(Medida::Valor(5)),
            ..Fatos::default()
        };
        let top = [
            l("/Applications/X.app/Contents", 100),
            l("~/Library/Application Support/Y", 50),
            l("~/Library/Containers/com.exemplo.A/Data", 30),
            l("~/Library/Containers/com.exemplo.B/Data", 7),
            l("~/dev", 200),
            l("~/Documents", 10),
            l("~/.Trash", 3),
            l("~/Music/Music/Media.localized", 1000),
            l("~/Pictures/Photos Library.photoslibrary", 40),
            l("~/Library/Caches", 999),
        ];
        let r = reproduzir(&top, &f, Some(27), false, false);
        assert_eq!(por_id(&r, regras::APPS).medida, Medida::Valor(180));
        assert_eq!(por_id(&r, regras::APPS).grau, Grau::B);
        assert_eq!(por_id(&r, DOCS).medida, Medida::Valor(210));
        assert_eq!(por_id(&r, DOCS).grau, Grau::A);
        assert_eq!(por_id(&r, LIXO).medida, Medida::Valor(3));
        // Música vem do banco, não da pasta.
        assert_eq!(por_id(&r, MUSICA).medida, Medida::Valor(900));
        assert_eq!(por_id(&r, FOTOS).medida, Medida::Valor(40));
        assert_eq!(por_id(&r, MENSAGENS).medida, Medida::Valor(5));
        assert!(matches!(por_id(&r, regras::ICLOUD).medida, Medida::NaoMedida(_)));
        assert!(matches!(por_id(&r, TV).medida, Medida::NaoMedida(_)));
        assert_eq!(r.nao_colocado, 0);
    }

    #[test]
    fn sem_fatos_vira_parcial_e_nao_zero() {
        let top = [l("~/Library/Containers/com.exemplo.A/Data", 30), l("~/Documents", 10), l("/Applications/X.app", 100)];
        let r = reproduzir(&top, &Fatos::default(), Some(27), false, false);
        assert_eq!(por_id(&r, regras::APPS).medida, Medida::Parcial(100, "app containers not classified (no Spotlight app list)".into()));
        assert!(matches!(por_id(&r, DOCS).medida, Medida::Parcial(0, _)));
        assert_eq!(r.nao_colocado, 40);
        assert_eq!(r.motivos, vec![Aberto::Containers, Aberto::MesaDocumentos]);
    }

    #[test]
    fn mail_e_outros_usuarios() {
        let top = [l("~/Library/Mail/V10", 100), l("/Users/Shared", 4)];
        let mut f = Fatos::default();
        assert_eq!(por_id(&reproduzir(&top, &f, None, false, false), MAIL).grau, Grau::B);
        f.mail_purgeable = Some(30);
        assert_eq!(por_id(&reproduzir(&top, &f, None, false, false), MAIL).medida, Medida::Valor(70));
        f.mail_purgeable = Some(300);
        assert!(matches!(por_id(&reproduzir(&top, &f, None, false, false), MAIL).medida, Medida::NaoMedida(_)));
        f.outras_homes = Some(1);
        assert!(matches!(por_id(&reproduzir(&top, &f, None, false, false), OUTROS).medida, Medida::Parcial(4, _)));
        assert_eq!(por_id(&reproduzir(&top, &f, None, true, false), OUTROS).medida, Medida::Valor(4));
        // Travessia interrompida: tudo o que vem da árvore é piso.
        assert!(matches!(por_id(&reproduzir(&top, &f, None, true, true), OUTROS).medida, Medida::Parcial(4, _)));
    }

    #[test]
    fn por_linha_soma_o_top() {
        let v = json!({"top": [
            {"bytes": 10, "settings_line": "system_data", "settings_line_grade": "A"},
            {"bytes": 5, "settings_line": "system_data", "settings_line_grade": "B"},
            {"bytes": 7, "settings_line": "mixed", "settings_line_grade": "B"},
            {"bytes": 99, "settings_line": null},
            {"bytes": 20, "settings_line": "com.apple.STMExtension.Documents", "settings_line_grade": "A"},
        ]});
        assert_eq!(
            por_linha(&v).unwrap(),
            json!([
                {"line": "com.apple.STMExtension.Documents", "bytes": 20, "grade_b_bytes": 0, "rows": 1},
                {"line": "system_data", "bytes": 15, "grade_b_bytes": 5, "rows": 2},
                {"line": "mixed", "bytes": 7, "grade_b_bytes": 7, "rows": 1},
            ])
        );
        assert!(por_linha(&json!({})).is_none());
    }

    #[test]
    fn anexar_compara_com_a_tela() {
        let mut v = json!({
            "macos_version": "27.0", "root": false, "incomplete_reasons": [],
            "top": [{"path": "~/.Trash", "term": "tree", "bytes": 100, "folded": false}],
            "settings_screen": {"screen": {"categories": [{"id": regras::LIXO, "bytes": 100}, {"id": regras::ICLOUD, "bytes": 30}]}},
        });
        anexar(&mut v, &Fatos::default());
        let r = &v["system_data"]["reproduced"];
        let lixo = r["categories"].as_array().unwrap().iter().find(|c| c["id"] == regras::LIXO).unwrap();
        assert_eq!((lixo["bytes"].as_i64(), lixo["error_bytes"].as_i64(), lixo["error_percent"].as_f64()), (Some(100), Some(0), Some(0.0)));
        assert_eq!(r["total"]["screen_bytes"], 130);
        // Falta o iCloud Drive: 30 B da tela sem reprodução explicam o erro.
        assert_eq!(r["total"]["not_reproduced_screen_bytes"], 30);
        assert_eq!(r["total"]["within_tolerance"], false);
        assert_eq!(r["state"], "partial");
        // Sem log da tela, não há erro nem veredito.
        let mut s = json!({"macos_version": "27.0", "top": [{"path": "~/.Trash", "term": "tree", "bytes": 1, "folded": false}]});
        anexar(&mut s, &Fatos::default());
        assert_eq!(s["system_data"]["reproduced"]["total"]["within_tolerance"], Value::Null);
    }
}

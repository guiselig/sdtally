//! A conta que o sdtally mostra (PLANO §3, Frente E §1.2): modelo puro, montado a partir da
//! coleta, e o JSON. O texto fica em `texto`, os idiomas em `i18n`.

pub mod i18n;
pub mod texto;

use crate::anon::DOBRADO;
use crate::coleta::{self, Coleta};
use crate::ledger::{self, Medida};
use crate::sources::snapshots;
use i18n::Lang;
use serde_json::{Value, json};

pub(crate) const SCHEMA_VERSION: u32 = 1;

/// De onde vem cada número (vira enum no JSON e texto traduzido na tela).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fonte {
    Travessia,
    Xattr,
    Snapshots,
    Lsof,
    DirStats,
    VolSpaceUsed,
    Diferenca,
    CapacityInUse,
    DirStatsStm,
    LogPainel,
    CacheDelete,
    AddedTime,
    ApfsUserClient,
}

impl Fonte {
    pub fn chave(self) -> &'static str {
        match self {
            Fonte::Travessia => "getattrlistbulk",
            Fonte::Xattr => "listxattr",
            Fonte::Snapshots => "fsctl_snapshot_private_size",
            Fonte::Lsof => "lsof_proc_pidfdinfo",
            Fonte::DirStats => "fsctl_dir_stats",
            Fonte::VolSpaceUsed => "attr_vol_spaceused",
            Fonte::Diferenca => "difference",
            Fonte::CapacityInUse => "diskutil_capacity_in_use",
            Fonte::DirStatsStm => "dir_stats_like_settings",
            Fonte::LogPainel => "storage_management_log",
            Fonte::CacheDelete => "cachedelete",
            Fonte::AddedTime => "getattrlist_addedtime",
            Fonte::ApfsUserClient => "apfs_user_client",
        }
    }
}

/// Os termos da conta, na ordem da síntese §2.3 (metadados do APFS por último).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chave {
    Arvore,
    Xattr,
    Snapshots,
    Apagados,
    Negadas,
    Metadados,
}

impl Chave {
    pub fn json(self) -> &'static str {
        match self {
            Chave::Arvore => "tree",
            Chave::Xattr => "large_xattrs",
            Chave::Snapshots => "snapshots",
            Chave::Apagados => "deleted_open",
            Chave::Negadas => "protected_folders",
            Chave::Metadados => "apfs_metadata",
        }
    }

    pub fn fonte(self) -> Fonte {
        match self {
            Chave::Arvore => Fonte::Travessia,
            Chave::Xattr => Fonte::Xattr,
            Chave::Snapshots => Fonte::Snapshots,
            Chave::Apagados => Fonte::Lsof,
            Chave::Negadas => Fonte::DirStats,
            Chave::Metadados => Fonte::ApfsUserClient,
        }
    }
}

/// Uma linha somável do ranking "o que mais pesa".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Caminho (home como ~). Vazio nas linhas de termo único (snapshots, apagados, xattr).
    pub nome: String,
    pub termo: Chave,
    pub bytes: i64,
    /// Só a travessia mede o privado ("libera se apagar").
    pub privado: Option<i64>,
    /// Piso: o termo saiu parcial (interrompido, arquivo sem alocado legível...).
    pub parcial: bool,
    /// Snapshots: quantidade.
    pub quantidade: Option<usize>,
    /// Apagados: (processo que mais segura, bytes dele).
    pub processo: Option<(String, i64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tela {
    pub macos: Medida,
    pub ai: Medida,
    pub ai_fisico: Medida,
    /// Último "System Data" do log do serviço: (bytes, "AAAA-MM-DD HH:MM"). Ok(None): log lido,
    /// sem registro. Err(motivo): o log não foi lido.
    pub dados_do_sistema: Result<Option<(i64, String)>, String>,
    /// A tela pelo próprio log (linha StorageUI + bloco do serviço); None = log não lido.
    pub log_tela: Option<crate::sources::painel::TelaLog>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Repetidos {
    pub clones: (u64, i64),
    pub hardlinks: (u64, i64),
    /// Arquivos com blocos divididos com outro (clone parcial) e os bytes que saíram do alocado
    /// para contar cada bloco uma vez.
    pub compartilhados: (u64, i64),
    pub enxertos: u64,
    /// (arquivos, bytes aparentes, pastas não abertas).
    pub dataless: (u64, i64, u64),
    /// Travessia interrompida: tudo acima é piso.
    pub parcial: bool,
}

/// Um processo que segura apagados-abertos (`bytes` None: nenhum arquivo com alocado legível).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Processo {
    pub nome: String,
    pub bytes: Option<i64>,
    pub arquivos: u64,
    pub sem_alocado: u64,
    /// SIZE do lsof dos arquivos sem alocado legível (lógico, fora da soma).
    pub logico_sem_alocado: i64,
    /// Inodes únicos em com.apple.LaunchServices.dv, contados antes da deduplicação.
    pub dv_launchservices: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conta {
    pub ferramenta: String,
    pub data_hora: String,
    pub macos: Option<String>,
    /// Ligado desde (kern.boottime), "AAAA-MM-DD HH:MM" local.
    pub boot_time: Option<String>,
    pub root: bool,
    /// None: não deu para saber.
    pub fda: Option<bool>,
    /// Pastas que a travessia não pôde abrir (EPERM/EACCES).
    pub negadas: usize,
    pub interrompida: bool,
    pub tela: Tela,
    pub usado: Medida,
    pub termos: Vec<(Chave, Medida)>,
    /// Todas as linhas somáveis medidas, da maior para a menor.
    pub ranking: Vec<Item>,
    /// Usado − Σ termos, com sinal. Negativo é contagem dupla; use `sem_explicacao`/`sobreposicao`.
    pub resto: Option<i64>,
    pub purgeable: Medida,
    pub purgeable_top: Vec<(String, i64)>,
    /// JSON bruto dos volumes (`sources::volumes`), vira `containers`.
    pub volumes: Value,
    /// (total, usado) do swap.
    pub swap: Option<(u64, u64)>,
    /// Cada snapshot; None se a etapa não rodou.
    pub snapshots: Option<Vec<crate::sources::snapshots::Snap>>,
    /// Bloco `informational.time_machine`; Null se a etapa não rodou.
    pub time_machine: Value,
    /// Os 10 processos que mais seguram apagados-abertos; None sem lsof.
    pub processos: Option<Vec<Processo>>,
    /// None sem travessia.
    pub repetidos: Option<Repetidos>,
    /// (etapa, erro) das etapas da coleta.
    pub erros: Vec<(String, String)>,
    /// Segundos de cada etapa da coleta (nome interno da etapa), de `Coleta::tempos`.
    pub tempos: serde_json::Map<String, Value>,
    /// `~/.Trash` por idade (Err = motivo de não medido); None se a etapa não rodou.
    pub lixo: Option<Result<crate::sources::lixo::Lixo, String>>,
    /// Metadados das árvores vivas do APFS no Data; None se a etapa não rodou.
    pub apfs_meta: Option<crate::sources::apfs_meta::ApfsMeta>,
    /// Pastas negadas sem dir-stats: (balde, errno; None = dir-stats devolveu 0). None sem a etapa.
    pub negadas_sem_tamanho: Option<Vec<(String, Option<i32>)>>,
}

impl Conta {
    pub fn medida(&self, k: Chave) -> Option<&Medida> {
        self.termos.iter().find(|t| t.0 == k).map(|t| &t.1)
    }

    /// O resto que se pode mostrar: nunca negativo.
    pub fn sem_explicacao(&self) -> Option<i64> {
        self.resto.filter(|r| *r >= 0)
    }

    /// Quanto Σ termos passou do usado. O dir-stats das pastas protegidas conta clone em dobro e
    /// bytes que a travessia já contou por outro caminho (ex.: cache do iCloud clonado de
    /// ~/Library/Mobile Documents); sem abrir a pasta não há como separar. Aí o resto não existe.
    pub fn sobreposicao(&self) -> Option<i64> {
        self.resto.filter(|r| *r < 0).map(|r| -r)
    }

    /// Motivos do INCOMPLETO, na ordem em que aparecem: "interrupted", "no_root", "no_full_disk_access",
    /// "stage_failed" (alguma etapa da coleta registrou erro ou panic; o termo dela fica fora da conta).
    pub fn incompleto(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.interrompida {
            v.push("interrupted");
        }
        if !self.root {
            v.push("no_root");
        }
        if self.fda == Some(false) {
            v.push("no_full_disk_access");
        }
        if !self.erros.is_empty() {
            v.push("stage_failed");
        }
        v
    }
}

pub fn montar(c: &Coleta, ferramenta: &str, data_hora: &str) -> Conta {
    let t = c.termos();
    let usado = c.usado.clone();
    let resto = ledger::resto(usado.valor(), t.iter().map(|x| &x.1.medida));
    let termos: Vec<(Chave, Medida)> = t.into_iter().map(|(k, x)| (k, x.medida)).collect();
    let parcial = |k: Chave| matches!(termos.iter().find(|t| t.0 == k), Some((_, Medida::Parcial(..))));
    let unico = |k: Chave| Item { nome: String::new(), termo: k, bytes: 0, privado: None, parcial: parcial(k), quantidade: None, processo: None };

    let mut ranking = Vec::new();
    if let Some(r) = &c.arvore {
        for (k, s) in coleta::baldes(r) {
            ranking.push(Item { nome: k, bytes: s.alloc, privado: Some(s.private), ..unico(Chave::Arvore) });
        }
    }
    if let Some(n) = &c.negadas {
        for (k, b) in &n.baldes {
            ranking.push(Item { nome: k.clone(), bytes: *b, parcial: false, ..unico(Chave::Negadas) });
        }
    }
    for (k, m) in &termos {
        let Some(v) = m.valor() else { continue };
        match k {
            Chave::Snapshots => {
                let n = c.snaps.as_ref().and_then(|s| s.json["quantidade"].as_u64()).map(|q| q as usize);
                ranking.push(Item { bytes: v, quantidade: n, ..unico(*k) });
            }
            Chave::Apagados => {
                // O de maior alocado medido; a lista vem por max(alocado, lógico).
                let pp = c.lsof["por_processo"].as_array().map(Vec::as_slice).unwrap_or_default();
                let p = pp.iter().filter(|p| p["bytes"].is_i64()).max_by_key(|p| p["bytes"].as_i64());
                let processo = p.and_then(|p| p["processo"].as_str().zip(p["bytes"].as_i64())).map(|(n, b)| (n.to_string(), b));
                ranking.push(Item { bytes: v, processo, ..unico(*k) });
            }
            Chave::Xattr | Chave::Metadados => ranking.push(Item { bytes: v, ..unico(*k) }),
            Chave::Arvore | Chave::Negadas => {}
        }
    }
    ranking.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.nome.cmp(&b.nome)));

    let falta = || Medida::NaoMedida(if c.interrompida { "interrompido".into() } else { "falha interna".into() });
    let tela = Tela {
        macos: c.macos.clone(),
        ai: c.painel.as_ref().map_or_else(falta, |p| p.apple_intelligence.clone()),
        ai_fisico: c.painel.as_ref().map_or_else(falta, |p| p.apple_intelligence_fisico.clone()),
        dados_do_sistema: match &c.painel {
            Some(p) => p.log_dados_do_sistema.clone().map(|o| o.map(|(b, ts)| (b, ts.chars().take(16).collect()))),
            None => Err(if c.interrompida { "interrompido".into() } else { "falha interna".into() }),
        },
        log_tela: c.tela_log.clone(),
    };
    Conta {
        ferramenta: ferramenta.into(),
        data_hora: data_hora.into(),
        macos: c.caps.macos.clone(),
        boot_time: c.boot_time.clone(),
        root: c.root,
        fda: c.fda.as_bool(),
        negadas: c.arvore.as_ref().map_or(0, |r| r.denied.len()),
        interrompida: c.interrompida,
        tela,
        usado,
        termos,
        ranking,
        resto,
        purgeable: c.purg_total.clone(),
        purgeable_top: c.purg.as_ref().map(|p| p.top.clone()).unwrap_or_default(),
        volumes: c.volumes.clone(),
        swap: c.swap,
        snapshots: c.snaps.as_ref().map(|s| s.itens.clone()),
        time_machine: c.snaps.as_ref().map_or(Value::Null, |s| s.tm.clone()),
        processos: c.lsof["por_processo"].as_array().map(|v| {
            v.iter()
                .take(10)
                .map(|p| Processo {
                    nome: p["processo"].as_str().unwrap_or("").to_string(),
                    bytes: p["bytes"].as_i64(),
                    arquivos: p["arquivos"].as_u64().unwrap_or(0),
                    sem_alocado: p["sem_alocado"].as_u64().unwrap_or(0),
                    logico_sem_alocado: p["logico_sem_alocado"].as_i64().unwrap_or(0),
                    dv_launchservices: p["launchservices_dv"].as_u64().unwrap_or(0),
                })
                .collect()
        }),
        repetidos: c.arvore.as_ref().map(|r| Repetidos {
            clones: (r.clone_dup_files, r.clone_dup_bytes),
            hardlinks: (r.link_dup_files, r.link_dup_bytes),
            compartilhados: (r.shared_dup_files, r.shared_dup_bytes),
            enxertos: r.graft_roots,
            dataless: (r.total.dataless_n, r.total.dataless_logical, r.total.dataless_dirs),
            parcial: r.interrompida,
        }),
        erros: c
            .erros
            .iter()
            .map(|e| (e["etapa"].as_str().unwrap_or("").to_string(), e["erro"].as_str().map_or_else(|| e["erro"].to_string(), String::from)))
            .collect(),
        tempos: c.tempos.clone(),
        lixo: c.lixo.clone(),
        apfs_meta: c.apfs_meta.clone(),
        negadas_sem_tamanho: c.negadas.as_ref().map(|n| n.sem_tamanho.clone()),
    }
}

fn medida_json(m: &Medida, fonte: Fonte) -> Value {
    let (estado, motivo) = m.estado_json();
    json!({"bytes": m.valor(), "state": estado, "reason": motivo.map(|r| Lang::En.motivo(r)), "source": fonte.chave()})
}

/// Resto em % do usado, com 2 casas.
pub fn pct(resto: i64, usado: i64) -> Option<f64> {
    (usado > 0).then(|| coleta::round2(resto as f64 * 100.0 / usado as f64))
}

/// `informational.trash`: o Lixo e a parte com mais de 30 dias, cada um no formato Medida mais `items`.
fn lixo_json(l: Option<&Result<crate::sources::lixo::Lixo, String>>) -> Value {
    use crate::sources::lixo::LIMITE_S;
    let comum = |m: &Medida, itens: Option<u64>| {
        let mut v = medida_json(m, Fonte::AddedTime);
        v["items"] = json!(itens);
        v
    };
    let (total, velho, sem_data) = match l {
        None => return Value::Null,
        Some(Err(r)) => (comum(&Medida::NaoMedida(r.clone()), None), comum(&Medida::NaoMedida(r.clone()), None), Value::Null),
        Some(Ok(x)) => {
            let total = if x.piso { Medida::Parcial(x.bytes, "algum item sem tamanho lido".into()) } else { Medida::Valor(x.bytes) };
            (comum(&total, Some(x.itens)), comum(&x.medida_velhos(), Some(x.velhos_itens)), json!({"items": x.sem_data_itens, "bytes": x.sem_data_bytes}))
        }
    };
    json!({
        "scope": "~/.Trash",
        "threshold_days": LIMITE_S / 86_400,
        "total": total,
        "older": velho,
        "undated": sem_data,
        "source": Fonte::AddedTime.chave(),
    })
}

/// `informational.apfs_metadata`: nós das árvores vivas × bloco; o valor também é o termo `apfs_metadata`.
fn apfs_meta_json(m: Option<&crate::sources::apfs_meta::ApfsMeta>) -> Value {
    let Some(m) = m else { return Value::Null };
    let mut v = medida_json(&m.medida, Fonte::ApfsUserClient);
    v["nodes"] = json!(m.detalhe.map(|d| json!({"omap": d.nos_omap, "fsroot": d.nos_fsroot, "extentref": d.nos_extentref})));
    v["block_size"] = json!(m.detalhe.map(|d| d.bloco));
    v
}

/// Máximo de pastas listadas em `informational.protected_without_size.folders`; `count` é o total.
const LIMITE_SEM_TAMANHO: usize = 200;

/// `informational.protected_without_size`: pastas negadas sem dir-stats (errno ou 0), pelo balde.
fn sem_tamanho_json(v: Option<&[(String, Option<i32>)]>) -> Value {
    let Some(v) = v else { return Value::Null };
    json!({
        "count": v.len(),
        "folders": v.iter().take(LIMITE_SEM_TAMANHO).map(|(p, e)| json!({"path": p, "errno": e})).collect::<Vec<_>>(),
    })
}

/// O relatório completo em JSON (`sdtally --json`): conta, conselhos, o que não voltou, catálogo
/// e categorias. `home` resolve o `~` dos caminhos do ranking (com sudo é a home do SUDO_USER).
/// `fatos`: os das categorias já lidos (o `--auto` guarda os seus); None lê agora.
pub fn json_completo(c: &Coleta, ferramenta: &str, data_hora: &str, home: &str, fatos: Option<&crate::sources::categorias::Fatos>) -> Value {
    let conta = montar(c, ferramenta, data_hora);
    let mut v = json(&conta);
    v["home"] = Value::from(home);
    v["advice"] = crate::conselho::json(&crate::conselho::conselhos(&conta));
    crate::conselho::detectar(&mut v);
    v["not_freed"] = crate::voltou::json(&crate::voltou::causas(&conta));
    // Só a travessia única guarda arquivo; o modo vivo (pasta) sai null.
    v["largest_files"] = json!(c.arvore.as_ref().and_then(|r| r.maiores.as_ref()).map(|m| m
        .iter()
        .map(|(p, a, pr)| {
            let p = p.to_string_lossy();
            json!({"path": p.strip_prefix(crate::sources::DATA).filter(|r| r.starts_with('/')).unwrap_or(&p), "bytes": a, "private_bytes": pr})
        })
        .collect::<Vec<_>>()));
    crate::catalogo::anexar(&mut v);
    match fatos {
        Some(f) => crate::sources::categorias::anexar(&mut v, f),
        None => crate::sources::categorias::anexar_coletando(&mut v, home, c.fda.as_bool(), c.purg.as_ref().map(|p| &p.json), c.arvore.as_ref(), c.interrompida),
    }
    v
}

pub fn json(c: &Conta) -> Value {
    let inc = c.incompleto();
    let rest_state = match (c.sem_explicacao(), c.termos.iter().all(|t| matches!(t.1, Medida::Valor(_)))) {
        (None, _) => "not_measured",
        (Some(_), true) => "measured",
        // Termo que faltou fica dentro do resto: é um teto, não uma medida limpa.
        (Some(_), false) => "partial",
    };
    let mut ai = medida_json(&c.tela.ai, Fonte::DirStatsStm);
    ai["physical"] = medida_json(&c.tela.ai_fisico, Fonte::DirStats);
    let mut purg = medida_json(&c.purgeable, Fonte::CacheDelete);
    purg["top_services"] = json!(c.purgeable_top.iter().map(|(n, b)| json!({"name": n, "bytes": b})).collect::<Vec<_>>());
    let sd = c.tela.dados_do_sistema.as_ref().ok().and_then(|x| x.as_ref());
    json!({
        "schema_version": SCHEMA_VERSION,
        "tool": c.ferramenta,
        "generated_at": c.data_hora,
        "macos_version": c.macos,
        "boot_time": c.boot_time,
        "complete": inc.is_empty(),
        "incomplete_reasons": inc,
        "root": c.root,
        "full_disk_access": c.fda,
        "denied_folders": c.negadas,
        "containers": containers_json(&c.volumes),
        "swap": swap_json(c.swap),
        "settings_screen": {
            "macos": medida_json(&c.tela.macos, Fonte::CapacityInUse),
            "apple_intelligence": ai,
            "system_data": {
                "bytes": sd.map(|x| x.0),
                "state": if sd.is_some() { "measured" } else { "not_measured" },
                "reason": match &c.tela.dados_do_sistema {
                    Ok(Some(_)) => None,
                    Ok(None) => Some("no StorageManagementService record in the log (kept for less than ~21 h); open System Settings > General > Storage and run again".to_string()),
                    Err(r) => Some(Lang::En.motivo(r)),
                },
                "logged_at": sd.map(|x| &x.1),
                "source": Fonte::LogPainel.chave(),
            },
            "screen": tela_json(c.tela.log_tela.as_ref(), c.tela.macos.valor()),
        },
        "data_volume": {
            "used": medida_json(&c.usado, Fonte::VolSpaceUsed),
            "terms": c.termos.iter().map(|(k, m)| {
                let mut v = medida_json(m, k.fonte());
                v["key"] = json!(k.json());
                v
            }).collect::<Vec<_>>(),
            "unexplained": {
                "bytes": c.sem_explicacao(),
                "percent_of_used": c.sem_explicacao().zip(c.usado.valor()).and_then(|(r, u)| pct(r, u)),
                "state": rest_state,
                "reason": c.sobreposicao().map(|_| "the measured terms add up to more than the used space: dir-stats of protected folders counts clones twice and bytes also counted in readable folders"),
                "overlap_bytes": c.sobreposicao(),
                "source": Fonte::Diferenca.chave(),
            },
        },
        "top": c.ranking.iter().map(|i| json!({
            "path": (!i.nome.is_empty()).then(|| i.nome.strip_suffix(DOBRADO).unwrap_or(&i.nome)),
            "folded": i.nome.ends_with(DOBRADO),
            "term": i.termo.json(),
            "bytes": i.bytes,
            "private_bytes": i.privado,
            "lower_bound": i.parcial,
            "count": i.quantidade,
            "top_process": i.processo.as_ref().map(|(n, b)| json!({"name": n, "bytes": b})),
            "source": i.termo.fonte().chave(),
        })).collect::<Vec<_>>(),
        "informational": {
            "purgeable": purg,
            "snapshots": c.snapshots.as_ref().map(|v| v.iter().map(|(k, d, p, m)| {
                let idade = d.as_deref().and_then(|d| snapshots::idade_h(&c.data_hora, d));
                json!({
                    "kind": k, "date": d, "private_bytes": p,
                    "purgeable": m.purgeable, "cumulative_bytes": m.cumulativo, "creator": m.criador,
                    "tm_role": m.papel, "verdict": snapshots::veredito(k, m.papel, idade),
                    // Preenchido pelo catalogo::anexar.
                    "card": null,
                })
            }).collect::<Vec<_>>()),
            "time_machine": c.time_machine,
            "deleted_open_processes": c.processos.as_ref().map(|v| v.iter().map(|p| json!({
                "name": p.nome, "bytes": p.bytes, "files": p.arquivos,
                "files_without_allocated": p.sem_alocado, "logical_bytes_without_allocated": p.logico_sem_alocado,
                "launchservices_dv_files": p.dv_launchservices,
            })).collect::<Vec<_>>()),
            "trash": lixo_json(c.lixo.as_ref()),
            "apfs_metadata": apfs_meta_json(c.apfs_meta.as_ref()),
            "protected_without_size": sem_tamanho_json(c.negadas_sem_tamanho.as_deref()),
            "scan_state": c.repetidos.as_ref().map(|r| if r.parcial { "partial" } else { "measured" }),
            "repeated_full_clones": c.repetidos.as_ref().map(|r| json!({"files": r.clones.0, "bytes": r.clones.1})),
            "repeated_hardlinks": c.repetidos.as_ref().map(|r| json!({"files": r.hardlinks.0, "bytes": r.hardlinks.1})),
            "repeated_shared_blocks": c.repetidos.as_ref().map(|r| json!({"files": r.compartilhados.0, "bytes": r.compartilhados.1})),
            "graft_roots_skipped": c.repetidos.as_ref().map(|r| r.enxertos),
            "dataless": c.repetidos.as_ref().map(|r| json!({"files": r.dataless.0, "apparent_bytes": r.dataless.1, "folders_not_opened": r.dataless.2})),
            "source": Fonte::Travessia.chave(),
        },
        "stage_errors": c.erros.iter().map(|(e, m)| json!({"stage": e, "error": Lang::En.motivo(m)})).collect::<Vec<_>>(),
        "stage_seconds": c.tempos,
    })
}

/// `containers` a partir do JSON bruto de `sources::volumes`, sem nome de volume nem de disco.
/// `in_settings_macos` é a mesma marca que soma o macOS da tela: Σ dela = `settings_screen.macos`.
pub fn containers_json(v: &Value) -> Value {
    let Some(cs) = v.as_array() else { return Value::Null };
    json!(cs.iter().map(|c| {
        let vols = c["volumes"].as_array().map(Vec::as_slice).unwrap_or_default();
        json!({
            "internal": match c["tipo"].as_str() { Some("interno") => json!(true), Some("externo") => json!(false), _ => Value::Null },
            "isc": c["isc_fora_do_macos"] == true,
            "holds_data": vols.iter().any(|x| x["e_o_data_de_boot"] == true),
            "ceiling": c["tamanho"],
            "free": c["livre"],
            "volumes": vols.iter().map(|x| json!({
                "roles": x["roles"],
                "capacity_in_use": x["capacity_in_use"],
                "in_settings_macos": x["no_macos_da_tela"] == true,
                "device": x["dispositivo"],
            })).collect::<Vec<_>>(),
        })
    }).collect::<Vec<_>>())
}

fn swap_json(s: Option<(u64, u64)>) -> Value {
    s.map_or(Value::Null, |(t, u)| json!({"total": t, "used": u, "source": "sysctl_vm_swapusage"}))
}

/// JSON do detalhe de pasta (`sdtally <caminho> --json`). Caminhos absolutos para o app poder
/// descer de novo; `home` resolve o `~` do `display`.
pub fn json_ramo(r: &crate::detalhe::Ramo, home: &str, ferramenta: &str) -> Value {
    let prot: i64 = r.filhos.iter().filter(|f| f.protegida).map(|f| f.alocado).sum();
    let base = r.raiz.trim_end_matches('/');
    json!({
        "schema_version": SCHEMA_VERSION,
        "kind": "branch",
        "tool": ferramenta,
        "home": home,
        "root": r.raiz,
        "display": texto::til(&r.raiz, home),
        "depth": r.profundidade,
        "complete": !r.interrompida && r.protegidas_sem == 0,
        "interrupted": r.interrompida,
        "children": r.filhos.iter().map(|f| json!({
            // Vazio = arquivos soltos na própria raiz.
            "name": f.nome,
            "path": if f.nome.is_empty() { r.raiz.clone() } else { format!("{base}/{}", f.nome) },
            "loose_files": f.nome.is_empty(),
            "bytes": f.alocado,
            "private_bytes": f.privado,
            "protected": f.protegida,
            "source": if f.protegida { Fonte::DirStats.chave() } else { Fonte::Travessia.chave() },
        })).collect::<Vec<_>>(),
        "total": {
            // Piso quando a leitura parou ou sobrou pasta protegida sem dir-stats.
            "bytes": r.total.alloc + prot,
            "lower_bound": r.interrompida || r.protegidas_sem > 0,
            "private_bytes": r.total.private,
            "files": r.total.files,
            "folders": r.dirs,
            "dataless_files": r.total.dataless_n,
            "dataless_apparent_bytes": r.total.dataless_logical,
        },
        "protected_without_size": r.protegidas_sem,
        "read_errors": r.erros,
        "other_volume_entries": r.outros_volumes,
    })
}

/// JSON dos atributos brutos (`sdtally why <caminho> --json`).
pub fn json_why(w: &crate::detalhe::Why, home: &str, ferramenta: &str) -> Value {
    use crate::detalhe::{Tipo, ext_flags_nomes, flags_nomes};
    let erro = |e: i32| std::io::Error::from_raw_os_error(e).to_string();
    let (attrs, attrs_err) = match &w.attrs {
        Ok(a) => (
            json!({
                "logical_bytes": a.logico,
                "allocated_bytes": a.alocado,
                "private_bytes": a.privado,
                "file_id": a.fileid,
                "clone_id": a.cloneid,
                "ext_flags": a.ext_flags,
                "ext_flag_names": a.ext_flags.map(ext_flags_nomes),
                "flags": a.flags,
                "flag_names": a.flags.map(flags_nomes),
                "device": a.devid,
                "links": a.links,
            }),
            None,
        ),
        Err(e) => (Value::Null, Some(e.clone())),
    };
    let (size, blocks, nlink, dev, flags) = w.st;
    json!({
        "schema_version": SCHEMA_VERSION,
        "kind": "item",
        "tool": ferramenta,
        "home": home,
        "path": w.caminho,
        "display": texto::til(&w.caminho, home),
        "type": match w.tipo { Tipo::Arquivo => "file", Tipo::Pasta => "folder", Tipo::Link => "symlink", Tipo::Outro => "other" },
        "attributes": attrs,
        "attributes_error": attrs_err,
        "lstat": {"size": size, "blocks_bytes": blocks, "links": nlink, "device": dev, "flags": flags, "flag_names": flags_nomes(flags)},
        "xattrs": match &w.xattrs {
            Ok(v) => json!({"count": v.len(), "bytes": v.iter().sum::<i64>(), "error": null}),
            Err(e) => json!({"count": null, "bytes": null, "error": erro(*e)}),
        },
        // Pasta só: raiz de enxerto (cryptex/MobileAsset) e conteúdo pelo dir-stats.
        "graft_root": w.enxerto.as_ref().map(|r| r.as_ref().ok().copied()),
        "graft_error": w.enxerto.as_ref().and_then(|r| r.as_ref().err().map(|e| erro(*e))),
        "dir_stats_bytes": w.dirstats.as_ref().and_then(|r| r.as_ref().ok().copied()),
        "dir_stats_error": w.dirstats.as_ref().and_then(|r| r.as_ref().err().map(|e| erro(*e))),
    })
}

/// A tela do Ajustes pelo próprio log: `used` = cabeçalho ao byte; `system_data` = used − Σ
/// extensões − macOS da tela. `not_measured` quando o painel não foi aberto na última hora.
pub fn tela_json(t: Option<&crate::sources::painel::TelaLog>, macos: Option<i64>) -> Value {
    let tela = t.and_then(|t| t.tela.as_ref());
    let sd = t.and_then(|t| t.dados_do_sistema(macos));
    let razao = match (t, tela, sd) {
        (None, ..) => Some("Settings log not read"),
        (Some(_), None, _) => Some("no Settings screen line in the log window read (last hour, or since the previous reading); open System Settings > General > Storage"),
        (Some(_), Some(_), None) if macos.is_none() => Some("macOS not measured"),
        (Some(_), Some(_), None) => Some("no StorageManagementService block before the screen line"),
        _ => None,
    };
    json!({
        "state": if sd.is_some() { "measured" } else if tela.is_some() { "partial" } else { "not_measured" },
        "reason": razao,
        "logged_at": tela.map(|x| &x.0),
        "used": tela.map(|x| x.1),
        "available": tela.map(|x| x.2),
        "capacity": tela.map(|x| x.3),
        "system_data": sd,
        // Σ categories, pronta: a ficha "outras linhas" do app não soma nada.
        "categories_total": t.and_then(|t| t.soma_extensoes()),
        "categories": t.map(|t| t.categorias().into_iter().map(|(id, b)| json!({"id": id, "bytes": b})).collect::<Vec<_>>()),
        "service": t.and_then(|t| t.servico.as_ref()).map(|(ts, m)| json!({
            "logged_at": ts,
            "total": m.get("totalSpace"), "available": m.get("availSpace"), "purgeable": m.get("purgeSpace"),
            "other_volumes": m.get("otherVols"), "used": m.get("Used"), "system": m.get("System"), "other": m.get("Other"),
        })),
        "source": "settings_ui_log",
    })
}

/// JSON do modo rápido (`sdtally --quick --json`, kind "quick").
pub fn json_rapido(r: &crate::coleta::Rapido, ferramenta: &str, data_hora: &str, home: &str) -> Value {
    let mut purg = medida_json(&r.purg_total, Fonte::CacheDelete);
    purg["top_services"] = json!(r.purg.as_ref().map(|p| p.top.iter().map(|(n, b)| json!({"name": n, "bytes": b})).collect::<Vec<_>>()));
    let mut tela = tela_json(r.tela_log.as_ref(), r.macos.valor());
    if r.tela_pulada {
        // Não é falha de leitura (o app trata "not read" como log ilegível): o painel estava fechado.
        tela["reason"] = json!("Settings Storage pane closed; screen log skipped (--since)");
    }
    let mut v = json!({
        "schema_version": SCHEMA_VERSION,
        "kind": "quick",
        "tool": ferramenta,
        "generated_at": data_hora,
        "macos_version": r.caps.macos,
        "boot_time": r.boot_time,
        "home": home,
        "root": r.root,
        "full_disk_access": r.fda.as_bool(),
        "interrupted": r.interrompida,
        "settings_screen": {
            "macos": medida_json(&r.macos, Fonte::CapacityInUse),
            "screen": tela,
        },
        "data_volume": {"used": medida_json(&r.usado, Fonte::VolSpaceUsed)},
        "containers": containers_json(&r.volumes),
        "swap": swap_json(r.swap),
        "capacity": r.capacidade.map(|(t, l)| json!({"total": t, "free": l, "source": "statfs"})),
        "purgeable": purg,
        "stage_errors": r.erros.iter().map(|e| json!({"stage": e["etapa"], "error": e["erro"].as_str().map_or_else(|| e["erro"].to_string(), |m| Lang::En.motivo(m))})).collect::<Vec<_>>(),
        "stage_seconds": r.tempos,
    });
    // --parts: some a chave da parte não lida; o app junta com a leitura anterior. A tela só sai
    // lida de verdade (pulada com --since sai como hoje, com o motivo).
    let p = r.partes;
    let o = v.as_object_mut().expect("objeto");
    for (lida, chaves) in [
        (p.meta, &["full_disk_access"][..]),
        (p.volumes, &["containers"][..]),
        (p.swap, &["swap"][..]),
        (p.uso, &["data_volume", "capacity"][..]),
        (p.purgeable, &["purgeable"][..]),
    ] {
        if !lida {
            for c in chaves {
                o.remove(*c);
            }
        }
    }
    let ss = o["settings_screen"].as_object_mut().expect("objeto");
    if !p.volumes {
        ss.remove("macos");
    }
    if !p.tela {
        ss.remove("screen");
    }
    v
}

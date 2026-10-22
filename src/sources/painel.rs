//! O painel do Ajustes: log do StorageManagementService e as fórmulas da tela reproduzidas
//! (macOS e Apple Intelligence, síntese §1c).

use crate::anon::Ctx;
use crate::caps::Caps;
use crate::ledger::Medida;
use crate::sys;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::sync::Arc;

const ASSETS: &str = "/System/Library/AssetsV2";
/// Saída do `log show` guardada inteira (a tela: poucas linhas).
pub type Log = Result<String, String>;
/// Resultado de uma leitura em fluxo; Err = não leu.
pub type Lido<T> = Result<T, String>;

const UAF_FM: &str = "/System/Library/AssetsV2/com_apple_MobileAsset_UAF_FM_";

/// O que o painel tira do log do StorageManagementService (linhas "Disk:" e STMVolumeCategory),
/// lido em fluxo: o texto do log não fica guardado.
#[derive(Default)]
pub struct LogDisco {
    /// As 5 últimas linhas "Disk:".
    ultimas: Vec<Value>,
    /// Última lista STMVolumeCategory não vazia.
    cats: Option<(String, Vec<Value>)>,
    log_sd: Option<(i64, String)>,
}

impl LogDisco {
    fn evento(&mut self, ts: &str, m: &str, ctx: &Ctx) {
        if let Some(r) = m.strip_prefix("Disk: ") {
            let mut o = Map::new();
            o.insert("timestamp".into(), json!(ts));
            for part in r.split(" / ") {
                if let Some((k, v)) = part.split_once(": ") {
                    o.insert(k.trim().to_string(), json!(v.trim().parse::<i64>().ok()));
                }
            }
            if self.ultimas.len() == 5 {
                self.ultimas.remove(0);
            }
            self.ultimas.push(Value::Object(o));
        } else if m.contains("STMVolumeCategory") {
            let mut l = Vec::new();
            for chunk in m.split("<STMVolumeCategory:").skip(1) {
                let body = chunk.split('>').next().unwrap_or("");
                let toks: Vec<&str> = body.split_whitespace().collect();
                if toks.len() >= 3 {
                    let bytes = toks[toks.len() - 1].parse::<i64>().ok();
                    let nome = toks[2..toks.len() - 1].join(" ");
                    // Pelo id: o nome é o rótulo na língua do Mac ("Dados do Sistema" em pt-BR).
                    if toks[1] == "DISK_OTHER"
                        && let Some(b) = bytes
                    {
                        self.log_sd = Some((b, ts.to_string()));
                    }
                    l.push(json!({"id": ctx.clean(toks[1]), "categoria": ctx.clean(&nome), "bytes": bytes}));
                }
            }
            if !l.is_empty() {
                self.cats = Some((ts.to_string(), l));
            }
        }
    }
}

/// Log do painel (7 dias, predicado por subsystem: ~12 s aqui), em fluxo. Roda em thread própria.
/// Segundo log: o último "System size set to" (timestamp, bytes).
pub fn ler_log_painel(ctx: Arc<Ctx>) -> (Lido<LogDisco>, Lido<Option<(String, i64)>>) {
    let a = sys::run_linhas(
        "/usr/bin/log",
        &["show", "--last", "7d", "--style", "ndjson", "--predicate",
          r#"subsystem == "com.apple.storage" AND (eventMessage BEGINSWITH "Disk: TOT:" OR eventMessage CONTAINS "STMVolumeCategory")"#],
        90,
        move |l, d: &mut LogDisco| {
            if let Some((ts, m)) = evento(l) {
                d.evento(&ts, &m, &ctx);
            }
        },
    );
    // "System size set to" é nível Debug: só aparece se ainda estiver no buffer.
    let b = sys::run_linhas(
        "/usr/bin/log",
        &["show", "--info", "--debug", "--last", "7d", "--style", "ndjson", "--predicate",
          r#"subsystem == "com.apple.settings.Storage" AND eventMessage BEGINSWITH "System size set to""#],
        60,
        |l, t: &mut Option<(String, i64)>| {
            if let Some((ts, m)) = evento(l)
                && let Some(n) = m.strip_prefix("System size set to ").and_then(|r| r.split_whitespace().next()).and_then(|x| x.parse().ok())
            {
                *t = Some((ts, n));
            }
        },
    );
    (a, b)
}

/// IDs dos assets UAF_FM de uma mensagem do log "AssetGC determine".
fn ids_do_evento(s: &str, ids: &mut HashSet<String>) {
    for piece in s.split(UAF_FM).skip(1) {
        // (GenerativeModels|Visual)/<purpose>/<40 hex>.asset
        if let Some(end) = piece.find(".asset") {
            let rel = &piece[..end];
            let parts: Vec<&str> = rel.split('/').collect();
            if parts.len() == 3
                && (parts[0] == "GenerativeModels" || parts[0] == "Visual")
                && parts[1].chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && parts[2].len() == 40
                && parts[2].chars().all(|c| c.is_ascii_hexdigit())
            {
                ids.insert(format!("{UAF_FM}{rel}.asset"));
            }
        }
    }
}

/// A consulta ao CacheDelete dispara o log "AssetGC determine" do mobileassetd (Frente H §3):
/// ler depois do purgeable. Em fluxo: só os IDs dos assets ficam.
pub fn ler_log_assets() -> Lido<HashSet<String>> {
    sys::run_linhas(
        "/usr/bin/log",
        &["show", "--last", "24h", "--style", "ndjson", "--predicate",
          r#"subsystem == "com.apple.mobileassetd" AND category == "CacheDelete" AND eventMessage CONTAINS "AssetGC determine" AND eventMessage CONTAINS "UAF_FM_""#],
        90,
        |l, ids: &mut HashSet<String>| {
            // ndjson escapa "/" como "\/": ler o eventMessage de cada linha.
            if let Some((_, m)) = evento(l) {
                ids_do_evento(&m, ids);
            }
        },
    )
}

pub struct Painel {
    pub json: Value,
    /// Último "System Data" do log do serviço: (bytes, timestamp); Ok(None) = log lido e sem
    /// registro; Err = o log não foi lido (falhou, timeout ou morto pelo Ctrl-C).
    pub log_dados_do_sistema: Result<Option<(i64, String)>, String>,
    pub apple_intelligence: Medida,
    pub apple_intelligence_fisico: Medida,
}

/// (timestamp, mensagem) de uma linha ndjson do `log show`; sem os dois campos, a linha some.
fn evento(l: &str) -> Option<(String, String)> {
    let v: Value = serde_json::from_str(l).ok()?;
    Some((v["timestamp"].as_str()?.to_string(), v["eventMessage"].as_str()?.to_string()))
}

fn eventos(r: &Log) -> Vec<(String, String)> {
    r.as_ref().map(|s| s.lines().filter_map(evento).collect()).unwrap_or_default()
}

fn dirstats_medida(r: &Result<u64, i32>) -> Medida {
    match r {
        // 0 não se distingue de "não mantido" (dirstats.rs): não vira 0 medido.
        Ok(0) => Medida::NaoMedida("dir-stats 0 (não mantido)".into()),
        Ok(v) => Medida::Valor(*v as i64),
        Err(e) if *e == libc::ENOENT => Medida::NaoMedida("pasta ausente: modelos não baixados".into()),
        Err(e) => Medida::NaoMedida(format!("dir-stats errno {e}")),
    }
}

pub fn coletar(
    (la, lb): (Lido<LogDisco>, Lido<Option<(String, i64)>>),
    la2: Lido<HashSet<String>>,
    macos: &Medida,
    macos_det: &[Value],
    caps: &Caps,
    err: &mut Vec<Value>,
) -> Painel {
    let log_sd = la.as_ref().map(|d| d.log_sd.clone()).map_err(|e| e.clone());
    let (ultimas, cats) = match la {
        Ok(d) => (d.ultimas, d.cats),
        Err(e) => {
            err.push(json!(e));
            (Vec::new(), None)
        }
    };
    let sys_size = lb.ok().flatten();

    // Apple Intelligence como o Ajustes: dir-stats(GenerativeModels) + dir-stats(Visual) + Σ dir-stats(<asset>/.AssetData)
    if let Err(e) = &la2 {
        err.push(json!(format!("log mobileassetd: {e}")));
    }
    let vazio = HashSet::new();
    let ids = la2.as_ref().unwrap_or(&vazio);
    // Antes do 15.1 nada é medido: None, e a linha sai "não disponível nesta versão".
    let pode = caps.apple_intelligence();
    let medir = |p: String| pode.then(|| sys::apfs::dirstats(&p));
    let (gm, vi) = (medir(format!("{UAF_FM}GenerativeModels")), medir(format!("{UAF_FM}Visual")));
    let (mut enx, mut montados, mut vazias, mut ausentes, mut falhas) = (0i64, 0u64, 0u64, 0u64, 0u64);
    if pode {
        for id in ids {
            match sys::apfs::dirstats(&format!("{id}/.AssetData")) {
                // 0 = não mantido (dirstats.rs): sem medida, como uma falha.
                Ok(0) => {
                    vazias += 1;
                    falhas += 1;
                }
                Ok(v) => {
                    montados += 1;
                    enx += v as i64;
                }
                // Asset do log que não está mais no disco: não ocupa nada.
                Err(libc::ENOENT) => ausentes += 1,
                Err(_) => falhas += 1,
            }
        }
    }
    // As duas pastas são obrigatórias: uma sem medida não vira 0 na soma.
    let fisico = match (gm.as_ref().map(dirstats_medida), vi.as_ref().map(dirstats_medida)) {
        (Some(Medida::Valor(a)), Some(Medida::Valor(b))) => Medida::Valor(a + b),
        (Some(Medida::Valor(_)), Some(m)) | (Some(m), _) => m,
        _ => Medida::NaoDisponivel,
    };
    // A terceira parcela (Σ .AssetData) vem dos IDs do log: sem o log, sem nenhum ID com modelos
    // no disco ou com .AssetData sem dir-stats, a soma sairia "medida" com GB a menos.
    let ai = match (&fisico, &la2) {
        (Medida::Valor(_), Err(e)) => Medida::NaoMedida(format!("log do mobileassetd: {e}")),
        (Medida::Valor(f), _) if ids.is_empty() && *f > 0 => {
            Medida::NaoMedida("nenhum asset no log 'AssetGC determine' das últimas 24 h".into())
        }
        (Medida::Valor(_), _) if falhas > 0 => Medida::NaoMedida(format!("{falhas} .AssetData sem dir-stats")),
        (Medida::Valor(f), _) => Medida::Valor(f + enx),
        (m, _) => m.clone(),
    };
    // Todas as pastas de modelos UAF (medido pelo APFS, Frente H §9).
    let mut uaf = BTreeMap::new();
    match std::fs::read_dir(ASSETS) {
        Ok(rd) => {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if n.starts_with("com_apple_MobileAsset_UAF_FM_") || n.starts_with("com_apple_MobileAsset_UAF_IF_Planner") {
                    uaf.insert(n.clone(), json!(sys::apfs::dirstats(&format!("{ASSETS}/{n}")).ok()));
                }
            }
        }
        Err(e) => err.push(json!(format!("read_dir {ASSETS}: {e}"))),
    }
    let ok = |r: &Option<Result<u64, i32>>| r.as_ref().and_then(|r| r.as_ref().ok()).copied();
    let erro = |r: &Option<Result<u64, i32>>| r.as_ref().and_then(|r| r.as_ref().err()).copied();
    let j = json!({
        "log": {
            "disk_ultimas": ultimas,
            "disk_fonte": "log show 7d, subsystem com.apple.storage, linhas 'Disk: TOT/USE/AVL/PRG/OTH/SYS' do StorageManagementService",
            "categorias_ultima": cats.as_ref().map(|c| json!({"timestamp": c.0, "categorias": c.1})),
            "categorias_fonte": "última lista STMVolumeCategory (StorageManagementService)",
            "system_size_set_to": sys_size.as_ref().map(|s| json!({"timestamp": s.0, "bytes": s.1})),
            "system_size_nota": "linha Debug da extensão com.apple.settings.Storage; em geral não fica gravada, só aparece se o Ajustes foi aberto há pouco",
            "vazio_significa": "Ajustes > Geral > Armazenamento não foi aberto há pouco (o macOS 27 retém essas linhas por menos de ~21 h)",
        },
        "macos_reproduzido": {
            "bytes": macos.valor(),
            "estado": macos.estado(),
            "volumes": macos_det,
            "fonte": "Σ CapacityInUse (diskutil apfs list -plist) de todos os volumes dos containers do disco interno, menos o Data (Frente M)",
        },
        "apple_intelligence_reproduzido": {
            "bytes": ai.valor(),
            "estado": ai.estado(),
            "dirstats_generativemodels": ok(&gm), "dirstats_visual": ok(&vi),
            "erro_generativemodels": erro(&gm), "erro_visual": erro(&vi),
            "assetdata_enxertos_bytes": enx, "assetdata_com_conteudo": montados, "assetdata_vazias": vazias, "assets_ausentes": ausentes, "assetdata_falhas": falhas,
            "assets_no_log": ids.len(),
            "ocupacao_fisica_bytes": fisico.valor(),
            "fonte": "dir-stats(UAF_FM_GenerativeModels) + dir-stats(UAF_FM_Visual) + Σ dir-stats(<asset>/.AssetData); IDs dos assets do log 'AssetGC determine' do mobileassetd",
            "nota": "os enxertos (.AssetData) repetem blocos das imagens .dmg já contadas; a ocupação física é só a soma das duas pastas. Está no volume Data, dentro de Dados do Sistema",
        },
        "modelos_uaf_dirstats": uaf,
    });
    Painel { json: j, log_dados_do_sistema: log_sd, apple_intelligence: ai, apple_intelligence_fisico: fisico }
}

// ---- a tela do Ajustes pelo próprio log (02/10, Mac do dono) ----

/// Com o painel aberto, a cada ~5 s: a linha da tela ("Sum of all categories ... Sum of
/// categories: X, available: Y, capacity: Z", X = "usado" do cabeçalho ao byte) e o bloco
/// `StorageLogInvestigation - <chave>: <bytes>` do StorageManagementService (uma linha por
/// categoria). Janela de 1 h: ~1,2 s, 180 MB; a de 45 s: ~0,12 s de CPU, 108 MB. 24 h com o
/// painel aberto passa de 30 s.
pub fn ler_log_tela(janela_s: u64) -> Log {
    sys::run("/usr/bin/log", &["show", "--last", &format!("{janela_s}s"), "--style", "ndjson", "--predicate", PREDICADO_TELA], 60)
}

/// As duas fontes da tela: a linha do painel e o bloco do serviço (o `log show` e o `log stream`).
pub const PREDICADO_TELA: &str = r#"(subsystem == "com.apple.settings.Storage" AND category == "StorageUI" AND eventMessage BEGINSWITH "Sum of all categories") OR (subsystem == "com.apple.storage" AND eventMessage BEGINSWITH "StorageLogInvestigation - " AND NOT eventMessage CONTAINS "sendSidebarItems")"#;

/// Eventos guardados pelo `--watch` para juntar o bloco do serviço (~20 linhas, sai ~2 s antes)
/// à linha da tela: 400 cobrem vários ciclos de ~5 s do painel e limitam a memória.
const GUARDADOS: usize = 400;

/// Usado, disponível e capacidade da tela, e o bloco do serviço.
type Numeros = (Option<(i64, i64, i64)>, Option<BTreeMap<String, i64>>);

/// `--watch`: o que fica entre uma linha e outra do `log stream`.
#[derive(Default)]
pub struct Vigia {
    ev: VecDeque<(String, String)>,
    /// Números da última tela devolvida: o painel repete a mesma linha a cada ~5 s.
    ultima: Option<Numeros>,
}

impl Vigia {
    /// Uma linha ndjson do stream. Devolve a tela quando a linha é a da tela e os números mudaram
    /// desde a última devolvida.
    pub fn linha(&mut self, l: &str) -> Option<TelaLog> {
        let (ts, m) = evento(l)?;
        let da_tela = m.starts_with("Sum of all categories");
        if self.ev.len() == GUARDADOS {
            self.ev.pop_front();
        }
        self.ev.push_back((ts, m));
        if !da_tela {
            return None;
        }
        let t = tela_do_log(self.ev.make_contiguous());
        let numeros: Numeros = (t.tela.as_ref().map(|x| (x.1, x.2, x.3)), t.servico.as_ref().map(|s| s.1.clone()));
        if self.ultima.as_ref() == Some(&numeros) {
            return None;
        }
        self.ultima = Some(numeros);
        Some(t)
    }
}

/// Janela inteira do `--quick` sem `--since`: a do painel aberto e fechado há minutos.
pub const JANELA_CHEIA_S: u64 = 3600;
/// Folga antes do carimbo da leitura anterior: o bloco do serviço sai ~2 s antes da linha da tela e
/// as duas têm de cair na mesma janela, senão a tela vira `partial`.
const SOBREPOSICAO_S: i64 = 10;

/// Segundos de log a ler. Sem `desde`, ou com `desde` no futuro ou há mais de 1 h: a janela cheia.
/// Senão, do carimbo da leitura anterior menos a sobreposição até agora.
pub fn janela_log(agora: i64, desde: Option<i64>) -> u64 {
    match desde {
        Some(d) if d <= agora && agora - d + SOBREPOSICAO_S < JANELA_CHEIA_S as i64 => (agora - d + SOBREPOSICAO_S) as u64,
        _ => JANELA_CHEIA_S,
    }
}

/// Chaves do bloco do serviço que não são categoria.
const TOTAIS: [&str; 7] = ["totalSpace", "availSpace", "purgeSpace", "otherVols", "Used", "System", "Other"];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TelaLog {
    /// (timestamp, usado X, disponível Y, capacidade Z) da última linha da tela.
    pub tela: Option<(String, i64, i64, i64)>,
    /// (timestamp, chave → bytes) do último bloco completo do serviço (tem Used, System e Other)
    /// não posterior à linha da tela; sem linha da tela, o último bloco.
    pub servico: Option<(String, BTreeMap<String, i64>)>,
}

impl TelaLog {
    /// Σ das categorias das extensões (tudo menos macOS e Dados do Sistema) = Used − System − Other.
    pub fn soma_extensoes(&self) -> Option<i64> {
        let m = &self.servico.as_ref()?.1;
        Some(m.get("Used")? - m.get("System")? - m.get("Other")?)
    }
    /// Dados do Sistema como a tela: X − Σ extensões − macOS da tela.
    pub fn dados_do_sistema(&self, macos: Option<i64>) -> Option<i64> {
        Some(self.tela.as_ref()?.1 - self.soma_extensoes()? - macos?)
    }
    pub fn categorias(&self) -> Vec<(String, i64)> {
        self.servico.as_ref().map_or_else(Vec::new, |(_, m)| {
            let mut v: Vec<_> = m.iter().filter(|(k, _)| !TOTAIS.contains(&k.as_str())).map(|(k, b)| (k.clone(), *b)).collect();
            v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            v
        })
    }
}

fn numero_apos(m: &str, rotulo: &str) -> Option<i64> {
    let r = &m[m.find(rotulo)? + rotulo.len()..];
    r.trim_start().split(|c: char| !c.is_ascii_digit() && c != '-').next()?.parse().ok()
}

/// Puro: eventos (timestamp, mensagem) em ordem cronológica.
pub(crate) fn tela_do_log(ev: &[(String, String)]) -> TelaLog {
    let mut tela = None;
    let mut blocos: Vec<(String, BTreeMap<String, i64>)> = Vec::new();
    for (ts, m) in ev {
        if m.starts_with("Sum of all categories") {
            if let (Some(x), Some(y), Some(z)) = (numero_apos(m, "Sum of categories:"), numero_apos(m, "available:"), numero_apos(m, "capacity:")) {
                tela = Some((ts.clone(), x, y, z));
            }
        } else if let Some(r) = m.strip_prefix("StorageLogInvestigation - ")
            && let Some((k, v)) = r.rsplit_once(": ")
            && let Ok(b) = v.trim().parse::<i64>()
        {
            // "totalSpace" abre o bloco (as linhas variam no milissegundo); o bloco fica com a hora dele.
            match blocos.last_mut() {
                Some((_, mapa)) if k != "totalSpace" => {
                    mapa.insert(k.to_string(), b);
                }
                _ if k == "totalSpace" => blocos.push((ts.clone(), BTreeMap::from([(k.to_string(), b)]))),
                _ => {}
            }
        }
    }
    let completo = |m: &BTreeMap<String, i64>| ["Used", "System", "Other"].iter().all(|k| m.contains_key(*k));
    // Timestamps ISO do mesmo fuso comparam como texto.
    let servico = blocos
        .iter()
        .rev()
        .filter(|(_, m)| completo(m))
        .find(|(t, _)| tela.as_ref().is_none_or(|(tt, ..)| t <= tt))
        .or_else(|| blocos.iter().rev().find(|(_, m)| completo(m)))
        .cloned();
    TelaLog { tela, servico }
}

pub fn coletar_tela(log: &Log, err: &mut Vec<Value>) -> TelaLog {
    if let Err(e) = log {
        err.push(json!(format!("log da tela: {e}")));
    }
    tela_do_log(&eventos(log))
}

#[cfg(test)]
mod testes_fluxo {
    use super::*;

    #[test]
    fn linhas_em_fluxo_sem_guardar_a_saida() {
        let r = sys::run_linhas("/usr/bin/printf", &["a\nb\r\n\\377\nc"], 5, |l, v: &mut Vec<String>| v.push(l.to_string()));
        assert_eq!(r, Ok(vec!["a".into(), "b".into(), "\u{fffd}".into(), "c".into()]));
        // Status diferente de 0: erro, não resumo parcial.
        assert!(sys::run_linhas("/usr/bin/false", &[], 5, |_, _: &mut u8| ()).is_err());
    }

    #[test]
    fn log_do_disco_fica_com_as_cinco_ultimas_e_a_ultima_lista() {
        let ctx = Ctx::real("x");
        let mut d = LogDisco::default();
        for i in 0..7 {
            d.evento(&format!("t{i}"), &format!("Disk: TOT: {i} / USE: 5 / AVL: x"), &ctx);
        }
        d.evento("c1", "listas <STMVolumeCategory: 0x1 DISK_OTHER System Data 111> <STMVolumeCategory: 0x2 id.b Apps 222>", &ctx);
        // Mac em pt-BR: o rótulo vem traduzido, o id não.
        d.evento("c2", "<STMVolumeCategory: 0x1 DISK_OTHER Dados do Sistema 333>", &ctx);
        d.evento("c3", "sem lista nenhuma", &ctx);
        let tots: Vec<_> = d.ultimas.iter().map(|v| (v["timestamp"].as_str().unwrap_or(""), v["TOT"].as_i64(), v["AVL"].as_i64())).collect();
        assert_eq!(tots, [("t2", Some(2), None), ("t3", Some(3), None), ("t4", Some(4), None), ("t5", Some(5), None), ("t6", Some(6), None)]);
        assert_eq!(d.log_sd, Some((333, "c2".into())));
        let (ts, l) = d.cats.expect("lista");
        assert_eq!((ts.as_str(), l.len(), l[0]["categoria"].as_str(), l[0]["bytes"].as_i64()), ("c2", 1, Some("Dados do Sistema"), Some(333)));
    }

    #[test]
    fn ids_dos_assets_do_log() {
        let id = "a".repeat(40);
        let m = format!("x {UAF_FM}GenerativeModels/com_apple_x/{id}.asset y {UAF_FM}Visual/ok_1/{id}.asset z {UAF_FM}Outro/ok/{id}.asset {UAF_FM}Visual/ok/curto.asset");
        let mut ids = HashSet::new();
        ids_do_evento(&m, &mut ids);
        let mut v: Vec<_> = ids.into_iter().collect();
        v.sort();
        assert_eq!(v, [format!("{UAF_FM}GenerativeModels/com_apple_x/{id}.asset"), format!("{UAF_FM}Visual/ok_1/{id}.asset")]);
        assert_eq!(evento(r#"{"timestamp":"t","eventMessage":"m","x":1}"#), Some(("t".into(), "m".into())));
        assert_eq!(evento(r#"{"timestamp":"t"}"#), None);
        assert_eq!(evento("nao json"), None);
    }
}

#[cfg(test)]
mod testes_tela {
    use super::*;

    #[test]
    fn janela_do_log_incremental() {
        // Sem carimbo, no futuro ou há mais de 1 h: a janela cheia.
        assert_eq!(janela_log(10_000, None), JANELA_CHEIA_S);
        assert_eq!(janela_log(10_000, Some(10_001)), JANELA_CHEIA_S);
        assert_eq!(janela_log(10_000, Some(10_000 - 3600)), JANELA_CHEIA_S);
        assert_eq!(janela_log(10_000, Some(10_000 - 3590)), JANELA_CHEIA_S);
        // Leitura de 30 s atrás: 30 s + a sobreposição.
        assert_eq!(janela_log(10_000, Some(9_970)), 40);
        assert_eq!(janela_log(10_000, Some(10_000)), 10);
        // O bloco do serviço sai ~2 s antes da linha da tela. Uma leitura em t=1001 vê o bloco
        // (t=1000) sem a linha (t=1002); a leitura seguinte, em t=1031, tem de recuar até ele.
        let comeco = 1031 - janela_log(1031, Some(1001)) as i64;
        assert!(comeco <= 1000);
    }

    #[test]
    fn leitura_incremental_junta_bloco_e_linha_da_leitura_anterior() {
        let bloco = [
            ("2026-10-02 00:55:46.000-0300", "StorageLogInvestigation - totalSpace: 10"),
            ("2026-10-02 00:55:46.000-0300", "StorageLogInvestigation - Used: 8"),
            ("2026-10-02 00:55:46.000-0300", "StorageLogInvestigation - System: 1"),
            ("2026-10-02 00:55:46.001-0300", "StorageLogInvestigation - Other: 2"),
        ];
        let linha = ("2026-10-02 00:55:48.482-0300", "Sum of all categories and available space doe not match capacity of the volume. Sum of categories: 8, available: 2, capacity: 10. Difference is 0");
        // Leitura 1 pegou só o bloco: sem linha da tela, ainda não é tela.
        let a = tela_do_log(&ev(&bloco));
        assert!(a.tela.is_none() && a.servico.is_some());
        // Leitura 2 (janela recuada pela sobreposição) traz o bloco e a linha: tela completa.
        let mut tudo = bloco.to_vec();
        tudo.push(linha);
        let b = tela_do_log(&ev(&tudo));
        assert_eq!(b.tela.as_ref().map(|x| (x.1, x.2, x.3)), Some((8, 2, 10)));
        assert_eq!(b.dados_do_sistema(Some(1)), Some(8 - 5 - 1));
    }

    fn ev(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn tela_e_bloco_do_servico() {
        let e = ev(&[
            ("2026-10-02 00:55:46.000-0300", "StorageLogInvestigation - totalSpace: 494384795648"),
            ("2026-10-02 00:55:46.000-0300", "StorageLogInvestigation - Used: 383977040420"),
            ("2026-10-02 00:55:46.000-0300", "StorageLogInvestigation - System: 23023337472"),
            ("2026-10-02 00:55:46.000-0300", "StorageLogInvestigation - com.apple.STMExtension.Applications: 81507991552"),
            ("2026-10-02 00:55:46.000-0300", "StorageLogInvestigation - com.apple.STMExtension.Developer: 2050097152"),
            ("2026-10-02 00:55:46.001-0300", "StorageLogInvestigation - Other: 168115181382"),
            ("2026-10-02 00:55:48.482-0300", "Sum of all categories and available space doe not match capacity of the volume, the proportions of the gauge will be adjusted. Sum of categories: 403752650702, available: 90622482482, capacity: 494384795648. Difference is 9662464"),
            // Bloco depois da linha da tela: não é o que ela usou.
            ("2026-10-02 00:55:51.323-0300", "StorageLogInvestigation - totalSpace: 1"),
            ("2026-10-02 00:55:51.323-0300", "StorageLogInvestigation - Used: 2"),
            ("2026-10-02 00:55:51.323-0300", "StorageLogInvestigation - System: 1"),
            ("2026-10-02 00:55:51.323-0300", "StorageLogInvestigation - Other: 1"),
        ]);
        let t = tela_do_log(&e);
        assert_eq!(t.tela.as_ref().map(|x| (x.1, x.2, x.3)), Some((403752650702, 90622482482, 494384795648)));
        assert_eq!(t.servico.as_ref().map(|s| s.0.as_str()), Some("2026-10-02 00:55:46.000-0300"));
        assert_eq!(t.soma_extensoes(), Some(383977040420 - 23023337472 - 168115181382));
        assert_eq!(t.dados_do_sistema(Some(38749663232)), Some(403752650702 - 192838521566 - 38749663232));
        assert_eq!(t.categorias().first().map(|c| c.0.as_str()), Some("com.apple.STMExtension.Applications"));
        assert_eq!(t.categorias().len(), 2);
    }

    /// O stream: a tela sai na linha dela, com o bloco que veio antes, e só quando os números mudam.
    #[test]
    fn vigia_do_stream() {
        let l = |ts: &str, m: &str| json!({"timestamp": ts, "eventMessage": m}).to_string();
        let bloco = |ts: &str, used: i64| {
            [("totalSpace", 10), ("Used", used), ("System", 1), ("Other", 2)].map(|(k, b)| l(ts, &format!("StorageLogInvestigation - {k}: {b}")))
        };
        let tela = |ts: &str, x: i64| l(ts, &format!("Sum of all categories and available space doe not match. Sum of categories: {x}, available: 2, capacity: 10. Difference is 0"));
        let mut v = Vigia::default();
        assert!(bloco("t1", 8).iter().all(|x| v.linha(x).is_none()));
        assert!(v.linha("Filtering the log data using ...").is_none());
        let t = v.linha(&tela("t2", 8)).expect("tela");
        assert_eq!((t.tela.as_ref().map(|x| x.1), t.soma_extensoes()), (Some(8), Some(5)));
        // Mesmos números ~5 s depois: não devolve de novo.
        bloco("t3", 8).iter().for_each(|x| assert!(v.linha(x).is_none()));
        assert!(v.linha(&tela("t4", 8)).is_none());
        // O serviço recalculou: devolve.
        bloco("t5", 9).iter().for_each(|x| assert!(v.linha(x).is_none()));
        assert_eq!(v.linha(&tela("t6", 9)).and_then(|t| t.tela.map(|x| x.1)), Some(9));
        // A memória fica limitada.
        (0..1000).for_each(|i| assert!(v.linha(&l("t7", &format!("StorageLogInvestigation - x{i}: 1"))).is_none()));
        assert_eq!(v.ev.len(), GUARDADOS);
    }

    #[test]
    fn sem_linha_da_tela() {
        let e = ev(&[
            ("t1", "StorageLogInvestigation - totalSpace: 10"),
            ("t1", "StorageLogInvestigation - Used: 5"),
            ("t1", "StorageLogInvestigation - System: 1"),
        ]);
        let t = tela_do_log(&e);
        assert!(t.tela.is_none());
        // Bloco sem "Other" não é completo.
        assert!(t.servico.is_none());
        assert_eq!(t.dados_do_sistema(Some(1)), None);
    }
}

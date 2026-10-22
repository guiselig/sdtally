//! A coleta inteira, usada pelo sdc-coleta e pelo sdtally: volumes, volume Data, snapshots,
//! purgeable, apagados-abertos, travessia, dir-stats das negadas e painel. Só leitura.
//! Cada etapa tem tempo medido; panic vira erro registrado. Base: síntese §1c, §2, §4.

use crate::anon::{self, Ctx};
use crate::caps::Caps;
use crate::ledger::{Medida, Termo};
use crate::report::Chave;
use crate::sources::{self, DATA, LIMITE_BALDE};
use crate::sources::{apfs_meta::ApfsMeta, dirstats::Negadas, lixo::Lixo, painel::Painel, painel::TelaLog, purgeable::Purgeable, snapshots::Snapshots};
use crate::{sys, walk};
use serde_json::{Map, Value, json};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct Opcoes {
    pub threads: Option<usize>,
    pub xattr: bool,
    pub root: bool,
}

pub struct Coleta {
    pub caps: Caps,
    pub tempos: Map<String, Value>,
    pub erros: Vec<Value>,
    /// true/false, ou texto "indeterminado (errno)".
    pub fda: Value,
    pub perf: Option<u64>,
    /// Ligado desde, "AAAA-MM-DD HH:MM" local.
    pub boot_time: Option<String>,
    pub threads: usize,
    pub xattr: bool,
    pub root: bool,
    pub volumes: Value,
    /// macOS como o Ajustes calcula (Σ CapacityInUse − Data).
    pub macos: Medida,
    pub macos_det: Vec<Value>,
    pub disco_tamanho: Value,
    /// (total, usado) do swap (vm.swapusage).
    pub swap: Option<(u64, u64)>,
    pub usado: Medida,
    pub data_json: Value,
    pub snaps: Option<Snapshots>,
    pub snap_total: Medida,
    pub purg: Option<Purgeable>,
    pub purg_total: Medida,
    pub lsof: Value,
    pub apagados: Medida,
    pub arvore: Option<walk::Result>,
    pub negadas: Option<Negadas>,
    pub negadas_total: Medida,
    pub painel: Option<Painel>,
    /// A tela do Ajustes pelo próprio log (última hora).
    pub tela_log: Option<TelaLog>,
    /// `~/.Trash` por idade (Err = motivo de não medido); None se a etapa não rodou.
    pub lixo: Option<Result<Lixo, String>>,
    /// Metadados das árvores vivas do APFS no Data; None se a etapa não rodou.
    pub apfs_meta: Option<ApfsMeta>,
    /// Ctrl-C: etapas depois dele não rodaram e a travessia é um piso.
    pub interrompida: bool,
}

/// Ordem em que as etapas aparecem em `stage_errors`, qualquer que seja a ordem em que terminem.
const ORDEM: [&str; 14] =
    ["meta", "volumes", "swap", "volume_data", "capacidade", "snapshots", "purgeable", "apagados_abertos", "lixo", "apfs_metadata", "arvore", "dirstats_negadas", "painel", "tela_log"];

/// Compartilhada entre as threads das etapas independentes (`coletar`): por isso os Mutex.
struct Etapas {
    tempos: Mutex<Map<String, Value>>,
    erros: Mutex<Vec<Value>>,
    ctl: Arc<walk::Controle>,
}

impl Etapas {
    fn new(ctl: Arc<walk::Controle>) -> Etapas {
        Etapas { tempos: Mutex::new(Map::new()), erros: Mutex::new(Vec::new()), ctl }
    }

    /// (tempos, erros) no fim da coleta; erros na ordem fixa das etapas (sort estável: dentro de
    /// uma etapa fica a ordem em que ela os registrou).
    fn fim(self) -> (Map<String, Value>, Vec<Value>) {
        let mut erros = self.erros.into_inner().unwrap_or_else(|e| e.into_inner());
        erros.sort_by_key(|e| ORDEM.iter().position(|n| e["etapa"] == *n).unwrap_or(ORDEM.len()));
        (self.tempos.into_inner().unwrap_or_else(|e| e.into_inner()), erros)
    }

    /// Roda a etapa com tempo medido; panic vira erro registrado, nunca derruba o coletor.
    /// Depois de um Ctrl-C a etapa não roda (None). Ctrl-C no meio também descarta: o sinal mata
    /// os filhos (lsof, log) e a saída parcial viraria número cheio. A árvore guarda o próprio
    /// corte (piso).
    fn run<T>(&self, nome: &str, f: impl FnOnce(&mut Vec<Value>) -> T) -> Option<T> {
        if self.ctl.cancelado() {
            return None;
        }
        let t = Instant::now();
        let mut erros = Vec::new();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut erros)));
        let tempo = json!(round2(t.elapsed().as_secs_f64()));
        self.tempos.lock().unwrap_or_else(|e| e.into_inner()).insert(nome.into(), tempo);
        let mut todos = self.erros.lock().unwrap_or_else(|e| e.into_inner());
        todos.extend(erros.into_iter().map(|e| json!({"etapa": nome, "erro": e})));
        if self.ctl.cancelado() && nome != "arvore" {
            return None;
        }
        match r {
            Ok(v) => Some(v),
            Err(_) => {
                todos.push(json!({"etapa": nome, "erro": "falha interna (panic capturado)"}));
                None
            }
        }
    }

    /// Motivo de uma etapa sem resultado.
    fn falhou(&self) -> Medida {
        Medida::NaoMedida(if self.ctl.cancelado() { "interrompido".into() } else { "falha interna (panic capturado)".into() })
    }
}

/// `f` numa thread de prioridade baixa (os logs, que o `log show` demora).
fn em_fundo<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> std::thread::JoinHandle<T> {
    std::thread::spawn(move || {
        sys::qos_utility();
        f()
    })
}

/// Threads da travessia: o pedido, ou os núcleos de desempenho (1 a 16).
pub fn threads(pedido: Option<usize>) -> usize {
    pedido.or(sys::sysctl_u64("hw.perflevel0.physicalcpu").map(|x| x as usize)).unwrap_or(4).clamp(1, 16)
}

pub fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// Nome que o balde (`anon::Ctx::real`) compara com a pasta em /Users: o da pasta da home, que
/// pode diferir do nome curto da conta (renomeada, conta móvel); fora de /Users, o usuário.
pub fn nome_da_home<'a>(user: &'a str, home: &'a str) -> &'a str {
    home.strip_prefix("/Users/").filter(|n| !n.is_empty() && !n.contains('/')).unwrap_or(user)
}

/// (usuário, home) da pessoa: com sudo é SUDO_USER, não o root.
pub fn identidade(root: bool) -> (String, String) {
    match std::env::var("SUDO_USER").ok().filter(|u| root && !u.is_empty() && u != "root") {
        Some(u) => {
            let home = sys::home_of(&u).unwrap_or_else(|| format!("/Users/{u}"));
            (u, home)
        }
        None => {
            let home = std::env::var("HOME").unwrap_or_default();
            (std::env::var("USER").unwrap_or_else(|_| home.rsplit('/').next().unwrap_or("").to_string()), home)
        }
    }
}

/// Baldes da árvore dobrados pelo limite, do maior para o menor.
pub fn baldes(r: &walk::Result) -> Vec<(String, walk::Sum)> {
    let b = anon::fold(r.buckets.clone(), LIMITE_BALDE, |s| s.alloc.max(s.dataless_logical), |x, y| x.add(y));
    let mut v: Vec<_> = b.into_iter().collect();
    v.sort_by_key(|x| -x.1.alloc);
    v
}

pub fn coletar(o: &Opcoes, home: &str, ctx: Arc<Ctx>, ctl: Arc<walk::Controle>) -> Coleta {
    let threads = threads(o.threads);
    let (c, t) = (ctx.clone(), ctl.clone());
    coletar_com(o, home, ctx, ctl, move || walk::walk_ctl(DATA, threads, c, t))
}

/// A coleta completa com a árvore vinda de `arvore` (a travessia, ou a árvore viva do `--auto`).
pub(crate) fn coletar_com(o: &Opcoes, home: &str, ctx: Arc<Ctx>, ctl: Arc<walk::Controle>, arvore: impl FnOnce() -> Result<walk::Result, String>) -> Coleta {
    let et = Etapas::new(ctl.clone());
    // Versão e capacidades lidas uma vez; cada fonte consulta daqui.
    let caps = Caps::ler(DATA);
    walk::MEASURE_XATTR.store(o.xattr, Ordering::Relaxed);

    // ---- logs em paralelo (não disparam se já veio cancelada) ----
    let log_painel = (!ctl.cancelado()).then(|| {
        let ctx = ctx.clone();
        em_fundo(move || sources::painel::ler_log_painel(ctx))
    });
    let log_tela = (!ctl.cancelado()).then(|| em_fundo(|| sources::painel::ler_log_tela(sources::painel::JANELA_CHEIA_S)));
    let log_assets = (!ctl.cancelado()).then(|| em_fundo(sources::painel::ler_log_assets));

    // ---- meta ----
    let fda = et.run("meta", |_| fda_de(home)).unwrap_or(Value::Null);
    let perf = sys::sysctl_u64("hw.perflevel0.physicalcpu");
    let boot_time = sys::boot_time();
    let threads = threads(o.threads);

    // ---- a travessia (árvore + dir-stats das negadas) roda junto das etapas que não dependem dela
    // (volumes, snapshots, purgeable, apagados): estas somam ~10 a 15 s, quase todo subprocesso e
    // fsctl; a travessia ocupa os núcleos, mas não a espera deles. Cada etapa guarda o resultado
    // e o tempo no `et`; o que sai do escopo é o mesmo que a execução em fila daria.
    let (vols, swap, volume_data, snaps, purg, apag, lixo, apfs_meta, arvore, negadas) = std::thread::scope(|sc| {
        let base = sc.spawn(|| {
            let vols = et.run("volumes", sources::volumes::coletar);
            let swap = et.run("swap", swap).flatten();
            let data = et.run("volume_data", sources::volumes::volume_data);
            let meta = et.run("apfs_metadata", |_| sources::apfs_meta::coletar());
            (vols, swap, data, meta)
        });
        let snaps = sc.spawn(|| et.run("snapshots", sources::snapshots::coletar));
        let outras = sc.spawn(|| {
            let purg = et.run("purgeable", |err| sources::purgeable::coletar(&ctx, err));
            let apag = et.run("apagados_abertos", |err| sources::apagados::coletar(&ctx, o.root, err));
            // O motivo de não medido fica no próprio resultado: informativo, não derruba a coleta (INCOMPLETO).
            let lixo = et.run("lixo", |_| sources::lixo::medir(Path::new(&format!("{home}/.Trash")), sys::agora_epoch(), 2, ctl.clone()));
            (purg, apag, lixo)
        });
        // ---- árvore ----
        let arvore = et
            .run("arvore", |err| match arvore() {
                Ok(r) => Some(r),
                Err(e) => {
                    err.push(json!(e));
                    None
                }
            })
            .flatten();
        // ---- dir-stats das pastas negadas ----
        let negadas = et.run("dirstats_negadas", |_| arvore.as_ref().map(|r| sources::dirstats::negadas(&r.denied, &ctx))).flatten();
        let (vols, swap, data, meta) = base.join().unwrap_or((None, None, None, None));
        let (purg, apag, lixo) = outras.join().unwrap_or((None, None, None));
        (vols, swap, data, snaps.join().unwrap_or(None), purg, apag, lixo, meta, arvore, negadas)
    });

    let (volumes, macos, macos_det, disco_tamanho) = match vols {
        Some(v) => (v.json, v.macos, v.macos_det, v.disco_tamanho),
        None => (Value::Null, et.falhou(), Vec::new(), Value::Null),
    };
    let (usado, data_json) = volume_data.unwrap_or_else(|| (et.falhou(), Value::Null));
    let snap_total = snaps.as_ref().map_or_else(|| et.falhou(), |s| s.total.clone());
    let purg_total = purg.as_ref().map_or_else(|| et.falhou(), |p| p.total.clone());
    let (lsof, apagados) = apag.unwrap_or_else(|| (Value::Null, et.falhou()));
    let negadas_total = match &negadas {
        Some(n) => n.total.clone(),
        None if ctl.cancelado() => Medida::NaoMedida("interrompido".into()),
        None => Medida::NaoMedida("sem travessia".into()),
    };

    // ---- painel: log + fórmulas reproduzidas ----
    let painel = et.run("painel", |err| {
        let la = log_painel.and_then(|h| h.join().ok()).unwrap_or((Err("thread".into()), Err("thread".into())));
        let la2 = log_assets.and_then(|h| h.join().ok()).unwrap_or(Err("thread".into()));
        sources::painel::coletar(la, la2, &macos, &macos_det, &caps, err)
    });

    let tela_log = et.run("tela_log", |err| {
        let l = log_tela.and_then(|h| h.join().ok()).unwrap_or(Err("thread".into()));
        sources::painel::coletar_tela(&l, err)
    });

    let (tempos, erros) = et.fim();
    Coleta {
        caps,
        tempos,
        erros,
        fda,
        perf,
        boot_time,
        threads,
        xattr: o.xattr,
        root: o.root,
        volumes,
        macos,
        macos_det,
        disco_tamanho,
        swap,
        usado,
        data_json,
        snaps,
        snap_total,
        purg,
        purg_total,
        lsof,
        apagados,
        arvore,
        negadas,
        negadas_total,
        painel,
        tela_log,
        lixo,
        apfs_meta,
        interrompida: ctl.cancelado(),
    }
}

/// Modo rápido (`sdtally --quick`): o que muda a toda hora e custa pouco, para o app ler a cada
/// minuto. Sem travessia, snapshots, lsof nem dir-stats (mapa-de-fontes §4).
/// Partes da leitura rápida (`--parts`): cada uma com o seu ritmo no app. Sem `--parts`, todas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Partes {
    /// Acesso Total ao Disco (`full_disk_access`).
    pub meta: bool,
    /// Volumes do container e o macOS da tela (`containers`, `settings_screen.macos`): ~0,19 s.
    pub volumes: bool,
    pub swap: bool,
    /// Usado do volume Data e statfs (`data_volume`, `capacity`).
    pub uso: bool,
    pub purgeable: bool,
    /// Linhas do Ajustes no log (`settings_screen.screen`); com o painel aberto, lê os volumes também.
    pub tela: bool,
}

impl Partes {
    pub const TODAS: Partes = Partes { meta: true, volumes: true, swap: true, uso: true, purgeable: true, tela: true };
    pub const NOMES: [&'static str; 6] = ["meta", "volumes", "swap", "uso", "purgeable", "tela"];

    /// "volumes,tela" → Partes; nome desconhecido ou lista vazia → None.
    pub fn de(lista: &str) -> Option<Partes> {
        let mut p = Partes { meta: false, volumes: false, swap: false, uso: false, purgeable: false, tela: false };
        for n in lista.split(',') {
            match n.trim() {
                "meta" => p.meta = true,
                "volumes" => p.volumes = true,
                "swap" => p.swap = true,
                "uso" => p.uso = true,
                "purgeable" => p.purgeable = true,
                "tela" => p.tela = true,
                _ => return None,
            }
        }
        Some(p)
    }
}

pub struct Rapido {
    pub partes: Partes,
    pub caps: Caps,
    pub tempos: Map<String, Value>,
    pub erros: Vec<Value>,
    pub fda: Value,
    pub root: bool,
    pub boot_time: Option<String>,
    pub macos: Medida,
    /// JSON bruto dos volumes (`sources::volumes`), vira `containers` no relatório.
    pub volumes: Value,
    pub swap: Option<(u64, u64)>,
    pub usado: Medida,
    /// (total, livre para não root) do volume Data, pelo statfs.
    pub capacidade: Option<(u64, u64)>,
    pub purg: Option<Purgeable>,
    pub purg_total: Medida,
    pub tela_log: Option<TelaLog>,
    /// `--since` com o painel do Ajustes fechado: o log não foi lido de propósito.
    pub tela_pulada: bool,
    pub interrompida: bool,
}

/// `desde`: segundos desde a época da leitura anterior (`--since`). Com ele a leitura é incremental:
/// o log da tela só é lido da leitura anterior em diante, e só se o painel do Ajustes estiver
/// aberto agora (fechado, o log não ganha linha nova; `tela_log` fica `None`, "não lido").
pub fn coletar_rapido(root: bool, home: &str, ctx: Arc<Ctx>, ctl: Arc<walk::Controle>, desde: Option<i64>, partes: Partes) -> Rapido {
    let et = Etapas::new(ctl.clone());
    let caps = Caps::ler(DATA);
    let pular_tela = !partes.tela || (desde.is_some() && !sys::painel_do_ajustes_aberto());
    // A tela precisa do macOS dos volumes (tela_json); com ela pulada, os volumes só se pedidos.
    let ler_volumes = partes.volumes || !pular_tela;
    let partes = Partes { volumes: ler_volumes, ..partes };
    let janela = sources::painel::janela_log(sys::agora_epoch(), desde);
    let log_tela = (!ctl.cancelado() && !pular_tela).then(|| std::thread::spawn(move || sources::painel::ler_log_tela(janela)));
    let nao_lida = || Medida::NaoMedida("not read (--parts)".into());
    let fda = if partes.meta { et.run("meta", |_| fda_de(home)).unwrap_or(Value::Null) } else { Value::Null };
    let (macos, volumes) =
        if ler_volumes { et.run("volumes", sources::volumes::coletar).map_or_else(|| (et.falhou(), Value::Null), |v| (v.macos, v.json)) } else { (nao_lida(), Value::Null) };
    let swap = if partes.swap { et.run("swap", swap).flatten() } else { None };
    let usado = if partes.uso { et.run("volume_data", sources::volumes::volume_data).map_or_else(|| et.falhou(), |v| v.0) } else { nao_lida() };
    let capacidade =
        if partes.uso { et.run("capacidade", |err| sys::statfs(DATA).map_err(|e| err.push(json!(format!("statfs errno {e}")))).ok()).flatten() } else { None };
    let purg = if partes.purgeable { et.run("purgeable", |err| sources::purgeable::coletar(&ctx, err)) } else { None };
    let purg_total = if partes.purgeable { purg.as_ref().map_or_else(|| et.falhou(), |p| p.total.clone()) } else { nao_lida() };
    let tela_log = if pular_tela {
        None
    } else {
        et.run("tela_log", |err| {
            let l = log_tela.and_then(|h| h.join().ok()).unwrap_or(Err("thread".into()));
            sources::painel::coletar_tela(&l, err)
        })
    };
    let (tempos, erros) = et.fim();
    Rapido { partes, caps, tempos, erros, fda, root, boot_time: sys::boot_time(), macos, volumes, swap, usado, capacidade, purg, purg_total, tela_log, tela_pulada: pular_tela, interrompida: ctl.cancelado() }
}

/// `--watch`: escuta o log da tela pelo `log stream` (parado, ~0 de CPU) e chama `emitir` com a
/// leitura rápida só da tela e do macOS a cada tela nova; sem `log show`. Os volumes (o macOS da
/// tela, ~0,1 s) são relidos no máximo a cada minuto: com o painel aberto a linha sai a cada ~5 s.
/// Volta Ok quando o stdin fecha (o app que o abriu saiu ou o fechou); Err se o stream acabar.
pub fn vigiar_tela(root: bool, ctl: Arc<walk::Controle>, mut emitir: impl FnMut(&Rapido)) -> Result<(), String> {
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    let mut ch = Command::new("/usr/bin/log")
        .args(["stream", "--style", "ndjson", "--predicate", sources::painel::PREDICADO_TELA])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("/usr/bin/log: {e}"))?;
    let _filho = sys::filho(ch.id());
    let out = ch.stdout.take().ok_or("/usr/bin/log: sem stdout")?;
    let ch = Arc::new(Mutex::new(ch));
    let fechou = Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let (ch, fechou) = (ch.clone(), fechou.clone());
        // stdin fechado: mata o stream, o laço abaixo acaba. Sem isso o `log stream` ficaria órfão.
        std::thread::spawn(move || {
            let _ = std::io::copy(&mut std::io::stdin().lock(), &mut std::io::sink());
            fechou.store(true, Ordering::Relaxed);
            let _ = ch.lock().map(|mut c| c.kill());
        });
    }
    let mut vol: Option<(Instant, Medida, Value)> = None;
    let mut vigia = sources::painel::Vigia::default();
    let (mut r, mut linha) = (std::io::BufReader::new(out), Vec::new());
    while r.read_until(b'\n', &mut linha).is_ok_and(|n| n > 0) {
        if let Some(t) = vigia.linha(String::from_utf8_lossy(&linha).trim_end()) {
            let et = Etapas::new(ctl.clone());
            if vol.as_ref().is_none_or(|(em, ..)| em.elapsed().as_secs() >= 60) {
                vol = et.run("volumes", sources::volumes::coletar).map(|v| (Instant::now(), v.macos, v.json));
            }
            let (macos, volumes) = vol.as_ref().map_or_else(|| (et.falhou(), Value::Null), |(_, m, v)| (m.clone(), v.clone()));
            let (tempos, erros) = et.fim();
            let nao_lida = || Medida::NaoMedida("not read (--parts)".into());
            let partes = Partes { meta: false, volumes: true, swap: false, uso: false, purgeable: false, tela: true };
            emitir(&Rapido {
                partes, caps: Caps::ler(DATA), tempos, erros, fda: Value::Null, root, boot_time: sys::boot_time(), macos, volumes, swap: None,
                usado: nao_lida(), capacidade: None, purg: None, purg_total: nao_lida(), tela_log: Some(t), tela_pulada: false, interrompida: false,
            });
        }
        linha.clear();
    }
    let _ = ch.lock().map(|mut c| {
        let _ = c.kill();
        c.wait()
    });
    if fechou.load(Ordering::Relaxed) { Ok(()) } else { Err("/usr/bin/log stream acabou".into()) }
}

/// Mudança no total da árvore que vira linha nova no `--auto` (como a escuta da tela no app).
const MUDANCA_AUTO: i64 = 10_000_000;

/// "Crescendo agora": janelas (segundos) e o mínimo líquido para uma pasta aparecer.
const CRESCER_JANELAS: [u64; 2] = [300, 3600];
const CRESCER_MIN: i64 = 1_000_000;
const CRESCER_TOP: usize = 10;
/// Teto de releituras guardadas (a mais velha sai primeiro); uma hora normal fica bem abaixo.
const CRESCER_MAX: usize = 100_000;

/// As pastas que mais cresceram em cada janela, a partir do que cada releitura mudou. A soma de
/// cada pasta inclui as de baixo; aparece a mais funda que passou do mínimo (a mãe só aparece
/// quando o crescimento está espalhado em filhas que, sozinhas, ficam abaixo dele).
fn crescendo(eventos: &std::collections::VecDeque<(std::time::Instant, String, i64)>, agora: std::time::Instant, desde: std::time::Instant) -> Value {
    use std::collections::{HashMap, HashSet};
    let janelas: Vec<Value> = CRESCER_JANELAS
        .iter()
        .map(|&seg| {
            let janela = std::time::Duration::from_secs(seg);
            let (mut soma, mut liquido) = (HashMap::<&str, i64>::new(), 0i64);
            for (_, p, d) in eventos.iter().filter(|e| agora.saturating_duration_since(e.0) <= janela) {
                liquido += d;
                let p = p.trim_end_matches('/');
                if p.is_empty() {
                    continue;
                }
                for (i, _) in p.match_indices('/').skip(1) {
                    *soma.entry(&p[..i]).or_default() += d;
                }
                *soma.entry(p).or_default() += d;
            }
            let cands: Vec<(&str, i64)> = soma.into_iter().filter(|e| e.1 >= CRESCER_MIN).collect();
            let com_filha: HashSet<&str> = cands.iter().flat_map(|(p, _)| p.match_indices('/').skip(1).map(move |(i, _)| &p[..i])).collect();
            let mut top: Vec<(&str, i64)> = cands.into_iter().filter(|(p, _)| !com_filha.contains(p)).collect();
            top.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
            top.truncate(CRESCER_TOP);
            json!({
                "window_seconds": seg,
                "covered_seconds": agora.saturating_duration_since(desde).as_secs().min(seg),
                "net_bytes": liquido,
                "top": top.iter().map(|(p, b)| json!({"path": p, "bytes": b})).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!(janelas)
}

/// `sdtally --auto`: árvore viva. Uma passada inteira e, depois, só as pastas que o FSEvents
/// avisar; `emitir` recebe uma linha a cada mudança de `MUDANCA_AUTO` ou mais no total (ou, com
/// mudança menor, uma por minuto). Aviso perdido (o macOS diz) refaz a passada. Sai do processo
/// assim que o stdin fecha, mesmo no meio da passada (o app que o abriu saiu ou o fechou).
pub fn vigiar_arvore(root: bool, home: &str, ferramenta: &str, ctx: Arc<Ctx>, ctl: Arc<walk::Controle>, mut emitir: impl FnMut(&Value)) -> Result<(), String> {
    use std::sync::mpsc;
    use std::time::Duration;
    use sys::fsevents::{self, PERDIDOS_KERNEL, PERDIDOS_USUARIO, RAIZ_MUDOU, RELER_ABAIXO};
    // Sem xattr: medir triplica a passada (~150 s de CPU contra ~52 s) por ~0,5 GB; fica com a
    // medição completa.
    walk::MEASURE_XATTR.store(false, Ordering::Relaxed);
    let mut vivo = walk::vivo::Vivo::novo(DATA, ctx.clone(), ctl.clone())?;
    vivo.intervalo_pesado = Duration::from_secs(300);
    let (tx, rx) = mpsc::channel::<fsevents::Lote>();
    // Escuta antes da passada: o que mudar durante ela é relido logo depois.
    let _vigia = fsevents::vigiar("/", 1.0, tx)?;
    // A passada (~40 s) e as outras fontes (~20 s) não olham o stdin: esperar por elas deixava o
    // processo vivo, órfão, depois de o app sair. Nada fica por gravar, então sai direto.
    std::thread::spawn(|| {
        let _ = std::io::copy(&mut std::io::stdin().lock(), &mut std::io::sink());
        sys::sair(0);
    });
    // As outras fontes (volumes, snapshots, purgeable, apagados abertos, lixo, logs do painel) e os
    // fatos das categorias (apps, bibliotecas, Mensagens) são lidos à parte e relidos a cada
    // `OUTRAS`; cada linha refaz só a conta com a árvore do momento, que não fica guardada entre
    // uma linha e outra. A primeira leitura vem antes da passada: o malloc guarda o que elas
    // usaram (~30 MB) e a passada reaproveita, em vez de somar ao pico.
    const OUTRAS: Duration = Duration::from_secs(600);
    let opcoes = Opcoes { threads: None, xattr: false, root };
    let coletar_outras = |r: Option<walk::Result>| {
        let t = Instant::now();
        let tem = r.is_some();
        let mut c = coletar_com(&opcoes, home, ctx.clone(), ctl.clone(), || r.ok_or_else(String::new));
        if !tem {
            c.erros.retain(|e| e["etapa"] != "arvore");
        }
        c.arvore = None;
        let mut erros = Vec::new();
        let f = sources::categorias::fatos::coletar(home, c.fda.as_bool(), c.purg.as_ref().map(|p| &p.json), None, &mut erros);
        c.erros.extend(erros.into_iter().map(|e| json!({"etapa": "categories", "erro": e})));
        if std::env::var_os("SDTALLY_DEBUG_AUTO").is_some() {
            eprintln!("outras fontes {} ms {:?}", t.elapsed().as_millis(), c.tempos);
        }
        (c, f)
    };
    let mut c = coletar_outras(None);
    let mut outras = Instant::now();
    let n = threads(None);
    let t = Instant::now();
    vivo.passada(n);
    let mut passada = t.elapsed().as_secs_f64();
    let (mut avisos, mut releituras, mut passadas) = (0u64, 0u64, 1u64);
    let (mut ms_reler, mut ms_somar) = (0f64, 0f64);
    // (quando, pasta, quanto mudou) de cada releitura, da última hora.
    let mut cresc: std::collections::VecDeque<(Instant, String, i64)> = std::collections::VecDeque::new();
    let vivo_desde = Instant::now();
    let linha = |(c, f): &mut (Coleta, sources::categorias::Fatos), r: walk::Result, passada: f64, avisos: u64, releituras: u64, passadas: u64, pastas: usize, outras: &Instant, crescimento: Value| {
        let (agora, _) = sys::now_local();
        f.arvore = Some(sources::categorias::fatos::arvore(&r));
        c.arvore = Some(r);
        let t = Instant::now();
        let mut v = crate::report::json_completo(c, ferramenta, &agora.chars().take(19).collect::<String>().replace('T', " "), home, Some(f));
        (c.arvore, f.arvore) = (None, None);
        if std::env::var_os("SDTALLY_DEBUG_AUTO").is_some() {
            eprintln!("relatório {} ms", t.elapsed().as_millis());
        }
        v["kind"] = json!("auto");
        v["auto"] = json!({
            "last_pass_seconds": round2(passada),
            "passes": passadas,
            "events": avisos,
            "rereads": releituras,
            "folders_in_memory": pastas,
            "other_sources_age_seconds": outras.elapsed().as_secs(),
        });
        v["growing_now"] = crescimento;
        v
    };
    let r = vivo.resultado();
    // As pastas negadas só se conhecem depois da passada.
    let neg = sources::dirstats::negadas(&r.denied, &ctx);
    c.0.negadas_total = neg.total.clone();
    c.0.negadas = Some(neg);
    c.0.tempos.insert("arvore".into(), json!(round2(passada)));
    let (mut total, mut desde) = (r.total.alloc, Instant::now());
    emitir(&linha(&mut c, r, passada, avisos, releituras, passadas, vivo.pastas(), &outras, crescendo(&cresc, Instant::now(), vivo_desde)));
    // Orçamento de releitura: `ORCAMENTO_MS` de trabalho por segundo (0,5 % de um núcleo), com
    // reserva de até 3 s. Só pasta cara (5 ms ou mais da última vez, como um cache com dezenas de
    // milhares de arquivos reescrita o tempo todo) espera crédito, e também 20× o que custou, no
    // mínimo 5 s, antes de ser relida de novo; pasta barata é relida na hora. Aviso no meio fica
    // pendente para quando vencer.
    // caminho -> (pode reler em, flags pendentes, pendente)
    const ORCAMENTO_MS: f64 = 5.0;
    let mut espera: std::collections::HashMap<String, (Instant, u32, bool)> = std::collections::HashMap::new();
    let (mut credito, mut credito_em) = (3000.0f64, Instant::now());
    let mut caras: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    loop {
        // Sem aviso nenhum, acorda ao menos para reler as outras fontes.
        let prazo = espera.values().filter(|e| e.2).map(|e| e.0).chain([outras + OUTRAS]).min();
        let mut lote = match prazo {
            Some(p) => match rx.recv_timeout(p.saturating_duration_since(Instant::now())) {
                Ok(l) => l,
                Err(mpsc::RecvTimeoutError::Timeout) => Vec::new(),
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            },
            None => match rx.recv() {
                Ok(l) => l,
                Err(_) => break,
            },
        };
        // Junta o que chegar em seguida, até 2 s, para reler cada pasta uma vez só.
        let junta = Instant::now();
        while junta.elapsed() < Duration::from_secs(2) {
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(mais) => lote.extend(mais),
                Err(_) => break,
            }
        }
        avisos += lote.len() as u64;
        if std::env::var_os("SDTALLY_DEBUG_AUTO").is_some() {
            for (p, f) in lote.iter().filter(|(_, f)| f & (PERDIDOS_USUARIO | PERDIDOS_KERNEL | RAIZ_MUDOU | RELER_ABAIXO) != 0) {
                eprintln!("aviso {f:#x} {p}");
            }
        }
        if lote.iter().any(|(_, f)| f & (PERDIDOS_USUARIO | PERDIDOS_KERNEL | RAIZ_MUDOU) != 0) {
            let t = Instant::now();
            vivo.passada(n);
            passada = t.elapsed().as_secs_f64();
            passadas += 1;
        } else {
            let agora = Instant::now();
            for (p, e) in espera.iter_mut().filter(|(_, e)| e.2 && e.0 <= agora) {
                lote.push((p.clone(), e.1));
                (e.1, e.2) = (0, false);
            }
            espera.retain(|_, e| e.2 || e.0 > agora);
            lote.sort_unstable();
            lote.dedup_by(|a, b| {
                let igual = a.0 == b.0;
                if igual {
                    b.1 |= a.1;
                }
                igual
            });
            let t = Instant::now();
            credito = (credito + credito_em.elapsed().as_secs_f64() * ORCAMENTO_MS).min(3000.0);
            credito_em = t;
            for (p, f) in &lote {
                if let Some(e) = espera.get_mut(p).filter(|e| e.0 > agora) {
                    (e.1, e.2) = (e.1 | f, true);
                    continue;
                }
                if credito <= 0.0 && caras.contains_key(p) {
                    let volta = Instant::now() + Duration::from_secs_f64(-credito / ORCAMENTO_MS + 0.1);
                    let e = espera.entry(p.clone()).or_insert((volta, 0, true));
                    (e.0, e.1, e.2) = (e.0.max(volta), e.1 | f, true);
                    continue;
                }
                let t1 = Instant::now();
                let d = vivo.reler(p, f & RELER_ABAIXO != 0);
                let custo = t1.elapsed();
                if d != 0 {
                    if cresc.len() >= CRESCER_MAX {
                        cresc.pop_front();
                    }
                    cresc.push_back((t1, p.clone(), d));
                }
                credito -= custo.as_secs_f64() * 1e3;
                if custo >= Duration::from_millis(5) {
                    espera.insert(p.clone(), (Instant::now() + (custo * 20).max(Duration::from_secs(5)), 0, false));
                    caras.insert(p.clone(), custo.as_secs_f64() * 1e3);
                } else {
                    caras.remove(p);
                }
                if std::env::var_os("SDTALLY_DEBUG_AUTO").is_some() && custo.as_millis() >= 30 {
                    eprintln!("lenta {} ms {f:#x} {p}", custo.as_millis());
                }
                releituras += 1;
            }
            ms_reler += t.elapsed().as_secs_f64() * 1e3;
        }
        let t = Instant::now();
        let agora_total = vivo.total_alloc();
        ms_somar += t.elapsed().as_secs_f64() * 1e3;
        if std::env::var_os("SDTALLY_DEBUG_AUTO").is_some() {
            eprintln!("lote {} avisos · reler {ms_reler:.0} ms · somar {ms_somar:.0} ms (acumulados)", lote.len());
        }
        let velhas = outras.elapsed() >= OUTRAS;
        if velhas {
            c = coletar_outras(Some(vivo.resultado()));
            outras = Instant::now();
        }
        if velhas || (agora_total - total).abs() >= MUDANCA_AUTO || (agora_total != total && desde.elapsed() >= Duration::from_secs(60)) {
            let r = vivo.resultado();
            (total, desde) = (r.total.alloc, Instant::now());
            let agora = Instant::now();
            while cresc.front().is_some_and(|e| agora.saturating_duration_since(e.0).as_secs() > CRESCER_JANELAS[1]) {
                cresc.pop_front();
            }
            emitir(&linha(&mut c, r, passada, avisos, releituras, passadas, vivo.pastas(), &outras, crescendo(&cresc, agora, vivo_desde)));
        }
    }
    Ok(())
}

fn swap(err: &mut Vec<Value>) -> Option<(u64, u64)> {
    let s = sys::swap_usage();
    if s.is_none() {
        err.push(json!(format!("sysctl vm.swapusage errno {}", sys::errno())));
    }
    s
}

/// Acesso Total ao Disco pela pasta do Safari (TCC): true/false, ou o errno se indeterminado.
fn fda_de(home: &str) -> Value {
    match std::fs::read_dir(format!("{home}/Library/Safari")).map(|mut d| d.next().map(|e| e.is_ok())) {
        Ok(_) => json!(true),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => json!(false),
        Err(e) => json!(format!("indeterminado ({})", e.raw_os_error().unwrap_or(0))),
    }
}

impl Coleta {
    /// Os termos da conta do volume Data, na ordem da síntese §2.3, cada um com a sua chave.
    pub fn termos(&self) -> [(Chave, Termo); 6] {
        let arv_alloc = match &self.arvore {
            None if self.interrompida => Medida::NaoMedida("interrompido".into()),
            None => Medida::NaoMedida("travessia falhou".into()),
            Some(r) if r.interrompida => Medida::Parcial(r.total.alloc, "interrompido".into()),
            Some(r) => Medida::Valor(r.total.alloc),
        };
        let xattr_s = match (&self.arvore, self.xattr) {
            (_, false) => Medida::NaoMedida("--sem-xattr".into()),
            (None, true) if self.interrompida => Medida::NaoMedida("interrompido".into()),
            (None, true) => Medida::NaoMedida("travessia falhou".into()),
            (Some(r), true) if r.xattr_erros > 0 => {
                Medida::Parcial(r.total.xattr_stream, format!("{} objetos com xattr ilegível", r.xattr_erros))
            }
            (Some(r), true) if r.interrompida => Medida::Parcial(r.total.xattr_stream, "interrompido".into()),
            (Some(r), true) => Medida::Valor(r.total.xattr_stream),
        };
        // Sem o resultado da etapa: Ctrl-C ou panic capturado (o erro já está em `stage_errors`).
        let meta = match &self.apfs_meta {
            Some(m) => m.medida.clone(),
            None if self.interrompida => Medida::NaoMedida("interrompido".into()),
            None => Medida::NaoMedida("falha interna".into()),
        };
        [
            (Chave::Arvore, Termo { chave: "arvore_medida", medida: arv_alloc, fonte: "travessia (arvore.totais.alocado_unico)" }),
            (
                Chave::Xattr,
                Termo {
                    chave: "xattr_fluxo_proprio",
                    medida: xattr_s,
                    fonte: if self.xattr { "listxattr/getxattr, valores > 3804 B" } else { "não medido (--sem-xattr)" },
                },
            ),
            (Chave::Snapshots, Termo { chave: "snapshots_faixa_total", medida: self.snap_total.clone(), fonte: "fsctl 0xC0184A13 faixa total" }),
            (
                Chave::Apagados,
                Termo {
                    chave: "apagados_abertos",
                    medida: self.apagados.clone(),
                    fonte: if self.root { "lsof +L1 + alocado do fd (todos os processos)" } else { "lsof +L1 + alocado do fd (só processos do usuário)" },
                },
            ),
            (Chave::Negadas, Termo { chave: "pastas_negadas_dirstats", medida: self.negadas_total.clone(), fonte: "dir-stats das pastas EPERM/EACCES" }),
            (Chave::Metadados, Termo { chave: "metadados_apfs", medida: meta, fonte: "AppleAPFSUserClient: nós omap+fsroot+extentref × bloco (piso)" }),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mais funda acima do mínimo aparece; a mãe, só quando o crescimento está espalhado.
    #[test]
    fn crescendo_mostra_a_pasta_mais_funda() {
        let agora = Instant::now();
        let ha = |s: u64| agora - std::time::Duration::from_secs(s);
        let ev: std::collections::VecDeque<(Instant, String, i64)> = [
            (ha(10), "/Users/x/Library/Caches/app/a", 3_000_000),
            (ha(20), "/Users/x/Library/Caches/app/a", 2_000_000),
            (ha(30), "/Users/x/Library/Logs/l1", 600_000),
            (ha(30), "/Users/x/Library/Logs/l2", 600_000),
            (ha(40), "/Users/x/Downloads", -9_000_000),
            (ha(1000), "/Users/x/Movies", 50_000_000),
        ]
        .into_iter()
        .map(|(t, p, d)| (t, p.to_string(), d))
        .collect();
        let v = crescendo(&ev, agora, ha(2000));
        let curta: Vec<(&str, i64)> = v[0]["top"].as_array().unwrap().iter().map(|e| (e["path"].as_str().unwrap(), e["bytes"].as_i64().unwrap())).collect();
        assert_eq!(curta, [("/Users/x/Library/Caches/app/a", 5_000_000), ("/Users/x/Library/Logs", 1_200_000)]);
        assert_eq!(v[0]["net_bytes"], -2_800_000);
        assert_eq!(v[0]["covered_seconds"], 300);
        assert_eq!(v[1]["top"][0]["path"], "/Users/x/Movies");
    }

    /// Etapas em threads terminam em qualquer ordem; `stage_errors` sai sempre na ordem de ORDEM,
    /// e o que uma etapa registrou fica junto e na ordem em que ela registrou.
    #[test]
    fn erros_das_etapas_em_ordem_fixa() {
        static NAO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        let et = Etapas::new(Arc::new(walk::Controle::new(&NAO)));
        std::thread::scope(|s| {
            for nome in ["tela_log", "snapshots", "volumes"] {
                let et = &et;
                s.spawn(move || {
                    et.run(nome, |err| {
                        err.push(json!("a"));
                        err.push(json!("b"));
                    });
                });
            }
        });
        let r: Option<u8> = et.run("meta", |_| panic!("falha de teste"));
        assert_eq!(r, None);
        let (tempos, erros) = et.fim();
        let ordem: Vec<_> = erros.iter().map(|e| (e["etapa"].as_str().unwrap_or(""), e["erro"].as_str().unwrap_or(""))).collect();
        assert_eq!(
            ordem,
            [("meta", "falha interna (panic capturado)"), ("volumes", "a"), ("volumes", "b"), ("snapshots", "a"), ("snapshots", "b"), ("tela_log", "a"), ("tela_log", "b")]
        );
        assert_eq!(tempos.len(), 4);
    }

    #[test]
    fn nome_da_home_pela_pasta() {
        assert_eq!(super::nome_da_home("curto", "/Users/exemplo"), "exemplo");
        assert_eq!(super::nome_da_home("exemplo", "/Users/exemplo"), "exemplo");
        assert_eq!(super::nome_da_home("exemplo", "/var/root"), "exemplo");
        assert_eq!(super::nome_da_home("exemplo", "/Users/a/b"), "exemplo");
    }
}

#[cfg(test)]
mod testes_partes {
    use super::Partes;

    #[test]
    fn partes_da_lista() {
        let p = Partes::de("uso,purgeable").expect("lista válida");
        assert!(p.uso && p.purgeable && !p.meta && !p.volumes && !p.swap && !p.tela);
        assert_eq!(Partes::de(&Partes::NOMES.join(",")), Some(Partes::TODAS));
        assert_eq!(Partes::de("uso,xyz"), None);
        assert_eq!(Partes::de(""), None);
    }
}

//! FFI e chamadas de sistema, todas só leitura (exceto `PastaSaida`, que só cria o relatório do
//! sdc-coleta). Único lugar com `unsafe` (lib.rs nega no resto).

pub mod apfs;
pub mod cachedelete;
pub mod cripto;
pub mod disco;
pub mod fsevents;
pub mod iokit;

use std::ffi::{CStr, CString, OsStr, c_int, c_void};
use std::io::{BufRead, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const IOPOL_TYPE_VFS_ATIME_UPDATES: c_int = 2;
const IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES: c_int = 3;
const IOPOL_SCOPE_PROCESS: c_int = 0;
const IOPOL_ATIME_UPDATES_OFF: c_int = 1;
const IOPOL_MATERIALIZE_DATALESS_FILES_OFF: c_int = 1;
const QOS_CLASS_UTILITY: u32 = 0x11;
const PROC_PIDFDVNODEINFO: c_int = 1;
const PROC_PIDREGIONPATHINFO: c_int = 8;

/// struct vnode_fdinfo (sys/proc_info.h): proc_fileinfo (24 B) + vnode_info. Ausente no crate libc.
#[repr(C)]
struct VnodeFdinfo {
    pfi: [u64; 3],
    pvi: libc::vnode_info,
}

/// struct proc_regionwithpathinfo (sys/proc_info.h): proc_regioninfo (96 B, endereço e tamanho
/// no fim) + vnode_info_path. Ausente no crate libc.
#[repr(C)]
struct RegiaoComVnode {
    pri: [u64; 10],
    pri_address: u64,
    pri_size: u64,
    vip: libc::vnode_info_path,
}

/// (dispositivo, inode, alocado = st_blocks × 512) de um vinfo_stat.
fn alocado(st: &libc::vinfo_stat) -> (u32, u64, i64) {
    (st.vst_dev, st.vst_ino, st.vst_blocks * 512)
}

unsafe extern "C" {
    fn setiopolicy_np(iotype: c_int, scope: c_int, policy: c_int) -> c_int;
    fn pthread_set_qos_class_self_np(qos: u32, rel: c_int) -> c_int;
}

pub fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

/// Síntese §4 (1 e 2b): sem materializar dataless e sem gravar atime. Chamar antes de qualquer
/// thread; Err((materialize, atime)) com os retornos se alguma falhar.
pub fn politicas_seguranca() -> Result<(), (i32, i32)> {
    let r1 = unsafe { setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS, IOPOL_MATERIALIZE_DATALESS_FILES_OFF) };
    let r2 = unsafe { setiopolicy_np(IOPOL_TYPE_VFS_ATIME_UPDATES, IOPOL_SCOPE_PROCESS, IOPOL_ATIME_UPDATES_OFF) };
    if r1 != 0 || r2 != 0 { Err((r1, r2)) } else { Ok(()) }
}

pub fn qos_utility() {
    unsafe { pthread_set_qos_class_self_np(QOS_CLASS_UTILITY, 0) };
}

/// Prioridade de segundo plano no processo inteiro (CPU e disco), como o `taskpolicy -b`: o
/// macOS adia o trabalho enquanto a pessoa usa o Mac. false se o kernel recusar.
/// Os filhos herdam a prioridade, e os prazos de `run_io` crescem 5x: com a pessoa usando o Mac, o
/// `log show` de 7 dias passou de 44 s para 200 s (medido em 05/10/2026, prazo era 90 s).
pub fn modo_fundo() -> bool {
    let ok = unsafe { libc::setpriority(libc::PRIO_DARWIN_PROCESS, 0, libc::PRIO_DARWIN_BG) == 0 };
    if ok {
        ESCALA_PRAZO.store(5, std::sync::atomic::Ordering::Relaxed);
    }
    ok
}

static ESCALA_PRAZO: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// (pid, início do processo em abstime, nome, bytes escritos no disco, escritas lógicas) de cada
/// processo vivo, pelo proc_pid_rusage. Sem root, só os do próprio usuário; quem já saiu some.
pub fn escrita_processos() -> Vec<(i32, u64, String, u64, u64)> {
    let n = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    if n <= 0 {
        return Vec::new();
    }
    let mut pids = vec![0i32; n as usize + 64];
    let n = unsafe { libc::proc_listallpids(pids.as_mut_ptr() as *mut c_void, (pids.len() * 4) as c_int) };
    pids.truncate(n.max(0) as usize);
    pids.into_iter()
        .filter(|&p| p > 0)
        .filter_map(|pid| {
            let mut ri: libc::rusage_info_v4 = unsafe { std::mem::zeroed() };
            if unsafe { libc::proc_pid_rusage(pid, libc::RUSAGE_INFO_V4, &mut ri as *mut _ as *mut libc::rusage_info_t) } != 0 {
                return None;
            }
            let mut b = [0u8; 256];
            let k = unsafe { libc::proc_name(pid, b.as_mut_ptr() as *mut c_void, b.len() as u32) };
            let nome = String::from_utf8_lossy(&b[..k.max(0) as usize]).into_owned();
            Some((pid, ri.ri_proc_start_abstime, nome, ri.ri_diskio_byteswritten, ri.ri_logical_writes))
        })
        .collect()
}

/// Caminhos dos dois processos que gravam a tela do Ajustes no log: a extensão `Storage` (linha
/// "Sum of all categories") e o StorageManagementService (bloco StorageLogInvestigation).
pub(crate) fn e_processo_do_painel(caminho: &str) -> bool {
    caminho.ends_with("/Storage.appex/Contents/MacOS/Storage")
        || caminho.ends_with("/StorageManagement.framework/PlugIns/StorageManagementService")
}

/// O painel Ajustes > Geral > Armazenamento está aberto agora: algum desses processos vive. É o
/// único jeito de o log ganhar linha da tela (fechado, nada novo foi gravado), então a leitura
/// rápida incremental só abre o `log show` (~110 MB, ~0,12 s de CPU) quando isto é verdade.
pub fn painel_do_ajustes_aberto() -> bool {
    let n = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    if n <= 0 {
        return false;
    }
    let mut pids = vec![0i32; n as usize + 64];
    let n = unsafe { libc::proc_listallpids(pids.as_mut_ptr() as *mut c_void, (pids.len() * 4) as c_int) };
    pids.truncate(n.max(0) as usize);
    pids.into_iter().filter(|&p| p > 0).any(|pid| {
        let mut nome = [0u8; 64];
        let k = unsafe { libc::proc_name(pid, nome.as_mut_ptr() as *mut c_void, nome.len() as u32) };
        let nome = &nome[..k.max(0) as usize];
        if nome != b"Storage" && !nome.starts_with(b"StorageManagementS") {
            return false;
        }
        let mut b = [0u8; 1024];
        let k = unsafe { libc::proc_pidpath(pid, b.as_mut_ptr() as *mut c_void, b.len() as u32) };
        k > 0 && e_processo_do_painel(&String::from_utf8_lossy(&b[..k as usize]))
    })
}

pub fn e_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Caminho para C; NUL no meio vira EINVAL, como o kernel responderia.
pub fn cpath(p: impl AsRef<OsStr>) -> Result<CString, i32> {
    CString::new(p.as_ref().as_bytes()).map_err(|_| libc::EINVAL)
}

/// statfs: (total, livre) — no APFS é o container.
pub fn statfs(p: &str) -> Result<(u64, u64), i32> {
    let c = cpath(p)?;
    let mut st: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
        return Err(errno());
    }
    Ok((st.f_blocks * st.f_bsize as u64, st.f_bavail * st.f_bsize as u64))
}

/// Dispositivo ("disk3s5") do volume montado em `p`, pelo `f_mntfromname` do statfs.
pub fn dispositivo_de(p: &str) -> Result<String, i32> {
    let c = cpath(p)?;
    let mut st: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
        return Err(errno());
    }
    let d = unsafe { CStr::from_ptr(st.f_mntfromname.as_ptr()) }.to_string_lossy().into_owned();
    Ok(d.strip_prefix("/dev/").unwrap_or(&d).to_string())
}

/// Ponto de montagem do volume que guarda `p` (statfs segue firmlink e symlink). No grupo de
/// volumes System e Data dão o mesmo st_dev; aqui vêm "/" e "/System/Volumes/Data".
pub fn montado_em(p: &Path) -> Result<String, i32> {
    let c = cpath(p)?;
    let mut st: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
        return Err(errno());
    }
    Ok(unsafe { CStr::from_ptr(st.f_mntonname.as_ptr()) }.to_string_lossy().into_owned())
}

pub fn sysctl_str(name: &str) -> Option<String> {
    let c = cpath(name).ok()?;
    let mut len = 0usize;
    if unsafe { libc::sysctlbyname(c.as_ptr(), std::ptr::null_mut(), &mut len, std::ptr::null_mut(), 0) } != 0 {
        return None;
    }
    let mut b = vec![0u8; len];
    if unsafe { libc::sysctlbyname(c.as_ptr(), b.as_mut_ptr() as *mut c_void, &mut len, std::ptr::null_mut(), 0) } != 0 {
        return None;
    }
    CStr::from_bytes_until_nul(&b).ok().map(|s| s.to_string_lossy().into_owned())
}

pub fn sysctl_u64(name: &str) -> Option<u64> {
    let c = cpath(name).ok()?;
    let mut b = [0u8; 8];
    let mut len = 8usize;
    if unsafe { libc::sysctlbyname(c.as_ptr(), b.as_mut_ptr() as *mut c_void, &mut len, std::ptr::null_mut(), 0) } != 0 {
        return None;
    }
    Some(match len {
        4 => u32::from_ne_bytes(b[..4].try_into().ok()?) as u64,
        _ => u64::from_ne_bytes(b),
    })
}

/// (total, usado) dos arquivos de swap no volume VM, pelo sysctl vm.swapusage (sem root).
pub fn swap_usage() -> Option<(u64, u64)> {
    let c = cpath("vm.swapusage").ok()?;
    let mut x: libc::xsw_usage = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::xsw_usage>();
    if unsafe { libc::sysctlbyname(c.as_ptr(), &mut x as *mut _ as *mut c_void, &mut len, std::ptr::null_mut(), 0) } != 0 {
        return None;
    }
    (len == std::mem::size_of::<libc::xsw_usage>()).then_some((x.xsu_total, x.xsu_used))
}

/// (dispositivo, inode, alocado) do arquivo aberto no descritor `fd` do processo `pid`, pelo
/// proc_pidfdinfo (o que o lsof usa). Sem root, só processos do próprio usuário.
pub fn fd_alocado(pid: i32, fd: i32) -> Result<(u32, u64, i64), i32> {
    let mut v: VnodeFdinfo = unsafe { std::mem::zeroed() };
    let tam = std::mem::size_of::<VnodeFdinfo>() as c_int;
    let n = unsafe { libc::proc_pidfdinfo(pid, fd, PROC_PIDFDVNODEINFO, &mut v as *mut _ as *mut c_void, tam) };
    if n != tam {
        return Err(if n <= 0 { errno() } else { libc::EIO });
    }
    Ok(alocado(&v.pvi.vi_stat))
}

/// (dispositivo, inode, alocado) de cada arquivo mapeado na memória do processo (o "txt" do
/// lsof: apagado sem fd). Percorre as regiões com PROC_PIDREGIONPATHINFO; região sem arquivo
/// vem com inode 0 e fica de fora.
pub fn mapeados_alocado(pid: i32) -> Result<Vec<(u32, u64, i64)>, i32> {
    let tam = std::mem::size_of::<RegiaoComVnode>() as c_int;
    let (mut end, mut out) = (0u64, Vec::new());
    loop {
        let mut r: RegiaoComVnode = unsafe { std::mem::zeroed() };
        let n = unsafe { libc::proc_pidinfo(pid, PROC_PIDREGIONPATHINFO, end, &mut r as *mut _ as *mut c_void, tam) };
        if n != tam {
            // Depois da última região o kernel devolve erro: fim normal se já leu alguma.
            return if out.is_empty() && end == 0 { Err(if n <= 0 { errno() } else { libc::EIO }) } else { Ok(out) };
        }
        let st = &r.vip.vip_vi.vi_stat;
        if st.vst_ino != 0 {
            out.push(alocado(st));
        }
        let prox = r.pri_address.saturating_add(r.pri_size);
        if prox <= end {
            return Ok(out);
        }
        end = prox;
    }
}

/// Comando externo com timeout. Ok(stdout) só com status 0.
pub fn run(cmd: &str, args: &[&str], secs: u64) -> Result<String, String> {
    match run_status(cmd, args, secs)? {
        (true, s) => Ok(s),
        (false, _) => Err(format!("{cmd}: saiu com erro")),
    }
}

/// Como `run`, mas devolve a saída também quando o status não é 0.
pub fn run_status(cmd: &str, args: &[&str], secs: u64) -> Result<(bool, String), String> {
    let (ok, o) = run_io(cmd, args, None, secs)?;
    Ok((ok, String::from_utf8_lossy(&o).into_owned()))
}

/// Comando com stdin opcional, stdout lido numa thread (sem travar em pipe cheio) e timeout.
fn run_io(cmd: &str, args: &[&str], stdin: Option<Vec<u8>>, secs: u64) -> Result<(bool, Vec<u8>), String> {
    run_leitor(cmd, args, stdin, secs, |mut o| {
        let mut s = Vec::new();
        let _ = o.read_to_end(&mut s);
        s
    })
}

/// Como `run`, mas a saída passa linha a linha por `f` (UTF-8 com perda, sem o `\n`) e nada dela
/// fica guardado: serve para saída grande, de que só um resumo interessa. O resumo `T` sai só se o
/// status for 0.
pub fn run_linhas<T: Default + Send + 'static>(cmd: &str, args: &[&str], secs: u64, mut f: impl FnMut(&str, &mut T) + Send + 'static) -> Result<T, String> {
    let (ok, t) = run_leitor(cmd, args, None, secs, move |o| {
        let (mut r, mut t, mut linha) = (std::io::BufReader::new(o), T::default(), Vec::new());
        while r.read_until(b'\n', &mut linha).is_ok_and(|n| n > 0) {
            f(String::from_utf8_lossy(&linha).trim_end_matches(['\n', '\r']), &mut t);
            linha.clear();
        }
        t
    })?;
    if ok { Ok(t) } else { Err(format!("{cmd}: saiu com erro")) }
}

/// Filhos vivos (pid), mortos junto quando o sdtally sai antes deles: sem isso um `log show
/// --last 7d` fica órfão e roda minutos depois de o app fechar. Atômicos porque o handler de
/// sinal lê; com as vagas cheias o filho só não é registrado.
static FILHOS: [std::sync::atomic::AtomicI32; 32] = [const { std::sync::atomic::AtomicI32::new(0) }; 32];

/// Registra o filho enquanto o valor viver.
pub struct Filho(i32);

pub fn filho(pid: u32) -> Filho {
    use std::sync::atomic::Ordering::SeqCst;
    let pid = pid as i32;
    let _ = FILHOS.iter().find(|v| v.compare_exchange(0, pid, SeqCst, SeqCst).is_ok());
    Filho(pid)
}

impl Drop for Filho {
    fn drop(&mut self) {
        use std::sync::atomic::Ordering::SeqCst;
        for v in &FILHOS {
            let _ = v.compare_exchange(self.0, 0, SeqCst, SeqCst);
        }
    }
}

/// SIGKILL nos filhos registrados. Só atômicos e kill(2): seguro dentro de handler de sinal.
pub(crate) fn matar_filhos() {
    for v in &FILHOS {
        let p = v.swap(0, std::sync::atomic::Ordering::SeqCst);
        if p > 0 {
            unsafe { libc::kill(p, libc::SIGKILL) };
        }
    }
}

/// `process::exit` que antes mata os filhos.
pub fn sair(codigo: i32) -> ! {
    matar_filhos();
    std::process::exit(codigo)
}

/// `leitor` consome o stdout numa thread própria; o resultado dele vem junto do status.
fn run_leitor<T: Default + Send + 'static>(cmd: &str, args: &[&str], stdin: Option<Vec<u8>>, secs: u64, leitor: impl FnOnce(std::process::ChildStdout) -> T + Send + 'static) -> Result<(bool, T), String> {
    let secs = secs * ESCALA_PRAZO.load(std::sync::atomic::Ordering::Relaxed);
    let mut ch = Command::new(cmd)
        .args(args)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{cmd}: {e}"))?;
    let _filho = filho(ch.id());
    if let (Some(mut i), Some(data)) = (ch.stdin.take(), stdin) {
        std::thread::spawn(move || {
            let _ = i.write_all(&data);
        });
    }
    let out = ch.stdout.take();
    let reader = std::thread::spawn(move || out.map(leitor).unwrap_or_default());
    let t0 = Instant::now();
    let status = loop {
        match ch.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if t0.elapsed() > Duration::from_secs(secs) => {
                let _ = ch.kill();
                let _ = ch.wait();
                return Err(format!("{cmd}: timeout de {secs} s"));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(e) => return Err(format!("{cmd}: {e}")),
        }
    };
    Ok((status.success(), reader.join().unwrap_or_default()))
}

/// plist (texto) → JSON via plutil. Status do plutil conferido: saída de erro não vira dado.
pub fn plist_json(cmd: &str, args: &[&str], secs: u64) -> Result<serde_json::Value, String> {
    let xml = run(cmd, args, secs)?;
    match run_io("/usr/bin/plutil", &["-convert", "json", "-o", "-", "-"], Some(xml.into_bytes()), 30)? {
        (true, o) => serde_json::from_slice(&o).map_err(|e| format!("plutil/json: {e}")),
        (false, _) => Err(format!("plutil: não converteu a saída de {cmd}")),
    }
}

/// Roda `f` numa thread; None se passar do tempo (a thread fica para trás).
pub fn with_timeout<T: Send + 'static>(secs: u64, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        qos_utility();
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(secs)).ok()
}

fn tm_local(t: libc::time_t) -> libc::tm {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    tm
}

/// Segundos desde a época → "AAAA-MM-DD HH:MM:SS" em hora local.
pub fn local(t: i64) -> String {
    let tm = tm_local(t);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec)
}

/// Ligado desde (kern.boottime), "AAAA-MM-DD HH:MM" local. O `sysctl_u64` lê só 8 bytes; o
/// timeval tem 16, por isso o tamanho é conferido.
pub fn boot_time() -> Option<String> {
    let c = cpath("kern.boottime").ok()?;
    let mut tv: libc::timeval = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::timeval>();
    if unsafe { libc::sysctlbyname(c.as_ptr(), &mut tv as *mut _ as *mut c_void, &mut len, std::ptr::null_mut(), 0) } != 0 {
        return None;
    }
    (len == std::mem::size_of::<libc::timeval>() && tv.tv_sec > 0).then(|| local(tv.tv_sec).chars().take(16).collect())
}

/// As únicas chaves que `plist_chave` aceita, com o formato de cada uma (spec fase 6 §4.4):
/// `Destinations` (raw: contagem) e, por destino N, as datas, o RESULT e as listas de datas.
/// NetworkURL, LastKnownVolumeName, DestinationID e UUIDs nunca são pedidos.
pub(crate) fn chave_tm_permitida(chave: &str, formato: &str) -> bool {
    if chave == "Destinations" {
        return formato == "raw";
    }
    let mut p = chave.split('.');
    match (p.next(), p.next(), p.next(), p.next()) {
        (Some("Destinations"), Some(n), Some(k), None) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => match k {
            "ReferenceLocalSnapshotDate" | "StableLocalSnapshotDate" | "RESULT" => formato == "raw",
            "SnapshotDates" | "AttemptDates" => formato == "xml1",
            _ => false,
        },
        _ => false,
    }
}

/// Uma chave do plist pelo `plutil -extract` (nunca -convert: o resto do arquivo nem entra no
/// processo). O erro não carrega a saída do plutil.
pub fn plist_chave(arquivo: &str, chave: &str, formato: &str) -> Result<String, String> {
    if !chave_tm_permitida(chave, formato) {
        return Err("plutil: chave fora da lista fixa".into());
    }
    run("/usr/bin/plutil", &["-extract", chave, formato, "-o", "-", arquivo], 10)
}

/// Segundos desde a época (relógio de parede).
pub fn agora_epoch() -> i64 {
    unsafe { libc::time(std::ptr::null_mut()) as i64 }
}

/// Data/hora local e fuso: ("2026-09-28T14:40:00-0300", "AAAA-MM-DD-HHMM").
/// Relógio monotônico que conta o tempo de repouso (CLOCK_MONOTONIC no Darwin; o `Instant` usa
/// CLOCK_UPTIME_RAW, que para enquanto o Mac dorme). Tempo desde um ponto qualquer.
pub fn mono() -> Duration {
    let mut ts: libc::timespec = unsafe { std::mem::zeroed() };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
}

pub fn now_local() -> (String, String) {
    let tm = tm_local(unsafe { libc::time(std::ptr::null_mut()) });
    let off = tm.tm_gmtoff / 60;
    let (y, mo, d, h, mi, s) = (tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec);
    let sign = if off < 0 { '-' } else { '+' };
    (
        format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}{sign}{:02}{:02}", off.abs() / 60, off.abs() % 60),
        format!("{y:04}-{mo:02}-{d:02}-{h:02}{mi:02}"),
    )
}

/// Pasta de início de um usuário (getpwnam), para achar a home da pessoa sob sudo.
pub fn home_of(user: &str) -> Option<String> {
    let c = cpath(user).ok()?;
    let pw = unsafe { libc::getpwnam(c.as_ptr()) };
    if pw.is_null() {
        return None;
    }
    let dir = unsafe { CStr::from_ptr((*pw).pw_dir) };
    Some(dir.to_string_lossy().into_owned())
}

/// Pontos de montagem atuais (getfsstat, MNT_NOWAIT: não acorda volume de rede).
pub fn montagens() -> Result<Vec<String>, i32> {
    let n = unsafe { libc::getfsstat(std::ptr::null_mut(), 0, libc::MNT_NOWAIT) };
    if n < 0 {
        return Err(errno());
    }
    // Folga para montagens novas entre as duas chamadas.
    let mut v: Vec<libc::statfs> = vec![unsafe { std::mem::zeroed() }; n as usize + 8];
    let bytes = (v.len() * std::mem::size_of::<libc::statfs>()) as libc::c_int;
    let n = unsafe { libc::getfsstat(v.as_mut_ptr(), bytes, libc::MNT_NOWAIT) };
    if n < 0 {
        return Err(errno());
    }
    Ok(v[..n as usize]
        .iter()
        .map(|st| unsafe { std::ffi::CStr::from_ptr(st.f_mntonname.as_ptr()) }.to_string_lossy().into_owned())
        .collect())
}

/// Setado pelo primeiro Ctrl-C (ver `ao_interromper`).
pub static INTERROMPIDO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

extern "C" fn sigint(_: c_int) {
    // Só um store atômico: seguro dentro de handler de sinal.
    INTERROMPIDO.store(true, std::sync::atomic::Ordering::Relaxed);
}

extern "C" fn sighup(_: c_int) {
    // Terminal fechado: a saída passa para /dev/null (open, dup2 e close são seguros em handler),
    // senão o próximo println daria EIO e pânico no meio da última gravação.
    unsafe {
        let fd = libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC);
        if fd >= 0 {
            libc::dup2(fd, 1);
            libc::dup2(fd, 2);
            if fd > 2 {
                libc::close(fd);
            }
        }
    }
    INTERROMPIDO.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Instala `h` para `sinal`, a não ser que o sinal já venha ignorado (`nohup` ignora o SIGHUP:
/// quem pediu para sobreviver ao Terminal fechado continua sobrevivendo).
fn instalar(sinal: c_int, h: extern "C" fn(c_int), flags: c_int) -> Result<(), i32> {
    let mut antes: libc::sigaction = unsafe { std::mem::zeroed() };
    if unsafe { libc::sigaction(sinal, std::ptr::null(), &mut antes) } == 0 && antes.sa_sigaction == libc::SIG_IGN {
        return Ok(());
    }
    let mut sa: libc::sigaction = unsafe { std::mem::zeroed() };
    sa.sa_sigaction = h as usize;
    sa.sa_flags = flags;
    if unsafe { libc::sigaction(sinal, &sa, std::ptr::null_mut()) } != 0 { Err(errno()) } else { Ok(()) }
}

extern "C" fn sigterm(sinal: c_int) {
    // Mata os filhos e morre do próprio sinal (signal e raise são seguros em handler; o sinal fica
    // bloqueado até o handler voltar e então age com o padrão).
    matar_filhos();
    unsafe {
        libc::signal(sinal, libc::SIG_DFL);
        libc::raise(sinal);
    }
}

/// SIGTERM e SIGHUP matam os filhos antes de encerrar (o app fechando o sdtally, o Terminal
/// fechado). O sdc-coleta troca os dois por `ao_fechar`.
pub fn ao_terminar() -> Result<(), i32> {
    instalar(libc::SIGTERM, sigterm, 0)?;
    instalar(libc::SIGHUP, sigterm, 0)
}

/// SIGINT seta `INTERROMPIDO` uma vez; SA_RESETHAND devolve o padrão, então o segundo Ctrl-C mata.
pub fn ao_interromper() -> Result<(), i32> {
    instalar(libc::SIGINT, sigint, libc::SA_RESETHAND)
}

/// Só no sdc-coleta (horas rodando): SIGTERM (logout, desligar, `kill`) e SIGHUP (fechar o Terminal)
/// também param com o que já foi medido. Os dois ficam instalados: o sudo repassa ao comando o
/// sinal que ele também recebeu direto, e o segundo não pode matar antes da gravação. Para forçar
/// sobram o segundo Ctrl-C e o kill -9.
pub fn ao_fechar() -> Result<(), i32> {
    instalar(libc::SIGTERM, sigint, 0)?;
    instalar(libc::SIGHUP, sighup, 0)
}

/// Pasta do relatório aberta uma vez: as gravações seguintes (horas depois, como root) são relativas
/// ao fd, então trocar a pasta ou um pai por symlink no meio não desvia nada. Nomes sem `/`.
pub struct PastaSaida(std::os::fd::OwnedFd);

/// Identidade de um arquivo: (dev, inode, tamanho, mtime s, mtime ns). Muda se alguém editar,
/// trocar ou apagar.
pub type Marca = (i32, u64, i64, i64, i64);

impl PastaSaida {
    pub fn abrir(p: &Path) -> Result<PastaSaida, i32> {
        let c = cpath(p)?;
        let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
        if fd < 0 { Err(errno()) } else { Ok(PastaSaida(unsafe { std::os::fd::FromRawFd::from_raw_fd(fd) })) }
    }

    pub fn dono(&self) -> Result<u32, i32> {
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(std::os::fd::AsRawFd::as_raw_fd(&self.0), &mut st) } != 0 { Err(errno()) } else { Ok(st.st_uid) }
    }

    /// Arquivo novo (O_EXCL|O_NOFOLLOW): recusa arquivo ou symlink já existente.
    pub fn criar(&self, nome: &str) -> Result<std::fs::File, i32> {
        let c = cpath(nome)?;
        let fl = libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        let fd = unsafe { libc::openat(std::os::fd::AsRawFd::as_raw_fd(&self.0), c.as_ptr(), fl, 0o600 as libc::c_uint) };
        if fd < 0 { Err(errno()) } else { Ok(unsafe { std::os::fd::FromRawFd::from_raw_fd(fd) }) }
    }

    pub fn renomear(&self, de: &str, para: &str) -> Result<(), i32> {
        let (a, b) = (cpath(de)?, cpath(para)?);
        let d = std::os::fd::AsRawFd::as_raw_fd(&self.0);
        if unsafe { libc::renameat(d, a.as_ptr(), d, b.as_ptr()) } != 0 { Err(errno()) } else { Ok(()) }
    }

    /// Só para o temporário do próprio programa.
    pub fn remover(&self, nome: &str) {
        if let Ok(c) = cpath(nome) {
            unsafe { libc::unlinkat(std::os::fd::AsRawFd::as_raw_fd(&self.0), c.as_ptr(), 0) };
        }
    }

    pub fn marca(&self, nome: &str) -> Option<Marca> {
        let c = cpath(nome).ok()?;
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        let r = unsafe { libc::fstatat(std::os::fd::AsRawFd::as_raw_fd(&self.0), c.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) };
        (r == 0).then_some((st.st_dev, st.st_ino, st.st_size, st.st_mtime, st.st_mtime_nsec))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn processos_do_painel_pelo_caminho() {
        use super::e_processo_do_painel as ok;
        assert!(ok("/System/Library/ExtensionKit/Extensions/Storage.appex/Contents/MacOS/Storage"));
        assert!(ok("/System/Library/PrivateFrameworks/StorageManagement.framework/PlugIns/StorageManagementService"));
        // Mesmo nome de processo fora do lugar do sistema não conta.
        assert!(!ok("/Applications/Storage"));
        assert!(!ok("/System/Library/ExtensionKit/Extensions/StorageSettingsIntentsExtension.appex/Contents/MacOS/StorageSettingsIntentsExtension"));
        assert!(!ok("/usr/libexec/storagekitd"));
    }

    #[test]
    fn montagens_inclui_raiz_e_data() {
        let m = super::montagens().expect("getfsstat");
        assert!(m.iter().any(|p| p == "/"));
        assert!(m.iter().any(|p| p == "/System/Volumes/Data"));
    }

    #[test]
    fn boot_time_formato() {
        let b = super::boot_time().expect("kern.boottime");
        let x = b.as_bytes();
        assert!(x.len() == 16 && x.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b' ',
            13 => *c == b':',
            _ => c.is_ascii_digit(),
        }), "{b}");
        let (agora, _) = super::now_local();
        assert!(b.as_str() <= &agora.replace('T', " ")[..16], "boot no futuro: {b}");
    }

    #[test]
    fn plist_so_chaves_fixas() {
        use super::{chave_tm_permitida as ok, plist_chave};
        for (k, f) in [("Destinations", "raw"), ("Destinations.0.ReferenceLocalSnapshotDate", "raw"), ("Destinations.12.StableLocalSnapshotDate", "raw"),
                       ("Destinations.0.RESULT", "raw"), ("Destinations.0.SnapshotDates", "xml1"), ("Destinations.1.AttemptDates", "xml1")] {
            assert!(ok(k, f), "{k} {f}");
        }
        for (k, f) in [("Destinations", "xml1"), ("Destinations.0.NetworkURL", "raw"), ("Destinations.0.LastKnownVolumeName", "raw"),
                       ("Destinations.0.DestinationID", "raw"), ("Destinations.0.DestinationUUIDs", "xml1"), ("Destinations.0", "xml1"),
                       ("Destinations.x.RESULT", "raw"), ("Destinations..RESULT", "raw"), ("Destinations.0.RESULT.0", "raw"),
                       ("Destinations.0.SnapshotDates", "raw"), ("Destinations.0.RESULT", "json"), ("AutoBackup", "raw"), ("", "raw")] {
            assert!(!ok(k, f), "{k} {f}");
            // Recusa antes de rodar o plutil, e o erro não traz nada do arquivo.
            assert_eq!(plist_chave("/dev/null", k, f), Err("plutil: chave fora da lista fixa".into()));
        }
    }

    #[test]
    fn swap_usado_cabe_no_total() {
        let (total, usado) = super::swap_usage().expect("vm.swapusage");
        assert!(usado <= total);
    }
}

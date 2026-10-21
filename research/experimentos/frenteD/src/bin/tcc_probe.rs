//! Tenta ler caminhos protegidos e registra o errno de cada camada.
//! Só leitura: lstat + open(O_RDONLY) + um readdir. Nunca lê conteúdo de arquivo.
//!
//! Uso:
//!   tcc_probe                 lista padrão completa (herda o TCC do processo responsável)
//!   tcc_probe --safe          só caminhos que não disparam prompt (FDA/root/volumes)
//!   tcc_probe --disclaim      re-executa a si mesmo com a responsabilidade renunciada
//!                             (o binário vira o próprio "responsible process") e usa --safe
//!   tcc_probe <caminho>...    caminhos avulsos
//!
//! EPERM (1)  = MAC/TCC/sandbox/SIP/data vault (Quinn, "On File System Permissions")
//! EACCES (13) = permissão BSD ou ACL

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;

unsafe extern "C" {
    // Privadas, exportadas pela libSystem (usadas por Terminal/Claude "disclaimer").
    fn responsibility_get_pid_responsible_for_pid(pid: libc::pid_t) -> libc::pid_t;
    fn responsibility_spawnattrs_setdisclaim(attrs: *mut libc::posix_spawnattr_t, disclaim: libc::c_int) -> libc::c_int;
}

fn pid_path(pid: libc::pid_t) -> String {
    let mut buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let n = unsafe { libc::proc_pidpath(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
    if n <= 0 { return "?".into() }
    String::from_utf8_lossy(&buf[..n as usize]).into_owned()
}

fn errname(e: i32) -> &'static str {
    match e {
        0 => "OK",
        libc::EPERM => "EPERM",
        libc::EACCES => "EACCES",
        libc::ENOENT => "ENOENT",
        libc::ENOTDIR => "ENOTDIR",
        libc::EISDIR => "EISDIR",
        _ => "OUTRO",
    }
}

fn errno() -> i32 { std::io::Error::last_os_error().raw_os_error().unwrap_or(-1) }

/// (lstat, open, readdir) — cada um OK ou errno.
fn probe(path: &str) -> (i32, i32, i32) {
    let c = CString::new(path).unwrap();
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let e_stat = if unsafe { libc::lstat(c.as_ptr(), &mut st) } == 0 { 0 } else { errno() };
    let is_dir = e_stat == 0 && (st.st_mode & libc::S_IFMT) == libc::S_IFDIR;
    // O_NONBLOCK: não travar em FIFO; O_NOFOLLOW implícito não — o caminho é o que se mede.
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK) };
    let e_open = if fd >= 0 { unsafe { libc::close(fd) }; 0 } else { errno() };
    let e_dir = if !is_dir { -2 } else {
        let d = unsafe { libc::opendir(c.as_ptr()) };
        if d.is_null() { errno() } else {
            unsafe { libc::__error().write(0) };
            let ent = unsafe { libc::readdir(d) };
            let e = if ent.is_null() { errno() } else { 0 };
            unsafe { libc::closedir(d) };
            e
        }
    };
    (e_stat, e_open, e_dir)
}

fn home(p: &str) -> String { format!("{}/{}", std::env::var("HOME").unwrap(), p) }

/// Caminhos em que uma negação é silenciosa (FDA, root, SIP): não há prompt de TCC.
fn safe_list() -> Vec<(String, &'static str)> {
    let mut v: Vec<(String, &'static str)> = vec![];
    for p in ["Library/Mail", "Library/Messages", "Library/Safari", "Library/Cookies", "Library/HomeKit",
        "Library/Suggestions", "Library/Metadata/CoreSpotlight", "Library/Application Support/AddressBook",
        "Library/Application Support/CallHistoryDB", "Library/Application Support/Knowledge",
        "Library/Application Support/FaceTime", "Library/Application Support/com.apple.sharedfilelist",
        "Library/Application Support/com.apple.TCC", "Library/Biome", "Library/IdentityServices",
        "Library/Accounts", "Library/Trial", "Library/DuetExpertCenter", "Library/Sharing",
        "Library/PersonalizationPortrait", "Library/Containers/com.apple.Safari",
        "Library/Containers/com.apple.mail", "Library/Containers/com.apple.stocks",
        "Library/Group Containers/group.com.apple.notes", ".Trash"] {
        v.push((home(p), "FDA"));
    }
    for p in ["/Library/Application Support/com.apple.TCC/TCC.db", "/Library/Application Support/com.apple.TCC"] {
        v.push((p.into(), "FDA"));
    }
    for p in ["/private/var/root", "/private/var/db/sudo", "/private/var/db/diagnostics", "/private/var/db/uuidtext",
        "/private/var/db/dslocal", "/private/var/db/SystemPolicy", "/private/var/folders/zz", "/private/var/vm",
        "/private/var/log", "/private/var/log/asl", "/private/var/audit", "/private/var/networkd",
        "/private/var/protected", "/private/var/containers", "/Library/Logs/DiagnosticReports",
        "/System/Volumes/Data/.Spotlight-V100", "/System/Volumes/Data/.fseventsd",
        "/System/Volumes/Data/.DocumentRevisions-V100", "/System/Volumes/Data/.TemporaryItems",
        "/System/Volumes/Preboot", "/System/Volumes/Update", "/System/Volumes/VM", "/System/Volumes/xarts",
        "/System/Volumes/iSCPreboot", "/System/Volumes/Hardware", "/Volumes/Recovery",
        "/Volumes/com.apple.TimeMachine.localsnapshots"] {
        v.push((p.into(), "root/SIP"));
    }
    v
}

/// Caminhos que podem disparar prompt de TCC num processo SEM FDA.
fn prompting_list() -> Vec<(String, &'static str)> {
    let mut v: Vec<(String, &'static str)> = vec![];
    for p in ["Desktop", "Documents", "Downloads", "Library/Mobile Documents"] {
        v.push((home(p), "Files&Folders"));
    }
    // App Data (macOS 14 Containers, macOS 15 Group Containers) e Golden Gate (macOS 27).
    let lib = home("Library/Containers");
    if let Ok(rd) = std::fs::read_dir(&lib) {
        for e in rd.flatten().take(400) {
            let n = e.file_name().to_string_lossy().into_owned();
            if !n.starts_with("com.apple.") && !n.starts_with('.') { v.push((format!("{lib}/{n}"), "AppData")); }
        }
    }
    let gc = home("Library/Group Containers");
    if let Ok(rd) = std::fs::read_dir(&gc) {
        for e in rd.flatten().take(400) {
            let n = e.file_name().to_string_lossy().into_owned();
            if !n.contains("com.apple") && !n.starts_with('.') { v.push((format!("{gc}/{n}"), "AppGroup")); }
        }
    }
    for p in ["discord", "Google/Chrome", "Firefox", "BraveSoftware", "Microsoft Edge", "Ledger Live", "Exodus"] {
        v.push((home(&format!("Library/Application Support/{p}")), "GoldenGate"));
    }
    v
}

/// Filho herda os argumentos restantes (padrão: --safe). O filho se mata após 30 s:
/// na 1ª rodada o acesso a "Photos Library.photoslibrary" travou >2 min sem FDA.
fn spawn_disclaimed(rest: &[String]) -> ! {
    let exe = std::env::current_exe().unwrap();
    let c_exe = CString::new(exe.as_os_str().as_bytes()).unwrap();
    let mut owned = vec![CString::new("--child").unwrap()];
    if rest.is_empty() { owned.push(CString::new("--safe").unwrap()) }
    owned.extend(rest.iter().map(|a| CString::new(a.as_str()).unwrap()));
    let mut argv: Vec<*mut libc::c_char> = vec![c_exe.as_ptr() as *mut _];
    argv.extend(owned.iter().map(|c| c.as_ptr() as *mut libc::c_char));
    argv.push(std::ptr::null_mut());
    unsafe {
        let mut attr: libc::posix_spawnattr_t = std::mem::zeroed();
        libc::posix_spawnattr_init(&mut attr);
        let r = responsibility_spawnattrs_setdisclaim(&mut attr, 1);
        assert_eq!(r, 0, "setdisclaim falhou");
        let mut pid = 0;
        unsafe extern "C" { static environ: *const *mut libc::c_char; }
        let r = libc::posix_spawn(&mut pid, c_exe.as_ptr(), std::ptr::null(), &attr, argv.as_ptr(), environ);
        assert_eq!(r, 0, "posix_spawn falhou");
        let mut status = 0;
        libc::waitpid(pid, &mut status, 0);
        std::process::exit(libc::WEXITSTATUS(status));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--disclaim") { spawn_disclaimed(&args[1..]) }
    let args: Vec<String> = if args.first().map(String::as_str) == Some("--child") {
        unsafe { libc::alarm(30) };
        args[1..].to_vec()
    } else { args };

    let me = unsafe { libc::getpid() };
    let resp = unsafe { responsibility_get_pid_responsible_for_pid(me) };
    println!("# pid {me} uid {} responsável: pid {resp} {}", unsafe { libc::getuid() }, pid_path(resp));

    let list: Vec<(String, &str)> = match args.first().map(String::as_str) {
        Some("--safe") => safe_list(),
        Some(_) => args.iter().map(|a| (a.clone(), "avulso")).collect(),
        None => { let mut v = safe_list(); v.extend(prompting_list()); v }
    };
    println!("classe\tlstat\topen\treaddir\tcaminho");
    let mut counts = std::collections::BTreeMap::<(String, &str), u32>::new();
    for (p, class) in &list {
        let (s, o, d) = probe(p);
        let dn = if d == -2 { "-" } else { errname(d) };
        println!("{class}\t{}\t{}\t{dn}\t{p}", errname(s), errname(o));
        let worst = if d > 0 { d } else if o != 0 { o } else { s };
        *counts.entry((class.to_string(), errname(worst.max(0)))).or_default() += 1;
    }
    println!("# resumo (classe, pior resultado): {counts:?}");
}

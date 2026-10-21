//! FFI à mão, só leitura: libobjc (objc_msgSend com transmute por assinatura), Foundation e
//! frameworks privados por dlopen/dlsym (só funções de tamanho), libsqlite3 do sistema (mode=ro,
//! no lugar, sem copiar), CommonCrypto e zlib (biblioteca da Música).

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub type Id = *mut c_void;
pub type Sel = *const c_void;
pub const NIL: Id = std::ptr::null_mut();

#[link(name = "objc")]
unsafe extern "C" {
    pub fn objc_getClass(n: *const c_char) -> Id;
    pub fn sel_registerName(n: *const c_char) -> Sel;
    pub fn objc_msgSend();
    fn class_getClassMethod(c: Id, s: Sel) -> *mut c_void;
    fn method_getTypeEncoding(m: *mut c_void) -> *const c_char;
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(p: *mut c_void);
}

/// `msg!(Ret; obj, "sel:com:", a => T, b => U)`: objc_msgSend com a assinatura dada (arm64: sem stret).
macro_rules! msg {
    ($r:ty; $obj:expr, $sel:literal $(, $a:expr => $t:ty)*) => {{
        #[allow(unused_unsafe)]
        unsafe {
            let f: unsafe extern "C" fn($crate::ffi::Id, $crate::ffi::Sel $(, $t)*) -> $r =
                std::mem::transmute($crate::ffi::objc_msgSend as unsafe extern "C" fn());
            f($obj, $crate::ffi::sel_registerName(concat!($sel, "\0").as_ptr() as *const std::ffi::c_char) $(, $a)*)
        }
    }};
}
pub(crate) use msg;

pub struct Pool(*mut c_void);
impl Pool {
    pub fn new() -> Pool {
        Pool(unsafe { objc_autoreleasePoolPush() })
    }
}
impl Drop for Pool {
    fn drop(&mut self) {
        unsafe { objc_autoreleasePoolPop(self.0) }
    }
}

pub fn dlopen(p: &str) -> Option<*mut c_void> {
    let c = CString::new(p).ok()?;
    let h = unsafe { libc::dlopen(c.as_ptr(), libc::RTLD_LAZY | libc::RTLD_GLOBAL) };
    (!h.is_null()).then_some(h)
}

pub fn dlsym(h: *mut c_void, n: &str) -> Option<*mut c_void> {
    let c = CString::new(n).ok()?;
    let s = unsafe { libc::dlsym(h, c.as_ptr()) };
    (!s.is_null()).then_some(s)
}

/// Foundation e CoreServices carregados uma vez (classes e constantes via dlsym).
pub fn init() -> Result<(), String> {
    static OK: OnceLock<bool> = OnceLock::new();
    let ok = *OK.get_or_init(|| {
        dlopen("/System/Library/Frameworks/Foundation.framework/Foundation").is_some()
            && dlopen("/System/Library/Frameworks/CoreServices.framework/CoreServices").is_some()
    });
    if ok { Ok(()) } else { Err("dlopen Foundation/CoreServices falhou".into()) }
}

/// Codificação de tipos de um método de classe (confere a aridade antes de chamar por transmute).
pub fn enc_classe(c: &str, sel: &str) -> Option<String> {
    let s = CString::new(sel).ok()?;
    let k = cls(c);
    if k.is_null() {
        return None;
    }
    let m = unsafe { class_getClassMethod(k, sel_registerName(s.as_ptr())) };
    if m.is_null() {
        return None;
    }
    let e = unsafe { method_getTypeEncoding(m) };
    (!e.is_null()).then(|| unsafe { std::ffi::CStr::from_ptr(e) }.to_string_lossy().into_owned())
}

pub fn cls(n: &str) -> Id {
    let c = CString::new(n).unwrap_or_default();
    unsafe { objc_getClass(c.as_ptr()) }
}

/// Constante exportada (NSString*/CFStringRef) lida pelo nome.
pub fn konst(n: &str) -> Id {
    dlsym(libc::RTLD_DEFAULT, n).map_or(NIL, |p| unsafe { *(p as *const Id) })
}

pub fn ns(s: &str) -> Id {
    let c = CString::new(s).unwrap_or_default();
    msg!(Id; cls("NSString"), "stringWithUTF8String:", c.as_ptr() => *const c_char)
}

pub fn rs(o: Id) -> Option<String> {
    if o.is_null() {
        return None;
    }
    let p = msg!(*const c_char; o, "UTF8String");
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
}

pub fn url(p: &str) -> Id {
    msg!(Id; cls("NSURL"), "fileURLWithPath:", ns(p) => Id)
}

/// -[NSURL getResourceValue:forKey:error:]; nil se a chave não veio.
pub fn res(u: Id, key: &str) -> Id {
    let mut v: Id = NIL;
    msg!(bool; u, "getResourceValue:forKey:error:", &mut v => *mut Id, konst(key) => Id, std::ptr::null_mut::<Id>() => *mut Id);
    v
}

pub fn i64v(o: Id) -> Option<i64> {
    (!o.is_null()).then(|| msg!(i64; o, "longLongValue"))
}

pub fn get(d: Id, k: &str) -> Id {
    if d.is_null() { NIL } else { msg!(Id; d, "objectForKey:", ns(k) => Id) }
}

pub fn itens(a: Id) -> Vec<Id> {
    if a.is_null() {
        return Vec::new();
    }
    let n = msg!(usize; a, "count");
    (0..n).map(|i| msg!(Id; a, "objectAtIndex:", i => usize)).collect()
}

pub fn nsarray(v: &[Id]) -> Id {
    msg!(Id; cls("NSArray"), "arrayWithObjects:count:", v.as_ptr() => *const Id, v.len() => usize)
}

pub fn iguais(a: Id, b: Id) -> bool {
    !a.is_null() && !b.is_null() && msg!(bool; a, "isEqual:", b => Id)
}

/// plist (bytes) → objeto Foundation.
pub fn plist(b: &[u8]) -> Id {
    let d = msg!(Id; cls("NSData"), "dataWithBytes:length:", b.as_ptr() => *const u8, b.len() => usize);
    msg!(Id; cls("NSPropertyListSerialization"), "propertyListWithData:options:format:error:",
         d => Id, 0usize => usize, std::ptr::null_mut::<usize>() => *mut usize, std::ptr::null_mut::<Id>() => *mut Id)
}

// ---- tamanho: STMSizeDirectoryWithExcludedChildURLs (StorageManagement, privado) ----

type SizeFn = unsafe extern "C" fn(Id, Id) -> u64;
const STM: &str = "/System/Library/PrivateFrameworks/StorageManagement.framework/Versions/A/StorageManagement";

pub fn stm() -> Result<SizeFn, String> {
    static F: OnceLock<Option<usize>> = OnceLock::new();
    let f = F.get_or_init(|| dlopen(STM).and_then(|h| dlsym(h, "STMSizeDirectoryWithExcludedChildURLs")).map(|p| p as usize));
    f.map(|p| unsafe { std::mem::transmute::<usize, SizeFn>(p) })
        .ok_or_else(|| "StorageManagement: STMSizeDirectoryWithExcludedChildURLs ausente".into())
}

pub fn lstat(p: &str) -> Option<libc::stat> {
    let c = CString::new(p).ok()?;
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    (unsafe { libc::lstat(c.as_ptr(), &mut st) } == 0).then_some(st)
}

pub fn stat(p: &str) -> Option<libc::stat> {
    let c = CString::new(p).ok()?;
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    (unsafe { libc::stat(c.as_ptr(), &mut st) } == 0).then_some(st)
}

pub fn existe(p: &str) -> bool {
    lstat(p).is_some()
}

/// Tamanho como a extensão da Apple: pasta = STMSize (NodeSizingV2) sem os filhos excluídos;
/// arquivo = NSURLTotalFileAllocatedSizeKey. None = caminho não existe (não soma).
pub fn tam(p: &str, excl: &[String]) -> Option<i64> {
    let st = lstat(p)?;
    let _pool = Pool::new();
    let u = url(p);
    if st.st_mode & libc::S_IFMT != libc::S_IFDIR {
        return Some(i64v(res(u, "NSURLTotalFileAllocatedSizeKey")).unwrap_or(0));
    }
    let f = stm().ok()?;
    let ex: Vec<Id> = excl.iter().map(|e| url(e)).collect();
    Some(unsafe { f(u, nsarray(&ex)) } as i64)
}

// ---- SQLite do sistema, no lugar e só leitura ----

#[link(name = "sqlite3")]
unsafe extern "C" {
    fn sqlite3_open_v2(f: *const c_char, db: *mut *mut c_void, flags: c_int, vfs: *const c_char) -> c_int;
    fn sqlite3_busy_timeout(db: *mut c_void, ms: c_int) -> c_int;
    fn sqlite3_prepare_v2(db: *mut c_void, sql: *const c_char, n: c_int, st: *mut *mut c_void, tail: *mut *const c_char) -> c_int;
    fn sqlite3_step(st: *mut c_void) -> c_int;
    fn sqlite3_column_count(st: *mut c_void) -> c_int;
    fn sqlite3_column_type(st: *mut c_void, i: c_int) -> c_int;
    fn sqlite3_column_int64(st: *mut c_void, i: c_int) -> i64;
    fn sqlite3_column_blob(st: *mut c_void, i: c_int) -> *const u8;
    fn sqlite3_column_bytes(st: *mut c_void, i: c_int) -> c_int;
    fn sqlite3_finalize(st: *mut c_void) -> c_int;
    fn sqlite3_close(db: *mut c_void) -> c_int;
    fn sqlite3_errmsg(db: *mut c_void) -> *const c_char;
}

#[derive(Debug, Clone)]
pub enum V {
    I(i64),
    T(String),
    N,
}

impl V {
    pub fn i(&self) -> Option<i64> {
        if let V::I(v) = self { Some(*v) } else { None }
    }
    pub fn t(&self) -> String {
        match self {
            V::T(s) => s.clone(),
            V::I(v) => v.to_string(),
            V::N => String::new(),
        }
    }
}

/// Caminho → URI `file:` (mode=ro): o banco é lido no lugar, sem cópia.
fn uri(p: &str) -> String {
    let mut s = String::from("file:");
    for b in p.bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s + "?mode=ro"
}

pub fn sql(path: &str, q: &str) -> Result<Vec<Vec<V>>, String> {
    let u = CString::new(uri(path)).map_err(|e| e.to_string())?;
    let qc = CString::new(q).map_err(|e| e.to_string())?;
    let mut db = std::ptr::null_mut();
    // SQLITE_OPEN_READONLY | SQLITE_OPEN_URI
    let rc = unsafe { sqlite3_open_v2(u.as_ptr(), &mut db, 0x1 | 0x40, std::ptr::null()) };
    let erro = |db| unsafe { CStr::from_ptr(sqlite3_errmsg(db)) }.to_string_lossy().into_owned();
    if rc != 0 {
        let e = erro(db);
        unsafe { sqlite3_close(db) };
        return Err(format!("sqlite open: {e}"));
    }
    unsafe { sqlite3_busy_timeout(db, 5000) };
    let mut st = std::ptr::null_mut();
    if unsafe { sqlite3_prepare_v2(db, qc.as_ptr(), -1, &mut st, std::ptr::null_mut()) } != 0 {
        let e = erro(db);
        unsafe { sqlite3_close(db) };
        return Err(format!("sqlite: {e}"));
    }
    let mut out = Vec::new();
    let r = loop {
        match unsafe { sqlite3_step(st) } {
            100 => {
                let n = unsafe { sqlite3_column_count(st) };
                out.push((0..n).map(|i| match unsafe { sqlite3_column_type(st, i) } {
                    1 | 2 => V::I(unsafe { sqlite3_column_int64(st, i) }),
                    5 => V::N,
                    _ => {
                        let (p, l) = unsafe { (sqlite3_column_blob(st, i), sqlite3_column_bytes(st, i)) };
                        let b = if p.is_null() { &[][..] } else { unsafe { std::slice::from_raw_parts(p, l as usize) } };
                        V::T(String::from_utf8_lossy(b).into_owned())
                    }
                }).collect());
            }
            101 => break Ok(out),
            _ => break Err(format!("sqlite step: {}", erro(db))),
        }
    };
    unsafe {
        sqlite3_finalize(st);
        sqlite3_close(db);
    }
    r
}

// ---- CommonCrypto (libSystem) e zlib ----

unsafe extern "C" {
    fn CCCrypt(op: u32, alg: u32, opt: u32, key: *const u8, kl: usize, iv: *const u8, i: *const u8, il: usize,
               o: *mut u8, ol: usize, moved: *mut usize) -> i32;
}
#[link(name = "z")]
unsafe extern "C" {
    fn uncompress(d: *mut u8, dl: *mut libc::c_ulong, s: *const u8, sl: libc::c_ulong) -> c_int;
}

pub fn aes128_ecb_dec(key: &[u8; 16], data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = vec![0u8; data.len() + 16];
    let mut moved = 0usize;
    // kCCDecrypt=1, kCCAlgorithmAES128=0, kCCOptionECBMode=2 (sem padding)
    let r = unsafe { CCCrypt(1, 0, 2, key.as_ptr(), 16, std::ptr::null(), data.as_ptr(), data.len(), out.as_mut_ptr(), out.len(), &mut moved) };
    if r != 0 {
        return Err(format!("CCCrypt {r}"));
    }
    out.truncate(moved);
    Ok(out)
}

/// zlib com cabeçalho; dados depois do fim do fluxo são ignorados (igual ao zlib.decompress).
pub fn inflate(s: &[u8]) -> Result<Vec<u8>, String> {
    let mut cap = s.len() * 8 + 4096;
    loop {
        let mut out = vec![0u8; cap];
        let mut dl = cap as libc::c_ulong;
        match unsafe { uncompress(out.as_mut_ptr(), &mut dl, s.as_ptr(), s.len() as libc::c_ulong) } {
            0 => {
                out.truncate(dl as usize);
                return Ok(out);
            }
            -5 if cap < 1 << 32 => cap *= 4, // Z_BUF_ERROR: saída pequena
            e => return Err(format!("zlib {e}")),
        }
    }
}

// ---- sistema ----

unsafe extern "C" {
    fn setiopolicy_np(t: c_int, s: c_int, p: c_int) -> c_int;
    fn fsctl(path: *const c_char, req: libc::c_ulong, data: *mut c_void, opt: libc::c_uint) -> c_int;
}

/// Síntese §4 (1 e 2b): sem materializar dataless e sem gravar atime, antes de qualquer thread.
pub fn politicas() -> Result<(), String> {
    // IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES=3, IOPOL_TYPE_VFS_ATIME_UPDATES=2, PROCESS=0, OFF=1
    let (a, b) = unsafe { (setiopolicy_np(3, 0, 1), setiopolicy_np(2, 0, 1)) };
    if a == 0 && b == 0 { Ok(()) } else { Err(format!("setiopolicy_np {a} {b}")) }
}

/// dir-stats APFS (0xC1104A71) só com buffer zerado ("get"; buf+4 nunca é escrito, síntese §4.6).
pub fn dirstats(p: &str) -> Option<i64> {
    let c = CString::new(p).ok()?;
    let mut b = [0u8; 0x110];
    if unsafe { fsctl(c.as_ptr(), 0xC110_4A71, b.as_mut_ptr() as *mut c_void, 0) } != 0 {
        return None;
    }
    Some(u64::from_le_bytes(b[0x40..0x48].try_into().ok()?) as i64)
}

pub fn getxattr(p: &str, n: &str) -> Option<Vec<u8>> {
    let (c, k) = (CString::new(p).ok()?, CString::new(n).ok()?);
    let mut b = [0u8; 64];
    let r = unsafe { libc::getxattr(c.as_ptr(), k.as_ptr(), b.as_mut_ptr() as *mut c_void, 64, 0, libc::XATTR_NOFOLLOW) };
    (r > 0).then(|| b[..r as usize].to_vec())
}

pub fn sysctl_str(n: &str) -> Option<String> {
    let c = CString::new(n).ok()?;
    let mut b = [0u8; 64];
    let mut l = b.len();
    (unsafe { libc::sysctlbyname(c.as_ptr(), b.as_mut_ptr() as *mut c_void, &mut l, std::ptr::null_mut(), 0) } == 0)
        .then(|| CStr::from_bytes_until_nul(&b).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
}

/// Comando externo com timeout (síntese §4.5). Ok(stdout) só com status 0.
pub fn run(cmd: &str, args: &[&str], secs: u64) -> Result<Vec<u8>, String> {
    let mut ch = Command::new(cmd).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().map_err(|e| format!("{cmd}: {e}"))?;
    let mut o = ch.stdout.take();
    let leitor = std::thread::spawn(move || {
        let mut s = Vec::new();
        if let Some(o) = o.as_mut() {
            let _ = o.read_to_end(&mut s);
        }
        s
    });
    let t0 = Instant::now();
    loop {
        match ch.try_wait() {
            Ok(Some(s)) if s.success() => return Ok(leitor.join().unwrap_or_default()),
            Ok(Some(_)) => return Err(format!("{cmd}: saiu com erro")),
            Ok(None) if t0.elapsed() > Duration::from_secs(secs) => {
                let _ = ch.kill();
                let _ = ch.wait();
                return Err(format!("{cmd}: timeout de {secs} s"));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(format!("{cmd}: {e}")),
        }
    }
}

// ---- bloco global (ABI de blocos) para callbacks síncronos ----

#[repr(C)]
pub struct Desc {
    pub reservado: usize,
    pub tamanho: usize,
}

#[repr(C)]
pub struct Bloco {
    isa: *const c_void,
    flags: i32,
    reservado: i32,
    invoke: *const c_void,
    desc: *const Desc,
}

unsafe extern "C" {
    static _NSConcreteGlobalBlock: [u8; 0];
}

pub static DESC: Desc = Desc { reservado: 0, tamanho: std::mem::size_of::<Bloco>() };

/// Bloco sem captura (BLOCK_IS_GLOBAL): Block_copy devolve o mesmo ponteiro. Só para chamadas
/// que invocam o bloco antes de retornar; o estado vai por static.
pub fn bloco(invoke: *const c_void) -> Bloco {
    Bloco { isa: (&raw const _NSConcreteGlobalBlock) as *const c_void, flags: 1 << 28, reservado: 0, invoke, desc: &DESC }
}

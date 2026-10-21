//! FSEvents (CoreServices): aviso de pasta mudada, no nível de pasta (sem evento por arquivo).
//! Só escuta: não lê nem grava nada no disco.

use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::mpsc::Sender;

type CF = *const c_void;
const UTF8: u32 = 0x0800_0100;
/// kFSEventStreamEventIdSinceNow.
const DESDE_AGORA: u64 = u64::MAX;
/// kFSEventStreamCreateFlagWatchRoot: avisa se a própria raiz vigiada mudar.
const VIGIAR_RAIZ: u32 = 0x4;

/// Flags de cada aviso (FSEventStreamEventFlags).
pub const RELER_ABAIXO: u32 = 0x1;
pub const PERDIDOS_USUARIO: u32 = 0x2;
pub const PERDIDOS_KERNEL: u32 = 0x4;
pub const RAIZ_MUDOU: u32 = 0x20;

/// (caminho da pasta, flags) de um lote de avisos.
pub type Lote = Vec<(String, u32)>;

#[repr(C)]
struct Contexto {
    version: isize,
    info: *mut c_void,
    retain: *const c_void,
    release: *const c_void,
    copy_description: *const c_void,
}

type Callback = extern "C" fn(*const c_void, *mut c_void, usize, *mut c_void, *const u32, *const u64);

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn FSEventStreamCreate(a: CF, cb: Callback, ctx: *const Contexto, paths: CF, desde: u64, latencia: f64, flags: u32) -> *mut c_void;
    fn FSEventStreamSetDispatchQueue(s: *mut c_void, q: *mut c_void);
    fn FSEventStreamStart(s: *mut c_void) -> bool;
    fn FSEventStreamStop(s: *mut c_void);
    fn FSEventStreamInvalidate(s: *mut c_void);
    fn FSEventStreamRelease(s: *mut c_void);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFTypeArrayCallBacks: [u8; 40];
    fn CFStringCreateWithCString(a: CF, s: *const c_char, enc: u32) -> CF;
    fn CFArrayCreate(a: CF, v: *const CF, n: isize, cb: *const c_void) -> CF;
    fn CFRelease(x: CF);
}

unsafe extern "C" {
    fn dispatch_queue_create(label: *const c_char, attr: *const c_void) -> *mut c_void;
    fn dispatch_release(o: *mut c_void);
}

extern "C" fn aviso(_s: *const c_void, info: *mut c_void, n: usize, paths: *mut c_void, flags: *const u32, _ids: *const u64) {
    // Sem kFSEventStreamCreateFlagUseCFTypes os caminhos chegam como `char **`.
    let tx = unsafe { &*(info as *const Sender<Lote>) };
    let ps = paths as *const *const c_char;
    let lote = (0..n).map(|i| unsafe { (CStr::from_ptr(*ps.add(i)).to_string_lossy().into_owned(), *flags.add(i)) }).collect();
    let _ = tx.send(lote);
}

/// Escuta ativa; para ao sair de escopo.
pub struct Vigia {
    s: *mut c_void,
    q: *mut c_void,
    tx: *mut Sender<Lote>,
}

impl Drop for Vigia {
    fn drop(&mut self) {
        unsafe {
            FSEventStreamStop(self.s);
            FSEventStreamInvalidate(self.s);
            FSEventStreamRelease(self.s);
            dispatch_release(self.q);
            drop(Box::from_raw(self.tx));
        }
    }
}

/// Avisa por `tx` cada lote de pastas mudadas abaixo de `caminho`, a partir de agora, juntando os
/// avisos de `latencia` segundos.
pub fn vigiar(caminho: &str, latencia: f64, tx: Sender<Lote>) -> Result<Vigia, String> {
    let c = CString::new(caminho).map_err(|_| "caminho com NUL".to_string())?;
    let tx = Box::into_raw(Box::new(tx));
    let ctx = Contexto { version: 0, info: tx as *mut c_void, retain: std::ptr::null(), release: std::ptr::null(), copy_description: std::ptr::null() };
    unsafe {
        let s = CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), UTF8);
        let arr = CFArrayCreate(std::ptr::null(), &s, 1, (&raw const kCFTypeArrayCallBacks) as *const c_void);
        CFRelease(s);
        let st = FSEventStreamCreate(std::ptr::null(), aviso, &ctx, arr, DESDE_AGORA, latencia, VIGIAR_RAIZ);
        CFRelease(arr);
        if st.is_null() {
            drop(Box::from_raw(tx));
            return Err("FSEventStreamCreate falhou".into());
        }
        let q = dispatch_queue_create(c"sdtally.fsevents".as_ptr(), std::ptr::null());
        FSEventStreamSetDispatchQueue(st, q);
        let v = Vigia { s: st, q, tx };
        if !FSEventStreamStart(st) {
            return Err("FSEventStreamStart falhou".into());
        }
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn avisa_a_pasta_que_mudou() {
        let tmp = std::env::temp_dir().join(format!("sdtally-fse-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join("sub")).unwrap();
        // O caminho real: /var/folders é /private/var/folders, e é assim que o aviso chega.
        let tmp = std::fs::canonicalize(&tmp).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let v = vigiar(tmp.to_str().unwrap(), 0.1, tx).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        std::fs::write(tmp.join("sub/a"), b"x").unwrap();
        let alvo = tmp.join("sub");
        let mut viu = false;
        while let Ok(lote) = rx.recv_timeout(Duration::from_secs(5)) {
            if lote.iter().any(|(p, _)| std::path::Path::new(p.trim_end_matches('/')) == alvo) {
                viu = true;
                break;
            }
        }
        drop(v);
        std::fs::remove_dir_all(&tmp).unwrap();
        assert!(viu, "aviso da pasta sub");
    }
}

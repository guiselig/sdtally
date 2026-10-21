//! CacheDelete (framework privado): purgeable por serviço. Só consulta, nunca purge.

use super::cpath;
use std::ffi::{CStr, c_char, c_void};

/// Pelo install-name, que existe em todas as versões (Frente O §5); o atalho sem Versions/A
/// é symlink e pode faltar.
const CACHEDELETE: &CStr = c"/System/Library/PrivateFrameworks/CacheDelete.framework/Versions/A/CacheDelete";

type CF = *const c_void;
const UTF8: u32 = 0x0800_0100;
const CF_SINT64: isize = 4;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFTypeDictionaryKeyCallBacks: [u8; 48];
    static kCFTypeDictionaryValueCallBacks: [u8; 40];
    fn CFStringCreateWithCString(a: CF, s: *const c_char, enc: u32) -> CF;
    fn CFNumberCreate(a: CF, t: isize, v: *const c_void) -> CF;
    fn CFDictionaryCreate(a: CF, k: *const CF, v: *const CF, n: isize, kc: *const c_void, vc: *const c_void) -> CF;
    fn CFDictionaryGetCount(d: CF) -> isize;
    fn CFDictionaryGetKeysAndValues(d: CF, k: *mut CF, v: *mut CF);
    fn CFGetTypeID(x: CF) -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFDictionaryGetTypeID() -> usize;
    fn CFNumberGetValue(n: CF, t: isize, out: *mut c_void) -> bool;
    fn CFStringGetCString(s: CF, b: *mut c_char, n: isize, enc: u32) -> bool;
    fn CFRelease(x: CF);
}

fn cf_str(s: CF) -> Option<String> {
    let mut b = [0 as c_char; 512];
    unsafe { CFStringGetCString(s, b.as_mut_ptr(), b.len() as isize, UTF8) }
        .then(|| unsafe { CStr::from_ptr(b.as_ptr()) }.to_string_lossy().into_owned())
}

/// CacheDeleteCopyItemizedPurgeableSpaceWithInfo(volume, urgência): chaves numéricas.
pub fn cachedelete_itemized(volume: &str, urgency: i64) -> Result<Vec<(String, i64)>, String> {
    let h = unsafe { libc::dlopen(CACHEDELETE.as_ptr(), libc::RTLD_LAZY) };
    if h.is_null() {
        return Err("dlopen CacheDelete falhou".into());
    }
    let f = unsafe { libc::dlsym(h, c"CacheDeleteCopyItemizedPurgeableSpaceWithInfo".as_ptr()) };
    if f.is_null() {
        return Err("símbolo CacheDeleteCopyItemizedPurgeableSpaceWithInfo ausente".into());
    }
    let f: extern "C" fn(CF) -> CF = unsafe { std::mem::transmute(f) };
    let v1 = cpath(volume).map_err(|_| "volume com NUL no nome".to_string())?;
    unsafe {
        let keys = [
            CFStringCreateWithCString(std::ptr::null(), c"CACHE_DELETE_VOLUME".as_ptr(), UTF8),
            CFStringCreateWithCString(std::ptr::null(), c"CACHE_DELETE_URGENCY_LIMIT".as_ptr(), UTF8),
        ];
        let vals = [
            CFStringCreateWithCString(std::ptr::null(), v1.as_ptr(), UTF8),
            CFNumberCreate(std::ptr::null(), CF_SINT64, &urgency as *const i64 as *const c_void),
        ];
        let info = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            vals.as_ptr(),
            2,
            (&raw const kCFTypeDictionaryKeyCallBacks) as *const c_void,
            (&raw const kCFTypeDictionaryValueCallBacks) as *const c_void,
        );
        let r = f(info);
        for x in keys.iter().chain(vals.iter()) {
            CFRelease(*x);
        }
        CFRelease(info);
        if r.is_null() {
            return Err("CacheDelete devolveu nulo".into());
        }
        if CFGetTypeID(r) != CFDictionaryGetTypeID() {
            CFRelease(r);
            return Err("CacheDelete devolveu tipo inesperado".into());
        }
        let n = CFDictionaryGetCount(r).max(0) as usize;
        let mut ks = vec![std::ptr::null(); n];
        let mut vs = vec![std::ptr::null(); n];
        CFDictionaryGetKeysAndValues(r, ks.as_mut_ptr(), vs.as_mut_ptr());
        let mut out = Vec::new();
        for i in 0..n {
            if CFGetTypeID(ks[i]) != CFStringGetTypeID() || CFGetTypeID(vs[i]) != CFNumberGetTypeID() {
                continue;
            }
            let mut v: i64 = 0;
            if let Some(k) = cf_str(ks[i])
                && CFNumberGetValue(vs[i], CF_SINT64, &mut v as *mut i64 as *mut c_void)
            {
                out.push((k, v));
            }
        }
        CFRelease(r);
        Ok(out)
    }
}

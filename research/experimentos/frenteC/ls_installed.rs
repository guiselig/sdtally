// ls_installed.rs - Frente C: "este bundle id esta instalado?" via LaunchServices, sem crates.
// Le bundle ids (um por linha) no stdin; imprime: <id>\t<n_urls>\t<primeira url>\t<us>
// Compilar: rustc -O ls_installed.rs -o ls_installed   (linka CoreServices + CoreFoundation)
// API: LSCopyApplicationURLsForBundleIdentifier (CoreServices/LaunchServices, macOS 10.10+).
use std::ffi::{c_void, CString};
use std::io::{self, BufRead, Write};
use std::os::raw::c_char;
use std::time::Instant;

type CFTypeRef = *const c_void;
type CFIndex = isize;
const UTF8: u32 = 0x0800_0100; // kCFStringEncodingUTF8

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(a: CFTypeRef, s: *const c_char, enc: u32) -> CFTypeRef;
    fn CFArrayGetCount(a: CFTypeRef) -> CFIndex;
    fn CFArrayGetValueAtIndex(a: CFTypeRef, i: CFIndex) -> CFTypeRef;
    fn CFURLGetFileSystemRepresentation(u: CFTypeRef, resolve: u8, buf: *mut u8, len: CFIndex) -> u8;
    fn CFRelease(o: CFTypeRef);
}
#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn LSCopyApplicationURLsForBundleIdentifier(id: CFTypeRef, err: *mut CFTypeRef) -> CFTypeRef;
}

fn lookup(id: &str) -> (usize, String) {
    let Ok(c) = CString::new(id) else { return (0, String::new()) };
    unsafe {
        let s = CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), UTF8);
        let mut err: CFTypeRef = std::ptr::null();
        let arr = LSCopyApplicationURLsForBundleIdentifier(s, &mut err);
        CFRelease(s);
        if !err.is_null() { CFRelease(err); } // kLSApplicationNotFoundErr
        if arr.is_null() { return (0, String::new()); }
        let n = CFArrayGetCount(arr) as usize;
        let mut first = String::new();
        if n > 0 {
            let mut buf = [0u8; 4096];
            if CFURLGetFileSystemRepresentation(CFArrayGetValueAtIndex(arr, 0), 1, buf.as_mut_ptr(), 4096) != 0 {
                let end = buf.iter().position(|&b| b == 0).unwrap_or(0);
                first = String::from_utf8_lossy(&buf[..end]).into_owned();
            }
        }
        CFRelease(arr);
        (n, first)
    }
}

fn main() {
    let out = io::stdout();
    let mut out = out.lock();
    for line in io::stdin().lock().lines().map_while(Result::ok) {
        let id = line.trim();
        if id.is_empty() { continue; }
        let t = Instant::now();
        let (n, first) = lookup(id);
        let _ = writeln!(out, "{id}\t{n}\t{first}\t{}", t.elapsed().as_micros());
    }
}

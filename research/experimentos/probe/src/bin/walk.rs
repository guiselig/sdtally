//! Experimento: varredura com getattrlistbulk(2), single-thread, sem root.
//! Uso: walk <diretório>
//! Mede: tempo, alocado real (hardlinks e clones completos contados uma vez),
//! espaço privado (liberável agora), preso em snapshot, dataless, esparso.

use std::collections::HashSet;
use std::ffi::{CStr, CString};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::time::Instant;

const SF_DATALESS: u32 = 0x4000_0000;
const EF_MAY_SHARE_BLOCKS: u64 = 0x01;
const EF_IS_SPARSE: u64 = 0x10;
const EF_SHARES_ALL_BLOCKS: u64 = 0x40;
const VDIR: u32 = 2;
const VREG: u32 = 1;

#[derive(Default)]
struct Totals {
    files: u64,
    dirs: u64,
    errors: u64,
    other_dev: u64,
    alloc: i64,
    private: i64,
    snapshot_trapped: i64,
    clone_shared: i64,
    hardlink_dup: i64,
    dataless_files: u64,
    dataless_logical: i64,
    sparse_logical: i64,
    sparse_alloc: i64,
}

fn rd32(b: &[u8], p: &mut usize) -> u32 {
    let v = u32::from_ne_bytes(b[*p..*p + 4].try_into().unwrap());
    *p += 4;
    v
}
fn rd64(b: &[u8], p: &mut usize) -> u64 {
    let v = u64::from_ne_bytes(b[*p..*p + 8].try_into().unwrap());
    *p += 8;
    v
}

fn main() {
    let root = PathBuf::from(std::env::args_os().nth(1).expect("uso: walk <dir>"));
    let root_dev = std::fs::symlink_metadata(&root).map(|m| {
        use std::os::unix::fs::MetadataExt;
        m.dev() as u32
    }).expect("raiz inacessível");

    let mut al: libc::attrlist = unsafe { std::mem::zeroed() };
    al.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
    al.commonattr = libc::ATTR_CMN_RETURNED_ATTRS
        | libc::ATTR_CMN_NAME
        | libc::ATTR_CMN_DEVID
        | libc::ATTR_CMN_OBJTYPE
        | libc::ATTR_CMN_FLAGS
        | libc::ATTR_CMN_FILEID;
    al.fileattr = libc::ATTR_FILE_LINKCOUNT | libc::ATTR_FILE_TOTALSIZE | libc::ATTR_FILE_ALLOCSIZE;
    al.forkattr = libc::ATTR_CMNEXT_PRIVATESIZE | libc::ATTR_CMNEXT_CLONEID | libc::ATTR_CMNEXT_EXT_FLAGS;
    let opts = (libc::FSOPT_NOFOLLOW | libc::FSOPT_ATTR_CMN_EXTENDED) as u64;
    if std::env::var_os("SDC_NOEXT").is_some() { al.forkattr = 0; }
    if std::env::var_os("SDC_NOPRIV").is_some() { al.forkattr &= !libc::ATTR_CMNEXT_PRIVATESIZE; }

    let mut t = Totals::default();
    let mut seen_links: HashSet<u64> = HashSet::new();
    let mut seen_clones: HashSet<u64> = HashSet::new();
    let mut stack = vec![root];
    let mut buf = vec![0u8; 256 * 1024];
    let start = Instant::now();

    while let Some(dir) = stack.pop() {
        let c = CString::new(dir.as_os_str().as_bytes()).unwrap();
        let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW) };
        if fd < 0 {
            t.errors += 1;
            continue;
        }
        t.dirs += 1;
        loop {
            let n = unsafe {
                libc::getattrlistbulk(fd, &mut al as *mut _ as *mut _, buf.as_mut_ptr() as *mut _, buf.len(), opts)
            };
            if n <= 0 {
                if n < 0 { t.errors += 1; }
                break;
            }
            let mut entry = 0usize;
            for _ in 0..n {
                let len = u32::from_ne_bytes(buf[entry..entry + 4].try_into().unwrap()) as usize;
                let mut p = entry + 4;
                entry += len; // antes de qualquer `continue`
                let ret_cmn = rd32(&buf, &mut p);
                let _vol = rd32(&buf, &mut p);
                let _dir = rd32(&buf, &mut p);
                let ret_file = rd32(&buf, &mut p);
                let ret_ext = rd32(&buf, &mut p);

                // ATTR_CMN_NAME: attrreference relativo à própria posição.
                let name_ref = p;
                let off = i32::from_ne_bytes(buf[p..p + 4].try_into().unwrap());
                p += 8;
                let name = unsafe { CStr::from_ptr(buf.as_ptr().add((name_ref as isize + off as isize) as usize) as *const _) };
                let dev = if ret_cmn & libc::ATTR_CMN_DEVID != 0 { rd32(&buf, &mut p) } else { 0 };
                let objtype = if ret_cmn & libc::ATTR_CMN_OBJTYPE != 0 { rd32(&buf, &mut p) } else { 0 };
                let flags = if ret_cmn & libc::ATTR_CMN_FLAGS != 0 { rd32(&buf, &mut p) } else { 0 };
                let fileid = if ret_cmn & libc::ATTR_CMN_FILEID != 0 { rd64(&buf, &mut p) } else { 0 };

                if objtype == VDIR {
                    if dev != root_dev {
                        t.other_dev += 1;
                    } else {
                        stack.push(dir.join(std::ffi::OsStr::from_bytes(name.to_bytes())));
                    }
                } else if objtype == VREG {
                    t.files += 1;
                    let links = if ret_file & libc::ATTR_FILE_LINKCOUNT != 0 { rd32(&buf, &mut p) } else { 1 };
                    let logical = if ret_file & libc::ATTR_FILE_TOTALSIZE != 0 { rd64(&buf, &mut p) as i64 } else { 0 };
                    let alloc = if ret_file & libc::ATTR_FILE_ALLOCSIZE != 0 { rd64(&buf, &mut p) as i64 } else { 0 };
                    let private = if ret_ext & libc::ATTR_CMNEXT_PRIVATESIZE != 0 { rd64(&buf, &mut p) as i64 } else { alloc };
                    let cloneid = if ret_ext & libc::ATTR_CMNEXT_CLONEID != 0 { rd64(&buf, &mut p) } else { 0 };
                    let ext = if ret_ext & libc::ATTR_CMNEXT_EXT_FLAGS != 0 { rd64(&buf, &mut p) } else { 0 };
                    if std::env::var_os("SDC_DEBUG").is_some() { eprintln!("{:?} cmn=0x{:x} file=0x{:x} ext=0x{:x} links={} log={} alloc={} priv={} clone={} ef=0x{:x}", name, ret_cmn, ret_file, ret_ext, links, logical, alloc, private, cloneid, ext); }

                    if flags & SF_DATALESS != 0 {
                        t.dataless_files += 1;
                        t.dataless_logical += logical;
                    }
                    // Hardlink: mesmo fileid, conta uma vez.
                    if links > 1 && !seen_links.insert(fileid) {
                        t.hardlink_dup += alloc;
                        continue;
                    }
                    // Clone completo: mesmo cloneid, blocos contam uma vez.
                    if ext & EF_SHARES_ALL_BLOCKS != 0 && !seen_clones.insert(cloneid) {
                        t.clone_shared += alloc;
                        if alloc > (100 << 20) && std::env::var_os("SDC_CLONES").is_some() { println!("clone {:>8.2} GiB {}", alloc as f64 / (1u64 << 30) as f64, dir.join(std::ffi::OsStr::from_bytes(name.to_bytes())).display()); }
                        continue;
                    }
                    t.alloc += alloc;
                    t.private += private;
                    if ext & EF_MAY_SHARE_BLOCKS == 0 {
                        t.snapshot_trapped += alloc - private;
                    }
                    if ext & EF_IS_SPARSE != 0 {
                        t.sparse_logical += logical;
                        t.sparse_alloc += alloc;
                    }
                }
            }
        }
        unsafe { libc::close(fd) };
    }

    let g = |b: i64| b as f64 / (1u64 << 30) as f64;
    println!("tempo            {:.1} s", start.elapsed().as_secs_f64());
    println!("arquivos/pastas  {} / {} (erros {}, outros volumes {})", t.files, t.dirs, t.errors, t.other_dev);
    println!("alocado real     {:.2} GiB", g(t.alloc));
    println!("privado          {:.2} GiB  (liberado na hora se apagar tudo)", g(t.private));
    println!("preso snapshot   {:.2} GiB  (alocado - privado, sem clone)", g(t.snapshot_trapped));
    println!("hardlinks repet. {:.2} GiB  (não somado ao alocado)", g(t.hardlink_dup));
    println!("clones repetidos {:.2} GiB  (não somado ao alocado)", g(t.clone_shared));
    println!("dataless         {} arquivos, {:.2} GiB aparentes, 0 no disco", t.dataless_files, g(t.dataless_logical));
    println!("esparsos         {:.2} GiB aparentes, {:.2} GiB alocados", g(t.sparse_logical), g(t.sparse_alloc));
}

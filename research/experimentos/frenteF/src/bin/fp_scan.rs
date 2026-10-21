//! Frente F: flags de nuvem por balde, sem baixar nada.
//! Uso: fp_scan <raiz> [prof_balde] [--xattr] [--list MASK]
//!
//! - setiopolicy_np(MATERIALIZE_DATALESS_FILES_OFF) no processo e não desce em pasta SF_DATALESS.
//! - Por balde: arquivos, alocado, purgeable (EF_IS_PURGEABLE), dataless (SF_DATALESS),
//!   EF_MAY_SHARE_BLOCKS, PRIVATESIZE, pastas EF_IS_SYNC_ROOT, pastas dataless não abertas.
//! - --xattr: histograma de nomes de xattr (listxattr com XATTR_NOFOLLOW; pula EF_NO_XATTRS).
//! - --list MASK: imprime caminho de arquivos cujo estado casa (np = não purgeable com dados,
//!   p = purgeable, dl = dataless) em stderr, para amostragem local. Não vai para relatório.
//! ponytail: serial e sem dedup de hardlink/clone (em Mobile Documents dá 0,05 GiB); para o
//! volume inteiro use walk_par, que deduplica.

use std::collections::{BTreeMap, HashMap};
use std::ffi::{CStr, CString, OsStr};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

const SF_DATALESS: u32 = 0x4000_0000;
const ATTR_CMN_ERROR: u32 = 0x2000_0000;
const EF_MAY_SHARE_BLOCKS: u64 = 0x01;
const EF_NO_XATTRS: u64 = 0x02;
const EF_IS_SYNC_ROOT: u64 = 0x04;
const EF_IS_PURGEABLE: u64 = 0x08;
const VREG: u32 = 1;
const VDIR: u32 = 2;
unsafe extern "C" {
    fn setiopolicy_np(iotype: i32, scope: i32, policy: i32) -> i32;
}

#[derive(Default)]
struct Sum {
    files: u64,
    alloc: i64,
    logical: i64,
    private: i64,
    purg_n: u64,
    purg_alloc: i64,
    purg_share: i64,
    dl_n: u64,
    dl_logical: i64,
    share_alloc: i64,
    sync_roots: u64,
    dl_dirs: u64,
}

struct Ctx {
    depth: usize,
    dev: u32,
    other_dev: u64,
    xattr: bool,
    list: Option<String>,
    buckets: BTreeMap<String, Sum>,
    xnames: HashMap<String, (u64, u64)>, // nome -> (itens, bytes do valor)
    errors: HashMap<i32, u64>,
    buf: Vec<u8>,
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

fn xattrs(ctx: &mut Ctx, path: &Path) {
    let c = CString::new(path.as_os_str().as_bytes()).unwrap();
    let mut names = vec![0u8; 8192];
    let n = unsafe { libc::listxattr(c.as_ptr(), names.as_mut_ptr() as *mut _, names.len(), libc::XATTR_NOFOLLOW) };
    if n < 0 {
        let e = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        *ctx.errors.entry(-e).or_default() += 1; // negativo = erro de listxattr
        return;
    }
    for name in names[..n as usize].split(|b| *b == 0).filter(|s| !s.is_empty()) {
        let cn = CString::new(name).unwrap();
        // Só o tamanho do valor (buffer nulo), sem ler conteúdo.
        let sz = unsafe { libc::getxattr(c.as_ptr(), cn.as_ptr(), std::ptr::null_mut(), 0, 0, libc::XATTR_NOFOLLOW) };
        let key = String::from_utf8_lossy(name).into_owned();
        let e = ctx.xnames.entry(key).or_default();
        e.0 += 1;
        e.1 += sz.max(0) as u64;
    }
}

fn scan(ctx: &mut Ctx, dir: &Path, bucket: &str, depth: usize) {
    let c = CString::new(dir.as_os_str().as_bytes()).unwrap();
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW) };
    if fd < 0 {
        *ctx.errors.entry(std::io::Error::last_os_error().raw_os_error().unwrap_or(0)).or_default() += 1;
        return;
    }
    let mut al: libc::attrlist = unsafe { std::mem::zeroed() };
    al.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
    al.commonattr = libc::ATTR_CMN_RETURNED_ATTRS | libc::ATTR_CMN_NAME | libc::ATTR_CMN_DEVID | libc::ATTR_CMN_OBJTYPE | libc::ATTR_CMN_FLAGS | ATTR_CMN_ERROR;
    al.fileattr = libc::ATTR_FILE_TOTALSIZE | libc::ATTR_FILE_ALLOCSIZE;
    al.forkattr = libc::ATTR_CMNEXT_PRIVATESIZE | libc::ATTR_CMNEXT_EXT_FLAGS;
    let opts = (libc::FSOPT_NOFOLLOW | libc::FSOPT_ATTR_CMN_EXTENDED) as u64;
    let mut subdirs = Vec::new();
    let mut xa = Vec::new();
    let mut buf = std::mem::take(&mut ctx.buf);
    loop {
        let n = unsafe { libc::getattrlistbulk(fd, &mut al as *mut _ as *mut _, buf.as_mut_ptr() as *mut _, buf.len(), opts) };
        if n <= 0 {
            if n < 0 {
                *ctx.errors.entry(std::io::Error::last_os_error().raw_os_error().unwrap_or(0)).or_default() += 1;
            }
            break;
        }
        let mut entry = 0usize;
        for _ in 0..n {
            let len = u32::from_ne_bytes(buf[entry..entry + 4].try_into().unwrap()) as usize;
            let mut p = entry + 4;
            entry += len;
            let ret_cmn = rd32(&buf, &mut p);
            let _ = rd32(&buf, &mut p);
            let _ = rd32(&buf, &mut p);
            let ret_file = rd32(&buf, &mut p);
            let ret_ext = rd32(&buf, &mut p);
            let name_ref = p;
            let off = i32::from_ne_bytes(buf[p..p + 4].try_into().unwrap());
            p += 8;
            let name = unsafe { CStr::from_ptr(buf.as_ptr().add((name_ref as isize + off as isize) as usize) as *const _) }.to_bytes().to_vec();
            let dev = if ret_cmn & libc::ATTR_CMN_DEVID != 0 { rd32(&buf, &mut p) } else { 0 };
            let objtype = if ret_cmn & libc::ATTR_CMN_OBJTYPE != 0 { rd32(&buf, &mut p) } else { 0 };
            let flags = if ret_cmn & libc::ATTR_CMN_FLAGS != 0 { rd32(&buf, &mut p) } else { 0 };
            if ret_cmn & ATTR_CMN_ERROR != 0 {
                let e = rd32(&buf, &mut p) as i32;
                if e != 0 {
                    *ctx.errors.entry(e).or_default() += 1;
                    continue;
                }
            }
            let logical = if ret_file & libc::ATTR_FILE_TOTALSIZE != 0 { rd64(&buf, &mut p) as i64 } else { 0 };
            let alloc = if ret_file & libc::ATTR_FILE_ALLOCSIZE != 0 { rd64(&buf, &mut p) as i64 } else { 0 };
            let private = if ret_ext & libc::ATTR_CMNEXT_PRIVATESIZE != 0 { rd64(&buf, &mut p) as i64 } else { 0 };
            let ext = if ret_ext & libc::ATTR_CMNEXT_EXT_FLAGS != 0 { rd64(&buf, &mut p) } else { 0 };

            let path = dir.join(OsStr::from_bytes(&name));
            let b = if depth < ctx.depth {
                format!("{}/{}", bucket, String::from_utf8_lossy(&name)).trim_start_matches('/').to_string()
            } else {
                bucket.to_string()
            };
            if ctx.xattr && ext & EF_NO_XATTRS == 0 {
                xa.push(path.clone());
            }
            let s = ctx.buckets.entry(b.clone()).or_default();
            if objtype == VDIR {
                if dev != ctx.dev {
                    ctx.other_dev += 1; // outro volume montado aqui: não atravessa
                    continue;
                }
                if ext & EF_IS_SYNC_ROOT != 0 {
                    s.sync_roots += 1;
                }
                if flags & SF_DATALESS != 0 {
                    s.dl_dirs += 1;
                    continue;
                }
                subdirs.push((path, b, depth + 1));
            } else if objtype == VREG {
                s.files += 1;
                s.alloc += alloc;
                s.logical += logical;
                s.private += private;
                if ext & EF_MAY_SHARE_BLOCKS != 0 {
                    s.share_alloc += alloc;
                }
                let tag = if flags & SF_DATALESS != 0 {
                    s.dl_n += 1;
                    s.dl_logical += logical;
                    "dl"
                } else if ext & EF_IS_PURGEABLE != 0 {
                    s.purg_n += 1;
                    s.purg_alloc += alloc;
                    if ext & EF_MAY_SHARE_BLOCKS != 0 {
                        s.purg_share += alloc;
                    }
                    "p"
                } else {
                    "np"
                };
                if ctx.list.as_deref().is_some_and(|m| m.split(',').any(|x| x == tag)) {
                    eprintln!("{tag}\t{alloc}\t{}", path.display());
                }
            }
        }
    }
    unsafe { libc::close(fd) };
    ctx.buf = buf;
    for p in xa {
        xattrs(ctx, &p);
    }
    for (p, b, d) in subdirs {
        scan(ctx, &p, &b, d);
    }
}

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let root = PathBuf::from(args.first().expect("uso: fp_scan <raiz> [prof] [--xattr] [--list np,p,dl]"));
    let depth = args.get(1).and_then(|a| a.to_str()?.parse().ok()).unwrap_or(1);
    let xattr = args.iter().any(|a| a == "--xattr");
    let list = args.iter().position(|a| a == "--list").and_then(|i| args.get(i + 1)).map(|s| s.to_string_lossy().into_owned());
    let rc = unsafe { setiopolicy_np(3, 0, 1) }; // VFS_MATERIALIZE_DATALESS_FILES, PROCESS, OFF
    assert_eq!(rc, 0, "setiopolicy_np falhou");

    let dev = {
        use std::os::unix::fs::MetadataExt;
        std::fs::symlink_metadata(&root).expect("raiz inacessível").dev() as u32
    };
    let mut ctx = Ctx { depth, dev, other_dev: 0, xattr, list, buckets: BTreeMap::new(), xnames: HashMap::new(), errors: HashMap::new(), buf: vec![0; 256 * 1024] };
    scan(&mut ctx, &root, "", 0);

    let g = |b: i64| b as f64 / (1u64 << 30) as f64;
    let mut t = Sum::default();
    println!("{:>7} {:>8} {:>8} {:>8} {:>8} {:>7} {:>9} {:>6} {:>5} {:>5}  balde", "arqs", "alocGiB", "purgGiB", "npGiB", "shareGiB", "privGiB", "dl_n", "dlGiB", "sroot", "dldir");
    let mut v: Vec<_> = ctx.buckets.iter().collect();
    v.sort_by_key(|(_, s)| -(s.alloc + s.dl_logical));
    for (k, s) in &v {
        println!(
            "{:>7} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>7.2} {:>9} {:>6.2} {:>5} {:>5}  {}",
            s.files, g(s.alloc), g(s.purg_alloc), g(s.alloc - s.purg_alloc), g(s.share_alloc), g(s.private), s.dl_n, g(s.dl_logical), s.sync_roots, s.dl_dirs, k
        );
        t.files += s.files;
        t.alloc += s.alloc;
        t.logical += s.logical;
        t.purg_n += s.purg_n;
        t.purg_alloc += s.purg_alloc;
        t.purg_share += s.purg_share;
        t.dl_n += s.dl_n;
        t.dl_logical += s.dl_logical;
        t.share_alloc += s.share_alloc;
        t.private += s.private;
        t.sync_roots += s.sync_roots;
        t.dl_dirs += s.dl_dirs;
    }
    println!(
        "TOTAL arquivos {} | alocado {:.2} GiB | purgeable {} arqs {:.2} GiB (com MAY_SHARE {:.2}) | não purgeable {:.2} GiB | may_share {:.2} | privado {:.2} | dataless {} arqs {:.2} GiB lógicos | sync roots {} | pastas dataless {}",
        t.files, g(t.alloc), t.purg_n, g(t.purg_alloc), g(t.purg_share), g(t.alloc - t.purg_alloc), g(t.share_alloc), g(t.private), t.dl_n, g(t.dl_logical), t.sync_roots, t.dl_dirs
    );
    let mut e: Vec<_> = ctx.errors.into_iter().collect();
    e.sort();
    println!("erros (errno; negativo = listxattr) {:?} | pontos de montagem não atravessados {}", e, ctx.other_dev);
    if ctx.xattr {
        let mut x: Vec<_> = ctx.xnames.into_iter().collect();
        x.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
        println!("\nxattr: itens  bytes  nome");
        for (k, (n, b)) in x {
            println!("{n:>8} {b:>10}  {k}");
        }
    }
}

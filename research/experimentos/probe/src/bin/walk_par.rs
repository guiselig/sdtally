//! Experimento: varredura paralela com getattrlistbulk(2), sem root.
//! Uso: walk_par <raiz> [threads] [profundidade_do_balde]
//!
//! Fica no mesmo volume da raiz (dev id). Deduplica hardlinks (fileid) e clones
//! completos (cloneid) globalmente, no fim. Agrupa por prefixo de caminho ("balde").
//! Técnicas inspiradas no dua-core (MIT, Byron/dua-cli): ATTR_CMN_ERROR por entrada,
//! checagem bit a bit do attribute_set retornado.

use std::collections::HashMap;
use std::ffi::{CStr, CString, OsStr};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

const SF_DATALESS: u32 = 0x4000_0000;
const ATTR_CMN_ERROR: u32 = 0x2000_0000;
const EF_MAY_SHARE_BLOCKS: u64 = 0x01;
const EF_IS_PURGEABLE: u64 = 0x08;
const EF_IS_SPARSE: u64 = 0x10;
const EF_SHARES_ALL_BLOCKS: u64 = 0x40;
const VDIR: u32 = 2;
// <sys/resource.h>: impede que o processo peça ao File Provider para baixar/materializar.
const IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES: i32 = 3;
const IOPOL_SCOPE_PROCESS: i32 = 0;
const IOPOL_MATERIALIZE_DATALESS_FILES_OFF: i32 = 1;
// Sem isto, listar uma pasta grava o atime dela (medido: 22.200 de 22.200 pastas; Frente K).
const IOPOL_TYPE_VFS_ATIME_UPDATES: i32 = 2;
const IOPOL_ATIME_UPDATES_OFF: i32 = 1;
unsafe extern "C" {
    fn setiopolicy_np(iotype: i32, scope: i32, policy: i32) -> i32;
}
const VREG: u32 = 1;
static MEASURE_XATTR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Default, Clone, Copy)]
struct Sum {
    files: u64,
    alloc: i64,
    private: i64,
    snap: i64,
    dataless_n: u64,
    dataless_logical: i64,
    sparse_logical: i64,
    sparse_alloc: i64,
    purgeable: i64,
    /// Atributos estendidos fora do ResourceFork: bytes de valor, e só os > 3804 B
    /// (limite de xattr embutido no registro do inode, APFS Reference).
    xattr_all: i64,
    xattr_stream: i64,
    xattr_files: u64,
}

impl Sum {
    fn add(&mut self, o: &Sum) {
        self.files += o.files;
        self.alloc += o.alloc;
        self.private += o.private;
        self.snap += o.snap;
        self.dataless_n += o.dataless_n;
        self.dataless_logical += o.dataless_logical;
        self.sparse_logical += o.sparse_logical;
        self.sparse_alloc += o.sparse_alloc;
        self.purgeable += o.purgeable;
        self.xattr_all += o.xattr_all;
        self.xattr_stream += o.xattr_stream;
        self.xattr_files += o.xattr_files;
    }
}

/// Arquivo que pode se repetir (hardlink ou clone completo): decidido no fim.
struct Shared {
    bucket: Arc<str>,
    sum: Sum,
    /// cloneid quando o inode é clone completo (EF_SHARES_ALL_BLOCKS), senão 0.
    clone_of: u64,
}

#[derive(Default)]
struct Local {
    buckets: HashMap<Arc<str>, Sum>,
    links: HashMap<u64, (Shared, u64)>,
    clones: HashMap<u64, (Shared, u64)>,
    dirs: u64,
    errors: HashMap<i32, u64>,
    denied: Vec<PathBuf>,
    other_dev: u64,
    dataless_dirs: u64,
    graft_roots: Vec<PathBuf>,
}

struct Queue {
    stack: Mutex<(Vec<(PathBuf, Arc<str>, usize)>, usize)>, // (trabalho, pendentes)
    cv: Condvar,
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

const EF_NO_XATTRS: u64 = 0x02;
const XATTR_MAX_EMBEDDED: i64 = 3804;

/// Soma o tamanho dos valores de xattr (sem ler o conteúdo). ResourceFork e decmpfs ficam
/// de fora: já entram no ATTR_FILE_ALLOCSIZE (conferido em arquivo comprimido).
fn xattr_sizes(path: &std::path::Path, s: &mut Sum) {
    let c = CString::new(path.as_os_str().as_bytes()).unwrap();
    let opts = libc::XATTR_NOFOLLOW | libc::XATTR_SHOWCOMPRESSION;
    let n = unsafe { libc::listxattr(c.as_ptr(), std::ptr::null_mut(), 0, opts) };
    if n <= 0 {
        return;
    }
    let mut names = vec![0u8; n as usize];
    let n = unsafe { libc::listxattr(c.as_ptr(), names.as_mut_ptr() as *mut _, names.len(), opts) };
    if n <= 0 {
        return;
    }
    let mut any = false;
    for name in names[..n as usize].split(|b| *b == 0).filter(|x| !x.is_empty()) {
        if name == b"com.apple.ResourceFork" || name == b"com.apple.decmpfs" {
            continue;
        }
        let cn = CString::new(name).unwrap();
        let sz = unsafe { libc::getxattr(c.as_ptr(), cn.as_ptr(), std::ptr::null_mut(), 0, 0, opts) } as i64;
        if sz > 0 {
            any = true;
            s.xattr_all += sz;
            if sz > XATTR_MAX_EMBEDDED {
                s.xattr_stream += sz;
            }
        }
    }
    if any {
        s.xattr_files += 1;
    }
}

fn attrlist() -> libc::attrlist {
    let mut al: libc::attrlist = unsafe { std::mem::zeroed() };
    al.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
    al.commonattr = libc::ATTR_CMN_RETURNED_ATTRS
        | libc::ATTR_CMN_NAME
        | libc::ATTR_CMN_DEVID
        | libc::ATTR_CMN_OBJTYPE
        | libc::ATTR_CMN_FLAGS
        | libc::ATTR_CMN_FILEID
        | ATTR_CMN_ERROR;
    al.fileattr = libc::ATTR_FILE_LINKCOUNT | libc::ATTR_FILE_TOTALSIZE | libc::ATTR_FILE_ALLOCSIZE;
    al.forkattr = libc::ATTR_CMNEXT_PRIVATESIZE | libc::ATTR_CMNEXT_CLONEID | libc::ATTR_CMNEXT_EXT_FLAGS;
    al
}

/// (fileid, cloneid) da pasta aberta.
fn dir_ids(fd: i32) -> (u64, u64) {
    let mut al: libc::attrlist = unsafe { std::mem::zeroed() };
    al.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
    al.commonattr = libc::ATTR_CMN_FILEID;
    al.forkattr = libc::ATTR_CMNEXT_CLONEID;
    let mut b = [0u8; 32];
    let rc = unsafe { libc::fgetattrlist(fd, &mut al as *mut _ as *mut _, b.as_mut_ptr() as *mut _, b.len(), libc::FSOPT_ATTR_CMN_EXTENDED as u32) };
    if rc != 0 {
        return (0, 0);
    }
    let mut p = 4;
    (rd64(&b, &mut p), rd64(&b, &mut p))
}

fn scan_dir(
    dir: &PathBuf,
    bucket: &Arc<str>,
    depth: usize,
    bucket_depth: usize,
    root_dev: u32,
    buf: &mut [u8],
    l: &mut Local,
    push: &mut Vec<(PathBuf, Arc<str>, usize)>,
) {
    let c = CString::new(dir.as_os_str().as_bytes()).unwrap();
    // EINTR: repetir; perder a pasta seria erro de medição (Frente K: 12 de 18 rodadas).
    let fd = loop {
        let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW) };
        if fd >= 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
            break fd;
        }
    };
    if fd < 0 {
        let e = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        *l.errors.entry(e).or_default() += 1;
        if l.denied.len() < 5000 {
            l.denied.push(PathBuf::from(format!("[open errno {e}] {}", dir.display())));
        }
        return;
    }
    l.dirs += 1;
    // Raiz de enxerto (cryptex do MobileAsset, ex. AssetsV2/*/.AssetData): aberta, devolve
    // cloneid 2 e fileid do Data; pasta comum tem cloneid == fileid. Os blocos são os do
    // *_Cryptex.dmg vizinho, já contado (Frente L). A raiz do volume também tem cloneid 2:
    // não testar a raiz da travessia.
    if depth > 0 {
        let (id, cid) = dir_ids(fd);
        if cid != 0 && cid != id {
            l.graft_roots.push(dir.clone());
            unsafe { libc::close(fd) };
            return;
        }
    }
    let mut al = attrlist();
    let opts = (libc::FSOPT_NOFOLLOW | libc::FSOPT_ATTR_CMN_EXTENDED) as u64;
    loop {
        let n = unsafe { libc::getattrlistbulk(fd, &mut al as *mut _ as *mut _, buf.as_mut_ptr() as *mut _, buf.len(), opts) };
        if n < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
            continue;
        }
        if n <= 0 {
            if n < 0 {
                let e = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
                *l.errors.entry(e).or_default() += 1;
                if l.denied.len() < 5000 {
                    l.denied.push(PathBuf::from(format!("[bulk errno {e}] {}", dir.display())));
                }
            }
            break;
        }
        let mut entry = 0usize;
        for _ in 0..n {
            let len = u32::from_ne_bytes(buf[entry..entry + 4].try_into().unwrap()) as usize;
            let mut p = entry + 4;
            entry += len;
            let ret_cmn = rd32(buf, &mut p);
            let _ = rd32(buf, &mut p);
            let _ = rd32(buf, &mut p);
            let ret_file = rd32(buf, &mut p);
            let ret_ext = rd32(buf, &mut p);
            // ATTR_CMN_ERROR vem logo depois do attribute_set, antes do nome (man getattrlistbulk;
            // vfs_attrlist.c). Lido depois do FILEID, o errno virava offset do nome (o ERANGE fantasma).
            if ret_cmn & ATTR_CMN_ERROR != 0 {
                let e = rd32(buf, &mut p) as i32;
                if e != 0 {
                    *l.errors.entry(e).or_default() += 1;
                    continue;
                }
            }

            // Ordem de bits comuns: NAME(0x1) DEVID(0x2) OBJTYPE(0x8) FLAGS FILEID ERROR(0x20000000).
            let name_ref = p;
            let off = i32::from_ne_bytes(buf[p..p + 4].try_into().unwrap());
            p += 8;
            let name = unsafe { CStr::from_ptr(buf.as_ptr().add((name_ref as isize + off as isize) as usize) as *const _) };
            let dev = if ret_cmn & libc::ATTR_CMN_DEVID != 0 { rd32(buf, &mut p) } else { 0 };
            let objtype = if ret_cmn & libc::ATTR_CMN_OBJTYPE != 0 { rd32(buf, &mut p) } else { 0 };
            let flags = if ret_cmn & libc::ATTR_CMN_FLAGS != 0 { rd32(buf, &mut p) } else { 0 };
            let fileid = if ret_cmn & libc::ATTR_CMN_FILEID != 0 { rd64(buf, &mut p) } else { 0 };

            if objtype == VDIR {
                if dev != root_dev {
                    l.other_dev += 1;
                    continue;
                }
                // Pasta dataless: listar pede ao File Provider para materializar (ETIMEDOUT).
                if flags & SF_DATALESS != 0 && std::env::var_os("SDC_ENTER_DATALESS").is_none() {
                    l.dataless_dirs += 1;
                    continue;
                }
                let path = dir.join(OsStr::from_bytes(name.to_bytes()));
                if MEASURE_XATTR.load(std::sync::atomic::Ordering::Relaxed) {
                    // cmnext de pasta: pular PRIVATESIZE/CLONEID se vierem, ler EXT_FLAGS.
                    if ret_ext & libc::ATTR_CMNEXT_PRIVATESIZE != 0 { p += 8; }
                    if ret_ext & libc::ATTR_CMNEXT_CLONEID != 0 { p += 8; }
                    let ext = if ret_ext & libc::ATTR_CMNEXT_EXT_FLAGS != 0 { rd64(buf, &mut p) } else { 0 };
                    if ext & EF_NO_XATTRS == 0 {
                        let mut s = Sum::default();
                        xattr_sizes(&path, &mut s);
                        l.buckets.entry(bucket.clone()).or_default().add(&s);
                    }
                }
                let b = if depth < bucket_depth {
                    Arc::from(format!("{}/{}", bucket, name.to_string_lossy()).trim_start_matches('/'))
                } else {
                    bucket.clone()
                };
                push.push((path, b, depth + 1));
            } else if objtype == VREG {
                let links = if ret_file & libc::ATTR_FILE_LINKCOUNT != 0 { rd32(buf, &mut p) } else { 1 };
                let logical = if ret_file & libc::ATTR_FILE_TOTALSIZE != 0 { rd64(buf, &mut p) as i64 } else { 0 };
                let alloc = if ret_file & libc::ATTR_FILE_ALLOCSIZE != 0 { rd64(buf, &mut p) as i64 } else { 0 };
                let private = if ret_ext & libc::ATTR_CMNEXT_PRIVATESIZE != 0 { rd64(buf, &mut p) as i64 } else { alloc };
                let cloneid = if ret_ext & libc::ATTR_CMNEXT_CLONEID != 0 { rd64(buf, &mut p) } else { 0 };
                let ext = if ret_ext & libc::ATTR_CMNEXT_EXT_FLAGS != 0 { rd64(buf, &mut p) } else { 0 };

                let mut s = Sum { files: 1, alloc, private, ..Default::default() };
                if ext & EF_NO_XATTRS == 0 && MEASURE_XATTR.load(std::sync::atomic::Ordering::Relaxed) {
                    xattr_sizes(&dir.join(OsStr::from_bytes(name.to_bytes())), &mut s);
                }
                if ext & EF_MAY_SHARE_BLOCKS == 0 {
                    s.snap = alloc - private;
                }
                if flags & SF_DATALESS != 0 {
                    s.dataless_n = 1;
                    s.dataless_logical = logical;
                }
                if ext & EF_IS_SPARSE != 0 {
                    s.sparse_logical = logical;
                    s.sparse_alloc = alloc;
                }
                if ext & EF_IS_PURGEABLE != 0 {
                    s.purgeable = alloc;
                }
                let clone_of = if ext & EF_SHARES_ALL_BLOCKS != 0 { cloneid } else { 0 };
                if links > 1 {
                    let e = l.links.entry(fileid).or_insert((Shared { bucket: bucket.clone(), sum: s, clone_of }, 0));
                    e.1 += 1;
                } else if clone_of != 0 {
                    let e = l.clones.entry(cloneid).or_insert((Shared { bucket: bucket.clone(), sum: s, clone_of }, 0));
                    e.1 += 1;
                } else {
                    l.buckets.entry(bucket.clone()).or_default().add(&s);
                }
            }
        }
    }
    unsafe { libc::close(fd) };
}

fn main() {
    let mut args = std::env::args_os().skip(1);
    let root = PathBuf::from(args.next().expect("uso: walk_par <raiz> [threads] [prof]"));
    let threads: usize = args.next().and_then(|t| t.to_str()?.parse().ok()).unwrap_or(8);
    let bucket_depth: usize = args.next().and_then(|t| t.to_str()?.parse().ok()).unwrap_or(2);
    let root_dev = {
        use std::os::unix::fs::MetadataExt;
        std::fs::symlink_metadata(&root).expect("raiz inacessível").dev() as u32
    };

    MEASURE_XATTR.store(std::env::var_os("SDC_XATTR").is_some(), std::sync::atomic::Ordering::Relaxed);
    let rc = unsafe { setiopolicy_np(IOPOL_TYPE_VFS_ATIME_UPDATES, IOPOL_SCOPE_PROCESS, IOPOL_ATIME_UPDATES_OFF) };
    assert_eq!(rc, 0, "setiopolicy_np(ATIME_UPDATES) falhou");
    if std::env::var_os("SDC_MATERIALIZE").is_none() {
        let rc = unsafe { setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS, IOPOL_MATERIALIZE_DATALESS_FILES_OFF) };
        assert_eq!(rc, 0, "setiopolicy_np falhou");
    }
    let q = Arc::new(Queue { stack: Mutex::new((vec![(root.clone(), Arc::from(""), 0)], 1)), cv: Condvar::new() });
    let start = Instant::now();
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let q = q.clone();
            std::thread::spawn(move || {
                let mut l = Local::default();
                let mut buf = vec![0u8; 256 * 1024];
                let mut push = Vec::new();
                loop {
                    let job = {
                        let mut g = q.stack.lock().unwrap();
                        loop {
                            if let Some(j) = g.0.pop() {
                                break Some(j);
                            }
                            if g.1 == 0 {
                                break None;
                            }
                            g = q.cv.wait(g).unwrap();
                        }
                    };
                    let Some((dir, bucket, depth)) = job else {
                        q.cv.notify_all();
                        return l;
                    };
                    scan_dir(&dir, &bucket, depth, bucket_depth, root_dev, &mut buf, &mut l, &mut push);
                    let mut g = q.stack.lock().unwrap();
                    g.1 += push.len();
                    g.1 -= 1;
                    g.0.append(&mut push);
                    drop(g);
                    q.cv.notify_all();
                }
            })
        })
        .collect();

    let mut all = Local::default();
    let mut links: HashMap<u64, (Shared, u64)> = HashMap::new();
    let mut clones: HashMap<u64, (Shared, u64)> = HashMap::new();
    for h in handles {
        let l = h.join().unwrap();
        for (k, v) in l.buckets {
            all.buckets.entry(k).or_default().add(&v);
        }
        for (k, (s, n)) in l.links {
            links.entry(k).or_insert((s, 0)).1 += n;
        }
        for (k, (s, n)) in l.clones {
            clones.entry(k).or_insert((s, 0)).1 += n;
        }
        all.dirs += l.dirs;
        all.other_dev += l.other_dev;
        all.dataless_dirs += l.dataless_dirs;
        all.graft_roots.extend(l.graft_roots);
        for (k, v) in l.errors {
            *all.errors.entry(k).or_default() += v;
        }
        all.denied.extend(l.denied);
    }
    let elapsed = start.elapsed().as_secs_f64();

    // Repetidos contam uma vez, no balde de quem foi guardado primeiro.
    let (mut link_dup, mut clone_dup, mut link_files, mut clone_files) = (0i64, 0i64, 0u64, 0u64);
    // Um inode com hardlinks pode também ser clone completo: depois de deduplicar por
    // fileid, ele entra na deduplicação por cloneid como uma ocorrência.
    for (s, n) in links.into_values() {
        link_dup += s.sum.alloc * (n as i64 - 1);
        link_files += n - 1;
        if s.clone_of != 0 {
            clones.entry(s.clone_of).or_insert((Shared { bucket: s.bucket.clone(), sum: s.sum, clone_of: s.clone_of }, 0)).1 += 1;
        } else {
            all.buckets.entry(s.bucket.clone()).or_default().add(&s.sum);
        }
    }
    for (s, n) in clones.values() {
        all.buckets.entry(s.bucket.clone()).or_default().add(&s.sum);
        clone_dup += s.sum.alloc * (*n as i64 - 1);
        clone_files += n - 1;
    }

    let mut total = Sum::default();
    for s in all.buckets.values() {
        total.add(s);
    }
    let g = |b: i64| b as f64 / (1u64 << 30) as f64;
    let mut v: Vec<_> = all.buckets.iter().collect();
    v.sort_by_key(|(_, s)| -s.alloc);

    // statfs no APFS devolve o container inteiro (todos os volumes). O uso só deste volume
    // é ATTR_VOL_SPACEUSED (o "Volume Used Space" do diskutil).
    let mut st: libc::statfs = unsafe { std::mem::zeroed() };
    let c = CString::new(root.as_os_str().as_bytes()).unwrap();
    unsafe { libc::statfs(c.as_ptr(), &mut st) };
    let container_used = (st.f_blocks as i64 - st.f_bfree as i64) * st.f_bsize as i64;
    let vol_used = {
        let mut al: libc::attrlist = unsafe { std::mem::zeroed() };
        al.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
        al.volattr = libc::ATTR_VOL_INFO | libc::ATTR_VOL_SPACEUSED;
        let mut b = [0u8; 64];
        let mnt = unsafe { CStr::from_ptr(st.f_mntonname.as_ptr()) };
        let rc = unsafe { libc::getattrlist(mnt.as_ptr(), &mut al as *mut _ as *mut _, b.as_mut_ptr() as *mut _, b.len(), 0) };
        assert_eq!(rc, 0, "getattrlist ATTR_VOL_SPACEUSED falhou");
        i64::from_ne_bytes(b[4..12].try_into().unwrap())
    };

    println!("raiz {} | {threads} threads | {elapsed:.1} s", root.display());
    println!("pastas {} | arquivos {} (+{} hardlinks, +{} clones repetidos) | outros volumes {}", all.dirs, total.files, link_files, clone_files, all.other_dev);
    println!("raízes de enxerto puladas (blocos contados no *_Cryptex.dmg): {}", all.graft_roots.len());
    if std::env::var_os("SDC_GRAFT_DEBUG").is_some() {
        for p in &all.graft_roots { eprintln!("G\t{}", p.display()); }
    }
    let mut errs: Vec<_> = all.errors.iter().collect();
    errs.sort();
    println!("erros por errno {:?}", errs);
    println!();
    println!("container usado (statfs)                {:>9.2} GiB", g(container_used));
    println!("volume usado (ATTR_VOL_SPACEUSED)       {:>9.2} GiB", g(vol_used));
    println!("alocado único medido                    {:>9.2} GiB", g(total.alloc));
    println!("diferença (volume - medido)             {:>9.2} GiB", g(vol_used - total.alloc));
    println!("  hardlinks repetidos (não somados)     {:>9.2} GiB", g(link_dup));
    println!("  clones completos repetidos            {:>9.2} GiB", g(clone_dup));
    println!("privado (libera ao apagar tudo)         {:>9.2} GiB", g(total.private));
    println!("preso em snapshot (sem clone)           {:>9.2} GiB", g(total.snap));
    println!("marcado purgeable (EF_IS_PURGEABLE)     {:>9.2} GiB", g(total.purgeable));
    println!("dataless {} arquivos, {:.2} GiB aparentes; {} pastas dataless não abertas", total.dataless_n, g(total.dataless_logical), all.dataless_dirs);
    println!("esparsos {:.2} GiB aparentes, {:.2} GiB alocados", g(total.sparse_logical), g(total.sparse_alloc));
    if MEASURE_XATTR.load(std::sync::atomic::Ordering::Relaxed) {
        println!("xattrs (sem ResourceFork) {:.2} GiB em {} objetos; > 3804 B (fluxo próprio): {:.2} GiB", g(total.xattr_all), total.xattr_files, g(total.xattr_stream));
        let mut xv: Vec<_> = all.buckets.iter().filter(|(_, s)| s.xattr_stream > 0).collect();
        xv.sort_by_key(|(_, s)| -s.xattr_stream);
        for (k, s) in xv.iter().take(25) {
            println!("   xattr {:>8.3} GiB  {}", g(s.xattr_stream), k);
        }
    }
    println!();
    println!("{:>9} {:>9} {:>9}  balde", "alocado", "privado", "snapshot");
    for (k, s) in v.iter().take(60) {
        println!("{:>9.2} {:>9.2} {:>9.2}  {}", g(s.alloc), g(s.private), g(s.snap), k);
    }
    if std::env::var_os("SDC_DENIED").is_some() {
        println!("\nnegados (primeiros 200):");
        all.denied.sort();
        for d in all.denied.iter().take(200) {
            println!("  {}", d.display());
        }
    }
}

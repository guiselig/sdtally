//! Frente L: varredura com deduplicação física exata e detecção de enxerto (graft).
//! Uso: frentel <raiz> [threads] [profundidade_do_balde]
//!
//! Derivado de probe/src/bin/walk_par.rs (mesma travessia getattrlistbulk, sem root).
//! Diferenças:
//! 1. Enxerto: pasta com cloneid != fileid é raiz de graft (o cloneid dela é 2, a raiz do
//!    fs enxertado). Não desce; conta à parte. SDC_GRAFT_WALK=1 desce e soma o conteúdo
//!    à parte (nunca no total), para conferir.
//! 2. Candidato = arquivo com EF_MAY_SHARE_BLOCKS ou cloneid != fileid. Para cada candidato,
//!    F_LOG2PHYS_EXT lista as faixas físicas (fork de recurso se comprimido); a soma usa a
//!    união das faixas. Quem não mapeia cai no método antigo (alloc, clone completo 1x).
//! Só leitura: open(O_RDONLY) + fcntl de consulta. Nada é gravado.

use std::collections::HashMap;
use std::ffi::{CStr, CString, OsStr};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

const SF_DATALESS: u32 = 0x4000_0000;
const UF_COMPRESSED: u32 = 0x20;
const ATTR_CMN_ERROR: u32 = 0x2000_0000;
const EF_MAY_SHARE_BLOCKS: u64 = 0x01;
const EF_SHARES_ALL_BLOCKS: u64 = 0x40;
const VREG: u32 = 1;
const VDIR: u32 = 2;
const BLK: u64 = 4096;
const F_LOG2PHYS_EXT: i32 = 65;
const IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES: i32 = 3;
const IOPOL_SCOPE_PROCESS: i32 = 0;
const IOPOL_MATERIALIZE_DATALESS_FILES_OFF: i32 = 1;
unsafe extern "C" {
    fn setiopolicy_np(iotype: i32, scope: i32, policy: i32) -> i32;
    fn fsgetpath(buf: *mut libc::c_char, size: usize, fsid: *mut libc::fsid_t, objid: u64) -> isize;
}

#[repr(C, packed(4))] // <sys/fcntl.h> usa #pragma pack(4): 20 bytes
struct Log2Phys {
    flags: u32,
    contigbytes: i64,
    devoffset: i64,
}

/// Categoria do candidato, para dizer de onde vem a economia.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Kind {
    CloneAll,   // EF_SHARES_ALL_BLOCKS
    ClonePart,  // MAY_SHARE sem SHARES_ALL
    CloneIdOnly, // cloneid != fileid sem MAY_SHARE (clone cujo sinalizador foi limpo)
    Plain,       // só com SDC_MAP_ALL=1: arquivo comum mapeado, para prova contra a verdade total
    Source,      // arquivo comum cujo fileid é o cloneid de algum candidato (origem do clone)
}

struct Cand {
    path: PathBuf,
    bucket: Arc<str>,
    alloc: u64,
    cloneid: u64,
    kind: Kind,
    compressed: bool,
    dataless: bool,
}

#[derive(Default)]
struct Local {
    buckets: HashMap<Arc<str>, u64>,
    // ponytail: mapa de todos os arquivos comuns (~40 B cada, ~80 MiB em 2 mi); vetor ordenado se pesar.
    plain: HashMap<u64, (Arc<str>, u64)>, // fileid -> (balde, alloc), não candidatos, 1 link
    links: HashMap<u64, (Arc<str>, u64, u64)>, // fileid -> (balde, alloc, ocorrências), não candidatos
    cands: HashMap<u64, Cand>,                  // fileid -> candidato (hardlink 1x)
    files: u64,
    dirs: u64,
    errors: HashMap<i32, u64>,
    other_dev: u64,
    dataless_dirs: u64,
    graft_roots: Vec<(PathBuf, u64, u64)>, // (caminho, fileid, cloneid)
    graft_files: u64,
    graft_alloc: u64,
}

type Job = (PathBuf, Arc<str>, usize, bool); // (pasta, balde, profundidade, dentro de graft)

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
    al.fileattr = libc::ATTR_FILE_LINKCOUNT | libc::ATTR_FILE_ALLOCSIZE;
    al.forkattr = libc::ATTR_CMNEXT_CLONEID | libc::ATTR_CMNEXT_EXT_FLAGS;
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

fn scan_dir(job: &Job, bucket_depth: usize, root_dev: u32, buf: &mut [u8], l: &mut Local, push: &mut Vec<Job>) {
    let (dir, bucket, depth, in_graft) = job;
    let mut in_graft = *in_graft;
    let in_graft = &mut in_graft;
    let c = CString::new(dir.as_os_str().as_bytes()).unwrap();
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW) };
    if fd < 0 {
        let e = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        *l.errors.entry(e).or_default() += 1;
        return;
    }
    l.dirs += 1;
    // A listagem do pai mostra a pasta coberta (do host, cloneid == fileid); só a pasta aberta
    // revela a raiz do graft (cloneid 2 = raiz do fs enxertado). Uma chamada por pasta.
    // A raiz do volume Data também tem cloneid 2 e fileid sintético (0x0FFFFFFF00000002):
    // não testar a raiz da travessia.
    if !*in_graft && *depth > 0 {
        let (id, cid) = dir_ids(fd);
        if cid != 0 && cid != id {
            l.graft_roots.push((dir.clone(), id, cid));
            *in_graft = true;
            if std::env::var_os("SDC_GRAFT_WALK").is_none() {
                unsafe { libc::close(fd) };
                return;
            }
        }
    }
    let mut al = attrlist();
    let opts = (libc::FSOPT_NOFOLLOW | libc::FSOPT_ATTR_CMN_EXTENDED) as u64;
    loop {
        let n = unsafe { libc::getattrlistbulk(fd, &mut al as *mut _ as *mut _, buf.as_mut_ptr() as *mut _, buf.len(), opts) };
        if n <= 0 {
            if n < 0 {
                let e = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
                *l.errors.entry(e).or_default() += 1;
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
            let name_ref = p;
            let off = i32::from_ne_bytes(buf[p..p + 4].try_into().unwrap());
            p += 8;
            let name = unsafe { CStr::from_ptr(buf.as_ptr().add((name_ref as isize + off as isize) as usize) as *const _) };
            let dev = if ret_cmn & libc::ATTR_CMN_DEVID != 0 { rd32(buf, &mut p) } else { 0 };
            let objtype = if ret_cmn & libc::ATTR_CMN_OBJTYPE != 0 { rd32(buf, &mut p) } else { 0 };
            let flags = if ret_cmn & libc::ATTR_CMN_FLAGS != 0 { rd32(buf, &mut p) } else { 0 };
            let fileid = if ret_cmn & libc::ATTR_CMN_FILEID != 0 { rd64(buf, &mut p) } else { 0 };
            if ret_cmn & ATTR_CMN_ERROR != 0 {
                let e = rd32(buf, &mut p) as i32;
                if e != 0 {
                    *l.errors.entry(e).or_default() += 1;
                    continue;
                }
            }
            // Ordem no buffer: comuns, (pasta nada), arquivo, cmnext.
            let links = if ret_file & libc::ATTR_FILE_LINKCOUNT != 0 { rd32(buf, &mut p) } else { 1 };
            let alloc = if ret_file & libc::ATTR_FILE_ALLOCSIZE != 0 { rd64(buf, &mut p) } else { 0 };
            let cloneid = if ret_ext & libc::ATTR_CMNEXT_CLONEID != 0 { rd64(buf, &mut p) } else { 0 };
            let ext = if ret_ext & libc::ATTR_CMNEXT_EXT_FLAGS != 0 { rd64(buf, &mut p) } else { 0 };
            let path = || dir.join(OsStr::from_bytes(name.to_bytes()));

            if objtype == VDIR {
                if dev != root_dev {
                    l.other_dev += 1;
                    continue;
                }
                if flags & SF_DATALESS != 0 {
                    l.dataless_dirs += 1;
                    continue;
                }
                // Pasta dentro de graft também tem cloneid != fileid (id interno do fs enxertado).
                let graft_root = !*in_graft && cloneid != 0 && cloneid != fileid;
                if graft_root {
                    l.graft_roots.push((path(), fileid, cloneid));
                    if std::env::var_os("SDC_GRAFT_WALK").is_none() {
                        continue;
                    }
                }
                let b = if depth < &bucket_depth {
                    Arc::from(format!("{}/{}", bucket, name.to_string_lossy()).trim_start_matches('/'))
                } else {
                    bucket.clone()
                };
                push.push((path(), b, depth + 1, *in_graft || graft_root));
            } else if objtype == VREG {
                if *in_graft {
                    l.graft_files += 1;
                    l.graft_alloc += alloc;
                    continue;
                }
                l.files += 1;
                let kind = if ext & EF_SHARES_ALL_BLOCKS != 0 {
                    Some(Kind::CloneAll)
                } else if ext & EF_MAY_SHARE_BLOCKS != 0 {
                    Some(Kind::ClonePart)
                } else if cloneid != 0 && cloneid != fileid {
                    Some(Kind::CloneIdOnly)
                } else if std::env::var_os("SDC_MAP_ALL").is_some() {
                    Some(Kind::Plain)
                } else {
                    None
                };
                if let Some(kind) = kind {
                    l.cands.entry(fileid).or_insert_with(|| Cand {
                        path: path(),
                        bucket: bucket.clone(),
                        alloc,
                        cloneid,
                        kind,
                        compressed: flags & UF_COMPRESSED != 0,
                        dataless: flags & SF_DATALESS != 0,
                    });
                } else if links > 1 {
                    l.links.entry(fileid).or_insert((bucket.clone(), alloc, 0)).2 += 1;
                } else {
                    *l.buckets.entry(bucket.clone()).or_default() += alloc;
                    l.plain.insert(fileid, (bucket.clone(), alloc));
                }
            }
        }
    }
    unsafe { libc::close(fd) };
}

/// Faixas físicas [início, fim) de um arquivo, em blocos de 4 KiB. Err(errno) se não mapeia.
/// Comprimido: os dados estão no fork de recurso; o fork de dados dá ENOTSUP.
fn extents(c: &Cand, fcntls: &AtomicUsize) -> Result<Vec<(u64, u64)>, i32> {
    let mut p = c.path.as_os_str().as_bytes().to_vec();
    if c.compressed {
        p.extend_from_slice(b"/..namedfork/rsrc");
    }
    let cp = CString::new(p).unwrap();
    let fd = unsafe { libc::open(cp.as_ptr(), libc::O_RDONLY | libc::O_NOFOLLOW) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(0));
    }
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    unsafe { libc::fstat(fd, &mut st) };
    let size = st.st_size as i64;
    let mut out = Vec::new();
    let mut off = 0i64;
    let mut res = Ok(());
    while off < size {
        let mut l = Log2Phys { flags: 0, contigbytes: size - off, devoffset: off };
        fcntls.fetch_add(1, Ordering::Relaxed);
        if unsafe { libc::fcntl(fd, F_LOG2PHYS_EXT, &mut l) } < 0 {
            res = Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(0));
            break;
        }
        if l.contigbytes <= 0 {
            break;
        }
        if l.devoffset >= 0 {
            // Faixa física em bytes; o fim do arquivo não é múltiplo de bloco: arredonda.
            let s = l.devoffset as u64;
            out.push((s, (s + l.contigbytes as u64).div_ceil(BLK) * BLK));
        } // devoffset -1 = buraco (esparso)
        off += l.contigbytes;
    }
    unsafe { libc::close(fd) };
    res.map(|_| out)
}

/// Soma a união das faixas; cada trecho novo vai para o primeiro dono na ordem física.
/// Devolve bytes novos por candidato.
fn union(mut ex: Vec<(u64, u64, u32)>, n: usize) -> Vec<u64> {
    ex.sort_unstable();
    let mut fresh = vec![0u64; n];
    let mut covered = 0u64;
    for (s, e, i) in ex {
        let from = s.max(covered);
        if e > from {
            fresh[i as usize] += e - from;
            covered = e;
        }
    }
    fresh
}

fn self_check() {
    // a:[0,8) b:[4,12) c:[20,24) d:[0,8) => união 16
    let f = union(vec![(0, 8, 0), (4, 12, 1), (20, 24, 2), (0, 8, 3)], 4);
    assert_eq!(f, vec![8, 4, 4, 0]);
    assert_eq!(f.iter().sum::<u64>(), 16);
}

fn main() {
    self_check();
    let mut args = std::env::args_os().skip(1);
    let root = PathBuf::from(args.next().expect("uso: frentel <raiz> [threads] [prof]"));
    let threads: usize = args.next().and_then(|t| t.to_str()?.parse().ok()).unwrap_or(10);
    let bucket_depth: usize = args.next().and_then(|t| t.to_str()?.parse().ok()).unwrap_or(2);
    let root_dev = {
        use std::os::unix::fs::MetadataExt;
        std::fs::symlink_metadata(&root).expect("raiz inacessível").dev() as u32
    };
    let rc = unsafe { setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS, IOPOL_MATERIALIZE_DATALESS_FILES_OFF) };
    assert_eq!(rc, 0, "setiopolicy_np falhou");

    // Fase 1: travessia (igual ao walk_par).
    let q = Arc::new((Mutex::new((vec![(root.clone(), Arc::<str>::from(""), 0usize, false)], 1usize)), Condvar::new()));
    let t0 = Instant::now();
    let hs: Vec<_> = (0..threads)
        .map(|_| {
            let q = q.clone();
            std::thread::spawn(move || {
                let mut l = Local::default();
                let mut buf = vec![0u8; 256 * 1024];
                let mut push = Vec::new();
                loop {
                    let job = {
                        let mut g = q.0.lock().unwrap();
                        loop {
                            if let Some(j) = g.0.pop() {
                                break Some(j);
                            }
                            if g.1 == 0 {
                                break None;
                            }
                            g = q.1.wait(g).unwrap();
                        }
                    };
                    let Some(job) = job else {
                        q.1.notify_all();
                        return l;
                    };
                    scan_dir(&job, bucket_depth, root_dev, &mut buf, &mut l, &mut push);
                    let mut g = q.0.lock().unwrap();
                    g.1 += push.len();
                    g.1 -= 1;
                    g.0.append(&mut push);
                    drop(g);
                    q.1.notify_all();
                }
            })
        })
        .collect();
    let mut all = Local::default();
    let mut links: HashMap<u64, (Arc<str>, u64, u64)> = HashMap::new();
    for h in hs {
        let l = h.join().unwrap();
        for (k, v) in l.buckets {
            *all.buckets.entry(k).or_default() += v;
        }
        for (k, v) in l.links {
            links.entry(k).or_insert((v.0, v.1, 0)).2 += v.2;
        }
        all.plain.extend(l.plain);
        for (k, v) in l.cands {
            all.cands.entry(k).or_insert(v);
        }
        all.files += l.files;
        all.dirs += l.dirs;
        all.other_dev += l.other_dev;
        all.dataless_dirs += l.dataless_dirs;
        all.graft_roots.extend(l.graft_roots);
        all.graft_files += l.graft_files;
        all.graft_alloc += l.graft_alloc;
        for (k, v) in l.errors {
            *all.errors.entry(k).or_default() += v;
        }
    }
    for (id, (b, alloc, _)) in links {
        *all.buckets.entry(b.clone()).or_default() += alloc;
        all.plain.insert(id, (b, alloc));
    }
    // Origem de clone sem sinalizador: o cloneid de um candidato é o fileid de um arquivo comum.
    // Vira candidato (Source) para a união física enxergar o compartilhamento.
    let mut fsid: libc::statfs = unsafe { std::mem::zeroed() };
    let croot = CString::new(root.as_os_str().as_bytes()).unwrap();
    unsafe { libc::statfs(croot.as_ptr(), &mut fsid) };
    let wanted: Vec<u64> = all.cands.values().map(|c| c.cloneid).filter(|id| all.plain.contains_key(id)).collect();
    let mut sources = 0u64;
    for id in wanted {
        let Some((b, alloc)) = all.plain.remove(&id) else { continue };
        let mut pb = vec![0u8; 4096];
        let n = unsafe { fsgetpath(pb.as_mut_ptr() as *mut _, pb.len(), &mut fsid.f_fsid, id) };
        if n <= 0 {
            continue; // fica como arquivo comum
        }
        let rel = unsafe { CStr::from_ptr(pb.as_ptr() as *const _) }.to_bytes().to_vec();
        // fsgetpath devolve o caminho pelo firmlink ("/System/...") ou já com o mount; tenta os dois.
        let mut full = unsafe { CStr::from_ptr(fsid.f_mntonname.as_ptr()) }.to_bytes().to_vec();
        full.extend_from_slice(&rel);
        let Some((path, md)) = [PathBuf::from(OsStr::from_bytes(&rel)), PathBuf::from(OsStr::from_bytes(&full))]
            .into_iter()
            .find_map(|p| std::fs::symlink_metadata(&p).ok().map(|m| (p, m)))
        else {
            continue;
        };
        use std::os::macos::fs::MetadataExt;
        let compressed = md.st_flags() & UF_COMPRESSED != 0;
        *all.buckets.get_mut(&b).unwrap() -= alloc;
        sources += 1;
        all.cands.insert(id, Cand { path, bucket: b, alloc, cloneid: id, kind: Kind::Source, compressed, dataless: false });
    }
    let t_walk = t0.elapsed().as_secs_f64();
    let plain: u64 = all.buckets.values().sum();

    // Fase 2: mapear os candidatos.
    let cands: Vec<Cand> = all.cands.into_values().collect();
    let t1 = Instant::now();
    let next = AtomicUsize::new(0);
    let fcntls = AtomicUsize::new(0);
    let mapped: Vec<Result<Vec<(u64, u64)>, i32>> = {
        let slots: Vec<Mutex<Option<Result<Vec<(u64, u64)>, i32>>>> = (0..cands.len()).map(|_| Mutex::new(None)).collect();
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= cands.len() {
                        break;
                    }
                    let c = &cands[i];
                    let r = if c.dataless || c.alloc == 0 { Ok(Vec::new()) } else { extents(c, &fcntls) };
                    *slots[i].lock().unwrap() = Some(r);
                });
            }
        });
        slots.into_iter().map(|m| m.into_inner().unwrap().unwrap()).collect()
    };
    let t_map = t1.elapsed().as_secs_f64();

    // Candidatos que não mapeiam: método antigo (clone completo 1x por cloneid, senão alloc).
    let mut ex = Vec::new();
    let mut map_err: HashMap<i32, (u64, u64)> = HashMap::new();
    let mut fallback_clone: HashMap<u64, u64> = HashMap::new();
    let mut fallback = vec![0u64; cands.len()];
    let mut mapped_sum = vec![0u64; cands.len()];
    for (i, (c, r)) in cands.iter().zip(&mapped).enumerate() {
        match r {
            Ok(v) => {
                for &(s, e) in v {
                    ex.push((s, e, i as u32));
                    mapped_sum[i] += e - s;
                }
            }
            Err(e) => {
                let m = map_err.entry(*e).or_default();
                m.0 += 1;
                m.1 += c.alloc;
                if c.kind == Kind::CloneAll {
                    if fallback_clone.insert(c.cloneid, c.alloc).is_none() {
                        fallback[i] = c.alloc;
                    }
                } else {
                    fallback[i] = c.alloc;
                }
            }
        }
    }
    let n_ext = ex.len();
    let fresh = union(ex, cands.len());

    // Por categoria: alloc ingênuo, alloc com clone completo 1x (walk_par), físico.
    #[derive(Default)]
    struct K {
        n: u64,
        alloc: u64,
        walkpar: u64,
        phys: u64,
        beyond: u64,
    }
    let mut by: HashMap<Kind, K> = HashMap::new();
    let mut seen_clone: HashMap<u64, ()> = HashMap::new();
    let mut phys_bucket: HashMap<Arc<str>, i64> = HashMap::new();
    for (i, c) in cands.iter().enumerate() {
        let k = by.entry(c.kind).or_default();
        k.n += 1;
        k.alloc += c.alloc;
        if c.kind != Kind::CloneAll || seen_clone.insert(c.cloneid, ()).is_none() {
            k.walkpar += c.alloc;
        }
        // alloc além do mapeado (pré-alocação após o EOF): conta como privado.
        let beyond = if mapped[i].is_ok() { c.alloc.saturating_sub(mapped_sum[i]) } else { 0 };
        k.beyond += beyond;
        let p = fresh[i] + fallback[i] + beyond;
        k.phys += p;
        *phys_bucket.entry(c.bucket.clone()).or_default() += p as i64 - c.alloc as i64;
    }

    let g = |b: u64| b as f64 / (1u64 << 30) as f64;
    let vol_used = {
        let mut st: libc::statfs = unsafe { std::mem::zeroed() };
        let c = CString::new(root.as_os_str().as_bytes()).unwrap();
        unsafe { libc::statfs(c.as_ptr(), &mut st) };
        let mut al: libc::attrlist = unsafe { std::mem::zeroed() };
        al.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
        al.volattr = libc::ATTR_VOL_INFO | libc::ATTR_VOL_SPACEUSED;
        let mut b = [0u8; 64];
        let rc = unsafe { libc::getattrlist(st.f_mntonname.as_ptr(), &mut al as *mut _ as *mut _, b.as_mut_ptr() as *mut _, b.len(), 0) };
        assert_eq!(rc, 0);
        u64::from_ne_bytes(b[4..12].try_into().unwrap())
    };
    let tot = |f: fn(&K) -> u64| by.values().map(f).sum::<u64>();
    let (c_alloc, c_walk, c_phys) = (tot(|k| k.alloc), tot(|k| k.walkpar), tot(|k| k.phys));

    println!("raiz {} | {threads} threads | travessia {t_walk:.1} s | mapeamento {t_map:.1} s", root.display());
    println!("pastas {} | arquivos {} | outros volumes {} | pastas dataless {}", all.dirs, all.files, all.other_dev, all.dataless_dirs);
    let mut errs: Vec<_> = all.errors.iter().collect();
    errs.sort();
    println!("erros da travessia por errno {:?}", errs);
    println!("enxertos: {} raízes; conteúdo {} arquivos {:.3} GiB (fora do total{})", all.graft_roots.len(), all.graft_files, g(all.graft_alloc),
        if std::env::var_os("SDC_GRAFT_WALK").is_some() { "" } else { "; não percorrido, use SDC_GRAFT_WALK=1" });
    all.graft_roots.sort();
    for (p, id, cid) in &all.graft_roots {
        println!("   graft fileid={id} cloneid={cid}  {}", p.display());
    }
    println!("origens de clone promovidas: {sources}");
    println!("candidatos {} | faixas {} | fcntl {} | {:.1} µs por candidato",
        cands.len(), n_ext, fcntls.load(Ordering::Relaxed), t_map * 1e6 / cands.len().max(1) as f64);
    let mut me: Vec<_> = map_err.iter().collect();
    me.sort();
    for (e, (n, b)) in me {
        println!("   não mapeou errno {e}: {n} arquivos, {:.3} GiB (método antigo)", g(*b));
    }
    println!();
    println!("{:<34} {:>9} {:>11} {:>11} {:>11} {:>9}", "categoria", "arquivos", "alloc GiB", "walk_par", "físico", "pós-EOF");
    let mut ks: Vec<_> = by.iter().collect();
    ks.sort_by_key(|(k, _)| format!("{k:?}"));
    for (k, v) in ks {
        println!("{:<34} {:>9} {:>11.3} {:>11.3} {:>11.3} {:>9.3}", format!("{k:?}"), v.n, g(v.alloc), g(v.walkpar), g(v.phys), g(v.beyond));
    }
    println!();
    println!("volume usado (ATTR_VOL_SPACEUSED)          {:>10.3} GiB", g(vol_used));
    println!("não candidatos (hardlink 1x)               {:>10.3} GiB", g(plain));
    println!("total ingênuo (alloc de tudo)              {:>10.3} GiB", g(plain + c_alloc));
    println!("total walk_par (clone completo 1x)         {:>10.3} GiB", g(plain + c_walk));
    println!("total físico (união de faixas)             {:>10.3} GiB", g(plain + c_phys));
    println!("  físico − walk_par                        {:>10.3} GiB", (c_phys as f64 - c_walk as f64) / (1u64 << 30) as f64);
    println!("  volume − físico                          {:>10.3} GiB", (vol_used as f64 - (plain + c_phys) as f64) / (1u64 << 30) as f64);
    println!("bytes exatos: vol={vol_used} plain={plain} cand_alloc={c_alloc} cand_walkpar={c_walk} cand_fisico={c_phys} graft={}", all.graft_alloc);
    let mut pb: Vec<_> = phys_bucket.into_iter().filter(|(_, d)| *d != 0).collect();
    pb.sort_by_key(|(_, d)| *d);
    println!("\nbaldes que mais mudam (físico − alloc):");
    for (b, d) in pb.iter().take(25) {
        println!("  {:>9.3} GiB  {}", *d as f64 / (1u64 << 30) as f64, b);
    }
}

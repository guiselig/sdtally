//! Varre um volume sem cruzar para outros (st_dev + pontos de montagem) e lista
//! as pastas que não abriram, com errno. Serve para responder "o que fica de fora
//! neste nível de privilégio". Soma alocado (st_blocks×512) com hardlink uma vez.
//! Não deduplica clones: para isso usar experimentos/probe (walk).
//!
//! Uso: errwalk [raiz]   (padrão /System/Volumes/Data)
//! Saída: resumo em stdout; cada pasta negada em stderr ("errno\tcaminho").

use std::collections::{BTreeMap, HashSet};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).unwrap_or("/System/Volumes/Data".into()));
    let root_dev = std::fs::symlink_metadata(&root).expect("raiz").dev();
    // Pontos de montagem abaixo da raiz (autofs /home tem outro dev; snapshots do TM, /Volumes/*).
    let mut mounts: HashSet<PathBuf> = HashSet::new();
    let mut p: *mut libc::statfs = std::ptr::null_mut();
    let n = unsafe { libc::getmntinfo(&mut p, libc::MNT_NOWAIT) };
    for m in unsafe { std::slice::from_raw_parts(p, n as usize) } {
        let on = unsafe { std::ffi::CStr::from_ptr(m.f_mntonname.as_ptr()) }.to_string_lossy().into_owned();
        mounts.insert(PathBuf::from(&on));
        // /Volumes/X também aparece como /System/Volumes/Data/Volumes/X (firmlink).
        if let Some(rest) = on.strip_prefix('/') { mounts.insert(PathBuf::from("/System/Volumes/Data").join(rest)); }
    }
    mounts.remove(&root);

    let mut stack = vec![root.clone()];
    let mut seen: HashSet<(u64, u64)> = HashSet::new();
    let (mut files, mut dirs, mut alloc, mut hl_dup, mut other_dev) = (0u64, 0u64, 0i64, 0i64, 0u64);
    let mut denied: BTreeMap<String, u64> = BTreeMap::new();
    let mut per_top: BTreeMap<String, i64> = BTreeMap::new();
    while let Some(dir) = stack.pop() {
        let rd = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(e) => {
                let name = match e.raw_os_error() { Some(libc::EPERM) => "EPERM", Some(libc::EACCES) => "EACCES", _ => "OUTRO" };
                *denied.entry(name.into()).or_default() += 1;
                eprintln!("{name}\t{}", dir.display());
                continue;
            }
        };
        for ent in rd.flatten() {
            let path = ent.path();
            let Ok(md) = std::fs::symlink_metadata(&path) else { continue };
            if md.dev() != root_dev || mounts.contains(&path) { other_dev += 1; continue }
            let top = path.strip_prefix(&root).ok().and_then(|r| r.components().next())
                .map(|c| c.as_os_str().to_string_lossy().into_owned()).unwrap_or_default();
            if md.is_dir() { dirs += 1; stack.push(path); continue }
            files += 1;
            let b = md.blocks() as i64 * 512;
            if md.nlink() > 1 && !seen.insert((md.dev(), md.ino())) { hl_dup += b; continue }
            alloc += b;
            *per_top.entry(top).or_default() += b;
        }
    }
    let g = |b: i64| b as f64 / (1u64 << 30) as f64;
    println!("raiz {}  arquivos {files}  pastas {dirs}  entradas de outro volume puladas {other_dev}", root.display());
    println!("alocado (hardlink 1×, clone NÃO dedup) {:.3} GiB   hardlinks repetidos {:.3} GiB", g(alloc), g(hl_dup));
    println!("pastas negadas: {denied:?}");
    let mut v: Vec<_> = per_top.into_iter().collect();
    v.sort_by_key(|(_, b)| -b);
    for (t, b) in v.iter().take(20) { println!("  {:>9.3} GiB  {t}", g(*b)) }
}

//! Experimento: velocidade do dua-core (paralelo, getattrlistbulk) contra o walk próprio.
//! Uso: dua_bench <diretório> <threads>

use std::time::Instant;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let root = std::path::PathBuf::from(args.next().expect("uso: dua_bench <dir> <threads>"));
    let threads: usize = args.next().and_then(|t| t.to_str()?.parse().ok()).unwrap_or(1);

    let mut options = dua_core::Options::default();
    options.apfs_clone_metadata = true;
    let start = Instant::now();
    let (mut files, mut errors, mut alloc) = (0u64, 0u64, 0u64);
    for entry in dua_core::walk(&root, threads, dua_core::Order::Completion, options, |_| true) {
        match entry.map(|e| e.metadata) {
            Ok(Some(Ok(m))) if m.is_file() => {
                files += 1;
                alloc += m.allocated_size();
            }
            Ok(Some(Ok(_))) => {}
            _ => errors += 1,
        }
    }
    println!(
        "threads {threads:>2} | {:.1} s | {files} arquivos | {errors} erros | {:.2} GiB alocados (sem dedup)",
        start.elapsed().as_secs_f64(),
        alloc as f64 / (1u64 << 30) as f64
    );
}

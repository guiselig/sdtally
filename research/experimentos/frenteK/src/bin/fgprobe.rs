//! Frente K: sonda de "primeiro plano". Mede, durante <segundos>, a latência que um processo
//! comum (QoS padrão, I/O IMPORTANT) sente enquanto a varredura roda ao lado.
//! Uso: fgprobe <arquivo_de_teste> <segundos>
//!   - agenda: dorme 2 ms e mede o atraso ao acordar (CPU disputada);
//!   - disco: pread de 4 KiB em offset aleatório com F_NOCACHE (vai ao SSD, sem cache).
//! Cria o arquivo de teste (1 GiB) se não existir. Não toca em nada fora dele.

use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::time::{Duration, Instant};

const SIZE: u64 = 1 << 30;

fn pct(v: &mut Vec<f64>) -> String {
    if v.is_empty() {
        return "sem amostras".into();
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p = |q: f64| v[((v.len() - 1) as f64 * q) as usize];
    format!("n {} p50 {:.3} p95 {:.3} p99 {:.3} max {:.3} ms", v.len(), p(0.5), p(0.95), p(0.99), v[v.len() - 1])
}

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("uso: fgprobe <arquivo> <segundos>");
    let secs: u64 = a.next().and_then(|s| s.parse().ok()).unwrap_or(15);
    if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) < SIZE {
        let mut f = std::fs::File::create(&path).unwrap();
        let mut blk = vec![0u8; 1 << 20];
        for (i, b) in blk.iter_mut().enumerate() {
            *b = (i * 131 % 251) as u8;
        }
        for i in 0..(SIZE >> 20) {
            blk[0] = i as u8;
            f.write_all(&blk).unwrap();
        }
        f.sync_all().unwrap();
    }
    let end = Instant::now() + Duration::from_secs(secs);

    let sched = std::thread::spawn(move || {
        let mut v = Vec::new();
        while Instant::now() < end {
            let t = Instant::now();
            std::thread::sleep(Duration::from_millis(2));
            v.push((t.elapsed().as_secs_f64() - 0.002) * 1e3);
        }
        v
    });
    let disk = std::thread::spawn(move || {
        let f = std::fs::File::open(&path).unwrap();
        unsafe { libc::fcntl(f.as_raw_fd(), libc::F_NOCACHE, 1) };
        let mut buf = vec![0u8; 4096];
        let mut x: u64 = 0x9E3779B97F4A7C15;
        let mut v = Vec::new();
        while Instant::now() < end {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let off = (x % (SIZE / 4096)) * 4096;
            let t = Instant::now();
            let n = unsafe { libc::pread(f.as_raw_fd(), buf.as_mut_ptr() as *mut _, 4096, off as i64) };
            assert_eq!(n, 4096);
            v.push(t.elapsed().as_secs_f64() * 1e3);
            std::thread::sleep(Duration::from_millis(1));
        }
        v
    });
    println!("agenda (atraso ao acordar de 2 ms): {}", pct(&mut sched.join().unwrap()));
    println!("disco (pread 4 KiB F_NOCACHE):     {}", pct(&mut disk.join().unwrap()));
}

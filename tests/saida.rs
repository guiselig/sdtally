//! Saída em stdout: leitor que fecha o pipe cedo (`sdtally --catalog --json | head -c 10`) não vira
//! panic (saída 101); o sdtally encerra com 0.

use std::io::Read;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_sdtally");

#[test]
fn pipe_fechado_sai_com_zero() {
    let mut c = Command::new(BIN).args(["--catalog", "--json"]).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut o = c.stdout.take().unwrap();
    let mut b = [0u8; 10];
    o.read_exact(&mut b).unwrap();
    // O catálogo (~800 KB) não cabe no buffer do pipe: a escrita seguinte dá EPIPE.
    drop(o);
    let s = c.wait_with_output().unwrap();
    assert_eq!(s.status.code(), Some(0), "{}", String::from_utf8_lossy(&s.stderr));
}

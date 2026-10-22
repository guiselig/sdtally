//! AES-128-ECB e zlib do sistema, para ler o `Library.musicdb` (só leitura, sem dependência nova).

use std::ffi::c_int;

unsafe extern "C" {
    // CommonCrypto, em libSystem.
    fn CCCrypt(op: u32, alg: u32, opts: u32, key: *const u8, klen: usize, iv: *const u8, din: *const u8, nin: usize, dout: *mut u8, nout: usize, moved: *mut usize) -> c_int;
}

#[link(name = "z")]
unsafe extern "C" {
    fn uncompress(d: *mut u8, dl: *mut libc::c_ulong, s: *const u8, sl: libc::c_ulong) -> c_int;
}

/// Decifra `dados` (múltiplo de 16) sem padding. kCCDecrypt = 1, kCCAlgorithmAES128 = 0,
/// kCCOptionECBMode = 2.
pub fn aes128_ecb(chave: &[u8; 16], dados: &[u8]) -> Result<Vec<u8>, String> {
    if !dados.len().is_multiple_of(16) {
        return Err("AES: tamanho não é múltiplo de 16".into());
    }
    let mut saida = vec![0u8; dados.len() + 16];
    let mut feito = 0usize;
    let r = unsafe { CCCrypt(1, 0, 2, chave.as_ptr(), 16, std::ptr::null(), dados.as_ptr(), dados.len(), saida.as_mut_ptr(), saida.len(), &mut feito) };
    if r != 0 {
        return Err(format!("CCCrypt {r}"));
    }
    saida.truncate(feito);
    Ok(saida)
}

/// Teto da saída do `inflate`.
const TETO: usize = 1 << 32;

/// Próximo tamanho da saída depois de um Z_BUF_ERROR: ×4 até o teto, nunca acima; no teto, None.
fn crescer(cap: usize) -> Option<usize> {
    (cap < TETO).then(|| cap.saturating_mul(4).min(TETO))
}

/// zlib com cabeçalho; dado depois do fim do fluxo é ignorado. Teto de 4 GiB na saída.
pub fn inflate(s: &[u8]) -> Result<Vec<u8>, String> {
    let mut cap = s.len().saturating_mul(8).saturating_add(4096).min(TETO);
    loop {
        let mut saida = vec![0u8; cap];
        let mut n = cap as libc::c_ulong;
        match unsafe { uncompress(saida.as_mut_ptr(), &mut n, s.as_ptr(), s.len() as libc::c_ulong) } {
            0 => {
                saida.truncate(n as usize);
                return Ok(saida);
            }
            // Z_BUF_ERROR: a saída não coube.
            -5 => cap = crescer(cap).ok_or("zlib: saída maior que 4 GiB")?,
            e => return Err(format!("zlib {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zlib_e_aes() {
        // "hello hello hello hello" comprimido com zlib (python3 zlib.compress, nível 6).
        let z = [120, 156, 203, 72, 205, 201, 201, 87, 200, 64, 39, 1, 104, 3, 8, 177];
        assert_eq!(inflate(&z).unwrap(), b"hello hello hello hello");
        assert!(inflate(&z[..8]).is_err());
        // FIPS-197 C.1: chave 000102..0f, bloco 00112233..ff → 69c4e0d8 6a7b0430 d8cdb780 70b4c55a.
        let chave: [u8; 16] = std::array::from_fn(|i| i as u8);
        let cifrado = [0x69, 0xc4, 0xe0, 0xd8, 0x6a, 0x7b, 0x04, 0x30, 0xd8, 0xcd, 0xb7, 0x80, 0x70, 0xb4, 0xc5, 0x5a];
        let claro: Vec<u8> = (0..16).map(|i| (i * 0x11) as u8).collect();
        assert_eq!(aes128_ecb(&chave, &cifrado).unwrap(), claro);
        assert!(aes128_ecb(&chave, &cifrado[..15]).is_err());
    }

    #[test]
    fn saida_para_no_teto() {
        assert_eq!(crescer(4096), Some(16384));
        assert_eq!(crescer(3 << 30), Some(TETO));
        assert_eq!(crescer(TETO - 1), Some(TETO));
        assert_eq!(crescer(TETO), None);
    }
}

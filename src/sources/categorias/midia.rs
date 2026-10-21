//! `Library.musicdb` e `Library.tvdb` (Música e TV): "hfma" + AES-128-ECB nos primeiros
//! `max_crypt` bytes + zlib. Formato privado, lido no lugar e só leitura. O parser é puro; o que
//! sai é o `FileSize` gravado de cada faixa e o caminho do arquivo (nunca sai do processo).

use crate::ledger::Medida;
use crate::sys::cripto;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// Chave fixa do formato, igual em todas as bibliotecas.
const CHAVE: &[u8; 16] = b"BHUILuilfghuila3";
/// Maior biblioteca aceita (o arquivo vai inteiro para a memória).
const LIMITE: u64 = 512 << 20;

fn u32le(d: &[u8], o: usize) -> Option<usize> {
    Some(u32::from_le_bytes(d.get(o..o.checked_add(4)?)?.try_into().ok()?) as usize)
}

/// O conteúdo decifrado e descomprimido de um `Library.*db`.
pub fn abrir(arquivo: &[u8]) -> Result<Vec<u8>, String> {
    if arquivo.get(..4) != Some(b"hfma") {
        return Err("not an hfma file".into());
    }
    let (cab, max_crypt) = (u32le(arquivo, 4).ok_or("short header")?, u32le(arquivo, 0x54).ok_or("short header")?);
    let corpo = arquivo.get(cab..).ok_or("short header")?;
    let n = max_crypt.min(corpo.len()) / 16 * 16;
    let mut z = cripto::aes128_ecb(CHAVE, &corpo[..n])?;
    z.extend_from_slice(&corpo[n..]);
    cripto::inflate(&z)
}

/// Uma faixa: (`FileSize` gravado, caminho local se a URL é `file://`).
pub type Faixa = (Option<u64>, Option<String>);

/// Registros "itma" com seus "boma" de FileSize (tipo 1, deslocamento 316) e URL (tipo 11).
/// Estrutura fora do esperado é Err, nunca um total menor.
pub fn faixas(d: &[u8]) -> Result<Vec<Faixa>, String> {
    let mut out: Vec<Faixa> = Vec::new();
    let (mut dentro, mut i) = (false, 0usize);
    while i + 8 < d.len() {
        let etiqueta = &d[i..i + 4];
        let tam = u32le(d, i + 4).ok_or("truncated")?;
        if etiqueta == b"hsma" {
            i = i.checked_add(tam).ok_or("bad length")?;
            continue;
        }
        if etiqueta == b"boma" {
            let total = u32le(d, i + 8).ok_or("truncated")?;
            let tipo = u32le(d, i + 12).ok_or("truncated")?;
            if dentro && let Some(f) = out.last_mut() {
                if tipo == 1 && f.0.is_none() && total >= 324 {
                    f.0 = Some(u64::from_le_bytes(d.get(i + 316..i + 324).ok_or("truncated")?.try_into().map_err(|_| "truncated")?));
                } else if tipo == 11 {
                    let cru = d.get(i + 36..i.checked_add(total).ok_or("bad length")?).ok_or("truncated")?;
                    let url = String::from_utf8_lossy(cru.split(|b| *b == 0).next().unwrap_or_default()).into_owned();
                    f.1 = url.strip_prefix("file://").map(|r| desescapar(r).trim_end_matches('/').to_string());
                }
            }
            if total == 0 {
                return Err(format!("empty boma at {i}"));
            }
            i = i.checked_add(total).ok_or("bad length")?;
            continue;
        }
        dentro = etiqueta == b"itma";
        if dentro {
            out.push((None, None));
        }
        if tam < 8 {
            return Err(format!("invalid record at {i}"));
        }
        i = i.checked_add(tam).ok_or("bad length")?;
    }
    Ok(out)
}

/// %XX de URL → byte.
fn desescapar(s: &str) -> String {
    let b = s.as_bytes();
    let (mut o, mut i) = (Vec::with_capacity(b.len()), 0);
    while i < b.len() {
        if b[i] == b'%'
            && let Some(v) = s.get(i + 1..i + 3).and_then(|x| u8::from_str_radix(x, 16).ok())
        {
            o.push(v);
            i += 3;
            continue;
        }
        o.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&o).into_owned()
}

/// Σ `FileSize` das faixas com arquivo local no volume `dev` (o da home), como a extensão. Biblioteca
/// ausente é 0 medido; pacote sem o banco, formato novo: não medida.
pub fn total(banco: &Path, dev: u64) -> Medida {
    let Some(pacote) = banco.parent() else { return Medida::NaoMedida("no library path".into()) };
    if !pacote.exists() {
        return Medida::Valor(0);
    }
    let bytes = match std::fs::metadata(banco) {
        Ok(m) if m.len() <= LIMITE => std::fs::read(banco),
        Ok(_) => return Medida::NaoMedida("library file larger than 512 MiB".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Medida::NaoMedida("library package without the expected database".into()),
        Err(e) => return Medida::NaoMedida(format!("library not readable ({e})")),
    };
    let faixas = match bytes.map_err(|e| e.to_string()).and_then(|b| abrir(&b)).and_then(|d| faixas(&d)) {
        Ok(f) => f,
        Err(e) => return Medida::NaoMedida(format!("library format not recognised ({e})")),
    };
    let soma: u64 = faixas
        .iter()
        .filter_map(|(tam, p)| std::fs::metadata(p.as_deref()?).ok().filter(|m| m.dev() == dev).and(*tam))
        .sum();
    Medida::Valor(soma as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn itma() -> Vec<u8> {
        let mut d = Vec::new();
        d.extend(b"itma");
        d.extend(12u32.to_le_bytes());
        d.extend([0u8; 4]);
        d
    }

    fn boma(tipo: u32, corpo: &[u8], min: usize) -> Vec<u8> {
        let mut b = vec![0u8; 36.max(min)];
        b[..4].copy_from_slice(b"boma");
        b[12..16].copy_from_slice(&tipo.to_le_bytes());
        b.extend(corpo);
        let n = b.len() as u32;
        b[8..12].copy_from_slice(&n.to_le_bytes());
        b
    }

    #[test]
    fn faixa_sintetica() {
        let mut d = itma();
        let mut tam = boma(1, &[], 324);
        tam[316..324].copy_from_slice(&4242u64.to_le_bytes());
        d.extend(tam);
        d.extend(boma(11, b"file:///x/a%20b.m4a\0resto", 0));
        // Segunda faixa: URL que não é arquivo, sem tamanho.
        d.extend(itma());
        d.extend(boma(11, b"https://x/y\0", 0));
        assert_eq!(faixas(&d).unwrap(), vec![(Some(4242), Some("/x/a b.m4a".to_string())), (None, None)]);
    }

    #[test]
    fn estrutura_quebrada_e_err() {
        let mut d = itma();
        d.extend(boma(11, b"file:///x\0", 0));
        d.truncate(d.len() - 3);
        // Corte no meio do boma: erro, não um total menor.
        assert!(faixas(&d).is_err());
        let mut zero = itma();
        zero.extend(vec![0u8; 36]);
        zero[12..16].copy_from_slice(b"boma");
        assert!(faixas(&zero).is_err());
        assert!(abrir(b"nope").is_err());
        assert!(abrir(b"hfma").is_err());
    }

    #[test]
    fn buffers_aleatorios_nao_entram_em_panico() {
        let mut x = 0x9E37_79B9_7F4A_7C15u64;
        for n in 0..100_000usize {
            let mut d: Vec<u8> = (0..(n % 400)).map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x as u8
            }).collect();
            // Metade dos casos começa com uma etiqueta válida, para entrar nas ramas.
            if n % 2 == 0 && d.len() > 16 {
                d[..4].copy_from_slice(if n % 4 == 0 { b"boma" } else { b"itma" });
            }
            let _ = faixas(&d);
            let _ = abrir(&d);
        }
    }

    #[test]
    fn total_sem_biblioteca_e_zero() {
        let p = std::env::temp_dir().join("sdtally-sem-biblioteca-xyz/Library.musicdb");
        assert_eq!(total(&p, 0), Medida::Valor(0));
    }
}

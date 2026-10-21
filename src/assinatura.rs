//! Entitlements gravados na assinatura de um executável Mach-O, lidos direto do arquivo (sem
//! `codesign`, sem Security.framework): `LC_CODE_SIGNATURE` → SuperBlob → blob de entitlements em
//! XML (magic 0xfade7171). Só leitura; todo tamanho vem do arquivo e é conferido.

use crate::plist;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::Path;

const FAT: u32 = 0xcafe_babe;
const MACHO64: u32 = 0xfeed_facf;
const ARM64: u32 = 0x0100_000c;
const LC_CODE_SIGNATURE: u32 = 0x1d;
const SUPERBLOB: u32 = 0xfade_0cc0;
const ENTITLEMENTS_XML: u32 = 0xfade_7171;
/// Assinatura (SuperBlob) e comandos de carga acima disto são lixo.
const LIMITE: usize = 16 << 20;
const LIMITE_COMANDOS: usize = 256 << 10;
const GRUPOS: &str = "com.apple.security.application-groups";

fn be(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(i..i.checked_add(4)?)?.try_into().ok()?))
}

fn le(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i.checked_add(4)?)?.try_into().ok()?))
}

fn ler_em(f: &File, off: u64, n: usize) -> Option<Vec<u8>> {
    let mut b = vec![0u8; n];
    f.read_exact_at(&mut b, off).ok()?;
    Some(b)
}

/// Onde começa o Mach-O de 64 bits no arquivo: 0 (fino) ou o slice arm64 (fat, preferido) ou o
/// primeiro slice de 64 bits.
fn slice(f: &File) -> Option<u64> {
    let h = ler_em(f, 0, 8)?;
    match be(&h, 0)? {
        FAT => {
            let n = be(&h, 4)?.min(8) as usize;
            let t = ler_em(f, 8, n * 20)?;
            let arq: Vec<(u32, u32)> = (0..n).filter_map(|i| Some((be(&t, i * 20)?, be(&t, i * 20 + 8)?))).collect();
            arq.iter().find(|a| a.0 == ARM64).or_else(|| arq.iter().find(|a| a.0 & 0x0100_0000 != 0)).map(|a| a.1 as u64)
        }
        _ if le(&h, 0)? == MACHO64 => Some(0),
        _ => None,
    }
}

/// O SuperBlob da assinatura do executável.
fn assinatura(f: &File) -> Option<Vec<u8>> {
    let ini = slice(f)?;
    let h = ler_em(f, ini, 32)?;
    if le(&h, 0)? != MACHO64 {
        return None;
    }
    let (ncmds, tam) = (le(&h, 16)? as usize, le(&h, 20)? as usize);
    let cmds = ler_em(f, ini + 32, tam.min(LIMITE_COMANDOS))?;
    let mut p = 0;
    for _ in 0..ncmds {
        let (cmd, sz) = (le(&cmds, p)?, le(&cmds, p + 4)? as usize);
        if cmd == LC_CODE_SIGNATURE {
            let (off, n) = (le(&cmds, p + 8)? as u64, le(&cmds, p + 12)? as usize);
            return (n <= LIMITE).then(|| ler_em(f, ini + off, n)).flatten();
        }
        if sz < 8 {
            return None;
        }
        p += sz;
    }
    None
}

/// Grupos de app (`com.apple.security.application-groups`) que o executável declara; vazio se não
/// tem assinatura legível ou o blob de entitlements.
pub fn grupos(exe: &Path) -> Vec<String> {
    File::open(exe).ok().and_then(|f| assinatura(&f)).and_then(|s| grupos_do_blob(&s)).unwrap_or_default()
}

fn grupos_do_blob(s: &[u8]) -> Option<Vec<String>> {
    if be(s, 0)? != SUPERBLOB {
        return None;
    }
    for i in 0..be(s, 8)?.min(64) as usize {
        let off = be(s, 12 + i * 8 + 4)? as usize;
        if be(s, off)? == ENTITLEMENTS_XML {
            let fim = off.checked_add(be(s, off + 4)? as usize)?.min(s.len());
            let v = plist::ler(s.get(off + 8..fim)?)?;
            return Some(v.get(GRUPOS)?.lista().iter().filter_map(|g| g.texto().map(str::to_string)).collect());
        }
    }
    None
}

#[cfg(test)]
pub(crate) mod testes {
    use super::*;

    /// SuperBlob com um índice (tipo 5) que aponta para um blob de entitlements.
    pub fn superblob(xml: &str) -> Vec<u8> {
        let mut blob = ENTITLEMENTS_XML.to_be_bytes().to_vec();
        blob.extend_from_slice(&(8 + xml.len() as u32).to_be_bytes());
        blob.extend_from_slice(xml.as_bytes());
        let mut s = SUPERBLOB.to_be_bytes().to_vec();
        s.extend_from_slice(&((20 + blob.len()) as u32).to_be_bytes());
        s.extend_from_slice(&1u32.to_be_bytes());
        s.extend_from_slice(&5u32.to_be_bytes());
        s.extend_from_slice(&20u32.to_be_bytes());
        s.extend(blob);
        s
    }

    /// Mach-O 64 fino mínimo com LC_CODE_SIGNATURE no fim do arquivo.
    pub fn macho(sig: &[u8]) -> Vec<u8> {
        let mut m = MACHO64.to_le_bytes().to_vec();
        for x in [ARM64, 0, 2, 1, 16, 0, 0] {
            m.extend_from_slice(&x.to_le_bytes());
        }
        let dataoff = 32 + 16;
        for x in [LC_CODE_SIGNATURE, 16, dataoff as u32, sig.len() as u32] {
            m.extend_from_slice(&x.to_le_bytes());
        }
        m.extend_from_slice(sig);
        m
    }

    const XML: &str = r#"<?xml version="1.0"?><plist version="1.0"><dict><key>com.apple.security.application-groups</key><array><string>group.com.exemplo.app</string><string>ABCDE12345.exemplo</string></array></dict></plist>"#;

    #[test]
    fn le_grupos_de_macho_fino_e_fat() {
        let d = std::env::temp_dir().join(format!("sdtally-assin-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let fino = macho(&superblob(XML));
        std::fs::write(d.join("fino"), &fino).unwrap();
        assert_eq!(grupos(&d.join("fino")), ["group.com.exemplo.app", "ABCDE12345.exemplo"]);
        // Fat com um slice arm64 deslocado de 64 bytes.
        let mut fat = FAT.to_be_bytes().to_vec();
        fat.extend_from_slice(&1u32.to_be_bytes());
        for x in [ARM64, 0, 64, fino.len() as u32, 14] {
            fat.extend_from_slice(&x.to_be_bytes());
        }
        fat.resize(64, 0);
        fat.extend_from_slice(&fino);
        std::fs::write(d.join("fat"), &fat).unwrap();
        assert_eq!(grupos(&d.join("fat")), ["group.com.exemplo.app", "ABCDE12345.exemplo"]);
        // Sem assinatura, lixo, truncado e inexistente: vazio.
        std::fs::write(d.join("lixo"), b"#!/bin/sh\necho oi\n").unwrap();
        assert!(grupos(&d.join("lixo")).is_empty());
        std::fs::write(d.join("corte"), &fino[..fino.len() - 10]).unwrap();
        assert!(grupos(&d.join("corte")).is_empty());
        assert!(grupos(&d.join("nao-existe")).is_empty());
        std::fs::remove_dir_all(&d).ok();
    }
}

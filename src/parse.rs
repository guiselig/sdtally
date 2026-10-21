//! Parser puro do buffer do getattrlistbulk(2): bytes → entradas tipadas. Sem `unsafe` e sem
//! syscall, para rodar com buffers sintéticos e Miri. Nunca entra em pânico: fora do layout
//! vira `ErroParse`.
//!
//! Layout de cada entrada (man getattrlistbulk; vfs_attrlist.c, xnu 12377):
//! `len(4) | attribute_set(20) | [errno(4) se ATTR_CMN_ERROR] | nameref{off,len}(8) | demais
//! comuns | arquivo | estendidos | nome`. Numa entrada com erro o kernel só devolve
//! RETURNED_ATTRS, NAME e ERROR (Frente O §1.4).

/// Ausente no crate libc (sys/attr.h).
pub const ATTR_CMN_ERROR: u32 = 0x2000_0000;
pub const SF_DATALESS: u32 = 0x4000_0000;
pub const EF_MAY_SHARE_BLOCKS: u64 = 0x01;
pub const EF_NO_XATTRS: u64 = 0x02;
/// st_flags: arquivo comprimido (decmpfs); os dados não estão no fork de dados.
pub const UF_COMPRESSED: u32 = 0x20;
pub const EF_SHARES_ALL_BLOCKS: u64 = 0x40;
pub const VREG: u32 = 1;
pub const VDIR: u32 = 2;

// Os bits que sys::apfs::ler_lote pede (usa estas constantes). Outro bit mudaria o layout.
pub const CMN: u32 = libc::ATTR_CMN_RETURNED_ATTRS
    | libc::ATTR_CMN_NAME
    | libc::ATTR_CMN_DEVID
    | libc::ATTR_CMN_OBJTYPE
    | libc::ATTR_CMN_FLAGS
    | libc::ATTR_CMN_FILEID
    | ATTR_CMN_ERROR;
pub const FILE: u32 = libc::ATTR_FILE_LINKCOUNT | libc::ATTR_FILE_TOTALSIZE | libc::ATTR_FILE_ALLOCSIZE;
pub const EXT: u32 = libc::ATTR_CMNEXT_PRIVATESIZE | libc::ATTR_CMNEXT_CLONEID | libc::ATTR_CMNEXT_EXT_FLAGS;

/// Atributo ausente é `None`, nunca 0.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entrada<'a> {
    /// Sem o NUL; nunca vazio, ".", ".." nem com '/'.
    pub nome: &'a [u8],
    pub devid: Option<u32>,
    pub objtype: Option<u32>,
    pub flags: Option<u32>,
    pub fileid: Option<u64>,
    pub linkcount: Option<u32>,
    pub totalsize: Option<u64>,
    pub allocsize: Option<u64>,
    pub privatesize: Option<u64>,
    pub cloneid: Option<u64>,
    pub ext_flags: Option<u64>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Item<'a> {
    Entrada(Entrada<'a>),
    /// ATTR_CMN_ERROR != 0: a entrada existe, mas o kernel não leu os atributos.
    Erro(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroParse {
    Truncado,
    TamanhoZero,
    NomeInvalido,
    AtributoInesperado,
}

/// Itera as `n` entradas que o getattrlistbulk disse ter gravado em `buf`. Depois de um
/// erro no tamanho da entrada não há como achar a próxima: a iteração para.
pub fn entradas(buf: &[u8], n: usize) -> Entradas<'_> {
    Entradas { buf, pos: 0, faltam: n }
}

pub struct Entradas<'a> {
    buf: &'a [u8],
    pos: usize,
    faltam: usize,
}

impl<'a> Iterator for Entradas<'a> {
    type Item = Result<Item<'a>, ErroParse>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.faltam == 0 {
            return None;
        }
        self.faltam -= 1;
        let fim = |s: &mut Self, e| {
            s.faltam = 0;
            Some(Err(e))
        };
        let Ok(len) = Cursor { b: self.buf, p: self.pos }.u32() else { return fim(self, ErroParse::Truncado) };
        if len == 0 {
            return fim(self, ErroParse::TamanhoZero);
        }
        let Some(e) = self.pos.checked_add(len as usize).and_then(|f| self.buf.get(self.pos..f)) else {
            return fim(self, ErroParse::Truncado);
        };
        self.pos += len as usize;
        Some(entrada(e))
    }
}

struct Cursor<'a> {
    b: &'a [u8],
    p: usize,
}

impl Cursor<'_> {
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N], ErroParse> {
        let s = self.b.get(self.p..self.p.saturating_add(N)).ok_or(ErroParse::Truncado)?;
        self.p += N;
        s.try_into().map_err(|_| ErroParse::Truncado)
    }
    fn u32(&mut self) -> Result<u32, ErroParse> {
        self.bytes().map(u32::from_ne_bytes)
    }
    // Campos de 8 bytes vêm só alinhados a 4 (getattrlist não preenche até 8).
    fn u64(&mut self) -> Result<u64, ErroParse> {
        self.bytes().map(u64::from_ne_bytes)
    }
    fn opt32(&mut self, tem: bool) -> Result<Option<u32>, ErroParse> {
        if tem { self.u32().map(Some) } else { Ok(None) }
    }
    fn opt64(&mut self, tem: bool) -> Result<Option<u64>, ErroParse> {
        if tem { self.u64().map(Some) } else { Ok(None) }
    }
}

/// Uma entrada já recortada pelo seu `len` (inclui os 4 bytes do próprio len).
fn entrada(e: &[u8]) -> Result<Item<'_>, ErroParse> {
    let mut c = Cursor { b: e, p: 4 };
    let (cmn, vol, dir, file, ext) = (c.u32()?, c.u32()?, c.u32()?, c.u32()?, c.u32()?);
    if cmn & !CMN != 0 || vol != 0 || dir != 0 || file & !FILE != 0 || ext & !EXT != 0 {
        return Err(ErroParse::AtributoInesperado);
    }
    if cmn & ATTR_CMN_ERROR != 0 {
        let errno = c.u32()? as i32;
        if errno != 0 {
            return Ok(Item::Erro(errno));
        }
    }
    // O nome é obrigatório: sem ele o walker não monta o caminho.
    if cmn & libc::ATTR_CMN_NAME == 0 {
        return Err(ErroParse::NomeInvalido);
    }
    let nref = c.p;
    // attr_dataoffset é relativo ao próprio attrreference; attr_length inclui o NUL.
    let off = c.u32()? as i32 as i64;
    let tam = c.u32()? as i64;
    let ini = nref as i64 + off;
    let nome = usize::try_from(ini)
        .ok()
        .zip(usize::try_from(ini + tam).ok())
        .and_then(|(i, f)| e.get(i..f))
        .and_then(|s| s.iter().position(|b| *b == 0).map(|z| &s[..z]))
        .filter(|n| !n.is_empty() && *n != b"." && *n != b".." && !n.contains(&b'/'))
        .ok_or(ErroParse::NomeInvalido)?;
    let tem = |m: u32, a: u32| m & a != 0;
    Ok(Item::Entrada(Entrada {
        nome,
        devid: c.opt32(tem(cmn, libc::ATTR_CMN_DEVID))?,
        objtype: c.opt32(tem(cmn, libc::ATTR_CMN_OBJTYPE))?,
        flags: c.opt32(tem(cmn, libc::ATTR_CMN_FLAGS))?,
        fileid: c.opt64(tem(cmn, libc::ATTR_CMN_FILEID))?,
        linkcount: c.opt32(tem(file, libc::ATTR_FILE_LINKCOUNT))?,
        totalsize: c.opt64(tem(file, libc::ATTR_FILE_TOTALSIZE))?,
        allocsize: c.opt64(tem(file, libc::ATTR_FILE_ALLOCSIZE))?,
        privatesize: c.opt64(tem(ext, libc::ATTR_CMNEXT_PRIVATESIZE))?,
        cloneid: c.opt64(tem(ext, libc::ATTR_CMNEXT_CLONEID))?,
        ext_flags: c.opt64(tem(ext, libc::ATTR_CMNEXT_EXT_FLAGS))?,
    }))
}

/// Resposta do getattrlist com `RETURNED_ATTRS | ADDEDTIME`: u32 de tamanho, attribute_set_t (5 × u32)
/// e o timespec. None se o kernel não marcou o ADDEDTIME como devolvido ou o buffer veio curto.
pub fn added_time(b: &[u8]) -> Option<i64> {
    let cmn = u32::from_ne_bytes(b.get(4..8)?.try_into().ok()?);
    if cmn & libc::ATTR_CMN_ADDEDTIME == 0 {
        return None;
    }
    Some(i64::from_ne_bytes(b.get(24..32)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use libc::{
        ATTR_CMN_DEVID as DEVID, ATTR_CMN_FILEID as FILEID, ATTR_CMN_FLAGS as FLAGS, ATTR_CMN_NAME as NAME,
        ATTR_CMN_OBJTYPE as OBJTYPE, ATTR_CMN_RETURNED_ATTRS as RET, ATTR_CMNEXT_CLONEID as CLONEID,
        ATTR_CMNEXT_EXT_FLAGS as EXTF, ATTR_CMNEXT_PRIVATESIZE as PRIV, ATTR_FILE_ALLOCSIZE as ALLOC,
        ATTR_FILE_LINKCOUNT as LINKS, ATTR_FILE_TOTALSIZE as TOTAL,
    };

    /// Monta uma entrada como o kernel: len | attrset | [errno] | nameref | fixos | nome (alinhado a 4).
    fn ent(cmn: u32, file: u32, ext: u32, erro: Option<u32>, nome: &[u8], fixos: &[u8]) -> Vec<u8> {
        let mut v = vec![0u8; 4];
        for x in [cmn, 0, 0, file, ext] {
            v.extend(x.to_ne_bytes());
        }
        if let Some(e) = erro {
            v.extend(e.to_ne_bytes());
        }
        let nref = v.len();
        v.extend([0u8; 8]);
        v.extend(fixos);
        let off = (v.len() - nref) as u32;
        v.extend(nome);
        v.push(0);
        while !v.len().is_multiple_of(4) {
            v.push(0);
        }
        v[nref..nref + 4].copy_from_slice(&off.to_ne_bytes());
        v[nref + 4..nref + 8].copy_from_slice(&(nome.len() as u32 + 1).to_ne_bytes());
        let len = v.len() as u32;
        v[..4].copy_from_slice(&len.to_ne_bytes());
        v
    }

    fn campos(u32s: &[u32], u64s: &[u64]) -> Vec<u8> {
        let mut v = Vec::new();
        u32s.iter().for_each(|x| v.extend(x.to_ne_bytes()));
        u64s.iter().for_each(|x| v.extend(x.to_ne_bytes()));
        v
    }

    fn arquivo() -> Vec<u8> {
        // devid, objtype, flags | fileid | linkcount | total, alloc, private, cloneid, extflags
        let mut f = campos(&[7, VREG, 0], &[42]);
        f.extend(campos(&[2], &[1000, 4096, 4096, 99, EF_SHARES_ALL_BLOCKS]));
        ent(RET | NAME | DEVID | OBJTYPE | FLAGS | FILEID | ATTR_CMN_ERROR, LINKS | TOTAL | ALLOC, PRIV | CLONEID | EXTF, Some(0), b"a.bin", &f)
    }

    fn so(buf: &[u8], n: usize) -> Vec<Result<Item<'_>, ErroParse>> {
        entradas(buf, n).collect()
    }

    #[test]
    fn arquivo_regular() {
        let b = arquivo();
        let r = so(&b, 1);
        assert_eq!(
            r,
            vec![Ok(Item::Entrada(Entrada {
                nome: b"a.bin",
                devid: Some(7),
                objtype: Some(VREG),
                flags: Some(0),
                fileid: Some(42),
                linkcount: Some(2),
                totalsize: Some(1000),
                allocsize: Some(4096),
                privatesize: Some(4096),
                cloneid: Some(99),
                ext_flags: Some(EF_SHARES_ALL_BLOCKS),
            }))]
        );
    }

    #[test]
    fn pasta_dataless() {
        // Pasta: o kernel não devolve atributos de arquivo.
        let f = campos(&[7, VDIR, SF_DATALESS], &[5]);
        let mut f2 = f.clone();
        f2.extend(campos(&[], &[0, 5, 0]));
        let b = ent(RET | NAME | DEVID | OBJTYPE | FLAGS | FILEID, 0, PRIV | CLONEID | EXTF, None, b"Docs", &f2);
        let Some(Ok(Item::Entrada(e))) = entradas(&b, 1).next() else { panic!("esperava entrada") };
        assert_eq!((e.nome, e.objtype, e.flags, e.fileid, e.linkcount, e.cloneid), (&b"Docs"[..], Some(VDIR), Some(SF_DATALESS), Some(5), None, Some(5)));
    }

    #[test]
    fn entrada_com_erro_e_a_seguinte() {
        // Layout de erro: len | attrset | errno | nameref | nome. O errno não pode virar offset.
        let mut b = ent(RET | NAME | ATTR_CMN_ERROR, 0, 0, Some(libc::EACCES as u32), b"negada", &[]);
        b.extend(arquivo());
        let r = so(&b, 2);
        assert_eq!(r[0], Ok(Item::Erro(libc::EACCES)));
        assert!(matches!(&r[1], Ok(Item::Entrada(e)) if e.nome == b"a.bin" && e.fileid == Some(42)));
    }

    #[test]
    fn opcionais_ausentes() {
        let b = ent(RET | NAME, 0, 0, None, b"x", &[]);
        assert_eq!(so(&b, 1), vec![Ok(Item::Entrada(Entrada { nome: b"x", ..Default::default() }))]);
    }

    #[test]
    fn nameref_fora_da_entrada() {
        let mut b = ent(RET | NAME, 0, 0, None, b"x", &[]);
        b[24..28].copy_from_slice(&0x7fff_0000u32.to_ne_bytes());
        let mut neg = ent(RET | NAME, 0, 0, None, b"y", &[]);
        neg[24..28].copy_from_slice(&(-100i32).to_ne_bytes());
        b.extend(neg);
        b.extend(arquivo());
        let r = so(&b, 3);
        assert_eq!(r[0], Err(ErroParse::NomeInvalido));
        assert_eq!(r[1], Err(ErroParse::NomeInvalido));
        // O len estava certo: a próxima entrada continua legível.
        assert!(matches!(&r[2], Ok(Item::Entrada(e)) if e.nome == b"a.bin"));
    }

    #[test]
    fn nameref_nao_passa_do_len() {
        // O nome apontado existe no buffer, mas é o da entrada seguinte, depois do len desta.
        let mut b = ent(RET | NAME, 0, 0, None, b"x", &[]);
        let prox = arquivo();
        let alvo = b.len() + prox.windows(5).position(|w| w == b"a.bin").expect("nome em arquivo()");
        b[24..32].copy_from_slice(&campos(&[(alvo - 24) as u32, 6], &[]));
        b.extend(prox);
        let r = so(&b, 2);
        assert_eq!(r[0], Err(ErroParse::NomeInvalido));
        assert!(matches!(&r[1], Ok(Item::Entrada(e)) if e.nome == b"a.bin"));
    }

    #[test]
    fn campos_nao_passam_do_len() {
        // len(36) | attrset | nameref → "x\0\0\0", lido também como DEVID; o OBJTYPE fica depois
        // do len, onde há bytes válidos que o parser não pode usar.
        let mut b = vec![0u8; 4];
        for x in [RET | NAME | DEVID | OBJTYPE, 0, 0, 0, 0] {
            b.extend(x.to_ne_bytes());
        }
        b.extend(campos(&[8, 2], &[]));
        b.extend(b"x\0\0\0");
        let len = b.len() as u32;
        b[..4].copy_from_slice(&len.to_ne_bytes());
        b.extend(VREG.to_ne_bytes());
        assert_eq!(so(&b, 1), vec![Err(ErroParse::Truncado)]);
    }

    #[test]
    fn nomes_perigosos() {
        for n in [&b".."[..], b".", b"a/b"] {
            assert_eq!(so(&ent(RET | NAME, 0, 0, None, n, &[]), 1), vec![Err(ErroParse::NomeInvalido)]);
        }
    }

    #[test]
    fn truncado() {
        let b = arquivo();
        for corte in 0..b.len() {
            let r = so(&b[..corte], 1);
            assert_eq!(r, vec![Err(ErroParse::Truncado)], "corte {corte}");
        }
        // n maior que o que há no buffer: para no primeiro erro.
        assert_eq!(so(&b, 5).len(), 2);
    }

    #[test]
    fn len_zero() {
        let mut b = arquivo();
        b[..4].copy_from_slice(&0u32.to_ne_bytes());
        assert_eq!(so(&b, 3), vec![Err(ErroParse::TamanhoZero)]);
    }

    #[test]
    fn atributo_nao_pedido() {
        let b = ent(RET | NAME | libc::ATTR_CMN_OBJID, 0, 0, None, b"x", &[0; 8]);
        assert_eq!(so(&b, 1), vec![Err(ErroParse::AtributoInesperado)]);
    }

    /// xorshift64*: determinístico, sem crate.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
    }

    #[test]
    fn robustez_100_mil_buffers() {
        let mut r = Rng(0x5d7a_11e5_0000_0001);
        let base = {
            let mut b = arquivo();
            b.extend(ent(RET | NAME | ATTR_CMN_ERROR, 0, 0, Some(13), b"negada", &[]));
            b.extend(arquivo());
            b
        };
        let mut vistos = 0usize;
        for i in 0..100_000u32 {
            let buf: Vec<u8> = if i.is_multiple_of(2) {
                // Aleatório puro, às vezes com um len plausível no começo.
                let n = (r.next() % 512) as usize;
                let mut b: Vec<u8> = (0..n).map(|_| r.next() as u8).collect();
                if n >= 4 && r.next().is_multiple_of(2) {
                    let l = (r.next() % (n as u64 + 8)) as u32;
                    b[..4].copy_from_slice(&l.to_ne_bytes());
                }
                b
            } else {
                // Buffer válido com 1 a 8 bytes trocados e corte aleatório.
                let mut b = base.clone();
                for _ in 0..1 + r.next() % 8 {
                    let p = (r.next() % b.len() as u64) as usize;
                    b[p] = r.next() as u8;
                }
                let corte = (r.next() % (b.len() as u64 + 1)) as usize;
                b.truncate(corte);
                b
            };
            let n = (r.next() % 6) as usize;
            vistos += entradas(&buf, n).count();
        }
        assert!(vistos > 0);
    }

    #[test]
    fn added_time_do_buffer() {
        let mut b = vec![0u8; 40];
        b[4..8].copy_from_slice(&libc::ATTR_CMN_ADDEDTIME.to_ne_bytes());
        b[24..32].copy_from_slice(&1_700_000_000i64.to_ne_bytes());
        assert_eq!(added_time(&b), Some(1_700_000_000));
        // Não devolvido ou curto: sem data, nunca 0.
        b[4..8].fill(0);
        assert_eq!(added_time(&b), None);
        assert_eq!(added_time(&b[..20]), None);
    }
}

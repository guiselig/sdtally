//! getattrlist/getattrlistbulk, fsctl privados do APFS, snapshots e xattrs. Só leitura.
//! O buffer do getattrlistbulk é interpretado em `parse` (código puro); aqui só a syscall.

use super::{cpath, errno};
use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_ulong, c_void};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::Path;

/// Estatísticas de pasta do APFS (o `apfs.util -S`). Buffer zerado = leitura.
/// buf+4 != 0 GRAVA no volume (`apfs.util -M`): este código nunca escreve no buffer.
const APFS_DIRSTATS: c_ulong = 0xC110_4A71;
/// Tamanho privado de faixa de snapshots {from_xid, to_xid, out}.
const APFS_SNAP_PRIVATE_SIZE: c_ulong = 0xC018_4A13;

unsafe extern "C" {
    fn fsctl(path: *const c_char, request: c_ulong, data: *mut c_void, options: c_uint) -> c_int;
    fn fs_snapshot_list(dirfd: c_int, al: *mut libc::attrlist, buf: *mut c_void, size: usize, flags: u32) -> c_int;
}

fn attrlist(common: u32, vol: u32, file: u32, ext: u32) -> libc::attrlist {
    let mut al: libc::attrlist = unsafe { std::mem::zeroed() };
    al.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
    al.commonattr = common;
    al.volattr = vol;
    al.fileattr = file;
    al.forkattr = ext;
    al
}

/// dir-stats: bytes físicos da pasta, sem abrir nem listar. Err(errno).
pub fn dirstats(path: &str) -> Result<u64, i32> {
    let c = cpath(path)?;
    let mut b = [0u8; 0x110]; // zerado: operação "get"
    // FSOPT_NOFOLLOW: symlink trocado no lugar da pasta não leva o fsctl a outro lugar.
    let rc = unsafe { fsctl(c.as_ptr(), APFS_DIRSTATS, b.as_mut_ptr() as *mut c_void, libc::FSOPT_NOFOLLOW) };
    if rc != 0 {
        return Err(errno());
    }
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[0x40..0x48]);
    Ok(u64::from_ne_bytes(v))
}

pub fn snap_private(mount: &str, from: u64, to: u64) -> Result<u64, i32> {
    let c = cpath(mount)?;
    let mut r = [from, to, 0u64];
    let rc = unsafe { fsctl(c.as_ptr(), APFS_SNAP_PRIVATE_SIZE, r.as_mut_ptr() as *mut c_void, 0) };
    if rc != 0 { Err(errno()) } else { Ok(r[2]) }
}

/// (nome, xid) dos snapshots do volume; xid None se o kernel não devolveu OBJID.
pub fn snapshots(mount: &str) -> Result<Vec<(String, Option<u64>)>, i32> {
    let c = cpath(mount)?;
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY) };
    if fd < 0 {
        return Err(errno());
    }
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    let mut al = attrlist(libc::ATTR_CMN_RETURNED_ATTRS | libc::ATTR_CMN_NAME | libc::ATTR_CMN_OBJID, 0, 0, 0);
    let mut buf = vec![0u8; 64 * 1024];
    let mut out = Vec::new();
    loop {
        let n = unsafe { fs_snapshot_list(fd.as_raw_fd(), &mut al, buf.as_mut_ptr() as *mut c_void, buf.len(), 0) };
        if n < 0 {
            if errno() == libc::EINTR {
                continue;
            }
            return Err(errno());
        }
        if n == 0 {
            return Ok(out);
        }
        let rd = |o: usize| buf.get(o..o + 4).map(|s| u32::from_ne_bytes([s[0], s[1], s[2], s[3]]));
        let mut e = 0usize;
        for _ in 0..n {
            let Some(len) = rd(e).map(|x| x as usize).filter(|l| *l > 0) else { break };
            let ret_cmn = rd(e + 4).unwrap_or(0);
            let nref = e + 24;
            let name = rd(nref)
                .and_then(|off| buf.get((nref as isize + off as i32 as isize) as usize..))
                .and_then(|s| CStr::from_bytes_until_nul(s).ok())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            // No snapshot APFS, ATTR_CMN_OBJID = XID (conferido contra diskutil; Frente B).
            let xid = if ret_cmn & libc::ATTR_CMN_OBJID != 0 {
                rd(nref + 8).zip(rd(nref + 12)).map(|(lo, hi)| lo as u64 | ((hi as u64) << 32))
            } else {
                None
            };
            out.push((name, xid));
            e += len;
        }
    }
}

/// ATTR_VOL_SPACEUSED do volume montado em `mount`.
pub fn vol_space_used(mount: &str) -> Result<u64, i32> {
    let b = vol_attr(mount, libc::ATTR_VOL_SPACEUSED)?;
    Ok(u64::from_ne_bytes([b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11]]))
}

/// `ATTR_CMN_ADDEDTIME`: segundos desde a época em que o item foi criado ou renomeado para dentro
/// da pasta onde está (getattrlist(2); no Lixo, a data em que foi para lá). Não segue symlink.
/// Ok(None): o sistema de arquivos não devolveu o atributo. Err(errno).
pub fn added_time(p: &Path) -> Result<Option<i64>, i32> {
    let c = cpath(p)?;
    let mut al = attrlist(libc::ATTR_CMN_RETURNED_ATTRS | libc::ATTR_CMN_ADDEDTIME, 0, 0, 0);
    let mut b = [0u8; 64];
    let rc = unsafe { libc::getattrlist(c.as_ptr(), &mut al as *mut _ as *mut c_void, b.as_mut_ptr() as *mut c_void, b.len(), libc::FSOPT_NOFOLLOW) };
    if rc != 0 {
        return Err(errno());
    }
    Ok(crate::parse::added_time(&b))
}

/// VOL_CAPABILITIES_FORMAT: (válidos, ativos).
pub fn vol_cap_formato(mount: &str) -> Result<(u32, u32), i32> {
    // vol_capabilities_attr_t = capabilities[4] seguido de valid[4]; índice 0 = FORMAT.
    let b = vol_attr(mount, libc::ATTR_VOL_CAPABILITIES)?;
    let u = |o: usize| u32::from_ne_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    Ok((u(4 + 16), u(4)))
}

fn vol_attr(mount: &str, attr: u32) -> Result<[u8; 64], i32> {
    let c = cpath(mount)?;
    let mut al = attrlist(0, libc::ATTR_VOL_INFO | attr, 0, 0);
    let mut b = [0u8; 64];
    let rc = unsafe { libc::getattrlist(c.as_ptr(), &mut al as *mut _ as *mut c_void, b.as_mut_ptr() as *mut c_void, b.len(), 0) };
    if rc != 0 { Err(errno()) } else { Ok(b) }
}

/// Abre a pasta sem seguir symlink, repetindo em EINTR (síntese §4 2c).
pub fn abrir_pasta(p: &Path) -> Result<OwnedFd, i32> {
    let c = cpath(p)?;
    loop {
        let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW) };
        if fd >= 0 {
            return Ok(unsafe { OwnedFd::from_raw_fd(fd) });
        }
        if errno() != libc::EINTR {
            return Err(errno());
        }
    }
}

/// Abre `rel` (relativo, sem `..`) a partir da pasta já aberta `raiz`, sem seguir symlink em
/// nenhum componente (O_NOFOLLOW_ANY): uma pasta do meio trocada por symlink durante a travessia
/// não leva para fora. Caminho longo demais para uma chamada (ENAMETOOLONG): desce componente a
/// componente, cada um com O_NOFOLLOW. Repete em EINTR.
pub fn abrir_pasta_sob(raiz: &OwnedFd, rel: &Path) -> Result<OwnedFd, i32> {
    const FLAGS: c_int = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC;
    // O_NOFOLLOW_ANY já cobre o último componente; junto com O_NOFOLLOW o kernel dá EINVAL.
    let abrir = |dir: c_int, c: &CString, nofollow: c_int| loop {
        let fd = unsafe { libc::openat(dir, c.as_ptr(), FLAGS | nofollow) };
        if fd >= 0 {
            return Ok(unsafe { OwnedFd::from_raw_fd(fd) });
        }
        if errno() != libc::EINTR {
            return Err(errno());
        }
    };
    match abrir(raiz.as_raw_fd(), &cpath(rel)?, libc::O_NOFOLLOW_ANY) {
        Err(libc::ENAMETOOLONG) => {}
        r => return r,
    }
    let mut cur: Option<OwnedFd> = None;
    for comp in rel.components() {
        let std::path::Component::Normal(nome) = comp else { return Err(libc::EINVAL) };
        let dir = cur.as_ref().unwrap_or(raiz).as_raw_fd();
        cur = Some(abrir(dir, &cpath(nome)?, libc::O_NOFOLLOW)?);
    }
    cur.ok_or(libc::EINVAL)
}

/// Faixas físicas [início, fim) em bytes do volume dos dados do arquivo (`F_LOG2PHYS_EXT`), o fim
/// arredondado ao bloco de 4 KiB; buraco de arquivo esparso não entra. Só leitura: `open` sem ler
/// nada e `fcntl` de consulta; nunca em arquivo dataless (o chamador filtra). Err(errno).
pub fn faixas_fisicas(p: &Path) -> Result<Vec<(u64, u64)>, i32> {
    const BLOCO: u64 = 4096;
    // Arquivo com mais faixas que isto fica sem mapa (conta cheia), não consome memória sem fim.
    const MAX_FAIXAS: usize = 100_000;
    let c = cpath(p)?;
    let fd = loop {
        let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC) };
        if fd >= 0 {
            break unsafe { OwnedFd::from_raw_fd(fd) };
        }
        if errno() != libc::EINTR {
            return Err(errno());
        }
    };
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd.as_raw_fd(), &mut st) } != 0 {
        return Err(errno());
    }
    if st.st_mode & libc::S_IFMT != libc::S_IFREG {
        return Err(libc::EINVAL);
    }
    let (tamanho, mut off, mut out) = (st.st_size, 0i64, Vec::new());
    while off < tamanho {
        let mut l = libc::log2phys { l2p_flags: 0, l2p_contigbytes: tamanho - off, l2p_devoffset: off };
        if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_LOG2PHYS_EXT, &mut l as *mut libc::log2phys) } < 0 {
            return Err(errno());
        }
        let n = l.l2p_contigbytes;
        if n <= 0 {
            break;
        }
        // devoffset < 0: buraco.
        let ini = l.l2p_devoffset;
        if ini >= 0 {
            out.push((ini as u64, (ini as u64 + n as u64).div_ceil(BLOCO) * BLOCO));
            if out.len() > MAX_FAIXAS {
                return Err(libc::EFBIG);
            }
        }
        off += n;
    }
    Ok(out)
}

/// Liga F_NOCACHE no arquivo aberto: o que for lido dele não fica no cache de páginas. Err(errno).
pub fn sem_cache(fd: &impl AsRawFd) -> Result<(), i32> {
    if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_NOCACHE, 1) } < 0 {
        return Err(errno());
    }
    Ok(())
}

/// st_dev da pasta aberta: o volume em que ela está de fato (depois de uma montagem, o de dentro).
pub fn dev_da_pasta(fd: &OwnedFd) -> Result<u32, i32> {
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd.as_raw_fd(), &mut st) } != 0 { Err(errno()) } else { Ok(st.st_dev as u32) }
}

/// (fileid, cloneid) da pasta aberta.
pub fn ids_da_pasta(fd: &OwnedFd) -> Option<(u64, u64)> {
    let mut al = attrlist(libc::ATTR_CMN_FILEID, 0, 0, libc::ATTR_CMNEXT_CLONEID);
    let mut b = [0u8; 32];
    let rc = unsafe { libc::fgetattrlist(fd.as_raw_fd(), &mut al as *mut _ as *mut _, b.as_mut_ptr() as *mut _, b.len(), libc::FSOPT_ATTR_CMN_EXTENDED) };
    if rc != 0 {
        return None;
    }
    let u = |o: usize| u64::from_ne_bytes([b[o], b[o + 1], b[o + 2], b[o + 3], b[o + 4], b[o + 5], b[o + 6], b[o + 7]]);
    Some((u(4), u(12)))
}

/// Um lote do getattrlistbulk: Ok(n entradas; 0 = fim) ou Err(errno). Repete em EINTR.
/// Pede exatamente os bits que o `parse` entende (uma lista só, lá).
pub fn ler_lote(fd: &OwnedFd, buf: &mut [u8]) -> Result<usize, i32> {
    use crate::parse::{CMN, EXT, FILE};
    let mut al = attrlist(CMN, 0, FILE, EXT);
    let opts = (libc::FSOPT_NOFOLLOW | libc::FSOPT_ATTR_CMN_EXTENDED) as u64;
    loop {
        let n = unsafe { libc::getattrlistbulk(fd.as_raw_fd(), &mut al as *mut _ as *mut _, buf.as_mut_ptr() as *mut _, buf.len(), opts) };
        if n >= 0 {
            return Ok(n as usize);
        }
        if errno() != libc::EINTR {
            return Err(errno());
        }
    }
}

/// Os mesmos atributos do `ler_lote`, de um objeto só (sem seguir symlink), no layout de uma
/// entrada do getattrlistbulk: o `parse` lê. ATTR_CMN_ERROR só existe no bulk, fica de fora.
pub fn atributos(p: &Path) -> Result<Vec<u8>, i32> {
    use crate::parse::{ATTR_CMN_ERROR, CMN, EXT, FILE};
    let c = cpath(p)?;
    let mut al = attrlist(CMN & !ATTR_CMN_ERROR, 0, FILE, EXT);
    let mut b = vec![0u8; 2048];
    let opts = libc::FSOPT_NOFOLLOW | libc::FSOPT_ATTR_CMN_EXTENDED;
    loop {
        let rc = unsafe { libc::getattrlist(c.as_ptr(), &mut al as *mut _ as *mut c_void, b.as_mut_ptr() as *mut c_void, b.len(), opts) };
        if rc == 0 {
            return Ok(b);
        }
        if errno() != libc::EINTR {
            return Err(errno());
        }
    }
}

/// Valor de um xattr da própria entrada (sem seguir link), até 1 KiB. None se não há ou não deu.
pub fn xattr_valor(p: &Path, nome: &str) -> Option<Vec<u8>> {
    let (c, n) = (cpath(p).ok()?, CString::new(nome).ok()?);
    let mut b = vec![0u8; 1024];
    let r = unsafe { libc::getxattr(c.as_ptr(), n.as_ptr(), b.as_mut_ptr() as *mut _, b.len(), 0, libc::XATTR_NOFOLLOW) };
    if r < 0 {
        return None;
    }
    b.truncate(r as usize);
    Some(b)
}

/// Tamanho (> 0) de cada xattr, sem ler conteúdo. ResourceFork e decmpfs já estão no ALLOCSIZE:
/// pulados antes do getxattr. Err(errno) se não deu para ler (ex.: ENAMETOOLONG): nunca lista vazia.
pub fn xattr_tamanhos(p: &Path) -> Result<Vec<i64>, i32> {
    let c = cpath(p)?;
    let opts = libc::XATTR_NOFOLLOW | libc::XATTR_SHOWCOMPRESSION;
    let n = unsafe { libc::listxattr(c.as_ptr(), std::ptr::null_mut(), 0, opts) };
    if n <= 0 {
        return if n == 0 { Ok(Vec::new()) } else { Err(errno()) };
    }
    let mut names = vec![0u8; n as usize];
    let n = unsafe { libc::listxattr(c.as_ptr(), names.as_mut_ptr() as *mut _, names.len(), opts) };
    if n < 0 {
        return Err(errno());
    }
    let mut out = Vec::new();
    for name in names[..n as usize].split(|b| *b == 0) {
        if name.is_empty() || name == b"com.apple.ResourceFork" || name == b"com.apple.decmpfs" {
            continue;
        }
        let Ok(cn) = CString::new(name) else { continue };
        let sz = unsafe { libc::getxattr(c.as_ptr(), cn.as_ptr(), std::ptr::null_mut(), 0, 0, opts) };
        match sz {
            // Removido entre o list e o get: não existe mais, não é falha.
            _ if sz < 0 && errno() == libc::ENOATTR => {}
            _ if sz < 0 => return Err(errno()),
            0 => {}
            _ => out.push(sz as i64),
        }
    }
    Ok(out)
}

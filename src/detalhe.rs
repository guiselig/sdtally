//! Detalhe de um ramo (`sdtally <caminho>`) e de um item (`sdtally why <caminho>`). Só leitura,
//! mesmas regras da travessia (síntese §4). A trava de volume fica no chamador (`mesmo_volume`):
//! aqui não há fsctl fora do caminho pedido.

use crate::anon::Ctx;
use crate::parse::{self, Item};
use crate::sys;
use crate::walk::{self, Controle};
use std::os::darwin::fs::MetadataExt as _;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::Arc;

/// `p` está no volume montado em `volume`? Pelo statfs (o st_dev do System e do Data é o mesmo
/// no grupo de volumes) e pelo st_dev do próprio item (um symlink noutro volume não passa).
/// Symlink: o statfs vai no pai, porque seguiria o link (quebrado dá erro, apontando para
/// outro volume seria recusado sem motivo).
pub fn mesmo_volume(p: &Path, volume: &str) -> Result<bool, String> {
    let meta = |x: &Path| std::fs::symlink_metadata(x).map_err(|e| format!("{}: {e}", x.display()));
    let m = meta(p)?;
    let alvo = if m.file_type().is_symlink() { p.parent().unwrap_or(p) } else { p };
    let mnt = sys::montado_em(alvo).map_err(|e| format!("{}: {}", alvo.display(), std::io::Error::from_raw_os_error(e)))?;
    Ok(mnt == volume && m.dev() == meta(Path::new(volume))?.dev())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filho {
    /// Caminho relativo à raiz do ramo; vazio = arquivos soltos na própria raiz.
    pub nome: String,
    pub alocado: i64,
    /// None nas pastas protegidas (dir-stats não dá o privado).
    pub privado: Option<i64>,
    pub protegida: bool,
}

#[derive(Debug, Default)]
pub struct Ramo {
    pub raiz: String,
    pub profundidade: usize,
    /// Do maior para o menor.
    pub filhos: Vec<Filho>,
    pub total: walk::Sum,
    pub dirs: u64,
    /// Pastas negadas sem dir-stats (errno ou 0): ficam fora da soma.
    pub protegidas_sem: u64,
    /// Entradas com erro de leitura, sem contar as negadas.
    pub erros: u64,
    pub outros_volumes: u64,
    pub interrompida: bool,
}

pub fn ramo(raiz: &Path, profundidade: usize, threads: usize, ctl: Arc<Controle>) -> Result<Ramo, String> {
    let s = raiz.to_str().ok_or("caminho não é UTF-8")?;
    let r = walk::walk_ctl(s, threads, Arc::new(Ctx::ramo(profundidade)), ctl)?;
    let mut filhos: Vec<Filho> = r
        .buckets
        .iter()
        // Balde vazio (pasta só com subpastas) não é linha.
        .filter(|(_, v)| v.alloc != 0 || v.files != 0 || v.dataless_dirs != 0)
        .map(|(k, v)| Filho { nome: if k == "/" { String::new() } else { k.clone() }, alocado: v.alloc, privado: Some(v.private), protegida: false })
        .collect();
    let mut sem = 0;
    for (p, _) in &r.denied {
        let nome = p.strip_prefix(raiz).map_or_else(|_| p.display().to_string(), |x| x.display().to_string());
        match sys::apfs::dirstats(&p.to_string_lossy()) {
            Ok(v) if v > 0 => filhos.push(Filho { nome, alocado: v as i64, privado: None, protegida: true }),
            // 0 não se distingue de "não mantido": não vira 0 medido.
            _ => sem += 1,
        }
    }
    filhos.sort_by(|a, b| b.alocado.cmp(&a.alocado).then_with(|| a.nome.cmp(&b.nome)));
    let negada = |k: &i32| *k == libc::EPERM || *k == libc::EACCES;
    Ok(Ramo {
        raiz: s.into(),
        profundidade,
        filhos,
        total: r.total,
        dirs: r.dirs,
        protegidas_sem: sem,
        erros: r.errors.iter().filter(|(k, _)| !negada(k)).map(|(_, v)| v).sum(),
        outros_volumes: r.other_dev,
        interrompida: r.interrompida,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Arquivo,
    Pasta,
    Link,
    Outro,
}

/// Atributos do getattrlist (None = o kernel não devolveu).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Attrs {
    pub logico: Option<u64>,
    pub alocado: Option<u64>,
    pub privado: Option<u64>,
    pub fileid: Option<u64>,
    pub cloneid: Option<u64>,
    pub ext_flags: Option<u64>,
    pub flags: Option<u32>,
    pub devid: Option<u32>,
    pub links: Option<u32>,
}

#[derive(Debug)]
pub struct Why {
    pub caminho: String,
    pub tipo: Tipo,
    pub attrs: Result<Attrs, String>,
    /// lstat: tamanho, st_blocks × 512, nlink, dev, st_flags.
    pub st: (u64, u64, u64, u64, u32),
    /// Tamanho de cada xattr (sem ResourceFork e decmpfs, que já estão no alocado).
    pub xattrs: Result<Vec<i64>, i32>,
    /// Pasta: raiz de enxerto? None = não se aplica (arquivo, ou raiz do volume).
    pub enxerto: Option<Result<bool, i32>>,
    pub dirstats: Option<Result<u64, i32>>,
}

/// Fileid da raiz de um volume APFS; ela tem cloneid próprio e não é enxerto (Frente L).
const RAIZ_APFS: u64 = 2;

pub fn why(p: &Path) -> Result<Why, String> {
    let m = std::fs::symlink_metadata(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let ft = m.file_type();
    let tipo = if ft.is_symlink() {
        Tipo::Link
    } else if ft.is_dir() {
        Tipo::Pasta
    } else if ft.is_file() {
        Tipo::Arquivo
    } else {
        Tipo::Outro
    };
    let attrs = sys::apfs::atributos(p).map_err(|e| format!("getattrlist errno {e}")).and_then(|b| match parse::entradas(&b, 1).next() {
        Some(Ok(Item::Entrada(e))) => Ok(Attrs {
            logico: e.totalsize,
            alocado: e.allocsize,
            privado: e.privatesize,
            fileid: e.fileid,
            cloneid: e.cloneid,
            ext_flags: e.ext_flags,
            flags: e.flags,
            devid: e.devid,
            links: e.linkcount,
        }),
        Some(Ok(Item::Erro(e))) => Err(format!("errno {e}")),
        Some(Err(e)) => Err(format!("{e:?}")),
        None => Err("sem entrada".into()),
    });
    let (mut enxerto, mut dirstats) = (None, None);
    if tipo == Tipo::Pasta {
        let fd = sys::apfs::abrir_pasta(p);
        enxerto = match fd.map(|fd| sys::apfs::ids_da_pasta(&fd)) {
            Ok(Some((id, _))) if id == RAIZ_APFS => None,
            Ok(Some((id, cid))) => Some(Ok(cid != 0 && cid != id)),
            Ok(None) => Some(Err(libc::EIO)),
            Err(e) => Some(Err(e)),
        };
        dirstats = Some(sys::apfs::dirstats(&p.to_string_lossy()));
    }
    Ok(Why {
        caminho: p.display().to_string(),
        tipo,
        attrs,
        st: (m.len(), m.blocks() * 512, m.nlink(), m.dev() as u64, m.st_flags()),
        xattrs: sys::apfs::xattr_tamanhos(p),
        enxerto,
        dirstats,
    })
}

/// Nomes dos bits de ext flags (sys/stat.h do SDK).
pub fn ext_flags_nomes(f: u64) -> Vec<&'static str> {
    const N: [(u64, &str); 7] = [
        (0x01, "MAY_SHARE_BLOCKS"),
        (0x02, "NO_XATTRS"),
        (0x04, "IS_SYNC_ROOT"),
        (0x08, "IS_PURGEABLE"),
        (0x10, "IS_SPARSE"),
        (0x20, "IS_SYNTHETIC"),
        (0x40, "SHARES_ALL_BLOCKS"),
    ];
    N.iter().filter(|x| f & x.0 != 0).map(|x| x.1).collect()
}

/// Nomes dos bits de st_flags que importam para o tamanho (sys/stat.h).
pub fn flags_nomes(f: u32) -> Vec<&'static str> {
    const N: [(u32, &str); 5] = [
        (0x20, "UF_COMPRESSED"),
        (0x40, "UF_TRACKED"),
        (0x8000, "UF_HIDDEN"),
        (0x0008_0000, "SF_RESTRICTED"),
        (parse::SF_DATALESS, "SF_DATALESS"),
    ];
    N.iter().filter(|x| f & x.0 != 0).map(|x| x.1).collect()
}

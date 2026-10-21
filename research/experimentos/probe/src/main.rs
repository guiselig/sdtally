//! Experimento: o que getattrlist(2) devolve por arquivo no APFS, sem root.
//! Uso: sdc-probe <arquivo>...

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

// Ausentes no crate libc 0.2.189 (valores de <sys/stat.h> e <sys/attr.h> do SDK).
const SF_DATALESS: u32 = 0x4000_0000;
const ATTR_CMNEXT_CLONE_REFCNT: u32 = 0x0000_1000;
const EF_MAY_SHARE_BLOCKS: u64 = 0x01;
const EF_IS_PURGEABLE: u64 = 0x08;
const EF_IS_SPARSE: u64 = 0x10;
const EF_SHARES_ALL_BLOCKS: u64 = 0x40;

#[derive(Debug, Default)]
struct Info {
    returned: [u32; 5],
    objtype: u32,
    flags: u32,
    fileid: u64,
    linkcount: u32,
    logical: i64,
    alloc: i64,
    private: i64,
    cloneid: u64,
    ext: u64,
    clone_refcnt: u32,
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn u32(&mut self) -> u32 {
        let v = u32::from_ne_bytes(self.buf[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        v
    }
    fn u64(&mut self) -> u64 {
        let v = u64::from_ne_bytes(self.buf[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        v
    }
}

fn probe(path: &Path) -> std::io::Result<Info> {
    let mut al: libc::attrlist = unsafe { std::mem::zeroed() };
    al.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
    al.commonattr = libc::ATTR_CMN_RETURNED_ATTRS
        | libc::ATTR_CMN_OBJTYPE
        | libc::ATTR_CMN_FLAGS
        | libc::ATTR_CMN_FILEID;
    al.fileattr = libc::ATTR_FILE_LINKCOUNT | libc::ATTR_FILE_TOTALSIZE | libc::ATTR_FILE_ALLOCSIZE;
    // Com FSOPT_ATTR_CMN_EXTENDED, forkattr passa a significar ATTR_CMNEXT_*.
    al.forkattr = libc::ATTR_CMNEXT_PRIVATESIZE
        | libc::ATTR_CMNEXT_CLONEID
        | libc::ATTR_CMNEXT_EXT_FLAGS
        | ATTR_CMNEXT_CLONE_REFCNT;

    let c = CString::new(path.as_os_str().as_bytes()).unwrap();
    let mut buf = [0u8; 256];
    let opts = libc::FSOPT_NOFOLLOW | libc::FSOPT_ATTR_CMN_EXTENDED | libc::FSOPT_PACK_INVAL_ATTRS;
    let rc = unsafe {
        libc::getattrlist(
            c.as_ptr(),
            &mut al as *mut _ as *mut libc::c_void,
            buf.as_mut_ptr() as *mut libc::c_void,
            buf.len(),
            opts as _,
        )
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }

    // Layout: u32 tamanho, attribute_set, comuns em ordem de bit, arquivo, cmnext.
    let mut r = Reader { buf: &buf, pos: 4 };
    let mut i = Info::default();
    for k in 0..5 {
        i.returned[k] = r.u32();
    }
    i.objtype = r.u32();
    i.flags = r.u32();
    i.fileid = r.u64();
    i.linkcount = r.u32();
    i.logical = r.u64() as i64;
    i.alloc = r.u64() as i64;
    i.private = r.u64() as i64;
    i.cloneid = r.u64();
    i.ext = r.u64();
    i.clone_refcnt = r.u32();
    Ok(i)
}

fn gib(b: i64) -> String {
    format!("{:.3} GiB", b as f64 / (1u64 << 30) as f64)
}

fn main() {
    for arg in std::env::args_os().skip(1) {
        let p = Path::new(&arg);
        match probe(p) {
            Err(e) => println!("{}: erro {e}", p.display()),
            Ok(i) => {
                let mut tags = vec![];
                if i.flags & SF_DATALESS != 0 { tags.push("DATALESS"); }
                if i.ext & EF_MAY_SHARE_BLOCKS != 0 { tags.push("MAY_SHARE"); }
                if i.ext & EF_SHARES_ALL_BLOCKS != 0 { tags.push("SHARES_ALL"); }
                if i.ext & EF_IS_SPARSE != 0 { tags.push("SPARSE"); }
                if i.ext & EF_IS_PURGEABLE != 0 { tags.push("PURGEABLE"); }
                println!("{}", p.display());
                println!(
                    "  lógico {} | alocado {} | privado {} | links {} | fileid {} | cloneid {} | clones {} | cmnext retornados 0x{:x}",
                    gib(i.logical), gib(i.alloc), gib(i.private), i.linkcount, i.fileid,
                    i.cloneid, i.clone_refcnt, i.returned[4]
                );
                println!("  flags [{}] tipo {}", tags.join(","), i.objtype);
            }
        }
    }
}

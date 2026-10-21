//! Mede cada volume montado sem root: statfs(2) + getattrlist(2) ATTR_VOL_*.
//! Compara os dois e agrupa por container APFS (diskN de /dev/diskNsM).
//! Pula snapshots montados (MNT_SNAPSHOT) e o que não é apfs.
//!
//! Uso: vols

use std::collections::BTreeMap;
use std::ffi::{CStr, CString};

const MNT_SNAPSHOT: u32 = 0x4000_0000; // <sys/mount.h>

#[derive(Debug, Default)]
struct VolAttrs { size: i64, free: i64, avail: i64, objs: u32, files: u32, dirs: u32, used: i64, quota: i64, reserved: i64 }

fn vol_attrs(mnt: &CStr) -> std::io::Result<VolAttrs> {
    let mut al: libc::attrlist = unsafe { std::mem::zeroed() };
    al.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
    al.commonattr = libc::ATTR_CMN_RETURNED_ATTRS;
    al.volattr = libc::ATTR_VOL_INFO | libc::ATTR_VOL_SIZE | libc::ATTR_VOL_SPACEFREE | libc::ATTR_VOL_SPACEAVAIL
        | libc::ATTR_VOL_OBJCOUNT | libc::ATTR_VOL_FILECOUNT | libc::ATTR_VOL_DIRCOUNT
        | libc::ATTR_VOL_SPACEUSED | libc::ATTR_VOL_QUOTA_SIZE | libc::ATTR_VOL_RESERVED_SIZE;
    let mut buf = [0u8; 512];
    let r = unsafe { libc::getattrlist(mnt.as_ptr(), (&mut al as *mut libc::attrlist).cast(), buf.as_mut_ptr().cast(), buf.len(), 0) };
    if r != 0 { return Err(std::io::Error::last_os_error()) }
    // layout: u32 length | attribute_set_t (5×u32) | valores na ordem dos bits (alinhados a 4)
    let rd32 = |o: usize| u32::from_ne_bytes(buf[o..o + 4].try_into().unwrap());
    let rd64 = |o: usize| i64::from_ne_bytes(buf[o..o + 8].try_into().unwrap());
    if std::env::var_os("VOLS_HEX").is_some() { let len = rd32(0) as usize; eprintln!("{:?} len {len} {:02x?}", mnt, &buf[..len]); }
    let volset = rd32(8); // após length: commonattr(4) volattr(8) dirattr fileattr forkattr
    let mut o = 24;
    let mut v = VolAttrs::default();
    let take64 = |bit: u32, o: &mut usize| -> i64 { if volset & bit != 0 { let x = rd64(*o); *o += 8; x } else { -1 } };
    v.size = take64(libc::ATTR_VOL_SIZE, &mut o);
    v.free = take64(libc::ATTR_VOL_SPACEFREE, &mut o);
    v.avail = take64(libc::ATTR_VOL_SPACEAVAIL, &mut o);
    // Medido (hexdump): o kernel empacota SPACEUSED logo após SPACEAVAIL, fora da ordem
    // dos bits (xnu vfs_attrlist.c). Não confiar em "ordem de bit" para ATTR_VOL_*.
    v.used = take64(libc::ATTR_VOL_SPACEUSED, &mut o);
    for (bit, slot) in [(libc::ATTR_VOL_OBJCOUNT, &mut v.objs), (libc::ATTR_VOL_FILECOUNT, &mut v.files), (libc::ATTR_VOL_DIRCOUNT, &mut v.dirs)] {
        if volset & bit != 0 { *slot = rd32(o); o += 4 }
    }
    v.quota = take64(libc::ATTR_VOL_QUOTA_SIZE, &mut o);
    v.reserved = take64(libc::ATTR_VOL_RESERVED_SIZE, &mut o);
    Ok(v)
}

fn cstr(a: &[libc::c_char]) -> String { unsafe { CStr::from_ptr(a.as_ptr()) }.to_string_lossy().into_owned() }
fn gib(b: i64) -> String { format!("{:.3}", b as f64 / (1u64 << 30) as f64) }

fn main() {
    let mut p: *mut libc::statfs = std::ptr::null_mut();
    let n = unsafe { libc::getmntinfo(&mut p, libc::MNT_NOWAIT) };
    let mounts = unsafe { std::slice::from_raw_parts(p, n as usize) };
    println!("dev\tmount\tblocks\tbfree\tbavail\tbsize\tstatfs_used_GiB\tspaceused_GiB\tsize_GiB\tfree_GiB\tavail_GiB\tfiles\tdirs\tquota\treserved\tfsid");
    let mut per_container: BTreeMap<String, (i64, i64, i64)> = BTreeMap::new(); // soma used, size, free
    for m in mounts {
        let fstype = cstr(&m.f_fstypename);
        // "/" é montado de um snapshot (disk3s1s1, SSV) e tem MNT_SNAPSHOT: manter.
        // Snapshots do Time Machine têm "nome@/dev/diskNsM" no mntfrom: pular.
        if fstype != "apfs" { continue }
        let snap = m.f_flags & MNT_SNAPSHOT != 0;
        if snap && cstr(&m.f_mntfromname).contains('@') { continue }
        let from = cstr(&m.f_mntfromname);
        let on = cstr(&m.f_mntonname);
        let bs = m.f_bsize as i64;
        let statfs_used = (m.f_blocks as i64 - m.f_bfree as i64) * bs;
        let c = CString::new(on.clone()).unwrap();
        match vol_attrs(&c) {
            Ok(v) => {
                let fsid: [i32; 2] = unsafe { std::mem::transmute(m.f_fsid) };
                println!("{from}\t{on}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:x}:{:x}", m.f_blocks, m.f_bfree, m.f_bavail, m.f_bsize, gib(statfs_used), gib(v.used), gib(v.size), gib(v.free), gib(v.avail),
                    v.files, v.dirs, v.quota, v.reserved, fsid[0], fsid[1]);
                // /dev/disk3s5 -> disk3 ; nomes de snapshot já foram pulados
                let dev = from.trim_start_matches("/dev/");
                // disk3s1s1 (snapshot do SSV) -> disk3s1 -> disk3
                let dev = if dev.matches('s').count() > 2 { dev.rsplit_once('s').unwrap().0 } else { dev };
                let container = dev.rsplit_once('s').map(|(a, _)| a).unwrap_or(dev).to_string();
                let e = per_container.entry(container).or_default();
                e.0 += v.used; e.1 = v.size; e.2 = v.free;
            }
            Err(e) => println!("{from}\t{on}\tgetattrlist: {e}"),
        }
    }
    println!("\n# soma de ATTR_VOL_SPACEUSED dos volumes MONTADOS, por container (volumes não montados ficam de fora):");
    for (c, (used, size, free)) in per_container {
        println!("{c}\tsoma_usado {} GiB\tcontainer tamanho {} GiB\tlivre {} GiB\ttamanho-livre {} GiB\tdiferença {} bytes",
            gib(used), gib(size), gib(free), gib(size - free), size - free - used);
    }
}

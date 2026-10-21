//! Arquivos apagados mas ainda abertos (nlink == 0): ocupam disco e nenhuma
//! varredura de árvore os vê. Equivalente a `lsof +L1`, via libproc, sem root.
//!
//! proc_listallpids -> proc_pidinfo(PROC_PIDLISTFDS) -> proc_pidfdinfo(PROC_PIDFDVNODEPATHINFO)
//! Sem root só se inspecionam processos do mesmo uid (os outros dão EPERM e são contados).
//! Soma deduplicada por (dev, ino); tamanho = vst_blocks × 512 (alocado), não vst_size.
//!
//! Uso: deleted_open [--all]   (--all: lista todos os arquivos, não só os 25 maiores)

use std::collections::BTreeMap;
use std::ffi::CStr;

const PROC_PIDFDVNODEPATHINFO: libc::c_int = 2; // <sys/proc_info.h>

#[repr(C)]
struct ProcFileInfo { fi_openflags: u32, fi_status: u32, fi_offset: libc::off_t, fi_type: i32, fi_guardflags: u32 }

#[repr(C)]
struct VnodeFdInfoWithPath { pfi: ProcFileInfo, pvip: libc::vnode_info_path }

const PROC_PIDREGIONPATHINFO: libc::c_int = 8;

#[repr(C)]
struct ProcRegionInfo {
    pri_protection: u32, pri_max_protection: u32, pri_inheritance: u32, pri_flags: u32, pri_offset: u64,
    pri_behavior: u32, pri_user_wired_count: u32, pri_user_tag: u32, pri_pages_resident: u32,
    pri_pages_shared_now_private: u32, pri_pages_swapped_out: u32, pri_pages_dirtied: u32, pri_ref_count: u32,
    pri_shadow_depth: u32, pri_share_mode: u32, pri_private_pages_resident: u32, pri_shared_pages_resident: u32,
    pri_obj_id: u32, pri_depth: u32, pri_address: u64, pri_size: u64,
}

#[repr(C)]
struct RegionWithPath { prp_prinfo: ProcRegionInfo, prp_vip: libc::vnode_info_path }

struct Hit { pids: Vec<(i32, String)>, alloc: i64, size: i64, path: String, dev: u32 }

fn pid_name(pid: i32) -> String {
    let mut buf = [0u8; 64];
    let n = unsafe { libc::proc_name(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
    String::from_utf8_lossy(&buf[..n.max(0) as usize]).into_owned()
}

fn main() {
    let verbose = std::env::args().any(|a| a == "--all");
    let n = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    let mut pids = vec![0i32; n as usize + 64];
    let n = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), (pids.len() * 4) as i32) };
    pids.truncate(n.max(0) as usize);

    let (mut ok, mut denied) = (0u32, 0u32);
    let mut hits: BTreeMap<(u32, u64), Hit> = BTreeMap::new();
    let mut no_stat: std::collections::BTreeSet<String> = Default::default();
    for &pid in &pids {
        if pid == 0 { continue }
        let bytes = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, std::ptr::null_mut(), 0) };
        if bytes <= 0 { denied += 1; continue }
        let cap = bytes as usize / size_of::<libc::proc_fdinfo>() + 32;
        let mut fds: Vec<libc::proc_fdinfo> = Vec::with_capacity(cap);
        let got = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, fds.as_mut_ptr().cast(), (cap * size_of::<libc::proc_fdinfo>()) as i32) };
        if got <= 0 { denied += 1; continue }
        unsafe { fds.set_len(got as usize / size_of::<libc::proc_fdinfo>()) };
        ok += 1;
        for fd in &fds {
            if fd.proc_fdtype != libc::PROX_FDTYPE_VNODE as u32 { continue }
            let mut vi: VnodeFdInfoWithPath = unsafe { std::mem::zeroed() };
            let sz = size_of::<VnodeFdInfoWithPath>() as i32;
            let r = unsafe { libc::proc_pidfdinfo(pid, fd.proc_fd, PROC_PIDFDVNODEPATHINFO, (&mut vi as *mut VnodeFdInfoWithPath).cast(), sz) };
            if r != sz { continue }
            let st = &vi.pvip.vip_vi.vi_stat;
            if st.vst_nlink != 0 || (st.vst_mode & libc::S_IFMT) != libc::S_IFREG { continue }
            let path = unsafe { CStr::from_ptr(vi.pvip.vip_path.as_ptr().cast()) }.to_string_lossy().into_owned();
            let e = hits.entry((st.vst_dev, st.vst_ino)).or_insert(Hit { pids: vec![], alloc: st.vst_blocks * 512, size: st.vst_size, path, dev: st.vst_dev });
            e.pids.push((pid, pid_name(pid)));
        }
        // Mapeamentos em memória (lsof mostra como FD "txt"): arquivo apagado mas mapeado
        // também segura blocos. Não aparece em PROC_PIDLISTFDS.
        let mut addr: u64 = 0;
        loop {
            let mut rp: RegionWithPath = unsafe { std::mem::zeroed() };
            let sz = size_of::<RegionWithPath>() as i32;
            let r = unsafe { libc::proc_pidinfo(pid, PROC_PIDREGIONPATHINFO, addr, (&mut rp as *mut RegionWithPath).cast(), sz) };
            if r != sz { break }
            let next = rp.prp_prinfo.pri_address + rp.prp_prinfo.pri_size;
            if next <= addr { break }
            addr = next;
            let st = &rp.prp_vip.vip_vi.vi_stat;
            // vi_stat zerado com caminho presente: o kernel não fez vn_stat (medido em
            // ~/…/com.apple.LaunchServices.dv, data vault). O /usr/sbin/lsof tem o entitlement
            // com.apple.rootless.datavault.metadata e vê; binário comum não. Contar à parte.
            if st.vst_ino == 0 {
                let p = unsafe { CStr::from_ptr(rp.prp_vip.vip_path.as_ptr().cast()) }.to_string_lossy().into_owned();
                if !p.is_empty() { no_stat.insert(p); }
                continue;
            }
            if st.vst_nlink != 0 || (st.vst_mode & libc::S_IFMT) != libc::S_IFREG { continue }
            let path = unsafe { CStr::from_ptr(rp.prp_vip.vip_path.as_ptr().cast()) }.to_string_lossy().into_owned();
            let e = hits.entry((st.vst_dev, st.vst_ino)).or_insert(Hit { pids: vec![], alloc: st.vst_blocks * 512, size: st.vst_size, path, dev: st.vst_dev });
            if !e.pids.iter().any(|(p, _)| *p == pid) { e.pids.push((pid, format!("{}[mmap]", pid_name(pid)))) }
        }
    }

    let mut v: Vec<_> = hits.values().collect();
    v.sort_by_key(|h| -h.alloc);
    let total_alloc: i64 = v.iter().map(|h| h.alloc).sum();
    let total_size: i64 = v.iter().map(|h| h.size).sum();
    println!("# processos inspecionados {ok}, negados (outro uid, sem root) {denied}");
    println!("# arquivos únicos com nlink=0: {}  alocado {:.3} GiB  aparente {:.3} GiB", v.len(),
        total_alloc as f64 / (1u64 << 30) as f64, total_size as f64 / (1u64 << 30) as f64);
    println!("# mapeamentos sem stat (data vault; tamanho/nlink não medíveis sem entitlement): {} caminhos únicos", no_stat.len());
    let mut per_dev: BTreeMap<u32, i64> = BTreeMap::new();
    for h in &v { *per_dev.entry(h.dev).or_default() += h.alloc }
    println!("# alocado por st_dev: {:?}", per_dev.iter().map(|(d, b)| (format!("{d:#x}"), *b)).collect::<Vec<_>>());
    println!("alocado_bytes\taparente_bytes\tprocessos\tcaminho_no_momento_da_abertura");
    for h in v.iter().take(if verbose { usize::MAX } else { 25 }) {
        let names: Vec<String> = h.pids.iter().take(4).map(|(p, n)| format!("{n}({p})")).collect();
        let more = if h.pids.len() > 4 { format!(" +{}", h.pids.len() - 4) } else { String::new() };
        println!("{}\t{}\t{}{more}\t{}", h.alloc, h.size, names.join(","), h.path);
    }
}

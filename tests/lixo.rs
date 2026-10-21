//! Lixo por idade (`sources::lixo`): a idade vem de ATTR_CMN_ADDEDTIME, nunca do mtime. A pasta é
//! temporária; o ADDEDTIME é gravado aqui no teste (setattrlist), porque o código de produção só lê.

use sdtally::sources::lixo::{self, Lixo};
use sdtally::walk::Controle;
use std::os::darwin::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

const DIA: i64 = 86_400;
const AGORA: i64 = 1_800_000_000;

struct Pasta(PathBuf);

impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn pasta(nome: &str) -> Pasta {
    let p = std::env::temp_dir().join(format!("sdtally-lixo-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("criar pasta de teste");
    Pasta(p)
}

#[allow(unsafe_code)]
fn data_de_entrada(p: &Path, t: i64) {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(p.as_os_str().as_bytes()).expect("caminho");
    let mut al: libc::attrlist = unsafe { std::mem::zeroed() };
    al.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
    al.commonattr = libc::ATTR_CMN_ADDEDTIME;
    let mut ts = libc::timespec { tv_sec: t, tv_nsec: 0 };
    let rc = unsafe { libc::setattrlist(c.as_ptr(), &mut al as *mut _ as *mut libc::c_void, &mut ts as *mut _ as *mut libc::c_void, std::mem::size_of::<libc::timespec>(), 0) };
    assert_eq!(rc, 0, "setattrlist ADDEDTIME: {}", std::io::Error::last_os_error());
}

fn escrever(p: &Path, n: usize) -> i64 {
    std::fs::write(p, vec![7u8; n]).expect("escrever");
    std::fs::symlink_metadata(p).expect("stat").st_blocks() as i64 * 512
}

fn medir(p: &Path) -> Result<Lixo, String> {
    lixo::medir(p, AGORA, 2, Arc::new(Controle::new(&sdtally::sys::INTERROMPIDO)))
}

#[test]
fn idade_pela_data_de_entrada_e_nao_pelo_mtime() {
    let t = pasta("idade");
    let r = &t.0;
    // Arquivo velho no Lixo; mtime recente de propósito.
    let a = escrever(&r.join("velho.txt"), 100_000);
    data_de_entrada(&r.join("velho.txt"), AGORA - 40 * DIA);
    std::fs::File::options().write(true).open(r.join("velho.txt")).expect("abrir").set_modified(SystemTime::now()).expect("mtime");
    // Pasta velha com dois arquivos dentro (a data é a da pasta, não a dos arquivos).
    std::fs::create_dir(r.join("pasta")).expect("pasta");
    let b = escrever(&r.join("pasta/a.bin"), 50_000) + escrever(&r.join("pasta/b.bin"), 20_000);
    data_de_entrada(&r.join("pasta"), AGORA - 31 * DIA);
    // Arquivo novo, com mtime de 2023 (copiado de longe): a idade é a de hoje.
    let c = escrever(&r.join("novo.txt"), 30_000);
    data_de_entrada(&r.join("novo.txt"), AGORA - 2 * DIA);
    std::fs::File::options().write(true).open(r.join("novo.txt")).expect("abrir").set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_680_000_000)).expect("mtime");
    // Exatamente 30 dias não passa de 30.
    let d = escrever(&r.join("limite.txt"), 10_000);
    data_de_entrada(&r.join("limite.txt"), AGORA - 30 * DIA);

    let m = medir(r).expect("medir");
    assert_eq!(m, Lixo { itens: 4, bytes: a + b + c + d, velhos_itens: 2, velhos_bytes: a + b, ..Lixo::default() });
    assert!(m.velhos_bytes > 0);
    assert_eq!(m.medida_velhos(), sdtally::ledger::Medida::Valor(a + b));
}

#[test]
fn lixo_vazio_e_ausente_sao_zero_medido() {
    let t = pasta("vazio");
    assert_eq!(medir(&t.0), Ok(Lixo::default()));
    assert_eq!(medir(&t.0.join("nao-existe")), Ok(Lixo::default()));
}

#[test]
fn pasta_ilegivel_e_motivo_nao_zero() {
    use std::os::unix::fs::PermissionsExt;
    let t = pasta("negada");
    std::fs::set_permissions(&t.0, std::fs::Permissions::from_mode(0o000)).expect("chmod");
    let r = medir(&t.0);
    std::fs::set_permissions(&t.0, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    // Como root a leitura passa; o que não pode é virar Ok com número inventado.
    match r {
        Err(m) => assert!(m.contains("errno"), "{m}"),
        Ok(l) => assert_eq!(l, Lixo::default()),
    }
}

#[test]
fn sem_data_nao_entra_na_idade() {
    let l = Lixo { itens: 3, bytes: 30, velhos_itens: 1, velhos_bytes: 10, sem_data_itens: 1, sem_data_bytes: 5, piso: false };
    assert!(matches!(l.medida_velhos(), sdtally::ledger::Medida::Parcial(10, _)));
}

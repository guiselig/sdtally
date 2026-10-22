//! Contadores do volume APFS pelo AppleAPFSUserClient (IOKit), sem root e sem escrita: só os
//! seletores de leitura 6, 7 e 0x1e que o APFS.framework chama nas funções Get* (Frente G §2.2).
//! Privados e conferidos só no macOS 27: qualquer erro vira "não medido", nunca um número.

use std::ffi::{c_char, c_int, c_void};

type Porta = u32;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOBSDNameMatching(main_port: Porta, opcoes: u32, nome: *const c_char) -> *mut c_void;
    fn IOServiceGetMatchingService(main_port: Porta, matching: *mut c_void) -> Porta;
    fn IORegistryEntryGetParentEntry(entrada: Porta, plano: *const c_char, pai: *mut Porta) -> c_int;
    fn IOObjectConformsTo(objeto: Porta, classe: *const c_char) -> c_int;
    fn IOObjectRelease(objeto: Porta) -> c_int;
    fn IOServiceOpen(servico: Porta, tarefa: Porta, tipo: u32, conexao: *mut Porta) -> c_int;
    fn IOServiceClose(conexao: Porta) -> c_int;
    fn IOConnectCallStructMethod(conexao: Porta, seletor: u32, entrada: *const c_void, n_entrada: usize, saida: *mut c_void, n_saida: *mut usize) -> c_int;
}

unsafe extern "C" {
    /// Porta da própria tarefa (libSystem); o `libc` marca a dele como obsoleta.
    static mach_task_self_: Porta;
}

const SEL_CONTAINER: u32 = 6;
const SEL_VOLUME_USADO: u32 = 7;
const SEL_ESTATISTICAS: u32 = 0x1e;
const TAM_CONTAINER: usize = 0x4c8;
const TAM_ESTATISTICAS: usize = 0x110;

/// Contadores de um volume APFS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EstatVolume {
    /// Tamanho do bloco do container (seletor 6, offset 0x08).
    pub bloco: u64,
    /// `fs_used` do volume (seletor 7); serve para conferir que o índice é o volume pedido.
    pub usado: u64,
    pub arquivos: u64,
    pub pastas: u64,
    pub nos_omap: u64,
    pub nos_fsroot: u64,
    pub nos_extentref: u64,
}

impl EstatVolume {
    /// Metadados das árvores vivas do volume: (omap + fsroot + extentref) × bloco. None se
    /// estoura (contador de outra versão do macOS): não vira número.
    pub fn metadados(&self) -> Option<u64> {
        self.nos_omap.checked_add(self.nos_fsroot)?.checked_add(self.nos_extentref)?.checked_mul(self.bloco)
    }
}

fn u64_em(b: &[u8], off: usize) -> Option<u64> {
    b.get(off..off + 8).map(|s| u64::from_ne_bytes(s.try_into().unwrap_or([0; 8])))
}

/// Decodifica as três respostas (layout da Frente G §2.2). None se alguma veio curta.
pub(crate) fn decodificar(container: &[u8], usado: &[u8], estat: &[u8]) -> Option<EstatVolume> {
    Some(EstatVolume {
        bloco: u64_em(container, 0x08)?,
        usado: u64_em(usado, 8)?,
        arquivos: u64_em(estat, 0x08)?,
        pastas: u64_em(estat, 0x10)?,
        nos_omap: u64_em(estat, 0x20)?,
        nos_fsroot: u64_em(estat, 0x30)?,
        nos_extentref: u64_em(estat, 0x28)?,
    })
}

/// Fecha o que abriu, em qualquer saída.
struct Objeto(Porta);

impl Drop for Objeto {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe { IOObjectRelease(self.0) };
        }
    }
}

struct Conexao(Porta);

impl Drop for Conexao {
    fn drop(&mut self) {
        unsafe { IOServiceClose(self.0) };
    }
}

/// Contadores do volume `bsd` ("disk3s5"). O índice nos seletores é o sufixo `sN` menos 1 (o
/// registro do IOKit mostra "Preboot@2" para disk3s2). Err(motivo em pt).
pub fn estatisticas_volume(bsd: &str) -> Result<EstatVolume, String> {
    let idx = bsd
        .rsplit_once('s')
        .and_then(|(_, n)| n.parse::<u32>().ok())
        .filter(|n| *n >= 1)
        .map(|n| n - 1)
        .ok_or_else(|| format!("volume {bsd} sem sufixo sN"))?;
    let nome = std::ffi::CString::new(bsd).map_err(|_| "nome de volume inválido".to_string())?;
    let vol = unsafe {
        let m = IOBSDNameMatching(0, 0, nome.as_ptr());
        if m.is_null() {
            return Err("IOBSDNameMatching sem resposta".into());
        }
        Objeto(IOServiceGetMatchingService(0, m))
    };
    if vol.0 == 0 {
        return Err(format!("volume {bsd} fora do registro do IOKit"));
    }
    // O pai do volume no plano IOService é o AppleAPFSContainer.
    let mut pai = 0;
    let rc = unsafe { IORegistryEntryGetParentEntry(vol.0, c"IOService".as_ptr(), &mut pai) };
    let pai = Objeto(pai);
    if rc != 0 || pai.0 == 0 || unsafe { IOObjectConformsTo(pai.0, c"AppleAPFSContainer".as_ptr()) } == 0 {
        return Err(format!("o pai de {bsd} não é um AppleAPFSContainer"));
    }
    let mut con = 0;
    let rc = unsafe { IOServiceOpen(pai.0, mach_task_self_, 0, &mut con) };
    if rc != 0 {
        return Err(format!("IOServiceOpen do container falhou (0x{rc:x})"));
    }
    let con = Conexao(con);
    let chamar = |sel: u32, entrada: Option<u32>, saida: &mut [u8]| -> Result<usize, String> {
        let mut n = saida.len();
        let (p, ni) = entrada.as_ref().map_or((std::ptr::null(), 0), |e| (e as *const u32 as *const c_void, 4));
        let rc = unsafe { IOConnectCallStructMethod(con.0, sel, p, ni, saida.as_mut_ptr() as *mut c_void, &mut n) };
        if rc != 0 { Err(format!("seletor 0x{sel:x} do AppleAPFSUserClient falhou (0x{rc:x})")) } else { Ok(n) }
    };
    let mut c = vec![0u8; TAM_CONTAINER];
    let mut u = [0u8; 16];
    let mut e = vec![0u8; TAM_ESTATISTICAS];
    let nc = chamar(SEL_CONTAINER, None, &mut c)?;
    let nu = chamar(SEL_VOLUME_USADO, Some(idx), &mut u)?;
    let ne = chamar(SEL_ESTATISTICAS, Some(idx), &mut e)?;
    if nc < TAM_CONTAINER || nu < 16 || ne < TAM_ESTATISTICAS {
        return Err("resposta curta do AppleAPFSUserClient".into());
    }
    decodificar(&c, &u, &e).ok_or_else(|| "resposta curta do AppleAPFSUserClient".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodifica_offsets() {
        let mut c = vec![0u8; TAM_CONTAINER];
        c[0x08..0x10].copy_from_slice(&4096u64.to_ne_bytes());
        let mut u = [0u8; 16];
        u[8..].copy_from_slice(&777u64.to_ne_bytes());
        let mut e = vec![0u8; TAM_ESTATISTICAS];
        for (off, v) in [(0x08, 10u64), (0x10, 3), (0x20, 100), (0x28, 5), (0x30, 1000)] {
            e[off..off + 8].copy_from_slice(&v.to_ne_bytes());
        }
        let s = decodificar(&c, &u, &e).expect("decodifica");
        assert_eq!((s.arquivos, s.pastas, s.usado), (10, 3, 777));
        assert_eq!(s.metadados(), Some((100 + 1000 + 5) * 4096));
        assert!(decodificar(&c, &u, &e[..0x20]).is_none());
    }

    #[test]
    fn nome_ruim_nao_chama_o_kernel() {
        assert!(estatisticas_volume("disk3").unwrap_err().contains("sufixo"));
        assert!(estatisticas_volume("disk3s0").unwrap_err().contains("sufixo"));
    }
}

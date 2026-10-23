//! Cópias idênticas (só na medição completa, a mesma rodada que monta `largest_files`; nunca no
//! `--auto`, no `--quick` nem no ramo). Os candidatos vêm da travessia (`walk`): arquivo comum de
//! 100 MB ou mais, fora da nuvem (dataless nunca é aberto), um por hardlink (FILEID) e um por grupo
//! de clone completo (CLONEID); clone parcial que divide blocos fica de fora. Aqui: agrupa pelo
//! tamanho lógico exato; nos grupos de 2 ou mais compara o hash do primeiro e do último MiB e, onde
//! bate, o conteúdo inteiro byte a byte. O hash (SipHash da std, chave fixa) só separa; quem decide
//! é a comparação dos bytes, sem falso positivo. Só lê: open O_RDONLY sem seguir symlink em nenhum
//! componente, F_NOCACHE, e o arquivo aberto tem de ser o da travessia (volume, FILEID, tamanho,
//! não dataless). Para no orçamento (bytes lidos ou tempo) ou no Ctrl-C: aí sai parcial.

use crate::walk::Controle;
use std::collections::BTreeMap;
use std::fs::File;
use std::hash::{DefaultHasher, Hasher};
use std::os::macos::fs::MetadataExt;
use std::os::unix::fs::{FileExt, OpenOptionsExt};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// No máximo isto de candidatos (a lista vem da travessia); passou, a lista é cortada e sai parcial.
pub const CAND_MAX: usize = 5000;
/// Orçamento da comparação: bytes lidos e tempo.
pub const LIMITE_BYTES: u64 = 20_000_000_000;
pub const LIMITE_TEMPO: Duration = Duration::from_secs(60);
/// Pontas comparadas antes do conteúdo inteiro.
const PONTA: u64 = 1 << 20;
/// Bloco da comparação inteira, por arquivo aberto.
const BLOCO: usize = 1 << 20;
/// Arquivos comparados juntos (memória: MEMBROS_MAX × BLOCO); o resto do grupo sai parcial.
const MEMBROS_MAX: usize = 16;

/// Motivos de parcial (pt; o JSON traduz por `i18n::MOTIVOS_EN`).
pub const M_CHEIO: &str = "mais de 5000 candidatos";
pub const M_TRAVESSIA: &str = "travessia interrompida";
const M_BYTES: &str = "limite de 20 GB lidos na comparação";
const M_TEMPO: &str = "limite de 60 s na comparação";
const M_INTERROMPIDO: &str = "interrompido";
const M_MUDOU: &str = "arquivo que mudou ou não abriu durante a comparação";
const M_MUITOS: &str = "mais de 16 arquivos com o mesmo tamanho e as mesmas pontas";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cand {
    pub caminho: PathBuf,
    pub alloc: i64,
    /// TOTALSIZE da travessia: o tamanho que o arquivo aberto tem de ter.
    pub logico: u64,
    pub fileid: u64,
}

/// O que a travessia entrega: candidatos já deduplicados (em ordem de caminho), o volume deles e
/// por que a lista pode estar incompleta.
#[derive(Debug, Clone, Default)]
pub struct Candidatos {
    pub lista: Vec<Cand>,
    pub dev: u32,
    pub motivos: Vec<&'static str>,
}

/// Cópias de conteúdo idêntico. `bytes_each` é o menor alocado entre elas (piso do que sobra).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grupo {
    pub bytes_each: i64,
    pub caminhos: Vec<PathBuf>,
}

impl Grupo {
    pub fn desperdicio(&self) -> i64 {
        self.bytes_each * (self.caminhos.len() as i64 - 1)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Duplicados {
    /// Todos os grupos achados, do maior desperdício para o menor (empate: caminhos).
    pub grupos: Vec<Grupo>,
    pub candidatos: u64,
    pub bytes_lidos: u64,
    pub motivos: Vec<&'static str>,
}

impl Duplicados {
    pub fn parcial(&self) -> bool {
        !self.motivos.is_empty()
    }

    pub fn desperdicio(&self) -> i64 {
        self.grupos.iter().map(Grupo::desperdicio).sum()
    }

    pub fn motivo(&self) -> Option<String> {
        self.parcial().then(|| self.motivos.join("; "))
    }
}

pub struct Orcamento {
    pub bytes: u64,
    pub tempo: Duration,
}

pub const ORCAMENTO: Orcamento = Orcamento { bytes: LIMITE_BYTES, tempo: LIMITE_TEMPO };

enum Parada {
    /// Orçamento ou Ctrl-C: para tudo.
    Fim,
    /// O arquivo não leu: sai só ele.
    Falha,
}

struct Leitor<'a> {
    orc: &'a Orcamento,
    ctl: &'a Controle,
    inicio: Instant,
    lidos: u64,
    parou: Option<&'static str>,
}

impl Leitor<'_> {
    /// `buf.len()` bytes em `off` (read_exact_at repete em EINTR), dentro do orçamento.
    fn ler(&mut self, f: &File, buf: &mut [u8], off: u64) -> Result<(), Parada> {
        if self.parou.is_none() {
            self.parou = if self.ctl.cancelado() {
                Some(M_INTERROMPIDO)
            } else if self.lidos + buf.len() as u64 > self.orc.bytes {
                Some(M_BYTES)
            } else if self.inicio.elapsed() > self.orc.tempo {
                Some(M_TEMPO)
            } else {
                None
            };
        }
        if self.parou.is_some() {
            return Err(Parada::Fim);
        }
        f.read_exact_at(buf, off).map_err(|_| Parada::Falha)?;
        self.lidos += buf.len() as u64;
        Ok(())
    }
}

/// Abre só para ler, sem seguir symlink em nenhum componente nem travar em FIFO, e confere que é o
/// arquivo da travessia: comum, mesmo volume, FILEID e tamanho, e não virou dataless.
fn abrir(c: &Cand, dev: u32) -> Option<File> {
    let f = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW_ANY | libc::O_NONBLOCK).open(&c.caminho).ok()?;
    let m = f.metadata().ok()?;
    let mesmo = m.file_type().is_file() && m.st_dev() as u32 == dev && m.st_ino() == c.fileid && m.st_size() == c.logico && m.st_flags() & crate::parse::SF_DATALESS == 0;
    if !mesmo {
        return None;
    }
    // Sem F_NOCACHE a leitura só fica no cache de páginas: segue.
    let _ = crate::sys::apfs::sem_cache(&f);
    Some(f)
}

/// Hash do primeiro e do último MiB.
fn pontas(f: &File, tam: u64, l: &mut Leitor, buf: &mut [u8]) -> Result<u64, Parada> {
    let n = tam.min(PONTA) as usize;
    let mut h = DefaultHasher::new();
    l.ler(f, &mut buf[..n], 0)?;
    h.write(&buf[..n]);
    if tam > PONTA {
        l.ler(f, &mut buf[..n], tam - n as u64)?;
        h.write(&buf[..n]);
    }
    Ok(h.finish())
}

/// Conteúdo inteiro, em blocos, de todos de uma vez: cada classe se divide pelo bloco lido e quem
/// fica sozinho sai. Devolve as classes de 2 ou mais e se algum arquivo falhou. Err: o orçamento
/// acabou no meio (nada deste grupo conta).
fn iguais<'a>(g: &[&'a Cand], tam: u64, dev: u32, l: &mut Leitor) -> Result<(Vec<Vec<&'a Cand>>, bool), ()> {
    let mut falhou = false;
    let abertos: Vec<(&Cand, File)> = g
        .iter()
        .filter_map(|x| {
            let f = abrir(x, dev);
            falhou |= f.is_none();
            Some((*x, f?))
        })
        .collect();
    let mut bufs = vec![vec![0u8; BLOCO.min(tam as usize)]; abertos.len()];
    let mut classes: Vec<Vec<usize>> = vec![(0..abertos.len()).collect()];
    let mut off = 0u64;
    while off < tam && !classes.is_empty() {
        let n = (tam - off).min(BLOCO as u64) as usize;
        let mut novas = Vec::new();
        for cl in classes {
            let mut lidos = Vec::new();
            for i in cl {
                match l.ler(&abertos[i].1, &mut bufs[i][..n], off) {
                    Ok(()) => lidos.push(i),
                    Err(Parada::Falha) => falhou = true,
                    Err(Parada::Fim) => return Err(()),
                }
            }
            let mut sub: Vec<Vec<usize>> = Vec::new();
            for i in lidos {
                match sub.iter_mut().find(|s| bufs[s[0]][..n] == bufs[i][..n]) {
                    Some(s) => s.push(i),
                    None => sub.push(vec![i]),
                }
            }
            novas.extend(sub.into_iter().filter(|s| s.len() >= 2));
        }
        classes = novas;
        off += n as u64;
    }
    Ok((classes.into_iter().filter(|c| c.len() >= 2).map(|c| c.into_iter().map(|i| abertos[i].0).collect()).collect(), falhou))
}

/// Acha as cópias idênticas entre os candidatos. Os tamanhos maiores vão primeiro: com o orçamento
/// curto, o que mais pesa sai antes.
pub fn achar(c: &Candidatos, ctl: &Controle, orc: &Orcamento) -> Duplicados {
    let mut d = Duplicados { candidatos: c.lista.len() as u64, motivos: c.motivos.clone(), ..Default::default() };
    let mut l = Leitor { orc, ctl, inicio: Instant::now(), lidos: 0, parou: None };
    let mut por_tam: BTreeMap<u64, Vec<&Cand>> = BTreeMap::new();
    for x in &c.lista {
        por_tam.entry(x.logico).or_default().push(x);
    }
    let (mut mudou, mut muitos) = (false, false);
    let mut buf = vec![0u8; PONTA as usize];
    'tam: for (&tam, v) in por_tam.iter().rev().filter(|(_, v)| v.len() >= 2) {
        let mut por_ponta: BTreeMap<u64, Vec<&Cand>> = BTreeMap::new();
        for x in v {
            let Some(f) = abrir(x, c.dev) else {
                mudou = true;
                continue;
            };
            match pontas(&f, tam, &mut l, &mut buf) {
                Ok(h) => por_ponta.entry(h).or_default().push(x),
                Err(Parada::Falha) => mudou = true,
                Err(Parada::Fim) => break 'tam,
            }
        }
        for mut g in por_ponta.into_values().filter(|g| g.len() >= 2) {
            if g.len() > MEMBROS_MAX {
                muitos = true;
                g.truncate(MEMBROS_MAX);
            }
            let Ok((classes, falhou)) = iguais(&g, tam, c.dev, &mut l) else { break 'tam };
            mudou |= falhou;
            for cl in classes {
                let mut caminhos: Vec<PathBuf> = cl.iter().map(|x| x.caminho.clone()).collect();
                caminhos.sort();
                d.grupos.push(Grupo { bytes_each: cl.iter().map(|x| x.alloc).min().unwrap_or(0), caminhos });
            }
        }
    }
    d.motivos.extend(l.parou);
    if mudou {
        d.motivos.push(M_MUDOU);
    }
    if muitos {
        d.motivos.push(M_MUITOS);
    }
    d.bytes_lidos = l.lidos;
    d.grupos.sort_by(|a, b| b.desperdicio().cmp(&a.desperdicio()).then_with(|| a.caminhos.cmp(&b.caminhos)));
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::AtomicBool;

    struct Pasta(PathBuf);
    impl Drop for Pasta {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn pasta(nome: &str) -> Pasta {
        let p = std::env::temp_dir().join(format!("{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("mkdir");
        Pasta(std::fs::canonicalize(&p).expect("canonicalize"))
    }

    fn cand(p: &Path) -> Cand {
        let m = std::fs::metadata(p).expect("stat");
        Cand { caminho: p.to_path_buf(), alloc: m.st_blocks() as i64 * 512, logico: m.st_size(), fileid: m.st_ino() }
    }

    /// 3 MiB com um byte trocado em `troca` (None: original).
    fn conteudo(troca: Option<usize>) -> Vec<u8> {
        let mut x = 0x9E37_79B9_7F4A_7C15u64;
        let mut v: Vec<u8> = (0..3 << 20)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x as u8
            })
            .collect();
        if let Some(i) = troca {
            v[i] ^= 0xFF;
        }
        v
    }

    /// a == b; c tem o mesmo tamanho e as mesmas pontas, mas difere no meio (só a comparação inteira separa).
    fn tres(p: &Path) -> Candidatos {
        std::fs::write(p.join("a"), conteudo(None)).expect("a");
        std::fs::write(p.join("b"), conteudo(None)).expect("b");
        std::fs::write(p.join("c"), conteudo(Some(3 << 19))).expect("c");
        let dev = std::fs::metadata(p).expect("stat").st_dev() as u32;
        Candidatos { lista: ["a", "b", "c"].iter().map(|n| cand(&p.join(n))).collect(), dev, motivos: Vec::new() }
    }

    #[test]
    fn so_conteudo_igual_conta() {
        static NUNCA: AtomicBool = AtomicBool::new(false);
        let p = pasta("sdtdup-igual");
        let c = tres(&p.0);
        let d = achar(&c, &Controle::new(&NUNCA), &ORCAMENTO);
        assert_eq!(d.motivos, Vec::<&str>::new());
        assert_eq!(d.grupos.len(), 1, "{d:?}");
        assert_eq!(d.grupos[0].caminhos, [p.0.join("a"), p.0.join("b")]);
        assert_eq!(d.desperdicio(), c.lista[0].alloc.min(c.lista[1].alloc));
        // Pontas dos 3 (2 MiB cada) + os 3 MiB de a e b + 2 de c, que sai no bloco do meio.
        assert_eq!((d.candidatos, d.bytes_lidos), (3, 3 * (2 << 20) + 2 * (3 << 20) + (2 << 20)));
    }

    /// Pela travessia: a == b (cópia de verdade), c do mesmo tamanho e diferente no meio, d clone
    /// de a (`cp -c`) e h hardlink de b. Só o par a/b conta; o clone e o hardlink não custam em dobro.
    #[test]
    fn travessia_tira_clone_e_hardlink() {
        use std::io::Write;
        static NUNCA: AtomicBool = AtomicBool::new(false);
        let p = pasta("sdtdup-walk");
        let bloco = conteudo(None);
        // 34 × 3 MiB ≈ 107 MB: acima do piso de 100 MB. b é escrito, não copiado (std::fs::copy clona).
        for (n, troca) in [("a", false), ("b", false), ("c", true)] {
            let mut f = std::fs::File::create(p.0.join(n)).expect("cria");
            for i in 0..34 {
                let mut b = bloco.clone();
                if troca && i == 17 {
                    b[0] ^= 0xFF;
                }
                f.write_all(&b).expect("escreve");
            }
        }
        if !std::process::Command::new("/bin/cp").arg("-c").arg(p.0.join("a")).arg(p.0.join("d")).status().is_ok_and(|s| s.success()) {
            return; // volume sem clone (não é APFS)
        }
        std::fs::hard_link(p.0.join("b"), p.0.join("h")).expect("ln");
        let r = crate::walk::walk(p.0.to_str().expect("utf8"), 2, std::sync::Arc::new(crate::anon::Ctx::ramo(1))).expect("walk");
        let c = r.duplicaveis.expect("candidatos");
        let nomes: Vec<_> = c.lista.iter().map(|x| x.caminho.file_name().and_then(|n| n.to_str()).unwrap_or("")).collect();
        assert_eq!(nomes, ["a", "b", "c"], "d (clone de a) e h (hardlink de b) deduplicados");
        let d = achar(&c, &Controle::new(&NUNCA), &ORCAMENTO);
        assert_eq!(d.motivos, Vec::<&str>::new());
        assert_eq!(d.grupos.len(), 1, "{d:?}");
        assert_eq!(d.grupos[0].caminhos, [p.0.join("a"), p.0.join("b")]);
        assert!(d.desperdicio() >= 34 * (3 << 20), "{}", d.desperdicio());
    }

    #[test]
    fn orcamento_e_ctrl_c_deixam_parcial() {
        static NUNCA: AtomicBool = AtomicBool::new(false);
        static JA: AtomicBool = AtomicBool::new(true);
        let p = pasta("sdtdup-orc");
        let c = tres(&p.0);
        // Dá para as pontas (6 MiB), não para o conteúdo inteiro: nenhum grupo pela metade.
        let d = achar(&c, &Controle::new(&NUNCA), &Orcamento { bytes: 8 << 20, tempo: LIMITE_TEMPO });
        assert_eq!((d.grupos.len(), d.motivo().as_deref()), (0, Some(M_BYTES)));
        assert!(d.bytes_lidos <= 8 << 20);
        let d = achar(&c, &Controle::new(&NUNCA), &Orcamento { bytes: LIMITE_BYTES, tempo: Duration::ZERO });
        assert_eq!((d.grupos.len(), d.bytes_lidos, d.motivo().as_deref()), (0, 0, Some(M_TEMPO)));
        let d = achar(&c, &Controle::new(&JA), &ORCAMENTO);
        assert_eq!((d.bytes_lidos, d.motivo().as_deref()), (0, Some(M_INTERROMPIDO)));
        // Motivo que veio da travessia segue junto.
        let d = achar(&Candidatos { motivos: vec![M_CHEIO], ..c.clone() }, &Controle::new(&NUNCA), &ORCAMENTO);
        assert_eq!((d.grupos.len(), d.motivo().as_deref()), (1, Some(M_CHEIO)));
    }

    #[test]
    fn arquivo_trocado_nao_e_lido() {
        static NUNCA: AtomicBool = AtomicBool::new(false);
        let p = pasta("sdtdup-troca");
        let mut c = tres(&p.0);
        // b mudou de tamanho depois da travessia; c virou symlink para a.
        std::fs::write(p.0.join("b"), &conteudo(None)[1..]).expect("b");
        std::fs::rename(p.0.join("c"), p.0.join("c0")).expect("mv");
        std::os::unix::fs::symlink(p.0.join("a"), p.0.join("c")).expect("ln");
        c.lista[2] = Cand { fileid: c.lista[0].fileid, ..c.lista[2].clone() };
        let d = achar(&c, &Controle::new(&NUNCA), &ORCAMENTO);
        assert_eq!((d.grupos.len(), d.motivo().as_deref()), (0, Some(M_MUDOU)));
    }
}

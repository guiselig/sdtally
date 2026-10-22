//! Travessia paralela com getattrlistbulk(2), sem root. Lógica do
//! research/experimentos/probe/src/bin/walk_par.rs, hoje só no histórico do git (técnicas inspiradas no dua-core, MIT):
//! mesmo volume (DEVID), não desce em pasta dataless nem em raiz de enxerto, repete em EINTR,
//! deduplica hardlink (fileid) e clone completo (cloneid). Baldes vêm de anon::Ctx::bucket.

pub mod vivo;

use crate::anon::Ctx;
use crate::parse::{self, EF_MAY_SHARE_BLOCKS, EF_NO_XATTRS, EF_SHARES_ALL_BLOCKS, Entrada, Item, SF_DATALESS, UF_COMPRESSED, VDIR, VREG};
use crate::sys;
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsStr;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

/// Xattr maior que isto tem fluxo próprio, fora do ALLOCSIZE (síntese §2.1).
pub const XATTR_MAX_EMBEDDED: i64 = 3804;
/// Chaves de `errors` que não são errno: panic numa pasta, entrada fora do layout, atributo
/// ausente (ALLOCSIZE, PRIVATESIZE, LINKCOUNT, FILEID de hardlink, ou TOTALSIZE de dataless).
pub const ERRO_INTERNO: i32 = -1;
pub const ERRO_PARSE: i32 = -2;
pub const ERRO_ATRIBUTO_AUSENTE: i32 = -3;
/// getattrlistbulk falhou depois de lotes já lidos: a pasta ficou pela metade (fora do dir-stats,
/// que contaria de novo o que já foi somado).
pub const ERRO_LISTAGEM: i32 = -4;

/// Erros que deixam algo sem ler: tudo menos EPERM/EACCES, que vão para o dir-stats
/// (`Result::denied`). Com algum, a árvore é um piso.
pub fn nao_lidos(errors: &HashMap<i32, u64>) -> u64 {
    errors.iter().filter(|(k, _)| **k != libc::EPERM && **k != libc::EACCES).map(|(_, v)| v).sum()
}

pub static MEASURE_XATTR: AtomicBool = AtomicBool::new(false);

/// Cancelamento (checado a cada pasta) e contador de arquivos para o progresso.
pub struct Controle {
    /// `static` porque o handler de SIGINT (sys::INTERROMPIDO) só alcança estáticos.
    pub cancelar: &'static AtomicBool,
    pub arquivos: AtomicU64,
}

impl Controle {
    pub fn new(cancelar: &'static AtomicBool) -> Controle {
        Controle { cancelar, arquivos: AtomicU64::new(0) }
    }

    pub fn cancelado(&self) -> bool {
        self.cancelar.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Sum {
    pub files: u64,
    pub alloc: i64,
    pub private: i64,
    pub dataless_n: u64,
    pub dataless_logical: i64,
    pub dataless_dirs: u64,
    pub xattr_stream: i64,
    pub xattr_all: i64,
    /// Cópias de clone completo que moram neste balde e que `alloc` não conta (só uma cópia de cada
    /// grupo entra na soma). O `STMSize` do Ajustes as conta de novo: `alloc + dup_clone` é o número
    /// dele (Frente Q §4).
    pub dup_clone: i64,
}

impl Sum {
    pub fn add(&mut self, o: &Sum) {
        self.files += o.files;
        self.alloc += o.alloc;
        self.private += o.private;
        self.dataless_n += o.dataless_n;
        self.dataless_logical += o.dataless_logical;
        self.dataless_dirs += o.dataless_dirs;
        self.xattr_stream += o.xattr_stream;
        self.xattr_all += o.xattr_all;
        self.dup_clone += o.dup_clone;
    }

    pub fn sub(&mut self, o: &Sum) {
        self.files -= o.files;
        self.alloc -= o.alloc;
        self.private -= o.private;
        self.dataless_n -= o.dataless_n;
        self.dataless_logical -= o.dataless_logical;
        self.dataless_dirs -= o.dataless_dirs;
        self.xattr_stream -= o.xattr_stream;
        self.xattr_all -= o.xattr_all;
        self.dup_clone -= o.dup_clone;
    }
}

/// Trava que sobrevive a panic de outra thread: o panic de uma pasta já vira erro contado.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Nomes de balde internados: cada balde é um `u32` (índice em `lista`), não um `Arc<str>` por
/// item. Os ids saem na ordem de chegada; a ordem alfabética (desempate) vem do texto.
#[derive(Default)]
struct Nomes {
    ids: HashMap<Arc<str>, u32>,
    lista: Vec<Arc<str>>,
}

impl Nomes {
    fn id(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.ids.get(s) {
            return i;
        }
        let a: Arc<str> = Arc::from(s);
        let i = self.lista.len() as u32;
        self.lista.push(a.clone());
        self.ids.insert(a, i);
        i
    }

    /// `a` vem antes de `b` em ordem alfabética?
    fn antes(&self, a: u32, b: u32) -> bool {
        self.lista[a as usize] < self.lista[b as usize]
    }
}

/// Os campos de `Sum` que um repetido (hardlink ou clone) leva para a conta: `files` é sempre 1 e
/// `dataless_dirs` e `dup_clone` são 0 em arquivo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Peso {
    alloc: i64,
    private: i64,
    dataless_logical: i64,
    xattr_stream: i64,
    xattr_all: i64,
    dataless_n: u8,
}

impl Peso {
    fn de(s: &Sum) -> Peso {
        Peso { alloc: s.alloc, private: s.private, dataless_logical: s.dataless_logical, xattr_stream: s.xattr_stream, xattr_all: s.xattr_all, dataless_n: s.dataless_n as u8 }
    }

    fn soma(&self) -> Sum {
        Sum {
            files: 1,
            alloc: self.alloc,
            private: self.private,
            dataless_n: self.dataless_n as u64,
            dataless_logical: self.dataless_logical,
            xattr_stream: self.xattr_stream,
            xattr_all: self.xattr_all,
            ..Default::default()
        }
    }
}

/// Sem candidato de sobreposição.
const SEM_CAND: u32 = u32::MAX;

/// Um hardlink (FILEID) ou grupo de clone completo (CLONEID): contado uma vez, no balde que vem
/// antes em ordem alfabética (resultado igual entre rodadas).
struct Grupo {
    bucket: u32,
    /// Candidato do grupo em `Mapas` (só de clone, e só se o primeiro arquivo visto tinha faixas).
    cand: u32,
    /// Arquivos vistos.
    n: u64,
    clone_of: u64,
    /// O menor FILEID visto (u64::MAX = nenhum) e o peso desse arquivo: o representante é o mesmo
    /// em toda rodada, não o que chegou primeiro.
    id: u64,
    peso: Peso,
    /// (balde, ocorrências), por balde: só de clone, para pôr as cópias no balde delas.
    onde: Vec<(u32, u32)>,
}

/// Um arquivo visto: FILEID, balde, peso e CLONEID (0 = não é clone completo).
struct Ocorr {
    id: u64,
    bucket: u32,
    peso: Peso,
    clone_of: u64,
}

/// Tabela compartilhada pelas threads, em fatias com trava: cada repetido existe uma vez só, sem
/// uma tabela por thread nem a cópia da junção no fim.
struct Tabela(Vec<Mutex<HashMap<u64, Grupo>>>);

impl Tabela {
    const FATIAS: usize = 64;

    fn new() -> Tabela {
        Tabela((0..Self::FATIAS).map(|_| Mutex::new(HashMap::new())).collect())
    }

    /// Mais uma ocorrência de `k`. `cand` roda só se o grupo é novo.
    fn put(&self, nomes: &Mutex<Nomes>, k: u64, o: Ocorr, com_onde: bool, cand: impl FnOnce() -> u32) {
        let Ocorr { id, bucket, peso, clone_of } = o;
        let mut fatia = lock(&self.0[(k.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 58) as usize]);
        match fatia.entry(k) {
            Entry::Occupied(mut o) => {
                let g = o.get_mut();
                if bucket != g.bucket && lock(nomes).antes(bucket, g.bucket) {
                    g.bucket = bucket;
                }
                g.n += 1;
                if id < g.id {
                    (g.id, g.peso) = (id, peso);
                }
                if com_onde {
                    match g.onde.binary_search_by_key(&bucket, |x| x.0) {
                        Ok(i) => g.onde[i].1 += 1,
                        Err(i) => g.onde.insert(i, (bucket, 1)),
                    }
                }
            }
            Entry::Vacant(v) => {
                v.insert(Grupo { bucket, cand: cand(), n: 1, clone_of, id, peso, onde: if com_onde { vec![(bucket, 1)] } else { Vec::new() } });
            }
        }
    }
}

/// Arquivo contado inteiro (ALLOCSIZE) que pode dividir blocos com outro: clone completo (o
/// representante do grupo de CLONEID) ou parcial (EF_MAY_SHARE_BLOCKS sem SHARES_ALL). O ALLOCSIZE
/// de um clone parcial inclui os blocos que ele divide com o original; somados, contam duas vezes.
/// Depois da travessia as faixas físicas de todos os `Cand` dão a união, e o que um `Cand` tem
/// dentro de faixa de outro sai do alocado do seu balde.
struct Cand {
    /// FILEID: desempata a ordem de forma igual em toda rodada.
    id: u64,
    alloc: i64,
    bucket: u32,
    /// Fim das faixas dele em `Mapas.faixas` (o começo é o fim do candidato anterior).
    fim: u32,
}

/// Faixas físicas [início, fim) de todos os candidatos num Vec só, `u64` puro, sem Vec por arquivo.
#[derive(Default)]
struct Mapas {
    cands: Vec<Cand>,
    faixas: Vec<(u64, u64)>,
}

impl Mapas {
    /// Índice do candidato; None se as faixas passam de `u32` (o arquivo fica sem mapa).
    fn push(&mut self, id: u64, alloc: i64, bucket: u32, faixas: &[(u64, u64)]) -> Option<u32> {
        let fim = u32::try_from(self.faixas.len() + faixas.len()).ok()?;
        let i = u32::try_from(self.cands.len()).ok().filter(|&i| i != SEM_CAND)?;
        self.faixas.extend_from_slice(faixas);
        self.cands.push(Cand { id, alloc, bucket, fim });
        Some(i)
    }

    fn faixas_de(&self, i: usize) -> &[(u64, u64)] {
        let ini = if i == 0 { 0 } else { self.cands[i - 1].fim as usize };
        &self.faixas[ini..self.cands[i].fim as usize]
    }
}

/// Maiores arquivos da travessia: a partir deste tamanho (alocado), no máximo `MAIORES_N`. O piso
/// deixa o custo perto de zero (o caminho só é montado para arquivo grande).
const MAIORES_MIN: i64 = 100_000_000;
const MAIORES_N: usize = 30;

/// Do maior para o menor (empate: caminho), no máximo `MAIORES_N`. Hardlink aparece uma vez por
/// nome: fica só o primeiro nome de cada FILEID.
fn podar_maiores(v: &mut Vec<(i64, i64, PathBuf, Option<u64>)>) {
    v.sort_unstable_by(|a, b| b.0.cmp(&a.0).then_with(|| a.2.cmp(&b.2)));
    let mut vistos = HashSet::new();
    v.retain(|x| x.3.is_none_or(|id| vistos.insert(id)));
    v.truncate(MAIORES_N);
}

#[derive(Default)]
struct Local {
    buckets: HashMap<u32, Sum>,
    /// (alocado, privado, caminho, FILEID se hardlink) dos arquivos de `MAIORES_MIN` ou mais.
    maiores: Vec<(i64, i64, PathBuf, Option<u64>)>,
    sem_mapa: u64,
    dirs: u64,
    errors: HashMap<i32, u64>,
    denied: Vec<(PathBuf, i32)>,
    other_dev: u64,
    graft_roots: u64,
    xattr_erros: u64,
    /// Só no modo vivo (um `Local` por pasta): a pasta não foi lida (abrir ou fstat falhou, fora
    /// do dir-stats) ou a listagem parou no meio (panic, erro depois de lotes lidos).
    nao_lida: bool,
    cortada: bool,
}

#[derive(Default)]
pub struct Result {
    pub buckets: HashMap<String, Sum>,
    pub total: Sum,
    pub dirs: u64,
    pub errors: HashMap<i32, u64>,
    /// Pastas que deram EPERM/EACCES (caminho real; nunca vai para o JSON).
    pub denied: Vec<(PathBuf, i32)>,
    pub other_dev: u64,
    pub graft_roots: u64,
    pub link_dup_files: u64,
    pub link_dup_bytes: i64,
    pub clone_dup_files: u64,
    pub clone_dup_bytes: i64,
    /// Arquivos com blocos divididos com outro (clone parcial ou completo) e os bytes que o
    /// ALLOCSIZE deles contava de novo e que saíram do alocado.
    pub shared_dup_files: u64,
    pub shared_dup_bytes: i64,
    /// Candidatos (EF_MAY_SHARE_BLOCKS) cujas faixas físicas não deu para ler: ficam com o
    /// ALLOCSIZE inteiro, sem desconto de bloco dividido.
    pub sem_mapa: u64,
    /// Objetos cujos xattrs não deu para ler (fora de `errors`, que é só de entradas): o termo
    /// sai parcial.
    pub xattr_erros: u64,
    /// Cancelada no meio: os totais são só do que foi lido (um piso).
    pub interrompida: bool,
    /// (caminho, alocado, privado) dos maiores arquivos; só a travessia única mede (o modo vivo
    /// guarda pasta, não arquivo): None nele.
    pub maiores: Option<Vec<(PathBuf, i64, i64)>>,
}

/// Soma dos valores de xattr sem ler conteúdo. Falha ao ler não some: vira erro contado.
fn xattr_sizes(path: &Path, s: &mut Sum, l: &mut Local) {
    match sys::apfs::xattr_tamanhos(path) {
        Ok(v) => {
            for sz in v {
                s.xattr_all += sz;
                if sz > XATTR_MAX_EMBEDDED {
                    s.xattr_stream += sz;
                }
            }
        }
        Err(_) => l.xattr_erros += 1,
    }
}

/// (alocado, privado) do arquivo, ou None se faltar atributo para contar certo: ALLOCSIZE,
/// PRIVATESIZE, LINKCOUNT, ou o FILEID de um hardlink (sem ele não há como deduplicar).
fn tamanhos(e: &Entrada) -> Option<(i64, i64)> {
    if e.linkcount? > 1 && e.fileid.is_none() {
        return None;
    }
    Some((e.allocsize? as i64, e.privatesize? as i64))
}

struct Job {
    path: PathBuf,
    bucket: u32,
    depth: usize,
    /// Pasta da árvore viva que este job preenche (a travessia única não usa).
    dono: u32,
}

struct Walker<'a> {
    root: &'a str,
    /// Ponto de montagem do volume da raiz (ver `e_montagem`).
    mnt: &'a str,
    root_dev: u32,
    /// A raiz aberta: as pastas abaixo dela abrem a partir dela (`abrir_pasta_sob`).
    raiz_fd: Option<&'a OwnedFd>,
    ctx: &'a Ctx,
    montagens: Option<&'a [String]>,
    ctl: &'a Controle,
    nomes: &'a Mutex<Nomes>,
}

/// O que a leitura de uma pasta achou, entrada a entrada. Quem lê decide onde somar: a travessia
/// única junta tudo nas tabelas; o modo vivo guarda por pasta para poder tirar depois.
enum Achado {
    /// Soma direta num balde (arquivo comum, xattr de pasta, pasta dataless).
    Soma(u32, Sum),
    /// Arquivo comum que pode dividir blocos: entra na soma e vira candidato com estas faixas.
    Cand(u32, Sum, u64, Vec<(u64, u64)>),
    /// Hardlink (FILEID) ainda não deduplicado.
    Link(u64, Ocorr),
    /// Cópia de clone completo (CLONEID), com as faixas se o arquivo as tem.
    Clone(u64, Ocorr, Option<(u64, Vec<(u64, u64)>)>),
    /// Pasta a descer.
    Pasta(Job),
}

/// `s` é ponto de montagem? Visto de dentro do volume montado em `mnt` (/System/Volumes/Data), as
/// montagens aparecem no espaço de nomes da raiz do sistema (/Volumes/X, /private/var/...): compara
/// com e sem o prefixo de `mnt`. Não é o prefixo da raiz da travessia: no ramo (`sdtally ~`),
/// ~/dev viraria "/dev", a montagem do devfs.
fn e_montagem(ms: &[String], mnt: &str, s: &str) -> bool {
    let sem_mnt = s.strip_prefix(mnt).filter(|_| mnt != "/").map(|r| if r.is_empty() { "/" } else { r });
    ms.iter().any(|m| m == s || sem_mnt == Some(m.as_str()))
}

impl Walker<'_> {
    fn e_montagem(&self, p: &Path) -> bool {
        self.montagens.is_some_and(|ms| e_montagem(ms, self.mnt, &p.to_string_lossy()))
    }

    fn bucket_of(&self, p: &std::path::Path) -> u32 {
        let s = p.to_string_lossy();
        let b = self.ctx.bucket(s.strip_prefix(self.root).unwrap_or(&s));
        lock(self.nomes).id(&b)
    }

    /// Pasta negada é deste volume? Pelo lstat quando ele responde; quando o próprio lstat é
    /// negado (sandbox, datavault), pela tabela de montagens: a entrada veio de uma listagem
    /// deste volume, então só não é dele se for ponto de montagem.
    fn mesmo_volume(&self, p: &Path) -> bool {
        match std::fs::symlink_metadata(p) {
            Ok(m) => m.dev() as u32 == self.root_dev,
            Err(_) => self.montagens.is_some_and(|ms| !e_montagem(ms, self.mnt, &p.to_string_lossy())),
        }
    }

    fn scan_dir(&self, job: &Job, buf: &mut [u8], l: &mut Local, achou: &mut impl FnMut(Achado)) {
        // Nada lido da pasta: EPERM/EACCES deste volume vai para o dir-stats; o resto fica não lido.
        let negada = |l: &mut Local, e: i32| {
            *l.errors.entry(e).or_default() += 1;
            // Só pasta deste volume vai para o dir-stats: um ponto de montagem negado levaria o
            // fsctl a outro volume, que pode ser o destino do Time Machine (síntese §4.3 e §4.4).
            if (e == libc::EPERM || e == libc::EACCES) && self.mesmo_volume(&job.path) {
                l.denied.push((job.path.clone(), e));
            } else {
                l.nao_lida = true;
            }
        };
        // Montagem que surgiu depois de a pasta mãe ser listada (o modo vivo relê a tabela a cada
        // leitura): pula antes de abrir. A raiz é a única montagem que se abre.
        if job.depth > 0 && self.e_montagem(&job.path) {
            l.other_dev += 1;
            return;
        }
        // Abaixo da raiz, a partir do fd dela: sem symlink em nenhum componente e sem o limite de
        // PATH_MAX do caminho inteiro.
        let rel = job.path.strip_prefix(self.root).ok().filter(|r| !r.as_os_str().is_empty());
        let aberta = match (self.raiz_fd, rel) {
            (Some(raiz), Some(rel)) => sys::apfs::abrir_pasta_sob(raiz, rel),
            _ => sys::apfs::abrir_pasta(&job.path),
        };
        let fd = match aberta {
            Ok(fd) => fd,
            Err(e) => return negada(l, e),
        };
        // Num ponto de montagem o DEVID da entrada é o da pasta coberta (volume pai) e o open
        // atravessa: a trava de volume vale para a pasta já aberta, antes de listar (§4.3).
        match sys::apfs::dev_da_pasta(&fd) {
            Ok(d) if d == self.root_dev => {}
            Ok(_) => {
                l.other_dev += 1;
                return;
            }
            Err(e) => {
                *l.errors.entry(e).or_default() += 1;
                l.nao_lida = true;
                return;
            }
        }
        l.dirs += 1;
        // Raiz de enxerto: cloneid != fileid na pasta aberta (Frente L). Não testar a raiz.
        if job.depth > 0 && sys::apfs::ids_da_pasta(&fd).is_some_and(|(id, cid)| cid != 0 && cid != id) {
            l.graft_roots += 1;
            return;
        }
        let xattr = MEASURE_XATTR.load(Ordering::Relaxed);
        let bucket = job.bucket;
        let mut arquivos = 0u64;
        let mut lotes = 0u32;
        loop {
            let n = match sys::apfs::ler_lote(&fd, buf) {
                Ok(0) => break,
                Ok(n) => n,
                // Nada lido ainda: como se não abrisse.
                Err(e) if lotes == 0 => {
                    negada(l, e);
                    break;
                }
                Err(_) => {
                    *l.errors.entry(ERRO_LISTAGEM).or_default() += 1;
                    l.cortada = true;
                    break;
                }
            };
            lotes += 1;
            for it in parse::entradas(buf, n) {
                let e = match it {
                    Ok(Item::Entrada(e)) => e,
                    // Erro por entrada é dado: conta e segue (síntese §2.1).
                    Ok(Item::Erro(errno)) => {
                        *l.errors.entry(errno).or_default() += 1;
                        continue;
                    }
                    Err(_) => {
                        *l.errors.entry(ERRO_PARSE).or_default() += 1;
                        continue;
                    }
                };
                let flags = e.flags.unwrap_or(0);
                let path = || job.path.join(OsStr::from_bytes(e.nome));
                match e.objtype {
                    Some(VDIR) => {
                        // Sem DEVID não dá para garantir o mesmo volume: não desce.
                        if e.devid != Some(self.root_dev) {
                            l.other_dev += 1;
                            continue;
                        }
                        let path = path();
                        // Ponto de montagem (disco externo, SMB, destino do Time Machine) tem o DEVID
                        // da pasta coberta: o open atravessaria e, com o servidor fora do ar, travaria
                        // ou abriria o aviso de conexão do macOS. Pula antes de abrir; a trava do
                        // dev_da_pasta fica para montagem que surgir depois da tabela.
                        if self.e_montagem(&path) {
                            l.other_dev += 1;
                            continue;
                        }
                        let b = self.bucket_of(&path);
                        if flags & SF_DATALESS != 0 {
                            // Listar pediria ao File Provider para materializar.
                            achou(Achado::Soma(b, Sum { dataless_dirs: 1, ..Default::default() }));
                            continue;
                        }
                        if xattr && e.ext_flags.unwrap_or(0) & EF_NO_XATTRS == 0 {
                            let mut s = Sum::default();
                            xattr_sizes(&path, &mut s, l);
                            achou(Achado::Soma(bucket, s));
                        }
                        achou(Achado::Pasta(Job { path, bucket: b, depth: job.depth + 1, dono: 0 }));
                    }
                    Some(VREG) => {
                        arquivos += 1;
                        // Atributo ausente não vira 0 (nem privado = alocado, nem link = 1): conta
                        // como erro e o arquivo fica no não atribuído.
                        let Some((alloc, private)) = tamanhos(&e) else {
                            *l.errors.entry(ERRO_ATRIBUTO_AUSENTE).or_default() += 1;
                            continue;
                        };
                        if alloc >= MAIORES_MIN {
                            l.maiores.push((alloc, private, path(), e.fileid.filter(|_| e.linkcount > Some(1))));
                            if l.maiores.len() >= 2 * MAIORES_N {
                                podar_maiores(&mut l.maiores);
                            }
                        }
                        let ext = e.ext_flags.unwrap_or(0);
                        let mut s = Sum { files: 1, alloc, private, ..Default::default() };
                        if xattr && ext & EF_NO_XATTRS == 0 {
                            xattr_sizes(&path(), &mut s, l);
                        }
                        if flags & SF_DATALESS != 0 {
                            s.dataless_n = 1;
                            match e.totalsize {
                                Some(t) => s.dataless_logical = t as i64,
                                None => *l.errors.entry(ERRO_ATRIBUTO_AUSENTE).or_default() += 1,
                            }
                        }
                        let clone_of = match e.cloneid {
                            Some(c) if ext & EF_SHARES_ALL_BLOCKS != 0 => c,
                            _ => 0,
                        };
                        // Faixas físicas só de quem pode dividir blocos. Dataless nunca é aberto
                        // (materializaria); comprimido não tem dados no fork de dados; hardlink
                        // fica de fora (sem FILEID único por caminho não há desempate certo).
                        let mapa = match e.fileid {
                            Some(id) if ext & EF_MAY_SHARE_BLOCKS != 0 && alloc > 0 && flags & (SF_DATALESS | UF_COMPRESSED) == 0 && e.linkcount <= Some(1) => {
                                match sys::apfs::faixas_fisicas(&path()) {
                                    Ok(f) if !f.is_empty() => Some((id, f)),
                                    Ok(_) => None,
                                    Err(_) => {
                                        l.sem_mapa += 1;
                                        None
                                    }
                                }
                            }
                            _ => None,
                        };
                        match e.fileid {
                            Some(id) if e.linkcount > Some(1) => achou(Achado::Link(id, Ocorr { id, bucket, peso: Peso::de(&s), clone_of })),
                            _ if clone_of != 0 => achou(Achado::Clone(clone_of, Ocorr { id: e.fileid.unwrap_or(u64::MAX), bucket, peso: Peso::de(&s), clone_of }, mapa)),
                            _ => achou(match mapa {
                                Some((id, f)) => Achado::Cand(bucket, s, id, f),
                                None => Achado::Soma(bucket, s),
                            }),
                        }
                    }
                    _ => {}
                }
            }
        }
        self.ctl.arquivos.fetch_add(arquivos, Ordering::Relaxed);
    }
}

/// Por candidato, os bytes das suas faixas que um candidato de FILEID menor já cobre: o arquivo mais
/// antigo (o original, em geral) fica com os blocos que divide, o clone só com o que é seu. A ordem
/// por FILEID (único no volume) é igual em toda rodada. Σ (mapeado − sobra) = tamanho da união.
fn sobreposicao(m: &Mapas) -> Vec<i64> {
    let itens: Vec<(u64, &[(u64, u64)])> = (0..m.cands.len()).map(|i| (m.cands[i].id, m.faixas_de(i))).collect();
    sobra_de(&itens)
}

/// `sobreposicao` sobre (FILEID, faixas) soltos, sem copiar as faixas para um `Mapas`.
fn sobra_de(itens: &[(u64, &[(u64, u64)])]) -> Vec<i64> {
    let mut ordem: Vec<usize> = (0..itens.len()).collect();
    ordem.sort_unstable_by_key(|&i| itens[i].0);
    // Faixas já tomadas: início -> fim, disjuntas.
    let mut tomado: BTreeMap<u64, u64> = BTreeMap::new();
    let mut sobra = vec![0i64; itens.len()];
    for i in ordem {
        for &(a, b) in itens[i].1 {
            // Começa na última faixa que abre antes de `a` (pode cobrir `a`).
            let de = tomado.range(..=a).next_back().map_or(a, |(&s, _)| s);
            let (mut ini, mut fim, mut dentro) = (a, b, 0u64);
            let cruzam: Vec<(u64, u64)> = tomado.range(de..=b).map(|(&s, &e)| (s, e)).filter(|&(s, e)| e >= a && s <= b).collect();
            for (s, e) in cruzam {
                dentro += e.min(b).saturating_sub(s.max(a));
                (ini, fim) = (ini.min(s), fim.max(e));
                tomado.remove(&s);
            }
            tomado.insert(ini, fim);
            sobra[i] += dentro as i64;
        }
    }
    sobra
}

/// Destino da travessia única: soma no balde, repetido na tabela, candidato nos mapas.
fn juntar(a: Achado, somas: &mut HashMap<u32, Sum>, push: &mut Vec<Job>, nomes: &Mutex<Nomes>, links: &Tabela, clones: &Tabela, mapas: &Mutex<Mapas>) {
    match a {
        Achado::Soma(b, s) => somas.entry(b).or_default().add(&s),
        Achado::Cand(b, s, id, f) => {
            somas.entry(b).or_default().add(&s);
            lock(mapas).push(id, s.alloc, b, &f);
        }
        Achado::Link(k, o) => links.put(nomes, k, o, false, || SEM_CAND),
        Achado::Clone(k, o, mapa) => {
            let (b, alloc) = (o.bucket, o.peso.alloc);
            clones.put(nomes, k, o, true, || mapa.and_then(|(id, f)| lock(mapas).push(id, alloc, b, &f)).unwrap_or(SEM_CAND));
        }
        Achado::Pasta(j) => push.push(j),
    }
}

struct Queue {
    stack: Mutex<(Vec<Job>, usize)>,
    cv: Condvar,
}

pub fn walk(root: &str, threads: usize, ctx: Arc<Ctx>) -> std::result::Result<Result, String> {
    walk_ctl(root, threads, ctx, Arc::new(Controle::new(&sys::INTERROMPIDO)))
}

/// Como `walk`, com cancelamento e progresso. Cancelada, as pastas que faltam não são abertas.
pub fn walk_ctl(root: &str, threads: usize, ctx: Arc<Ctx>, ctl: Arc<Controle>) -> std::result::Result<Result, String> {
    let root_dev = std::fs::symlink_metadata(root).map_err(|e| format!("raiz: {e}"))?.dev() as u32;
    let mnt: Arc<str> = Arc::from(sys::montado_em(Path::new(root)).unwrap_or_else(|_| root.to_string()));
    let root_s: Arc<str> = Arc::from(root);
    // Sem a tabela de montagens não dá para saber se uma pasta negada é ponto de montagem:
    // aí nenhuma negada sem lstat vai para o dir-stats (vira não medida).
    let montagens: Arc<Option<Vec<String>>> = Arc::new(sys::montagens().ok());
    let raiz_fd: Arc<Option<OwnedFd>> = Arc::new(sys::apfs::abrir_pasta(Path::new(root)).ok());
    let nomes = Arc::new(Mutex::new(Nomes::default()));
    let (links, clones, mapas) = (Arc::new(Tabela::new()), Arc::new(Tabela::new()), Arc::new(Mutex::new(Mapas::default())));
    let q = Arc::new(Queue {
        stack: Mutex::new((vec![Job { path: PathBuf::from(root), bucket: lock(&nomes).id("/"), depth: 0, dono: 0 }], 1)),
        cv: Condvar::new(),
    });
    let handles: Vec<_> = (0..threads.max(1))
        .map(|_| {
            let (q, ctx, root_s, mnt, montagens, ctl, raiz_fd) = (q.clone(), ctx.clone(), root_s.clone(), mnt.clone(), montagens.clone(), ctl.clone(), raiz_fd.clone());
            let (nomes, links, clones, mapas) = (nomes.clone(), links.clone(), clones.clone(), mapas.clone());
            std::thread::spawn(move || {
                sys::qos_utility();
                let w = Walker { root: &root_s, mnt: &mnt, root_dev, raiz_fd: raiz_fd.as_ref().as_ref(), ctx: &ctx, montagens: montagens.as_deref(), ctl: &ctl, nomes: &nomes };
                let mut l = Local::default();
                let mut somas: HashMap<u32, Sum> = HashMap::new();
                let mut buf = vec![0u8; 256 * 1024];
                let mut push = Vec::new();
                loop {
                    let job = {
                        let Ok(mut g) = q.stack.lock() else { return Local { buckets: somas, ..l } };
                        loop {
                            if let Some(j) = g.0.pop() {
                                break Some(j);
                            }
                            if g.1 == 0 {
                                break None;
                            }
                            g = match q.cv.wait(g) {
                                Ok(g) => g,
                                Err(_) => return Local { buckets: somas, ..l },
                            };
                        }
                    };
                    let Some(job) = job else {
                        q.cv.notify_all();
                        return Local { buckets: somas, ..l };
                    };
                    // Cancelada: a pasta sai da fila sem ser aberta (a fila esvazia e as threads saem).
                    // Uma pasta que der panic não derruba a travessia: conta como erro e segue.
                    let r = match ctl.cancelado() {
                        true => Ok(()),
                        false => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w.scan_dir(&job, &mut buf, &mut l, &mut |a| juntar(a, &mut somas, &mut push, &nomes, &links, &clones, &mapas)))),
                    };
                    if r.is_err() {
                        *l.errors.entry(ERRO_INTERNO).or_default() += 1;
                        push.clear();
                    }
                    let Ok(mut g) = q.stack.lock() else { return Local { buckets: somas, ..l } };
                    g.1 += push.len();
                    g.1 -= 1;
                    g.0.append(&mut push);
                    drop(g);
                    q.cv.notify_all();
                }
            })
        })
        .collect();

    let mut r = Result::default();
    let mut buckets: HashMap<u32, Sum> = HashMap::new();
    let mut maiores = Vec::new();
    for h in handles {
        let Ok(mut l) = h.join() else { return Err("thread da travessia morreu".into()) };
        maiores.append(&mut l.maiores);
        for (k, v) in l.buckets {
            buckets.entry(k).or_default().add(&v);
        }
        r.dirs += l.dirs;
        r.other_dev += l.other_dev;
        r.graft_roots += l.graft_roots;
        r.xattr_erros += l.xattr_erros;
        r.sem_mapa += l.sem_mapa;
        for (k, v) in l.errors {
            *r.errors.entry(k).or_default() += v;
        }
        r.denied.extend(l.denied);
    }
    podar_maiores(&mut maiores);
    r.maiores = Some(maiores.into_iter().map(|(a, p, c, _)| (c, a, p)).collect());
    // Repetidos contam uma vez; hardlink que também é clone completo entra nos clones. Fatia a
    // fatia, para a tabela ir embora aos poucos.
    for fatia in &links.0 {
        for (k, g) in std::mem::take(&mut *lock(fatia)) {
            r.link_dup_bytes += g.peso.alloc * (g.n as i64 - 1);
            r.link_dup_files += g.n - 1;
            if g.clone_of != 0 {
                clones.put(&nomes, g.clone_of, Ocorr { id: k, bucket: g.bucket, peso: g.peso, clone_of: g.clone_of }, true, || SEM_CAND);
            } else {
                buckets.entry(g.bucket).or_default().add(&g.peso.soma());
            }
        }
    }
    let mut mapas = std::mem::take(&mut *lock(&mapas));
    for fatia in &clones.0 {
        for (_, g) in std::mem::take(&mut *lock(fatia)) {
            buckets.entry(g.bucket).or_default().add(&g.peso.soma());
            r.clone_dup_bytes += g.peso.alloc * (g.n as i64 - 1);
            r.clone_dup_files += g.n - 1;
            // As cópias além da contada ficam no balde onde estão.
            for &(b, c) in &g.onde {
                let copias = if b == g.bucket { c - 1 } else { c };
                buckets.entry(b).or_default().dup_clone += g.peso.alloc * copias as i64;
            }
            if let Some(c) = mapas.cands.get_mut(g.cand as usize) {
                (c.bucket, c.id, c.alloc) = (g.bucket, g.id, g.peso.alloc);
            }
        }
    }
    // Blocos que dois arquivos dividem contam uma vez: o primeiro na ordem física fica com eles.
    for (c, sobra) in mapas.cands.iter().zip(sobreposicao(&mapas)) {
        let sobra = sobra.min(c.alloc);
        if sobra > 0 {
            if let Some(b) = buckets.get_mut(&c.bucket) {
                b.alloc -= sobra;
            }
            r.shared_dup_files += 1;
            r.shared_dup_bytes += sobra;
        }
    }
    let nomes = lock(&nomes);
    for (k, v) in buckets {
        r.total.add(&v);
        r.buckets.insert(nomes.lista[k as usize].to_string(), v);
    }
    r.denied.sort();
    r.denied.dedup_by(|a, b| a.0 == b.0);
    r.interrompida = ctl.cancelado();
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn montagem_casa_com_e_sem_a_raiz() {
        let ms = ["/".to_string(), "/System/Volumes/Data".into(), "/Volumes/NAS".into(), "/System/Volumes/Data/home".into()];
        let r = "/System/Volumes/Data";
        assert!(e_montagem(&ms, r, "/System/Volumes/Data/Volumes/NAS"), "SMB em /Volumes, visto pela raiz do Data");
        assert!(e_montagem(&ms, r, "/System/Volumes/Data/home"), "autofs com o caminho cheio");
        assert!(!e_montagem(&ms, r, "/System/Volumes/Data/Volumes"), "a pasta /Volumes é do Data");
        assert!(!e_montagem(&ms, r, "/System/Volumes/Data/Users"));
        // Ramo (`sdtally ~`): a pasta ~/dev não é o devfs.
        let ms = [ms.as_slice(), &["/dev".to_string()]].concat();
        assert!(!e_montagem(&ms, r, "/Users/x/dev"));
        assert!(e_montagem(&ms, r, "/Volumes/NAS"), "ramo em /Volumes");
    }

    /// (FILEID, faixas) → `Mapas`, na ordem dada.
    fn mapas(v: &[(u64, &[(u64, u64)])]) -> Mapas {
        let mut m = Mapas::default();
        for &(id, f) in v {
            m.push(id, 0, 0, f).expect("push");
        }
        m
    }

    #[test]
    fn faixa_dividida_conta_uma_vez() {
        // a:[0,8) b:[4,12) c:[20,24) d:[0,8): união = 16.
        let v: [(u64, &[(u64, u64)]); 4] = [(1, &[(0, 8)]), (2, &[(4, 12)]), (3, &[(20, 24)]), (4, &[(0, 8)])];
        let sobra = sobreposicao(&mapas(&v));
        let mapeado: Vec<i64> = v.iter().map(|(_, f)| f.iter().map(|&(a, b)| (b - a) as i64).sum()).collect();
        // Por id: a toma [0,8); b perde [4,8); c é só dele; d fica todo dentro de a.
        assert_eq!(sobra, [0, 4, 0, 8]);
        // A ordem de chegada não muda o resultado.
        let inv: Vec<_> = v.iter().rev().copied().collect();
        assert_eq!(sobreposicao(&mapas(&inv)), [8, 0, 4, 0]);
        // Faixa que liga duas tomadas, e faixa colada (fim == início) sem sobra.
        assert_eq!(sobreposicao(&mapas(&[(1, &[(0, 4), (8, 12)]), (2, &[(2, 10)]), (3, &[(12, 16)])])), [0, 4, 0]);
        assert_eq!(sobreposicao(&mapas(&[(1, &[(0, 4)]), (2, &[(0, 16)])])), [0, 4]);
        assert_eq!(mapeado.iter().zip(&sobra).map(|(m, s)| m - s).sum::<i64>(), 16);
        // Faixas de um arquivo só, em ordem qualquer: nada a descontar.
        assert_eq!(sobreposicao(&mapas(&[(1, &[(40, 48), (0, 8), (16, 24)])])), [0]);
    }

    #[test]
    fn mapas_guarda_faixas_num_vec_so() {
        let mut m = Mapas::default();
        assert_eq!(m.push(7, 100, 3, &[(0, 4), (8, 12)]), Some(0));
        assert_eq!(m.push(5, 200, 4, &[(20, 24)]), Some(1));
        assert_eq!(m.push(6, 300, 5, &[(30, 34), (40, 44), (50, 54)]), Some(2));
        assert_eq!((m.faixas_de(0), m.faixas_de(1), m.faixas_de(2)), (&[(0, 4), (8, 12)][..], &[(20, 24)][..], &[(30, 34), (40, 44), (50, 54)][..]));
        assert_eq!((m.cands[1].id, m.cands[1].alloc, m.cands[1].bucket), (5, 200, 4));
        assert_eq!(m.faixas.len(), 6);
    }

    #[test]
    fn nomes_internados() {
        let mut n = Nomes::default();
        let (a, b) = (n.id("~/Library"), n.id("/Applications"));
        assert_eq!((n.id("~/Library"), n.id("/Applications"), n.lista.len()), (a, b, 2), "mesmo nome, mesmo id");
        // A ordem alfabética vem do texto, não da ordem de chegada: "~" vem depois de "/".
        assert!(a < b && !n.antes(a, b) && n.antes(b, a));
    }

    #[test]
    fn grupo_fica_no_menor_balde_e_conta_as_copias_por_balde() {
        let nomes = Mutex::new(Nomes::default());
        let (zeta, alfa, meio) = {
            let mut n = lock(&nomes);
            (n.id("/zeta"), n.id("/alfa"), n.id("/meio"))
        };
        let peso = |alloc| Peso { alloc, private: 1, dataless_logical: 0, xattr_stream: 0, xattr_all: 0, dataless_n: 0 };
        let t = Tabela::new();
        let mut novos = 0;
        // Chegada: zeta, zeta, alfa, meio, alfa. O peso é o do menor FILEID; o balde, o menor.
        for (b, id, alloc) in [(zeta, 30, 10), (zeta, 31, 11), (alfa, 20, 12), (meio, 25, 13), (alfa, 29, 14)] {
            t.put(&nomes, 42, Ocorr { id, bucket: b, peso: peso(alloc), clone_of: 42 }, true, || {
                novos += 1;
                SEM_CAND
            });
        }
        assert_eq!(novos, 1, "cand só no grupo novo");
        let g = t.0.iter().find_map(|f| lock(f).remove(&42)).expect("grupo");
        assert_eq!((g.bucket, g.n, g.id, g.peso.alloc), (alfa, 5, 20, 12));
        // Por id de balde, ordenado: zeta=0, alfa=1, meio=2.
        assert_eq!(g.onde, [(zeta, 2), (alfa, 2), (meio, 1)]);
        // Sem `onde` (hardlink): só conta e balde.
        for b in [zeta, meio] {
            t.put(&nomes, 7, Ocorr { id: 7, bucket: b, peso: peso(1), clone_of: 0 }, false, || SEM_CAND);
        }
        let g = t.0.iter().find_map(|f| lock(f).remove(&7)).expect("grupo");
        assert_eq!((g.bucket, g.n, g.onde.len()), (meio, 2, 0));
    }

    #[test]
    fn peso_guarda_o_que_a_conta_usa() {
        let s = Sum { files: 1, alloc: 8192, private: 4096, dataless_n: 1, dataless_logical: 77, xattr_stream: 5000, xattr_all: 5100, ..Default::default() };
        assert_eq!(Peso::de(&s).soma(), s);
    }

    /// Pasta temporária própria do teste, apagada no Drop.
    struct Pasta(PathBuf);
    impl Drop for Pasta {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn aleatorio(n: usize, mut x: u64) -> Vec<u8> {
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x as u8
            })
            .collect()
    }

    /// Clone, clone parcial e hardlink em três baldes: o repetido vai para o menor balde em ordem
    /// alfabética, as cópias ficam no balde delas e o resultado é o mesmo em toda rodada.
    #[test]
    fn repetidos_no_menor_balde_e_resultado_estavel() {
        use std::os::unix::fs::FileExt;
        const MIB: usize = 1 << 20;
        let pasta = Pasta(std::env::temp_dir().join(format!("sdtwalk-{}", std::process::id())));
        let raiz = &pasta.0;
        let _ = std::fs::remove_dir_all(raiz);
        for d in ["zeta", "alfa", "meio"] {
            std::fs::create_dir_all(raiz.join(d)).expect("mkdir");
        }
        let (a, b, c, p) = (raiz.join("zeta/a.bin"), raiz.join("alfa/b.bin"), raiz.join("meio/c.bin"), raiz.join("zeta/p.bin"));
        std::fs::write(&a, aleatorio(MIB, 1)).expect("a");
        for d in [&b, &c, &p] {
            if !std::process::Command::new("/bin/cp").arg("-c").arg(&a).arg(d).status().is_ok_and(|s| s.success()) {
                return; // volume sem clone (não é APFS): nada a medir
            }
        }
        // Clone parcial: 256 KiB reescritos; o resto continua dividido com a.bin.
        std::fs::OpenOptions::new().write(true).open(&p).expect("abre p").write_all_at(&aleatorio(256 * 1024, 2), 256 * 1024).expect("escreve");
        std::fs::write(raiz.join("zeta/h1"), aleatorio(512 * 1024, 3)).expect("h1");
        std::fs::hard_link(raiz.join("zeta/h1"), raiz.join("meio/h2")).expect("ln");
        let bytes = |f: &Path| std::fs::metadata(f).expect("stat").blocks() as i64 * 512;
        let (tam, tam_h) = (bytes(&a), bytes(&raiz.join("zeta/h1")));
        let raiz_s = std::fs::canonicalize(raiz).expect("canonicalize");
        let rodar = || walk(raiz_s.to_str().expect("utf8"), 4, Arc::new(Ctx::ramo(1))).expect("walk");
        let r = rodar();
        let alloc = |k: &str| r.buckets[k].alloc;
        let dup = |k: &str| r.buckets[k].dup_clone;
        // Grupo de clone completo {a, b, c}: contado em "alfa"; as cópias de "zeta" e "meio" ficam lá.
        assert_eq!((r.clone_dup_files, r.clone_dup_bytes), (2, 2 * tam));
        assert_eq!((alloc("alfa"), dup("alfa"), dup("zeta"), dup("meio")), (tam, 0, tam, tam));
        // Hardlink h1/h2: contado uma vez, em "meio" (vem antes de "zeta").
        assert_eq!((r.link_dup_files, r.link_dup_bytes), (1, tam_h));
        assert_eq!(alloc("meio"), tam_h);
        // p.bin divide 768 KiB com o grupo (de FILEID menor): só os 256 KiB dele ficam em "zeta".
        assert_eq!((r.shared_dup_files, r.shared_dup_bytes, alloc("zeta")), (1, tam - 256 * 1024, 256 * 1024));
        let resumo = |r: &Result| format!("{:?} {:?}", r.buckets.iter().collect::<BTreeMap<_, _>>(), (r.link_dup_files, r.clone_dup_files, r.shared_dup_files, r.shared_dup_bytes));
        let base = resumo(&r);
        for _ in 0..5 {
            assert_eq!(resumo(&rodar()), base, "a rodada muda o resultado");
        }
    }

    #[test]
    fn atributo_ausente_nao_vira_numero() {
        let e = Entrada { linkcount: Some(1), allocsize: Some(8192), privatesize: Some(4096), ..Default::default() };
        assert_eq!(tamanhos(&e), Some((8192, 4096)));
        assert_eq!(tamanhos(&Entrada { privatesize: None, ..e.clone() }), None, "privado não vira o alocado");
        assert_eq!(tamanhos(&Entrada { allocsize: None, ..e.clone() }), None);
        assert_eq!(tamanhos(&Entrada { linkcount: None, ..e.clone() }), None, "link ausente não vira 1");
        // Hardlink sem FILEID seria somado uma vez por link.
        assert_eq!(tamanhos(&Entrada { linkcount: Some(2), ..e.clone() }), None);
        assert_eq!(tamanhos(&Entrada { linkcount: Some(2), fileid: Some(9), ..e }), Some((8192, 4096)));
    }

    /// Pasta além de PATH_MAX (1024 B): aberta a partir do fd da raiz, componente a componente. Antes
    /// o open do caminho inteiro dava ENAMETOOLONG e os 5 MB sumiam com a árvore "completa".
    #[test]
    fn caminho_longo_e_medido() {
        let pasta = Pasta(std::env::temp_dir().join(format!("sdtlongo-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&pasta.0);
        std::fs::create_dir_all(&pasta.0).expect("mkdir");
        let raiz = std::fs::canonicalize(&pasta.0).expect("canonicalize");
        let nome = "d".repeat(230);
        // O shell cria descendo com `cd` relativo (mkdir do caminho inteiro daria ENAMETOOLONG).
        let script = format!("for i in 1 2 3 4 5; do mkdir {nome} && cd -P {nome} || exit 1; done; head -c 5242880 /dev/urandom > f");
        let ok = std::process::Command::new("/bin/sh").arg("-c").arg(script).current_dir(&raiz).status().is_ok_and(|s| s.success());
        assert!(ok, "sh");
        let fundo = (0..5).fold(raiz.clone(), |p, _| p.join(&nome));
        assert!(fundo.join("f").as_os_str().len() >= 1180);
        assert!(sys::apfs::abrir_pasta(&fundo).is_err_and(|e| e == libc::ENAMETOOLONG), "o caminho passa de PATH_MAX");
        let r = walk(raiz.to_str().expect("utf8"), 2, Arc::new(Ctx::ramo(1))).expect("walk");
        assert_eq!((r.total.files, nao_lidos(&r.errors)), (1, 0), "{:?}", r.errors);
        assert!(r.total.alloc >= 5 << 20, "{}", r.total.alloc);
    }

    /// Montagem na tabela (injetada): nem a entrada na listagem nem um job direto nela abre a pasta.
    #[test]
    fn montagem_da_tabela_nao_e_aberta() {
        static NUNCA: AtomicBool = AtomicBool::new(false);
        let pasta = Pasta(std::env::temp_dir().join(format!("sdtmont-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&pasta.0);
        for d in ["a", "b"] {
            std::fs::create_dir_all(pasta.0.join(d)).expect("mkdir");
            std::fs::write(pasta.0.join(d).join("f"), b"x").expect("f");
        }
        let raiz = std::fs::canonicalize(&pasta.0).expect("canonicalize");
        let raiz_s = raiz.to_str().expect("utf8");
        let ms = vec!["/".to_string(), format!("{raiz_s}/a")];
        let (ctx, ctl, nomes) = (Ctx::ramo(1), Controle::new(&NUNCA), Mutex::new(Nomes::default()));
        let fd = sys::apfs::abrir_pasta(&raiz).expect("abre");
        let root_dev = std::fs::symlink_metadata(&raiz).expect("lstat").dev() as u32;
        let w = Walker { root: raiz_s, mnt: "/", root_dev, raiz_fd: Some(&fd), ctx: &ctx, montagens: Some(&ms), ctl: &ctl, nomes: &nomes };
        let mut buf = vec![0u8; 64 * 1024];
        let ler = |path: PathBuf, depth: usize| {
            let (mut l, mut pastas) = (Local::default(), Vec::new());
            w.scan_dir(&Job { path, bucket: 0, depth, dono: 0 }, &mut buf.clone(), &mut l, &mut |a| {
                if let Achado::Pasta(j) = a {
                    pastas.push(j.path);
                }
            });
            (l, pastas)
        };
        let (l, pastas) = ler(raiz.clone(), 0);
        assert_eq!((pastas, l.other_dev), (vec![raiz.join("b")], 1), "a/ é montagem: pulada na listagem");
        // Montou depois de a mãe ser listada: o job já na fila não abre.
        let (l, pastas) = ler(raiz.join("a"), 1);
        assert_eq!((l.dirs, l.other_dev, pastas.len(), l.errors.len()), (0, 1, 0, 0));
        let (l, _) = ler(raiz.join("b"), 1);
        assert_eq!((l.dirs, l.other_dev), (1, 0));
        buf.clear();
    }

    /// Pasta do meio trocada por symlink não leva a abertura para fora (O_NOFOLLOW_ANY).
    #[test]
    fn abrir_sob_nao_segue_symlink_do_meio() {
        let pasta = Pasta(std::env::temp_dir().join(format!("sdtlink-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&pasta.0);
        std::fs::create_dir_all(pasta.0.join("real/x")).expect("mkdir");
        std::os::unix::fs::symlink("real", pasta.0.join("link")).expect("symlink");
        let fd = sys::apfs::abrir_pasta(&pasta.0).expect("abre");
        assert!(sys::apfs::abrir_pasta_sob(&fd, Path::new("real/x")).is_ok());
        assert_eq!(sys::apfs::abrir_pasta_sob(&fd, Path::new("link/x")).err(), Some(libc::ELOOP));
        assert_eq!(sys::apfs::abrir_pasta_sob(&fd, Path::new("link")).err(), Some(libc::ELOOP));
    }

    #[test]
    fn maiores_sem_hardlink_repetido() {
        let mut v = vec![(500, 1, PathBuf::from("/z"), Some(7)), (500, 1, PathBuf::from("/a"), Some(7)), (500, 1, PathBuf::from("/m"), None), (900, 1, PathBuf::from("/c1"), None), (900, 1, PathBuf::from("/c2"), None)];
        podar_maiores(&mut v);
        let nomes: Vec<&str> = v.iter().map(|x| x.2.to_str().unwrap_or("")).collect();
        // O hardlink fica uma vez (o primeiro nome); arquivos distintos (clones inclusive) ficam todos.
        assert_eq!(nomes, ["/c1", "/c2", "/a", "/m"]);
    }

    #[test]
    fn cancelada_antes_marca_interrompida() {
        static JA: AtomicBool = AtomicBool::new(true);
        let ctl = Arc::new(Controle::new(&JA));
        let r = walk_ctl(env!("CARGO_MANIFEST_DIR"), 2, Arc::new(Ctx::ramo(1)), ctl.clone()).expect("walk");
        assert!(r.interrompida);
        assert_eq!((r.total.files, r.dirs, ctl.arquivos.load(Ordering::Relaxed)), (0, 0, 0));
        let r = walk(concat!(env!("CARGO_MANIFEST_DIR"), "/src"), 2, Arc::new(Ctx::ramo(1))).expect("walk");
        assert!(!r.interrompida && r.total.files > 0);
    }

    #[test]
    fn cancelada_no_meio_marca_interrompida() {
        static CORTE: AtomicBool = AtomicBool::new(false);
        let ctl = Arc::new(Controle::new(&CORTE));
        let c = ctl.clone();
        // Corta assim que o primeiro arquivo é contado.
        let vigia = std::thread::spawn(move || {
            while c.arquivos.load(Ordering::Relaxed) == 0 {
                std::hint::spin_loop();
            }
            CORTE.store(true, Ordering::Relaxed);
        });
        let r = walk_ctl("/usr/share", 2, Arc::new(Ctx::ramo(1)), ctl).expect("walk");
        let _ = vigia.join();
        let cheia = walk("/usr/share", 2, Arc::new(Ctx::ramo(1))).expect("walk");
        assert!(r.interrompida && !cheia.interrompida);
        assert!(r.total.files > 0 && r.total.files < cheia.total.files, "{} de {}", r.total.files, cheia.total.files);
    }
}

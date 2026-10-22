//! Árvore viva (`sdtally --auto`): a mesma leitura de pasta da travessia única (`scan_dir`), com o
//! que cada pasta achou guardado por pasta. Assim dá para reler só a pasta que o FSEvents avisar,
//! tirar da soma o que ela somava e pôr o novo. Guarda o total de cada pasta, nunca arquivo por
//! arquivo: com ~2 milhões de arquivos isso passaria do teto de 200 MB.
//!
//! A soma sai igual à da travessia única feita no mesmo instante (`resultado`), com uma exceção:
//! no grupo de clone completo a travessia pega as faixas físicas do primeiro arquivo que chega (e,
//! se ele não tem faixas, o grupo fica sem); aqui vale o primeiro que tiver faixas. As cópias de
//! clone completo dividem todos os blocos, então as faixas de qualquer uma servem.

use super::*;
use std::os::darwin::fs::MetadataExt as _;

const SEM_PAI: u32 = u32::MAX;
/// Pastas por bloco do arranjo: crescer não copia o que já existe (sem o pico do Vec que dobra).
const BLOCO: usize = 4096;

/// Um repetido (hardlink ou cópia de clone) visto numa pasta. Só alocado e privado; os campos
/// raros do `Peso` (dataless, xattr) ficam em `Estado::raros`, pelo FILEID.
struct Visto {
    dir: u32,
    bucket: u32,
    id: u64,
    alloc: i64,
    private: i64,
    raro: bool,
}

impl Visto {
    fn de(dir: u32, o: &Ocorr) -> (Visto, Option<Peso>) {
        let p = o.peso;
        let raro = p.dataless_n != 0 || p.dataless_logical != 0 || p.xattr_stream != 0 || p.xattr_all != 0;
        (Visto { dir, bucket: o.bucket, id: o.id, alloc: p.alloc, private: p.private, raro }, raro.then_some(p))
    }
}

#[derive(Default)]
struct GrupoV {
    vistos: Vec<Visto>,
    /// CLONEID do hardlink (0 = não é clone completo); o mesmo em todo nome do inode.
    clone_of: u64,
    /// Faixas físicas de uma das cópias (clone completo); hardlink não tem.
    faixas: Option<Box<[(u64, u64)]>>,
}

/// Arquivo comum que pode dividir blocos, pelo FILEID.
struct CandV {
    dir: u32,
    alloc: i64,
    bucket: u32,
    faixas: Box<[(u64, u64)]>,
}

/// O que uma pasta tem além da soma simples no próprio balde; ausente na maioria das pastas.
#[derive(Default)]
struct Extra {
    /// Soma completa por balde: pasta dataless filha (outro balde) ou, no próprio balde, quando há
    /// campo além de arquivos, alocado e privado.
    outras: Vec<(u32, Sum)>,
    links: Vec<u64>,
    clones: Vec<u64>,
    cands: Vec<u64>,
    erros: Vec<(i32, u64)>,
    /// EPERM/EACCES ao abrir esta pasta (deste volume).
    negada: Option<i32>,
    other_dev: u64,
    graft: u64,
    sem_mapa: u64,
    xattr_erros: u64,
}

impl Extra {
    fn vazio(&self) -> bool {
        self.outras.is_empty()
            && self.links.is_empty()
            && self.clones.is_empty()
            && self.cands.is_empty()
            && self.erros.is_empty()
            && self.negada.is_none()
            && self.other_dev + self.graft + self.sem_mapa + self.xattr_erros == 0
    }
}

struct Pasta {
    pai: u32,
    bucket: u32,
    /// Soma simples desta pasta no próprio balde, sem descer.
    files: u32,
    /// Aberta e contada como pasta (o `dirs` da travessia).
    aberta: bool,
    alloc: i64,
    private: i64,
    /// Xattrs dos arquivos (a travessia do modo vivo mede, como a completa).
    xattr_stream: i64,
    xattr_all: i64,
    nome: Box<[u8]>,
    filhas: Box<[u32]>,
    extra: Option<Box<Extra>>,
}

impl Pasta {
    fn nova(pai: u32, nome: Box<[u8]>, bucket: u32) -> Pasta {
        Pasta { pai, bucket, files: 0, aberta: false, alloc: 0, private: 0, xattr_stream: 0, xattr_all: 0, nome, filhas: Box::default(), extra: None }
    }

    fn simples(&self) -> Sum {
        Sum { files: self.files as u64, alloc: self.alloc, private: self.private, xattr_stream: self.xattr_stream, xattr_all: self.xattr_all, ..Default::default() }
    }
}

/// Repetidos já resolvidos (só refeito quando um grupo muda).
#[derive(Default)]
struct Grupos {
    buckets: HashMap<u32, Sum>,
    link: (u64, i64),
    clone: (u64, i64),
    /// Representante de cada grupo de clone com faixas: (CLONEID, FILEID, alocado, balde).
    reps: Vec<(u64, u64, i64, u32)>,
}

/// Blocos divididos já descontados (só refeito quando um candidato muda).
#[derive(Default)]
struct Sobra {
    buckets: Vec<(u32, i64)>,
    files: u64,
    bytes: i64,
}

#[derive(Default)]
struct Estado {
    blocos: Vec<Box<[Option<Pasta>]>>,
    livres: Vec<u32>,
    usadas: usize,
    /// Σ das somas diretas de todas as pastas, por balde (sem repetidos nem desconto de blocos).
    plano: HashMap<u32, Sum>,
    links: HashMap<u64, GrupoV>,
    clones: HashMap<u64, GrupoV>,
    cands: HashMap<u64, CandV>,
    // shortcut: entrada de `raros` não sai quando o arquivo some; é rara (xattr medido ou
    // hardlink/clone dataless) e pequena; trocar por contagem se o modo vivo medir xattr.
    raros: HashMap<u64, Peso>,
    grupos: Option<Grupos>,
    sobra: Option<Sobra>,
    /// Algum repetido ou candidato mudou desde a última conta pesada.
    grupos_sujo: bool,
    sobra_sujo: bool,
    /// Quando a conta pesada (repetidos e blocos divididos) foi refeita.
    refeito: Option<std::time::Instant>,
}

impl Estado {
    /// Σ do alocado direto de todas as pastas (barato: um item por balde).
    fn plano_alloc(&self) -> i64 {
        self.plano.values().map(|s| s.alloc).sum()
    }

    fn pasta(&self, id: u32) -> Option<&Pasta> {
        self.blocos.get(id as usize / BLOCO)?[id as usize % BLOCO].as_ref()
    }

    fn pasta_mut(&mut self, id: u32) -> Option<&mut Pasta> {
        self.blocos.get_mut(id as usize / BLOCO)?[id as usize % BLOCO].as_mut()
    }

    fn tirar(&mut self, id: u32) -> Option<Pasta> {
        let p = self.blocos.get_mut(id as usize / BLOCO)?[id as usize % BLOCO].take()?;
        self.livres.push(id);
        self.usadas -= 1;
        Some(p)
    }

    fn nova(&mut self, p: Pasta) -> u32 {
        let id = match self.livres.pop() {
            Some(id) => id,
            None => {
                let id = (self.blocos.len() * BLOCO) as u32;
                self.blocos.push((0..BLOCO).map(|_| None).collect());
                self.livres.extend((id + 1..id + BLOCO as u32).rev());
                id
            }
        };
        self.blocos[id as usize / BLOCO][id as usize % BLOCO] = Some(p);
        self.usadas += 1;
        id
    }

    fn todas(&self) -> impl Iterator<Item = (u32, &Pasta)> {
        self.blocos.iter().flat_map(|b| b.iter()).enumerate().filter_map(|(i, p)| Some((i as u32, p.as_ref()?)))
    }

    /// A filha de `id` com este nome.
    fn filha(&self, id: u32, nome: &[u8]) -> Option<u32> {
        self.pasta(id)?.filhas.iter().copied().find(|&f| self.pasta(f).is_some_and(|p| &*p.nome == nome))
    }

    fn peso(&self, v: &Visto) -> Peso {
        match v.raro.then(|| self.raros.get(&v.id)).flatten() {
            Some(p) => *p,
            None => Peso { alloc: v.alloc, private: v.private, dataless_logical: 0, xattr_stream: 0, xattr_all: 0, dataless_n: 0 },
        }
    }

    /// Tira da soma o que a pasta somava, sem mexer nas filhas.
    fn limpar(&mut self, id: u32) {
        let Some(p) = self.pasta_mut(id) else { return };
        let (bucket, simples, extra) = (p.bucket, p.simples(), p.extra.take());
        (p.files, p.alloc, p.private, p.xattr_stream, p.xattr_all, p.aberta) = (0, 0, 0, 0, 0, false);
        if let Some(s) = self.plano.get_mut(&bucket) {
            s.sub(&simples);
        }
        let Some(x) = extra else { return };
        for (b, s) in &x.outras {
            if let Some(t) = self.plano.get_mut(b) {
                t.sub(s);
            }
        }
        for (tabela, chaves) in [(&mut self.links, &x.links), (&mut self.clones, &x.clones)] {
            for k in chaves {
                if let Some(g) = tabela.get_mut(k) {
                    g.vistos.retain(|v| v.dir != id);
                    if g.vistos.is_empty() {
                        tabela.remove(k);
                    }
                }
            }
        }
        if !x.links.is_empty() || !x.clones.is_empty() {
            self.grupos_sujo = true;
        }
        for k in &x.cands {
            // Arquivo que mudou de pasta pode já estar registrado pela outra.
            if self.cands.get(k).is_some_and(|c| c.dir == id) {
                self.cands.remove(k);
                self.sobra_sujo = true;
            }
        }
    }

    /// Tira a pasta e tudo abaixo dela. A lista de filhas do pai fica com quem chamou.
    fn remover(&mut self, id: u32) {
        let mut fila = vec![id];
        while let Some(x) = fila.pop() {
            self.limpar(x);
            if let Some(p) = self.tirar(x) {
                fila.extend(p.filhas.iter());
            }
        }
    }

    fn caminho(&self, raiz: &str, id: u32) -> PathBuf {
        let mut nomes = Vec::new();
        let mut x = id;
        while let Some(p) = self.pasta(x) {
            if p.pai == SEM_PAI {
                break;
            }
            nomes.push(&*p.nome);
            x = p.pai;
        }
        let mut c = PathBuf::from(raiz);
        for n in nomes.iter().rev() {
            c.push(OsStr::from_bytes(n));
        }
        c
    }

    /// Guarda o que a leitura de `id` achou e devolve as filhas que ainda precisam ser lidas
    /// (novas). Filha que sumiu sai da árvore; filha que continua fica como estava.
    fn aplicar(&mut self, id: u32, achados: Vec<Achado>, l: Local) -> Vec<Job> {
        let Some(bucket) = self.pasta(id).map(|p| p.bucket) else { return Vec::new() };
        let mut soma = Sum::default();
        let mut x = Extra::default();
        let mut subs = Vec::new();
        for a in achados {
            let (b, s) = match a {
                Achado::Soma(b, s) => (b, s),
                Achado::Cand(b, s, fid, f) => {
                    self.cands.insert(fid, CandV { dir: id, alloc: s.alloc, bucket: b, faixas: f.into() });
                    self.sobra_sujo = true;
                    x.cands.push(fid);
                    (b, s)
                }
                Achado::Link(k, o) => {
                    let (v, raro) = Visto::de(id, &o);
                    if let Some(p) = raro {
                        self.raros.insert(v.id, p);
                    }
                    let g = self.links.entry(k).or_default();
                    g.clone_of = o.clone_of;
                    g.vistos.push(v);
                    self.grupos_sujo = true;
                    x.links.push(k);
                    continue;
                }
                Achado::Clone(k, o, mapa) => {
                    let (v, raro) = Visto::de(id, &o);
                    if let Some(p) = raro {
                        self.raros.insert(v.id, p);
                    }
                    let g = self.clones.entry(k).or_default();
                    if g.faixas.is_none() {
                        g.faixas = mapa.map(|(_, f)| f.into());
                    }
                    g.vistos.push(v);
                    self.grupos_sujo = true;
                    x.clones.push(k);
                    continue;
                }
                Achado::Pasta(j) => {
                    subs.push(j);
                    continue;
                }
            };
            if b == bucket {
                soma.add(&s);
            } else {
                match x.outras.iter_mut().find(|o| o.0 == b) {
                    Some(o) => o.1.add(&s),
                    None => x.outras.push((b, s)),
                }
            }
        }
        // Só arquivos, alocado, privado e xattr cabem na pasta; o resto vai inteiro para `outras`.
        let simples = Sum { files: soma.files, alloc: soma.alloc, private: soma.private, xattr_stream: soma.xattr_stream, xattr_all: soma.xattr_all, ..Default::default() } == soma && soma.files <= u32::MAX as u64;
        if !simples {
            x.outras.push((bucket, soma));
            soma = Sum::default();
        }
        self.plano.entry(bucket).or_default().add(&soma);
        for (b, s) in &x.outras {
            self.plano.entry(*b).or_default().add(s);
        }
        let cortada = l.cortada;
        let mut erros: Vec<(i32, u64)> = l.errors.into_iter().collect();
        erros.sort_unstable();
        x.erros = erros;
        x.negada = l.denied.first().map(|d| d.1);
        (x.other_dev, x.graft, x.sem_mapa, x.xattr_erros) = (l.other_dev, l.graft_roots, l.sem_mapa, l.xattr_erros);

        // Filhas: as que sumiram saem; as novas viram pasta e voltam para ler. Listagem cortada
        // no meio não diz quem sumiu: as antigas ficam.
        let antigas: Vec<u32> = self.pasta(id).map(|p| p.filhas.to_vec()).unwrap_or_default();
        let nomes: Vec<Box<[u8]>> = subs.iter().map(|j| j.path.file_name().map(|n| n.as_bytes().into()).unwrap_or_default()).collect();
        let mut ficam = Vec::new();
        for f in antigas {
            match self.pasta(f).map(|p| nomes.contains(&p.nome)) {
                Some(true) => ficam.push(f),
                Some(false) if cortada => ficam.push(f),
                _ => self.remover(f),
            }
        }
        let mut ler = Vec::new();
        for (mut j, nome) in subs.into_iter().zip(nomes) {
            if ficam.iter().any(|&f| self.pasta(f).is_some_and(|p| p.nome == nome)) {
                continue;
            }
            let f = self.nova(Pasta::nova(id, nome, j.bucket));
            ficam.push(f);
            j.dono = f;
            ler.push(j);
        }
        if let Some(p) = self.pasta_mut(id) {
            (p.files, p.alloc, p.private, p.xattr_stream, p.xattr_all) = (soma.files as u32, soma.alloc, soma.private, soma.xattr_stream, soma.xattr_all);
            p.aberta = l.dirs > 0;
            p.extra = (!x.vazio()).then(|| Box::new(x));
            p.filhas = ficam.into();
        }
        ler
    }

    /// Hardlinks e clones completos, contados uma vez como na travessia única.
    fn grupos(&self, nomes: &Nomes) -> Grupos {
        let mut r = Grupos::default();
        // O balde que vem antes em ordem alfabética fica com o grupo.
        let menor = |a: u32, b: u32| if b != a && nomes.antes(b, a) { b } else { a };
        // Hardlink que também é clone completo entra nos clones, como uma ocorrência só.
        let mut de_links: HashMap<u64, Vec<(u64, u32, Peso)>> = HashMap::new();
        for (&k, g) in &self.links {
            // Todos são o mesmo inode: vale o peso da leitura mais recente (a pasta de outro nome do
            // mesmo arquivo não recebe aviso quando ele muda).
            let Some(rep) = g.vistos.last() else { continue };
            let bucket = g.vistos.iter().fold(g.vistos[0].bucket, |a, v| menor(a, v.bucket));
            let n = g.vistos.len() as i64;
            let peso = self.peso(rep);
            r.link.0 += (n - 1) as u64;
            r.link.1 += peso.alloc * (n - 1);
            if g.clone_of != 0 {
                de_links.entry(g.clone_of).or_default().push((k, bucket, peso));
            } else {
                r.buckets.entry(bucket).or_default().add(&peso.soma());
            }
        }
        let chaves: Vec<u64> = self.clones.keys().chain(de_links.keys().filter(|k| !self.clones.contains_key(k))).copied().collect();
        for k in chaves {
            let g = self.clones.get(&k);
            let ocorr: Vec<(u64, u32, Peso)> = g.into_iter().flat_map(|g| g.vistos.iter().map(|v| (v.id, v.bucket, self.peso(v)))).chain(de_links.get(&k).into_iter().flatten().copied()).collect();
            let Some(&(rep_id, _, peso)) = ocorr.iter().min_by_key(|o| o.0) else { continue };
            let bucket = ocorr.iter().fold(ocorr[0].1, |a, o| menor(a, o.1));
            let n = ocorr.len() as i64;
            r.buckets.entry(bucket).or_default().add(&peso.soma());
            r.clone.0 += (n - 1) as u64;
            r.clone.1 += peso.alloc * (n - 1);
            let mut onde: Vec<(u32, i64)> = Vec::new();
            for o in &ocorr {
                match onde.iter_mut().find(|x| x.0 == o.1) {
                    Some(x) => x.1 += 1,
                    None => onde.push((o.1, 1)),
                }
            }
            for (b, c) in onde {
                let copias = if b == bucket { c - 1 } else { c };
                r.buckets.entry(b).or_default().dup_clone += peso.alloc * copias;
            }
            if g.is_some_and(|g| g.faixas.is_some()) {
                r.reps.push((k, rep_id, peso.alloc, bucket));
            }
        }
        r
    }

    /// Blocos que dois arquivos dividem contam uma vez (o de menor FILEID fica com eles).
    fn sobra(&self, reps: &[(u64, u64, i64, u32)]) -> Sobra {
        let mut itens: Vec<(u64, &[(u64, u64)])> = Vec::with_capacity(reps.len() + self.cands.len());
        let mut donos: Vec<(i64, u32)> = Vec::with_capacity(itens.capacity());
        for &(k, id, alloc, b) in reps {
            if let Some(f) = self.clones.get(&k).and_then(|g| g.faixas.as_deref()) {
                itens.push((id, f));
                donos.push((alloc, b));
            }
        }
        for (&id, c) in &self.cands {
            itens.push((id, &c.faixas));
            donos.push((c.alloc, c.bucket));
        }
        let mut r = Sobra::default();
        for ((alloc, b), sobra) in donos.into_iter().zip(sobra_de(&itens)) {
            let sobra = sobra.min(alloc);
            if sobra > 0 {
                match r.buckets.iter_mut().find(|x| x.0 == b) {
                    Some(x) => x.1 += sobra,
                    None => r.buckets.push((b, sobra)),
                }
                r.files += 1;
                r.bytes += sobra;
            }
        }
        r
    }
}

pub struct Vivo {
    /// Intervalo mínimo entre duas contas pesadas (~200 ms com 300 mil repetidos): entre elas
    /// os repetidos e os blocos divididos ficam como na última. Zero = sempre exata.
    pub intervalo_pesado: std::time::Duration,
    raiz: String,
    mnt: String,
    root_dev: u32,
    /// Relida a cada leitura: volume montado depois (USB, SMB, destino do Time Machine em
    /// /Volumes/.timemachine) não é aberto.
    montagens: Mutex<Option<Vec<String>>>,
    raiz_fd: Option<OwnedFd>,
    ctx: Arc<Ctx>,
    ctl: Arc<Controle>,
    nomes: Mutex<Nomes>,
    estado: Mutex<Estado>,
}

impl Vivo {
    pub fn novo(raiz: &str, ctx: Arc<Ctx>, ctl: Arc<Controle>) -> std::result::Result<Vivo, String> {
        let root_dev = std::fs::symlink_metadata(raiz).map_err(|e| format!("raiz: {e}"))?.dev() as u32;
        let v = Vivo {
            intervalo_pesado: std::time::Duration::ZERO,
            raiz: raiz.to_string(),
            mnt: sys::montado_em(Path::new(raiz)).unwrap_or_else(|_| raiz.to_string()),
            root_dev,
            montagens: Mutex::new(sys::montagens().ok()),
            raiz_fd: sys::apfs::abrir_pasta(Path::new(raiz)).ok(),
            ctx,
            ctl,
            nomes: Mutex::new(Nomes::default()),
            estado: Mutex::new(Estado::default()),
        };
        let b = lock(&v.nomes).id("/");
        lock(&v.estado).nova(Pasta::nova(SEM_PAI, Box::default(), b));
        Ok(v)
    }

    fn walker<'a>(&'a self, montagens: Option<&'a [String]>) -> Walker<'a> {
        Walker { root: &self.raiz, mnt: &self.mnt, root_dev: self.root_dev, raiz_fd: self.raiz_fd.as_ref(), ctx: &self.ctx, montagens, ctl: &self.ctl, nomes: &self.nomes }
    }

    /// Relê a tabela de montagens (getfsstat MNT_NOWAIT, barato); se falhar, fica a anterior.
    fn montagens(&self) -> Option<Vec<String>> {
        let mut m = lock(&self.montagens);
        if let Ok(novas) = sys::montagens() {
            *m = Some(novas);
        }
        m.clone()
    }

    /// Lê uma pasta e guarda; devolve as filhas novas a ler.
    fn ler(&self, w: &Walker, job: &Job, buf: &mut [u8]) -> Vec<Job> {
        let mut l = Local::default();
        let mut achados = Vec::new();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w.scan_dir(job, buf, &mut l, &mut |a| achados.push(a))));
        if r.is_err() {
            // Como na travessia: a pasta conta como erro e não desce; as filhas que já tinha ficam.
            *l.errors.entry(ERRO_INTERNO).or_default() += 1;
            l.cortada = true;
            achados.retain(|a| !matches!(a, Achado::Pasta(_)));
        }
        let mut e = lock(&self.estado);
        if l.nao_lida && e.pasta(job.dono).is_some_and(|p| p.aberta) {
            // Não abriu desta vez (EMFILE, EIO, ...): fica o que a última leitura mediu, com o erro
            // (o termo sai parcial até uma releitura dar certo).
            if let Some(p) = e.pasta_mut(job.dono) {
                let x = p.extra.get_or_insert_with(Box::default);
                for (k, v) in l.errors {
                    if !x.erros.iter().any(|o| o.0 == k) {
                        x.erros.push((k, v));
                    }
                }
            }
            return Vec::new();
        }
        e.limpar(job.dono);
        e.aplicar(job.dono, achados, l)
    }

    /// Lê os jobs e tudo abaixo deles, com `threads` threads.
    fn percorrer(&self, jobs: Vec<Job>, threads: usize, montagens: Option<&[String]>) {
        let n = jobs.len();
        let q = Queue { stack: Mutex::new((jobs, n)), cv: Condvar::new() };
        let trabalho = || {
            sys::qos_utility();
            let w = self.walker(montagens);
            let mut buf = vec![0u8; 256 * 1024];
            loop {
                let job = {
                    let mut g = lock(&q.stack);
                    loop {
                        if let Some(j) = g.0.pop() {
                            break Some(j);
                        }
                        if g.1 == 0 {
                            break None;
                        }
                        g = q.cv.wait(g).unwrap_or_else(|e| e.into_inner());
                    }
                };
                let Some(job) = job else {
                    q.cv.notify_all();
                    return;
                };
                let mut novas = if self.ctl.cancelado() { Vec::new() } else { self.ler(&w, &job, &mut buf) };
                let mut g = lock(&q.stack);
                g.1 += novas.len();
                g.1 -= 1;
                g.0.append(&mut novas);
                drop(g);
                q.cv.notify_all();
            }
        };
        if threads <= 1 {
            trabalho();
        } else {
            std::thread::scope(|sc| {
                for _ in 0..threads {
                    sc.spawn(trabalho);
                }
            });
        }
    }

    /// Esvazia as filhas de `id` (e tudo abaixo).
    fn podar(e: &mut Estado, id: u32) {
        let filhas: Vec<u32> = e.pasta(id).map(|p| p.filhas.to_vec()).unwrap_or_default();
        for f in filhas {
            e.remover(f);
        }
        if let Some(p) = e.pasta_mut(id) {
            p.filhas = Box::default();
        }
    }

    /// A passada inteira, do zero.
    pub fn passada(&self, threads: usize) {
        let job = {
            let mut e = lock(&self.estado);
            Self::podar(&mut e, 0);
            Job { path: PathBuf::from(&self.raiz), bucket: e.pasta(0).map_or(0, |p| p.bucket), depth: 0, dono: 0 }
        };
        let ms = self.montagens();
        self.percorrer(vec![job], threads, ms.as_deref());
        let mut g = lock(&self.estado);
        let e = &mut *g;
        for g in e.links.values_mut().chain(e.clones.values_mut()) {
            g.vistos.shrink_to_fit();
        }
        e.links.shrink_to_fit();
        e.clones.shrink_to_fit();
        e.cands.shrink_to_fit();
        e.plano.shrink_to_fit();
    }

    /// Relê a pasta do `caminho` (absoluto, como o FSEvents manda: "/Users/x/..." ou com a raiz na
    /// frente). Sem `recursivo`, só ela: filhas que sumiram saem, novas são lidas inteiras.
    /// Com `recursivo`, relê tudo abaixo. Pasta que a árvore não conhece: relê a mais funda que
    /// ela conhece no caminho, que acha a nova como filha. Devolve quanto o alocado somado mudou
    /// (soma simples, sem repetidos nem blocos divididos): é o que alimenta o "crescendo agora".
    pub fn reler(&self, caminho: &str, recursivo: bool) -> i64 {
        let rel = caminho.strip_prefix(self.raiz.as_str()).filter(|r| r.is_empty() || r.starts_with('/')).unwrap_or(caminho);
        let ms = self.montagens();
        let (job, antes) = {
            let mut e = lock(&self.estado);
            let antes = e.plano_alloc();
            let mut id = 0u32;
            for c in rel.split('/').filter(|c| !c.is_empty()) {
                match e.filha(id, c.as_bytes()) {
                    Some(f) => id = f,
                    None => break,
                }
            }
            let mut path = e.caminho(&self.raiz, id);
            // Virou ponto de montagem (sem lstat, que acordaria um volume de rede) ou pasta
            // dataless (listar materializaria): relê a mãe, que a pula ou a conta como dataless
            // e a tira da árvore.
            let montagem = id != 0 && ms.as_deref().is_some_and(|ms| e_montagem(ms, &self.mnt, &path.to_string_lossy()));
            let lstat = (id != 0 && !montagem).then(|| std::fs::symlink_metadata(&path));
            // Sumiu do disco (só ENOENT/ENOTDIR): sai da árvore; o pai tem o seu próprio aviso.
            // Outro erro (EIO, EACCES, ...) não diz que sumiu: a pasta é relida e fica com o erro.
            if matches!(&lstat, Some(Err(err)) if matches!(err.raw_os_error(), Some(libc::ENOENT | libc::ENOTDIR))) {
                let pai = e.pasta(id).map_or(SEM_PAI, |p| p.pai);
                e.remover(id);
                if let Some(p) = e.pasta_mut(pai) {
                    p.filhas = p.filhas.iter().copied().filter(|&f| f != id).collect();
                }
                return e.plano_alloc() - antes;
            }
            let subir = montagem || matches!(&lstat, Some(Ok(m)) if m.st_flags() & SF_DATALESS != 0);
            if subir {
                id = e.pasta(id).map_or(0, |p| p.pai);
                path = e.caminho(&self.raiz, id);
            }
            if recursivo && !subir {
                Self::podar(&mut e, id);
            }
            let Some(p) = e.pasta(id) else { return 0 };
            (Job { path, bucket: p.bucket, depth: usize::from(id != 0), dono: id }, antes)
        };
        self.percorrer(vec![job], 1, ms.as_deref());
        lock(&self.estado).plano_alloc() - antes
    }

    /// Pastas na árvore.
    pub fn pastas(&self) -> usize {
        lock(&self.estado).usadas
    }

    /// Refaz repetidos e blocos divididos se algum mudou e o intervalo venceu.
    fn pesada(&self, e: &mut Estado, nomes: &Nomes) {
        let vence = e.refeito.is_none_or(|t| t.elapsed() >= self.intervalo_pesado);
        if e.grupos.is_none() || (e.grupos_sujo && vence) {
            let g = e.grupos(nomes);
            (e.grupos, e.grupos_sujo, e.sobra_sujo) = (Some(g), false, true);
            e.refeito = Some(std::time::Instant::now());
        }
        if e.sobra.is_none() || (e.sobra_sujo && vence) {
            let s = e.sobra(e.grupos.as_ref().map_or(&[][..], |g| &g.reps));
            (e.sobra, e.sobra_sujo) = (Some(s), false);
            e.refeito = Some(std::time::Instant::now());
        }
    }

    /// Só o alocado total (barato: sem percorrer as pastas), para saber se vale montar o resultado.
    pub fn total_alloc(&self) -> i64 {
        let mut e = lock(&self.estado);
        let nomes = lock(&self.nomes);
        self.pesada(&mut e, &nomes);
        let plano: i64 = e.plano.values().map(|s| s.alloc).sum();
        let grupos: i64 = e.grupos.as_ref().map_or(0, |g| g.buckets.values().map(|s| s.alloc).sum());
        // Desconto só onde o balde existe, como em `resultado`.
        let sobra: i64 = e.sobra.as_ref().map_or(0, |s| s.buckets.iter().filter(|(b, _)| e.plano.contains_key(b) || e.grupos.as_ref().is_some_and(|g| g.buckets.contains_key(b))).map(|x| x.1).sum());
        plano + grupos - sobra
    }

    /// A soma de agora, no formato da travessia única. Repetidos e blocos divididos só são
    /// refeitos se algum mudou desde a última vez (e, com `intervalo_pesado`, se ele venceu).
    pub fn resultado(&self) -> Result {
        let mut e = lock(&self.estado);
        let nomes = lock(&self.nomes);
        self.pesada(&mut e, &nomes);
        let mut r = Result::default();
        let mut buckets = e.plano.clone();
        if let Some(g) = &e.grupos {
            for (b, s) in &g.buckets {
                buckets.entry(*b).or_default().add(s);
            }
            ((r.link_dup_files, r.link_dup_bytes), (r.clone_dup_files, r.clone_dup_bytes)) = (g.link, g.clone);
        }
        if let Some(s) = &e.sobra {
            for &(b, x) in &s.buckets {
                if let Some(t) = buckets.get_mut(&b) {
                    t.alloc -= x;
                }
            }
            (r.shared_dup_files, r.shared_dup_bytes) = (s.files, s.bytes);
        }
        for (id, p) in e.todas() {
            r.dirs += u64::from(p.aberta);
            let Some(x) = &p.extra else { continue };
            for &(k, v) in &x.erros {
                *r.errors.entry(k).or_default() += v;
            }
            if let Some(errno) = x.negada {
                r.denied.push((e.caminho(&self.raiz, id), errno));
            }
            r.other_dev += x.other_dev;
            r.graft_roots += x.graft;
            r.sem_mapa += x.sem_mapa;
            r.xattr_erros += x.xattr_erros;
        }
        for (k, v) in buckets {
            if v == Sum::default() {
                continue;
            }
            r.total.add(&v);
            r.buckets.insert(nomes.lista[k as usize].to_string(), v);
        }
        r.denied.sort();
        r.interrompida = self.ctl.cancelado();
        r
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn ctx() -> Arc<Ctx> {
        Arc::new(Ctx::new("ninguem", "", &[]))
    }

    fn ctl() -> Arc<Controle> {
        static NUNCA: AtomicBool = AtomicBool::new(false);
        Arc::new(Controle::new(&NUNCA))
    }

    /// Soma viva == travessia nova, nos campos que a travessia publica.
    fn confere(v: &Vivo, raiz: &str, quando: &str) {
        let a = v.resultado();
        let b = walk_ctl(raiz, 2, ctx(), ctl()).expect("walk");
        assert_eq!(a.total, b.total, "{quando}: total");
        assert_eq!(a.buckets, b.buckets, "{quando}: baldes");
        assert_eq!(a.dirs, b.dirs, "{quando}: pastas");
        assert_eq!((a.link_dup_files, a.link_dup_bytes, a.clone_dup_files, a.clone_dup_bytes), (b.link_dup_files, b.link_dup_bytes, b.clone_dup_files, b.clone_dup_bytes), "{quando}: repetidos");
        assert_eq!((a.shared_dup_files, a.shared_dup_bytes), (b.shared_dup_files, b.shared_dup_bytes), "{quando}: blocos divididos");
        assert_eq!(v.pastas() as u64, b.dirs, "{quando}: pastas guardadas");
    }

    fn escreve(p: &Path, n: usize) {
        fs::write(p, vec![7u8; n]).expect("escrever");
    }

    /// Listagem cortada no meio (erro depois de lotes lidos, panic) não apaga as filhas que não
    /// vieram; a completa, sim.
    #[test]
    fn listagem_cortada_mantem_filhas() {
        let mut e = Estado::default();
        let raiz = e.nova(Pasta::nova(SEM_PAI, Box::default(), 0));
        let pasta = |n: &str| Achado::Pasta(Job { path: PathBuf::from(format!("/r/{n}")), bucket: 0, depth: 1, dono: 0 });
        let lida = |cortada| Local { dirs: 1, cortada, ..Default::default() };
        assert_eq!(e.aplicar(raiz, vec![pasta("a"), pasta("b")], lida(false)).len(), 2);
        let filhas = |e: &Estado| e.pasta(raiz).map_or(0, |p| p.filhas.len());
        e.limpar(raiz);
        assert!(e.aplicar(raiz, vec![pasta("a")], lida(true)).is_empty());
        assert_eq!(filhas(&e), 2, "cortada: b fica");
        e.limpar(raiz);
        e.aplicar(raiz, vec![pasta("a")], lida(false));
        assert_eq!((filhas(&e), e.usadas), (1, 2), "completa: b sai");
    }

    /// Releitura que não abre a pasta (aqui ENOENT; no uso real EMFILE, EIO, que deixam a árvore
    /// parcial) fica com o que a última leitura mediu e com o erro.
    #[test]
    fn releitura_que_nao_abre_mantem_a_pasta() {
        let tmp = std::env::temp_dir().join(format!("sdtally-vivo-na-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("a/b")).unwrap();
        escreve(&tmp.join("a/f"), 40_000);
        escreve(&tmp.join("a/b/g"), 40_000);
        let raiz = tmp.to_str().unwrap().to_string();
        let v = Vivo::novo(&raiz, ctx(), ctl()).unwrap();
        v.passada(1);
        let antes = v.resultado();
        let a = lock(&v.estado).filha(0, b"a").expect("a");
        let ms = v.montagens();
        let w = v.walker(ms.as_deref());
        let job = Job { path: tmp.join("nao-existe"), bucket: 0, depth: 1, dono: a };
        assert!(v.ler(&w, &job, &mut vec![0u8; 64 * 1024]).is_empty());
        let depois = v.resultado();
        assert_eq!((depois.total, depois.dirs, v.pastas()), (antes.total, antes.dirs, 3));
        assert_eq!(depois.errors.get(&libc::ENOENT), Some(&1), "{:?}", depois.errors);
        // Relida de verdade, o erro sai.
        v.reler(&format!("{raiz}/a"), false);
        assert_eq!((v.resultado().total, v.resultado().errors.len()), (antes.total, 0));
        fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn vivo_acompanha_criar_apagar_mover() {
        let tmp = std::env::temp_dir().join(format!("sdtally-vivo-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("a/b/c")).unwrap();
        fs::create_dir_all(tmp.join("d")).unwrap();
        escreve(&tmp.join("a/f1"), 10_000);
        escreve(&tmp.join("a/b/f2"), 50_000);
        escreve(&tmp.join("a/b/c/f3"), 200_000);
        escreve(&tmp.join("d/f4"), 5_000);
        fs::hard_link(tmp.join("a/f1"), tmp.join("d/link-f1")).unwrap();
        // Clone completo (APFS): clonefile pelo cp -c.
        let clonou = std::process::Command::new("/bin/cp").args(["-c"]).arg(tmp.join("a/b/c/f3")).arg(tmp.join("d/clone-f3")).status().is_ok_and(|s| s.success());
        let raiz = tmp.to_str().unwrap().to_string();
        let v = Vivo::novo(&raiz, ctx(), ctl()).unwrap();
        v.passada(2);
        confere(&v, &raiz, "passada");
        assert!(clonou, "cp -c");
        assert!(v.resultado().clone_dup_files > 0 && v.resultado().link_dup_files > 0, "o teste tem clone e hardlink");

        // Arquivo novo e arquivo maior (com outro nome na pasta d): relê só a pasta dele.
        escreve(&tmp.join("a/b/novo"), 300_000);
        escreve(&tmp.join("a/f1"), 90_000);
        // A releitura devolve o que a pasta mudou (o f1 tem hardlink: fica nos repetidos, fora dela).
        let d = v.reler(&format!("{raiz}/a/b"), false);
        assert!((300_000..310_000).contains(&d), "a/b cresceu {d}");
        v.reler(&format!("{raiz}/a"), false);
        confere(&v, &raiz, "criar e crescer");

        // Pasta nova com subpasta: o aviso vem pela pasta mãe (ou pela própria, ainda desconhecida).
        fs::create_dir_all(tmp.join("d/e/g")).unwrap();
        escreve(&tmp.join("d/e/g/f5"), 70_000);
        let d = v.reler(&format!("{raiz}/d/e/g"), false);
        assert!((70_000..80_000).contains(&d), "pasta nova cresceu {d}");
        confere(&v, &raiz, "pasta nova");

        // Mover uma pasta de a/b para d: avisos nas duas mães.
        fs::rename(tmp.join("a/b/c"), tmp.join("d/c")).unwrap();
        v.reler(&format!("{raiz}/a/b"), false);
        v.reler(&format!("{raiz}/d"), false);
        confere(&v, &raiz, "mover");

        // Apagar o original do hardlink e a pasta inteira d/e.
        fs::remove_file(tmp.join("a/f1")).unwrap();
        fs::remove_dir_all(tmp.join("d/e")).unwrap();
        v.reler(&format!("{raiz}/a"), false);
        let d = v.reler(&format!("{raiz}/d/e"), false);
        assert!((-80_000..=-70_000).contains(&d), "d/e apagada {d}");
        // Sem o original, o link-f1 deixa de ser hardlink e passa a contar na pasta d.
        let d = v.reler(&format!("{raiz}/d"), false);
        assert!((90_000..100_000).contains(&d), "d ganhou o link-f1 {d}");
        confere(&v, &raiz, "apagar");

        // Relê-subpastas: o mesmo resultado.
        v.reler(&raiz, true);
        confere(&v, &raiz, "recursivo");
        fs::remove_dir_all(&tmp).unwrap();
    }
}

//! Saída em texto. Cor nunca é a única pista: todo estado também sai escrito (INCOMPLETO, ≥,
//! "não medido"). Sem crate de cor: ANSI à mão, só quando o chamador liga.

use super::i18n::Lang;
use super::{Chave, Conta, Fonte, Item, pct};
use crate::anon::DOBRADO;
use crate::detalhe::{self, Ramo, Tipo, Why};
use crate::ledger::Medida;
use crate::walk::XATTR_MAX_EMBEDDED;
use serde_json::Value;
use std::fmt::Write;

pub struct Estilo {
    pub lang: Lang,
    /// GiB em vez de GB decimal.
    pub binario: bool,
    pub cor: bool,
    pub top: usize,
}

/// Nenhuma linha passa disso: o que não cabe (fonte longa, motivo, nota) vai para a linha de baixo.
const LARG: usize = 80;
/// Coluna do rótulo (com o recuo) e do valor na conta.
const W: usize = 54;
const V: usize = 12;

impl Estilo {
    fn t(&self, k: &str) -> String {
        self.lang.t(k)
    }

    fn f(&self, k: &str, a: &[&str]) -> String {
        self.lang.f(k, a)
    }

    fn unidade(&self) -> &'static str {
        if self.binario { "GiB" } else { "GB" }
    }

    pub fn num(&self, b: i64) -> String {
        let v = if self.binario { b as f64 / (1u64 << 30) as f64 } else { b as f64 / 1e9 };
        // Medido e pequeno não pode sair igual a zero.
        if b > 0 && v < 0.005 {
            return format!("<0{}01", self.lang.dec());
        }
        format!("{v:.2}").replace('.', &self.lang.dec().to_string())
    }

    fn med(&self, m: &Medida) -> String {
        match m {
            Medida::Valor(v) => self.num(*v),
            Medida::Parcial(v, _) => format!("≥{}", self.num(*v)),
            Medida::NaoMedida(_) => self.t("nao_medido_curto"),
            Medida::NaoDisponivel => self.t("nao_disp_curto"),
        }
    }

    /// "parcial (motivo)", "não medido (motivo)"...
    fn estado(&self, m: &Medida) -> String {
        match m {
            Medida::Valor(_) => String::new(),
            Medida::Parcial(_, r) => format!("{} ({})", self.t("parcial"), self.lang.motivo(r)),
            Medida::NaoMedida(r) => format!("{} ({})", self.t("nao_medido"), self.lang.motivo(r)),
            Medida::NaoDisponivel => self.t("nao_disp"),
        }
    }

    fn pct(&self, p: f64) -> String {
        let n = format!("{p:.2}").replace('.', &self.lang.dec().to_string());
        if self.lang == Lang::Pt { format!("{n} %") } else { format!("{n}%") }
    }

    fn bytes(&self, n: u64) -> String {
        format!("{} B ({} {})", self.lang.int(n), self.num(n as i64), self.unidade())
    }

    fn negrito(&self, s: &str) -> String {
        if self.cor { format!("\x1b[1m{s}\x1b[0m") } else { s.to_string() }
    }

    fn aviso(&self, s: &str) -> String {
        if self.cor { format!("\x1b[1;33m{s}\x1b[0m") } else { s.to_string() }
    }

    fn fonte(&self, f: Fonte) -> String {
        match f {
            Fonte::Travessia => self.t("fonte_travessia"),
            Fonte::Xattr => self.t("fonte_xattr"),
            Fonte::Snapshots => self.t("fonte_snapshots"),
            Fonte::Lsof => self.t("fonte_lsof"),
            Fonte::DirStats => self.t("fonte_dirstats"),
            Fonte::VolSpaceUsed => "getattrlist".into(),
            // Só aparecem com número junto (texto::conta monta a frase); aqui fica a chave do JSON.
            Fonte::Diferenca | Fonte::DirStatsStm => f.chave().into(),
            Fonte::CapacityInUse => self.t("fonte_capacity"),
            Fonte::LogPainel => self.t("fonte_log"),
            Fonte::CacheDelete => self.t("fonte_cachedelete"),
            Fonte::AddedTime => self.t("fonte_addedtime"),
            Fonte::ApfsUserClient => self.t("fonte_apfs_uc"),
        }
    }
}

/// Corta no meio com "…" para caber em `max` caracteres.
fn cabe(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    let ini = (max - 1) / 2;
    let fim = max - 1 - ini;
    let c: Vec<char> = s.chars().collect();
    c[..ini].iter().chain(['…'].iter()).chain(c[n - fim..].iter()).collect()
}

/// Quebra por palavras em LARG: `r1` de recuo na primeira linha, `r2` nas seguintes.
fn quebra(o: &mut String, r1: usize, r2: usize, s: &str) {
    let mut l = String::new();
    let mut r = r1;
    for p in s.split(' ') {
        if !l.is_empty() && r + l.chars().count() + 1 + p.chars().count() > LARG {
            let _ = writeln!(o, "{:r$}{l}", "");
            l.clear();
            r = r2;
        } else if !l.is_empty() {
            l.push(' ');
        }
        l.push_str(p);
    }
    let _ = writeln!(o, "{:r$}{}", "", l.trim_end());
}

/// Rótulo à esquerda, valor alinhado à direita, fonte depois; fonte que estoura LARG desce.
fn linha(o: &mut String, recuo: usize, rotulo: &str, valor: &str, fonte: &str) {
    // Caminho longo corta no meio; frase longa vai inteira para a linha de cima.
    let max = W - recuo - 1;
    let r = if rotulo.starts_with(['/', '~']) || rotulo.chars().count() <= max {
        cabe(rotulo, max)
    } else {
        quebra(o, recuo, recuo + 2, rotulo);
        String::new()
    };
    let base = format!("{:recuo$}{r:<w$}{valor:>V$}", "", w = W - recuo);
    if fonte.is_empty() || base.chars().count() + 3 + fonte.chars().count() <= LARG {
        let _ = writeln!(o, "{}", format!("{base}   {fonte}").trim_end());
    } else {
        let _ = writeln!(o, "{}", base.trim_end());
        quebra(o, recuo + 4, recuo + 4, fonte);
    }
}

fn rotulo_item(e: &Estilo, i: &Item) -> String {
    match i.termo {
        Chave::Snapshots => e.f("item_snapshots", &[&i.quantidade.map_or_else(|| e.t("nao_medido_curto"), |n| n.to_string())]),
        Chave::Apagados => e.t("item_apagados"),
        Chave::Xattr => e.t("item_xattr"),
        Chave::Metadados => e.t("item_metadados"),
        Chave::Arvore | Chave::Negadas => match i.nome.strip_suffix(DOBRADO) {
            Some(pai) => format!("{pai}/{}", e.t("balde_demais")),
            None => i.nome.clone(),
        },
    }
}

fn fonte_item(e: &Estilo, i: &Item) -> String {
    match &i.processo {
        Some((p, b)) => e.f("fonte_lsof_proc", &[p, &e.num(*b)]),
        None => e.fonte(i.termo.fonte()),
    }
}

pub fn conta(c: &Conta, e: &Estilo) -> String {
    let mut o = String::new();
    let unidade = e.t(if e.binario { "unidade_gib" } else { "unidade_gb" });
    let macos = c.macos.as_deref().map_or(String::new(), |m| format!(" · macOS {m}"));
    let _ = writeln!(o, "{} · {}{macos}", c.ferramenta, c.data_hora);
    let _ = writeln!(o, "{unidade}");

    // ---- INCOMPLETO ----
    let inc = c.incompleto();
    if !inc.is_empty() {
        let motivos: Vec<String> = inc.iter().map(|m| e.t(&format!("motivo_{m}"))).collect();
        let _ = writeln!(o, "{}", e.aviso(&format!("{} ({})", e.t("incompleto"), motivos.join(", "))));
        if c.interrompida {
            quebra(&mut o, 2, 2, &e.t("interrompido_nota"));
        }
        if !c.root {
            quebra(&mut o, 2, 4, &format!("- {}", e.t("falta_lsof")));
            if c.negadas > 0 {
                quebra(&mut o, 2, 4, &format!("- {}", e.lang.p("falta_negadas", c.negadas as u64)));
            }
            let _ = writeln!(o, "  {}", e.t("rode_sudo"));
        }
        if c.fda == Some(false) {
            let mut t = String::new();
            quebra(&mut t, 2, 2, &e.lang.p("conceda_fda", c.negadas as u64));
            let _ = write!(o, "{}", e.aviso(&t));
        }
    }

    // ---- Tela do Ajustes ----
    let _ = writeln!(o);
    let cab = |o: &mut String, titulo: &str, valor: &str, fonte: &str| {
        let mut t = String::new();
        linha(&mut t, 0, titulo, valor, fonte);
        for (i, l) in t.lines().enumerate() {
            let _ = writeln!(o, "{}", if i == 0 { e.negrito(l) } else { l.to_string() });
        }
    };
    cab(&mut o, &e.t("tela"), e.unidade(), &e.t("fonte"));
    let motivo_ou = |m: &Medida, f: String| match m {
        Medida::NaoMedida(r) => e.lang.motivo(r),
        _ => f,
    };
    linha(&mut o, 2, &e.t("tela_macos"), &e.med(&c.tela.macos), &motivo_ou(&c.tela.macos, e.fonte(Fonte::CapacityInUse)));
    let vols = volumes_do_macos(&c.volumes, e);
    if !vols.is_empty() {
        quebra(&mut o, 4, 4, &e.f("macos_volumes", &[&vols.join(" · ")]));
    }
    let ai_f = match &c.tela.ai {
        Medida::NaoDisponivel => e.t("nao_disp"),
        m => motivo_ou(m, e.f("fonte_ai", &[&e.med(&c.tela.ai_fisico)])),
    };
    linha(&mut o, 2, &e.t("tela_ai"), &e.med(&c.tela.ai), &ai_f);
    match &c.tela.dados_do_sistema {
        Ok(Some((b, ts))) => linha(&mut o, 2, &e.f("tela_ds", &[ts]), &e.num(*b), &e.fonte(Fonte::LogPainel)),
        Ok(None) => {
            linha(&mut o, 2, &e.t("tela_ds_sem"), &e.t("sem_registro"), &e.fonte(Fonte::LogPainel));
            quebra(&mut o, 4, 4, &e.t("tela_ds_instrucao"));
        }
        Err(r) => linha(&mut o, 2, &e.t("tela_ds_sem"), &e.t("nao_medido_curto"), &e.lang.motivo(r)),
    }

    // ---- Volume Data ----
    let _ = writeln!(o);
    cab(&mut o, &e.t("volume_data"), &e.med(&c.usado), &motivo_ou(&c.usado, e.fonte(Fonte::VolSpaceUsed)));
    // Top N caminhos; snapshots, apagados e xattr sempre aparecem (no "demais" sumiriam).
    let caminhos = c.ranking.iter().filter(|i| !i.nome.is_empty()).count();
    let mut vistos = 0;
    let (mostrar, resto_rank): (Vec<&Item>, Vec<&Item>) = c.ranking.iter().partition(|i| {
        vistos += usize::from(!i.nome.is_empty());
        i.nome.is_empty() || vistos <= e.top
    });
    let n = e.top.min(caminhos);
    let _ = writeln!(o, "  {}", e.lang.pf("mais_pesa", caminhos as u64, &[&e.lang.int(n as u64), &e.lang.int(caminhos as u64)]));
    for i in mostrar {
        let v = if i.parcial { format!("≥{}", e.num(i.bytes)) } else { e.num(i.bytes) };
        linha(&mut o, 4, &rotulo_item(e, i), &v, &fonte_item(e, i));
    }
    if !resto_rank.is_empty() {
        let soma: i64 = resto_rank.iter().map(|i| i.bytes).sum();
        let mut fontes: Vec<String> = Vec::new();
        for i in &resto_rank {
            let f = e.fonte(i.termo.fonte());
            if !fontes.contains(&f) {
                fontes.push(f);
            }
        }
        let piso = resto_rank.iter().any(|i| i.parcial);
        let v = if piso { format!("≥{}", e.num(soma)) } else { e.num(soma) };
        linha(&mut o, 4, &e.lang.p("demais", resto_rank.len() as u64), &v, &fontes.join(", "));
    }
    let faltas: Vec<_> = c.termos.iter().filter(|t| !matches!(t.1, Medida::Valor(_))).collect();
    if !faltas.is_empty() {
        let _ = writeln!(o, "  {}", e.aviso(&e.t("estado_titulo")));
        for (k, m) in faltas {
            quebra(&mut o, 4, 6, &format!("{}: {}", e.t(&format!("termo_{}", k.json())), e.estado(m)));
        }
    }
    let completo = c.termos.iter().all(|t| matches!(t.1, Medida::Valor(_)));
    let (rv, rf) = match (c.sem_explicacao(), c.usado.valor()) {
        _ if c.sobreposicao().is_some() => (e.t("nao_medido_curto"), e.f("fonte_sobreposicao", &[&format!("{} {}", e.num(c.sobreposicao().unwrap_or(0)), e.unidade())])),
        (Some(r), Some(u)) => {
            let f = match pct(r, u) {
                Some(p) => e.f(if completo { "fonte_diferenca" } else { "fonte_diferenca_teto" }, &[&e.pct(p)]),
                None => e.fonte(Fonte::Diferenca),
            };
            (e.num(r), f)
        }
        _ => (e.t("nao_medido_curto"), e.estado(&c.usado)),
    };
    linha(&mut o, 2, &e.t("sem_explicacao"), &rv, &rf);

    // ---- Informativo ----
    let _ = writeln!(o);
    let _ = writeln!(o, "{}", e.negrito(&e.t("informativo")));
    linha(&mut o, 2, &e.t("purgeable"), &e.med(&c.purgeable), &motivo_ou(&c.purgeable, e.fonte(Fonte::CacheDelete)));
    for (s, b) in c.purgeable_top.iter().take(3) {
        linha(&mut o, 4, s, &e.num(*b), "");
    }
    if let Some(r) = &c.repetidos {
        let tr = e.fonte(Fonte::Travessia);
        // Travessia interrompida: tudo aqui é piso.
        let ge = if r.parcial { "≥" } else { "" };
        let arqs = |n: u64| format!("{ge}{}", e.lang.p("n_arquivos", n));
        let gb = |b: i64| format!("{ge}{}", e.num(b));
        linha(&mut o, 2, &e.f("clones", &[&arqs(r.clones.0)]), &gb(r.clones.1), &tr);
        linha(&mut o, 2, &e.f("hardlinks", &[&arqs(r.hardlinks.0)]), &gb(r.hardlinks.1), &tr);
        linha(&mut o, 2, &e.f("enxertos", &[&format!("{ge}{}", e.lang.int(r.enxertos))]), "", &tr);
        let pastas = format!("{ge}{}", e.lang.p("n_pastas", r.dataless.2));
        linha(&mut o, 2, &e.f("dataless", &[&arqs(r.dataless.0), &pastas]), &gb(r.dataless.1), &e.t("fonte_dataless"));
    }
    if let Some(m) = &c.apfs_meta {
        let f = match &m.medida {
            Medida::NaoMedida(r) => e.lang.motivo(r),
            _ => e.fonte(Fonte::ApfsUserClient),
        };
        linha(&mut o, 2, &e.t("apfs_meta"), &e.med(&m.medida), &f);
        if let Some(d) = &m.detalhe {
            quebra(&mut o, 4, 4, &e.f("apfs_meta_nota", &[&d.bloco.to_string()]));
        }
    }
    match &c.lixo {
        Some(Ok(l)) => {
            let rot = e.f("trash_old", &[&e.lang.int(l.velhos_itens), &e.lang.int(l.itens)]);
            linha(&mut o, 2, &rot, &e.med(&l.medida_velhos()), &e.fonte(Fonte::AddedTime));
            let piso = if l.piso { "≥" } else { "" };
            quebra(&mut o, 4, 4, &e.f("trash_note", &[&format!("{piso}{}", e.num(l.bytes)), e.unidade()]));
            if l.sem_data_itens > 0 {
                quebra(&mut o, 4, 4, &e.f("trash_undated", &[&e.lang.int(l.sem_data_itens), &format!("{} {}", e.num(l.sem_data_bytes), e.unidade())]));
            }
        }
        Some(Err(r)) => linha(&mut o, 2, &e.t("trash_old_none"), &e.t("nao_medido_curto"), &e.lang.motivo(r)),
        None => {}
    }
    let voltou = crate::voltou::causas(c);
    if !voltou.is_empty() {
        let _ = writeln!(o);
        let _ = writeln!(o, "{}", e.negrito(&e.t("nao_voltou")));
        let i = usize::from(e.lang == Lang::En);
        for x in &voltou {
            let Some(tx) = crate::voltou::texto(x.id) else { continue };
            let n = match (x.quantidade, x.id) {
                (Some(n), "local_snapshot") => format!(" ({n})"),
                (Some(n), _) => format!(" ({})", e.lang.p("n_arquivos", n)),
                _ => String::new(),
            };
            let v = if x.piso { format!("≥{}", e.num(x.bytes)) } else { e.num(x.bytes) };
            let (titulo, texto) = if i == 0 { (tx[0].0, tx[1].0) } else { (tx[0].1, tx[1].1) };
            linha(&mut o, 2, &format!("{titulo}{n}"), &v, &e.fonte(x.fonte));
            quebra(&mut o, 4, 4, texto);
        }
    }
    if !c.erros.is_empty() {
        let etapas: Vec<&str> = c.erros.iter().map(|(s, _)| s.as_str()).collect();
        let _ = writeln!(o);
        quebra(&mut o, 0, 2, &format!("{} [{}]", e.lang.p("erros_etapas", c.erros.len() as u64), etapas.join(", ")));
    }
    o
}

/// Volumes que o Ajustes soma na linha macOS (fora de Dados do Sistema), somados por papel, na
/// ordem em que aparecem: "Sistema 12,65", "Preboot 10,82"... Vazio sem o JSON do diskutil.
fn volumes_do_macos(v: &Value, e: &Estilo) -> Vec<String> {
    let mut por_papel: Vec<(String, Option<i64>)> = Vec::new();
    for vol in v.as_array().into_iter().flatten().filter(|c| c["tipo"] == "interno").flat_map(|c| c["volumes"].as_array().into_iter().flatten()) {
        if vol["no_macos_da_tela"] != true {
            continue;
        }
        let papel = match vol["roles"].get(0).and_then(Value::as_str) {
            Some("System") => e.t("papel_system"),
            Some(r) => r.to_string(),
            None => e.t("papel_sem"),
        };
        let b = vol["capacity_in_use"].as_i64();
        match por_papel.iter_mut().find(|x| x.0 == papel) {
            Some(x) => x.1 = x.1.zip(b).map(|(a, b)| a + b),
            None => por_papel.push((papel, b)),
        }
    }
    // Ordem fixa (a do Ajustes), não a dos containers; papel fora da lista vai depois, na ordem em que veio.
    let ordem = |p: &str| ["Sistema", "System", "Preboot", "Recovery", "Update", "VM"].iter().position(|x| *x == p).unwrap_or(99);
    por_papel.sort_by_key(|x| ordem(&x.0));
    por_papel.into_iter().map(|(p, b)| format!("{p} {}", b.map_or_else(|| e.t("nao_medido_curto"), |b| e.num(b)))).collect()
}

/// Troca o prefixo da home por ~.
pub fn til(p: &str, home: &str) -> String {
    match p.strip_prefix(home) {
        Some(r) if !home.is_empty() && (r.is_empty() || r.starts_with('/')) => format!("~{r}"),
        _ => p.to_string(),
    }
}

/// Colunas do drill: nome (com recuo), alocado, "libera se apagar", fonte (36 + 10 + 3 + 16 + 3 + 9 = 77).
const WR: usize = 36;

pub fn ramo(r: &Ramo, home: &str, e: &Estilo) -> String {
    let mut o = String::new();
    let mut t = String::new();
    quebra(&mut t, 0, 2, &e.f("ramo", &[&til(&r.raiz, home), &r.profundidade.to_string()]));
    for l in t.lines() {
        let _ = writeln!(o, "{}", e.negrito(l));
    }
    if r.interrompida {
        let _ = writeln!(o, "{}", e.aviso(&format!("{} ({})", e.t("incompleto"), e.t("motivo_interrupted"))));
    }
    let fmt = |rot: &str, a: &str, p: &str, f: &str| format!("  {:<w$}{a:>10}   {p:>16}   {f}", cabe(rot, WR - 3), w = WR - 2).trim_end().to_string();
    let _ = writeln!(o, "{}", e.negrito(&fmt("", e.unidade(), &e.t("col_privado"), &e.t("fonte"))));
    let n = e.top.min(r.filhos.len());
    for f in &r.filhos[..n] {
        let nome = if f.nome.is_empty() { e.t("soltos") } else { f.nome.clone() };
        let p = f.privado.map_or_else(|| e.t("nao_medido_curto"), |p| e.num(p));
        let fonte = e.fonte(if f.protegida { Fonte::DirStats } else { Fonte::Travessia });
        let _ = writeln!(o, "{}", fmt(&nome, &e.num(f.alocado), &p, &fonte));
    }
    let resto = &r.filhos[n..];
    if !resto.is_empty() {
        let a: i64 = resto.iter().map(|f| f.alocado).sum();
        let p: i64 = resto.iter().filter_map(|f| f.privado).sum();
        let sem_p = resto.iter().any(|f| f.privado.is_none());
        let p = if sem_p { format!("≥{}", e.num(p)) } else { e.num(p) };
        let _ = writeln!(o, "{}", fmt(&e.lang.p("demais", resto.len() as u64), &e.num(a), &p, ""));
    }
    let prot: i64 = r.filhos.iter().filter(|f| f.protegida).map(|f| f.alocado).sum();
    // Piso como o `total.lower_bound` do JSON: leitura parada, pasta protegida sem dir-stats ou
    // entrada que não foi lida.
    let piso = r.interrompida || r.protegidas_sem > 0 || r.nao_lidos > 0;
    let a = e.num(r.total.alloc + prot);
    let a = if piso { format!("≥{a}") } else { a };
    let _ = writeln!(o, "{}", fmt(&e.t("ramo_total"), &a, &format!("≥{}", e.num(r.total.private)), ""));
    let _ = writeln!(o, "    {}, {}", e.lang.p("n_arquivos", r.total.files), e.lang.p("n_pastas", r.dirs));
    if r.protegidas_sem > 0 {
        quebra(&mut o, 2, 4, &e.lang.p("ramo_negada_sem", r.protegidas_sem));
    }
    if r.erros > 0 {
        quebra(&mut o, 2, 4, &e.lang.p("ramo_erros", r.erros));
    }
    if r.outros_volumes > 0 {
        quebra(&mut o, 2, 4, &e.lang.p("ramo_outro_volume", r.outros_volumes));
    }
    quebra(&mut o, 0, 0, &e.t("ramo_nota_privado"));
    o
}

pub fn why(w: &Why, home: &str, e: &Estilo) -> String {
    let mut o = String::new();
    let _ = writeln!(o, "{}", e.negrito(&til(&w.caminho, home)));
    let kv = |o: &mut String, k: &str, v: String| quebra(o, 2, 32, &format!("{:<30}{v}", e.t(k)));
    // Pasta: os tamanhos da própria entrada leem-se como o da pasta (e são ~0); o conteúdo é dir-stats.
    let pasta = w.tipo == Tipo::Pasta;
    let tam_pasta = || e.f("why_pasta_tamanho", &[&til(&w.caminho, home)]);
    let opt = |v: Option<u64>| v.map_or_else(|| e.t("ausente"), |v| e.bytes(v));
    let errno = |x: i32| e.f("erro_errno", &[&x.to_string()]);
    let sim = |b: bool| e.t(if b { "sim" } else { "nao" });
    kv(
        &mut o,
        "why_tipo",
        e.t(match w.tipo {
            Tipo::Arquivo => "why_arquivo",
            Tipo::Pasta => "why_pasta",
            Tipo::Link => "why_link",
            Tipo::Outro => "why_outro",
        }),
    );
    let (tam, blocos, nlink, dev, stf) = w.st;
    match &w.attrs {
        Ok(a) => {
            if pasta {
                kv(&mut o, "why_tamanho", tam_pasta());
            } else {
                kv(&mut o, "why_logico", if w.tipo == Tipo::Arquivo { opt(a.logico) } else { e.bytes(tam) });
                kv(&mut o, "why_alocado", if w.tipo == Tipo::Arquivo { opt(a.alocado) } else { e.bytes(blocos) });
                kv(&mut o, "why_privado", opt(a.privado));
            }
            let id = |v: Option<u64>| v.map_or_else(|| e.t("ausente"), |v| v.to_string());
            kv(&mut o, "why_ids", format!("{} / {}", id(a.fileid), id(a.cloneid)));
            kv(&mut o, "why_ext", a.ext_flags.map_or_else(|| e.t("ausente"), |f| format!("0x{f:x} ({})", detalhe::ext_flags_nomes(f).join(", "))));
            kv(&mut o, "why_links", a.links.map_or(nlink, u64::from).to_string());
            kv(&mut o, "why_dev", a.devid.map_or(dev, u64::from).to_string());
        }
        Err(m) => {
            if pasta {
                kv(&mut o, "why_tamanho", tam_pasta());
            } else {
                kv(&mut o, "why_logico", e.bytes(tam));
                kv(&mut o, "why_alocado", e.bytes(blocos));
                kv(&mut o, "why_privado", format!("{} ({})", e.t("nao_medido"), e.lang.motivo(m)));
            }
            kv(&mut o, "why_links", nlink.to_string());
            kv(&mut o, "why_dev", dev.to_string());
        }
    }
    kv(&mut o, "why_flags", format!("0x{stf:x} ({})", detalhe::flags_nomes(stf).join(", ")));
    kv(&mut o, "why_dataless", sim(stf & crate::parse::SF_DATALESS != 0));
    kv(&mut o, "why_comprimido", sim(stf & 0x20 != 0));
    kv(
        &mut o,
        "why_xattrs",
        match &w.xattrs {
            Ok(v) => {
                let fluxo: i64 = v.iter().filter(|x| **x > XATTR_MAX_EMBEDDED).sum();
                e.f("why_xattrs_val", &[&v.len().to_string(), &e.lang.int(v.iter().sum::<i64>() as u64), &e.lang.int(fluxo as u64)])
            }
            Err(x) => errno(*x),
        },
    );
    if pasta {
        kv(
            &mut o,
            "why_enxerto",
            match w.enxerto {
                None => "-".into(),
                Some(Ok(b)) => sim(b),
                Some(Err(x)) => errno(x),
            },
        );
        kv(
            &mut o,
            "why_dirstats",
            match w.dirstats {
                Some(Ok(v)) => e.bytes(v),
                Some(Err(x)) => errno(x),
                None => "-".into(),
            },
        );
    }
    quebra(&mut o, 0, 0, &e.t("why_nota"));
    if let Ok(a) = &w.attrs
        && !pasta
        && a.privado.zip(a.alocado).is_some_and(|(p, al)| p < al)
    {
        quebra(&mut o, 0, 0, &e.t("ramo_nota_privado"));
    }
    o
}

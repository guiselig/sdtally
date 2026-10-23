//! "O que fazer" (spec da fase 5, §4.4 e §6): recomendações só a partir do que foi medido.
//! Pura: nada aqui apaga, roda ou lê disco. O `text` sai em inglês; o app traduz pela ficha (`card`).

use crate::anon::DOBRADO;
use crate::catalogo;
use crate::ledger::Medida;
use crate::report::{self, Chave, Conta};
use serde_json::{Value, json};

const GB: i64 = 1_000_000_000;

/// Os únicos comandos que podem sair em `command` (o app tem a mesma lista). Nenhum leva caminho
/// medido; a ferramenta oficial confere o estado na hora em que roda.
pub const COMANDOS: [&str; 8] = [
    "brew cleanup --dry-run",
    "npm cache clean --force",
    "go clean -cache",
    "pip cache purge",
    "xcrun simctl delete unavailable",
    "xcrun simctl runtime list",
    "ollama list",
    "tmutil listlocalsnapshots /",
];

/// O que rodar antes, só para ver, em cada comando de `COMANDOS` (PLANO Fase 3, "dry-run
/// conferido"): `leitura` é a simulação ou a listagem que não muda nada; igual a `comando` quando
/// ele próprio só lê. `fonte` é onde isso está documentado (ajuda do próprio programa, conferida
/// em 18/10/2026, salvo os dois do simctl, que não responderam nesta conferência: só a ajuda dele). O teste `tests/dryrun.rs` barra comando que entre em `COMANDOS`
/// sem linha aqui. Nenhum leva caminho medido.
pub struct Simulacao {
    pub comando: &'static str,
    pub leitura: &'static str,
    pub fonte: &'static str,
}

pub const SIMULACOES: [Simulacao; 8] = [
    Simulacao { comando: "brew cleanup --dry-run", leitura: "brew cleanup --dry-run", fonte: "brew cleanup --help: -n, --dry-run mostra o que sairia, sem remover" },
    Simulacao { comando: "npm cache clean --force", leitura: "npm cache ls", fonte: "npm cache --help: ls lista as entradas do cache (o clean não tem simulação)" },
    Simulacao { comando: "go clean -cache", leitura: "go clean -cache -n", fonte: "go help clean: -n imprime os comandos de remoção sem rodá-los" },
    Simulacao { comando: "pip cache purge", leitura: "pip cache info", fonte: "pip cache --help: info mostra informações do cache (o purge não tem simulação)" },
    Simulacao { comando: "xcrun simctl delete unavailable", leitura: "xcrun simctl list devices", fonte: "xcrun simctl help list: lista os simuladores; os sem runtime saem como unavailable" },
    Simulacao { comando: "xcrun simctl runtime list", leitura: "xcrun simctl runtime list", fonte: "xcrun simctl runtime help: list só lista os runtimes" },
    Simulacao { comando: "ollama list", leitura: "ollama list", fonte: "ollama list --help: lista os modelos" },
    Simulacao { comando: "tmutil listlocalsnapshots /", leitura: "tmutil listlocalsnapshots /", fonte: "tmutil help listlocalsnapshots: só lista os snapshots do ponto de montagem" },
];

pub fn simulacao(comando: &str) -> Option<&'static Simulacao> {
    SIMULACOES.iter().find(|s| s.comando == comando)
}

const PEDIR: &str = " Ask the user before running.";

/// Regra sobre as linhas do ranking. Uma linha casa se o caminho for igual a um prefixo ou estiver
/// dentro dele, por componente inteiro; `*` só no último componente do prefixo.
pub struct Regra {
    pub id: &'static str,
    pub nivel: &'static str,
    pub prefixos: &'static [&'static str],
    /// Também toda linha de `protected_folders`, em qualquer caminho.
    pub protegidas: bool,
    pub comando: Option<&'static str>,
    /// Mínimo de Σ `private_bytes` (ou `bytes`, quando algum privado é null).
    pub limite: i64,
    /// Id em `catalogo::CATALOGO`; o `text` sai do `passo.en` dela.
    pub ficha: &'static str,
    /// [min, max) do `macos_version`, numérico por componente; fora da faixa: não casa.
    pub versoes: Option<(&'static str, &'static str)>,
    /// Limite contra Σ `bytes` (Ocupa) em vez de Σ privado.
    pub por_ocupa: bool,
    /// `kind: "signature"`, com `evidence` genérica.
    pub assinatura: bool,
}

const fn r(id: &'static str, nivel: &'static str, prefixos: &'static [&'static str], comando: Option<&'static str>) -> Regra {
    let limite = if matches!(nivel.as_bytes(), b"do_not_touch") { 5 * GB } else { GB };
    Regra { id, nivel, prefixos, protegidas: false, comando, limite, ficha: id, versoes: None, por_ocupa: false, assinatura: false }
}

/// Recurso baixado pelo macOS (`/System/Library/AssetsV2`).
macro_rules! asset {
    ($x:literal) => {
        concat!("/System/Library/AssetsV2/com_apple_MobileAsset_", $x)
    };
}

/// Na ordem do §6. Cada linha entra só na primeira regra que casa (o Cinebench vem antes da
/// anomalia de Preferences).
pub const REGRAS: &[Regra] = &[
    r("trash", "safe", &["~/.Trash"], None),
    r("logs", "safe", &["~/Library/Logs"], None),
    r("homebrew_cache", "safe", &["~/Library/Caches/Homebrew"], Some("brew cleanup --dry-run")),
    r("npm_cache", "safe", &["~/.npm/_cacache"], Some("npm cache clean --force")),
    r("go_build_cache", "safe", &["~/Library/Caches/go-build"], Some("go clean -cache")),
    r("pip_cache", "safe", &["~/Library/Caches/pip"], Some("pip cache purge")),
    r("updater_caches", "safe", &["~/Library/Caches/*.ShipIt", "~/Library/Caches/*-updater", "~/Library/Caches/node-gyp"], None),
    r("xcode_derived_data", "cost", &["~/Library/Developer/Xcode/DerivedData"], None),
    r("simulators", "cost", &["~/Library/Developer/CoreSimulator/Devices"], Some("xcrun simctl delete unavailable")),
    r("simulator_runtimes", "cost", &["/Library/Developer/CoreSimulator"], Some("xcrun simctl runtime list")),
    r("playwright_browsers", "cost", &["~/Library/Caches/ms-playwright"], None),
    r("ollama_models", "cost", &["~/.ollama/models"], Some("ollama list")),
    r("cinebench_assets", "cost", &["~/Library/Preferences/Maxon/Cinebench_*"], None),
    r("cloudkit_cache", "check_first", &["~/Library/Caches/CloudKit"], None),
    r("claude_vm_bundles", "check_first", &["~/Library/Application Support/Claude/vm_bundles"], None),
    r("preferences_anomaly", "anomaly", &["~/Library/Preferences/*"], None),
    r("icloud_drive", "do_not_touch", &["~/Library/Mobile Documents/*"], None),
    r("cloud_storage", "do_not_touch", &["~/Library/CloudStorage/*"], None),
    // Fase 6 (§6.2): antes de system_folders, que fica com o resto de /System.
    Regra { por_ocupa: true, ..r("asset_voices", "cost", &[asset!("MacinTalkVoiceAssets"), asset!("VoiceServices_*"), asset!("TTSAXResourceModelAssets"), asset!("Trial_Siri_SiriTextToSpeech")], None) },
    Regra { por_ocupa: true, ..r("asset_xcode", "cost", &[asset!("AppleDeveloperDocumentation"), asset!("MetalToolchain")], None) },
    Regra { por_ocupa: true, ..r("asset_sim_runtime", "cost", &[asset!("*SimulatorRuntime")], Some("xcrun simctl runtime list")) },
    Regra { por_ocupa: true, ..r("asset_os_update", "system_handles", &[asset!("MacSoftwareUpdate"), asset!("MacSplatSoftwareUpdate"), asset!("*UpdateBrain"), asset!("SFRSoftwareUpdate")], None) },
    Regra { limite: 20 * GB, assinatura: true, ..r("spotlight_index_large", "check_first", &["/.Spotlight-V100", "~/Library/Metadata/CoreSpotlight"], None) },
    Regra { assinatura: true, ..r("failed_smb_mount", "anomaly", &["/Volumes"], None) },
    Regra { protegidas: true, ..r("system_folders", "do_not_touch", &["/System", "/private/var/db", "/private/var/vm"], None) },
];

/// Um número que fez a assinatura disparar, com o limite e a fonte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidencia {
    pub chave: String,
    pub valor: i64,
    pub limite: i64,
    pub fonte: &'static str,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Conselho {
    pub id: &'static str,
    pub nivel: &'static str,
    pub bytes: i64,
    pub privado: Option<i64>,
    pub caminhos: Vec<String>,
    pub termo: Option<&'static str>,
    pub processo: Option<String>,
    pub comando: Option<&'static str>,
    pub texto: String,
    /// Some: `kind: "signature"` com esta `evidence`; None: `rule`.
    pub evidencia: Option<Vec<Evidencia>>,
    /// `bytes` lógicos, fora de qualquer soma (`bytes_logical: true`).
    pub logico: bool,
    /// `bytes` é um piso, não o total (`bytes_floor: true`).
    pub piso: bool,
}

fn item(id: &'static str, nivel: &'static str, bytes: i64, texto: String) -> Conselho {
    Conselho { id, nivel, bytes, texto, ..Default::default() }
}

fn gb_en(b: i64) -> String {
    format!("{:.1} GB", b as f64 / 1e9)
}

fn versao(v: &str) -> Vec<u64> {
    v.split('.').map(|x| x.parse().unwrap_or(0)).collect()
}

/// `v` em [min, max), numérico por componente: 15.1.1 está em [15.1, 15.2).
pub fn na_faixa(v: &str, (min, max): (&str, &str)) -> bool {
    let v = versao(v);
    v >= versao(min) && v < versao(max)
}

/// Componente do prefixo contra o do caminho; um `*` vale por qualquer trecho não vazio ou não.
fn glob(padrao: &str, nome: &str) -> bool {
    match padrao.split_once('*') {
        Some((a, z)) => nome.len() >= a.len() + z.len() && nome.starts_with(a) && nome.ends_with(z),
        None => padrao == nome,
    }
}

/// `caminho` igual ao `prefixo` ou dentro dele, por componente inteiro. Ancestral nunca casa.
pub fn casa(prefixo: &str, caminho: &str) -> bool {
    let p: Vec<&str> = prefixo.split('/').collect();
    let c: Vec<&str> = caminho.split('/').collect();
    c.len() >= p.len() && p.iter().zip(&c).enumerate().all(|(i, (a, b))| if i + 1 == p.len() { glob(a, b) } else { a == b })
}

fn medido(m: Option<&Medida>) -> Option<i64> {
    m.and_then(Medida::valor)
}

pub fn conselhos(c: &Conta) -> Vec<Conselho> {
    let mut v = Vec::new();
    let termo_ok = |k: Chave| medido(c.medida(k)).is_some();

    // ---- medição ----
    if c.fda == Some(false) {
        v.push(item("grant_full_disk_access", "measurement", c.sem_explicacao().unwrap_or(0), "Grant Full Disk Access to sdtally (or the app) and measure again.".into()));
    }
    if !c.root
        && let Some(r) = c.sem_explicacao()
        && let Some(p) = c.usado.valor().and_then(|u| report::pct(r, u))
        && p > 2.0
    {
        v.push(item("measure_as_admin", "measurement", r, format!("Measure as administrator: {p}% of used space is unexplained.")));
    }

    // ---- termos fora da árvore ----
    if termo_ok(Chave::Snapshots) {
        for (id, tipo, nivel, comando, texto) in [
            ("time_machine_snapshots", "time_machine", "system_handles", Some("tmutil listlocalsnapshots /"), "Nothing to do: macOS deletes local Time Machine snapshots when space runs low. To list them: tmutil listlocalsnapshots /"),
            ("other_snapshots", "other", "check_first", None, "Snapshot from another app (backup or cloning); manage it in the app that created it."),
        ] {
            let s: i64 = c.snapshots.iter().flatten().filter(|x| x.0 == tipo).filter_map(|x| x.2).sum();
            if s >= GB {
                let t = if comando.is_some() { format!("{texto}{PEDIR}") } else { texto.into() };
                v.push(Conselho { privado: Some(s), termo: Some(Chave::Snapshots.json()), comando, ..item(id, nivel, s, t) });
            }
        }
    }
    if let Some(b) = c.purgeable.valor().filter(|b| *b >= 5 * GB) {
        v.push(item("purgeable_auto", "system_handles", b, "Nothing to do: macOS frees purgeable space when it needs it.".into()));
    }
    let apagados = medido(c.medida(Chave::Apagados)).filter(|b| *b >= GB).map(|b| {
        let p = c.ranking.iter().find(|i| i.termo == Chave::Apagados).and_then(|i| i.processo.as_ref()).map(|x| x.0.clone());
        let t = match &p {
            Some(n) => format!("Quit and reopen {n}, or restart the Mac."),
            None => "Restarting the Mac frees this space.".into(),
        };
        Conselho { termo: Some(Chave::Apagados.json()), processo: p, ..item("deleted_open", "safe", b, t) }
    });
    let swap = c.swap.filter(|s| s.0 >= 8 * GB as u64).map(|(t, _)| item("swap_large", "system_handles", t as i64, "The Mac is using the disk as memory; quitting heavy apps or restarting gives it back.".into()));
    match (apagados, swap) {
        // As duas juntas viram uma só (§4.2).
        (Some(a), Some(s)) => {
            let t = format!("Restarting gives back {} of swap (it leaves the macOS line in Settings) and {} of deleted files apps still hold (it leaves System Data). Save open work first. Swap comes back with use.", gb_en(s.bytes), gb_en(a.bytes));
            v.push(Conselho { processo: a.processo, ..item("restart_worth", "safe", s.bytes + a.bytes, t) });
        }
        (a, s) => v.extend(a.into_iter().chain(s)),
    }
    if let Some(b) = c.tela.ai_fisico.valor().filter(|b| *b >= 5 * GB) {
        v.push(item("apple_intelligence_models", "system_handles", b, "Apple Intelligence models; macOS manages them. Do not delete the folder.".into()));
    }

    // ---- cópias idênticas (só a medição completa compara conteúdo) ----
    if let Some(d) = c.duplicados.as_ref().filter(|d| d.desperdicio() >= GB) {
        let w = d.desperdicio();
        let texto = match d.grupos.as_slice() {
            [g] => format!("{} identical copies of a {} file take {} more.", g.caminhos.len(), gb_en(g.bytes_each), gb_en(w)),
            gs => format!("{} files are identical copies of others and take {} more.", gs.iter().map(|g| g.caminhos.len() - 1).sum::<usize>(), gb_en(w)),
        };
        let caminhos = d.grupos.iter().take(report::LIMITE_GRUPOS).flat_map(|g| &g.caminhos).map(|p| report::caminho_json(p)).collect();
        let texto = format!("{texto} Keep one of each; before removing the others, check which copy your apps use.");
        v.push(Conselho { caminhos, termo: Some(Chave::Arvore.json()), piso: d.parcial(), ..item("duplicate_files", "check_first", w, texto) });
    }

    // ---- linhas do ranking ----
    let mut grupos: Vec<Vec<&report::Item>> = REGRAS.iter().map(|_| Vec::new()).collect();
    let ativa: Vec<bool> = REGRAS.iter().map(|r| r.versoes.is_none_or(|f| c.macos.as_deref().is_some_and(|v| na_faixa(v, f)))).collect();
    for i in c.ranking.iter().filter(|i| !i.nome.is_empty() && termo_ok(i.termo)) {
        let p = i.nome.strip_suffix(DOBRADO).unwrap_or(&i.nome);
        if let Some(n) = REGRAS.iter().zip(&ativa).position(|(r, a)| *a && (r.prefixos.iter().any(|x| casa(x, p)) || (r.protegidas && i.termo == Chave::Negadas))) {
            grupos[n].push(i);
        }
    }
    for (r, g) in REGRAS.iter().zip(grupos) {
        let bytes: i64 = g.iter().map(|i| i.bytes).sum();
        let privado: Option<i64> = g.iter().map(|i| i.privado).sum();
        let (chave, valor) = match privado {
            Some(p) if !r.por_ocupa => ("private_bytes", p),
            _ => ("bytes", bytes),
        };
        if g.is_empty() || valor < r.limite {
            continue;
        }
        let mut caminhos: Vec<String> = Vec::new();
        for i in &g {
            let p = i.nome.strip_suffix(DOBRADO).unwrap_or(&i.nome).to_string();
            if !caminhos.contains(&p) {
                caminhos.push(p);
            }
        }
        let termo = g.iter().all(|i| i.termo == g[0].termo).then(|| g[0].termo.json());
        let passo = catalogo::ficha(r.ficha).map_or("", |f| f.passo.en);
        let texto = if r.comando.is_some() { format!("{passo}{PEDIR}") } else { passo.into() };
        let evidencia = r.assinatura.then(|| vec![Evidencia { chave: chave.into(), valor, limite: r.limite, fonte: g[0].termo.fonte().chave() }]);
        v.push(Conselho { privado, caminhos, termo, comando: r.comando, evidencia, ..item(r.id, r.nivel, bytes, texto) });
    }
    v
}

pub fn json(v: &[Conselho]) -> Value {
    json!(v.iter().map(|x| {
        let mut j = json!({
            "id": x.id,
            "kind": if x.evidencia.is_some() { "signature" } else { "rule" },
            "card": catalogo::ficha(x.id).map(|f| f.id),
            "level": x.nivel,
            "bytes": x.bytes,
            "private_bytes": x.privado,
            "paths": x.caminhos,
            "term": x.termo,
            "process": x.processo,
            "command": x.comando,
            "dry_run": x.comando.and_then(simulacao).map(|s| json!({"command": s.leitura, "doc": s.fonte})),
            "text": x.texto,
        });
        if let Some(ev) = &x.evidencia {
            j["evidence"] = json!(ev.iter().map(|e| json!({"key": e.chave, "value": e.valor, "limit": e.limite, "source": e.fonte})).collect::<Vec<_>>());
        }
        if x.logico {
            j["bytes_logical"] = json!(true);
        }
        if x.piso {
            j["bytes_floor"] = json!(true);
        }
        j
    }).collect::<Vec<_>>())
}

/// O que um detector achou. `SemDado`: não deu para olhar (vai para `advice_unchecked`).
pub enum Achado {
    Sim { bytes: i64, privado: Option<i64>, processo: Option<String>, ev: Vec<Evidencia>, logico: bool, piso: bool, texto: String },
    Nao,
    SemDado(&'static str),
}

/// Para o que exige dados fora de `top[]`. `cond` só lê o JSON, nunca o disco.
pub struct Detector {
    pub id: &'static str,
    /// rule|signature
    pub tipo: &'static str,
    /// `term` do item emitido.
    pub termo: &'static str,
    pub cond: fn(&Value) -> Achado,
}

pub const DETECTORES: &[Detector] = &[
    Detector { id: "tm_backup_stuck", tipo: "rule", termo: "snapshots", cond: tm_parado },
    Detector { id: "chronod_datavault", tipo: "signature", termo: "deleted_open", cond: chronod },
];

/// Termo `measured` ou `partial` em `data_volume.terms`.
fn termo_medido(v: &Value, k: &str) -> bool {
    v["data_volume"]["terms"].as_array().into_iter().flatten().any(|t| t["key"] == k && matches!(t["state"].as_str(), Some("measured" | "partial")))
}

/// Termo `snapshots` com 1 GB+, referência do TM com mais de 24 h, 2+ tentativas depois do último
/// backup registrado no destino dela e o Time Machine parado (Running=0). O privado da referência
/// pode vir 0 (o espaço fica nos snapshots juntos): `bytes` é o termo (spec-inicio §9.2). O termo
/// é a faixa do mais antigo ao mais novo, de qualquer tipo: com um snapshot que não é do TM, vale
/// a soma dos privados do TM (um piso).
fn tm_parado(v: &Value) -> Achado {
    if !termo_medido(v, "snapshots") {
        return Achado::SemDado("term not measured");
    }
    let termo = v["data_volume"]["terms"].as_array().into_iter().flatten().find(|t| t["key"] == "snapshots").and_then(|t| t["bytes"].as_i64()).unwrap_or(0);
    let agora = v["generated_at"].as_str().unwrap_or("");
    let snaps = v["informational"]["snapshots"].as_array().map(Vec::as_slice).unwrap_or_default();
    let piso = snaps.iter().any(|s| s["kind"] != "time_machine");
    let total = if piso {
        snaps.iter().filter(|s| s["kind"] == "time_machine").filter_map(|s| s["private_bytes"].as_i64()).sum()
    } else {
        termo
    };
    // (1) sem o papel: termo abaixo de 1 GB ou nenhum snapshot do TM com mais de 24 h já responde "não".
    let candidatos: Vec<(&Value, &str, i64)> = snaps
        .iter()
        .filter(|s| s["kind"] == "time_machine")
        .filter_map(|s| Some((s, s["date"].as_str()?, s["private_bytes"].as_i64()?)))
        .filter(|(_, d, _)| crate::sources::snapshots::idade_h(agora, d).is_some_and(|h| h > 24))
        .collect();
    if total < GB || candidatos.is_empty() {
        return Achado::Nao;
    }
    let tm = &v["informational"]["time_machine"];
    if tm["state"] != "measured" {
        return Achado::SemDado("plist unreadable");
    }
    match tm["running"].as_bool() {
        None => return Achado::SemDado("tm status unreadable"),
        Some(true) => return Achado::Nao,
        Some(false) => {}
    }
    for (s, data, p) in candidatos.into_iter().filter(|c| c.0["tm_role"] == "reference") {
        let dest = tm["destinations"].as_array().into_iter().flatten().find(|d| d["reference_at"].as_str() == Some(data));
        let Some(d) = dest else { continue };
        let (Some(n), Some(feito)) = (d["attempts_since_completed"].as_u64(), d["last_completed_at"].as_str()) else { continue };
        if n < 2 {
            continue;
        }
        // "AAAA-MM-DD HH:MM:SS" → "DD/MM at HH:MM"
        let quando = match (feito.get(8..10), feito.get(5..7), feito.get(11..16)) {
            (Some(d), Some(m), Some(h)) => format!("{d}/{m} at {h}"),
            _ => feito.to_string(),
        };
        let mut t = format!("The last backup recorded by Time Machine is from {quando}, and it has tried again {n} times since then without recording another. Local Time Machine snapshots hold {}{}.", if piso { "at least " } else { "" }, gb_en(total));
        if s["purgeable"] == true {
            t.push_str(" If space runs low, macOS may delete them; then the next backup takes longer.");
        }
        t.push_str(" See the destination in System Settings > Time Machine.");
        return Achado::Sim { bytes: total, privado: Some(p), processo: None, ev: Vec::new(), logico: false, piso, texto: t };
    }
    Achado::Nao
}

/// chronod com 50+ arquivos sem alocado legível, 1 GB+ lógicos e 50+ inodes `.dv` do LaunchServices.
fn chronod(v: &Value) -> Achado {
    if !termo_medido(v, "deleted_open") {
        return Achado::SemDado("term not measured");
    }
    let Some(ps) = v["informational"]["deleted_open_processes"].as_array() else { return Achado::SemDado("no lsof") };
    const FONTE: &str = "lsof_proc_pidfdinfo";
    for p in ps.iter().filter(|p| p["name"] == "chronod") {
        let n = |k: &str| p[k].as_i64().unwrap_or(0);
        let ev: Vec<Evidencia> = [("files_without_allocated", 50), ("logical_bytes_without_allocated", GB), ("launchservices_dv_files", 50)]
            .into_iter()
            .map(|(k, l)| Evidencia { chave: k.into(), valor: n(k), limite: l, fonte: FONTE })
            .collect();
        if ev.iter().all(|e| e.valor >= e.limite) {
            let b = n("logical_bytes_without_allocated");
            let t = format!("chronod (widgets) holds deleted copies of the LaunchServices database: {} by the lsof logical size. The allocated size is not readable in that system folder, so it is not in the sum. Logging out or restarting gives the space back; save open work first.", gb_en(b));
            return Achado::Sim { bytes: b, privado: None, processo: Some("chronod".into()), ev, logico: true, piso: false, texto: t };
        }
    }
    Achado::Nao
}

/// Roda os `DETECTORES` sobre o JSON completo: o que dispara entra em `advice`, o que não deu
/// para olhar em `advice_unchecked`. A única supressão: `tm_backup_stuck` tira
/// `time_machine_snapshots`.
pub fn detectar(v: &mut Value) {
    let mut novos = Vec::new();
    let mut sem = Vec::new();
    for d in DETECTORES {
        let ficha = catalogo::ficha(d.id);
        match (d.cond)(v) {
            Achado::Sim { bytes, privado, processo, ev, logico, piso, texto } => {
                let comando = ficha.and_then(|f| f.agir.iter().find_map(|a| if let catalogo::Acao::Comando(c) = a { Some(*c) } else { None }));
                let texto = if comando.is_some() { format!("{texto}{PEDIR}") } else { texto };
                let nivel = ficha.map_or("check_first", |f| f.nivel);
                let evidencia = (d.tipo == "signature").then_some(ev);
                novos.push(Conselho { privado, termo: Some(d.termo), processo, comando, evidencia, logico, piso, ..item(d.id, nivel, bytes, texto) });
            }
            Achado::Nao => {}
            Achado::SemDado(r) => sem.push(json!({"id": d.id, "card": ficha.map(|f| f.id), "reason": r})),
        }
    }
    if !v["advice"].is_array() {
        v["advice"] = json!([]);
    }
    let a = v["advice"].as_array_mut().expect("lista");
    if novos.iter().any(|x| x.id == "tm_backup_stuck") {
        a.retain(|x| x["id"] != "time_machine_snapshots");
    }
    a.extend(json(&novos).as_array().cloned().unwrap_or_default());
    v["advice_unchecked"] = json!(sem);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regras_com_comando_da_lista() {
        assert!(REGRAS.iter().filter_map(|r| r.comando).all(|c| COMANDOS.contains(&c)));
    }

    #[test]
    fn casamento() {
        assert!(casa("~/Library/Caches/*.ShipIt", "~/Library/Caches/com.x.ShipIt"));
        assert!(casa("~/Library/Caches/*-updater", "~/Library/Caches/foo-updater/x"));
        assert!(!casa("~/Library/Caches/*-updater", "~/Library/Caches/foo-updaterx"));
        assert!(casa("/System", "/System/Library/AssetsV2"));
        assert!(!casa("/System", "/"));
    }
}

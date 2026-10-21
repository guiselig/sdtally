//! sdc-coleta: relatório JSON anonimizado da decomposição de "Dados do Sistema".
//! Só leitura. Única escrita: o arquivo do relatório.
//! Base: research/2026-09-27-sintese.md (§1c, §2, §3b–3e, §4).
#![deny(unsafe_code)]

use sdtally::coleta::{self, Opcoes, round2};
use sdtally::ledger;
use sdtally::report::{self, i18n::Lang, texto::{self, Estilo}};
use sdtally::{anon, conselho, sys, walk};
use serde_json::{Value, json};
use sdtally::sources::DATA;
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

const VERSAO: &str = env!("CARGO_PKG_VERSION");

struct Args {
    saida: Option<String>,
    xattr: bool,
    threads: Option<usize>,
    sem_sudo: bool,
    minutos: u64,
}

fn args() -> Args {
    let mut a = Args { saida: None, xattr: true, threads: None, sem_sudo: false, minutos: 120 };
    let mut it = std::env::args().skip(1);
    while let Some(x) = it.next() {
        match x.as_str() {
            "--saida" => a.saida = it.next(),
            // xattr custou +40 % na travessia no Mac de referência (< 50 %): ligado por padrão.
            "--sem-xattr" => a.xattr = false,
            // Sem root a conta fica incompleta (lsof só do usuário, pastas negadas sem dir-stats).
            "--sem-sudo" => a.sem_sudo = true,
            "--threads" => a.threads = it.next().and_then(|t| t.parse().ok()),
            // Amostras depois da coleta; 0 desliga. Teto de 4 h.
            "--minutos" => a.minutos = it.next().and_then(|t| t.parse().ok()).unwrap_or(120).min(240),
            "-V" | "--version" => {
                println!("sdc-coleta {VERSAO}");
                std::process::exit(0);
            }
            "-h" | "--ajuda" | "--help" => {
                println!("uso: ./sdc-coleta [--saida ARQUIVO.json] [--minutos N] [--sem-xattr] [--threads N] [--sem-sudo]\n\
                  Só leitura, em segundo plano. Pede a senha de administrador (sudo) sozinho. Precisa de Acesso Total ao Disco no app de terminal. Grava um relatório anonimizado (padrão: ~/Desktop/sdc-relatorio-AAAA-MM-DD-HHMM.json).\n\
                  --minutos N  depois da coleta, amostras a cada 5 min por N minutos (padrão 120, 0 desliga)\n\
                  --sem-sudo   roda sem root (conta incompleta)\n\
                  --sem-xattr  não mede atributos estendidos grandes (~30 % mais rápido)");
                std::process::exit(0);
            }
            o => {
                eprintln!("opção desconhecida: {o} (use --ajuda)");
                std::process::exit(2);
            }
        }
    }
    a
}

/// O que o sdtally conclui da mesma coleta, sem caminhos nem textos: swap, ligado desde,
/// histórico do Time Machine (só datas e contagens), veredito de cada snapshot, processos com
/// apagados-abertos e as recomendações/assinaturas (id, nível e bytes).
fn diagnostico(v: &Value, ctx: &anon::Ctx) -> Value {
    let inf = &v["informational"];
    let nome = |x: &Value| x.as_str().map_or(Value::Null, |n| json!(ctx.processo(n)));
    // Criador de snapshot só sai em forma de identificador reverso (com.apple.TimeMachine,
    // com.bombich.ccc): "Mac-de-Fulano.snap" ou "Fulanos-MacBook-Pro.local.2026-10-05" carregam nomes.
    let criador = |x: &Value| {
        x.as_str()
            .filter(|c| c.split('.').next().is_some_and(|t| (2..=4).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_lowercase())))
            .map_or(Value::Null, |c| json!(ctx.clean(c)))
    };
    let mut snapshots = inf["snapshots"].clone();
    for s in snapshots.as_array_mut().into_iter().flatten() {
        s["creator"] = criador(&s["creator"]);
    }
    let mut abertos = inf["deleted_open_processes"].clone();
    for p in abertos.as_array_mut().into_iter().flatten() {
        p["name"] = nome(&p["name"]);
    }
    let conselhos: Vec<Value> = v["advice"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|a| {
            json!({
                "id": a["id"], "kind": a["kind"], "level": a["level"], "term": a["term"], "process": nome(&a["process"]),
                "bytes": a["bytes"], "private_bytes": a["private_bytes"], "bytes_logical": a["bytes_logical"],
                "evidence": a["evidence"],
            })
        })
        .collect();
    json!({
        "fonte": "sdtally sobre esta coleta: report::montar, conselho::conselhos e conselho::detectar (sem caminhos, textos nem comandos)",
        "swap": v["swap"],
        "boot_time": v["boot_time"],
        "time_machine": inf["time_machine"],
        "snapshots": snapshots,
        "deleted_open_processes": abertos,
        "conselhos": conselhos,
        "nao_conferidos": v["advice_unchecked"],
    })
}

/// Acesso Total ao Disco pelo TCC.db, que existe em todo Mac: sem ele o open dá EPERM, em silêncio.
/// Sem esse acesso, ler Mesa, Documentos, Downloads ou dados de outros apps faz o macOS abrir
/// pedidos de permissão na tela: a coleta não começa.
fn tem_acesso_total() -> bool {
    std::fs::File::open("/Library/Application Support/com.apple.TCC/TCC.db").is_ok()
}

/// Abre Ajustes > Privacidade e Segurança > Acesso Total ao Disco na sessão de quem está no Mac:
/// com sudo, como o SUDO_USER (open como root não abre na tela da pessoa). Só depois do Enter.
fn abrir_ajustes_acesso_total(root: bool) {
    const URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";
    let mut cmd = match (root, std::env::var("SUDO_UID"), std::env::var("SUDO_USER")) {
        (true, Ok(uid), Ok(user)) => {
            let mut c = std::process::Command::new("/bin/launchctl");
            c.args(["asuser", &uid, "/usr/bin/sudo", "-u", &user, "/usr/bin/open", URL]);
            c
        }
        _ => {
            let mut c = std::process::Command::new("/usr/bin/open");
            c.arg(URL);
            c
        }
    };
    if !cmd.status().is_ok_and(|s| s.success()) {
        eprintln!("Não consegui abrir o Ajustes: abra Ajustes > Privacidade e Segurança > Acesso Total ao Disco.");
    }
}

/// Bytes escritos no disco e escritas lógicas por nome de processo desde a amostra `base`.
/// Processo que já estava vivo conta a diferença; o que nasceu depois conta tudo (começou na
/// janela). Quem saiu no meio não aparece: o kernel some com o contador.
fn escrita_na_janela(base: &HashMap<(i32, u64), (u64, u64)>, agora: &[(i32, u64, String, u64, u64)]) -> Vec<(String, u64, u64)> {
    let mut por_nome: HashMap<&str, (u64, u64)> = HashMap::new();
    for (pid, inicio, nome, b, l) in agora {
        let (b0, l0) = base.get(&(*pid, *inicio)).copied().unwrap_or((0, 0));
        let e = por_nome.entry(nome.as_str()).or_default();
        e.0 += b.saturating_sub(b0);
        e.1 += l.saturating_sub(l0);
    }
    let mut v: Vec<_> = por_nome.into_iter().filter(|x| x.1.0 > 0).map(|(n, (b, l))| (n.to_string(), b, l)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

/// Autoteste de privacidade no próprio texto: sem @, usuário, nome ou host.
fn autoteste(rel: &mut Value, ctx: &anon::Ctx) -> usize {
    if let Some(o) = rel.as_object_mut() {
        o.remove("autoteste_privacidade");
    }
    let achados = ctx.achados(&serde_json::to_string(rel).unwrap_or_default());
    rel["autoteste_privacidade"] = json!({"ok": achados == 0, "achados": achados});
    achados
}

const MEXIDO: &str = "o relatório foi editado, movido ou apagado";

/// Regrava o relatório: temporário oculto (mesmas garantias do `gravar`) e rename por cima. Quem lê
/// nunca vê arquivo pela metade, e um symlink no lugar do relatório é trocado, não seguido. Se a
/// pessoa editou, moveu ou apagou o arquivo desde a última gravação, não grava por cima (`MEXIDO`).
/// Falhou no meio (disco cheio)? O temporário sai: na Mesa só fica o relatório.
fn regravar(dir: &sys::PastaSaida, nome: &str, dados: &[u8], root: bool, marca: &mut Option<sys::Marca>) -> Result<(), String> {
    static N: AtomicU32 = AtomicU32::new(0);
    if marca.is_none() || dir.marca(nome) != *marca {
        return Err(MEXIDO.into());
    }
    let tmp = format!(".{nome}.{}.{}.tmp", std::process::id(), N.fetch_add(1, Ordering::Relaxed));
    let r = gravar(dir, &tmp, dados, root).and_then(|_| dir.renomear(&tmp, nome).map_err(|e| std::io::Error::from_raw_os_error(e).to_string()));
    if r.is_err() {
        dir.remover(&tmp);
    }
    r?;
    *marca = dir.marca(nome);
    Ok(())
}

/// Espera `s` segundos; false no primeiro Ctrl-C.
fn dormir(s: u64) -> bool {
    for _ in 0..s {
        if sys::INTERROMPIDO.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    !sys::INTERROMPIDO.load(Ordering::Relaxed)
}

/// Amostras leves a cada 5 min até `minutos`, Ctrl-C ou fechar o Terminal: livre e usado do Data,
/// swap e escrita no disco por processo. Nada de CacheDelete, lsof ou travessia aqui (custo e
/// bateria). Regrava o relatório a cada 3 amostras (menos versões no iCloud e no Spotlight), na
/// última e ao interromper; para de vez se o arquivo foi mexido ou a gravação falhou.
fn amostrar(rel: &mut Value, dir: &sys::PastaSaida, nome: &str, root: bool, minutos: u64, ctx: &anon::Ctx, marca: &mut Option<sys::Marca>) {
    const PASSO: u64 = 300;
    let base: HashMap<(i32, u64), (u64, u64)> = sys::escrita_processos().into_iter().map(|(p, t, _, b, l)| ((p, t), (b, l))).collect();
    let t0 = Instant::now();
    let total = minutos * 60 / PASSO;
    let mut pontos = Vec::new();
    for i in 0..=total {
        let parar = i > 0 && !dormir(PASSO);
        if !parar {
            let swap = sys::swap_usage();
            pontos.push(json!({
                "t": sys::now_local().0,
                "livre": sys::statfs(DATA).ok().map(|x| x.1),
                "usado_data": sys::apfs::vol_space_used(DATA).ok(),
                "swap_total": swap.map(|x| x.0),
                "swap_usado": swap.map(|x| x.1),
            }));
            let escrita = escrita_na_janela(&base, &sys::escrita_processos());
            rel["amostras"] = json!({
                "fonte": "statfs e ATTR_VOL_SPACEUSED do Data, sysctl vm.swapusage, proc_pid_rusage (ri_diskio_byteswritten, ri_logical_writes) de cada processo",
                "intervalo_s": PASSO,
                "minutos_pedidos": minutos,
                "decorrido_s": t0.elapsed().as_secs(),
                "pontos": pontos,
                "escrita_por_processo": escrita.iter().take(40).map(|(n, b, l)| json!({"processo": ctx.processo(n), "bytes_escritos": b, "escritas_logicas": l})).collect::<Vec<_>>(),
                "nota": "escrita desde a primeira amostra; processo que saiu no meio não aparece; com o Mac dormindo não há amostra",
            });
        }
        if parar || i == total || i % 3 == 0 {
            let achados = autoteste(rel, ctx);
            let ok = serde_json::to_string_pretty(rel).map_err(|e| e.to_string()).and_then(|s| regravar(dir, nome, s.as_bytes(), root, marca));
            if let Err(e) = ok {
                println!("\nParei as amostras: {e}. Não gravo mais nada; o arquivo fica como está.");
                return;
            }
            print!("\ramostra {}/{} gravada{}   ", pontos.len().saturating_sub(1), total, if achados > 0 { " (autoteste achou dado pessoal)" } else { "" });
            let _ = std::io::stdout().flush();
        }
        if parar {
            break;
        }
    }
    println!();
}

/// Grava o relatório na pasta já aberta. O_EXCL|O_NOFOLLOW recusa arquivo ou symlink já existente:
/// como root, um symlink plantado faria sobrescrever o alvo. O dono volta a quem rodou o sudo pelo
/// próprio fd (fchown), sem novo lookup do caminho; falha aí não é fatal.
fn gravar(dir: &sys::PastaSaida, nome: &str, dados: &[u8], root: bool) -> Result<(), String> {
    let mut f = dir
        .criar(nome)
        .map_err(|e| if e == libc::EEXIST { "já existe (escolha outro --saida)".to_string() } else { std::io::Error::from_raw_os_error(e).to_string() })?;
    if let Err(e) = f.write_all(dados) {
        // Criado agora com O_EXCL, então é do programa: arquivo vazio ou pela metade não fica na Mesa.
        drop(f);
        dir.remover(nome);
        return Err(e.to_string());
    }
    if root
        && let (Some(uid), Some(gid)) = (std::env::var("SUDO_UID").ok().and_then(|v| v.parse().ok()), std::env::var("SUDO_GID").ok().and_then(|v| v.parse().ok()))
    {
        let _ = std::os::unix::fs::fchown(&f, Some(uid), Some(gid));
    }
    Ok(())
}

fn main() {
    let a = args();
    // Panic capturado vira erro no relatório; SDC_PANIC=1 (CI) deixa a mensagem no stderr.
    if std::env::var_os("SDC_PANIC").is_none() {
        std::panic::set_hook(Box::new(|_| {}));
    }

    // ---- segurança (síntese §4): obrigatórias, senão aborta ----
    let root = sys::e_root();
    // Sem sudo (dois cliques no Finder ou ./sdc-coleta): pede a senha e se reabre como root, com os
    // mesmos argumentos. exec troca o processo: Ctrl-C e o fim vão direto para o novo.
    if !root && !a.sem_sudo {
        use std::os::unix::process::CommandExt;
        println!("sdc-coleta: precisa de permissão de administrador para medir as pastas do sistema (continua só lendo).");
        let erro = match std::env::current_exe() {
            Ok(eu) => std::process::Command::new("/usr/bin/sudo")
                .args(["-p", "Senha do Mac (de administrador; não aparece ao digitar): "])
                .arg(eu)
                .args(std::env::args_os().skip(1))
                .exec()
                .to_string(),
            Err(e) => e.to_string(),
        };
        eprintln!("Não consegui pedir a senha ({erro}). Rode: sudo ./sdc-coleta  (ou --sem-sudo para uma coleta incompleta).");
        std::process::exit(2);
    }
    if let Err((r1, r2)) = sys::politicas_seguranca() {
        eprintln!("setiopolicy_np falhou (materialize={r1}, atime={r2}); abortando para não alterar nada.");
        std::process::exit(1);
    }
    sys::qos_utility();
    // Segundo plano de verdade: o macOS adia CPU e disco enquanto a pessoa usa o Mac.
    let fundo = sys::modo_fundo();

    let t_total = Instant::now();
    let (agora, carimbo) = sys::now_local();

    if !tem_acesso_total() {
        eprintln!(
            "Falta o Acesso Total ao Disco no app de terminal (Terminal, iTerm, Warp...). Sem ele o macOS mostraria\n\
             pedidos de permissão na tela durante a coleta. Nenhum programa consegue ligar isso sozinho, nem com sudo:\n\
             só você, no Ajustes. Aperte Enter para abrir o Ajustes já na lista certa (ou Ctrl-C para sair)."
        );
        let mut l = String::new();
        if matches!(std::io::stdin().read_line(&mut l), Ok(n) if n > 0) {
            abrir_ajustes_acesso_total(root);
        }
        eprintln!(
            "Na lista Acesso Total ao Disco, ligue o seu app de terminal (se não estiver lá, use o botão +).\n\
             Depois feche o terminal com Cmd-Q e rode de novo (o mesmo comando ou dois cliques no programa). Nada foi lido."
        );
        std::process::exit(2);
    }
    // Mac acordado enquanto roda (o caffeinate da Apple solta sozinho quando este processo sai, até por
    // kill -9). Só o sistema e o disco: a tela pode apagar e bloquear, o que não interrompe a coleta.
    let acordado = std::process::Command::new("/usr/bin/caffeinate")
        .args(["-ims", "-w", &std::process::id().to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .is_ok();
    // Antes da coleta: Ctrl-C ou fechar o Terminal na primeira parte grava o parcial em vez de perder tudo.
    let _ = sys::ao_interromper();
    let _ = sys::ao_fechar();
    println!("sdc-coleta {VERSAO}: coletando em segundo plano (só leitura). Pode continuar usando o Mac; a primeira parte leva de 3 a 30 minutos...");

    // ---- identidade local, usada só para anonimizar (nunca vai para o JSON) ----
    // Com sudo, a pessoa é SUDO_USER: home e nomes vêm dela, não do root.
    let (user, home) = coleta::identidade(root);
    let full = sys::run("/usr/bin/id", &["-F", &user], 10).unwrap_or_default();
    let mut hosts: Vec<String> = ["ComputerName", "LocalHostName", "HostName"]
        .iter()
        .filter_map(|k| sys::run("/usr/sbin/scutil", &["--get", k], 10).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if let Some(h) = sys::sysctl_str("kern.hostname") {
        hosts.push(h.trim_end_matches(".local").to_string());
    }
    let ctx = Arc::new(anon::Ctx::new(&user, full.trim(), &hosts));
    // O arquivo não pode existir (ver `gravar`): avisar agora, não depois de minutos de coleta.
    // A pasta é aberta uma vez e todas as gravações (por horas, como root) são relativas a ela.
    // Com sudo, a pasta tem de ser de quem rodou: pasta de outro dono (ou symlink para uma pasta do
    // sistema) faria o root criar um arquivo do usuário onde ele não pode.
    let dono_esperado: Option<u32> = if root { std::env::var("SUDO_UID").ok().and_then(|v| v.parse().ok()) } else { None };
    let saida = a.saida.clone().unwrap_or_else(|| format!("{home}/Desktop/sdc-relatorio-{carimbo}.json"));
    let caminho = std::path::Path::new(&saida);
    let nome = caminho.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let pai = caminho.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(std::path::Path::new("."));
    let dir = match sys::PastaSaida::abrir(pai) {
        Ok(d) if nome.is_empty() => {
            drop(d);
            eprintln!("--saida precisa de um nome de arquivo.");
            std::process::exit(2);
        }
        Ok(d) => d,
        Err(e) => {
            eprintln!("Não consegui abrir a pasta de {saida}: {}", std::io::Error::from_raw_os_error(e));
            std::process::exit(2);
        }
    };
    if let Some(uid) = dono_esperado
        && dir.dono() != Ok(uid)
    {
        eprintln!("A pasta de {saida} não é sua; escolha outro --saida (por exemplo na sua Mesa).");
        std::process::exit(2);
    }
    if dir.marca(&nome).is_some() {
        eprintln!("{saida} já existe; escolha outro --saida.");
        std::process::exit(2);
    }

    let c = coleta::coletar(&Opcoes { threads: a.threads.or(Some(2)), xattr: a.xattr, root }, &home, ctx.clone(), Arc::new(walk::Controle::new(&sys::INTERROMPIDO)));
    let (caps, fda, perf, threads) = (&c.caps, c.fda.clone(), c.perf, c.threads);
    let (usado_data, snaps, purg, arv, neg, pn) = (&c.usado, &c.snaps, &c.purg, &c.arvore, &c.negadas, &c.painel);

    // ---- árvore (JSON) ----
    let arvore = match arv {
        None => Value::Null,
        Some(r) => {
            let v = coleta::baldes(r);
            let erros: BTreeMap<String, u64> = r
                .errors
                .iter()
                .map(|(k, v)| {
                    let k = match *k {
                        walk::ERRO_INTERNO => "interno".into(),
                        walk::ERRO_PARSE => "parse".into(),
                        walk::ERRO_ATRIBUTO_AUSENTE => "atributo_ausente".into(),
                        k => k.to_string(),
                    };
                    (k, *v)
                })
                .collect();
            let t = &r.total;
            json!({
                "fonte": "getattrlistbulk em /System/Volumes/Data, mesmo volume, sem descer em dataless nem em raiz de enxerto; hardlink e clone completo contados uma vez",
                "threads": threads,
                "totais": {
                    "pastas": r.dirs, "arquivos": t.files,
                    "alocado_unico": t.alloc, "privado": t.private,
                    "hardlinks_repetidos": {"arquivos": r.link_dup_files, "bytes": r.link_dup_bytes},
                    "clones_completos_repetidos": {"arquivos": r.clone_dup_files, "bytes": r.clone_dup_bytes},
                    "dataless": {"arquivos": t.dataless_n, "bytes_aparentes": t.dataless_logical, "pastas_nao_abertas": t.dataless_dirs},
                    "raizes_de_enxerto_puladas": r.graft_roots,
                    "outros_volumes_nao_cruzados": r.other_dev,
                    "erros_por_errno": erros,
                    "pastas_negadas": r.denied.len(),
                    "xattr_medido": a.xattr,
                    "xattr_objetos_com_erro": r.xattr_erros,
                    "xattr_fluxo_proprio": if a.xattr { json!(t.xattr_stream) } else { Value::Null },
                    "xattr_todos": if a.xattr { json!(t.xattr_all) } else { Value::Null },
                },
                "baldes": v.iter().map(|(k, s)| json!({
                    "balde": k, "alocado": s.alloc, "privado": s.private, "arquivos": s.files,
                    "dataless_arquivos": s.dataless_n, "dataless_bytes_aparentes": s.dataless_logical, "dataless_pastas": s.dataless_dirs,
                    "xattr_fluxo_proprio": if a.xattr { json!(s.xattr_stream) } else { Value::Null },
                })).collect::<Vec<_>>(),
                "regra_baldes": "baldes < 100 MB somados em (demais) do pai",
            })
        }
    };

    // ---- conta ----
    let termos = c.termos();
    let resto = ledger::resto(usado_data.valor(), termos.iter().map(|(_, t)| &t.medida));
    let conta = json!({
        "usado_volume_data": {"bytes": usado_data.valor(), "estado": usado_data.estado(), "fonte": "ATTR_VOL_SPACEUSED"},
        "menos": termos.iter().map(|(_, t)| json!({"termo": t.chave, "bytes": t.medida.valor(), "estado": t.medida.estado(), "fonte": t.fonte})).collect::<Vec<_>>(),
        "nao_atribuido": {"bytes": resto, "fonte": "diferença exata; contém pastas sem dir-stats, termos não medidos e objetos não vistos"},
        "nota": "termos medidos em instantes diferentes durante a coleta",
    });

    // ---- meta e escrita ----
    let mut tempos = c.tempos.clone();
    tempos.insert("total".into(), json!(round2(t_total.elapsed().as_secs_f64())));
    let meta = json!({
        "coletor": format!("sdc-coleta {VERSAO}"),
        "data_hora": agora,
        // Versão já lida no Caps; build pelo sysctl (o mesmo que `sw_vers -buildVersion`).
        "macos": {"versao": caps.macos, "build": sys::sysctl_str("kern.osversion")},
        "capacidades": caps.json(),
        "hw_model": sys::sysctl_str("hw.model"),
        "cpu": sys::sysctl_str("machdep.cpu.brand_string"),
        "memoria_bytes": sys::sysctl_u64("hw.memsize"),
        "nucleos_desempenho": perf,
        "disco_interno_tamanho": c.disco_tamanho,
        "root": root,
        "acesso_total_ao_disco": fda,
        "acesso_total_ao_disco_teste": "abrir ~/Library/Safari",
        "threads": threads,
        "segundo_plano": fundo,
        "mantem_acordado": acordado,
        // Ctrl-C, Terminal fechado ou logout antes do fim da primeira parte: números parciais.
        "interrompida": c.interrompida,
        "etapas_segundos": tempos,
    });
    let conta_r = report::montar(&c, &format!("sdc-coleta {VERSAO}"), &agora.chars().take(16).collect::<String>().replace('T', " "));
    let mut v_sdt = report::json(&conta_r);
    v_sdt["advice"] = conselho::json(&conselho::conselhos(&conta_r));
    conselho::detectar(&mut v_sdt);
    let mut rel = json!({
        "schema": 1,
        "meta": meta,
        "volumes": c.volumes,
        "painel": pn.as_ref().map_or(Value::Null, |p| p.json.clone()),
        "volume_data": c.data_json,
        "snapshots": snaps.as_ref().map_or(Value::Null, |s| s.json.clone()),
        "purgeable": purg.as_ref().map_or(Value::Null, |p| p.json.clone()),
        "apagados_abertos": c.lsof,
        "arvore": arvore,
        "pastas_negadas": neg.as_ref().map_or(Value::Null, |n| n.json.clone()),
        "conta": conta,
        "diagnostico": diagnostico(&v_sdt, &ctx),
        "erros": c.erros,
    });
    let achados = autoteste(&mut rel, &ctx);

    let gravou = serde_json::to_string_pretty(&rel).map_err(|e| e.to_string()).and_then(|s| gravar(&dir, &nome, s.as_bytes(), root));
    let mut marca = dir.marca(&nome);

    // ---- resumo ----
    println!();
    match &gravou {
        Ok(_) => println!("Relatório gravado em: {saida}"),
        Err(e) => println!("ERRO ao gravar {saida}: {e}"),
    }
    // Mesmo relatório do sdtally (pt, 15 linhas), com os nomes já anonimizados do JSON.
    let e = Estilo { lang: Lang::Pt, binario: false, cor: false, top: 15 };
    print!("\n{}", texto::conta(&conta_r, &e));
    println!(
        "Acesso Total ao Disco: {fda} | duração: {:.0} s | erros registrados: {}",
        t_total.elapsed().as_secs_f64(),
        rel["erros"].as_array().map_or(0, |e| e.len())
    );
    if achados > 0 {
        println!("ATENÇÃO: o autoteste achou {achados} possível dado pessoal no arquivo.");
    }
    if gravou.is_err() {
        std::process::exit(1);
    }
    if a.minutos > 0 && !sys::INTERROMPIDO.load(Ordering::Relaxed) {
        println!(
            "\nAgora ele fica medindo em segundo plano por até {} min (uma medida a cada 5 min; o arquivo é regravado a cada 15).\n\
             Pode continuar trabalhando; o Mac não dorme sozinho até acabar (a tela pode apagar). Notebook: não feche a tampa.\n\
             Não edite nem mova o arquivo até aparecer \"Revise o arquivo\". Para terminar antes: Ctrl-C ou feche o Terminal.",
            a.minutos
        );
        amostrar(&mut rel, &dir, &nome, root, a.minutos, &ctx, &mut marca);
    }
    println!("\nRevise o arquivo antes de mandar: {saida}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escrita_conta_diferenca_e_quem_nasceu_depois() {
        let base = HashMap::from([((10, 1), (100, 5)), ((11, 2), (50, 1))]);
        let agora = vec![
            (10, 1, "a".to_string(), 400, 9), // vivo desde a base: 300
            (12, 3, "a".to_string(), 50, 2),  // nasceu depois: tudo
            (11, 2, "b".to_string(), 50, 1),  // sem escrita nova: fica de fora
            (10, 9, "c".to_string(), 70, 1),  // pid reaproveitado (outro início): processo novo
        ];
        assert_eq!(escrita_na_janela(&base, &agora), vec![("a".to_string(), 350, 6), ("c".to_string(), 70, 1)]);
    }

    #[test]
    fn diagnostico_sem_caminho_texto_nem_comando() {
        let v = json!({"advice": [{"id": "logs", "kind": "rule", "level": "safe", "paths": ["~/Library/Logs"], "text": "x", "command": "brew cleanup --dry-run", "bytes": 1}]});
        let d = diagnostico(&v, &anon::Ctx::new("fulano", "Fulano de Tal", &["Mac de Fulano".to_string()]));
        let s = d.to_string();
        assert!(!s.contains("Library/Logs") && !s.contains("brew") && !s.contains("\"text\""), "{s}");
        assert_eq!(d["conselhos"][0]["id"], "logs");
    }

    #[test]
    fn diagnostico_tira_nomes_de_snapshot_e_processo() {
        let v = json!({
            "advice": [{"id": "x", "process": "relatorio-cliente-acme-guilherm"}],
            "informational": {
                "snapshots": [{"creator": "com.apple.TimeMachine"}, {"creator": "Mac-de-Fulano.snap"}, {"creator": "Joanas-MacBook-Pro.local.2026"}, {"creator": "com.fulano.backup"}],
                "deleted_open_processes": [{"name": "fulano-agent"}, {"name": "chronod"}],
            },
        });
        let d = diagnostico(&v, &anon::Ctx::new("fulano", "Fulano de Tal", &[]));
        let c: Vec<&Value> = d["snapshots"].as_array().unwrap().iter().map(|s| &s["creator"]).collect();
        assert_eq!(c, [&json!("com.apple.TimeMachine"), &Value::Null, &Value::Null, &json!("<privado>")]);
        assert_eq!(d["deleted_open_processes"][0]["name"], "<privado>");
        assert_eq!(d["deleted_open_processes"][1]["name"], "chronod");
        assert_eq!(d["conselhos"][0]["process"], "relatorio-cliente-acme-…", "nome cortado perde o último trecho");
    }

    #[test]
    fn autoteste_casa_token_curto_so_como_palavra() {
        let ctx = anon::Ctx::new("cida", "Cida Souza", &[]);
        let mut rel = json!({"level": "system_handles", "processo": "chronod"});
        assert_eq!(autoteste(&mut rel, &ctx), 0, "a própria chave do autoteste não conta");
        assert_eq!(autoteste(&mut rel, &ctx), 0, "nem na segunda passada");
        rel["x"] = json!("/Users/cida/Desktop");
        assert_eq!(autoteste(&mut rel, &ctx), 1);
    }

    #[test]
    fn regravar_nao_passa_por_cima_de_edicao_e_nao_deixa_temporario() {
        let base = std::env::temp_dir().join(format!("sdc-teste-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let dir = sys::PastaSaida::abrir(&base).unwrap();
        gravar(&dir, "r.json", b"1", false).unwrap();
        assert!(gravar(&dir, "r.json", b"x", false).is_err(), "O_EXCL");
        let mut marca = dir.marca("r.json");
        regravar(&dir, "r.json", b"22", false, &mut marca).unwrap();
        assert_eq!(std::fs::read(base.join("r.json")).unwrap(), b"22");
        std::fs::write(base.join("r.json"), b"editado pela pessoa").unwrap();
        assert_eq!(regravar(&dir, "r.json", b"333", false, &mut marca), Err(MEXIDO.into()));
        assert_eq!(std::fs::read(base.join("r.json")).unwrap(), b"editado pela pessoa");
        std::fs::remove_file(base.join("r.json")).unwrap();
        assert!(regravar(&dir, "r.json", b"333", false, &mut marca).is_err(), "apagado não volta");
        assert!(!base.join("r.json").exists());
        let sobra: Vec<_> = std::fs::read_dir(&base).unwrap().collect();
        assert!(sobra.is_empty(), "{sobra:?}");
        std::fs::remove_dir(&base).unwrap();
    }
}

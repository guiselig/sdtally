//! sdtally: a conta do "Dados do Sistema" (PLANO §3, Fase 1). Só leitura; não grava nada em
//! disco, nem cache. Texto em stdout (ou JSON com --json); progresso só em stderr de TTY.
#![deny(unsafe_code)]

use sdtally::coleta::{self, Opcoes};
use sdtally::report::{self, i18n::Lang, texto};
use sdtally::sources::DATA;
use sdtally::walk::Controle;
use sdtally::{anon::Ctx, catalogo, detalhe, historico, mcp, redigir, sys, verify};
use serde_json::Value;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const VERSAO: &str = env!("CARGO_PKG_VERSION");

enum Modo {
    Conta,
    Ramo(String),
    Why(String),
    // Acesso por IA (§4.5): só leem o que o app salvou ou o rápido; nunca como root.
    Ultima,
    Historico,
    /// `--changes --json`: o que mudou entre as duas medições salvas mais novas.
    Mudancas,
    Mcp,
    Catalogo,
    /// `--redact names --stdin`: redige o JSON da entrada (exportação do app).
    Stdin,
    /// `--verify [--json]`: confere com os comandos da Apple, agora; sem root.
    Verificar,
    /// `--auto`: árvore viva, uma linha JSON por mudança, até o stdin fechar; sem root.
    Auto,
}

enum Cor {
    Auto,
    Sempre,
    Nunca,
}

struct Args {
    modo: Modo,
    top: usize,
    depth: usize,
    json: bool,
    rapido: bool,
    /// `--since`: carimbo (segundos desde a época) da leitura rápida anterior.
    desde: Option<i64>,
    /// `--parts`: só estas partes da leitura rápida; None = todas.
    partes: Option<coleta::Partes>,
    /// `--watch`: a tela pelo `log stream`, uma linha JSON por tela nova, até o stdin fechar.
    vigiar: bool,
    binario: bool,
    lang: Lang,
    cor: Cor,
    /// `--redact names|off`; sem a opção, só o `--mcp` redige.
    redact: Option<bool>,
}

fn sair(msg: &str) -> ! {
    eprintln!("sdtally: {msg}");
    std::process::exit(2);
}

fn args() -> Args {
    let brutos: Vec<String> = std::env::args().skip(1).collect();
    // Idioma antes do resto, para as próprias mensagens de erro de argumento.
    let lang = brutos.iter().position(|x| x == "--lang").and_then(|i| brutos.get(i + 1)).and_then(|v| Lang::de(v)).unwrap_or_else(Lang::do_ambiente);
    let mut a = Args { modo: Modo::Conta, top: 15, depth: 1, json: false, rapido: false, desde: None, partes: None, vigiar: false, binario: false, lang, cor: Cor::Auto, redact: None };
    let mut it = brutos.into_iter();
    let num = |x: Option<String>, o: &str| x.and_then(|v| v.parse::<usize>().ok()).filter(|n| *n > 0).unwrap_or_else(|| sair(&lang.f("arg_numero", &[o])));
    let mut livres = Vec::new();
    let mut ia = Vec::new();
    let mut catalogo = false;
    let mut stdin = false;
    let mut verificar = false;
    let mut auto = false;
    while let Some(x) = it.next() {
        match x.as_str() {
            "--top" => a.top = num(it.next(), "--top"),
            "--depth" => a.depth = num(it.next(), "--depth"),
            "--json" => a.json = true,
            "--quick" => a.rapido = true,
            "--parts" => a.partes = Some(it.next().as_deref().and_then(coleta::Partes::de).unwrap_or_else(|| sair(&lang.t("arg_parts")))),
            "--since" => a.desde = Some(it.next().and_then(|v| v.parse::<i64>().ok()).filter(|n| *n > 0).unwrap_or_else(|| sair(&lang.t("arg_since")))),
            "--watch" => a.vigiar = true,
            "--binary" => a.binario = true,
            "--last" => ia.push(Modo::Ultima),
            "--history" => ia.push(Modo::Historico),
            "--changes" => ia.push(Modo::Mudancas),
            "--mcp" => ia.push(Modo::Mcp),
            "--catalog" => catalogo = true,
            "--stdin" => stdin = true,
            "--verify" => verificar = true,
            "--auto" => auto = true,
            "--redact" => {
                a.redact = match it.next().as_deref() {
                    Some("names") => Some(true),
                    Some("off") => Some(false),
                    _ => sair(&lang.t("arg_redact")),
                }
            }
            "--lang" => a.lang = it.next().as_deref().and_then(Lang::de).unwrap_or_else(|| sair(&lang.t("arg_lang"))),
            "--color" => {
                a.cor = match it.next().as_deref() {
                    Some("auto") => Cor::Auto,
                    Some("always") => Cor::Sempre,
                    Some("never") => Cor::Nunca,
                    _ => sair(&lang.t("arg_cor")),
                }
            }
            "-V" | "--version" => {
                escrever(&format!("sdtally {VERSAO}\n"));
                std::process::exit(0);
            }
            "-h" | "--ajuda" | "--help" => {
                escrever(&format!("{}\n", lang.f("ajuda", &[VERSAO])));
                std::process::exit(0);
            }
            o if o.starts_with('-') => sair(&lang.f("arg_desconhecida", &[o])),
            _ => livres.push(x),
        }
    }
    a.modo = match livres.as_slice() {
        [] => Modo::Conta,
        [w, p] if w == "why" => Modo::Why(p.clone()),
        [p] if p != "why" => Modo::Ramo(p.clone()),
        _ => sair(&lang.t("arg_uso")),
    };
    if !ia.is_empty() {
        // --last, --history e --changes exigem --json; --mcp vai sem; nenhum aceita caminho, --quick ou outro deles.
        let m = ia.remove(0);
        if !ia.is_empty() || !matches!(a.modo, Modo::Conta) || a.rapido || a.json == matches!(m, Modo::Mcp) {
            sair(&lang.t("arg_ia"));
        }
        a.modo = m;
    }
    if catalogo {
        if !a.json || a.rapido || !matches!(a.modo, Modo::Conta) {
            sair(&lang.t("arg_catalog"));
        }
        a.modo = Modo::Catalogo;
    }
    if verificar {
        if !ia.is_empty() || catalogo || stdin || a.rapido || !matches!(a.modo, Modo::Conta) {
            sair(&lang.t("arg_verify"));
        }
        a.modo = Modo::Verificar;
    }
    if auto {
        if !ia.is_empty() || catalogo || stdin || verificar || a.rapido || a.vigiar || a.redact.is_some() || !matches!(a.modo, Modo::Conta) {
            sair(&lang.t("arg_auto"));
        }
        a.modo = Modo::Auto;
    }
    if a.partes.is_some() && !a.rapido {
        sair(&lang.t("arg_parts"));
    }
    if a.desde.is_some() && !a.rapido {
        sair(&lang.t("arg_since"));
    }
    if a.vigiar && (!a.rapido || a.desde.is_some() || a.partes.is_some()) {
        sair(&lang.t("arg_watch"));
    }
    if a.rapido && (!a.json || !matches!(a.modo, Modo::Conta)) {
        sair(&lang.t("arg_quick"));
    }
    if stdin {
        if a.redact != Some(true) || !matches!(a.modo, Modo::Conta) || a.rapido {
            sair(&lang.t("arg_stdin"));
        }
        a.modo = Modo::Stdin;
    }
    // --redact só onde sai uma medição em JSON: completa, rápida, --last, --history, --changes, --mcp, --stdin.
    let medicao_json = matches!(a.modo, Modo::Ultima | Modo::Historico | Modo::Mudancas | Modo::Mcp | Modo::Stdin) || (matches!(a.modo, Modo::Conta) && a.json);
    if a.redact.is_some() && !medicao_json {
        sair(&lang.t("arg_redact"));
    }
    a
}

/// Toda saída em stdout passa por aqui. Leitor que fechou o pipe (`| head`) encerra com 0: o
/// `println!` daria panic, e o hook mudo viraria saída 101.
fn escrever(s: &str) {
    use std::io::Write;
    let mut o = std::io::stdout().lock();
    if let Err(e) = o.write_all(s.as_bytes()).and_then(|_| o.flush()) {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        sair(&e.to_string());
    }
}

/// JSON em stdout (`linha`: compacto, uma linha, para o app ler linha a linha); com `red`,
/// redigido e escapado (§4.5).
fn imprimir(mut v: Value, red: Option<(&Ctx, &str)>, linha: bool) {
    if let Some((c, home)) = red {
        redigir::redigir(&mut v, c, home);
        redigir::escapar_json(&mut v);
    }
    let s = if linha { serde_json::to_string(&v) } else { serde_json::to_string_pretty(&v) };
    escrever(&(s.unwrap_or_default() + "\n"));
}

/// Contagem de arquivos em stderr a cada ~250 ms, só em TTY. Devolve a trava para parar.
fn progresso(ctl: Arc<Controle>, lang: Lang) -> Option<(Arc<AtomicBool>, std::thread::JoinHandle<()>)> {
    if !std::io::stderr().is_terminal() {
        return None;
    }
    let fim = Arc::new(AtomicBool::new(false));
    let f = fim.clone();
    let h = std::thread::spawn(move || {
        while !f.load(Ordering::Relaxed) {
            eprint!("\r{}\x1b[K", lang.f("progresso", &[&lang.int(ctl.arquivos.load(Ordering::Relaxed))]));
            std::thread::sleep(Duration::from_millis(250));
        }
        eprint!("\r\x1b[K");
    });
    Some((fim, h))
}

fn parar(p: Option<(Arc<AtomicBool>, std::thread::JoinHandle<()>)>) {
    if let Some((f, h)) = p {
        f.store(true, Ordering::Relaxed);
        let _ = h.join();
    }
}

/// Caminho no volume Data, ou sai com a mensagem de recusa.
fn no_data(p: &std::path::Path, bruto: &str, lang: Lang) {
    match detalhe::mesmo_volume(p, DATA) {
        Ok(true) => {}
        Ok(false) => sair(&lang.f("outro_volume", &[bruto])),
        Err(e) => sair(&e),
    }
}

fn main() {
    let a = args();
    let lang = a.lang;
    if matches!(a.modo, Modo::Ultima | Modo::Historico | Modo::Mudancas | Modo::Mcp) {
        historico::guarda(sys::e_root()).unwrap_or_else(|k| sair(&lang.t(k)));
    }
    // Panic numa etapa vira erro registrado; SDC_PANIC=1 (CI) deixa a mensagem no stderr.
    if std::env::var_os("SDC_PANIC").is_none() {
        std::panic::set_hook(Box::new(|_| {}));
    }
    // ---- segurança (síntese §4): obrigatórias, senão aborta ----
    if let Err((r1, r2)) = sys::politicas_seguranca() {
        eprintln!("sdtally: {}", lang.f("seguranca_falhou", &[&r1.to_string(), &r2.to_string()]));
        std::process::exit(1);
    }
    sys::qos_utility();
    if let Err(e) = sys::ao_interromper().and_then(|_| sys::ao_terminar()) {
        eprintln!("sdtally: {}", lang.f("sigaction_falhou", &[&e.to_string()]));
    }
    let root = sys::e_root();
    let (user, home) = coleta::identidade(root);
    let dono = coleta::nome_da_home(&user, &home).to_string();
    let cor = !a.json
        && match a.cor {
            Cor::Sempre => true,
            Cor::Nunca => false,
            Cor::Auto => std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty()),
        };
    let e = texto::Estilo { lang, binario: a.binario, cor, top: a.top };
    let ctx = a.redact.unwrap_or(matches!(a.modo, Modo::Mcp)).then(|| redigir::ctx_anonimo(&user));
    let red = ctx.as_ref().map(|c| (c, home.as_str()));
    let ctl = Arc::new(Controle::new(&sys::INTERROMPIDO));

    match &a.modo {
        Modo::Why(bruto) => {
            let p = std::path::absolute(bruto).unwrap_or_else(|x| sair(&format!("{bruto}: {x}")));
            no_data(&p, bruto, lang);
            match detalhe::why(&p) {
                Ok(w) if a.json => {
                    let mut v = report::json_why(&w, &home, &format!("sdtally {VERSAO}"));
                    catalogo::anexar(&mut v);
                    imprimir(v, None, false);
                }
                Ok(w) => escrever(&texto::why(&w, &home, &e)),
                Err(x) => sair(&x),
            }
        }
        Modo::Ramo(bruto) => {
            let p: PathBuf = std::fs::canonicalize(bruto).unwrap_or_else(|x| sair(&format!("{bruto}: {x}")));
            no_data(&p, bruto, lang);
            if !p.is_dir() {
                sair(&lang.f("nao_e_pasta", &[bruto]));
            }
            let threads = coleta::threads(None);
            let pr = progresso(ctl.clone(), lang);
            let r = detalhe::ramo(&p, a.depth, threads, ctl.clone());
            parar(pr);
            match r {
                Ok(r) if a.json => imprimir(report::json_ramo(&r, &home, &format!("sdtally {VERSAO}")), None, false),
                Ok(r) => escrever(&texto::ramo(&r, &home, &e)),
                Err(x) => sair(&x),
            }
        }
        Modo::Verificar => {
            let v = verify::verificar(root, &home).unwrap_or_else(|k| sair(&lang.t(k)));
            if a.json {
                imprimir(v.clone(), None, false);
            } else {
                escrever(&verify::texto(&v, lang));
            }
            if verify::algum_fora(&v) {
                std::process::exit(1);
            }
        }
        Modo::Catalogo => imprimir(catalogo::json(), None, false),
        // Cru, sem `anexar`: o que sai é o arquivo que o app salvou.
        Modo::Ultima | Modo::Historico | Modo::Mudancas => {
            let r = match a.modo {
                Modo::Ultima => historico::ultima(&home),
                Modo::Mudancas => historico::mudancas(&home, red),
                _ => historico::historico(&home),
            };
            match r {
                // Re-serializa: o que sai é o JSON lido, nunca os bytes do arquivo.
                Ok(v) => imprimir(v, red, false),
                Err(x) => sair(&x),
            }
        }
        Modo::Stdin => match redigir::ler_entrada(std::io::stdin().lock()) {
            Ok(v) => imprimir(v, red, false),
            Err(x) => sair(x),
        },
        Modo::Mcp => {
            let mut executar = |nome: &str| {
                let r = match nome {
                    "quick" => {
                        let (agora, _) = sys::now_local();
                        let data_hora: String = agora.chars().take(19).collect::<String>().replace('T', " ");
                        let r = coleta::coletar_rapido(root, &home, Arc::new(Ctx::real(&dono)), ctl.clone(), None, coleta::Partes::TODAS);
                        Ok(report::json_rapido(&r, &format!("sdtally {VERSAO}"), &data_hora, &home))
                    }
                    "last_measurement" => historico::ultima(&home),
                    "what_changed" => historico::mudancas(&home, red),
                    _ => historico::historico(&home),
                };
                mcp::fronteira(r, red)
            };
            match mcp::servir(std::io::stdin().lock(), std::io::stdout().lock(), &mut executar) {
                Err(x) if x.kind() == std::io::ErrorKind::BrokenPipe => {}
                Err(x) => sair(&x.to_string()),
                Ok(()) => {}
            }
        }
        Modo::Auto => {
            // Compacto, uma linha por mudança: o app lê linha a linha.
            let r = coleta::vigiar_arvore(root, &home, &format!("sdtally {VERSAO}"), Arc::new(Ctx::real(&dono)), ctl.clone(), |v| {
                escrever(&(serde_json::to_string(v).unwrap_or_default() + "\n"))
            });
            if let Err(x) = r {
                sair(&x);
            }
        }
        Modo::Conta if a.vigiar => {
            // Compacto, uma linha por tela: o app lê linha a linha. `--redact names` vale aqui também.
            let r = coleta::vigiar_tela(root, ctl.clone(), |r| {
                let (agora, _) = sys::now_local();
                let data_hora: String = agora.chars().take(19).collect::<String>().replace('T', " ");
                imprimir(report::json_rapido(r, &format!("sdtally {VERSAO}"), &data_hora, &home), red, true);
            });
            if let Err(x) = r {
                sair(&x);
            }
        }
        Modo::Conta if a.rapido => {
            let (agora, _) = sys::now_local();
            let data_hora: String = agora.chars().take(19).collect::<String>().replace('T', " ");
            let r = coleta::coletar_rapido(root, &home, Arc::new(Ctx::real(&dono)), ctl.clone(), a.desde, a.partes.unwrap_or(coleta::Partes::TODAS));
            let mut v = report::json_rapido(&r, &format!("sdtally {VERSAO}"), &data_hora, &home);
            catalogo::anexar(&mut v);
            imprimir(v, red, false);
        }
        Modo::Conta => {
            let (agora, _) = sys::now_local();
            let epoca = sys::agora_epoch();
            let data_hora: String = agora.chars().take(16).collect::<String>().replace('T', " ");
            let pr = progresso(ctl.clone(), lang);
            let c = coleta::coletar(&Opcoes { threads: None, xattr: true, root }, &home, Arc::new(Ctx::real(&dono)), ctl.clone());
            parar(pr);
            if a.json {
                let mut v = report::json_completo(&c, &format!("sdtally {VERSAO}"), &data_hora, &home, None);
                // A hora local não tem fuso: o `--last` ordena por esta (historico::todas).
                v["generated_at_epoch"] = epoca.into();
                imprimir(v, red, false);
            } else {
                escrever(&texto::conta(&report::montar(&c, &format!("sdtally {VERSAO}"), &data_hora), &e));
            }
        }
    }
    if ctl.cancelado() {
        std::process::exit(130);
    }
}

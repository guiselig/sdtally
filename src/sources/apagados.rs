//! Arquivos apagados e ainda abertos (lsof +L1; com root vê todos os processos).

use super::DATA;
use crate::anon::Ctx;
use crate::ledger::Medida;
use crate::sys;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

pub fn coletar(ctx: &Ctx, root: bool, err: &mut Vec<Value>) -> (Value, Medida) {
    let data_stdev = {
        use std::os::unix::fs::MetadataExt;
        match std::fs::metadata(DATA) {
            Ok(m) => m.dev(),
            // Sem o st_dev do Data nenhuma linha casaria: seria um 0 falso.
            Err(e) => {
                err.push(json!(format!("stat {DATA}: {e}")));
                return (Value::Null, Medida::NaoMedida(format!("stat {DATA}: {e}")));
            }
        }
    };
    // -b: sem stat/readlink nos pontos de montagem. Com root e backup do Time Machine em curso, o
    // stat no destino SMB travou o lsof até o timeout (30/09). A saída é a mesma (conferido);
    // -w cala o aviso de cada stat evitado. O -e por volume não existe no lsof 4.91 da Apple.
    let out = match sys::run_status("/usr/sbin/lsof", &["-b", "-w", "-nP", "+L1", "-F", "pcftDsin"], 60) {
        // lsof sai com 1 quando algum processo não pôde ser lido; aceitar a saída mesmo assim.
        Ok((_, o)) => o,
        Err(e) => {
            err.push(json!(e));
            return (Value::Null, Medida::NaoMedida(e));
        }
    };
    let dev_hex = format!("0x{:x}", data_stdev);
    let mut mapas: HashMap<i32, Vec<(u32, u64, i64)>> = HashMap::new();
    let (vistos, dv) = ler(&out, &dev_hex, |pid, fd, ino| {
        // Dispositivo e inode conferidos: o fd pode ter sido fechado e reusado depois do lsof.
        let mesmo = |x: &(u32, u64, i64)| x.0 as u64 == data_stdev && x.1 == ino;
        match fd.parse::<i32>() {
            Ok(n) => sys::fd_alocado(pid, n).ok().filter(mesmo).map(|x| x.2),
            Err(_) if fd == "txt" => mapas.entry(pid).or_insert_with(|| sys::mapeados_alocado(pid).unwrap_or_default()).iter().find(|x| mesmo(x)).map(|x| x.2),
            Err(_) => None,
        }
    });
    let soma: i64 = vistos.values().filter_map(|x| x.0).sum();
    let sem = vistos.values().filter(|x| x.0.is_none()).count();
    let logico: i64 = vistos.values().filter(|x| x.0.is_none()).map(|x| x.2).sum();
    let pp = por_processo(&vistos, &dv, |c| ctx.processo(c));
    // MB inteiro: o motivo é traduzido por trechos (report/i18n.rs) e não troca a vírgula decimal.
    let mb = format!("({} MB lógicos pelo lsof, fora da soma)", logico / 1_000_000);
    let m = match sem {
        0 => Medida::Valor(soma),
        _ if sem == vistos.len() => Medida::NaoMedida(format!("{sem} arquivos sem tamanho alocado legível {mb}")),
        _ => Medida::Parcial(soma, format!("{sem} de {} arquivos sem tamanho alocado legível: datavault do sistema, mapeados fora das regiões, já fechados ou de processo inacessível {mb}", vistos.len())),
    };
    let j = json!({
        "bytes": m.valor(),
        "estado": m.estado(),
        "arquivos": vistos.len(),
        "sem_alocado": sem,
        "logico_sem_alocado": logico,
        "por_processo": pp,
        "fonte": "lsof -b -w -nP +L1 acha os arquivos (regulares, no dispositivo do Data, deduplicados por (DEVICE, NODE)); alocado = st_blocks pelo proc_pidfdinfo (fd) ou PROC_PIDREGIONPATHINFO (mapeado, 'txt'); logico_sem_alocado = SIZE do lsof dos que ficaram sem alocado (datavault: stat zerado sem entitlement)",
        "cobertura": if root { "todos os processos (root)" } else { "só processos do próprio usuário (sem sudo o lsof não vê os de outros usuários e do sistema)" },
    });
    (j, m)
}

/// (DEVICE, NODE) → (alocado, processo, SIZE).
type Vistos = HashMap<(String, String), (Option<i64>, String, i64)>;
/// Processo → inodes únicos em com.apple.LaunchServices.dv, contados antes da deduplicação.
type Dv = HashMap<String, HashSet<(String, String)>>;

/// Lê a saída `-F pcftDsin` do lsof: regulares no dispositivo do Data, deduplicados por
/// (DEVICE, NODE) no primeiro processo (ordem de PID). O SIZE do lsof é lógico (um esparso de
/// 1 GiB daria 1 GiB): o alocado vem de `alocado(pid, fd, inode)`, ou None (já fechado ou
/// ilegível) e o SIZE fica só como informação, fora da soma. Ilegível inclui datavault
/// (com.apple.LaunchServices.dv, onde o chronod segura GBs de .csstore): o kernel zera o stat da
/// região para quem não tem com.apple.rootless.datavault.metadata (nem root); o lsof da Apple
/// tem esse entitlement e por isso mostra NODE e SIZE. O nome (campo n) só alimenta `Dv` e é
/// descartado na mesma linha.
/// Com locale C o lsof escreve os bytes fora do ASCII do nome do processo como `\xNN` ("jo\xc3\xa3o")
/// e toda barra invertida como `\\`: volta aos bytes, senão o nome escapa dos tokens do anonimizador.
fn sem_escape(v: &str) -> String {
    let b = v.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && b.get(i + 1) == Some(&b'\\') {
            out.push(b'\\');
            i += 2;
        } else if b[i] == b'\\' && b.get(i + 1) == Some(&b'x') && let Some(n) = v.get(i + 2..i + 4).and_then(|h| u8::from_str_radix(h, 16).ok()) {
            out.push(n);
            i += 4;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn ler(out: &str, dev_hex: &str, mut alocado: impl FnMut(i32, &str, u64) -> Option<i64>) -> (Vistos, Dv) {
    let mut vistos = Vistos::new();
    let mut dv = Dv::new();
    let (mut pid, mut cmd, mut fd) = (-1i32, String::new(), String::new());
    let (mut t, mut d, mut i, mut sz, mut e_dv) = (String::new(), String::new(), String::new(), String::new(), false);
    let mut flush = |pid: i32, fd: &str, t: &str, d: &str, i: &str, sz: &str, cmd: &str, e_dv: bool| {
        if t != "REG" || d != dev_hex || i.is_empty() {
            return;
        }
        if e_dv {
            dv.entry(cmd.to_string()).or_default().insert((d.to_string(), i.to_string()));
        }
        let e = vistos.entry((d.to_string(), i.to_string())).or_insert((None, cmd.to_string(), sz.parse().unwrap_or(0)));
        if e.0.is_none() {
            e.0 = alocado(pid, fd, i.parse().unwrap_or(0));
        }
    };
    for l in out.lines() {
        let (k, v) = l.split_at(l.len().min(1));
        match k {
            "p" | "f" => {
                flush(pid, &fd, &t, &d, &i, &sz, &cmd, e_dv);
                t.clear();
                d.clear();
                i.clear();
                sz.clear();
                e_dv = false;
                if k == "p" {
                    pid = v.parse().unwrap_or(-1);
                    fd.clear();
                } else {
                    fd = v.to_string();
                }
            }
            "c" => cmd = sem_escape(v),
            "t" => t = v.to_string(),
            "D" => d = v.to_string(),
            "i" => i = v.to_string(),
            "s" => sz = v.to_string(),
            "n" => e_dv = v.contains("com.apple.LaunchServices.dv"),
            _ => {}
        }
    }
    flush(pid, &fd, &t, &d, &i, &sz, &cmd, e_dv);
    (vistos, dv)
}

/// Os 20 processos que mais seguram, por max(alocado, lógico sem alocado): o chronod, só com
/// lógico (datavault), não some atrás de processos de alocado pequeno. Processo só com arquivos
/// sem alocado sai com bytes null, não 0.
fn por_processo(vistos: &Vistos, dv: &Dv, limpar: impl Fn(&str) -> String) -> Vec<Value> {
    // (bytes medidos, arquivos, sem alocado, SIZE dos sem alocado, inodes .dv)
    let mut porproc: HashMap<String, (i64, u64, u64, i64, usize)> = HashMap::new();
    for (b, c, s) in vistos.values() {
        let e = porproc.entry(limpar(c)).or_default();
        e.0 += b.unwrap_or(0);
        e.1 += 1;
        e.2 += u64::from(b.is_none());
        e.3 += if b.is_none() { *s } else { 0 };
    }
    for (c, set) in dv {
        porproc.entry(limpar(c)).or_default().4 += set.len();
    }
    let mut pp: Vec<_> = porproc.into_iter().filter(|x| x.1.1 > 0).collect();
    // Empate pelo nome: a saída não depende da ordem do HashMap.
    pp.sort_by(|a, b| b.1.0.max(b.1.3).cmp(&a.1.0.max(a.1.3)).then_with(|| a.0.cmp(&b.0)));
    pp.into_iter()
        .take(20)
        .map(|(n, (b, q, sem, l, dv))| json!({"processo": n, "bytes": (sem < q).then_some(b), "arquivos": q, "sem_alocado": sem, "logico_sem_alocado": l, "launchservices_dv": dv}))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_com_escape_do_lsof_volta_aos_bytes() {
        assert_eq!(sem_escape("jo\\xc3\\xa3o-backup"), "joão-backup");
        assert_eq!(sem_escape("a\\xZZ\\x4"), "a\\xZZ\\x4", "escape inválido fica como veio");
        assert_eq!(sem_escape("chronod"), "chronod");
        assert_eq!(sem_escape(r"a\\x41b\\\\c"), r"a\x41b\\c", "barra literal dobrada pelo lsof");
        assert_eq!(sem_escape(r"\\xfabiana-sync"), r"\xfabiana-sync");
    }

    const GB: i64 = 1_000_000_000;

    #[test]
    fn chronod_fora_do_top_antigo() {
        let mut v = Vistos::new();
        for p in 0..10 {
            v.insert(("0x1".into(), format!("{p}")), (Some(1_000_000 + p), format!("p{p}"), 0));
        }
        v.insert(("0x1".into(), "99".into()), (None, "chronod".into(), 39 * GB / 10));
        let pp = por_processo(&v, &Dv::new(), str::to_string);
        assert_eq!(pp[0]["processo"], "chronod");
        assert!(pp[0]["bytes"].is_null() && pp[0]["logico_sem_alocado"] == 39 * GB / 10);
        assert!(pp.iter().take(10).any(|p| p["processo"] == "chronod"), "o report corta em 10");
    }

    #[test]
    fn dv_conta_antes_da_dedup() {
        // PID 10 (menor) abre o mesmo inode .dv que o chronod (PID 20): a soma fica com o PID 10,
        // a contagem de .dv conta para os dois.
        let out = "p10\ncoutro\nf3\ntREG\nD0x1\ns500\ni77\nn/private/var/folders/xx/0/com.apple.LaunchServices.dv/a.csstore\n\
                   p20\ncchronod\nf4\ntREG\nD0x1\ns500\ni77\nn/private/var/folders/xx/0/com.apple.LaunchServices.dv/a.csstore\n\
                   f5\ntREG\nD0x1\ns900\ni78\nn/private/var/folders/xx/0/com.apple.LaunchServices.dv/b.csstore\n\
                   f6\ntREG\nD0x1\ns100\ni79\nn/Users/exemplo/tmp/x\nf7\ntREG\nD0x2\ns100\ni80\nn/x/com.apple.LaunchServices.dv/c\n";
        let (vistos, dv) = ler(out, "0x1", |_, _, _| None);
        assert_eq!(vistos.len(), 3, "D0x2 fica de fora; 77 deduplicado");
        assert_eq!(vistos[&("0x1".to_string(), "77".to_string())].1, "outro");
        let pp = por_processo(&vistos, &dv, str::to_string);
        let p = |n: &str| pp.iter().find(|p| p["processo"] == n).cloned().unwrap_or_default();
        assert_eq!(p("chronod")["launchservices_dv"], 2);
        assert_eq!(p("chronod")["arquivos"], 2);
        assert_eq!(p("outro")["launchservices_dv"], 1);
        // O nome do arquivo não sai em lugar nenhum.
        let s = serde_json::to_string(&pp).unwrap_or_default();
        assert!(!s.contains("csstore") && !s.contains("exemplo"), "{s}");
    }
}

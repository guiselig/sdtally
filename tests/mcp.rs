//! Acesso por IA (spec da fase 5, §4.5 e marco R3): MCP por stdio, `--last` e `--history` sobre
//! medições sintéticas com /Users/exemplo numa home temporária.

use sdtally::historico;
use sdtally::mcp::{self, AVISO, SUPORTADAS};
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_sdtally");
const SEGREDO: &str = "SEGREDO-NAO-VAZA";

fn pedir(linha: &str) -> Option<Value> {
    mcp::responder(linha, &mut |n: &str| Ok(json!({"ferramenta": n})))
}

fn codigo(r: &Option<Value>) -> i64 {
    r.as_ref().expect("resposta")["error"]["code"].as_i64().expect("código")
}

#[test]
fn initialize_ecoa_suportada_e_troca_desconhecida() {
    for v in SUPORTADAS {
        let r = pedir(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": v}}).to_string()).unwrap();
        assert_eq!(r["result"]["protocolVersion"], v);
    }
    let r = pedir(r#"{"jsonrpc":"2.0","id":"a","method":"initialize","params":{"protocolVersion":"1900-01-01"}}"#).unwrap();
    assert_eq!(r["id"], "a");
    assert_eq!(r["result"]["protocolVersion"], *SUPORTADAS.last().unwrap());
    assert_eq!(r["result"]["serverInfo"]["name"], "sdtally");
    assert_eq!(r["result"]["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(r["result"]["capabilities"]["tools"].is_object());
}

#[test]
fn notificacao_sem_resposta_e_ping() {
    assert_eq!(pedir(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#), None);
    assert_eq!(pedir(r#"{"jsonrpc":"2.0","method":"metodo/desconhecido"}"#), None);
    let r = pedir(r#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#).unwrap();
    assert_eq!(r, json!({"jsonrpc": "2.0", "id": 7, "result": {}}));
}

#[test]
fn codigos_de_erro() {
    assert_eq!(codigo(&pedir(r#"{"jsonrpc":"2.0","id":1,"method":"resources/list"}"#)), -32601);
    assert_eq!(codigo(&pedir("{nao é json")), -32700);
    assert_eq!(codigo(&pedir("[1,2]")), -32600);
    for args in [json!({"x": 1}), json!([]), json!("a")] {
        let l = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "quick", "arguments": args}});
        assert_eq!(codigo(&pedir(&l.to_string())), -32602, "{args}");
    }
    assert_eq!(codigo(&pedir(r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"rm"}}"#)), -32602);
}

#[test]
fn tools_list_tres_com_schema_e_aviso() {
    let r = pedir(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).unwrap();
    let t = r["result"]["tools"].as_array().unwrap();
    let nomes: Vec<&str> = t.iter().map(|x| x["name"].as_str().unwrap()).collect();
    assert_eq!(nomes, ["quick", "last_measurement", "history"]);
    for x in t {
        assert_eq!(x["inputSchema"], json!({"type": "object", "properties": {}, "additionalProperties": false}));
        assert!(x["description"].as_str().unwrap().contains(AVISO));
    }
    assert!(AVISO.starts_with("Read-only."));
}

#[test]
fn tools_call_devolve_texto_json() {
    let r = pedir(r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"history","arguments":{}}}"#).unwrap();
    let c = &r["result"]["content"][0];
    assert_eq!(c["type"], "text");
    assert_eq!(serde_json::from_str::<Value>(c["text"].as_str().unwrap()).unwrap(), json!({"ferramenta": "history"}));
    let e = mcp::responder(r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"last_measurement"}}"#, &mut |_: &str| Err("sem medição".into())).unwrap();
    assert_eq!(e["result"]["isError"], true);
}

#[test]
fn linha_comprida_recusada_e_laco_segue() {
    let mut entrada = vec![b'x'; mcp::LIMITE_LINHA + 10];
    entrada.extend_from_slice(b"\n{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"ping\"}\n");
    let mut saida = Vec::new();
    mcp::servir(&entrada[..], &mut saida, &mut |_: &str| Ok(Value::Null)).unwrap();
    let linhas: Vec<Value> = String::from_utf8(saida).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(linhas.len(), 2);
    assert_eq!(linhas[0]["error"]["code"], -32600);
    assert_eq!(linhas[1]["id"], 9);
}

#[test]
fn recusa_como_root() {
    assert!(historico::guarda(true).is_err());
    assert!(historico::guarda(false).is_ok());
}

// ---- pelo binário, com HOME numa pasta temporária ----

fn home(nome: &str) -> PathBuf {
    let h = Path::new(env!("CARGO_TARGET_TMPDIR")).join(nome);
    let _ = std::fs::remove_dir_all(&h);
    std::fs::create_dir_all(historico::pasta(h.to_str().unwrap())).unwrap();
    h
}

fn medicao(gerada: &str, tool: &str, root: bool) -> Value {
    json!({
        "schema_version": 1, "home": "/Users/exemplo", "tool": tool, "generated_at": gerada,
        "complete": root, "root": root, "full_disk_access": true,
        "data_volume": {"used": {"bytes": 400_000_000_000_i64}, "unexplained": {"bytes": 5_000_000_000_i64, "percent_of_used": 1.25}},
        "top": [{"path": "~/Library/Caches", "folded": false}],
    })
}

fn gravar(h: &Path, nome: &str, v: &Value) {
    std::fs::write(historico::pasta(h.to_str().unwrap()).join(nome), serde_json::to_vec_pretty(v).unwrap()).unwrap();
}

/// Arquivos recusados, cada um com o segredo dentro: schema 2 (o mais novo), symlink e 21 MB.
fn gravar_ruins(h: &Path) {
    let d = historico::pasta(h.to_str().unwrap());
    let mut s2 = medicao("2026-12-31 23:59", "sdtally 9.9", true);
    s2["schema_version"] = json!(2);
    s2["home"] = json!(SEGREDO);
    gravar(h, "2026-12-31-2359.json", &s2);
    let alvo = h.join("fora.json");
    let mut f = medicao("2027-01-01 00:00", SEGREDO, true);
    f["home"] = json!(SEGREDO);
    std::fs::write(&alvo, f.to_string()).unwrap();
    std::os::unix::fs::symlink(&alvo, d.join("2027-01-01-0000.json")).unwrap();
    let mut g = std::fs::File::create(d.join("2027-01-02-0000.json")).unwrap();
    write!(g, "{{\"schema_version\":1,\"generated_at\":\"2027-01-02 00:00\",\"home\":\"{SEGREDO}\"}}").unwrap();
    g.set_len(21_000_000).unwrap();
}

fn rodar(h: &Path, args: &[&str], entrada: &str) -> Output {
    let mut c = Command::new(BIN).args(args).env("HOME", h).env("LANG", "en_US.UTF-8").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    c.stdin.take().unwrap().write_all(entrada.as_bytes()).unwrap();
    c.wait_with_output().unwrap()
}

fn sem_segredo(o: &Output) {
    assert!(!String::from_utf8_lossy(&o.stdout).contains(SEGREDO), "segredo no stdout");
    assert!(!String::from_utf8_lossy(&o.stderr).contains(SEGREDO), "segredo no stderr");
}

#[test]
fn mcp_pelo_binario_tres_linhas_jsonrpc() {
    let h = home("mcp-tres");
    let e = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#,
    ];
    let o = rodar(&h, &["--mcp"], &(e.join("\n") + "\n"));
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let s = String::from_utf8(o.stdout).unwrap();
    let linhas: Vec<Value> = s.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(linhas.len(), 3, "{s}");
    assert_eq!(linhas[2]["result"]["tools"].as_array().unwrap().len(), 3);
}

#[test]
fn mcp_pelo_binario_todo_stdout_e_jsonrpc() {
    let h = home("mcp-stdout");
    gravar(&h, "2026-10-01-0900.json", &medicao("2026-10-01 09:00", "sdtally 0.1.0", true));
    gravar_ruins(&h);
    let e = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"last_measurement","arguments":{}}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"history"}}"#,
        "lixo",
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"history","arguments":{"n":1}}}"#,
    ];
    let o = rodar(&h, &["--mcp"], &(e.join("\n") + "\n"));
    assert!(o.status.success());
    sem_segredo(&o);
    let s = String::from_utf8(o.stdout.clone()).unwrap();
    let linhas: Vec<Value> = s.lines().map(|l| serde_json::from_str(l).expect("linha não é JSON")).collect();
    assert_eq!(linhas.len(), 5);
    for l in &linhas {
        assert_eq!(l["jsonrpc"], "2.0");
        assert!(l.get("result").is_some() != l.get("error").is_some());
    }
    let ultima: Value = serde_json::from_str(linhas[1]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(ultima["generated_at"], "2026-10-01 09:00");
    assert_eq!(linhas[3]["error"]["code"], -32700);
    assert_eq!(linhas[4]["error"]["code"], -32602);
}

/// mtime do arquivo, em segundos desde a época.
fn mtime(h: &Path, nome: &str, epoca: u64) {
    let f = std::fs::File::options().write(true).open(historico::pasta(h.to_str().unwrap()).join(nome)).unwrap();
    f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(epoca)).unwrap();
}

#[test]
fn last_ordena_por_generated_at_e_recusa_sem_vazar() {
    let h = home("last");
    // O nome mais antigo guarda a medição mais nova: sem `generated_at_epoch`, vale o mtime.
    gravar(&h, "2026-09-01-0000.json", &medicao("2026-10-02 08:00", "sdtally 0.1.0", true));
    gravar(&h, "2026-10-01-0900.json", &medicao("2026-10-01 09:00", "sdtally 0.1.0", false));
    mtime(&h, "2026-09-01-0000.json", 1_790_938_800);
    mtime(&h, "2026-10-01-0900.json", 1_790_856_000);
    gravar_ruins(&h);
    let o = rodar(&h, &["--last", "--json"], "");
    assert!(o.status.success());
    sem_segredo(&o);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v, medicao("2026-10-02 08:00", "sdtally 0.1.0", true));
    let err = String::from_utf8_lossy(&o.stderr);
    for motivo in ["schema_version is not 1", "symlink", "larger than 20 MB"] {
        assert!(err.contains(motivo), "faltou recusa {motivo}: {err}");
    }

    let o = rodar(&h, &["--history", "--json"], "");
    assert!(o.status.success());
    sem_segredo(&o);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["kind"], "history");
    assert_eq!(v["schema_version"], 1);
    let m = v["measurements"].as_array().unwrap();
    assert_eq!(m.len(), 2);
    assert_eq!(m[0]["generated_at"], "2026-10-01 09:00");
    assert_eq!(m[1]["generated_at"], "2026-10-02 08:00");
    assert_eq!(m[1]["used"], 400_000_000_000_i64);
    assert_eq!(m[1]["unexplained"], 5_000_000_000_i64);
    assert_eq!(m[1]["percent_of_used"], 1.25);
    for k in ["tool", "root", "full_disk_access", "complete"] {
        assert!(m[0].get(k).is_some(), "{k}");
    }
}

#[test]
fn last_so_com_recusados_falha_sem_vazar() {
    let h = home("so-ruins");
    gravar_ruins(&h);
    let o = rodar(&h, &["--last", "--json"], "");
    assert!(!o.status.success());
    assert!(o.stdout.is_empty());
    sem_segredo(&o);
}

/// Fim do horário de verão (ou viagem para o oeste): a hora local volta. 01:50 e depois 01:10 do
/// mesmo dia; vale a `generated_at_epoch`, não o texto. Arquivo antigo, sem ela, entra pelo mtime.
#[test]
fn last_ordena_pela_epoca_e_nao_pela_hora_local() {
    let h = home("last-epoca");
    let mut antes = medicao("2026-11-01 01:50", "sdtally 0.1.0", true);
    antes["generated_at_epoch"] = json!(1_793_515_800_i64);
    let mut depois = medicao("2026-11-01 01:10", "sdtally 0.1.0", false);
    depois["generated_at_epoch"] = json!(1_793_517_000_i64);
    gravar(&h, "2026-11-01-0150.json", &antes);
    gravar(&h, "2026-11-01-0110.json", &depois);
    // Sem época, com mtime de antes das duas.
    gravar(&h, "2026-10-31-2300.json", &medicao("2026-10-31 23:00", "sdtally 0.0.9", true));
    mtime(&h, "2026-10-31-2300.json", 1_793_500_000);
    let o = rodar(&h, &["--last", "--json"], "");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["generated_at"], "2026-11-01 01:10");
    let o = rodar(&h, &["--history", "--json"], "");
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    let g: Vec<&str> = v["measurements"].as_array().unwrap().iter().map(|m| m["generated_at"].as_str().unwrap()).collect();
    assert_eq!(g, ["2026-10-31 23:00", "2026-11-01 01:50", "2026-11-01 01:10"]);
}

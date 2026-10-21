//! Atribuição a apps (PLANO Fase 2, Frente C §1.2 e §2): para cada linha de `top`, o app dono
//! quando há fonte no disco; sem fonte, nada (a chave `owner` nem sai). Só leitura: Info.plist dos
//! apps instalados, xattr `com.apple.containermanager.identifier` do container e a assinatura
//! (entitlements) do executável. Sem processo externo, sem rede, sem listar iCloud/FileProvider.
//!
//! Fontes (`owner.source`), da mais firme para a mais fraca:
//! - `app_bundle`: o caminho está dentro de um `.app`; o dono é o Info.plist dele.
//! - `container`: `~/Library/Containers/<x>`; o id é o que o containermanagerd gravou no xattr.
//! - `group_container`: `~/Library/Group Containers/<g>`; um único app instalado declara o grupo
//!   nos entitlements da assinatura. Grupo de vários apps (Office) fica sem dono.
//! - `folder_bundle_id`: `Application Support|Caches|Logs|HTTPStorages|WebKit/<id>` com o nome da
//!   pasta igual ao bundle id de um app instalado.
//! - `icloud_container`: `Mobile Documents/<id com ~>` igual ao bundle id de um app instalado (só o
//!   nome da pasta; o conteúdo nunca é aberto).
//! - `folder_app_name`: o nome da pasta é igual (sem maiúsculas) ao nome de um único app instalado.
//!   É a mais fraca: dois apps com o mesmo nome, ou nome diferente, e nada é atribuído.

use crate::{assinatura, plist, sys};
use serde_json::{Value, json};
use std::cell::OnceCell;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

const XATTR_ID: &str = "com.apple.containermanager.identifier";
/// Onde ficam apps instalados (relativo à raiz do sistema); mais `~/Applications`.
const PASTAS_APPS: [&str; 5] =
    ["Applications", "Applications/Utilities", "System/Applications", "System/Applications/Utilities", "System/Cryptexes/App/System/Applications"];
/// Pastas de `~/Library` cujas subpastas costumam ter o bundle id ou o nome do app.
const PASTAS_LIBRARY: [&str; 5] = ["Application Support", "Caches", "Logs", "HTTPStorages", "WebKit"];
const PLIST_MAX: u64 = 4 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dono {
    pub bundle_id: String,
    /// None: o id é conhecido, mas nenhum app instalado nas pastas de apps tem esse id.
    pub nome: Option<String>,
    pub fonte: &'static str,
}

#[derive(Debug, Clone)]
struct App {
    id: String,
    /// Nome do arquivo sem `.app`: o que o Finder mostra.
    nome: String,
    /// Minúsculas: o nome do arquivo, CFBundleDisplayName e CFBundleName.
    nomes: Vec<String>,
    dir: PathBuf,
    exe: Option<String>,
}

/// Resolve donos de caminhos lógicos (`~/...`, `/Applications/...`). Os índices de apps e de grupos
/// são montados na primeira vez que uma regra precisa deles.
pub struct Donos {
    home: PathBuf,
    /// Raiz do sistema de arquivos ("/"; uma pasta falsa nos testes).
    raiz: PathBuf,
    apps: OnceCell<Vec<App>>,
    grupos: OnceCell<HashMap<String, Vec<usize>>>,
}

/// Bundle id e id de grupo são reverse-DNS: só estes caracteres. O resto (invisível, `/`, controle)
/// vem de disco hostil e não vira dono.
fn id_valido(s: &str) -> bool {
    (1..=200).contains(&s.len()) && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

fn app_de(dir: &Path) -> Option<App> {
    let mut b = Vec::new();
    // App de iOS no Mac: o Info.plist fica no pacote embrulhado (Wrapper/<nome>.app).
    let (f, exe) = match std::fs::File::open(dir.join("Contents/Info.plist")) {
        Ok(f) => (f, true),
        Err(_) => (std::fs::File::open(dir.join("WrappedBundle/Info.plist")).ok()?, false),
    };
    f.take(PLIST_MAX).read_to_end(&mut b).ok()?;
    let p = plist::ler(&b)?;
    let txt = |k: &str| p.get(k).and_then(plist::V::texto).filter(|s| !s.is_empty()).map(str::to_string);
    let nome = dir.file_stem()?.to_str()?.to_string();
    let nomes = [Some(nome.clone()), txt("CFBundleDisplayName"), txt("CFBundleName")].into_iter().flatten().map(|n| n.to_lowercase()).collect();
    let id = txt("CFBundleIdentifier").filter(|i| id_valido(i))?;
    Some(App { id, nome, nomes, dir: dir.to_path_buf(), exe: txt("CFBundleExecutable").filter(|_| exe) })
}

impl Donos {
    pub fn new(home: &str) -> Donos {
        Donos::em("/".as_ref(), home)
    }

    /// Com a raiz do sistema trocada (testes): `~` é `home` e `/X` é `raiz/X`.
    pub fn em(raiz: &Path, home: &str) -> Donos {
        Donos { home: home.into(), raiz: raiz.into(), apps: OnceCell::new(), grupos: OnceCell::new() }
    }

    fn real(&self, p: &str) -> PathBuf {
        match p.strip_prefix('~') {
            Some(r) => self.home.join(r.trim_start_matches('/')),
            None => self.raiz.join(p.trim_start_matches('/')),
        }
    }

    /// Apps das pastas de apps, na ordem delas; o primeiro de cada bundle id fica.
    fn apps(&self) -> &[App] {
        self.apps.get_or_init(|| {
            let mut pastas: Vec<PathBuf> = PASTAS_APPS.iter().map(|p| self.raiz.join(p)).collect();
            pastas.push(self.home.join("Applications"));
            let mut v: Vec<App> = Vec::new();
            for pasta in pastas {
                let mut nomes: Vec<PathBuf> = std::fs::read_dir(&pasta).into_iter().flatten().flatten().map(|e| e.path()).collect();
                nomes.sort();
                for d in nomes.into_iter().filter(|d| d.extension().is_some_and(|x| x == "app")) {
                    if let Some(a) = app_de(&d)
                        && !v.iter().any(|x| x.id == a.id)
                    {
                        v.push(a);
                    }
                }
            }
            v
        })
    }

    fn por_id(&self, id: &str) -> Option<&App> {
        self.apps().iter().find(|a| a.id == id)
    }

    /// O único app com esse nome; dois bundle ids diferentes com o mesmo nome = ambíguo = nenhum.
    fn por_nome(&self, nome: &str) -> Option<&App> {
        let n = nome.to_lowercase();
        let mut it = self.apps().iter().filter(|a| a.nomes.contains(&n));
        let a = it.next()?;
        it.all(|b| b.id == a.id).then_some(a)
    }

    /// Grupo de app → índices dos apps que o declaram na assinatura do executável principal.
    fn por_grupo(&self, g: &str) -> Vec<&App> {
        let m = self.grupos.get_or_init(|| {
            let mut m: HashMap<String, Vec<usize>> = HashMap::new();
            for (i, a) in self.apps().iter().enumerate() {
                let Some(exe) = &a.exe else { continue };
                for g in assinatura::grupos(&a.dir.join("Contents/MacOS").join(exe)) {
                    m.entry(g).or_default().push(i);
                }
            }
            m
        });
        m.get(g).into_iter().flatten().map(|i| &self.apps()[*i]).collect()
    }

    /// O id gravado pelo containermanagerd na pasta do container.
    fn id_do_container(&self, p: &str) -> Option<String> {
        let v = sys::apfs::xattr_valor(&self.real(p), XATTR_ID)?;
        let s = String::from_utf8(v).ok()?;
        let s = s.trim_end_matches('\0').trim();
        id_valido(s).then(|| s.to_string())
    }

    /// Dono de uma linha do `top` pelo caminho lógico. None = sem fonte.
    pub fn dono(&self, p: &str) -> Option<Dono> {
        let c: Vec<&str> = p.split('/').collect();
        let app = |a: &App, fonte| Dono { bundle_id: a.id.clone(), nome: Some(a.nome.clone()), fonte };
        match c.as_slice() {
            // /Applications/<...>/X.app/... ou ~/Applications/X.app/...: o primeiro .app do caminho.
            ["" | "~", "Applications", rest @ ..] => {
                let i = rest.iter().position(|x| x.ends_with(".app"))?;
                let dir = self.real(&c[..i + 3].join("/"));
                app_de(&dir).map(|a| app(&a, "app_bundle"))
            }
            ["~", "Library", "Containers", x, ..] => {
                let id = self.id_do_container(&format!("~/Library/Containers/{x}"))?;
                match self.por_id(&id) {
                    Some(a) => Some(app(a, "container")),
                    // Id da Apple fora das pastas de apps: serviço do sistema, não app.
                    None if id.starts_with("com.apple.") => None,
                    None => Some(Dono { bundle_id: id, nome: None, fonte: "container" }),
                }
            }
            ["~", "Library", "Group Containers", x, ..] => {
                let g = self.id_do_container(&format!("~/Library/Group Containers/{x}"))?;
                match self.por_grupo(&g).as_slice() {
                    [a] => Some(app(a, "group_container")),
                    _ => None,
                }
            }
            ["~", "Library", "Mobile Documents", x, ..] => {
                let p: Vec<&str> = x.split('~').collect();
                // `iCloud~<id>` e `<TEAM>~<id>` perdem o prefixo; `com~apple~CloudDocs` é do sistema.
                let id = match p.as_slice() {
                    ["iCloud", r @ ..] if r.len() >= 2 => r.join("."),
                    [t, r @ ..] if t.len() == 10 && t.chars().all(|c| c.is_ascii_alphanumeric()) && r.len() >= 2 => r.join("."),
                    _ if p.len() >= 3 => p.join("."),
                    _ => return None,
                };
                self.por_id(&id).map(|a| app(a, "icloud_container"))
            }
            ["~", "Library", sub, x, resto @ ..] if PASTAS_LIBRARY.contains(sub) => {
                if let Some(a) = self.por_id(x) {
                    return Some(app(a, "folder_bundle_id"));
                }
                // "Google/Chrome" → app "Google Chrome": a pasta do fabricante + a do produto.
                self.por_nome(x).or_else(|| self.por_nome(&format!("{x} {}", resto.first()?))).map(|a| app(a, "folder_app_name"))
            }
            _ => None,
        }
    }
}

/// `top[].owner` em toda linha não dobrada que tem dono: `{bundle_id, name, source}`. Só com `home`
/// na raiz do JSON (o completo). Linha sem fonte fica sem a chave; `name` nulo = id conhecido, app
/// fora das pastas de apps.
pub fn anexar(v: &mut Value) {
    let Some(home) = v["home"].as_str().map(str::to_string) else { return };
    anexar_em(v, &Donos::new(&home));
}

pub fn anexar_em(v: &mut Value, d: &Donos) {
    for x in v.pointer_mut("/top").and_then(Value::as_array_mut).into_iter().flatten().filter(|x| x.is_object()) {
        if x["folded"] == true {
            continue;
        }
        if let Some(o) = x["path"].as_str().and_then(|p| d.dono(p)) {
            x["owner"] = json!({"bundle_id": o.bundle_id, "name": o.nome, "source": o.fonte});
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::assinatura::testes::{macho, superblob};

    const GRUPOS: &str = "<plist><dict><key>com.apple.security.application-groups</key><array><string>UBF8T346G9.Suite</string></array></dict></plist>";

    fn tmp(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sdtally-owners-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn app(raiz: &Path, rel: &str, id: &str, exe: &[u8]) {
        let c = raiz.join(rel).join("Contents");
        std::fs::create_dir_all(c.join("MacOS")).unwrap();
        let p = format!("<plist><dict><key>CFBundleIdentifier</key><string>{id}</string><key>CFBundleExecutable</key><string>bin</string></dict></plist>");
        std::fs::write(c.join("Info.plist"), p).unwrap();
        std::fs::write(c.join("MacOS/bin"), exe).unwrap();
    }

    fn container(home: &Path, rel: &str, id: &str) {
        let d = home.join(rel);
        std::fs::create_dir_all(&d).unwrap();
        let ok = std::process::Command::new("/usr/bin/xattr").args(["-w", XATTR_ID, id]).arg(&d).status().unwrap().success();
        assert!(ok, "xattr -w");
    }

    #[test]
    fn grupo_de_um_so_app_tem_dono_e_de_varios_nao() {
        let r = tmp("grupo");
        let home = r.join("home");
        let assinado = macho(&superblob(GRUPOS));
        app(&r, "Applications/Suite.app", "com.exemplo.Suite", &assinado);
        app(&r, "Applications/Sozinho.app", "com.exemplo.Sozinho", &macho(&superblob(&GRUPOS.replace("Suite", "Sozinho"))));
        app(&r, "Applications/Irmao.app", "com.exemplo.Irmao", &assinado);
        container(&home, "Library/Group Containers/UBF8T346G9.Suite", "UBF8T346G9.Suite");
        container(&home, "Library/Group Containers/UBF8T346G9.Sozinho", "UBF8T346G9.Sozinho");
        container(&home, "Library/Group Containers/UBF8T346G9.Orfao", "UBF8T346G9.Orfao");
        let d = Donos::em(&r, home.to_str().unwrap());
        let g = |x: &str| d.dono(&format!("~/Library/Group Containers/{x}/Library"));
        assert_eq!(g("UBF8T346G9.Sozinho"), Some(Dono { bundle_id: "com.exemplo.Sozinho".into(), nome: Some("Sozinho".into()), fonte: "group_container" }));
        // Suite e Irmao declaram o mesmo grupo: dono ambíguo, nenhum.
        assert_eq!(g("UBF8T346G9.Suite"), None);
        assert_eq!(g("UBF8T346G9.Orfao"), None);
        std::fs::remove_dir_all(&r).ok();
    }

    #[test]
    fn id_hostil_nao_vira_dono() {
        assert!(id_valido("com.exemplo.App-2_b"));
        for ruim in ["", "a/b", "com.x\u{202E}y", "com.x\ny", "com exemplo", &"a".repeat(201)] {
            assert!(!id_valido(ruim), "{ruim:?}");
        }
        let r = tmp("hostil");
        app(&r, "Applications/Ruim.app", "com.exemplo.\u{202E}Ruim", b"");
        let d = Donos::em(&r, r.join("home").to_str().unwrap());
        assert_eq!(d.dono("/Applications/Ruim.app/Contents"), None);
        std::fs::remove_dir_all(&r).ok();
    }
}

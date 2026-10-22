//! Caminho → balde. No modo anônimo (sdc-coleta) nenhum nome de arquivo sai daqui: só pastas,
//! cortadas pelas regras abaixo, e qualquer componente com @, usuário, nome ou host vira <privado>.
//! O sdtally usa as mesmas regras com os nomes reais, ou baldes relativos a um ramo.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    Anonimo,
    /// Mesmas regras de corte, nomes reais (a saída fica só na tela de quem roda).
    Real,
    /// Primeiros N componentes abaixo da raiz da travessia (`sdtally <caminho> --depth N`).
    Ramo(usize),
}

pub struct Ctx {
    pub user: String,
    /// Tokens proibidos, minúsculos: usuário, palavras do nome completo, nomes do host.
    pub tokens: Vec<String>,
    pub modo: Modo,
}

impl Ctx {
    pub fn new(user: &str, full_name: &str, hosts: &[String]) -> Ctx {
        let mut tokens: Vec<String> = Vec::new();
        let mut add = |s: &str| {
            let s = dobrar(s.trim());
            // Só dígitos (kern.hostname vira o IP em algumas redes): casaria com qualquer número.
            if s.chars().count() >= 3 && !s.chars().all(|c| c.is_ascii_digit()) && !tokens.contains(&s) {
                tokens.push(s);
            }
        };
        add(user);
        add(full_name);
        for w in full_name.split(|c: char| !c.is_alphanumeric()) {
            add(w);
        }
        // Nome colado ("analima"): com 5 letras ou mais casa em qualquer lugar, mesmo quando cada
        // palavra do nome é curta demais para isso.
        add(&full_name.split(|c: char| !c.is_alphanumeric()).collect::<String>());
        for h in hosts {
            add(h);
            // Num FQDN só o primeiro rótulo é da máquina; os outros são do provedor ("virtua",
            // "user", "dynamic") e apagariam com.apple.Virtualization, Code/User...
            let maquina = h.split('.').next().unwrap_or(h);
            for w in maquina.split(|c: char| !c.is_alphanumeric()) {
                // "MacBook", "Air", "Pro" são genéricos, e rótulos de DNS do kern.hostname ("com",
                // "home") casariam com com.apple.*: só palavras que não são de modelo nem de rede.
                const GENERICOS: [&str; 17] = ["macbook", "imac", "mac", "mini", "studio", "pro", "air", "local", "com", "net", "org", "edu", "gov", "lan", "home", "localdomain", "corp"];
                if !GENERICOS.contains(&w.to_lowercase().as_str()) {
                    add(w);
                }
            }
        }
        Ctx { user: user.to_string(), tokens, modo: Modo::Anonimo }
    }

    pub fn real(user: &str) -> Ctx {
        Ctx { user: user.to_string(), tokens: Vec::new(), modo: Modo::Real }
    }

    pub fn ramo(profundidade: usize) -> Ctx {
        Ctx { user: String::new(), tokens: Vec::new(), modo: Modo::Ramo(profundidade.max(1)) }
    }

    /// Texto externo (nome de processo, serviço, categoria) → seguro para o JSON.
    pub fn clean(&self, s: &str) -> String {
        if self.modo != Modo::Anonimo {
            return s.to_string();
        }
        if is_uuid(s) {
            return "<uuid>".into();
        }
        if self.achados(s) > 0 {
            "<privado>".into()
        } else {
            s.to_string()
        }
    }

    /// Quantos tokens (mais o @) aparecem em `s`, comparando sem acento e também com a troca de
    /// minúscula para Maiúscula como fronteira de palavra ("backupAna", "AnaLima").
    pub fn achados(&self, s: &str) -> usize {
        let (a, b) = (dobrar(s), dobrar(&separar_camel(s)));
        self.tokens.iter().filter(|t| tem_token(&a, t) || tem_token(&b, t)).count() + usize::from(s.contains('@'))
    }

    /// Nome de processo: proc_name e o lsof cortam em 31 bytes, e o fim pode ser um pedaço de nome
    /// ("...-acme-guilherm") que nenhum token pega. Nome cortado perde o último trecho.
    pub fn processo(&self, n: &str) -> String {
        if self.modo != Modo::Anonimo {
            return n.to_string();
        }
        if n.len() < 31 {
            return self.clean(n);
        }
        match n.rfind(['.', '-', '_', ' ']) {
            Some(i) if i > 0 => self.clean(&format!("{}…", &n[..=i])),
            _ => "<privado>".into(),
        }
    }

    /// Caminho lógico (sem o prefixo /System/Volumes/Data) → balde.
    pub fn bucket(&self, logical: &str) -> String {
        let c: Vec<&str> = logical.split('/').filter(|x| !x.is_empty()).collect();
        if let Modo::Ramo(n) = self.modo {
            return if c.is_empty() { "/".into() } else { c[..n.min(c.len())].join("/") };
        }
        let p = |v: &[&str], root: &str, depth: usize| -> String {
            let mut s = root.to_string();
            for x in v.iter().take(depth) {
                s.push('/');
                s.push_str(&self.clean(x));
            }
            s
        };
        match c.as_slice() {
            [] => "/".into(),
            ["Users"] => "/Users".into(),
            ["Users", u, rest @ ..] if *u == self.user => self.home(rest),
            ["Users", "Shared", ..] => "/Users/Shared".into(),
            ["Users", u, ..] if self.modo == Modo::Real => format!("/Users/{u}"),
            ["Users", ..] => "/Users/<outro>".into(),
            ["System", r @ ..] => p(r, "/System", 3),
            ["Library", r @ ..] => p(r, "/Library", 3),
            ["Applications", r @ ..] => p(r, "/Applications", 3),
            ["opt", r @ ..] => p(r, "/opt", 3),
            ["usr", "local", r @ ..] => p(r, "/usr/local", 3),
            ["private", "var", "folders", ..] => "/private/var/folders".into(),
            ["private", "var", r @ ..] => p(r, "/private/var", 3),
            ["private", r @ ..] => p(r, "/private", 1),
            [x, ..] => format!("/{}", self.clean(x)),
        }
    }

    fn home(&self, r: &[&str]) -> String {
        const COMUNS: [&str; 8] = ["Desktop", "Documents", "Downloads", "Pictures", "Movies", "Music", "Public", "Applications"];
        match r {
            [] => "~".into(),
            ["Library", "Mobile Documents", c, ..] => format!("~/Library/Mobile Documents/{}", self.clean(c)),
            ["Library", "CloudStorage", c, ..] => {
                // GoogleDrive-foo@x.com → GoogleDrive; OneDrive-Pessoal → OneDrive
                let prov = c.split('-').next().unwrap_or(c);
                format!("~/Library/CloudStorage/{}", self.clean(prov))
            }
            ["Library", rest @ ..] => {
                let mut s = String::from("~/Library");
                for x in rest.iter().take(3) {
                    s.push('/');
                    s.push_str(&self.clean(x));
                }
                s
            }
            [d, rest @ ..] if d.starts_with('.') => {
                let mut s = format!("~/{}", self.clean(d));
                if let Some(x) = rest.first() {
                    s.push('/');
                    s.push_str(&self.clean(x));
                }
                s
            }
            // Só com nomes reais: as bibliotecas de mídia ganham balde próprio, porque o Ajustes conta
            // cada uma de um jeito (Música, TV, Fotos) e o resto da pasta é Documentos (Frente P §3).
            [d @ ("Music" | "Movies"), a, b, ..] if self.modo == Modo::Real => format!("~/{d}/{a}/{b}"),
            ["Pictures", x, ..] if self.modo == Modo::Real && x.ends_with(".photoslibrary") => format!("~/Pictures/{x}"),
            [d, ..] if COMUNS.contains(d) || self.modo == Modo::Real => format!("~/{d}"),
            _ => "~/<pasta do usuário>".into(),
        }
    }
}

/// Minúsculas sem acento, para comparar com os tokens: "João" em NFC ou NFD (como vem do disco) e
/// "joao" ficam iguais.
pub fn dobrar(s: &str) -> String {
    // Letra-base de U+00C0..U+017F (Latin-1 e Latin Extended-A, gerada do NFD; ø, ł, đ à mão);
    // '0' = sem letra-base (×, ß, ligaduras).
    const BASE: &[u8; 192] = b"aaaaaa0ceeeeiiiidnooooo0ouuuuyt0aaaaaa0ceeeeiiiidnooooo0ouuuuytyaaaaaaccccccccddddeeeeeeeeeegggggggghhhhiiiiiiii0i00jjkkkllllllllllnnnnnnn00oooooo00rrrrrrssssssssttttttuuuuuuuuuuuuwwyyyzzzzzz0";
    s.to_lowercase()
        .chars()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .map(|c| match (c as u32).checked_sub(0xC0).and_then(|i| BASE.get(i as usize)) {
            Some(&b) if b != b'0' => b as char,
            _ => c,
        })
        .collect()
}

/// "AnaLima", "backupAna" viram "Ana Lima", "backup Ana": a troca de minúscula para Maiúscula
/// conta como fronteira de palavra ("TimeMachine" não casa com "tim": depois vem "e").
fn separar_camel(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 4);
    let mut minuscula = false;
    for c in s.chars() {
        if c.is_uppercase() && minuscula {
            o.push(' ');
        }
        minuscula = c.is_lowercase();
        o.push(c);
    }
    o
}

/// Token no texto (os dois já dobrados): com 5 letras ou mais, em qualquer lugar; mais curto, só
/// como palavra inteira ("ron" não casa com "chronod", nem "tim" com "TimeMachine").
pub(crate) fn tem_token(texto: &str, t: &str) -> bool {
    if t.chars().count() >= 5 {
        return texto.contains(t);
    }
    texto.match_indices(t).any(|(i, _)| {
        texto[..i].chars().next_back().is_none_or(|c| !c.is_alphanumeric()) && texto[i + t.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric())
    })
}

/// 8-4-4-4-12 hex (conta de Mail, Daemon Container, FileProvider...).
pub fn is_uuid(s: &str) -> bool {
    let p: Vec<&str> = s.split('-').collect();
    p.len() == 5
        && p.iter().zip([8, 4, 4, 4, 12]).all(|(x, n)| x.len() == n && x.chars().all(|c| c.is_ascii_hexdigit()))
}

fn parent(k: &str) -> Option<&str> {
    let (p, _) = k.rsplit_once('/')?;
    if p.is_empty() { None } else { Some(p) }
}

/// Sufixo do balde dobrado ("<pai>/(demais)"); o relatório traduz na hora de mostrar.
pub const DOBRADO: &str = "/(demais)";

/// Baldes abaixo do limite somam em "<pai>/(demais)". `metric` escolhe o valor comparado.
pub fn fold<T: Default + Clone>(
    mut m: HashMap<String, T>,
    limit: i64,
    metric: impl Fn(&T) -> i64,
    add: impl Fn(&mut T, &T),
) -> HashMap<String, T> {
    loop {
        let mut moved = false;
        let keys: Vec<String> = m.keys().cloned().collect();
        for k in keys {
            let Some(v) = m.get(&k) else { continue };
            if metric(v) >= limit {
                continue;
            }
            let owner = if k.ends_with(DOBRADO) { parent(&k).and_then(parent) } else { parent(&k) };
            let Some(owner) = owner else { continue };
            let dest = format!("{owner}{DOBRADO}");
            if dest == k {
                continue;
            }
            let v = m.remove(&k).unwrap_or_default();
            add(m.entry(dest).or_default(), &v);
            moved = true;
        }
        if !moved {
            return m;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_curto_so_como_palavra_e_sem_acento() {
        assert!(!tem_token("chronod_datavault deleted_open_processes privacidade", "ron"));
        assert!(!tem_token("autoteste_privacidade", "cida"));
        assert!(tem_token("\"/users/ron/x\"", "ron"));
        assert!(tem_token("macbook-de-marcela", "marcela"));
        let c = Ctx::new("tim", "João Souza", &["b1c2d3e4.virtua.com.br".into(), "fulanos-mbp.home".into()]);
        assert!(!c.tokens.iter().any(|t| t == "com" || t == "home"), "{:?}", c.tokens);
        assert_eq!(c.clean("com.apple.TimeMachine"), "com.apple.TimeMachine");
        assert_eq!(c.clean("com.bombich.ccc"), "com.bombich.ccc");
        assert_eq!(c.clean("joa\u{303}o-backup"), "<privado>", "NFD");
        assert_eq!(c.clean("joao.backup.x"), "<privado>", "sem acento");
        assert_eq!(c.clean("tim-agent"), "<privado>");
        assert!(!c.tokens.iter().any(|t| t == "virtua"), "rótulo do provedor: {:?}", c.tokens);
        assert_eq!(c.processo("com.apple.Virtualization.Virtua"), "com.apple.Virtualization.…");
    }

    #[test]
    fn nome_curto_colado_e_acento_fora_do_portugues() {
        let c = Ctx::new("ana", "Ana Lima", &["MacBook Pro de Ana".into()]);
        for s in ["AnaLima-sync", "com.analima.app", "backupAna", "Ana_Lima"] {
            assert_eq!(c.clean(s), "<privado>", "{s}");
        }
        for s in ["analyticsd", "ManagedClient", "com.apple.TimeMachine"] {
            assert_eq!(c.clean(s), s);
        }
        let t = Ctx::new("tomas", "Tomáš Dvořák", &[]);
        assert_eq!(t.clean("Dvor\u{30C}a\u{301}k"), "<privado>", "NFD");
        assert_eq!(t.clean("dvorak-backup"), "<privado>", "ASCII");
        assert_eq!(Ctx::new("lukasz", "Łukasz Wąs", &[]).clean("lukasz-sync"), "<privado>");
        assert_eq!(c.achados("{\"x\":\"/Users/ana/AnaLima\",\"y\":\"a@b\"}"), 5, "ana, lima, \"ana lima\" (no AnaLima separado), analima e o @");
    }

    #[test]
    fn processo_cortado_perde_o_ultimo_trecho() {
        let c = Ctx::new("marcela", "Marcela Duarte", &[]);
        assert_eq!(c.processo("relatorio-clientes-acmes-marcel"), "relatorio-clientes-acmes-…");
        assert_eq!(c.processo("com.apple.Virtualization.Virtua"), "com.apple.Virtualization.…");
        assert_eq!(c.processo("abcdefghijklmnopqrstuvwxyz01234"), "<privado>");
        assert_eq!(c.processo("chronod"), "chronod");
        assert_eq!(Ctx::real("x").processo("relatorio-clientes-acmes-marcel"), "relatorio-clientes-acmes-marcel");
    }

    fn ctx() -> Ctx {
        Ctx::new("fulano", "Fulano de Tal Souza", &["Mac de Fulano".into(), "Estacao-Souza".into()])
    }

    #[test]
    fn baldes() {
        let c = ctx();
        let casos = [
            ("/", "/"),
            ("/Users/fulano", "~"),
            ("/Users/fulano/Library/Containers/com.x.y/Data/Library/Caches", "~/Library/Containers/com.x.y/Data"),
            ("/Users/fulano/Library/Application Support/Code/User/workspaceStorage", "~/Library/Application Support/Code/User"),
            ("/Users/fulano/Library/Mobile Documents/com~apple~CloudDocs/Projetos/X", "~/Library/Mobile Documents/com~apple~CloudDocs"),
            ("/Users/fulano/Library/Mobile Documents/iCloud~md~obsidian/Cofre", "~/Library/Mobile Documents/iCloud~md~obsidian"),
            ("/Users/fulano/Library/CloudStorage/GoogleDrive-foo@x.com/Meu Drive", "~/Library/CloudStorage/GoogleDrive"),
            ("/Users/fulano/Library/CloudStorage/OneDrive-Pessoal", "~/Library/CloudStorage/OneDrive"),
            ("/Users/fulano/.colima/_lima/colima/disk", "~/.colima/_lima"),
            ("/Users/fulano/.cargo", "~/.cargo"),
            ("/Users/fulano/Documents/Contrato secreto", "~/Documents"),
            ("/Users/fulano/Downloads", "~/Downloads"),
            ("/Users/fulano/Projetos Pessoais/a/b", "~/<pasta do usuário>"),
            ("/Users/beltrano/Documents", "/Users/<outro>"),
            ("/Users/Shared/x", "/Users/Shared"),
            ("/private/var/folders/xx/yyyy/C", "/private/var/folders"),
            ("/private/var/db/diagnostics/Persist", "/private/var/db/diagnostics/Persist"),
            ("/private/tmp/qualquer/coisa", "/private/tmp"),
            ("/System/Library/AssetsV2/com_apple_x/purpose_auto", "/System/Library/AssetsV2/com_apple_x"),
            ("/Applications/Xcode.app/Contents/Developer/Platforms", "/Applications/Xcode.app/Contents/Developer"),
            ("/usr/local/Cellar/foo/1.0", "/usr/local/Cellar/foo/1.0"),
            ("/.Spotlight-V100/Store-V2", "/.Spotlight-V100"),
            // componente com @, usuário, nome ou host → <privado>
            ("/Users/fulano/Library/Application Support/conta@exemplo.com/x", "~/Library/Application Support/<privado>/x"),
            ("/Users/fulano/Library/Caches/fulano-cache", "~/Library/Caches/<privado>"),
            ("/Users/fulano/.config/Souza", "~/.config/<privado>"),
            ("/Library/Caches/Mac de Fulano", "/Library/Caches/<privado>"),
            ("/Users/fulano/Library/Group Containers/grp.estacao-souza.x", "~/Library/Group Containers/<privado>"),
            ("/Users/fulano/Library/Mail/V10/5A1E0F42-7C3B-4D9E-8F21-0B6D4C2E9A71/INBOX.mbox", "~/Library/Mail/V10/<uuid>"),
        ];
        for (inp, esperado) in casos {
            assert_eq!(c.bucket(inp), esperado, "caminho {inp}");
        }
        assert_eq!(c.clean("Fulano's App"), "<privado>");
        assert_eq!(c.clean("com.apple.Safari"), "com.apple.Safari");
    }

    #[test]
    fn modos_real_e_ramo() {
        let r = Ctx::real("fulano");
        assert_eq!(r.bucket("/Users/fulano/Projetos Pessoais/a/b"), "~/Projetos Pessoais");
        assert_eq!(r.bucket("/Users/fulano/Library/Caches/fulano-cache/x/y"), "~/Library/Caches/fulano-cache/x");
        assert_eq!(r.bucket("/Users/beltrano/Documents"), "/Users/beltrano");
        assert_eq!(r.clean("5A1E0F42-7C3B-4D9E-8F21-0B6D4C2E9A71"), "5A1E0F42-7C3B-4D9E-8F21-0B6D4C2E9A71");
        // Bibliotecas de mídia: balde próprio só com nomes reais (o anônimo continua em ~/Music).
        assert_eq!(r.bucket("/Users/fulano/Music/Music/Media.localized/Music/Artista/a.m4a"), "~/Music/Music/Media.localized");
        assert_eq!(r.bucket("/Users/fulano/Music/Logic/Projeto"), "~/Music/Logic/Projeto");
        assert_eq!(r.bucket("/Users/fulano/Music/solto.mp3"), "~/Music");
        assert_eq!(r.bucket("/Users/fulano/Movies/TV/Media.localized/Movies/x"), "~/Movies/TV/Media.localized");
        assert_eq!(r.bucket("/Users/fulano/Pictures/Photos Library.photoslibrary/originals/0"), "~/Pictures/Photos Library.photoslibrary");
        assert_eq!(r.bucket("/Users/fulano/Pictures/Viagem/a.jpg"), "~/Pictures");
        assert_eq!(ctx().bucket("/Users/fulano/Music/Music/Media.localized/Music/a.m4a"), "~/Music");
        let m = Ctx::ramo(2);
        assert_eq!(m.bucket("/a/b/c/d"), "a/b");
        assert_eq!(m.bucket("/a"), "a");
        assert_eq!(m.bucket("/"), "/");
    }

    #[test]
    fn host_ip_nao_vira_token() {
        let c = Ctx::new("fulano", "Fulano Tal", &["192.168.1.23".to_string()]);
        assert!(c.tokens.iter().all(|t| !t.chars().all(|d| d.is_ascii_digit())));
        assert!(c.tokens.contains(&"192.168.1.23".to_string()));
    }

    #[test]
    fn dobra() {
        let mut m: HashMap<String, i64> = HashMap::new();
        m.insert("~/Library/Caches/a".into(), 50);
        m.insert("~/Library/Caches/b".into(), 60);
        m.insert("~/Library/Caches".into(), 500);
        m.insert("~".into(), 1);
        let m = fold(m, 100, |v| *v, |a, b| *a += *b);
        assert_eq!(m.get("~/Library/Caches/(demais)"), Some(&110));
        assert_eq!(m.get("~"), Some(&1));
        assert_eq!(m.values().sum::<i64>(), 611);
    }
}

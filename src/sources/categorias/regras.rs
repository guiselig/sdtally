//! Em qual linha do Ajustes cai cada linha do `top`, com o que só a máquina sabe (apps instalados,
//! "Mesa e Documentos", versão do macOS). Puro: sem disco e sem `home` (o `~` já vem no caminho).
//! A tabela de prefixos é a de `linhas.rs`; aqui só se resolve o que ela deixa em `mixed`.

use super::Fatos;
use crate::linhas;

pub const APPS: &str = "com.apple.STMExtension.Applications";
pub const DOCS: &str = "com.apple.STMExtension.Documents";
pub const MUSICA: &str = "com.apple.Music.MusicStorageExtension";
pub const FOTOS: &str = "com.apple.Photos.StorageManagementExtension";
pub const TV: &str = "com.apple.TV.TVStorageExtension";
pub const LIVROS: &str = "com.apple.iBooksX.DiskSpaceEfficiency";
pub const PODCASTS: &str = "com.apple.podcasts.MacPodcastsStorageExtension";
pub const LIXO: &str = "com.apple.STMExtension.Trash";
pub const GARAGEBAND: &str = "com.apple.STMExtension.GarageBand";
pub const IOS: &str = "com.apple.STMExtension.iOSFiles";
pub const DEV: &str = "com.apple.STMExtension.Developer";
pub const ICLOUD: &str = "com.apple.CloudDocsDaemon.StorageManagement";
pub const MAIL: &str = "com.apple.STMExtension.Mail";
pub const MENSAGENS: &str = "com.apple.messages.StorageManagementExtension";
pub const OUTROS: &str = "com.apple.STMExtension.OtherUsers";
pub const SISTEMA: &str = "system_data";

/// As 15 categorias, na ordem da tabela da Frente P.
pub const IDS: [&str; 15] = [APPS, DOCS, MUSICA, FOTOS, TV, LIVROS, PODCASTS, LIXO, GARAGEBAND, IOS, DEV, ICLOUD, MAIL, MENSAGENS, OUTROS];

/// Apps que têm categoria própria: o container deles não é de Aplicativos (Frente P §2.1).
const FORA_DE_APPS: [&str; 7] = ["com.apple.Music", "com.apple.iBooksX", "com.apple.mail", "com.apple.TV", "com.apple.podcasts", "com.apple.Photos", "com.apple.MobileSMS"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grau {
    /// Regra lida no código da Apple.
    A,
    /// Hipótese ou aproximação.
    B,
}

impl Grau {
    pub fn letra(self) -> &'static str {
        if self == Grau::A { "A" } else { "B" }
    }
}

/// Por que uma linha não coube em nenhuma linha do Ajustes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Aberto {
    /// Container de app sem a lista de apps do Spotlight.
    Containers,
    /// Mesa ou Documentos sem saber se "Mesa e Documentos" do iCloud está ligado.
    MesaDocumentos,
    /// Caminho fora do que as regras previram.
    Outro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destino {
    /// Sem linha (xattr de fluxo próprio): fora da conta.
    Ignora,
    Em(&'static str, Grau),
    Aberto(Aberto),
}

/// Uma linha de `top` como a regra a vê.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Linha {
    pub path: Option<String>,
    pub term: String,
    pub bytes: i64,
    pub folded: bool,
}

/// `macos`: versão maior do macOS (27), ou None se ilegível.
pub fn destino(l: &Linha, f: &Fatos, macos: Option<u32>) -> Destino {
    let (Some(linha), grau) = linhas::linha(l.path.as_deref(), &l.term) else { return Destino::Ignora };
    let g = if grau == Some("B") { Grau::B } else { Grau::A };
    let Some(p) = l.path.as_deref() else { return Destino::Em(linha, g) };
    // Caminhos fixos que só existem a partir do macOS 27 (Frente Q: no 15 e no 26 o painel não conta).
    if linha == DEV && p.starts_with("/Library/Developer/CoreSimulator") && macos.is_some_and(|m| m < 27) {
        return Destino::Em(SISTEMA, Grau::A);
    }
    if linha != "mixed" {
        return Destino::Em(linha, g);
    }
    let dentro = |base: &str| p == base || p.strip_prefix(base).is_some_and(|r| r.starts_with('/'));
    match p {
        // Arquivos soltos da home; a linha dobrada junta pastas pequenas de Documentos.
        "~" => Destino::Em(DOCS, if l.folded { Grau::B } else { Grau::A }),
        "~/Library" | "~/Library/Developer" | "~/Library/Application Support/MobileSync" => Destino::Em(SISTEMA, Grau::B),
        "/Users" => Destino::Em(OUTROS, Grau::B),
        // O que a categoria Mensagens conta vem do banco; o resto da pasta é Dados do Sistema.
        _ if dentro("~/Library/Messages") => Destino::Em(SISTEMA, Grau::A),
        // Só a parte não despejável do iCloud Drive é da categoria, e ela vem do banco do File Provider.
        _ if dentro("~/Library/Mobile Documents") => Destino::Em(SISTEMA, Grau::B),
        // Linha dobrada: junta containers pequenos de apps e de sistema, sem como separar.
        "~/Library/Containers" if l.folded => Destino::Aberto(Aberto::Containers),
        "~/Library/Containers" => Destino::Em(SISTEMA, Grau::B),
        _ if dentro("~/Library/Containers") => container(p.trim_start_matches("~/Library/Containers/"), f),
        _ if dentro("~/Desktop") => mesa_documentos(f.mesa),
        _ if dentro("~/Documents") => mesa_documentos(f.documentos),
        _ => Destino::Aberto(Aberto::Outro),
    }
}

/// Com "Mesa e Documentos" ligado a pasta sai de Documentos (a parte não despejável é do iCloud
/// Drive, B); desligado, é Documentos.
fn mesa_documentos(ligado: Option<bool>) -> Destino {
    match ligado {
        Some(true) => Destino::Em(SISTEMA, Grau::B),
        Some(false) => Destino::Em(DOCS, Grau::A),
        None => Destino::Aberto(Aberto::MesaDocumentos),
    }
}

/// `resto` = "<pasta>[/Data...]". A pasta é o bundle id; as de UUID trazem o id no plist do container.
fn container(resto: &str, f: &Fatos) -> Destino {
    let pasta = resto.split('/').next().unwrap_or_default();
    let id = f.container_ids.get(pasta).map_or(pasta, String::as_str);
    if FORA_DE_APPS.contains(&id) {
        return Destino::Em(SISTEMA, Grau::B);
    }
    let Some(apps) = &f.apps else { return Destino::Aberto(Aberto::Containers) };
    // Os web apps do Safari têm o id `com.apple.Safari.WebApp` e dividem um container só.
    Destino::Em(if apps.contains(id) { APPS } else { SISTEMA }, Grau::B)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    fn l(path: &str, folded: bool) -> Linha {
        Linha { path: Some(path.into()), term: "tree".into(), bytes: 1, folded }
    }

    fn fatos() -> Fatos {
        Fatos {
            apps: Some(BTreeSet::from(["com.exemplo.Editor".to_string(), "com.apple.Safari".to_string(), "com.apple.Safari.WebApp".to_string()])),
            container_ids: BTreeMap::from([("00000000-0000-0000-0000-000000000001".to_string(), "com.exemplo.Editor".to_string())]),
            mesa: Some(true),
            documentos: Some(false),
            ..Fatos::default()
        }
    }

    #[test]
    fn mixed_resolvido_pelos_fatos() {
        let f = fatos();
        let d = |p: &str, fo: bool| destino(&l(p, fo), &f, Some(27));
        assert_eq!(d("~/Library/Containers/com.exemplo.Editor/Data", false), Destino::Em(APPS, Grau::B));
        assert_eq!(d("~/Library/Containers/00000000-0000-0000-0000-000000000001/Data", false), Destino::Em(APPS, Grau::B));
        assert_eq!(d("~/Library/Containers/com.exemplo.Outro/Data", false), Destino::Em(SISTEMA, Grau::B));
        assert_eq!(d("~/Library/Containers/com.apple.mail/Data", false), Destino::Em(SISTEMA, Grau::B));
        assert_eq!(d("~/Library/Containers/com.apple.Safari.WebApp/Data", false), Destino::Em(APPS, Grau::B));
        assert_eq!(d("~/Library/Containers", true), Destino::Aberto(Aberto::Containers));
        assert_eq!(d("~/Desktop", false), Destino::Em(SISTEMA, Grau::B));
        assert_eq!(d("~/Documents", false), Destino::Em(DOCS, Grau::A));
        assert_eq!(d("~", false), Destino::Em(DOCS, Grau::A));
        assert_eq!(d("~", true), Destino::Em(DOCS, Grau::B));
        assert_eq!(d("~/Library", true), Destino::Em(SISTEMA, Grau::B));
        assert_eq!(d("~/Library/Mobile Documents/com~apple~CloudDocs", false), Destino::Em(SISTEMA, Grau::B));
        assert_eq!(d("~/Library/Messages/Attachments", false), Destino::Em(SISTEMA, Grau::A));
        assert_eq!(d("/Users", true), Destino::Em(OUTROS, Grau::B));
    }

    #[test]
    fn sem_fatos_fica_aberto() {
        let f = Fatos::default();
        let d = |p: &str| destino(&l(p, false), &f, None);
        assert_eq!(d("~/Library/Containers/com.exemplo.Editor/Data"), Destino::Aberto(Aberto::Containers));
        assert_eq!(d("~/Documents"), Destino::Aberto(Aberto::MesaDocumentos));
        // O que não depende de fato continua certo.
        assert_eq!(d("~/.Trash"), Destino::Em(LIXO, Grau::A));
        assert_eq!(d("/Applications/X.app/Contents"), Destino::Em(APPS, Grau::A));
        assert_eq!(d("~/Library/Application Support/MobileSync/Backup"), Destino::Em(IOS, Grau::A));
        assert_eq!(d("~/Music/Music/Media.localized"), Destino::Em(MUSICA, Grau::B));
        assert_eq!(d("~/Music/Music/Music Library.musiclibrary"), Destino::Em(SISTEMA, Grau::A));
        assert_eq!(d("~/Music"), Destino::Em(DOCS, Grau::A));
        assert_eq!(d("~/dev"), Destino::Em(DOCS, Grau::A));
    }

    #[test]
    fn termos_unicos_e_versao() {
        let f = Fatos::default();
        let t = |term: &str| Linha { path: None, term: term.into(), bytes: 1, folded: false };
        assert_eq!(destino(&t("snapshots"), &f, None), Destino::Em(SISTEMA, Grau::B));
        assert_eq!(destino(&t("deleted_open"), &f, None), Destino::Em(SISTEMA, Grau::A));
        assert_eq!(destino(&t("large_xattrs"), &f, None), Destino::Ignora);
        assert_eq!(destino(&t("apfs_metadata"), &f, None), Destino::Em(SISTEMA, Grau::B));
        let sim = l("/Library/Developer/CoreSimulator/Volumes", false);
        assert_eq!(destino(&sim, &f, Some(27)), Destino::Em(DEV, Grau::A));
        assert_eq!(destino(&sim, &f, Some(26)), Destino::Em(SISTEMA, Grau::A));
        assert_eq!(destino(&l("/Library/Developer/CommandLineTools/SDKs", false), &f, Some(26)), Destino::Em(DEV, Grau::A));
    }
}

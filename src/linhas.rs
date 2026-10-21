//! Linha do Ajustes de cada linha de `top` (spec-inicio §9.1, regras da Frente P): o id da
//! categoria da Apple (o mesmo da `settings_screen.screen.categories`), `system_data`, `mixed` ou
//! nada, com o grau da regra. Vale a primeira que casa; o caminho vem no formato do `top`.

use crate::conselho::casa;
use crate::{anon, sys};
use std::path::Path;

const APPS: &str = "com.apple.STMExtension.Applications";
const DEV: &str = "com.apple.STMExtension.Developer";
const OUTROS: &str = "com.apple.STMExtension.OtherUsers";

/// Juntam várias linhas: só a própria pasta (arquivos soltos ou a linha `folded`, que traz o pai).
const MISTAS: [&str; 5] = ["~", "~/Library", "~/Library/Developer", "~/Library/Application Support/MobileSync", "/Users"];

/// (prefixos, linha, grau). Prefixo casa por componente inteiro, igual ou dentro (`conselho::casa`).
const REGRAS: &[(&[&str], &str, &str)] = &[
    (&["~/.Trash"], "com.apple.STMExtension.Trash", "A"),
    (&["~/Library/Application Support/MobileSync/Backup"], "com.apple.STMExtension.iOSFiles", "A"),
    (&["~/Library/Application Support/MobileSync"], "system_data", "A"),
    // Inclui FileProvider/<uuid>: research/frentes/P-categorias-ajustes.md:39 (só MobileSync sai).
    (&["~/Library/Application Support"], APPS, "A"),
    (&["~/Library/Mail"], "com.apple.STMExtension.Mail", "A"),
    (&["~/Library/Messages"], "mixed", "A"),
    (
        &["~/Library/Developer/Xcode/DerivedData", "~/Library/Developer/Xcode/Archives", "~/Library/Developer/Xcode/Installs", "~/Library/Developer/Xcode/* DeviceSupport", "~/Library/Developer/CoreSimulator/Caches"],
        DEV,
        "A",
    ),
    (&["~/Library/Containers", "~/Library/Mobile Documents"], "mixed", "B"),
    // Os .ipsw que o Finder baixa (iPhone, iPad...).
    (&["~/Library/iTunes/* Software Updates"], "com.apple.STMExtension.iOSFiles", "A"),
    // Caches, Group Containers, CloudStorage...
    (&["~/Library"], "system_data", "A"),
    // A travessia dá balde próprio a estas pastas (`anon::Ctx::bucket`, nomes reais). Música, TV e
    // Fotos contam o que está no banco da biblioteca (B: a pasta é o teto); Documentos exclui as
    // pastas das bibliotecas, e o que não está nelas fica em Dados do Sistema (Frente P §2.1).
    (&["~/Music/Music/Media.localized"], "com.apple.Music.MusicStorageExtension", "B"),
    (&["~/Movies/TV/Media.localized", "~/Movies/TV/Media", "~/Movies/Apple TV/Media"], "com.apple.TV.TVStorageExtension", "B"),
    (
        &[
            "~/Music/Music/Music Library.musiclibrary",
            "~/Music/Music/Previous Libraries.localized",
            "~/Music/iTunes/iTunes Media",
            "~/Movies/TV",
            "~/Movies/Apple TV",
        ],
        "system_data",
        "A",
    ),
    (&["~/Pictures/Photos Library.photoslibrary"], "com.apple.Photos.StorageManagementExtension", "B"),
    // Saem de Documentos só com "Mesa e Documentos" do iCloud ligado.
    (&["~/Desktop", "~/Documents"], "mixed", "B"),
    (&["~/Applications", "/Applications"], APPS, "A"),
    // Pastas ocultas, Downloads, Public...
    (&["~"], "com.apple.STMExtension.Documents", "A"),
    (&["/Users/Shared", "/Users/Deleted Users"], OUTROS, "A"),
    (&["/Users"], OUTROS, "B"),
    (&["/Library/Developer/CommandLineTools", "/Library/Developer/CoreSimulator"], DEV, "A"),
    // Os caminhos fixos da extensão do GarageBand (Frente P §3). Basic Lessons fica de fora: o balde
    // para em `Learn to Play`.
    (
        &[
            "/Library/Application Support/GarageBand/Instrument Library",
            "/Library/Application Support/Logic/Sampler Instruments",
            "/Library/Application Support/Logic/EXS Factory Samples",
            "/Library/Application Support/Logic/Alchemy Samples",
            "/Library/Application Support/Logic/Ultrabeat Samples",
            "/Library/Application Support/Logic/Channel Strip Settings",
            "/Library/Application Support/Logic/Plug-In Settings",
            "/Library/Audio/Impulse Responses/Apple",
            "/Library/Audio/Apple Loops/Apple",
        ],
        "com.apple.STMExtension.GarageBand",
        "A",
    ),
];

/// (`settings_line`, `settings_line_grade`) de uma linha de `top`. O resto (`/Library`, `/private`,
/// `/opt`, AssetsV2...) é Dados do Sistema.
pub fn linha(path: Option<&str>, term: &str) -> (Option<&'static str>, Option<&'static str>) {
    match (term, path) {
        ("snapshots", _) => (Some("system_data"), Some("B")),
        ("deleted_open", _) => (Some("system_data"), Some("A")),
        // Metadado do volume não é de nenhuma categoria: cai em Dados do Sistema por diferença (B: não está no código da Apple).
        ("apfs_metadata", _) => (Some("system_data"), Some("B")),
        ("large_xattrs", _) | (_, None) => (None, None),
        (_, Some(p)) if MISTAS.contains(&p) => (Some("mixed"), Some("A")),
        (_, Some(p)) => REGRAS.iter().find(|(ps, ..)| ps.iter().any(|x| casa(x, p))).map_or((Some("system_data"), Some("A")), |(_, l, g)| (Some(*l), Some(*g))),
    }
}

/// `top[].label` de `~/Library/Application Support/FileProvider/<uuid>`: o provedor, pelo xattr
/// `com.apple.file-provider-domain-id` da própria pasta ("<bundle>/<domínio>"). Grau B: xattr
/// observado no macOS 26 e 27, sem documentação da Apple. O domínio traz a conta: nunca sai.
pub fn rotulo(path: &str, home: &str) -> Option<&'static str> {
    let u = path.strip_prefix("~/Library/Application Support/FileProvider/").filter(|u| anon::is_uuid(u))?;
    let v = sys::apfs::xattr_valor(Path::new(&format!("{home}/Library/Application Support/FileProvider/{u}")), "com.apple.file-provider-domain-id");
    Some(provedor(&v.unwrap_or_default()))
}

/// Nome do provedor pelo bundle (antes da 1ª "/"); o resto, ou sem bundle, "File Provider".
pub fn provedor(v: &[u8]) -> &'static str {
    let b = v.split(|c| *c == b'/').next().unwrap_or_default();
    [("com.apple.CloudDocs.", "iCloud Drive"), ("com.google.drivefs.", "Google Drive"), ("com.microsoft.OneDrive", "OneDrive"), ("com.getdropbox.", "Dropbox")]
        .iter()
        .find(|(p, _)| b.starts_with(p.as_bytes()))
        .map_or("File Provider", |(_, n)| n)
}

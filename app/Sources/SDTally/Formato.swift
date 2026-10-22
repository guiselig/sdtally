// Números e rótulos na língua do app. Só texto: o app nunca mede nada, só mostra o que o sdtally mediu.
import SwiftUI

/// Números e datas no formato da língua do app (vírgula ou ponto decimal, dd/mm ou mm/dd).
nonisolated let localApp = Locale(identifier: linguaDoApp)

/// GB decimal com 2 casas, como o Ajustes; abaixo de 1 GB, MB com 1 casa.
nonisolated func gb(_ bytes: Int64) -> String {
    let v = Double(bytes)
    if abs(v) >= 1e9 { return (v / 1e9).formatted(.number.precision(.fractionLength(2)).locale(localApp)) + " GB" }
    if abs(v) >= 1e6 { return (v / 1e6).formatted(.number.precision(.fractionLength(1)).locale(localApp)) + " MB" }
    if abs(v) >= 1e3 { return (v / 1e3).formatted(.number.precision(.fractionLength(0)).locale(localApp)) + " KB" }
    return "\(bytes) B"
}

/// Bytes exatos com separador de milhar ("172.126.034.512 bytes").
nonisolated func bytesExatos(_ bytes: Int64) -> String {
    bytes.formatted(.number.locale(localApp)) + " bytes"
}

/// Rótulo curto de cada `source` do JSON (docs/json-schema.md).
nonisolated func nomeFonte(_ chave: String) -> String {
    switch chave {
    case "settings_ui_log": String(localized: "log do Ajustes")
    case "storage_management_log": String(localized: "log do serviço")
    case "diskutil_capacity_in_use": "diskutil"
    case "getattrlistbulk": String(localized: "travessia")
    case "fsctl_snapshot_private_size": String(localized: "snapshots (APFS)")
    case "fsctl_dir_stats", "dir_stats_like_settings": String(localized: "dir-stats (APFS)")
    case "lsof_proc_pidfdinfo": "lsof"
    case "listxattr": "xattr"
    case "attr_vol_spaceused": String(localized: "volume (APFS)")
    case "cachedelete": "CacheDelete"
    case "statfs": "statfs"
    case "difference": String(localized: "diferença")
    case "sysctl_vm_swapusage": "sysctl"
    case "diskutil_apfs_list": "diskutil"
    case "getattrlist_addedtime": String(localized: "data de entrada no Lixo")
    case "apfs_user_client": String(localized: "contadores do APFS")
    case "top": String(localized: "soma das linhas")
    default: chave
    }
}

/// Termos da conta do volume Data (`data_volume.terms[].key`).
nonisolated func nomeTermo(_ chave: String) -> String { termo(chave).nome }

/// Nome, SF Symbol, frase curta e cor de cada termo (§3.2). Só cores de sistema, distintas do destaque (Dados do Sistema).
nonisolated func termo(_ chave: String) -> (nome: String, simbolo: String, frase: String, cor: Color) {
    switch chave {
    case "tree": (String(localized: "Arquivos e pastas"), "folder", String(localized: "Tudo o que a travessia leu, sem contar clone nem hardlink duas vezes."), .teal)
    case "large_xattrs": (String(localized: "Atributos estendidos grandes"), "paperclip", String(localized: "Dados presos a arquivos, fora do tamanho deles."), .green)
    case "snapshots": (String(localized: "Snapshots locais do Time Machine"), "clock.arrow.circlepath", String(localized: "Cópias do disco que o Time Machine guarda no próprio Mac; o macOS apaga sozinho quando falta espaço."), .orange)
    case "deleted_open": (String(localized: "Apagados ainda abertos"), "doc.badge.ellipsis", String(localized: "Você apagou, mas um app ainda segura; o espaço volta quando ele fechar."), .indigo)
    case "protected_folders": (String(localized: "Pastas protegidas do sistema"), "lock", String(localized: "Pastas que nem o administrador abre, medidas pelo próprio APFS."), .yellow)
    case "apfs_metadata": (String(localized: "Metadados do APFS"), "cylinder.split.1x2", String(localized: "Índices que o próprio APFS mantém para achar os arquivos do volume."), .brown)
    default: (chave, "questionmark.folder", "", .gray)
    }
}

/// Volume do container pelo papel (`roles[0]` do diskutil): nome e SF Symbol. Sem papel, "Volume".
nonisolated func nomePapel(_ role: String?) -> (nome: String, simbolo: String) {
    switch role {
    case "System": (String(localized: "Sistema (selado, só leitura)"), "lock.shield")
    case "Data": (String(localized: "Dados"), "person.crop.square")
    case "Preboot": ("Preboot", "power")
    case "Recovery": (String(localized: "Recuperação"), "cross.case")
    case "VM": (String(localized: "VM (memória virtual)"), "memorychip")
    case "Update": (String(localized: "Atualização"), "arrow.down.circle")
    case "xART": ("xART", "key")
    case "Hardware": (String(localized: "Hardware"), "cpu")
    case let r?: (r, "externaldrive")
    case nil: (String(localized: "Volume"), "externaldrive")
    }
}

/// Serviços do CacheDelete com nome humano; id desconhecido aparece cru.
nonisolated func nomeServico(_ id: String) -> String {
    switch id {
    case "com.apple.TMCacheDelete": "Time Machine"
    case "com.apple.fspurgeable_data": String(localized: "arquivos marcados como liberáveis")
    case "com.apple.metadata.mds.cachedelete": "Spotlight"
    default: id
    }
}

/// Nome e cor de cada categoria do Ajustes (id da extensão no log), nas cores do próprio Ajustes.
nonisolated func categoria(_ id: String) -> (nome: String, cor: Color) {
    switch id {
    case "com.apple.STMExtension.Applications": (String(localized: "Aplicativos"), .red)  // g9: cor do Ajustes
    case "com.apple.STMExtension.Documents": (String(localized: "Documentos"), .orange)
    case "com.apple.Music.MusicStorageExtension": (String(localized: "Música"), .yellow)
    case "com.apple.Photos.StorageManagementExtension": (String(localized: "Fotos"), .green)
    case "com.apple.STMExtension.Developer": (String(localized: "Desenvolvedor"), .teal)
    case "com.apple.CloudDocsDaemon.StorageManagement": ("iCloud Drive", .blue)
    case "com.apple.STMExtension.Mail": ("Mail", .indigo)
    case "com.apple.messages.StorageManagementExtension": (String(localized: "Mensagens"), .mint)
    case "com.apple.STMExtension.Trash": (String(localized: "Lixo"), .brown)
    case "com.apple.STMExtension.OtherUsers": (String(localized: "Outros usuários"), .purple)
    case "com.apple.STMExtension.iOSFiles": (String(localized: "Arquivos do iOS"), .cyan)
    // Cinza: o destaque do sistema é só de Dados do Sistema.
    default: (id.split(separator: ".").last.map(String.init) ?? id, .gray)
    }
}

/// Hora "HH:MM" de um carimbo do sdtally ("2026-10-02 09:29:42.302128-0300" ou "2026-10-02 09:29");
/// nil sem carimbo: quem escreve a frase omite a cláusula em vez de pôr "?".
nonisolated func hora(_ carimbo: String?) -> String? {
    guard let c = carimbo, c.count >= 16 else { return nil }
    // No formato da língua do app ("18:49", "6:49 PM"); carimbo que não vira Date: o HH:MM cru.
    return data(c)?.formatted(.dateTime.hour().minute().locale(localApp)) ?? String(c.dropFirst(11).prefix(5))
}

/// Carimbo que sempre existe (generatedAt): sem checagem de nil na frase.
nonisolated func hora(_ carimbo: String) -> String { hora(Optional(carimbo)) ?? carimbo }

/// Date de um carimbo do log ("2026-10-02 09:29:42.302128-0300"), do sdtally
/// ("2026-10-02 09:36") ou de um ponto do histórico com fuso ("2026-10-02T09:36:00-03:00"), para idade.
nonisolated func data(_ carimbo: String?) -> Date? {
    guard let c = carimbo, c.count >= 16 else { return nil }
    if c.dropFirst(10).first == "T" { return try? Date(c, strategy: .iso8601) }
    let longo = c.count >= 19
    return (longo ? formatoComSegundos : formatoSemSegundos).date(from: String(c.prefix(longo ? 19 : 16)))
}

/// Carimbo dos pontos do histórico: a hora de parede do sdtally com o fuso de `agora`
/// ("2026-10-02T09:36:00-03:00"). Sem fuso, a hora repetida do fim do horário de verão voltava
/// no tempo e dava intervalo negativo. Carimbo que não vira Date volta como veio.
nonisolated func carimboComFuso(_ c: String, agora: Date = .now) -> String {
    guard data(c) != nil, c.dropFirst(10).first == " " else { return c }
    let parede = c.count >= 19 ? String(c.prefix(19)) : String(c.prefix(16)) + ":00"
    let s = TimeZone.current.secondsFromGMT(for: agora)
    return parede.replacing(" ", with: "T") + String(format: "%@%02d:%02d", s < 0 ? "-" : "+", abs(s) / 3600, abs(s) / 60 % 60)
}

// DateFormatter é caro de criar e seguro entre threads; um por formato.
private nonisolated let formatoComSegundos = formatadorCarimbo("yyyy-MM-dd HH:mm:ss")
private nonisolated let formatoSemSegundos = formatadorCarimbo("yyyy-MM-dd HH:mm")

private nonisolated func formatadorCarimbo(_ formato: String) -> DateFormatter {
    let f = DateFormatter()
    f.locale = Locale(identifier: "en_US_POSIX")
    f.dateFormat = formato
    return f
}

/// "agora" (< 2 min), "há 3 min", "há 2 h", ou "de 01/10 às 00:55".
nonisolated func idade(_ d: Date, agora: Date = .now) -> String {
    let s = agora.timeIntervalSince(d)
    if s < 120 { return String(localized: "agora") }
    if s < 3600 { return String(localized: "há \(Int(s / 60)) min") }
    if s < 6 * 3600 { return String(localized: "há \(Int(s / 3600)) h") }
    let dia = d.formatted(.dateTime.day(.twoDigits).month(.twoDigits).locale(localApp))
    let horario = d.formatted(.dateTime.hour().minute().locale(localApp))
    return String(localized: "de \(dia) às \(horario)")
}

/// Expande "~" com a home do JSON. Pastas do volume Data sem firmlink na raiz
/// (/.Spotlight-V100, /.DocumentRevisions-V100) só existem em /System/Volumes/Data.
nonisolated func absoluto(_ caminho: String, home: String) -> String {
    let p = caminho == "~" ? home : caminho.hasPrefix("~/") ? home + caminho.dropFirst() : caminho
    let data = "/System/Volumes/Data" + p
    let fm = FileManager.default
    return p.hasPrefix("/") && !fm.fileExists(atPath: p) && fm.fileExists(atPath: data) ? data : p
}

/// Texto de disco (nome de pasta, processo, criador) pronto para exibir: controle, bidi,
/// largura zero, U+2028/2029 e tag characters viram "\u{…}" visível, nos mesmos intervalos do
/// `redigir::escapar` do Rust (spec-fase6 §4.5). Sem corte: o app mostra o nome inteiro.
nonisolated func visivel(_ s: String) -> String {
    var o = ""
    for u in s.unicodeScalars {
        switch u.value {
        case 0x00...0x1F, 0x7F...0x9F,                                    // controle
             0x061C, 0x200E, 0x200F, 0x202A...0x202E, 0x2066...0x2069,    // bidi
             0x200B...0x200D, 0x2060...0x2064, 0xFEFF,                    // largura zero
             0x2028, 0x2029,                                             // separadores de linha e parágrafo
             0xE0000...0xE007F:                                          // tag characters
            o += "\\u{" + String(format: "%04X", u.value) + "}"
        default: o.unicodeScalars.append(u)
        }
    }
    return o
}

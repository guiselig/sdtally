// Causas do Início (spec-inicio §3 B2, B3, B5, B6): as linhas do `top` agrupadas pela ficha, a
// linha do Ajustes em que cada uma cai, a manchete e a barra do disco. Só funções
// puras, nonisolated e testadas (ResumoTests). API estável:
//   Causa, causas(top:swap:fichas:home:apelidos:), conselho(de:em:), Apelidos
//   segmentosDisco(_:top:containers:)
//   Manchete, manchete(advice:tm:snapshots:causas:usado:), purgavelNoAjustes(tela:livre:purg:)
import AppKit
import SwiftUI

nonisolated struct Causa: Identifiable, Sendable, Equatable {
    /// card, caminho, termo ou "swap".
    let id: String
    let nome: String
    /// O da maior linha do grupo.
    let caminho: String?
    /// tree, snapshots, protected_folders, deleted_open, large_xattrs ou swap.
    let termo: String
    let card: String?
    let bytes: Int64
    /// Linhas agrupadas ("iCloud Drive · 2 pastas").
    let pastas: Int
    /// App dono, quando todas as linhas do grupo têm o mesmo (`top[].owner`); nil sem fonte.
    var dono: String? = nil
}

/// Causas de pelo menos 1 GB, da maior para a menor (empate: id): as linhas não dobradas,
/// agrupadas por `card` (sem ficha, cada linha sozinha), e o swap em uso, que fica no macOS.
/// Pasta sua e conta de outra pessoa não se juntam: Música e Imagens são causas diferentes.
nonisolated func causas(top: [LinhaTop], swap: Swap?, fichas: MapaFichas?, home: String,
                        apelidos: [String: String] = Apelidos.todos()) -> [Causa] {
    let grupos = Dictionary(grouping: top.filter { $0.folded != true }) { l in
        l.card.flatMap { ["user_folder", "other_users"].contains($0) ? nil : $0 } ?? l.path ?? l.term
    }
    var cs = grupos.map { id, ls -> Causa in
        let maior = ls.max { $0.bytes < $1.bytes }!
        return Causa(id: id, nome: nomeCausa(maior, fichas: fichas, home: home, apelidos: apelidos), caminho: maior.path, termo: maior.term,
                     card: maior.card, bytes: ls.reduce(0) { $0 + $1.bytes }, pastas: Set(ls.map { $0.path ?? $0.term }).count,
                     dono: donoComum(ls))
    }
    if let u = swap?.used {
        cs.append(Causa(id: "swap", nome: String(localized: "Swap (memória no disco)"), caminho: nil, termo: "swap", card: nil, bytes: u, pastas: 1))
    }
    return cs.filter { $0.bytes >= 1_000_000_000 }.sorted { ($0.bytes, $1.id) > ($1.bytes, $0.id) }
}

/// Texto discreto do dono: o nome do app ou, sem app instalado com o id, o id do container.
nonisolated func nomeDono(_ o: DonoApp) -> String { o.name ?? String(localized: "Container de \(o.bundleId)") }

/// Como o sdtally soube o dono (`owner.source`), para o balão de ajuda.
nonisolated func fonteDono(_ s: String) -> String {
    switch s {
    case "app_bundle": String(localized: "Dentro de um app instalado (Info.plist)")
    case "container": String(localized: "Container do macOS (id gravado pelo sistema)")
    case "group_container": String(localized: "Grupo declarado na assinatura de um único app instalado")
    case "folder_bundle_id": String(localized: "Pasta com o bundle id de um app instalado")
    case "icloud_container": String(localized: "Pasta do iCloud com o bundle id de um app instalado")
    case "folder_app_name": String(localized: "Pasta com o nome de um único app instalado (menos certo)")
    default: s
    }
}

/// O dono de um grupo de linhas: só se todas têm dono e é o mesmo app.
nonisolated func donoComum(_ ls: [LinhaTop]) -> String? {
    let os = ls.compactMap(\.owner)
    return os.count == ls.count && Set(os.map(\.bundleId)).count == 1 ? os.first.map(nomeDono) : nil
}

/// Apelido que o dono deu a uma pasta (dono, 27/10). Chave: o caminho como vem no JSON ("~/..."). Fica só
/// nas preferências deste Mac: não vai para o JSON, o MCP nem a rede.
nonisolated enum Apelidos {
    static let chave = "apelidosPastas"
    static let maximo = 60

    static func todos(_ d: UserDefaults = .standard) -> [String: String] {
        d.dictionary(forKey: chave) as? [String: String] ?? [:]
    }

    /// Vazio ou só espaços apaga o apelido.
    static func definir(_ apelido: String?, para caminho: String, _ d: UserDefaults = .standard) {
        var a = todos(d)
        let t = (apelido ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        a[caminho] = t.isEmpty ? nil : String(t.prefix(maximo))
        d.set(a, forKey: chave)
    }
}

/// Apelido do dono; termo com contagem; senão o provedor do File Provider (`label`), o título do conselho, o `what` da ficha
/// sem o ponto final (fora `user_folder`, que fala de seis pastas), o nome do Finder de uma pasta direta da home
/// ("Música"), "File Provider" para FileProvider/<uuid> sem `label` ou o caminho inteiro. Pasta sem ficha não ganha
/// nome inventado: o dono dá um apelido se quiser (dono, 28/10).
private nonisolated func nomeCausa(_ l: LinhaTop, fichas: MapaFichas?, home: String, apelidos: [String: String]) -> String {
    if let p = l.path, let a = apelidos[p] { return visivel(a) }
    if ["snapshots", "deleted_open", "large_xattrs", "apfs_metadata"].contains(l.term) {
        return nomeTermo(l.term) + (l.count.map { " (\($0))" } ?? "")
    }
    if let r = l.label { return r == "File Provider" ? r : String(localized: "File Provider: \(r)") }
    if let t = l.card.flatMap(tituloConselho) { return t }
    if l.card != "user_folder", let w = fichas?[l.card]?.what.texto { return w.hasSuffix(".") ? String(w.dropLast()) : w }
    guard let p = l.path else { return nomeTermo(l.term) }
    // JSON sem `label` (anterior ao campo ou de outro Mac): o UUID não diz nada ao usuário.
    let fp = "~/Library/Application Support/FileProvider/"
    if p.hasPrefix(fp), !p.dropFirst(fp.count).contains("/") { return "File Provider" }
    if p.hasPrefix("~/"), !p.dropFirst(2).contains("/") { return FileManager.default.displayName(atPath: absoluto(p, home: home)) }
    return visivel(p)
}

/// Conselho da causa: o mesmo `card`; senão o que lista o caminho; senão o mesmo termo (fora tree
/// e protected_folders), com o backup parado antes. Swap: `restart_worth` antes de `swap_large`.
nonisolated func conselho(de c: Causa, em advice: [Conselho]) -> Conselho? {
    if c.id == "swap" { return advice.first { $0.id == "restart_worth" } ?? advice.first { $0.id == "swap_large" } }
    if let card = c.card, let a = advice.first(where: { $0.card == card }) { return a }
    if let p = c.caminho, let a = advice.first(where: { $0.paths.contains(p) }) { return a }
    guard !["tree", "protected_folders"].contains(c.termo) else { return nil }
    let doTermo = advice.filter { $0.term == c.termo }
    return doTermo.first { $0.id == "tm_backup_stuck" } ?? doTermo.first
}

/// Uma cor de sistema por posição nas 5 linhas de B5, na barra e no ícone da linha: as cores das
/// linhas do Ajustes se repetem (várias são Dados do Sistema, cinza) e não se distinguem na barra.
nonisolated let coresDisco: [Color] = [.blue, .orange, .green, .purple, .pink, .teal, .yellow, .indigo, .mint, .brown]
nonisolated func corDisco(_ i: Int) -> Color { coresDisco[i % coresDisco.count] }

/// Barra de B5, do container principal (diskutil) do mesmo instante das causas: as causas fora o
/// swap (que fica no macOS), o resto do volume de dados, o macOS e o livre. Sem causas (sem medição
/// completa), o volume de dados inteiro no lugar do resto. Sem container principal, vazia.
/// O nome leva a fonte, que só aparece no .help.
nonisolated func segmentosDisco(_ cs: [Causa], top: [LinhaTop], containers: [Conteiner]?) -> [Segmento] {
    guard let p = containers.flatMap(Armazenamento.principal) else { return [] }
    let partes = cs.filter { $0.id != "swap" }.enumerated().map { i, c in
        let fonte = top.first { $0.folded != true && $0.path == c.caminho && $0.term == c.termo }.map { " · " + nomeFonte($0.source) } ?? ""
        return Segmento(id: c.id, nome: c.nome + fonte, bytes: c.bytes, cor: corDisco(i))
    }
    let dados = p.volumes.first { $0.roles.contains("Data") }?.capacityInUse ?? 0
    let macos = p.volumes.filter(\.inSettingsMacos).reduce(0) { $0 + ($1.capacityInUse ?? 0) }
    return partes + [
        Segmento(id: "resto", nome: partes.isEmpty ? String(localized: "Volume de dados · diskutil") : String(localized: "Resto do volume de dados · diferença"),
                 bytes: max(0, dados - partes.reduce(0) { $0 + $1.bytes }), cor: Color.secondary.opacity(0.35)),
        Segmento(id: "macos", nome: "macOS · diskutil", bytes: macos, cor: .gray),
        Segmento(id: "livre", nome: String(localized: "Livre · diskutil"), bytes: p.free ?? 0, cor: Color(nsColor: .quaternaryLabelColor)),
    ]
}

/// Uma frase sob o Livre. `ajustes`: o painel do Ajustes do botão (chave de `ajustesPermitidos`).
nonisolated struct Manchete: Equatable { let simbolo: String; let texto: String; let ajustes: String? }

/// A primeira regra que bate (§3 B2), ou nil. R1: backup do Time Machine parado com snapshots de
/// 1 GB ou mais. R2: a maior causa fora o swap passa de 20 % do usado do volume de dados.
nonisolated func manchete(advice: [Conselho]?, tm: Completo.TimeMachine?, snapshots: Int64?,
                          causas: [Causa], usado: Int64?) -> Manchete? {
    if let tm0 = advice?.first(where: { $0.id == "tm_backup_stuck" }), let s = snapshots, s >= 1_000_000_000,
       let d = tm?.destinations?.first(where: { ($0.attemptsSinceCompleted ?? 0) >= 2 && $0.lastCompletedAt != nil }),
       let n = d.attemptsSinceCompleted, let feito = d.lastCompletedAt {
        let dia = Armazenamento.diaCurto(feito)
        return Manchete(simbolo: "clock.arrow.circlepath",
                        texto: String(localized: "Time Machine sem backup concluído desde \(dia) (\(n) tentativas). Snapshots: \(tm0.bytesFloor == true ? "≥ " : "")\(gb(s))."),
                        ajustes: "time_machine")
    }
    if let u = usado, u > 0, let c = causas.first(where: { $0.id != "swap" }), c.bytes * 5 > u {
        let pct = (Double(c.bytes) / Double(u) * 100).formatted(.number.precision(.fractionLength(1)).locale(localApp))
        return Manchete(simbolo: "chart.bar.fill",
                        texto: String(localized: "Maior pedaço: \(c.nome), \(gb(c.bytes)) (\(pct) % do usado)."),
                        ajustes: nil)
    }
    return nil
}

/// true: o purgeable está em Disponível nesta leitura; false: dentro de Dados do Sistema; nil: sem tela.
/// O Ajustes muda entre aberturas do painel (02/10); só compara dois números medidos, o Disponível
/// da tela e o livre do statfs.
nonisolated func purgavelNoAjustes(tela: Tela?, livre: Int64?, purg: Int64?) -> Bool? {
    guard let y = tela?.available, let livre, let purg, purg > 1_000_000_000 else { return nil }
    return y - livre >= 1_000_000_000
}


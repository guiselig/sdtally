// Início (spec-inicio §3, §10): causas, manchete, linha do Ajustes e o purgeable da tela, com os
// números do Mac do dono em linhas sintéticas (/Users/exemplo); mais o passo, o V1 e a barra lateral.
import Foundation
import SwiftUI
import Testing
@testable import SDTally

private func fixture(_ nome: String) throws -> Data {
    let url = try #require(Bundle.module.url(forResource: nome, withExtension: "json", subdirectory: "Fixtures"))
    return try Data(contentsOf: url)
}

private func linhaTop(_ path: String?, term: String = "tree", _ bytes: Int64, card: String? = nil, count: Int? = nil) -> LinhaTop {
    LinhaTop(path: path, term: term, bytes: bytes, privateBytes: nil, lowerBound: false, count: count, topProcess: nil,
             source: "getattrlistbulk", folded: false, card: card)
}

private func causa(_ id: String, termo: String = "tree", _ bytes: Int64, card: String? = nil) -> Causa {
    Causa(id: id, nome: id, caminho: id.hasPrefix("/") || id.hasPrefix("~") ? id : nil, termo: termo, card: card, bytes: bytes, pastas: 1)
}

private func regra(_ id: String, term: String? = nil, card: String? = nil, paths: [String] = []) -> Conselho {
    Conselho(id: id, level: "check_first", bytes: 1, privateBytes: nil, paths: paths, term: term, process: nil, command: nil, text: "texto do sdtally", card: card)
}

private let snapshotsDoDono = causa("term_snapshots", termo: "snapshots", 49_615_863_808, card: "term_snapshots")

@Test func mancheteBackup() {
    let tm = Completo.TimeMachine(state: "measured", reason: nil, source: "tm_preferences_plist", running: false, destinations: [
        .init(referenceAt: "2026-10-01 18:15:08", stableAt: "2026-10-05 05:55:50", lastCompletedAt: "2026-10-01 20:29:20",
              lastAttemptAt: "2026-10-05 16:42:30", attemptsSinceCompleted: 11, resultRaw: 18),
    ])
    let usado: Int64 = 434_859_917_312
    let m = manchete(advice: [regra("tm_backup_stuck", term: "snapshots", card: "tm_backup_stuck")], tm: tm,
                     snapshots: 49_615_863_808, causas: [snapshotsDoDono], usado: usado)
    #expect(m == Manchete(simbolo: "clock.arrow.circlepath",
                          texto: "Time Machine sem backup concluído desde 01/10 (11 tentativas). Snapshots: 49,62 GB.",
                          ajustes: "time_machine"))
    // Com snapshot de outro tipo, o CLI manda um piso (`bytes_floor`): "≥".
    var piso = regra("tm_backup_stuck", term: "snapshots", card: "tm_backup_stuck")
    piso.bytesFloor = true
    #expect(manchete(advice: [piso], tm: tm, snapshots: 2_000_000_000, causas: [], usado: usado)?.texto
        .contains("Snapshots: ≥ 2,00 GB.") == true)
    // Sem o conselho (JSON sem o M2): a maior causa tem 11,4 % do usado, abaixo dos 20 % da R2.
    #expect(manchete(advice: [], tm: tm, snapshots: 49_615_863_808, causas: [snapshotsDoDono], usado: usado) == nil)
    // Snapshots abaixo de 1 GB também não dão a R1.
    #expect(manchete(advice: [regra("tm_backup_stuck")], tm: tm, snapshots: 900_000_000, causas: [], usado: usado) == nil)
}

@Test func manchetePedacoGrande() {
    let usado: Int64 = 365_807_153_152
    let texto = "Maior pedaço: /Users/exemplo, 145,18 GB (39,7 % do usado)."
    let m = manchete(advice: nil, tm: nil, snapshots: nil, causas: [causa("/Users/exemplo", termo: "protected_folders", 145_180_639_232)], usado: usado)
    #expect(m == Manchete(simbolo: "chart.bar.fill", texto: texto, ajustes: nil))
    // O swap não conta como pedaço; a linha do Ajustes não entra na frase (está na lista).
    let swap = causa("swap", termo: "swap", 200_000_000_000)
    let outros = causa("/Users/exemplo", 145_180_639_232)
    #expect(manchete(advice: nil, tm: nil, snapshots: nil, causas: [swap, outros], usado: usado)?.texto == texto)
    #expect(manchete(advice: nil, tm: nil, snapshots: nil, causas: [causa("/Users/exemplo", 145_180_639_232)], usado: usado)?.texto == texto)
}

@Test func purgavelNoAjustes() {
    func tela(disponivel: Int64) -> Tela {
        Tela(state: "measured", reason: nil, loggedAt: "2026-10-05 18:49:00", used: nil, available: disponivel, capacity: nil,
             systemData: 176_450_000_000, categoriesTotal: nil, categories: nil, service: nil)
    }
    #expect(SDTally.purgavelNoAjustes(tela: tela(disponivel: 76_040_000_000), livre: 24_070_000_000, purg: 51_550_000_000) == true)
    #expect(SDTally.purgavelNoAjustes(tela: tela(disponivel: 24_070_000_000), livre: 24_070_000_000, purg: 51_550_000_000) == false)
    #expect(SDTally.purgavelNoAjustes(tela: nil, livre: 24_070_000_000, purg: 51_550_000_000) == nil)
}

@Test func categoriaDaLinha() {
    #expect(categoria("com.apple.STMExtension.OtherUsers").nome == "Outros usuários")
    // Linha que o app não conhece: cinza, nunca o rosa que lembra Dados do Sistema.
    #expect(categoria("x").cor == .gray)
}

@Test func causaAgrupaPorFicha() {
    let top = [
        linhaTop("/System/Library/AssetsV2/com_apple_MobileAsset_UAF_FM_GenerativeModels", term: "protected_folders", 17_397_940_224,
                 card: "apple_intelligence_models"),
        linhaTop("~/Library/Mobile Documents/com~apple~CloudDocs", 37_026_758_656, card: "icloud_drive"),
        linhaTop("~/Library/Mobile Documents/exemplo~App", 2_488_881_152, card: "icloud_drive"),
        linhaTop("/Users/exemplo", term: "protected_folders", 145_180_639_232),
        linhaTop(nil, term: "snapshots", 49_615_863_808, card: "term_snapshots", count: 9),
    ]
    let cs = Dictionary(uniqueKeysWithValues: causas(top: top, swap: nil, fichas: nil, home: "/Users/exemplo").map { ($0.id, $0) })
    #expect(cs["apple_intelligence_models"] != nil)
    // Duas pastas da mesma ficha: um grupo; o caminho é o da maior.
    let icloud = cs["icloud_drive"]
    #expect(icloud?.pastas == 2 && icloud?.bytes == 39_515_639_808 && icloud?.nome == "iCloud Drive")
    #expect(icloud?.caminho == "~/Library/Mobile Documents/com~apple~CloudDocs")
    #expect(cs["/Users/exemplo"] != nil)
    #expect(cs["term_snapshots"]?.nome == "Snapshots locais do Time Machine (9)")
    // A mesma pasta em dois termos (parte legível e parte protegida): soma, mas é uma pasta só.
    let pf = causas(top: [linhaTop("/private/var/folders", 1_380_000_000), linhaTop("/private/var/folders", term: "protected_folders", 840_000_000)],
                    swap: nil, fichas: nil, home: "/Users/exemplo")
    #expect(pf.count == 1 && pf[0].pastas == 1 && pf[0].bytes == 2_220_000_000)
}

/// §10: as linhas do Mac do dono (caminhos sem nome de usuário) dão as 5 causas na ordem do
/// wireframe; o swap entra pela causas() na 9ª posição; dobrada e abaixo de 1 GB ficam de fora.
@Test func causasDoDono() {
    let dobrada = LinhaTop(path: "/Applications", term: "tree", bytes: 2_324_385_792, privateBytes: 0, lowerBound: false, count: nil,
                           topProcess: nil, source: "getattrlistbulk", folded: true)
    let top = [
        linhaTop(nil, term: "snapshots", 49_615_863_808, card: "term_snapshots", count: 9),
        linhaTop("~/.colima/_lima", 41_201_725_440, card: "container_vm"),
        linhaTop("~/Music", 38_263_365_632, card: "user_folder"),
        linhaTop("~/Library/Mobile Documents/com~apple~CloudDocs", 37_026_758_656, card: "icloud_drive"),
        linhaTop("~/Library/Mobile Documents/exemplo~App", 2_488_881_152, card: "icloud_drive"),
        linhaTop("~/Library/Caches/CloudKit/com.apple.bird", 20_643_913_728, card: "cloudkit_cache"),
        linhaTop("/System/Library/AssetsV2/com_apple_MobileAsset_UAF_FM_GenerativeModels", term: "protected_folders", 17_397_940_224, card: "apple_intelligence_models"),
        linhaTop("/System/Library/AssetsV2/com_apple_MobileAsset_UAF_FM_Visual", term: "protected_folders", 100_790_272, card: "apple_intelligence_models"),
        linhaTop("~/Library/Application Support/Claude/vm_bundles", 12_738_473_984, card: "claude_vm_bundles"),
        linhaTop("~/Pictures", 10_444_460_032, card: "user_folder"),
        linhaTop("~/My Drive", 6_438_375_424),
        dobrada,
        linhaTop("~/.gemini", 158_707_712),
    ]
    let cs = causas(top: top, swap: Swap(total: 10_737_418_240, used: 9_181_462_528), fichas: nil, home: "/Users/exemplo")
    #expect(cs.prefix(5).map(\.bytes) == [49_615_863_808, 41_201_725_440, 39_515_639_808, 38_263_365_632, 20_643_913_728])
    #expect(cs[2].pastas == 2 && cs[2].card == "icloud_drive")
    // user_folder não junta Música e Imagens; o nome vem do título ou da pasta, não do what.
    #expect(cs[1].nome == "Máquina virtual de contêineres" && cs[3].nome == "Music" && cs[7].caminho == "~/Pictures")
    #expect(cs[5].bytes == 17_498_730_496)
    #expect(cs[8].id == "swap" && cs[8].bytes == 9_181_462_528)
    #expect(cs.count == 10 && !cs.contains { $0.caminho == "/Applications" || $0.caminho == "~/.gemini" })
}

/// Barra de B5: as causas sem o swap, o resto do volume de dados, o macOS e o livre (diskutil).
@Test func barraDoInicio() {
    func v(_ r: String, _ b: Int64, _ m: Bool) -> Conteiner.Volume { Conteiner.Volume(roles: [r], capacityInUse: b, inSettingsMacos: m) }
    let p = Conteiner(internal: true, isc: false, holdsData: true, ceiling: 494_384_795_648, free: 21_073_506_304,
                      volumes: [v("System", 12_648_747_008, true), v("Data", 434_836_656_128, false), v("VM", 11_811_696_640, true)])
    let s = segmentosDisco([snapshotsDoDono, causa("swap", termo: "swap", 9_181_462_528)], top: [], containers: [p])
    #expect(s.map(\.id) == ["term_snapshots", "resto", "macos", "livre"])
    #expect(s.map(\.bytes) == [49_615_863_808, 385_220_792_320, 24_460_443_648, 21_073_506_304])
    // Sem medição completa: o volume de dados inteiro, o macOS e o livre; sem container principal, nada.
    #expect(segmentosDisco([], top: [], containers: [p]).map(\.bytes) == [434_836_656_128, 24_460_443_648, 21_073_506_304])
    #expect(segmentosDisco([], top: [], containers: nil).isEmpty)
}

/// "Seguro:" de B6: nível safe, alocado (não lógico), 1 GB ou mais e fora das 5 linhas.
@Test func seguroForaDasLinhas() {
    func safe(_ id: String, _ priv: Int64?, logico: Bool? = nil) -> Conselho {
        Conselho(id: id, level: "safe", bytes: 7_780_000_000, privateBytes: priv, paths: [], term: nil, process: nil, command: nil,
                 text: "texto do sdtally", card: id, bytesLogical: logico)
    }
    let cinco = [causa("~/.npm/_cacache", 7_780_000_000, card: "npm_cache")]
    let a = [safe("chronod_datavault", 2_817_310_720, logico: true), safe("npm_cache", 7_780_000_000), safe("pip_cache", 500_000_000),
             regra("logs", card: "logs"), safe("go_build_cache", 2_000_000_000)]
    #expect(seguro(advice: a, cinco: cinco)?.id == "go_build_cache")
    #expect(seguro(advice: a, cinco: [])?.id == "npm_cache")
    #expect(seguro(advice: Array(a.prefix(3)), cinco: cinco) == nil)
}

@Test func conselhoDaCausa() {
    let a = [regra("time_machine_snapshots", term: "snapshots", card: "time_machine_snapshots"),
             regra("tm_backup_stuck", term: "snapshots", card: "tm_backup_stuck"),
             regra("swap_large"), regra("restart_worth", term: "deleted_open"),
             regra("logs", card: "logs", paths: ["~/Library/Logs"])]
    #expect(SDTally.conselho(de: snapshotsDoDono, em: a)?.id == "tm_backup_stuck")
    #expect(SDTally.conselho(de: causa("swap", termo: "swap", 9_181_462_528), em: a)?.id == "restart_worth")
    #expect(SDTally.conselho(de: causa("~/Library/Logs", 2_000_000_000), em: a)?.id == "logs")
    // tree sem card nem caminho num conselho: nenhum (o termo não basta).
    #expect(SDTally.conselho(de: causa("~/Music", 38_263_365_632), em: a) == nil)
}

@Test func vazioDaTelaDoAjustes() {
    let p = Ponto(t: "2026-10-02 09:00", total: nil, usado: nil, livre: nil, purgeable: nil, systemData: 221_266_053_536, telaEm: "2026-10-02 08:41:10.1-0300")
    #expect(vazioAjustes(nil, ultimo: p) == .v1(ultimo: 221_266_053_536, hora: "08:41"))
    #expect(vazioAjustes(nil, ultimo: nil) == .v1(ultimo: nil, hora: nil))
}

/// spec-inicio §1: a barra lateral tem 5 itens; Permissões e Sobre ficam só no enum (⚙ e ⓘ).
@MainActor @Test func secoesDaBarra() {
    #expect(Secao.barra.map(\.titulo) == ["Dados do Sistema", "Disco", "Caminhos", "Histórico", "Recomendações"])
    #expect(Secao.allCases.count == 7)
}

/// Barra lateral só de ícones e janela fixa sem barra de título.
@MainActor @Test func barraEJanelaFixa() {
    #expect(JanelaView.larguraBarra < 100)
    #expect(JanelaView.tamanho.width - JanelaView.larguraBarra >= 560 + 2 * 24)  // cartão de 560 pt com 24 pt de borda
}

@Test func fileProviderComRotulo() throws {
    let d = JSONDecoder()
    d.keyDecodingStrategy = .convertFromSnakeCase
    let json = #"{"path": "~/Library/Application Support/FileProvider/66666666-7777-4888-9999-AAAAAAAAAAAA", "folded": false, "term": "tree", "bytes": 22612619264, "private_bytes": 18649088, "lower_bound": false, "source": "getattrlistbulk", "label": "iCloud Drive"}"#
    let l = try d.decode(LinhaTop.self, from: Data(json.utf8))
    var outro = l
    outro.label = "File Provider"
    #expect(causas(top: [l], swap: nil, fichas: nil, home: "/Users/exemplo").map(\.nome) == ["File Provider: iCloud Drive"])
    #expect(causas(top: [outro], swap: nil, fichas: nil, home: "/Users/exemplo").map(\.nome) == ["File Provider"])
    outro.label = nil
    #expect(causas(top: [outro], swap: nil, fichas: nil, home: "/Users/exemplo").map(\.nome) == ["File Provider"])
    // Sem `label` e fora de FileProvider: o nome curto (a pasta do app em Application Support).
    let sem = linhaTop("~/Library/Application Support/Outro", 2_000_000_000)
    #expect(causas(top: [sem], swap: nil, fichas: nil, home: "/Users/exemplo").map(\.nome) == ["Outro"])
}

@Test func pastaDobradaEmCaminhoComTil() {
    let dobrada = LinhaTop(path: "~/Library", term: "tree", bytes: 1, privateBytes: nil, lowerBound: false, count: nil, topProcess: nil,
                           source: "getattrlistbulk", folded: true)
    #expect(rotulo(dobrada, home: "/Users/exemplo") == "Demais itens em ~/Library")
    #expect(rotulo(linhaTop("~/Library", 1), home: "/Users/exemplo") == "~/Library")
}

// MARK: - dono por app (Fase 2, Frente C)

private func comDono(_ l: LinhaTop, _ id: String?, _ nome: String? = nil) -> LinhaTop {
    var l = l
    l.owner = id.map { DonoApp(bundleId: $0, name: nome, source: "app_bundle") }
    return l
}

@Test func causaTemDonoSoQuandoTodasAsLinhasConcordam() {
    let top = [
        comDono(linhaTop("/Applications/Word.app/Contents/Frameworks", 3_000_000_000, card: "app_bundles"), "com.exemplo.Word", "Word"),
        comDono(linhaTop("/Applications/Word.app/Contents/Resources", 2_000_000_000, card: "app_bundles"), "com.exemplo.Word", "Word"),
        comDono(linhaTop("~/Library/Application Support/Sozinho", 4_000_000_000), "com.exemplo.Sozinho", "Sozinho"),
        linhaTop("~/Library/Caches/Livre", 5_000_000_000),
    ]
    let cs = causas(top: top, swap: nil, fichas: nil, home: "/Users/exemplo")
    #expect(cs.first { $0.id == "app_bundles" }?.dono == "Word")
    #expect(cs.first { $0.id == "~/Library/Application Support/Sozinho" }?.dono == "Sozinho")
    #expect(cs.first { $0.id == "~/Library/Caches/Livre" }?.dono == nil)
    // Duas linhas do mesmo card, apps diferentes (ou um sem dono): nenhum dono.
    let misto = top.prefix(1) + [comDono(linhaTop("/Applications/Excel.app/Contents/Frameworks", 2_000_000_000, card: "app_bundles"), "com.exemplo.Excel", "Excel")]
    #expect(causas(top: Array(misto), swap: nil, fichas: nil, home: "/Users/exemplo").first?.dono == nil)
    let semUma = top.prefix(1) + [linhaTop("/Applications/Outro.app/Contents/Frameworks", 2_000_000_000, card: "app_bundles")]
    #expect(causas(top: Array(semUma), swap: nil, fichas: nil, home: "/Users/exemplo").first?.dono == nil)
}

@Test func fonteDoDonoTemTextoParaCadaFonteDoSdtally() {
    for f in ["app_bundle", "container", "group_container", "folder_bundle_id", "icloud_container", "folder_app_name"] {
        #expect(fonteDono(f) != f)
    }
    #expect(fonteDono("futura") == "futura")
}

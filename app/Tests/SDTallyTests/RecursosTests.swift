// Recursos baixados pelo macOS (spec-fase6 §5.5, A10b): agrupado por card, só "Ocupa".
import Foundation
import Testing
@testable import SDTally

private func completo(_ nome: String) throws -> Completo {
    let url = try #require(Bundle.module.url(forResource: nome, withExtension: "json", subdirectory: "Fixtures"))
    return try decodificar(Completo.self, Data(contentsOf: url), kind: nil)
}

private func linha(_ sufixo: String, _ bytes: Int64, card: String?) -> LinhaTop {
    LinhaTop(path: Recursos.prefixo + sufixo, term: "tree", bytes: bytes, privateBytes: 0, lowerBound: false, count: nil,
             topProcess: nil, source: "getattrlistbulk", folded: false, card: card)
}

@Test func agrupadoPorCard() throws {
    let c = try completo("completo-assinaturas")
    let r = Recursos.grupos(c.top, fichas: c.cards)
    #expect(r.grupos.map(\.id) == ["asset_sim_runtime", "asset_os_update", "asset_xcode", "asset_siri_speech", "asset_voices", "asset_language"])
    #expect(r.grupos.first?.nome == "Runtimes de simulador baixados pelo macOS")  // what.pt sem o ponto
    #expect(r.grupos.allSatisfy { $0.passo != nil })
    #expect(r.outros == nil)
    // Duas pastas da mesma ficha viram uma linha; a pasta /System/Library/AssetsV2 inteira não entra.
    let d = Recursos.grupos([linha("UAF_Siri_A", 600_000_000, card: "asset_siri_speech"), linha("UAF_Speech_B", 500_000_000, card: "asset_siri_speech"),
                             LinhaTop(path: "/System/Library/AssetsV2", term: "protected_folders", bytes: 9, privateBytes: nil, lowerBound: true,
                                      count: nil, topProcess: nil, source: "fsctl_dir_stats", folded: false, card: "system_folders")],
                            fichas: nil)
    #expect(d.grupos == [Recursos.Grupo(id: "asset_siri_speech", nome: "UAF_Siri_A", passo: nil, bytes: 1_100_000_000, piso: false)])
}

@Test func semLiberaSeApagar() throws {
    // privado 0 em todas: o número mostrado é o Ocupa (`bytes`), nunca o privado.
    let c = try completo("completo-assinaturas")
    let r = Recursos.grupos(c.top, fichas: c.cards)
    #expect(r.grupos.first { $0.id == "asset_xcode" }?.bytes == 1_730_000_000)
    let fonte = try String(contentsOf: URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().appending(path: "Sources/SDTally/RecursosView.swift"), encoding: .utf8)
    #expect(!fonte.contains("Libera se apagar") && !fonte.contains("privateBytes"))
    #expect(fonte.contains("\"Ocupa\""))
}

@Test func pequenosDobrados() {
    let r = Recursos.grupos([linha("A", 2_000_000_000, card: nil), linha("B", 60_000_000, card: nil), linha("C", 30_000_000, card: "asset_language")], fichas: nil)
    #expect(r.grupos.map(\.nome) == ["A"])  // sem ficha: o nome da pasta
    #expect(r.outros == 90_000_000)
}

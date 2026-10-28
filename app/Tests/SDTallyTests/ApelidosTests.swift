// Nomes das pastas: apelido do dono ou o caminho (Causas.swift).
import Foundation
import Testing
@testable import SDTally

private func linha(_ p: String, _ bytes: Int64 = 5_000_000_000) -> LinhaTop {
    LinhaTop(path: p, term: "tree", bytes: bytes, privateBytes: nil, lowerBound: false, count: nil, topProcess: nil,
             source: "teste", folded: nil)
}

@Test func apelidoVemAntesDeTudo() {
    let p = "~/Library/Application Support/App/Dados/sessao"
    let cs = causas(top: [linha(p)], swap: nil, fichas: nil, home: "/Users/exemplo", apelidos: [p: "Sessões do app"])
    #expect(cs.map(\.nome) == ["Sessões do app"])
    #expect(cs.first?.caminho == p)  // o caminho real continua na causa
}

@Test func pastaSemFichaMostraOCaminho() {
    let p = "~/Music/Music/Media.localized"
    #expect(causas(top: [linha(p)], swap: nil, fichas: nil, home: "/Users/exemplo", apelidos: [:]).map(\.nome) == [p])
}

@Test func definirApelidoLimpaECorta() throws {
    let d = try #require(UserDefaults(suiteName: "apelidos-teste-\(UUID())"))
    Apelidos.definir("  Músicas  ", para: "~/Music/x", d)
    #expect(Apelidos.todos(d) == ["~/Music/x": "Músicas"])
    Apelidos.definir(String(repeating: "a", count: 100), para: "~/y", d)
    #expect(Apelidos.todos(d)["~/y"]?.count == Apelidos.maximo)
    Apelidos.definir("   ", para: "~/Music/x", d)  // vazio apaga
    #expect(Apelidos.todos(d)["~/Music/x"] == nil)
}

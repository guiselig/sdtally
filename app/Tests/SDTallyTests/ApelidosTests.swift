// Nomes das pastas: apelido do dono, bibliotecas conhecidas e nome curto (Causas.swift).
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

@Test func bibliotecaConhecidaTemNome() {
    let cs = causas(top: [linha("~/Music/Music/Media.localized")], swap: nil, fichas: nil, home: "/Users/exemplo", apelidos: [:])
    #expect(cs.map(\.nome) == ["Biblioteca do app Música"])
}

@Test func nomeCurtoUsaAppDonoOuPastaMae() {
    #expect(nomeCurto("~/Library/Application Support/Claude/a/b/Docs/session", home: "/Users/exemplo") == "Claude › session")
    #expect(nomeCurto("~/Library/Caches/com.exemplo.app/fsCachedData", home: "/Users/exemplo") == "com.exemplo.app › fsCachedData")
    #expect(nomeCurto("/opt/exemplo/dados/grande", home: "/Users/exemplo") == "dados › grande")
    #expect(nomeCurto("~/Library/Application Support/Outro", home: "/Users/exemplo") == "Outro")
    // Curto demais para cortar: o caminho como está.
    #expect(nomeCurto("/opt/x", home: "/Users/exemplo") == "/opt/x")
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

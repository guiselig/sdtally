// O que o macOS resolve sozinho (spec-fase6 §4.8, §5.3).
import Foundation
import Testing
@testable import SDTally

private func fixture(_ nome: String) throws -> Data {
    let url = try #require(Bundle.module.url(forResource: nome, withExtension: "json", subdirectory: "Fixtures"))
    return try Data(contentsOf: url)
}

@Test func purgeableNuncaSomaSnapshots() throws {
    let c = try decodificar(Completo.self, fixture("completo"), kind: nil)
    let r = try decodificar(Rapido.self, fixture("quick"), kind: "quick")
    for (cc, rr) in [(c, nil), (nil, r), (c, r)] as [(Completo?, Rapido?)] {
        #expect(cuidaSozinho(completo: cc, rapido: rr, lixeiraAuto: true).titulo == 47_000_000_000)
    }
    // Sem backup parado, o serviço do Time Machine vai na linha de snapshots, não numa linha própria.
    let ls = cuidaSozinho(completo: c, rapido: nil, lixeiraAuto: nil).linhas
    let s = try #require(ls.first { $0.nome.hasPrefix("Snapshots do Time Machine: 6, o mais antigo de 01/10 às 18:05") })
    #expect(s.bytes == 41_000_000_000 && s.detalhe?.contains("24 h") == true)
    #expect(!ls.contains { $0.nome == "Time Machine" })
    #expect(ls.contains { $0.nome == "Lixo automático (remover após 30 dias): não definido" })
    #expect(!ls.contains { $0.nome.hasPrefix("Atualização do macOS") })
    // Só o rápido: sem snapshots medidos, o serviço do Time Machine fica na lista.
    #expect(cuidaSozinho(completo: nil, rapido: r, lixeiraAuto: false).linhas.contains { $0.nome == "Time Machine" })
}

@Test func tituloSemTMComBackupParado() throws {
    let c = try decodificar(Completo.self, fixture("completo-assinaturas"), kind: nil)
    #expect(backupParado(c))
    let (t, ls) = cuidaSozinho(completo: c, rapido: nil, lixeiraAuto: true)
    #expect(t == Int64(47_000_000_000 - 41_000_000_000))
    #expect(!ls.contains { $0.nome == "Time Machine" })
    let s = try #require(ls.first { $0.nome.hasPrefix("Snapshots do Time Machine") })
    #expect(s.detalhe == "O mais antigo é a base do próximo backup. Se faltar espaço, o macOS pode apagá-lo; aí o próximo backup demora mais.")
    #expect(ls.first { $0.nome == "Atualização do macOS baixada" }?.bytes == 6_000_000_000)
    #expect(ls.contains { $0.nome.hasSuffix(": ligado") })
}

@Test func cuidaTextosPassamNoG9() throws {
    let raiz = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
    let p = try String(contentsOf: raiz.appending(path: "scripts/g9-proibidas.txt"), encoding: .utf8).split(separator: "\n").map(String.init)
    let c = try decodificar(Completo.self, fixture("completo-assinaturas"), kind: nil)
    let textos = cuidaSozinho(completo: c, rapido: nil, lixeiraAuto: nil).linhas.flatMap { [$0.nome, $0.detalhe ?? ""] }
        + reiniciar(completo: c).libera.map { $0.nome + ($0.detalhe ?? "") } + [reiniciar(completo: c).rodape]
    for t in textos { for x in p where !x.isEmpty { #expect(!t.contains(x), "\(t) contém \(x)") } }
}

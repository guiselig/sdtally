// Snapshots, raio-X (spec-fase6 §5.4, A10b): presos, textos da Situação, idade e criador.
import Foundation
import Testing
@testable import SDTally

private func completo(_ nome: String) throws -> Completo {
    let url = try #require(Bundle.module.url(forResource: nome, withExtension: "json", subdirectory: "Fixtures"))
    return try decodificar(Completo.self, Data(contentsOf: url), kind: nil)
}

private func snap(_ verdict: String?, purgeable: Bool? = nil, kind: String = "time_machine", date: String? = nil, creator: String? = nil) -> Completo.Snapshot {
    Completo.Snapshot(kind: kind, date: date, privateBytes: 1, purgeable: purgeable, creator: creator, verdict: verdict)
}

@Test func presosETermoMenosPrivados() throws {
    let c = try completo("completo")
    let s = try #require(c.informational?.snapshots)
    let termo = try #require(c.dataVolume.terms.first { $0.key == "snapshots" }?.bytes)
    let privados = s.reduce(0) { $0 + ($1.privateBytes ?? 0) }
    #expect(RaioX.presos(c) == termo - privados)
    #expect(RaioX.presos(c) == 26_000_000_000)  // 60 GB − 34 GB
    // Lista vazia com termo 0: nada preso.
    #expect(RaioX.presos(try completo("completo-vazios")) == 0)
}

@Test func tmReferencePelosTresPurgeable() {
    #expect(RaioX.situacao(snap("tm_reference", purgeable: true)) == "Base do próximo backup")
    #expect(RaioX.situacao(snap("tm_reference", purgeable: false)) == "Base do próximo backup · não removível")
    #expect(RaioX.situacao(snap("tm_reference", purgeable: nil)) == "Base do próximo backup")
    // A explicação fica só no tooltip.
    #expect(RaioX.situacaoDica(snap("tm_reference", purgeable: true)).contains("o macOS pode apagá-lo"))
}

@Test func verdictNullDaTraco() {
    #expect(RaioX.situacao(snap(nil)) == "—")
    #expect(RaioX.situacao(snap("algo_novo")) == "—")
    #expect(RaioX.situacao(snap("tm_aged")) == "Mais de 24 h")
}

@Test func idadeSoNoTM() {
    #expect(RaioX.idade(snap("tm_recent", date: "2026-10-02 05:12:00"), geradoEm: "2026-10-02 09:36") == "4 h")
    #expect(RaioX.idade(snap("tm_reference", date: "2026-09-30 21:15:08"), geradoEm: "2026-10-02 09:36") == "36 h")
    #expect(RaioX.idade(snap("tm_aged", date: "2026-09-27 09:00:00"), geradoEm: "2026-10-02 09:36") == "5 dias")
    #expect(RaioX.idade(snap(nil), geradoEm: "2026-10-02 09:36") == "—")  // TM sem data
    #expect(RaioX.idade(snap("third_party", kind: "other", date: "2026-10-02 05:12:00"), geradoEm: "2026-10-02 09:36") == "—")
}

@Test func criadorEscapado() {
    #expect(RaioX.criadoPor(snap("tm_recent", creator: "com.apple.TimeMachine")) == "Time Machine")
    #expect(RaioX.criadoPor(snap("apple_other", kind: "apple_other", creator: "com.apple.os")) == "macOS (com.apple.os)")
    #expect(RaioX.criadoPor(snap("third_party", kind: "other", creator: "com.exemplo\u{202E}x")) == "com.exemplo\\u{202E}x")
    #expect(RaioX.criadoPor(snap("third_party", kind: "other")) == "Outro app")
}

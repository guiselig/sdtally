// "Por que o espaço pode não voltar" (not_freed, src/voltou.rs) e "Ver antes" (advice[].dry_run,
// src/conselho.rs SIMULACOES). Funções puras sobre completo.json.
import Foundation
import Testing
@testable import SDTally

private func completo() throws -> Completo {
    let url = try #require(Bundle.module.url(forResource: "completo", withExtension: "json", subdirectory: "Fixtures"))
    return try decodificar(Completo.self, Data(contentsOf: url), kind: nil)
}

@Test func causasNaoVoltouSoComTextoENumero() throws {
    let c = try completo()
    let cs = causasNaoVoltou(c.notFreed)
    #expect(cs.map(\.id) == ["local_snapshot", "purgeable", "trash", "deleted_open"])
    #expect(tituloNaoVoltou(cs[0]) == "Snapshots locais (8)")
    #expect(tituloNaoVoltou(cs[1]) == "Purgeable")
    // Sem a chave (arquivo antigo) ou com causa vazia: nada.
    #expect(causasNaoVoltou(nil).isEmpty)
    let vazia = NaoVoltou(id: "x", title: TextoFicha(pt: "", en: ""), text: TextoFicha(pt: "t", en: "t"), bytes: 5, bytesFloor: nil, count: nil, source: "cachedelete")
    let zero = NaoVoltou(id: "y", title: TextoFicha(pt: "t", en: "t"), text: TextoFicha(pt: "t", en: "t"), bytes: 0, bytesFloor: nil, count: nil, source: "cachedelete")
    #expect(causasNaoVoltou([vazia, zero]).isEmpty)
    let arq = NaoVoltou(id: "shared_clone", title: TextoFicha(pt: "Clones APFS", en: ""), text: TextoFicha(pt: "t", en: ""), bytes: 2_000_000_000, bytesFloor: true, count: 1, source: "getattrlistbulk")
    #expect(tituloNaoVoltou(arq) == "Clones APFS (1 arquivo)")
}

@Test func verAntesSoDaListaFixa() throws {
    let c = try completo()
    let sim = try #require(c.advice?.first { $0.id == "simulators" })
    #expect(verAntes(sim)?.command == "xcrun simctl list devices")
    // Comando que já só lê não ganha outra linha.
    #expect(try verAntes(#require(c.advice?.first { $0.id == "homebrew_cache" })) == nil)
    // Simulação fora da lista fixa (JSON adulterado) ou comando fora da lista: sem linha.
    var mexido = sim
    mexido.dryRun = SimulacaoComando(command: "rm -rf ~", doc: "x")
    #expect(verAntes(mexido) == nil)
    mexido.dryRun = SimulacaoComando(command: "xcrun simctl list devices", doc: "x")
    mexido = Conselho(id: sim.id, level: sim.level, bytes: 1, privateBytes: nil, paths: [], term: nil, process: nil, command: "rm -rf ~", text: "t", dryRun: mexido.dryRun)
    #expect(verAntes(mexido) == nil)
    #expect(leiturasPermitidas.count == 8)
}

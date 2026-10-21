// Vale a pena reiniciar? (spec-fase6 §4.8, §5.5).
import Foundation
import Testing
@testable import SDTally

private func fixture(_ nome: String) throws -> Data {
    let url = try #require(Bundle.module.url(forResource: nome, withExtension: "json", subdirectory: "Fixtures"))
    return try Data(contentsOf: url)
}

@Test func logicoForaDaSoma() throws {
    let c = try decodificar(Completo.self, fixture("completo-assinaturas"), kind: nil)
    let k = reiniciar(completo: c, agora: try #require(data("2026-10-02 09:36")))
    // Coluna 1: swap + termo deleted_open medido; o chronod (lógico) não entra.
    #expect(k.libera.compactMap(\.bytes).reduce(0, +) == 8_589_934_592 + 1_000_000_000)
    #expect(!k.libera.contains { $0.nome == "chronod" })
    #expect(k.podeLiberar == [LinhaReinicio(nome: "chronod", detalhe: "Lógicos pelo lsof; o alocado não é legível nessa pasta do sistema",
                                            bytes: 3_900_000_000, fonte: "lsof_proc_pidfdinfo")])
    #expect(k.naoMuda.first?.bytes == 4_300_000_000 && k.naoMuda.last?.bytes == 60_000_000_000)
    #expect(k.rodape == "Ligado há 4 dias")
}

@Test func vmMenosSwapNegativoDaTraco() throws {
    var j = try JSONSerialization.jsonObject(with: fixture("completo")) as! [String: Any]
    j["swap"] = ["source": "sysctl_vm_swapusage", "total": 20_000_000_000, "used": 1]
    let c = try decodificar(Completo.self, JSONSerialization.data(withJSONObject: j), kind: nil)
    let l = try #require(reiniciar(completo: c).naoMuda.first { $0.nome.hasPrefix("Resto do VM") })
    #expect(l.traco && l.bytes == nil)
    // Positivo: o número, sem traço.
    let ok = try #require(reiniciar(completo: try decodificar(Completo.self, fixture("completo"), kind: nil)).naoMuda.first { $0.nome.hasPrefix("Resto do VM") })
    #expect(!ok.traco && ok.bytes == 9_663_676_416 - 8_589_934_592)
}

@Test func reiniciarVazios() throws {
    let c = try decodificar(Completo.self, fixture("completo-vazios"), kind: nil)
    let k = reiniciar(completo: c)
    #expect(k.podeLiberar == [])
    #expect(k.rodape.isEmpty)
}

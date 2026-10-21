// Selo do F6 (spec-fase6 §5.7): 5 estados, versão fora de `checked_on`, sem catálogo, comando fora da lista.
import Foundation
import Testing
@testable import SDTally

private func catalogo(_ mexer: (inout [String: Any]) -> Void = { _ in }) throws -> Catalogo {
    let url = try #require(Bundle.module.url(forResource: "catalogo", withExtension: "json", subdirectory: "Fixtures"))
    var j = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as! [String: Any]
    mexer(&j)
    return try decodificar(Catalogo.self, JSONSerialization.data(withJSONObject: j), kind: "catalog")
}

private func m(_ source: String, _ state: String = "measured", reason: String? = nil) -> Medida {
    Medida(bytes: state == "not_measured" ? nil : 1, state: state, reason: reason, source: source)
}

@Test func cincoEstados() throws {
    let c = try catalogo()
    let conferido = selo(medida: m("attr_vol_spaceused"), catalogo: c, macos: "27.0")
    #expect(conferido.estado == .conferido && conferido.simbolo == "checkmark.seal")
    #expect(conferido.texto == "Conferido ao byte pelo projeto contra a Apple (macOS 27, 01/10).")
    #expect(conferido.comando == "df -k /System/Volumes/Data")
    #expect(selo(medida: m("fsctl_snapshot_private_size"), catalogo: c, macos: "27.0").estado == .margem)
    let medido = selo(medida: m("listxattr"), catalogo: c, macos: "27.0")
    #expect(medido.estado == .medido && medido.texto == "Medido; sem comparação com a Apple." && medido.comando == nil)
    #expect(selo(medida: m("getattrlistbulk", "partial"), catalogo: c, macos: "27.0").texto == "Piso: só o que foi medido.")
    #expect(selo(medida: m("difference", "partial"), catalogo: c, macos: "27.0").texto == "Teto.")
    let nao = selo(medida: m("listxattr", "not_measured", reason: "no root"), catalogo: c, macos: "27.0")
    #expect(nao.estado == .naoMedido && nao.texto == "Não medido: no root." && nao.simbolo == "circle.dashed")
}

@Test func versaoForaDeOnDaMedido() throws {
    let s = selo(medida: m("statfs"), catalogo: try catalogo(), macos: "15.6")
    #expect(s.estado == .medido && s.texto == "Medido. Neste macOS (15.6) ainda não conferido; conferido no 27.")
    #expect(s.comando == "df -k /")  // o comando continua reproduzível
    #expect(selo(medida: m("attr_vol_spaceused"), catalogo: try catalogo(), macos: "15.6").estado == .conferido)
}

@Test func semCatalogoOuFonteForaDaMedido() throws {
    #expect(selo(medida: m("attr_vol_spaceused"), catalogo: nil, macos: "27.0").estado == .medido)
    #expect(selo(medida: m("fonte_nova"), catalogo: try catalogo(), macos: "27.0").estado == .medido)
}

@Test func comandoForaDaListaSemBotao() throws {
    let c = try catalogo { j in
        var s = j["sources"] as! [String: Any]
        var a = s["attr_vol_spaceused"] as! [String: Any]
        a["apple_command"] = "rm -rf ~"
        s["attr_vol_spaceused"] = a
        j["sources"] = s
    }
    let s = selo(medida: m("attr_vol_spaceused"), catalogo: c, macos: "27.0")
    #expect(s.estado == .conferido && s.comando == nil)
}

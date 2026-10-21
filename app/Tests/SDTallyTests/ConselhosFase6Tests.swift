// O que fazer da fase 6 (A11): cartão pela ficha, evidência, lógico, Ajustes permitidos e
// "O que não deu para conferir". Funções puras sobre completo-assinaturas.json.
import Foundation
import Testing
@testable import SDTally

private func assinaturas() throws -> Completo {
    let url = try #require(Bundle.module.url(forResource: "completo-assinaturas", withExtension: "json", subdirectory: "Fixtures"))
    return try decodificar(Completo.self, Data(contentsOf: url), kind: nil)
}

private func item(_ c: Completo, _ id: String) throws -> Conselho {
    try #require(c.advice?.first { $0.id == id })
}

/// Fichas do catálogo sintético com uma ficha mexida no JSON.
private func fichas(_ mexer: (inout [String: Any]) -> Void) throws -> MapaFichas {
    let url = try #require(Bundle.module.url(forResource: "catalogo", withExtension: "json", subdirectory: "Fixtures"))
    var j = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as! [String: Any]
    var cards = j["cards"] as! [String: Any]
    mexer(&cards)
    j["cards"] = cards
    return try decodificar(Catalogo.self, JSONSerialization.data(withJSONObject: j), kind: "catalog").cards
}

@Test func semFichaVoltaAoTextoConselho() throws {
    let c = try assinaturas()
    let trash = try item(c, "trash")
    let f = try #require(c.cards?[trash.card])
    #expect(textoMostrado(trash, ficha: f) == f.step.texto)
    #expect(textoMostrado(trash, ficha: nil) == textoConselho("trash"))
    // Card fora de `cards`: nil, e o cartão volta à fase 5.
    #expect(c.cards?["ficha_que_nao_existe"] == nil)
    // measure_as_admin com ficha: o percentual vem antes do passo da ficha.
    let admin = try item(c, "measure_as_admin")
    let fa = try #require(c.cards?[admin.card])
    #expect(textoMostrado(admin, ficha: fa, percentual: 3.9) == "Hoje 3,9 % do usado está bloqueado pelo macOS. " + fa.step.texto)
}

@Test func painelForaDeAjustesPermitidosSemBotao() throws {
    #expect(Set(ajustesPermitidos.keys) == ["storage", "time_machine", "spotlight"])
    let c = try assinaturas()
    let tm = try #require(c.cards?["tm_backup_stuck"])
    #expect(botoesAjustes(tm).map(\.url) == [ajustesPermitidos["time_machine"]!])
    #expect(botoesAjustes(nil).isEmpty)
    let mexidas = try fichas { cards in
        var f = cards["spotlight_index_large"] as! [String: Any]
        f["actions"] = [["kind": "settings", "target": "privacy", "command": NSNull()]]
        cards["spotlight_index_large"] = f
    }
    let spot = try #require(mexidas["spotlight_index_large"])
    #expect(spot.acoes == [.ajustes("privacy")])
    #expect(botoesAjustes(spot).isEmpty)
}

@MainActor @Test func armazenamentoIgualAoPainel() {
    #expect(ajustesPermitidos["storage"] == Painel.urlArmazenamento)
}

@Test func evidenceRenderizada() throws {
    let c = try assinaturas()
    let spot = try item(c, "spotlight_index_large")
    #expect(spot.kind == "signature")
    #expect(spot.evidence?.map(linhaEvidencia) == ["Espaço privado: 22,00 GB, limite 20,00 GB; fonte: travessia"])
    let chronod = try item(c, "chronod_datavault")
    let linhas = try #require(chronod.evidence?.map(linhaEvidencia))
    #expect(linhas.count == 3)
    #expect(linhas[0] == "Arquivos sem tamanho alocado legível: 247, limite 50; fonte: lsof")
    #expect(linhas[1] == "Tamanho lógico (alocado ilegível): 3,90 GB, limite 1,00 GB; fonte: lsof")
}

@Test func bytesLogicalMostraLogico() throws {
    let c = try assinaturas()
    let chronod = try item(c, "chronod_datavault")
    let m = try #require(numerosCartao(chronod, ficha: c.cards?[chronod.card], piso: false))
    #expect(m.rotulo.lowercased().contains("lógico"))
    #expect(m.rotulo != "Libera se apagar")
    // AssetsV2 com privado 0 e ação sem apagar: Ocupa, nunca "Libera se apagar 0".
    let voz = try item(c, "asset_voices")
    #expect(numerosCartao(voz, ficha: c.cards?[voz.card], piso: false)?.rotulo == "Ocupa")
    // Sem ficha, a regra da fase 5.
    let trash = try item(c, "trash")
    #expect(numerosCartao(trash, ficha: nil, piso: false)?.rotulo == "Libera se apagar")
    // Backup parado com privado > 0 na referência: Ocupa, nunca "Libera se apagar" (R1).
    let tm = try item(c, "tm_backup_stuck")
    #expect(numerosCartao(tm, ficha: c.cards?[tm.card], piso: false)?.rotulo == "Ocupa")
}

@Test func fichaNaOrdemETextos() throws {
    let c = try assinaturas()
    let chronod = try #require(c.cards?["chronod_datavault"])
    #expect(textoSozinho(chronod) == "Quando: ao encerrar a sessão ou reiniciar")
    #expect(textoSozinho(try #require(c.cards?["system_folders"])) == "Não")
    #expect(textoSozinho(try #require(c.cards?["spotlight_index_large"])) == "Não informado")
    #expect(textoConferido(chronod) == "Texto conferido no macOS 27")
    // Relato: texto com "Copiar link" só para https; nada é aberto.
    #expect(linkRelato(RefFicha(kind: "report", ref: "https://exemplo.invalid/relato")) == "https://exemplo.invalid/relato")
    #expect(linkRelato(RefFicha(kind: "report", ref: "file:///etc/passwd")) == nil)
    #expect(linkRelato(RefFicha(kind: "internal", ref: "https://exemplo.invalid")) == nil)
    #expect(textoRef(RefFicha(kind: "measured", ref: "research/mapa-de-fontes.md §3")) == "Medido pelo projeto: research/mapa-de-fontes.md §3")
}

@Test func naoConferidoComNomeEAcao() throws {
    let c = try assinaturas()
    let nc = try #require(c.adviceUnchecked)
    let linhas = nc.map { linhaNaoConferido(nome: tituloConselho($0.card ?? $0.id) ?? $0.id, reason: $0.reason) }
    #expect(linhas == [
        "Backup do Time Machine sem registro novo: não deu para saber se o Time Machine está fazendo backup agora",
        "Cópias apagadas que o chronod segura: a lista de apagados não rodou",
    ])
    #expect(acaoNaoConferido("term not measured") == .medir)
    #expect(acaoNaoConferido("no lsof") == .medirDeNovo)
    #expect(acaoNaoConferido("motivo novo") == nil)
    // Todo id de advice da fixture tem título.
    for a in c.advice ?? [] { #expect(tituloConselho(a.id) != nil, "\(a.id)") }
}

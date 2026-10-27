import Testing
@testable import SDTally

@Test func itensDoMenuDeContexto() {
    #expect(itensContexto(barra: false, medindo: false, podeParar: false, soAuto: false) == [.novaJanela, .medir])
    #expect(itensContexto(barra: true, medindo: false, podeParar: false, soAuto: false) == [.abrir, .medir, .mostrarNaBarra, .configuracoes, .separador, .sair])
    // Medindo: Parar no lugar da medição; pelo osascript ele fica desligado.
    #expect(itensContexto(barra: false, medindo: true, podeParar: true, soAuto: false) == [.novaJanela, .parar(ativo: true)])
    #expect(itensContexto(barra: true, medindo: true, podeParar: false, soAuto: false) == [.abrir, .parar(ativo: false), .mostrarNaBarra, .configuracoes, .separador, .sair])
    // Automático com root: sem medição manual.
    #expect(itensContexto(barra: false, medindo: false, podeParar: false, soAuto: true) == [.novaJanela])
}

@MainActor @Test func usadoDeTotalNaBarra() {
    #expect(usadoDeTotal(usado: 312_400_000_000, total: 494_380_000_000).hasSuffix(" GB"))
    #expect(usadoDeTotal(usado: 312_400_000_000, total: 494_380_000_000).replacingOccurrences(of: "\u{a0}", with: " ") == "312 / 494 GB")
    #expect(opcoesRotuloBarra.map(\.chave) == ["livre", "liberado", "sistema", "usado", "icone"])
}

// Quando a folha de novidades aparece (Novidades.deveMostrar).
import Testing
@testable import SDTally

private func com11(_ v: String) -> Bool { v == "1.1" }

@Test func novidadesInstalacaoNova() {
    // Boas-vindas ainda não vistas: elas aparecem no lugar, e o Começar grava a versão.
    #expect(!Novidades.deveMostrar(versaoApp: "1.1.0", vista: nil, viuBoasVindas: false, temLista: com11))
}

@Test func novidadesAtualizacaoComLista() {
    #expect(Novidades.deveMostrar(versaoApp: "1.1.0", vista: nil, viuBoasVindas: true, temLista: com11))
    #expect(Novidades.deveMostrar(versaoApp: "1.1.0", vista: "1.0", viuBoasVindas: true, temLista: com11))
}

@Test func novidadesVersaoSemLista() {
    #expect(!Novidades.deveMostrar(versaoApp: "1.2.0", vista: "1.1", viuBoasVindas: true, temLista: com11))
}

@Test func novidadesJaVistas() {
    #expect(!Novidades.deveMostrar(versaoApp: "1.1.0", vista: "1.1", viuBoasVindas: true, temLista: com11))
    // Correção da mesma versão não mostra de novo.
    #expect(!Novidades.deveMostrar(versaoApp: "1.1.3", vista: "1.1", viuBoasVindas: true, temLista: com11))
    // Comparação numérica: 1.10 é mais nova que 1.9.
    #expect(!Novidades.deveMostrar(versaoApp: "1.9.0", vista: "1.10", viuBoasVindas: true, temLista: { _ in true }))
}

@Test func novidadesListaDa11() {
    #expect(Novidades.maiorMenor("1.1.0") == "1.1")
    #expect(Novidades.itens("1.1")?.count == 3)
    #expect(Novidades.itens("1.2")?.count == 4)
}

@Test func avisoDeRedeQuemPulouA11() {
    // 1.0.x direto para uma 1.2 sem lista: nenhuma folha aparece, mas a busca automática é liberada.
    #expect(!Novidades.deveMostrar(versaoApp: "1.2.0", vista: nil, viuBoasVindas: true, temLista: com11))
    #expect(Novidades.avisoDeRedeVisto(jaAvisou: false, viuBoasVindas: true))
    // Instalação nova antes do Começar: segue travada.
    #expect(!Novidades.avisoDeRedeVisto(jaAvisou: false, viuBoasVindas: false))
    #expect(Novidades.avisoDeRedeVisto(jaAvisou: true, viuBoasVindas: false))
}

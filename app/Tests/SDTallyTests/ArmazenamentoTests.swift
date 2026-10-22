// Armazenamento e Resumo (A3): as contas puras sobre as fixtures sintéticas.
import Foundation
import Testing
@testable import SDTally

private func fixture(_ nome: String) throws -> Data {
    let url = try #require(Bundle.module.url(forResource: nome, withExtension: "json", subdirectory: "Fixtures"))
    return try Data(contentsOf: url)
}

@Test func somaDosVolumesEOMacosDaTela() throws {
    for r in [try decodificar(Rapido.self, fixture("quick"), kind: "quick")] {
        #expect(Armazenamento.macosDaTela(try #require(r.containers)) == r.settingsScreen.macos.bytes)
    }
    let c = try decodificar(Completo.self, fixture("completo"), kind: nil)
    #expect(Armazenamento.macosDaTela(try #require(c.containers)) == c.settingsScreen.macos.bytes)
}

@Test func restoDoVM() throws {
    let r = try decodificar(Rapido.self, fixture("quick"), kind: "quick")
    let cs = try #require(r.containers)
    #expect(Armazenamento.restoVM(cs, swap: r.swap) == 2_147_483_648)  // 6,44 GB do VM − 4,29 GB de swap
    // Swap maior que o VM: leituras em momentos diferentes, nunca um número negativo.
    let n = try decodificar(Rapido.self, fixture("quick-vm-negativo"), kind: "quick")
    #expect(Armazenamento.restoVM(try #require(n.containers), swap: n.swap) == nil)
    #expect(Armazenamento.restoVM(cs, swap: nil) == nil)
}

@Test func soContainersInternos() throws {
    let cs = try #require(try decodificar(Rapido.self, fixture("quick"), kind: "quick").containers)
    let i = Armazenamento.internos(cs)
    #expect(i.count == 2 && i.allSatisfy { $0.internal == true })
    #expect(Armazenamento.principal(cs)?.holdsData == true)
    // O externo (400 GB em uso) não entra no macOS da tela nem no VM.
    #expect(Armazenamento.macosDaTela(cs) < 400_000_000_000)
}

@Test func agrupamentoDasPartes() throws {
    let c = try decodificar(Completo.self, fixture("completo"), kind: nil)
    let p = Dictionary(uniqueKeysWithValues: Armazenamento.partes(c.top).map { ($0.prefixo, $0) })
    // ~/Library/Caches dobrada (6) + Homebrew (3,2) + CloudKit (2,5); a dobrada de ~/Library é ancestral e não conta.
    #expect(p["~/Library/Caches"]?.bytes == 11_700_000_000)
    #expect(p["~/Library/Developer"]?.bytes == 23_000_000_000)
    #expect(p["/private/var/vm"]?.bytes == 4_300_000_000)
    #expect(p["/System"]?.piso == true && p["/Library"]?.piso == false)
    // Componente inteiro: ~/Library/Application Support/Claude entra, /Library não pega ~/Library.
    #expect(p["~/Library/Application Support"]?.bytes == 10_000_000_000)
    #expect(p["/Library"]?.bytes == 5_000_000_000)
    // Ordem da tela, sem grupo vazio.
    let ordem = Armazenamento.grupos.map(\.prefixo)
    let ids = Armazenamento.partes(c.top).map(\.prefixo)
    #expect(ids == ordem.filter { ids.contains($0) })
    #expect(Armazenamento.partes([]).isEmpty)
}

@Test func dataCurtaEIdade() {
    #expect(Armazenamento.dataCurta("2026-10-02 08:12") == "02/10 às 08:12")
    // generated_at do sdtally vem sem segundos; a idade da medição (R7) depende disso.
    #expect(data("2026-10-02 09:36") != nil && data("2026-10-02 09:29:42.302128-0300") != nil)
}

/// Recuperação e Atualização moram noutro container interno: a lista de volumes precisa dele para
/// a soma do macOS da tela fechar. O ISC, o do Data e o externo ficam de fora.
@Test func outroContainerInternoFechaAContaDoMacos() throws {
    let json = """
    [{"internal":true,"isc":true,"holds_data":false,"ceiling":524288000,"free":1,"volumes":[{"roles":["xART"],"capacity_in_use":6000000,"in_settings_macos":false}]},
     {"internal":true,"isc":false,"holds_data":false,"ceiling":5368664064,"free":1,"volumes":[{"roles":["Recovery"],"capacity_in_use":2563006464,"in_settings_macos":true},{"roles":["Update"],"capacity_in_use":2928640,"in_settings_macos":true}]},
     {"internal":true,"isc":false,"holds_data":true,"ceiling":494384795648,"free":1,"volumes":[{"roles":["System"],"capacity_in_use":12648747008,"in_settings_macos":true},{"roles":["Data"],"capacity_in_use":421060308992,"in_settings_macos":false}]},
     {"internal":false,"isc":false,"holds_data":false,"ceiling":16000000000000,"free":1,"volumes":[{"roles":["Backup"],"capacity_in_use":754578124800,"in_settings_macos":false}]}]
    """
    let d = JSONDecoder()
    d.keyDecodingStrategy = .convertFromSnakeCase
    let cs = try d.decode([Conteiner].self, from: Data(json.utf8))
    let outros = Armazenamento.outrosInternos(cs)
    #expect(outros.count == 1 && outros[0].volumes.map(\.roles) == [["Recovery"], ["Update"]])
    // Principal + outro: é o macOS da tela inteiro (12.648.747.008 + 2.563.006.464 + 2.928.640).
    let listados = (Armazenamento.principal(cs).map { [$0] } ?? []) + outros
    let soma = listados.flatMap(\.volumes).filter(\.inSettingsMacos).reduce(Int64(0)) { $0 + ($1.capacityInUse ?? 0) }
    #expect(soma == Armazenamento.macosDaTela(cs) && soma == 15_214_682_112)
}

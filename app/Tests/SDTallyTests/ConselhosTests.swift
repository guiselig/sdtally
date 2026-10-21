// O que fazer (A4): textos pt-BR por id, lista fixa de comandos, R6 e ordem. Funções puras.
import Foundation
import Testing
@testable import SDTally

private func completo(_ nome: String) throws -> Completo {
    let url = try #require(Bundle.module.url(forResource: nome, withExtension: "json", subdirectory: "Fixtures"))
    return try decodificar(Completo.self, Data(contentsOf: url), kind: nil)
}

private func conselho(_ id: String, process: String? = nil, command: String? = nil) -> Conselho {
    Conselho(id: id, level: "safe", bytes: 1, privateBytes: nil, paths: [], term: nil, process: process, command: command, text: "texto do sdtally")
}

/// Os ids do §6.
private let idsDoSpec = [
    "grant_full_disk_access", "measure_as_admin", "time_machine_snapshots", "other_snapshots", "purgeable_auto",
    "deleted_open", "swap_large", "apple_intelligence_models", "trash", "logs", "homebrew_cache", "npm_cache",
    "go_build_cache", "pip_cache", "updater_caches", "xcode_derived_data", "simulators", "simulator_runtimes",
    "playwright_browsers", "ollama_models", "cinebench_assets", "cloudkit_cache", "claude_vm_bundles",
    "preferences_anomaly", "icloud_drive", "cloud_storage", "system_folders",
]

@Test func todaRegraTemTextoSeguro() throws {
    #expect(Set(conselhosPT.keys) == Set(idsDoSpec))
    for (id, t) in conselhosPT {
        for proibido in ["rm ", "sudo", "tmutil delete", "deletelocalsnapshots", "-rf"] {
            #expect(!t.texto.contains(proibido) && !t.titulo.contains(proibido), "\(id): \(proibido)")
        }
        // Todo passo com "Lixo pelo Finder" termina dizendo quando o espaço volta.
        if t.texto.contains("Lixo pelo Finder") { #expect(t.texto.hasSuffix("O espaço volta quando o Lixo é esvaziado."), "\(id)") }
    }
    // Todo id da fixture tem tradução.
    for c in try completo("completo").advice ?? [] { #expect(textoConselho(c.id) != nil, "\(c.id)") }
}

@Test func idDesconhecidoMostraText() {
    #expect(textoConselho("regra_nova") == nil)
    #expect(textoMostrado(conselho("regra_nova")) == "texto do sdtally")
    #expect(textoMostrado(conselho("trash")) == textoConselho("trash"))
}

@Test func comandoInjetadoSemBotao() throws {
    let bom = try #require(completo("completo").advice?.first { $0.id == "homebrew_cache" })
    #expect(comandoCopiavel(bom) == "brew cleanup --dry-run")
    let injetado = try #require(completo("completo-injetado").advice?.first { $0.id == "homebrew_cache" })
    #expect(injetado.command == "rm -rf ~/Library/Caches/Homebrew")
    #expect(comandoCopiavel(injetado) == nil)
    #expect(comandoCopiavel(conselho("x", command: "brew cleanup --dry-run; rm -rf ~")) == nil)
    // Mesma lista do Rust (src/conselho.rs, COMANDOS).
    #expect(comandosPermitidos.count == 8)
}

@Test func apagadosAbertosSoNomeiaApp() {
    let c = conselho("deleted_open", process: "Exemplo")
    let exemplo = (executavel: "Exemplo", nome: "Exemplo App")
    #expect(textoMostrado(c, apps: [exemplo]).hasPrefix("Feche e abra de novo o Exemplo App."))
    // Nenhum app, dois que casam, ou só começo com nome curto: reiniciar.
    #expect(textoMostrado(c, apps: []) == "Reiniciar o Mac libera esse espaço.")
    #expect(appParaReabrir("Exemplo", apps: [exemplo, (executavel: "Exemplo", nome: "Outro")]) == nil)
    #expect(appParaReabrir("Exemp", apps: [exemplo]) == nil)
    #expect(appParaReabrir(nil, apps: [exemplo]) == nil)
}

@Test func apagadosAbertosNomeCortado() {
    // p_comm cortado em 15 caracteres casa pelo começo.
    let app = (executavel: "Visual Studio Code Helper", nome: "Code")
    #expect(appParaReabrir("Visual Studio C", apps: [app]) == "Code")
    #expect(appParaReabrir("Visual Studio C", apps: [app, (executavel: "Visual Studio Community", nome: "VS")]) == nil)
}

@Test func ordemPorNivelEPrivado() throws {
    let a = try #require(completo("completo").advice)
    let o = ordenarConselhos(a)
    #expect(o.first?.level == "measurement")
    #expect(o.last?.level == "do_not_touch")
    let custo = o.filter { $0.level == "cost" }.map(\.id)
    #expect(custo == ["simulators", "xcode_derived_data"])
}

@Test func dataDaMedicaoAteOMinuto() {
    #expect(dataMedicao("2026-10-02 09:36") != nil)
    #expect(dataMedicao("2026-10-02 09:36:12") != nil)
    #expect(dataMedicao("ontem") == nil)
}

@Test func maisDeCincoCaminhosRecolheOResto() {
    func linha(_ p: String, _ b: Int64) -> LinhaTop {
        LinhaTop(path: p, term: "tree", bytes: b, privateBytes: nil, lowerBound: false, count: nil, topProcess: nil, source: "getattrlistbulk", folded: nil)
    }
    let paths = (1...7).map { "/System/p\($0)" }
    let top = paths.enumerated().map { linha($1, Int64($0 == 6 ? 900 : 100 - $0)) }
    let cs = caminhosPrincipais(paths, top: top)
    #expect(cs.visiveis == ["/System/p7", "/System/p1", "/System/p2", "/System/p3", "/System/p4"])
    #expect(cs.resto == ["/System/p5", "/System/p6"])
    #expect(caminhosPrincipais(Array(paths.prefix(5)), top: top).resto.isEmpty)
}

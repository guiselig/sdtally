// Barra que se desdobra (A9, spec-fase6 §4.8): os níveis sobre as fixtures sintéticas.
import Foundation
import Testing
@testable import SDTally

private func fixture(_ nome: String) throws -> Data {
    let url = try #require(Bundle.module.url(forResource: nome, withExtension: "json", subdirectory: "Fixtures"))
    return try Data(contentsOf: url)
}

private func soma(_ p: [Segmento]) -> Int64 { p.reduce(0) { $0 + $1.bytes } }

@Test func macosSomaOMacosDoAjustes() throws {
    let r = try decodificar(Rapido.self, fixture("quick"), kind: "quick")
    let s = try #require(segmentos(.macos, rapido: r, completo: nil))
    #expect(s.soma == r.settingsScreen.macos.bytes)
    #expect(soma(s.partes) == r.settingsScreen.macos.bytes)
}

@Test func niveisDoVolumeDizemVolumeData() throws {
    let c = try decodificar(Completo.self, fixture("completo"), kind: nil)
    for n in [NivelBarra.volumeData, .termo("tree"), .termo("snapshots"), .termo("deleted_open"), .termo("protected_folders")] {
        let s = try #require(segmentos(n, rapido: nil, completo: c))
        #expect(s.titulo.contains("volume Data"), "\(s.titulo)")
        #expect(!s.titulo.contains("Dados do Sistema"), "\(s.titulo)")
        for p in s.partes { #expect(!p.nome.contains("Dados do Sistema"), "\(p.nome)") }
    }
}

@Test func volumeDataMaisSemExplicacaoDaOUsado() throws {
    let c = try decodificar(Completo.self, fixture("completo"), kind: nil)
    let s = try #require(segmentos(.volumeData, rapido: nil, completo: c))
    #expect(s.partes.last?.id == "sem" && s.partes.last?.tracejado == true)
    #expect(soma(s.partes) == c.dataVolume.used.bytes && s.soma == c.dataVolume.used.bytes)
    // Termo não medido (large_xattrs null) não vira parte; os medidos desdobram, Bloqueado pelo macOS não.
    #expect(!s.partes.contains { $0.id == "large_xattrs" })
    #expect(s.partes.first { $0.id == "tree" }?.desdobra == .termo("tree") && s.partes.last?.desdobra == nil)
}

@Test func notaMostraOsDoisTotaisEADiferenca() throws {
    let c = try decodificar(Completo.self, fixture("completo"), kind: nil)
    let n = try #require(segmentos(.volumeData, rapido: nil, completo: c)?.nota)
    let t = try #require(c.settingsScreen.screen)
    let x = try #require(c.dataVolume.used.bytes), y = try #require(t.categoriesTotal) + (try #require(t.systemData))
    #expect(n == "O volume Data usa \(gb(x)) (medido às \(hora(c.generatedAt))). As categorias e Dados do Sistema do Ajustes (lido às \(hora(t.loggedAt) ?? "")) somam \(gb(y)). A diferença é \(gb(x - y)); o purgeable medido é \(gb(47_000_000_000)).")
    // Ajustes lido depois de liberar espaço: soma maior que o usado, sem diferença negativa.
    let maior = Tela(state: "measured", reason: nil, loggedAt: t.loggedAt, used: nil, available: nil, capacity: nil,
                     systemData: x, categoriesTotal: 1, categories: nil, service: nil)
    let nm = try #require(segmentos(.volumeData, rapido: nil, completo: c, tela: maior)?.nota)
    #expect(!nm.contains("A diferença") && nm.hasSuffix("O purgeable medido é \(gb(47_000_000_000)).") && !nm.contains("-"))
    // Sem a tela do Ajustes, não inventa o outro total.
    let v = try decodificar(Completo.self, fixture("completo-vazios"), kind: nil)
    #expect(segmentos(.volumeData, rapido: nil, completo: v)?.nota?.contains("Sem a tela do Ajustes") == true)
}

@Test func soComORapidoDaNil() throws {
    let r = try decodificar(Rapido.self, fixture("quick"), kind: "quick")
    #expect(segmentos(.volumeData, rapido: r, completo: nil) == nil)
    #expect(segmentos(.termo("tree"), rapido: r, completo: nil) == nil)
    #expect(segmentos(.ajustes, rapido: r, completo: nil) != nil)
    // Sem tela do Ajustes medida, o nível ajustes cai no V1.
    let v = try decodificar(Completo.self, fixture("completo-vazios"), kind: nil)
    #expect(segmentos(.ajustes, rapido: nil, completo: v) == nil)
}

@Test func ajustesDesdobraSoMacos() throws {
    let r = try decodificar(Rapido.self, fixture("quick"), kind: "quick")
    let s = try #require(segmentos(.ajustes, rapido: r, completo: nil))
    #expect(s.partes.first { $0.id == "macos" }?.desdobra == .macos)
    #expect(s.partes.filter { $0.desdobra != nil }.count == 1)  // Dados do Sistema, Disponível e categorias não desdobram
}

@Test func termoFechaComOTotal() throws {
    let c = try decodificar(Completo.self, fixture("completo"), kind: nil)
    // Snapshots: privados (2+3+4+5+7+9+0,5+3,5 = 34) + presos em dois ou mais (26) = 60 GB do termo.
    let s = try #require(segmentos(.termo("snapshots"), rapido: nil, completo: c))
    #expect(soma(s.partes) == 60_000_000_000 && s.soma == 60_000_000_000)
    #expect(s.partes.last?.nome == "Presos em dois ou mais snapshots" && s.partes.last?.bytes == 26_000_000_000)
    for k in ["tree", "deleted_open"] {
        let t = try #require(segmentos(.termo(k), rapido: nil, completo: c))
        #expect(soma(t.partes) == c.dataVolume.terms.first { $0.key == k }?.bytes)
    }
}

@Test func menoresJuntos() throws {
    let c = try decodificar(Completo.self, fixture("completo"), kind: nil)
    // deleted_open = 1 GB: mds_stores (60 MB) e ExemploEditorDe (40 MB) passam de 0,5 %; nada junta.
    let d = try #require(segmentos(.termo("deleted_open"), rapido: nil, completo: c))
    #expect(!d.partes.contains { $0.id == "menores" })
    // tree = 288 GB: o que fica abaixo de 1,44 GB vai para "menores (N)" quando são dois ou mais.
    let t = try #require(segmentos(.termo("tree"), rapido: nil, completo: c))
    let base = 288_000_000_000.0 * 0.005
    let pequenas = c.top.filter { $0.term == "tree" && Double($0.bytes) < base }
    if pequenas.count >= 2 {
        #expect(t.partes.first { $0.id == "menores" }?.nome == "menores (\(pequenas.count))")
    }
    #expect(t.partes.filter { $0.id != "menores" && $0.id != "resto" }.allSatisfy { pequenas.count < 2 || Double($0.bytes) >= base })
}

@Test func pilhaDeUmNivel() {
    #expect(pilhaInicial(nil).isEmpty && pilhaInicial(.ajustes).isEmpty)
    // O volumeData é a raiz em Dados do Sistema: a pilha só leva o termo.
    #expect(pilhaInicial(.termo("snapshots")) == [.termo("snapshots")] && pilhaInicial(.volumeData).isEmpty)
    #expect(pilhaInicial(Foto.nivelBarra("macos")) == [])  // desdobra no lugar
}

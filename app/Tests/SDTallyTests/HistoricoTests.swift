// A5: histórico em pasta temporária (nunca a real), sem @MainActor. Fixture: Fixtures/dados.
import Foundation
import Testing
@testable import SDTally

private func fixture(_ nome: String) throws -> Data {
    let url = try #require(Bundle.module.url(forResource: nome, withExtension: "json", subdirectory: "Fixtures"))
    return try Data(contentsOf: url)
}

private func dadosFixture() throws -> URL {
    try #require(Bundle.module.url(forResource: "dados", withExtension: nil, subdirectory: "Fixtures"))
}

private func temporaria() throws -> URL {
    let u = FileManager.default.temporaryDirectory.appending(path: "sdtally-teste-\(UUID().uuidString)", directoryHint: .isDirectory)
    try FileManager.default.createDirectory(at: u, withIntermediateDirectories: true)
    return u
}

private func permissoes(_ u: URL) throws -> Int {
    try #require(FileManager.default.attributesOfItem(atPath: u.path)[.posixPermissions] as? Int)
}

/// O completo da fixture com campos trocados.
private func completo(_ muda: (inout [String: Any]) -> Void) throws -> Completo {
    var j = try #require(JSONSerialization.jsonObject(with: fixture("completo")) as? [String: Any])
    muda(&j)
    return try decodificar(Completo.self, JSONSerialization.data(withJSONObject: j), kind: nil)
}

@Test func gravaCom0600E0700EModo() throws {
    let tmp = try temporaria()
    defer { try? FileManager.default.removeItem(at: tmp) }
    let pasta = tmp.appending(path: "SDTally", directoryHint: .isDirectory)
    let d = try fixture("completo")
    let a = try Historico.gravar(d, generatedAt: "2026-10-02 09:36", modo: .helper, em: pasta)
    let b = try Historico.gravar(d, generatedAt: "2026-10-02 09:36", modo: nil, em: pasta)
    #expect(a.lastPathComponent == "2026-10-02-0936.json")
    #expect(b.lastPathComponent == "2026-10-02-0936-2.json")
    #expect(try permissoes(pasta) == 0o700)
    #expect(try permissoes(pasta.appending(path: "Medicoes")) == 0o700)
    #expect(try permissoes(a) == 0o600)
    #expect(try Data(contentsOf: a) == d)  // stdout cru, sem transformar
    #expect(Historico.lerModo(a) == .helper)
    #expect(Historico.lerModo(b) == nil)
    try Historico.gravarPontos([Ponto(t: "2026-10-02 09:00:00", total: 1, usado: 1, livre: 0, purgeable: 0, systemData: nil, telaEm: nil)], em: pasta)
    #expect(try permissoes(pasta.appending(path: "pontos.jsonl")) == 0o600)
}

@Test func retencao() throws {
    let tmp = try temporaria()
    defer { try? FileManager.default.removeItem(at: tmp) }
    let dir = tmp.appending(path: "Medicoes")
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    for i in 0..<103 { try Data("{}".utf8).write(to: dir.appending(path: String(format: "2026-01-01-%04d.json", i))) }
    try Data().write(to: dir.appending(path: "notas.json"))  // nome fora do padrão: nunca é removido
    let alvo = dir.appending(path: "fora.txt")
    try Data().write(to: alvo)
    try FileManager.default.createSymbolicLink(at: dir.appending(path: "2025-01-01-0000.json"), withDestinationURL: alvo)
    let agora = try #require(data("2026-10-02 12:00:00"))
    try Historico.gravarPontos([
        Ponto(t: "2025-06-01 10:00:00", total: nil, usado: nil, livre: nil, purgeable: nil, systemData: nil, telaEm: nil),
        Ponto(t: "2026-10-01 10:00:00", total: nil, usado: nil, livre: nil, purgeable: nil, systemData: nil, telaEm: nil),
    ], em: tmp)
    try Historico.podar(url: tmp, agora: agora)
    let nomes = try FileManager.default.contentsOfDirectory(atPath: dir.path)
    #expect(nomes.count { Historico.nomeValido($0) } == 101)  // 100 + o symlink, que fica
    #expect(!nomes.contains("2026-01-01-0000.json") && !nomes.contains("2026-01-01-0002.json"))
    #expect(nomes.contains("2026-01-01-0102.json") && nomes.contains("notas.json") && nomes.contains("2025-01-01-0000.json"))
    #expect(Historico.lerPontos(tmp).map(\.t) == ["2026-10-01 10:00:00"])
}

@Test func retencaoPorTamanho() throws {
    let tmp = try temporaria()
    defer { try? FileManager.default.removeItem(at: tmp) }
    let dir = tmp.appending(path: "Medicoes")
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    let grande = Data(count: 20_000_000)
    for i in 0..<4 { try grande.write(to: dir.appending(path: "2026-01-0\(i + 1)-1000.json")) }
    try Historico.podar(url: tmp)
    #expect(try FileManager.default.contentsOfDirectory(atPath: dir.path).sorted() == ["2026-01-03-1000.json", "2026-01-04-1000.json"])
}

@Test func pastaGravacaoNilComDevAtivo() {
    #expect(Historico.pastaGravacao(bundleID: "io.github.guiselig.sdtally", dev: true) == nil)
    #expect(Historico.pastaGravacao(bundleID: nil, dev: false) == nil)
}

@Test func sdtallyDadosSoLe() async throws {
    let env = ["SDTALLY_DADOS": "/Users/exemplo/dados"]
    #expect(Historico.pastaLeitura(env: env, bundleID: nil)?.path == "/Users/exemplo/dados")
    #expect(Historico.pastaLeitura(env: env, bundleID: "io.github.guiselig.sdtally")?.path != "/Users/exemplo/dados")
    #expect(Historico.pastaGravacao(bundleID: nil) == nil)
    // Ler uma pasta que não existe não cria nada.
    let nada = FileManager.default.temporaryDirectory.appending(path: "sdtally-nada-\(UUID().uuidString)")
    let (m, p) = await Historico.ler(nada)
    #expect(m.isEmpty && p.isEmpty)
    #expect(!FileManager.default.fileExists(atPath: nada.path))
}

@Test func corrompidoPuladoEOrdemPorGeneratedAt() async throws {
    let tmp = try temporaria()
    defer { try? FileManager.default.removeItem(at: tmp) }
    let dir = tmp.appending(path: "Medicoes")
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    let fx = try dadosFixture().appending(path: "Medicoes")
    // Nomes trocados de propósito: a ordem vem do generated_at.
    try FileManager.default.copyItem(at: fx.appending(path: "2026-10-02-0936.json"), to: dir.appending(path: "2020-01-01-0000.json"))
    try FileManager.default.copyItem(at: fx.appending(path: "2026-09-28-1015.json"), to: dir.appending(path: "2030-01-01-0000.json"))
    try Data("{\"schema_version\":1,".utf8).write(to: dir.appending(path: "2026-10-01-0000.json"))
    var j = try #require(JSONSerialization.jsonObject(with: fixture("completo")) as? [String: Any])
    j["schema_version"] = 2
    try JSONSerialization.data(withJSONObject: j).write(to: dir.appending(path: "2026-10-01-0001.json"))
    let (m, _) = await Historico.ler(tmp)
    #expect(m.map(\.completo.generatedAt) == ["2026-09-28 10:15", "2026-10-02 09:36"])
}

@Test func fixtureGeraOQueMudou() async throws {
    let (m, p) = await Historico.ler(try dadosFixture())
    #expect(m.count == 3)
    #expect(p.count == 168)
    let mud = try #require(Historico.mudancas(de: m[1].completo, para: m[2].completo))
    let porId = Dictionary(uniqueKeysWithValues: mud.map { ($0.id, $0) })
    #expect(porId["termo|tree"]?.depois == 288_000_000_000)
    #expect(porId["~/Library/Developer/Xcode/DerivedData|tree|false"].map { $0.depois! - $0.antes! } == 4_000_000_000)
    #expect(porId["~/Library/Developer/CoreSimulator/Devices|tree|false"] == nil)  // +50 MB, abaixo do limite
    #expect(mud.first?.id == "termo|tree")
    // A mais antiga é de outra versão.
    #expect(Historico.mudancas(de: m[0].completo, para: m[1].completo) == nil)
}

@Test func naoComparaModos() throws {
    let semRoot = try completo { _ in }
    let comRoot = try completo { $0["root"] = true }
    #expect(!Historico.comparavel(semRoot, comRoot))
    #expect(Historico.mudancas(de: semRoot, para: comRoot) == nil)
    #expect(Historico.motivo(semRoot, comRoot) == "Sem comparação: modos diferentes (com e sem senha).")
    let semFDA = try completo { $0["full_disk_access"] = false }
    #expect(Historico.motivo(semRoot, semFDA) == Historico.textoModos)
    let parada = try completo { $0["incomplete_reasons"] = ["interrupted"] }
    #expect(Historico.mudancas(de: semRoot, para: parada) == nil)
}

@Test func naoComparaVersoes() throws {
    let a = try completo { _ in }
    let b = try completo { $0["tool"] = "sdtally 9.9.9" }
    #expect(Historico.mudancas(de: a, para: b) == nil)
    #expect(Historico.motivo(a, b) == "Sem comparação: versões diferentes do sdtally.")
    #expect(Historico.mudancas(de: a, para: a)?.isEmpty == true)
}

@Test func ausenteNaoEZero() throws {
    let a = try completo { _ in }
    let b = try completo { j in
        var top = j["top"] as! [[String: Any]]
        let saiu = top.removeFirst()
        var nova = saiu
        nova["path"] = "/Users/exemplo/nova"
        top.append(nova)
        j["top"] = top
    }
    let primeira = a.top[0]
    let mud = try #require(Historico.mudancas(de: a, para: b))
    let saiu = try #require(mud.first { $0.id == primeira.id })
    #expect(saiu.antes == primeira.bytes && saiu.depois == nil)
    let nova = try #require(mud.first { $0.linha?.path == "/Users/exemplo/nova" })
    #expect(nova.antes == nil && nova.depois == primeira.bytes)
}

@Test func pontoUmPorHoraOuTelaNova() throws {
    let r = try decodificar(Rapido.self, fixture("quick"), kind: "quick")
    let p = try #require(Historico.proximoPonto(r, depois: []))
    let c = try #require(r.containers?.first { $0.holdsData && $0.internal == true })
    #expect(p.usado == c.ceiling! - c.free!)
    #expect(p.systemData == r.settingsScreen.screen?.systemData && p.telaEm != nil)
    // Mesma tela, menos de 1 h depois: nada.
    #expect(Historico.proximoPonto(r, depois: [p]) == nil)
    let antigo = Ponto(t: "2026-10-02 08:00:00", total: nil, usado: nil, livre: nil, purgeable: nil, systemData: p.systemData, telaEm: p.telaEm)
    let novo = try #require(Historico.proximoPonto(r, depois: [antigo]))
    #expect(novo.systemData == nil && novo.telaEm == nil)
    #expect(Historico.leituraAnterior([antigo], telaEm: "2026-10-02 10:00:00.1-0300") == antigo)
    #expect(Historico.leituraAnterior([antigo], telaEm: p.telaEm) == nil)
}

/// Pontos novos com fuso; os velhos, sem fuso, continuam lendo. Na hora repetida do fim do
/// horário de verão o fuso desfaz a volta no tempo.
@Test func carimboComFusoLeOsDoisFormatos() throws {
    let velho = try #require(data("2026-10-02 09:36:00"))
    let novo = carimboComFuso("2026-10-02 09:36:00", agora: velho)
    #expect(novo.count == 25 && novo.dropFirst(10).first == "T" && data(novo) == velho)
    #expect(carimboComFuso("2026-10-02 09:36", agora: velho) == novo)
    #expect(carimboComFuso(novo) == novo && carimboComFuso("x") == "x")
    let antes = try #require(data("2026-11-01T01:50:00-04:00")), depois = try #require(data("2026-11-01T01:10:00-05:00"))
    #expect(depois.timeIntervalSince(antes) == 20 * 60)
}

/// Pastas do Histórico: primeira e última amostra de cada uma; precisa de duas; ordem pelo tamanho da mudança.
@MainActor @Test func variacoesDePastas() {
    let gb: Int64 = 1_000_000_000
    let a = [
        PontoPastas(t: "2026-10-20 10:00:00", pastas: ["/a": 1 * gb, "/b": 5 * gb, "/so-uma": 9 * gb]),
        PontoPastas(t: "2026-10-20 11:00:00", pastas: ["/b": 2 * gb]),
        PontoPastas(t: "2026-10-20 12:00:00", pastas: ["/a": 4 * gb, "/b": 2 * gb + 1]),
    ]
    #expect(variacoesPastas(a) == [VariacaoPasta(id: "/a", antes: 1 * gb, depois: 4 * gb), VariacaoPasta(id: "/b", antes: 5 * gb, depois: 2 * gb + 1)])
    #expect(variacoesPastas(Array(a.prefix(1))).isEmpty)
    #expect(PeriodoHistorico.semana.inicio(data("2026-10-21 12:00:00")!) == data("2026-10-14 12:00:00"))
}

@Test func zoomNoTempo() {
    let a = Date(timeIntervalSince1970: 0), b = a.addingTimeInterval(7 * 86_400)
    // Rolar para cima aproxima em volta do ponteiro: o ponto sob ele fica parado.
    let z = zoomTempo(a...b, limite: a...b, fracao: 0.5, delta: 50)
    #expect(z.upperBound.timeIntervalSince(z.lowerBound) < 7 * 86_400)
    #expect(abs(z.lowerBound.timeIntervalSince(a) + z.upperBound.timeIntervalSince(a) - 7 * 86_400) < 1)
    // Afastar não passa do período, nem aproximar fica abaixo de 10 min.
    #expect(zoomTempo(z, limite: a...b, fracao: 0.9, delta: -10_000) == a...b)
    let perto = zoomTempo(a...b, limite: a...b, fracao: 0, delta: 10_000)
    #expect(perto == a...a.addingTimeInterval(600))
}

@Test func eixoSegueOTrecho() {
    #expect(eixoTempo(3600).0 == .minute && eixoTempo(3600).1 == 15)
    #expect(eixoTempo(86_400).0 == .hour)
    #expect(eixoTempo(7 * 86_400).0 == .day && eixoTempo(7 * 86_400).1 == 1)
    #expect(eixoTempo(365 * 86_400).0 == .month)
}

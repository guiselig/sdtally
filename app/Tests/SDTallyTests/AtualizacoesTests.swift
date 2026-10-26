// Atualizacoes: leitura do release, hosts, SHA-256 e quando procurar (sem rede).
import Foundation
import Testing
@testable import SDTally

private func release() throws -> [String: Any] {
    let url = try #require(Bundle.module.url(forResource: "release", withExtension: "json", subdirectory: "Fixtures"))
    return try #require(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
}

private func ler(_ j: [String: Any]) throws -> Lancamento {
    try Lancamento.ler(JSONSerialization.data(withJSONObject: j))
}

/// Muda o asset do DMG (o segundo) do fixture.
private func comDMG(_ muda: (inout [String: Any]) -> Void) throws -> [String: Any] {
    var j = try release()
    var a = try #require(j["assets"] as? [[String: Any]])
    muda(&a[1])
    j["assets"] = a
    return j
}

@Test func leRelease() throws {
    let l = try ler(release())
    #expect(l.versao == "1.0.5")
    #expect(l.dmg.absoluteString == "https://github.com/guiselig/sdtally/releases/download/v1.0.5/SDTally-1.0.5.dmg")
    #expect(l.sha256 == "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
    #expect(l.pagina.absoluteString == "https://github.com/guiselig/sdtally/releases/tag/v1.0.5")
    // Ida e volta pelas preferências (a novidade guardada).
    #expect(Lancamento(info: l.info) == l)
    #expect(Lancamento(info: l.info.merging(["dmg": "https://exemplo.invalid/x.dmg"]) { $1 }) == nil)
}

@Test func recusaRelease() throws {
    func recusa(_ j: [String: Any]) throws -> Lancamento.Recusa? {
        do { _ = try ler(j); return nil } catch let e as Lancamento.Recusa { return e }
    }
    var j = try release()
    j["immutable"] = false
    #expect(try recusa(j) == .naoImutavel)
    j.removeValue(forKey: "immutable")
    #expect(try recusa(j) == .naoImutavel)
    #expect(try recusa(comDMG { $0.removeValue(forKey: "digest") }) == .semDigest)
    #expect(try recusa(comDMG { $0["digest"] = "sha512:" + String(repeating: "a", count: 128) }) == .semDigest)
    #expect(try recusa(comDMG { $0["digest"] = "sha256:abc" }) == .semDigest)
    #expect(try recusa(comDMG { $0["name"] = "SDTally-1.0.4.dmg" }) == .semDMG)
    #expect(try recusa(comDMG { $0["browser_download_url"] = "http://github.com/guiselig/sdtally/releases/download/v1.0.5/SDTally-1.0.5.dmg" }) == .urlFora)
    #expect(try recusa(comDMG { $0["browser_download_url"] = "https://exemplo.invalid/SDTally-1.0.5.dmg" }) == .urlFora)
    #expect(try recusa(comDMG { $0["browser_download_url"] = "https://github.com/outro/repo/releases/download/v1.0.5/SDTally-1.0.5.dmg" }) == .urlFora)
    j = try release()
    j["tag_name"] = "ultima"
    #expect(try recusa(j) == .versao)
    #expect(throws: Lancamento.Recusa.json) { try Lancamento.ler(Data("{".utf8)) }
}

@Test func hostsPermitidos() {
    #expect(Lancamento.permitido(URL(string: "https://release-assets.githubusercontent.com/x")))
    #expect(Lancamento.permitido(URL(string: "https://objects.githubusercontent.com/x")))
    #expect(!Lancamento.permitido(URL(string: "http://github.com/x")))
    #expect(!Lancamento.permitido(URL(string: "https://github.com.exemplo.invalid/x")))
    #expect(!Lancamento.permitido(URL(string: "https://raw.githubusercontent.com/x")))
    #expect(!Lancamento.permitido(nil))
}

@Test func confereSHA256() {
    #expect(Lancamento.sha256(Data("test".utf8)) == "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
    #expect(Lancamento.sha256(Data("test\n".utf8)) != "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
}

@Test func procuraACada24Horas() {
    let agora = Date(timeIntervalSince1970: 1_800_000_000)
    #expect(Atualizacoes.vence(agora: agora, ultima: nil))
    #expect(!Atualizacoes.vence(agora: agora, ultima: agora))
    #expect(!Atualizacoes.vence(agora: agora, ultima: agora - (24 * 3600 - 1)))
    #expect(Atualizacoes.vence(agora: agora, ultima: agora - 24 * 3600))
    // O Mac dormiu três dias: a primeira conferência depois de acordar já procura.
    #expect(Atualizacoes.vence(agora: agora, ultima: agora - 3 * 24 * 3600))
    // Relógio voltou: a última ficou no futuro.
    #expect(Atualizacoes.vence(agora: agora, ultima: agora + 60))
}

@Test func soProcuraLigadoForaDeDevEFoto() {
    #expect(Atualizacoes.pode(ligado: true, dev: false, foto: false))
    #expect(!Atualizacoes.pode(ligado: false, dev: false, foto: false))
    #expect(!Atualizacoes.pode(ligado: true, dev: true, foto: false))
    #expect(!Atualizacoes.pode(ligado: true, dev: false, foto: true))
    #expect(!Atualizacoes.pode(ligado: false, dev: true, foto: true))
}

/// O swift test roda sem bundle id (Dev.ativo): nem procurarAgora sai para a rede.
@Test @MainActor func emDevNaoProcura() {
    #expect(Dev.ativo)
    let a = Atualizacoes.unica
    a.procurarAgora()
    a.instalar()
    #expect(a.estado == .parado)
}

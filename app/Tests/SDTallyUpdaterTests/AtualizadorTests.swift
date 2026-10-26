// Atualizador: versão, leitura do release, SHA-256, assinatura e a troca do bundle (sem rede).
import Foundation
import Testing
@testable import SDTallyUpdater

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

@Test func versaoCompara() {
    #expect(Versao.maisNova("1.0.5", que: "1.0.4"))
    #expect(Versao.maisNova("v1.10.0", que: "1.9.9"))
    #expect(Versao.maisNova("2", que: "1.99.99"))
    #expect(!Versao.maisNova("1.0.4", que: "1.0.4"))
    #expect(!Versao.maisNova("1.0.4", que: "1.0.5"))
    #expect(!Versao.maisNova("1.0", que: "1.0.0"))
    #expect(Versao.maisNova("1.0.0", que: "1.0.0-beta"))
    #expect(!Versao.maisNova("1.0.0-beta", que: "1.0.0"))
    #expect(Versao.maisNova("1.0.0-rc", que: "1.0.0-beta"))
    // Ilegível nunca é mais nova, dos dois lados.
    for x in ["", "v", "1..2", "1.2.3.4", "+1.0", "1.0-", "a.b", "１.0"] {
        #expect(!Versao.maisNova(x, que: "0.0.1"), "\(x)")
        #expect(!Versao.maisNova("9.9.9", que: x), "\(x)")
    }
}

@Test func leRelease() throws {
    let l = try ler(release())
    #expect(l.versao == "1.0.5")
    #expect(l.dmg.absoluteString == "https://github.com/guiselig/sdtally/releases/download/v1.0.5/SDTally-1.0.5.dmg")
    #expect(l.sha256 == "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
    #expect(l.pagina.absoluteString == "https://github.com/guiselig/sdtally/releases/tag/v1.0.5")
    // Ida e volta pelo userInfo da notificação.
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

// MARK: Bundles de mentira

private func pasta() throws -> URL {
    let u = FileManager.default.temporaryDirectory.appending(path: "sdtally-updater-tests-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: u, withIntermediateDirectories: true)
    return u
}

/// Um .app mínimo: Info.plist e um executável de shell que só marca a versão.
@discardableResult
private func app(_ u: URL, id: String = "io.github.guiselig.sdtally", versao: String) throws -> URL {
    let fm = FileManager.default
    try fm.createDirectory(at: u.appending(path: "Contents/MacOS"), withIntermediateDirectories: true)
    let info: [String: Any] = ["CFBundleIdentifier": id, "CFBundleExecutable": "SDTally", "CFBundlePackageType": "APPL",
                               "CFBundleShortVersionString": versao]
    try PropertyListSerialization.data(fromPropertyList: info, format: .xml, options: 0).write(to: u.appending(path: "Contents/Info.plist"))
    let exe = u.appending(path: "Contents/MacOS/SDTally")
    try Data("#!/bin/sh\necho \(versao)\n".utf8).write(to: exe)
    try fm.setAttributes([.posixPermissions: 0o755], ofItemAtPath: exe.path)
    return u
}

private func codesignAdHoc(_ u: URL) throws {
    let p = Process()
    p.executableURL = URL(filePath: "/usr/bin/codesign")
    p.arguments = ["--force", "--sign", "-", u.path]
    p.standardError = FileHandle.nullDevice
    try p.run()
    p.waitUntilExit()
    #expect(p.terminationStatus == 0)
}

@Test func assinaturaRecusaSemTimeDoSDTally() throws {
    let raiz = try pasta()
    defer { Instalacao.apagarTemporario(raiz) }
    let u = try app(raiz.appending(path: "SDTally.app"), versao: "9.9.9")
    #expect(!Instalacao.assinaturaValida(u))  // sem assinatura
    try codesignAdHoc(u)
    #expect(!Instalacao.assinaturaValida(u))  // ad hoc: sem o time
    // A conferência em si funciona: o mesmo bundle passa num requisito que ad hoc cumpre.
    #expect(Instalacao.assinaturaValida(u, requisito: #"identifier "io.github.guiselig.sdtally""#))
    // Mexer no bundle depois de assinar quebra a validação estrita.
    try Data("#!/bin/sh\necho outro\n".utf8).write(to: u.appending(path: "Contents/MacOS/SDTally"))
    #expect(!Instalacao.assinaturaValida(u, requisito: #"identifier "io.github.guiselig.sdtally""#))
}

@Test func appDonoEVersao() throws {
    let raiz = try pasta()
    defer { Instalacao.apagarTemporario(raiz) }
    let dono = try app(raiz.appending(path: "Qualquer Pasta/SDTally.app"), versao: "1.0.4")
    let atualizador = try app(dono.appending(path: "Contents/Library/LoginItems/SDTallyUpdater.app"), id: "io.github.guiselig.sdtally.updater", versao: "1.0.4")
    #expect(Instalacao.appDono(atualizador)?.standardizedFileURL == dono.standardizedFileURL)
    #expect(Instalacao.versao(dono) == "1.0.4")
    // Fora de um SDTally.app (outro identifier no lugar do dono): nil.
    let solto = try app(raiz.appending(path: "Outro.app"), id: "exemplo.outro", versao: "1")
    #expect(Instalacao.appDono(solto.appending(path: "Contents/Library/LoginItems/SDTallyUpdater.app")) == nil)
}

@Test func trocaOBundle() throws {
    let raiz = try pasta()
    defer { Instalacao.apagarTemporario(raiz) }
    let destino = try app(raiz.appending(path: "Aplicativos/SDTally.app"), versao: "1.0.4")
    try Data("do usuário".utf8).write(to: raiz.appending(path: "Aplicativos/vizinho.txt"))
    let novo = try app(raiz.appending(path: "Volume/SDTally.app"), versao: "1.0.5")
    #expect(Instalacao.podeEscrever(destino))
    try Instalacao.trocar(destino, por: novo)
    #expect(Instalacao.versao(destino) == "1.0.5")
    #expect(try String(contentsOf: destino.appending(path: "Contents/MacOS/SDTally"), encoding: .utf8).contains("1.0.5"))
    // O original (o DMG) fica, o vizinho fica, e a pasta da troca não sobra ao lado.
    #expect(Instalacao.versao(novo) == "1.0.5")
    let ao = try FileManager.default.contentsOfDirectory(atPath: raiz.appending(path: "Aplicativos").path).sorted()
    #expect(ao == ["SDTally.app", "vizinho.txt"])
}

@Test func trocaQueFalhaNaoMexe() throws {
    let raiz = try pasta()
    defer { Instalacao.apagarTemporario(raiz) }
    let destino = try app(raiz.appending(path: "Aplicativos/SDTally.app"), versao: "1.0.4")
    #expect(throws: (any Error).self) { try Instalacao.trocar(destino, por: raiz.appending(path: "nao-existe.app")) }
    #expect(Instalacao.versao(destino) == "1.0.4")
}

@Test func semEscritaNaPasta() throws {
    let raiz = try pasta()
    defer {
        try? FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: raiz.appending(path: "Aplicativos").path)
        Instalacao.apagarTemporario(raiz)
    }
    let destino = try app(raiz.appending(path: "Aplicativos/SDTally.app"), versao: "1.0.4")
    try FileManager.default.setAttributes([.posixPermissions: 0o555], ofItemAtPath: raiz.appending(path: "Aplicativos").path)
    #expect(!Instalacao.podeEscrever(destino))
}

@Test func apagaSoTemporario() throws {
    let raiz = try pasta()
    defer { Instalacao.apagarTemporario(raiz) }
    // Fora do temporário, não apaga (nem a própria pasta temporária inteira).
    Instalacao.apagarTemporario(FileManager.default.temporaryDirectory)
    #expect(FileManager.default.fileExists(atPath: raiz.path))
    Instalacao.apagarTemporario(URL(filePath: "/Applications/SDTally.app"))
    let sub = raiz.appending(path: "x")
    try FileManager.default.createDirectory(at: sub, withIntermediateDirectories: true)
    Instalacao.apagarTemporario(sub)
    #expect(!FileManager.default.fileExists(atPath: sub.path))
}

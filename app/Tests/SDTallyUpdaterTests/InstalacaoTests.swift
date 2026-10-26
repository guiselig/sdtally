// Instalacao: versão, assinatura, a troca do bundle, os argumentos do executor e o trabalho dele,
// em bundles de mentira (sem rede, sem abrir o SDTally).
import Foundation
import Testing
@testable import SDTallyInstalacao

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

@Test func versaoEIdentificador() throws {
    let raiz = try pasta()
    defer { Instalacao.apagarTemporario(raiz) }
    let u = try app(raiz.appending(path: "Qualquer Pasta/SDTally.app"), versao: "1.0.4")
    #expect(Instalacao.versao(u) == "1.0.4")
    #expect(Instalacao.identificador(u) == Instalacao.idApp)
    #expect(Instalacao.versao(raiz.appending(path: "nao-existe.app")) == nil)
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

// MARK: Executor

private let pastaMontada = "/Volumes/SDTally 1.0.5"
private func argumentos(destino: String = "/Applications/SDTally.app", novo: String = pastaMontada + "/SDTally.app",
                        pid: String = "4321", montagem: String = pastaMontada) -> [String] {
    [destino, novo, pid, montagem]
}

private func recusa(_ a: [String]) -> Pedido.Recusa? {
    do { _ = try Pedido.ler(a, temporario: "/private/var/folders/xy/abc/T"); return nil } catch { return error }
}

@Test func pedidoAceitaOEsperado() throws {
    let p = try Pedido.ler(argumentos(), temporario: "/private/var/folders/xy/abc/T")
    #expect(p.destino.path == "/Applications/SDTally.app")
    #expect(p.novo.path == pastaMontada + "/SDTally.app")
    #expect(p.pid == 4321)
    #expect(p.montagem.path == pastaMontada)
    // Ida e volta: o que o app monta é o que o executor lê.
    #expect(try Pedido.ler(p.argumentos, temporario: "/private/var/folders/xy/abc/T") == p)
    #expect(try Pedido.ler(argumentos(destino: "/Users/x/Applications/SDTally 2.app")).destino.lastPathComponent == "SDTally 2.app")
}

@Test func pedidoRecusaForaDoLugar() {
    #expect(recusa([]) == .quantidade)
    #expect(recusa(argumentos() + ["extra"]) == .quantidade)
    // pid: só dígitos ASCII, maior que 1.
    for p in ["", "abc", "12a", "-5", "+5", " 5", "0", "1", "99999999999", "４", "5.0"] {
        #expect(recusa(argumentos(pid: p)) == .pid, "\(p)")
    }
    // Montagem: só uma pasta logo abaixo de /Volumes.
    for m in ["/tmp/x", "/Volumes", "/Volumes/", "/Volumes/a/b", "/Volumes/../etc", "Volumes/x", "/Volumes//x"] {
        #expect(recusa(argumentos(novo: m + "/SDTally.app", montagem: m)) == .montagem, "\(m)")
    }
    // O app novo: exatamente o SDTally.app da montagem.
    for n in ["/Volumes/Outro/SDTally.app", pastaMontada + "/Outro.app", pastaMontada + "/x/SDTally.app", "/Applications/SDTally.app"] {
        #expect(recusa(argumentos(novo: n)) == .novo, "\(n)")
    }
    // Destino: absoluto, limpo, um .app, fora do DMG, do /System e da pasta temporária.
    for d in ["Applications/SDTally.app", "/Applications/SDTally", "/Applications/../tmp/SDTally.app", "/Applications/./SDTally.app",
              "/Applications//SDTally.app", "/Applications/SDTally.app/", pastaMontada + "/SDTally.app", "/System/Applications/SDTally.app",
              "/private/var/folders/xy/abc/T/SDTally-atualizacao-1/SDTally.app", "/var/folders/xy/abc/T/x/SDTally.app"] {
        #expect(recusa(argumentos(destino: d)) == .destino, "\(d)")
    }
}

/// Um pid que já saiu (o Process espera e recolhe o processo).
private func pidQueSaiu() throws -> Int32 {
    let p = Process()
    p.executableURL = URL(filePath: "/usr/bin/true")
    try p.run()
    p.waitUntilExit()
    return p.processIdentifier
}

private let requisitoAdHoc = #"identifier "io.github.guiselig.sdtally""#

@Test func executorTroca() throws {
    let raiz = try pasta()
    defer { Instalacao.apagarTemporario(raiz) }
    let destino = try app(raiz.appending(path: "Aplicativos/SDTally.app"), versao: "1.0.4")
    let novo = try app(raiz.appending(path: "Volume/SDTally.app"), versao: "1.0.5")
    try codesignAdHoc(novo)
    #expect(Instalacao.executar(destino: destino, novo: novo, pid: try pidQueSaiu(), requisito: requisitoAdHoc, limite: 5) == nil)
    #expect(Instalacao.versao(destino) == "1.0.5")
    #expect(Instalacao.assinaturaValida(destino, requisito: requisitoAdHoc))
}

@Test func executorQueFalhaNaoMexe() throws {
    let raiz = try pasta()
    defer { Instalacao.apagarTemporario(raiz) }
    let destino = try app(raiz.appending(path: "Aplicativos/SDTally.app"), versao: "1.0.4")
    let novo = try app(raiz.appending(path: "Volume/SDTally.app"), versao: "1.0.5")
    let saiu = try pidQueSaiu()
    // Sem assinatura do time (o requisito de verdade): recusa antes de trocar.
    try codesignAdHoc(novo)
    #expect(Instalacao.executar(destino: destino, novo: novo, pid: saiu, limite: 5) == .assinatura)
    #expect(Instalacao.versao(destino) == "1.0.4")
    // Mexeram no app novo depois de assinar: a conferência de antes da troca vê.
    try Data("#!/bin/sh\necho outro\n".utf8).write(to: novo.appending(path: "Contents/MacOS/SDTally"))
    #expect(Instalacao.executar(destino: destino, novo: novo, pid: saiu, requisito: requisitoAdHoc, limite: 5) == .assinatura)
    #expect(Instalacao.versao(destino) == "1.0.4")
    // Versão igual ou mais velha.
    let velho = try app(raiz.appending(path: "Volume2/SDTally.app"), versao: "1.0.4")
    try codesignAdHoc(velho)
    #expect(Instalacao.executar(destino: destino, novo: velho, pid: saiu, requisito: requisitoAdHoc, limite: 5) == .versao)
    // O destino não é o SDTally.
    let outro = try app(raiz.appending(path: "Aplicativos/Outro.app"), id: "exemplo.outro", versao: "1.0.0")
    let bom = try app(raiz.appending(path: "Volume3/SDTally.app"), versao: "1.0.5")
    try codesignAdHoc(bom)
    #expect(Instalacao.executar(destino: outro, novo: bom, pid: saiu, requisito: requisitoAdHoc, limite: 5) == .destino)
    #expect(Instalacao.identificador(outro) == "exemplo.outro")
    // O SDTally não fechou (este processo, que segue vivo): não toca em nada.
    #expect(Instalacao.executar(destino: destino, novo: bom, pid: getpid(), requisito: requisitoAdHoc, limite: 0.3) == .aberto)
    #expect(Instalacao.versao(destino) == "1.0.4")
    // A troca em si falha (destino sumiu no meio): erro, sem trocar.
    let sumiu = raiz.appending(path: "Aplicativos/SDTally.app")
    try FileManager.default.setAttributes([.posixPermissions: 0o555], ofItemAtPath: raiz.appending(path: "Aplicativos").path)
    defer { try? FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: raiz.appending(path: "Aplicativos").path) }
    #expect(Instalacao.executar(destino: sumiu, novo: bom, pid: saiu, requisito: requisitoAdHoc, limite: 5) == .troca)
    #expect(Instalacao.versao(sumiu) == "1.0.4")
}

@Test func resultadoIdaEVolta() {
    for r in [Resultado(versao: "1.0.5", erro: nil), Resultado(versao: "1.0.5", erro: .assinatura), Resultado(versao: "", erro: .argumentos)] {
        #expect(Resultado(r.dicionario) == r)
    }
    #expect(Resultado(nil) == nil)
    #expect(Resultado(["erro": "troca"]) == nil)
    // Erro desconhecido (executor de outra versão) ainda é erro.
    #expect(Resultado(["versao": "1", "erro": "novo-motivo"])?.erro == .troca)
}


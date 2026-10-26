// SDTallyInstalacao: o que o app e o executor (SDTallyUpdater) dividem para instalar uma versão
// nova, sem rede (tests/sem_rede.rs): comparar versões, conferir a assinatura do app novo,
// montar o DMG, trocar o bundle, ler os argumentos do executor e o resultado que ele deixa para o
// app. A parte com rede (o GitHub) fica só em app/Sources/SDTally/Atualizacao.
import Darwin
import Foundation
import Security

public enum Versao {
    /// "1.2.3", "v1.2" ou "1.2.3-beta.1": os três números (faltando, 0) e o sufixo de pré-lançamento.
    public static func partes(_ s: String) -> (numeros: [Int], pre: String?)? {
        let t = s.hasPrefix("v") ? s.dropFirst() : Substring(s)
        let nucleo = t.split(separator: "-", maxSplits: 1, omittingEmptySubsequences: false)
        let n = nucleo[0].split(separator: ".", omittingEmptySubsequences: false)
        guard (1...3).contains(n.count), n.allSatisfy({ !$0.isEmpty && $0.allSatisfy { $0.isASCII && $0.isNumber } }),
              nucleo.count == 1 || !nucleo[1].isEmpty else { return nil }
        let numeros = n.compactMap { Int($0) }
        guard numeros.count == n.count else { return nil }
        return (numeros + Array(repeating: 0, count: 3 - numeros.count), nucleo.count == 2 ? String(nucleo[1]) : nil)
    }

    /// `a` é mais nova que `b`. Versão que não se lê nunca é mais nova.
    public static func maisNova(_ a: String, que b: String) -> Bool {
        guard let x = partes(a), let y = partes(b) else { return false }
        if x.numeros != y.numeros { return y.numeros.lexicographicallyPrecedes(x.numeros) }
        switch (x.pre, y.pre) {
        case (nil, nil), (_?, nil): return false
        case (nil, _?): return true
        // shortcut: pré-lançamentos comparados como texto ("beta.10" < "beta.9"); o publicar.sh não publica pré-lançamento.
        case let (p?, q?): return q < p
        }
    }
}

public enum Instalacao {
    public static let idApp = "io.github.guiselig.sdtally"
    /// Designated requirement que o SDTally.app novo tem de cumprir: o identifier e o time do montar.sh.
    public static let requisito = #"identifier "io.github.guiselig.sdtally" and anchor apple generic and certificate leaf[subject.OU] = "5DFAL23UXC""#
    /// Começo do nome da pasta temporária de cada instalação (o DMG e a cópia do executor).
    public static let prefixoTemporario = "SDTally-atualizacao-"

    public static func info(_ app: URL) -> [String: Any]? {
        guard let d = try? Data(contentsOf: app.appending(path: "Contents/Info.plist")) else { return nil }
        return try? PropertyListSerialization.propertyList(from: d, format: nil) as? [String: Any]
    }

    public static func versao(_ app: URL) -> String? { info(app)?["CFBundleShortVersionString"] as? String }

    public static func identificador(_ app: URL) -> String? { info(app)?["CFBundleIdentifier"] as? String }

    /// Validação estrita, todas as arquiteturas e o código aninhado, contra `requisito`.
    public static func assinaturaValida(_ app: URL, requisito: String = requisito) -> Bool {
        var codigo: SecStaticCode?
        var req: SecRequirement?
        guard SecStaticCodeCreateWithPath(app as CFURL, [], &codigo) == errSecSuccess, let codigo,
              SecRequirementCreateWithString(requisito as CFString, [], &req) == errSecSuccess, let req else { return false }
        let f = SecCSFlags(rawValue: kSecCSStrictValidate | kSecCSCheckAllArchitectures | kSecCSCheckNestedCode)
        return SecStaticCodeCheckValidity(codigo, f, req) == errSecSuccess
    }

    /// A troca é um rename na pasta de `app`: sem escrita nela, não dá.
    public static func podeEscrever(_ app: URL) -> Bool {
        FileManager.default.isWritableFile(atPath: app.deletingLastPathComponent().path)
    }

    /// Copia `novo` para uma pasta temporária no mesmo volume de `destino` e troca os dois de uma vez
    /// (replaceItemAt). O bundle antigo do SDTally sai; qualquer erro antes da troca deixa `destino` como estava.
    public static func trocar(_ destino: URL, por novo: URL) throws {
        let fm = FileManager.default
        let pasta = try fm.url(for: .itemReplacementDirectory, in: .userDomainMask, appropriateFor: destino, create: true)
        defer { apagarTemporario(pasta) }
        let copia = pasta.appending(path: destino.lastPathComponent)
        try fm.copyItem(at: novo, to: copia)
        _ = try fm.replaceItemAt(destino, withItemAt: copia)
    }

    /// Só as pastas temporárias da instalação: a do download e do executor, e a da troca. Nunca a
    /// própria pasta temporária. Compara sem links (/var é /private/var).
    public static func apagarTemporario(_ u: URL) {
        let p = u.standardizedFileURL.resolvingSymlinksInPath().path
        let tmp = FileManager.default.temporaryDirectory.standardizedFileURL.resolvingSymlinksInPath().path
        guard p.hasPrefix(tmp.hasSuffix("/") ? tmp : tmp + "/") || p.contains("/TemporaryItems/") else { return }
        try? FileManager.default.removeItem(at: u)
    }

    // MARK: hdiutil

    private static func hdiutil(_ args: [String]) -> Data? {
        let p = Process()
        p.executableURL = URL(filePath: "/usr/bin/hdiutil")
        p.arguments = args
        let saida = Pipe()
        p.standardOutput = saida
        p.standardError = FileHandle.nullDevice
        guard (try? p.run()) != nil else { return nil }
        let d = saida.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        return p.terminationStatus == 0 ? d : nil
    }

    /// Monta só leitura, sem Finder e sem abrir nada; devolve o ponto de montagem.
    public static func anexar(_ dmg: URL) -> URL? {
        guard let d = hdiutil(["attach", "-nobrowse", "-readonly", "-noautoopen", "-plist", dmg.path]),
              let p = try? PropertyListSerialization.propertyList(from: d, format: nil) as? [String: Any],
              let ents = p["system-entities"] as? [[String: Any]],
              let m = ents.compactMap({ $0["mount-point"] as? String }).first else { return nil }
        return URL(filePath: m, directoryHint: .isDirectory)
    }

    public static func desanexar(_ m: URL) {
        if hdiutil(["detach", m.path, "-quiet"]) == nil { _ = hdiutil(["detach", m.path, "-quiet", "-force"]) }
    }

    // MARK: Executor

    /// Abre `exe` numa sessão própria (setsid) e sem herdar descritores: ele segue vivo depois que
    /// quem o abriu fecha. Não espera.
    public static func lancarSolto(_ exe: URL, _ args: [String]) -> Bool {
        var attr: posix_spawnattr_t?
        posix_spawnattr_init(&attr)
        defer { posix_spawnattr_destroy(&attr) }
        posix_spawnattr_setflags(&attr, Int16(POSIX_SPAWN_SETSID | POSIX_SPAWN_CLOEXEC_DEFAULT))
        var acoes: posix_spawn_file_actions_t?
        posix_spawn_file_actions_init(&acoes)
        defer { posix_spawn_file_actions_destroy(&acoes) }
        for fd: Int32 in 0...2 { posix_spawn_file_actions_addopen(&acoes, fd, "/dev/null", fd == 0 ? O_RDONLY : O_WRONLY, 0) }
        let argv: [UnsafeMutablePointer<CChar>?] = ([exe.path] + args).map { strdup($0) } + [nil]
        defer { argv.forEach { free($0) } }
        var pid: pid_t = 0
        return posix_spawn(&pid, exe.path, &acoes, &attr, argv, environ) == 0
    }

    /// Espera o processo `pid` sair, até `limite` segundos.
    public static func esperarSair(_ pid: Int32, limite: TimeInterval) -> Bool {
        let fim = Date.now.addingTimeInterval(limite)
        repeat {
            if kill(pid, 0) != 0 && errno == ESRCH { return true }
            usleep(100_000)
        } while Date.now < fim
        return false
    }

    /// O trabalho do executor, depois de o SDTally (`pid`) fechar: confere de novo o destino, a
    /// assinatura e a versão do app novo (o DMG é só leitura, mas confere mesmo assim) e troca.
    /// nil: trocou. Qualquer erro deixa `destino` como estava.
    public static func executar(destino: URL, novo: URL, pid: Int32, requisito: String = requisito, limite: TimeInterval = 60) -> Resultado.Erro? {
        guard esperarSair(pid, limite: limite) else { return .aberto }
        guard identificador(destino) == idApp else { return .destino }
        guard assinaturaValida(novo, requisito: requisito) else { return .assinatura }
        guard let nova = versao(novo), let atual = versao(destino), Versao.maisNova(nova, que: atual) else { return .versao }
        do { try trocar(destino, por: novo) } catch { return .troca }
        return nil
    }
}

/// Os argumentos do executor, na ordem: o SDTally.app a trocar, o app novo dentro do DMG montado,
/// o pid do SDTally e o ponto de montagem. `ler` recusa o que sair do lugar esperado.
public struct Pedido: Equatable, Sendable {
    public let destino: URL
    public let novo: URL
    public let pid: Int32
    public let montagem: URL

    public enum Recusa: Error, Equatable { case quantidade, destino, montagem, novo, pid }

    public init(destino: URL, novo: URL, pid: Int32, montagem: URL) {
        (self.destino, self.novo, self.pid, self.montagem) = (destino, novo, pid, montagem)
    }

    public var argumentos: [String] { [destino.path, novo.path, String(pid), montagem.path] }

    /// Caminho absoluto, sem "." nem "..", sem "//" e sem barra no fim.
    private static func limpo(_ s: String) -> Bool {
        s.count > 1 && s.hasPrefix("/") && !s.hasSuffix("/") && !s.contains("//") && !s.contains("\0")
            && !s.split(separator: "/").contains { $0 == "." || $0 == ".." }
    }

    private static func dentro(_ s: String, _ pasta: String) -> Bool {
        s.hasPrefix(pasta.hasSuffix("/") ? pasta : pasta + "/")
    }

    /// `temporario`: a pasta temporária, onde o destino nunca está.
    public static func ler(_ a: [String], temporario: String = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().path) throws(Recusa) -> Pedido {
        guard a.count == 4 else { throw .quantidade }
        let (d, n, p, m) = (a[0], a[1], a[2], a[3])
        // Ponto de montagem: uma pasta logo abaixo de /Volumes, como o hdiutil monta.
        guard limpo(m), dentro(m, "/Volumes"), m.split(separator: "/").count == 2 else { throw .montagem }
        guard n == m + "/SDTally.app" else { throw .novo }
        guard limpo(d), d.hasSuffix(".app"), !dentro(d, m), !dentro(d, "/System"), !dentro(d, temporario),
              !dentro("/private" + d, temporario) else { throw .destino }
        guard (1...10).contains(p.count), p.allSatisfy({ $0.isASCII && $0.isNumber }), let pid = Int32(p), pid > 1 else { throw .pid }
        return Pedido(destino: URL(filePath: d, directoryHint: .isDirectory), novo: URL(filePath: n, directoryHint: .isDirectory),
                      pid: pid, montagem: URL(filePath: m, directoryHint: .isDirectory))
    }
}

/// O que o executor deixa para o app ler ao abrir: nas preferências do SDTally, chave `chave`.
/// Sem `erro`, trocou.
public struct Resultado: Equatable, Sendable {
    public enum Erro: String, Sendable {
        /// argumentos: o pedido não passou em Pedido.ler. aberto: o SDTally não fechou a tempo.
        /// destino: o app a trocar não é o SDTally. executor: o app não conseguiu abrir o executor.
        case argumentos, aberto, destino, assinatura, versao, troca, executor
    }

    public static let chave = "resultadoAtualizacao"
    public let versao: String
    public let erro: Erro?

    public init(versao: String, erro: Erro?) { (self.versao, self.erro) = (versao, erro) }

    public init?(_ d: [String: Any]?) {
        guard let v = d?["versao"] as? String else { return nil }
        let e = d?["erro"] as? String
        self.init(versao: v, erro: e.map { Erro(rawValue: $0) ?? .troca })
    }

    public var dicionario: [String: String] { ["versao": versao, "erro": erro?.rawValue].compactMapValues { $0 } }

    /// Grava nas preferências de outro processo (o executor não é o SDTally) e sincroniza antes de reabrir o app.
    public func gravar(dominio: String = Instalacao.idApp) {
        CFPreferencesSetAppValue(Self.chave as CFString, dicionario as CFDictionary, dominio as CFString)
        CFPreferencesAppSynchronize(dominio as CFString)
    }
}

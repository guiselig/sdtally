// Roda o sdtally embutido no .app (Contents/Helpers/sdtally) e devolve o stdout.
// Com root: osascript "with administrator privileges" (o macOS pede a senha de admin).
import Foundation
import SDTallyXPC
import os
import Synchronization

nonisolated enum ErroSdtally: LocalizedError {
    case semBinario
    case senhaCancelada
    case incompativel(Int)
    case json(String)
    case falhou(codigo: Int32, stderr: String)
    case caminhoRelativo

    var errorDescription: String? {
        switch self {
        case .semBinario: "O sdtally não está dentro do app. Monte de novo com app/montar.sh."
        case .senhaCancelada: "Senha cancelada."
        case .incompativel(let v): "Esta versão do sdtally não é compatível com o app (schema \(v))."
        case .json(let e): "JSON inesperado do sdtally: \(e)"
        case .falhou(let c, let e):
            "O sdtally parou com erro (código \(c))." + (e.isEmpty ? "" : " " + (e.split(separator: "\n").last.map(String.init) ?? ""))
        case .caminhoRelativo: "Caminho de pasta inválido: precisa começar com /."
        }
    }
}

nonisolated enum Executor {
    static let log = Logger(subsystem: "io.github.guiselig.sdtally", category: "executor")

    static var binario: String? { binario(env: ProcessInfo.processInfo.environment, bundleID: Bundle.main.bundleIdentifier) }

    /// SDTALLY_BIN só vale fora do .app (bundleID nil, Dev.ativo): dentro dele, sempre o embutido.
    static func binario(env: [String: String], bundleID: String?) -> String? {
        if bundleID == nil, let p = env["SDTALLY_BIN"] { return p }
        let p = Bundle.main.bundleURL.appending(path: "Contents/Helpers/sdtally").path
        return FileManager.default.isExecutableFile(atPath: p) ? p : nil
    }

    /// Roda como o usuário do app. Cancelar a Task manda SIGINT: o sdtally devolve o parcial
    /// marcado "interrupted" e sai com 130, que aqui não é erro.
    static func rodar(_ args: [String]) async throws -> Data {
        // Primeiro argumento que não é flag é pasta: só caminho absoluto.
        if let a = args.first, !a.hasPrefix("-"), !a.hasPrefix("/") { throw ErroSdtally.caminhoRelativo }
        guard let bin = binario else { throw ErroSdtally.semBinario }
        // Caminho de pasta não vai para o log (é dado pessoal).
        return try await processo(bin, args, modo: args.first.map { $0.hasPrefix("-") ? $0 : "pasta" } ?? "")
    }

    /// `sdtally --quick --json --watch`: fica aberto e manda uma linha JSON por tela nova do
    /// Ajustes (`log stream`, parado ~0 de CPU). `linha` e `saiu` rodam fora da main. Fechar o
    /// handle devolvido (o stdin dele) faz o sdtally matar o `log stream` e sair; o app morrer
    /// também fecha.
    /// `args` ["--auto"]: a árvore viva pelo FSEvents, uma linha (o relatório inteiro) por mudança.
    static func escutar(_ args: [String] = ["--quick", "--json", "--watch"], linha: @escaping @Sendable (Data) -> Void, saiu: @escaping @Sendable () -> Void) throws -> FileHandle {
        guard let bin = binario else { throw ErroSdtally.semBinario }
        let p = Process()
        p.executableURL = URL(fileURLWithPath: bin)
        p.arguments = args
        let ent = Pipe(), out = Pipe()
        p.standardInput = ent
        p.standardOutput = out
        p.standardError = FileHandle.nullDevice
        p.terminationHandler = { _ in saiu() }
        try p.run()
        lerLinhas(out.fileHandleForReading, linha: linha) {}
        return ent.fileHandleForWriting
    }

    /// Lê `h` linha a linha numa thread de fundo; `fim` quando o outro lado fecha.
    private static func lerLinhas(_ h: FileHandle, linha: @escaping @Sendable (Data) -> Void, fim: @escaping @Sendable () -> Void) {
        DispatchQueue.global(qos: .utility).async {
            var resto = Data()
            while case let d = h.availableData, !d.isEmpty {
                resto.append(d)
                while let i = resto.firstIndex(of: 0x0A) {
                    linha(Data(resto[..<i]))
                    resto = Data(resto[(i + 1)...])
                }
            }
            fim()
        }
    }

    /// Automático com root: o helper sobe o `sdtally --auto` como root e devolve o stdout dele.
    /// A conexão devolvida mantém o automático vivo; `invalidate()` nela faz o helper fechá-lo
    /// (o app morrer também). `saiu` pode chegar mais de uma vez (fim do stdout e conexão caída).
    static func escutarPeloHelper(linha: @escaping @Sendable (Data) -> Void, saiu: @escaping @Sendable () -> Void) async throws -> NSXPCConnection {
        guard let time = HelperXPC.timeProprio(),
              let req = HelperXPC.requisito(identifier: HelperXPC.idHelper, time: time) else {
            throw ErroSdtally.falhou(codigo: -1, stderr: "app sem assinatura de time: o helper root não funciona com assinatura ad hoc")
        }
        let c = Conexao(NSXPCConnection(machServiceName: HelperXPC.servico, options: .privileged))
        c.v.remoteObjectInterface = NSXPCInterface(with: HelperProtocolo.self)
        c.v.setCodeSigningRequirement(req)
        c.v.interruptionHandler = saiu
        c.v.invalidationHandler = saiu
        c.v.resume()
        do {
            let h: FileHandle = try await withCheckedThrowingContinuation { cont in
                let uma = Mutex(false)
                let fim: @Sendable (Result<FileHandle, Error>) -> Void = { r in
                    guard uma.withLock({ let ja = $0; $0 = true; return !ja }) else { return }
                    cont.resume(with: r)
                }
                let proxy = c.v.remoteObjectProxyWithErrorHandler { e in
                    fim(.failure(ErroSdtally.falhou(codigo: -1, stderr: "helper root indisponível: \(e.localizedDescription)")))
                } as? HelperProtocolo
                guard let proxy else { return fim(.failure(ErroSdtally.falhou(codigo: -1, stderr: "helper root: interface inválida"))) }
                proxy.iniciarAuto { h, erro in
                    if let h { fim(.success(h)) } else { fim(.failure(ErroSdtally.falhou(codigo: -1, stderr: erro ?? "helper root sem resposta"))) }
                }
            }
            lerLinhas(h, linha: linha, fim: saiu)
            return c.v
        } catch {
            c.v.invalidationHandler = nil
            c.v.invalidate()
            throw error
        }
    }

    /// `sdtally --redact names --stdin` (spec-fase6 §4.5): o JSON entra pelo stdin e volta redigido.
    /// `bin` nil: o embutido.
    static func redigir(_ dados: Data, bin: String? = nil) async throws -> Data {
        guard let bin = bin ?? binario else { throw ErroSdtally.semBinario }
        return try await processo(bin, ["--redact", "names", "--stdin"], modo: "--redact", entrada: dados)
    }

    /// Roda como root pedindo a senha. SUDO_USER/SUDO_UID: o sdtally usa para achar a home
    /// da pessoa (coleta::identidade), como faria com sudo. Não dá para cancelar: matar o
    /// osascript não mata o filho root.
    static func rodarComoAdmin(_ args: [String], prompt: String) async throws -> Data {
        guard let bin = binario else { throw ErroSdtally.semBinario }
        let req = HelperXPC.timeProprio().flatMap { HelperXPC.requisito(identifier: HelperXPC.idCLI, time: $0) }
        let cmd = scriptAdmin(bin: bin, args: args, usuario: NSUserName(), uid: getuid(), requisito: req)
        let script = [
            "on run argv",
            "do shell script (item 1 of argv) with prompt (item 2 of argv) with administrator privileges without altering line endings",
            "end run",
        ]
        do {
            return try await processo("/usr/bin/osascript", script.flatMap { ["-e", $0] } + [cmd, prompt], modo: "admin")
        } catch ErroSdtally.falhou(_, let e) where e.contains("(-128)") {
            throw ErroSdtally.senhaCancelada
        }
    }

    /// O shell do root: o sdtally do bundle fica numa pasta gravável pelo usuário e podia ser
    /// trocado entre a senha e o exec. Copia para uma pasta nova do root (mktemp -d, 0700), confere
    /// a assinatura da cópia com o mesmo requisito do helper (identifier + time), roda a cópia e
    /// apaga a pasta ao sair (trap). Tudo que vem de fora passa por `aspas`.
    /// shortcut: `requisito` nil (assinatura ad hoc, sem time) só confere que a assinatura é
    /// íntegra; numa build ad hoc o próprio app é trocável, então não há o que proteger aqui.
    static func scriptAdmin(bin: String, args: [String], usuario: String, uid: uid_t, requisito: String?) -> String {
        [
            "d=$(/usr/bin/mktemp -d /tmp/sdtally.XXXXXXXX) || exit 1",
            "trap '/bin/rm -rf \"$d\"' EXIT",
            "/bin/chmod 700 \"$d\" && /bin/cp \(aspas(bin)) \"$d/sdtally\" && /bin/chmod 500 \"$d/sdtally\" || exit 1",
            "/usr/bin/codesign --verify --strict " + (requisito.map { aspas("-R=" + $0) + " " } ?? "") + "\"$d/sdtally\" || exit 1",
            "SUDO_USER=\(aspas(usuario)) SUDO_UID=\(uid) \"$d/sdtally\" " + args.map(aspas).joined(separator: " "),
        ].joined(separator: "\n")
    }

    /// Mede o volume inteiro pelo helper root (LaunchDaemon aprovado uma vez): sem senha.
    /// Cancelar a Task manda o helper parar o sdtally (SIGINT, parcial "interrupted").
    static func rodarPeloHelper() async throws -> Data {
        guard let time = HelperXPC.timeProprio(),
              let req = HelperXPC.requisito(identifier: HelperXPC.idHelper, time: time) else {
            throw ErroSdtally.falhou(codigo: -1, stderr: "app sem assinatura de time: o helper root não funciona com assinatura ad hoc")
        }
        let inicio = ContinuousClock.now
        let c = Conexao(NSXPCConnection(machServiceName: HelperXPC.servico, options: .privileged))
        c.v.remoteObjectInterface = NSXPCInterface(with: HelperProtocolo.self)
        c.v.setCodeSigningRequirement(req)  // só fala com o helper do mesmo time
        c.v.resume()
        defer { c.v.invalidate() }
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Data, Error>) in
                let uma = Mutex(false)  // o handler de erro e a resposta podem chegar os dois
                let fim: @Sendable (Result<Data, Error>) -> Void = { r in
                    guard uma.withLock({ let ja = $0; $0 = true; return !ja }) else { return }
                    log.info("helper respondeu em \(ContinuousClock.now - inicio, privacy: .public)")
                    cont.resume(with: r)
                }
                let proxy = c.v.remoteObjectProxyWithErrorHandler { e in
                    fim(.failure(ErroSdtally.falhou(codigo: -1, stderr: "helper root indisponível: \(e.localizedDescription)")))
                } as? HelperProtocolo
                guard let proxy else { return fim(.failure(ErroSdtally.falhou(codigo: -1, stderr: "helper root: interface inválida"))) }
                // Parado antes de começar: sem isto o helper mediria tudo (o parar chega antes do medir
                // e não acha vaga; o helper também guarda esse parar, para a corrida que sobra).
                guard !Task.isCancelled else { return fim(.failure(CancellationError())) }
                proxy.medir { dados, erro in
                    if let dados { fim(.success(dados)) } else { fim(.failure(ErroSdtally.falhou(codigo: -1, stderr: erro ?? "helper root sem resposta"))) }
                }
            }
        } onCancel: {
            (c.v.remoteObjectProxy as? HelperProtocolo)?.parar()
            // Helper que não responde (não subiu, travou): invalidar faz o XPC chamar o handler
            // de erro e a espera acaba. ponytail: folga fixa de 10 s para o parcial chegar.
            DispatchQueue.global().asyncAfter(deadline: .now() + 10) { c.v.invalidate() }
        }
    }

    /// Aspas simples do shell.
    static func aspas(_ s: String) -> String {
        "'" + s.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }

    // ponytail: segura uma thread do GCD por até alguns minutos; trocar por terminationHandler
    // se rodar mais de 2 processos ao mesmo tempo.
    private static func processo(_ exe: String, _ args: [String], modo: String, entrada: Data? = nil) async throws -> Data {
        // pid e "cancelado" sob a mesma trava: um cancelamento antes do spawn não se perde.
        let estado = Mutex<(pid: pid_t, cancelado: Bool)>((0, false))
        let inicio = ContinuousClock.now
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { cont in
                DispatchQueue.global(qos: .userInitiated).async {
                    let p = Process()
                    p.executableURL = URL(fileURLWithPath: exe)
                    p.arguments = args
                    let out = Pipe(), err = Pipe()
                    p.standardOutput = out
                    p.standardError = err
                    let ent = entrada.map { _ in Pipe() }
                    if let ent {
                        p.standardInput = ent
                        // Se o sdtally sair antes de ler tudo, a escrita dá EPIPE em vez de SIGPIPE.
                        _ = fcntl(ent.fileHandleForWriting.fileDescriptor, F_SETNOSIGPIPE, 1)
                    }
                    let subiu: Bool
                    do {
                        subiu = try estado.withLock { e throws -> Bool in
                            if e.cancelado { return false }
                            try p.run()
                            e.pid = p.processIdentifier
                            return true
                        }
                    } catch { return cont.resume(throwing: error) }
                    guard subiu else { return cont.resume(throwing: CancellationError()) }
                    if let ent, let entrada {
                        DispatchQueue.global().async {
                            try? ent.fileHandleForWriting.write(contentsOf: entrada)
                            try? ent.fileHandleForWriting.close()
                        }
                    }
                    // stderr em paralelo e antes do wait: um pipe cheio (64 KB) travaria o processo.
                    let errDados = Mutex(Data())
                    let grupo = DispatchGroup()
                    grupo.enter()
                    DispatchQueue.global().async {
                        let d = err.fileHandleForReading.readDataToEndOfFile()
                        errDados.withLock { $0 = d }
                        grupo.leave()
                    }
                    let dados = out.fileHandleForReading.readDataToEndOfFile()
                    p.waitUntilExit()
                    grupo.wait()
                    estado.withLock { $0.pid = 0 }
                    let codigo = p.terminationStatus
                    log.info("\(modo, privacy: .public) saiu com \(codigo) em \(ContinuousClock.now - inicio, privacy: .public)")
                    if codigo == 0 || codigo == 130 {
                        cont.resume(returning: dados)
                    } else {
                        let e = errDados.withLock { String(decoding: $0, as: UTF8.self) }
                        cont.resume(throwing: ErroSdtally.falhou(codigo: codigo, stderr: e))
                    }
                }
            }
        } onCancel: {
            estado.withLock { e in
                e.cancelado = true
                if e.pid > 0 { kill(e.pid, SIGINT) }
            }
        }
    }
}

/// NSXPCConnection é thread-safe mas não é Sendable no SDK.
private nonisolated final class Conexao: @unchecked Sendable {
    let v: NSXPCConnection
    init(_ v: NSXPCConnection) { self.v = v }
}

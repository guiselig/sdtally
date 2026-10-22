// Helper root do SDTally: LaunchDaemon registrado pelo app (SMAppService), sobe sob demanda pelo
// serviço Mach. Só aceita conexão do SDTally.app assinado pelo mesmo time e só roda
// `sdtally --json` ou `sdtally --auto`, com argumentos fixos, para o usuário da conexão (audit
// token, não o que o cliente diz). O automático vive só enquanto a conexão do app existir.
// O sdtally do bundle fica numa pasta gravável pelo usuário: o helper copia para uma pasta do
// root, recusa o que não for arquivo comum (symlink), confere a assinatura da cópia e executa a
// cópia (sem janela entre conferir e executar).
// Limite conhecido (protótipo): a confiança é "assinado pelo time do dono". Com o certificado
// Apple Development no chaveiro do próprio usuário, um programa rodando como ele que consiga
// usar a chave assina um sdtally falso e vira root por aqui. Distribuir pede Developer ID, app em
// pasta do root (/Applications) e conferir que o bundle não é gravável pelo usuário.
import Foundation
import Security
import SDTallyXPC
import Synchronization
import os

/// Constantes do processo (fora do código de topo, que no Swift 6 é do MainActor).
enum Cfg {
    static let log = Logger(subsystem: "io.github.guiselig.sdtally", category: "helper")
    static let time = HelperXPC.timeProprio()
    static let reqApp = time.flatMap { HelperXPC.requisito(identifier: HelperXPC.idApp, time: $0) }
    static let reqCLI = time.flatMap { HelperXPC.requisito(identifier: HelperXPC.idCLI, time: $0) }
    static let estado = Estado()
    static let autos = Autos()
}

struct ErroHelper: Error, CustomStringConvertible { let description: String }

/// Uma medição por vez, de qualquer cliente. A vaga é reservada antes de copiar e conferir o
/// binário, e um "parar" que chega antes do processo existir fica guardado.
final class Estado: @unchecked Sendable {
    private let trava = NSLock()
    private var processo: Process?
    private var dono: ObjectIdentifier?
    private var cancelado = false

    func reservar(_ conexao: ObjectIdentifier) -> Bool {
        trava.withLock {
            guard dono == nil else { return false }
            dono = conexao; cancelado = false
            return true
        }
    }
    /// Sobe o processo sob a trava: ou o parar já chegou e não sobe, ou o parar acha o processo.
    /// `parado`: o parar (ou a conexão caída) que chegou antes de reservar a vaga.
    func iniciar(_ p: Process, parado: () -> Bool) throws {
        try trava.withLock {
            if cancelado || parado() { throw ErroHelper(description: "medição parada") }
            try p.run()
            processo = p
        }
    }
    func terminar() { trava.withLock { processo = nil; dono = nil; cancelado = false } }
    var ocioso: Bool { trava.withLock { dono == nil } }
    /// SIGINT: o sdtally devolve o parcial marcado "interrupted" e sai com 130.
    func parar(de conexao: ObjectIdentifier) {
        trava.withLock {
            guard conexao == dono else { return }
            cancelado = true
            if let p = processo, p.isRunning { p.interrupt() }
        }
    }
}

/// Só administradores: pelo osascript a medição root já exigia um admin.
func ehAdmin(_ uid: uid_t) -> Bool {
    guard let pw = getpwuid(uid), let admin = getgrnam("admin") else { return false }
    var grupos = [Int32](repeating: 0, count: 256)
    var n = Int32(grupos.count)
    guard getgrouplist(pw.pointee.pw_name, Int32(bitPattern: pw.pointee.pw_gid), &grupos, &n) != -1 else { return false }
    return grupos.prefix(Int(n)).contains(Int32(bitPattern: admin.pointee.gr_gid))
}
/// O único apagar do helper: a pasta 0700 que ele mesmo criou para a cópia do sdtally.
func apagarTemporaria(_ dir: URL) {
    try? FileManager.default.removeItem(at: dir)
}

/// Cópia conferida do sdtally do bundle numa pasta 0700 do root; `apagarTemporaria(dir)` depois.
func prepararBinario() throws -> (dir: URL, copia: URL) {
    let fm = FileManager.default
    let dir = URL(fileURLWithPath: NSTemporaryDirectory()).appending(path: "sdtally-\(UUID().uuidString)")
    try fm.createDirectory(at: dir, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
    do { return (dir, try conferir(fm, dir)) } catch { apagarTemporaria(dir); throw error }
}

private func conferir(_ fm: FileManager, _ dir: URL) throws -> URL {
    let origem = Bundle.main.bundleURL.appending(path: "Contents/Helpers/sdtally")
    let copia = dir.appending(path: "sdtally")
    try fm.copyItem(at: origem, to: copia)
    // copyItem copia symlink como symlink, e conferir/chmod/executar seguiriam o link até uma
    // pasta do usuário. attributesOfItem usa lstat; a cópia mora na pasta 0700 do root.
    guard (try fm.attributesOfItem(atPath: copia.path))[.type] as? FileAttributeType == .typeRegular else {
        throw ErroHelper(description: "o sdtally do app não é um arquivo comum")
    }
    try fm.setAttributes([.posixPermissions: 0o500], ofItemAtPath: copia.path)

    var codigo: SecStaticCode?
    var req: SecRequirement?
    guard let reqCLI = Cfg.reqCLI, SecStaticCodeCreateWithPath(copia as CFURL, [], &codigo) == errSecSuccess, let codigo,
          SecRequirementCreateWithString(reqCLI as CFString, [], &req) == errSecSuccess, let req else {
        throw ErroHelper(description: "não deu para ler a assinatura do sdtally")
    }
    let st = SecStaticCodeCheckValidity(codigo, SecCSFlags(rawValue: kSecCSCheckAllArchitectures | kSecCSStrictValidate), req)
    guard st == errSecSuccess else {
        Cfg.log.error("sdtally do bundle não passou na assinatura: \(st)")
        throw ErroHelper(description: "o sdtally do app não está assinado pelo mesmo time (\(st))")
    }
    return copia
}

/// Como o sudo faria: o sdtally acha a home da pessoa por SUDO_USER (coleta::identidade).
func ambiente(uid: uid_t) throws -> [String: String] {
    guard let pw = getpwuid(uid) else { throw ErroHelper(description: "usuário \(uid) não existe") }
    return ["SUDO_USER": String(cString: pw.pointee.pw_name), "SUDO_UID": "\(uid)", "PATH": "/usr/bin:/bin:/usr/sbin:/sbin"]
}

func executar(uid: uid_t, conexao: ObjectIdentifier, parado: () -> Bool) throws -> Data {
    let env = try ambiente(uid: uid)
    guard Cfg.estado.reservar(conexao) else { throw ErroHelper(description: "já há uma medição em andamento") }
    defer { Cfg.estado.terminar() }
    if parado() { throw ErroHelper(description: "medição parada") }
    let (dir, copia) = try prepararBinario()
    defer { apagarTemporaria(dir) }

    let p = Process()
    p.executableURL = copia
    p.arguments = ["--json"]
    p.environment = env
    let out = Pipe(), err = Pipe()
    p.standardOutput = out
    p.standardError = err
    try Cfg.estado.iniciar(p, parado: parado)
    Cfg.log.info("medindo para uid \(uid), pid \(p.processIdentifier)")
    // stderr em paralelo: pipe cheio travaria o sdtally.
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
    Cfg.log.info("sdtally saiu com \(p.terminationStatus)")
    guard p.terminationStatus == 0 || p.terminationStatus == 130 else {
        let ultima = errDados.withLock { String(decoding: $0, as: UTF8.self) }.split(separator: "\n").last.map(String.init) ?? ""
        throw ErroHelper(description: "o sdtally parou com erro (código \(p.terminationStatus)) \(ultima)")
    }
    return dados
}

/// Um automático por vez, de qualquer cliente (o pedido novo fecha o anterior): o processo e a
/// ponta de escrita do stdin dele.
/// Fechar o stdin faz o sdtally sair na hora (ele não tem nada por gravar).
final class Autos: @unchecked Sendable {
    private let trava = NSLock()
    private var atual: (dono: ObjectIdentifier, p: Process, stdin: FileHandle)?
    var ocioso: Bool { trava.withLock { atual == nil } }

    /// `fechada`: a conexão caiu enquanto o binário era copiado e conferido; conferida sob a trava,
    /// e o invalidationHandler marca antes de chamar `parar(de:)`, então um dos dois vê o outro.
    func iniciar(uid: uid_t, conexao: ObjectIdentifier, fechada: () -> Bool) throws -> FileHandle {
        let env = try ambiente(uid: uid)
        let (dir, copia) = try prepararBinario()
        let p = Process()
        p.executableURL = copia
        p.arguments = ["--auto"]
        p.environment = env
        let ent = Pipe(), out = Pipe()
        p.standardInput = ent
        p.standardOutput = out
        p.standardError = FileHandle.nullDevice
        p.terminationHandler = { [weak self] p in
            apagarTemporaria(dir)
            self?.trava.withLock { if self?.atual?.p === p { self?.atual = nil } }
            Cfg.log.info("automático saiu com \(p.terminationStatus)")
        }
        try trava.withLock {
            if fechada() { apagarTemporaria(dir); throw ErroHelper(description: "conexão fechada") }
            // Um por vez: o novo pedido (o app reaberto antes de a conexão velha cair) fecha o anterior.
            if let a = atual { encerrar(a.p, a.stdin) }
            do { try p.run() } catch { apagarTemporaria(dir); throw error }
            atual = (conexao, p, ent.fileHandleForWriting)
        }
        Cfg.log.info("automático para uid \(uid), pid \(p.processIdentifier)")
        return out.fileHandleForReading
    }

    func parar(de conexao: ObjectIdentifier) {
        trava.withLock {
            guard let a = atual, a.dono == conexao else { return }
            encerrar(a.p, a.stdin)
        }
    }

    /// Fecha o stdin (o sdtally sai na hora); se não sair em 5 s, SIGTERM.
    private func encerrar(_ p: Process, _ stdin: FileHandle) {
        try? stdin.close()
        DispatchQueue.global().asyncAfter(deadline: .now() + 5) { if p.isRunning { p.terminate() } }
    }
}

final class Servico: NSObject, HelperProtocolo, Sendable {
    let uid: uid_t
    let id: ObjectIdentifier
    /// A conexão caiu (invalidationHandler): nada mais sobe por ela.
    let fechada = Mutex(false)
    /// O app pediu parar ou a conexão caiu; fica marcado mesmo antes de o medir reservar a vaga.
    let parado = Mutex(false)
    init(uid: uid_t, id: ObjectIdentifier) { self.uid = uid; self.id = id }

    func medir(reply: @escaping @Sendable (Data?, String?) -> Void) {
        DispatchQueue.global(qos: .userInitiated).async { [self] in
            do { reply(try executar(uid: uid, conexao: id, parado: { parado.withLock { $0 } }), nil) } catch { reply(nil, "\(error)") }
        }
    }

    func parar() {
        parado.withLock { $0 = true }
        Cfg.estado.parar(de: id)
    }

    /// Marca antes de parar: quem sobe processo confere a marca sob a trava de Estado/Autos.
    func fechar() {
        fechada.withLock { $0 = true }
        parar()
        Cfg.autos.parar(de: id)
    }

    func iniciarAuto(reply: @escaping @Sendable (FileHandle?, String?) -> Void) {
        DispatchQueue.global(qos: .utility).async { [self] in
            do {
                let h = try Cfg.autos.iniciar(uid: uid, conexao: id, fechada: { fechada.withLock { $0 } })
                reply(h, nil)
                // O XPC duplicou o descritor para o app; a cópia do helper não serve mais.
                try? h.close()
            } catch { reply(nil, "\(error)") }
        }
    }
}

final class Delegado: NSObject, NSXPCListenerDelegate, Sendable {
    /// Conexões vivas. Sem nenhuma e sem processo rodando, o helper sai: o launchd sobe de novo
    /// na próxima conexão, já com o executável do app atualizado (sem registrar de novo).
    private let abertas = Mutex(0)

    /// Também ao subir: o launchd sobe o helper por uma conexão que pode ser recusada (não admin),
    /// e ele ficaria residente como root. Conferir e sair sob a mesma trava da contagem: uma
    /// conexão aceita no meio não morre com o exit.
    func sairSeOcioso() {
        DispatchQueue.global().asyncAfter(deadline: .now() + 10) { [self] in
            abertas.withLock { n in
                guard n == 0 else { return }
                guard Cfg.estado.ocioso, Cfg.autos.ocioso else { return sairSeOcioso() }
                Cfg.log.info("sem conexões; saindo")
                exit(0)
            }
        }
    }

    func listener(_ listener: NSXPCListener, shouldAcceptNewConnection c: NSXPCConnection) -> Bool {
        guard ehAdmin(c.effectiveUserIdentifier) else {
            Cfg.log.error("recusado: uid \(c.effectiveUserIdentifier) não é admin")
            return false
        }
        let s = Servico(uid: c.effectiveUserIdentifier, id: ObjectIdentifier(c))
        c.exportedInterface = NSXPCInterface(with: HelperProtocolo.self)
        c.exportedObject = s
        // App fechou no meio: não deixa medição nem automático órfão.
        abertas.withLock { $0 += 1 }
        c.invalidationHandler = { [self] in
            s.fechar()
            if abertas.withLock({ $0 -= 1; return $0 }) == 0 { sairSeOcioso() }
        }
        c.resume()
        return true
    }
}

// Ad hoc não tem time: sem como validar o cliente, o helper não atende ninguém.
guard let time = Cfg.time, let reqApp = Cfg.reqApp, Cfg.reqCLI != nil else {
    Cfg.log.error("helper sem time de assinatura; saindo")
    exit(78)  // EX_CONFIG
}
let listener = NSXPCListener(machServiceName: HelperXPC.servico)
listener.setConnectionCodeSigningRequirement(reqApp)
let delegado = Delegado()
listener.delegate = delegado
listener.resume()
delegado.sairSeOcioso()
Cfg.log.info("helper pronto (time \(time, privacy: .public))")
dispatchMain()

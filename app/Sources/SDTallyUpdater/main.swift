// SDTallyUpdater: o atualizador opcional, desligado por padrão. Vai dormente em
// SDTally.app/Contents/Library/LoginItems e só roda depois de a pessoa ligar em Configurações
// (Atualizador.swift registra o item de início). É o único código do SDTally que usa a rede: uma
// vez por dia api.github.com e, só depois do clique em Instalar, o DMG do release (github.com e o
// host para onde ele redireciona). Nada é instalado sem o clique.
import AppKit
import OSLog
import UserNotifications

let log = Logger(subsystem: "io.github.guiselig.sdtally.updater", category: "atualizador")

enum Falha: Error {
    case http(Int), digest, dmg, assinatura, versao, aberto, grande
}

/// Recusa redirecionamento para fora de `Lancamento.hosts` ou sem https.
nonisolated final class SoGithub: NSObject, URLSessionTaskDelegate, Sendable {
    func urlSession(_ s: URLSession, task: URLSessionTask, willPerformHTTPRedirection r: HTTPURLResponse, newRequest: URLRequest) async -> URLRequest? {
        guard Lancamento.permitido(newRequest.url) else {
            log.error("redirecionamento recusado: \(newRequest.url?.host() ?? "?", privacy: .public)")
            return nil
        }
        return newRequest
    }
}

final class Delegado: NSObject, NSApplicationDelegate, UNUserNotificationCenterDelegate {
    private var dono: URL!
    private var instalando = false
    private var relogio: Timer?
    private let sessao = URLSession(configuration: .ephemeral)
    private let versaoPropria = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "0"

    func applicationDidFinishLaunching(_ n: Notification) {
        guard let d = Instalacao.appDono(Bundle.main.bundleURL) else {
            log.error("fora de um SDTally.app: saindo")
            exit(0)
        }
        dono = d
        let c = UNUserNotificationCenter.current()
        c.delegate = self
        c.setNotificationCategories([
            UNNotificationCategory(identifier: "nova", actions: [
                UNNotificationAction(identifier: "instalar", title: String(localized: "Instalar")),
                UNNotificationAction(identifier: "depois", title: String(localized: "Depois")),
            ], intentIdentifiers: []),
            UNNotificationCategory(identifier: "falha", actions: [
                UNNotificationAction(identifier: "versoes", title: String(localized: "Ver versões")),
            ], intentIdentifiers: []),
        ])
        Task {
            _ = try? await c.requestAuthorization(options: [.alert])
            // Depois de instalar, o atualizador velho abre o novo antes de sair: os dois convivem por um instante.
            try? await Task.sleep(for: .seconds(5))
            if NSRunningApplication.runningApplications(withBundleIdentifier: Bundle.main.bundleIdentifier ?? "")
                .contains(where: { $0 != .current && ($0.launchDate ?? .distantFuture) < (NSRunningApplication.current.launchDate ?? .now) }) {
                log.info("outro atualizador já roda: saindo")
                exit(0)
            }
            await verificar()
        }
        // De hora em hora confere se já passou um dia: o Mac dorme e o Timer de 24 h atrasaria.
        relogio = Timer.scheduledTimer(withTimeInterval: 3600, repeats: true) { _ in
            Task { @MainActor in await (NSApp.delegate as? Delegado)?.verificar() }
        }
    }

    // MARK: Verificar

    /// No máximo uma consulta por dia, contada desde a última resposta do GitHub (também entre reinícios).
    /// Por isso a notificação da mesma versão também não repete no mesmo dia.
    func verificar() async {
        let d = UserDefaults.standard
        if let em = d.object(forKey: "verificouEm") as? Date, Date.now.timeIntervalSince(em) < 23 * 3600 { return }
        guard !instalando else { return }
        do {
            var r = URLRequest(url: Lancamento.api)
            r.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
            r.setValue("2022-11-28", forHTTPHeaderField: "X-GitHub-Api-Version")
            r.setValue("SDTallyUpdater/\(versaoPropria) (+https://github.com/guiselig/sdtally)", forHTTPHeaderField: "User-Agent")
            let (dados, resp) = try await sessao.data(for: r, delegate: SoGithub())
            d.set(Date.now, forKey: "verificouEm")
            let codigo = (resp as? HTTPURLResponse)?.statusCode ?? 0
            guard codigo == 200 else { throw Falha.http(codigo) }
            let l = try Lancamento.ler(dados)
            guard let atual = Instalacao.versao(dono), Versao.maisNova(l.versao, que: atual) else {
                log.info("sem versão nova (\(l.versao, privacy: .public))")
                return
            }
            avisar("nova-\(l.versao)", titulo: String(localized: "SDTally \(l.versao) disponível"),
                   texto: String(localized: "Clique em Instalar para atualizar. Nada é instalado sem o seu clique."), categoria: "nova", info: l.info)
        } catch {
            log.error("verificação: \(String(describing: error), privacy: .public)")
        }
    }

    private func avisar(_ id: String, titulo: String, texto: String, categoria: String, info: [String: String]) {
        let c = UNMutableNotificationContent()
        c.title = titulo
        c.body = texto
        c.categoryIdentifier = categoria
        c.userInfo = info
        UNUserNotificationCenter.current().add(UNNotificationRequest(identifier: id, content: c, trigger: nil))
    }

    private func falhar(_ l: Lancamento, _ motivo: String) {
        log.error("instalação: \(motivo, privacy: .public)")
        avisar("falha", titulo: String(localized: "Não deu para instalar o SDTally \(l.versao)"),
               texto: motivo + " " + String(localized: "O SDTally instalado continua como estava."), categoria: "falha",
               info: ["pagina": l.pagina.absoluteString])
    }

    nonisolated func userNotificationCenter(_ c: UNUserNotificationCenter, didReceive r: UNNotificationResponse) async {
        let acao = r.actionIdentifier
        let info = r.notification.request.content.userInfo as? [String: String] ?? [:]
        await MainActor.run {
            switch acao {
            case "instalar": if let l = Lancamento(info: info) { Task { await self.instalar(l) } }
            case "versoes":
                let u = info["pagina"].flatMap(URL.init(string:)).flatMap { Lancamento.permitido($0) ? $0 : nil }
                NSWorkspace.shared.open(u ?? Lancamento.versoes)
            default: break
            }
        }
    }

    // MARK: Instalar (só pelo clique)

    func instalar(_ l: Lancamento) async {
        guard !instalando else { return }
        instalando = true
        defer { instalando = false }
        let destino: URL = dono
        guard Instalacao.podeEscrever(destino) else {
            falhar(l, String(localized: "Sem permissão para gravar em \(destino.deletingLastPathComponent().path). Baixe o DMG na página de versões."))
            return
        }
        let tmp = FileManager.default.temporaryDirectory.appending(path: "SDTallyUpdater-\(UUID().uuidString)")
        defer { Instalacao.apagarTemporario(tmp) }
        var montado: URL?
        var fechou = false
        do {
            try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
            let (dados, resp) = try await sessao.data(for: URLRequest(url: l.dmg), delegate: SoGithub())
            let codigo = (resp as? HTTPURLResponse)?.statusCode ?? 0
            guard codigo == 200 else { throw Falha.http(codigo) }
            // shortcut: o DMG inteiro na memória (uns 6 MB); acima de 200 MB recusa.
            guard dados.count < 200_000_000 else { throw Falha.grande }
            guard Lancamento.sha256(dados) == l.sha256 else { throw Falha.digest }
            let dmg = tmp.appending(path: "SDTally-\(l.versao).dmg")
            try dados.write(to: dmg)
            montado = try anexar(dmg)
            let novo = montado!.appending(path: "SDTally.app")
            guard Instalacao.assinaturaValida(novo) else { throw Falha.assinatura }
            guard Instalacao.versao(novo) == l.versao, let atual = Instalacao.versao(destino), Versao.maisNova(l.versao, que: atual) else { throw Falha.versao }
            fechou = try await fecharSDTally(destino)
            try Instalacao.trocar(destino, por: novo)
            desanexar(montado!)
            montado = nil
            log.info("instalado: \(l.versao, privacy: .public)")
            reabrir(destino)
        } catch {
            if let montado { desanexar(montado) }
            if case Falha.aberto = error {
                avisar("nova-\(l.versao)", titulo: String(localized: "SDTally \(l.versao) disponível"),
                       texto: String(localized: "Feche o SDTally para terminar de instalar a versão \(l.versao)."), categoria: "nova", info: l.info)
                return
            }
            if fechou { NSWorkspace.shared.openApplication(at: destino, configuration: NSWorkspace.OpenConfiguration()) { _, _ in } }
            falhar(l, motivo(error))
        }
    }

    private func motivo(_ e: Error) -> String {
        switch e {
        case Falha.digest: String(localized: "O arquivo baixado não confere com o SHA-256 publicado.")
        case Falha.dmg: String(localized: "O DMG não abriu.")
        case Falha.assinatura: String(localized: "A assinatura do app novo não é a do SDTally.")
        case Falha.versao: String(localized: "A versão no DMG não é mais nova que a instalada.")
        case Falha.http, Falha.grande, is URLError: String(localized: "O download falhou: \(String(describing: e))")
        default: String(localized: "Não deu para trocar o app: \(e.localizedDescription)")
        }
    }

    /// Pede para o SDTally (o desta pasta) fechar e espera até 60 s. Medindo, ele pergunta antes
    /// ("Continuar medindo" / "Fechar mesmo assim"); continuou aberto, `Falha.aberto`.
    private func fecharSDTally(_ destino: URL) async throws -> Bool {
        let alvo = destino.resolvingSymlinksInPath()
        let abertos = NSRunningApplication.runningApplications(withBundleIdentifier: Instalacao.idApp)
            .filter { $0.bundleURL?.resolvingSymlinksInPath() == alvo }
        guard !abertos.isEmpty else { return false }
        abertos.forEach { $0.terminate() }
        for _ in 0..<60 {
            if abertos.allSatisfy(\.isTerminated) { return true }
            try? await Task.sleep(for: .seconds(1))
        }
        throw Falha.aberto
    }

    /// Abre o SDTally novo e o atualizador novo (outra instância: este sai logo depois).
    private func reabrir(_ destino: URL) {
        let novoAtualizador = destino.appending(path: "Contents/Library/LoginItems/SDTallyUpdater.app")
        NSWorkspace.shared.openApplication(at: destino, configuration: NSWorkspace.OpenConfiguration()) { _, _ in
            let c = NSWorkspace.OpenConfiguration()
            c.activates = false
            c.createsNewApplicationInstance = true
            NSWorkspace.shared.openApplication(at: novoAtualizador, configuration: c) { _, _ in exit(0) }
        }
    }

    // MARK: hdiutil

    private func hdiutil(_ args: [String]) -> Data? {
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
    private func anexar(_ dmg: URL) throws -> URL {
        guard let d = hdiutil(["attach", "-nobrowse", "-readonly", "-noautoopen", "-plist", dmg.path]),
              let p = try? PropertyListSerialization.propertyList(from: d, format: nil) as? [String: Any],
              let ents = p["system-entities"] as? [[String: Any]],
              let m = ents.compactMap({ $0["mount-point"] as? String }).first else { throw Falha.dmg }
        return URL(filePath: m, directoryHint: .isDirectory)
    }

    private func desanexar(_ m: URL) {
        if hdiutil(["detach", m.path, "-quiet"]) == nil { _ = hdiutil(["detach", m.path, "-quiet", "-force"]) }
    }
}

let app = NSApplication.shared
let delegado = Delegado()
app.delegate = delegado
app.run()

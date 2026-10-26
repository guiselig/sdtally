// Atualizações: a única pasta do app com rede (tests/sem_rede.rs e o grep do CI só deixam esta).
// Só o github.com, por https: o último release (sozinho ao abrir e a cada 24 h, ou quando a
// pessoa pede) e, só depois do clique em Instalar, o DMG dele. Nada sobre o disco vai junto.
// Desligado em Configurações > Versões, nenhuma conexão sai sozinha. O sdtally (CLI) e o helper
// root nunca usam a rede; vêm dentro do app e são atualizados com ele.
// Instalar: baixa para a pasta temporária, confere o SHA-256, monta o DMG só leitura, confere a
// assinatura e a versão do app novo, copia o executor (SDTallyUpdater) para a mesma pasta e fecha
// o app pelo caminho normal (que pergunta se há medição). Fechou: o executor troca e reabre.
// Continuou medindo: desmonta e apaga a pasta, e nada muda.
import AppKit
import OSLog
import SDTallyInstalacao
import ServiceManagement

@Observable final class Atualizacoes {
    struct Novidade: Equatable {
        let versao: String
        let pagina: URL
    }

    enum Estado: Equatable {
        case parado, procurando, baixando, instalando, erro(String)
    }

    static let unica = Atualizacoes()

    /// Procurar sozinho (padrão: sim). Desligar cancela o que estiver em andamento.
    var ligado: Bool {
        didSet {
            guard ligado != oldValue else { return }
            UserDefaults.standard.set(ligado, forKey: Self.chaveLigado)
            if ligado { procurarSePreciso() } else { desligar() }
        }
    }
    private(set) var disponivel: Novidade?
    private(set) var estado = Estado.parado
    /// A última resposta do GitHub (também entre reinícios): conta as 24 h.
    private(set) var ultimaChecagem: Date? {
        didSet { UserDefaults.standard.set(ultimaChecagem, forKey: Self.chaveChecagem) }
    }
    /// Aviso único de rede para quem já tinha visto as boas-vindas. Só a UI lê e escreve.
    var jaAvisouRede: Bool {
        didSet { UserDefaults.standard.set(jaAvisouRede, forKey: Self.chaveAvisou) }
    }

    static let chaveLigado = "procurarAtualizacoes"
    static let chaveAvisou = "avisouRedeAtualizacoes"
    private static let chaveChecagem = "ultimaChecagemAtualizacoes"
    private static let chaveNovidade = "novidadeAtualizacoes"
    nonisolated static let intervalo: TimeInterval = 24 * 3600
    /// O item de início do atualizador antigo (a184a39, nunca lançado; registrado por builds de desenvolvimento).
    private static let idAntigo = "io.github.guiselig.sdtally.updater"

    private static let log = Logger(subsystem: "io.github.guiselig.sdtally", category: "atualizacoes")
    private static let sessao = URLSession(configuration: .ephemeral)
    private static let versaoInstalada = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""

    private var lancamento: Lancamento? {
        didSet {
            disponivel = lancamento.map { Novidade(versao: $0.versao, pagina: $0.pagina) }
            if let lancamento { UserDefaults.standard.set(lancamento.info, forKey: Self.chaveNovidade) } else { UserDefaults.standard.removeObject(forKey: Self.chaveNovidade) }
        }
    }
    private var tarefa: Task<Void, Never>?
    private var relogio: Timer?
    /// Pronto para o executor: só entre o fim de `instalar` e o fechamento do app.
    private var entrega: (pasta: URL, executor: URL, pedido: Pedido, versao: String)?
    /// Pasta e montagem de um `preparar` em andamento: o app fechou no meio, aoFechar limpa.
    private var emPreparo: (pasta: URL, montagem: URL?)?

    private init() {
        let d = UserDefaults.standard
        d.register(defaults: [Self.chaveLigado: true])
        ligado = d.bool(forKey: Self.chaveLigado)
        jaAvisouRede = d.bool(forKey: Self.chaveAvisou)
        ultimaChecagem = d.object(forKey: Self.chaveChecagem) as? Date
        // A novidade guardada só vale se ainda for mais nova que a instalada (depois de instalar, some).
        let l = (d.dictionary(forKey: Self.chaveNovidade) as? [String: String]).flatMap(Lancamento.init(info:))
        lancamento = l.flatMap { Versao.maisNova($0.versao, que: Self.versaoInstalada) ? $0 : nil }
        disponivel = lancamento.map { Novidade(versao: $0.versao, pagina: $0.pagina) }
    }

    // MARK: Quando procurar (funções puras, testadas em SDTallyTests)

    /// Nunca em desenvolvimento nem em foto; sozinho, só ligado.
    nonisolated static func pode(ligado: Bool, dev: Bool, foto: Bool) -> Bool { ligado && !dev && !foto }

    /// Já passou `intervalo` desde a última resposta. Relógio voltou (última no futuro): procura.
    nonisolated static func vence(agora: Date, ultima: Date?, intervalo: TimeInterval = intervalo) -> Bool {
        guard let ultima else { return true }
        let d = agora.timeIntervalSince(ultima)
        return d >= intervalo || d < 0
    }

    private func pode(sozinho: Bool) -> Bool {
        Self.pode(ligado: !sozinho || ligado, dev: Dev.ativo, foto: Foto.env["SDTALLY_FOTO"] != nil)
    }

    private var ocupado: Bool { [.procurando, .baixando, .instalando].contains(estado) }

    // MARK: Ciclo de vida (SDTallyApp.Delegado)

    /// Ao abrir: o resultado do executor, o item de início antigo, e a checagem com o relógio.
    func iniciar() {
        lerResultado()
        guard !Dev.ativo else { return }
        tirarAtualizadorAntigo()
        procurarSePreciso()
        // De hora em hora (e ao acordar) confere se já passaram 24 h: o Mac dorme e um Timer de 24 h atrasaria.
        relogio = Timer.scheduledTimer(withTimeInterval: 3600, repeats: true) { _ in
            Task { @MainActor in Atualizacoes.unica.procurarSePreciso() }
        }
        NSWorkspace.shared.notificationCenter.addObserver(forName: NSWorkspace.didWakeNotification, object: nil, queue: .main) { _ in
            Task { @MainActor in Atualizacoes.unica.procurarSePreciso() }
        }
    }

    /// No applicationWillTerminate: com uma instalação pronta, abre o executor, que espera este processo sair.
    func aoFechar() {
        if let p = emPreparo {
            if let m = p.montagem { Instalacao.desanexar(m) }
            Instalacao.apagarTemporario(p.pasta)
        }
        guard let e = entrega else { return }
        entrega = nil
        estado = .instalando
        guard Instalacao.lancarSolto(e.executor, e.pedido.argumentos) else {
            Self.log.error("executor não abriu")
            Instalacao.desanexar(e.pedido.montagem)
            Instalacao.apagarTemporario(e.pasta)
            UserDefaults.standard.set(Resultado(versao: e.versao, erro: .executor).dicionario, forKey: Resultado.chave)
            return
        }
    }

    // MARK: Procurar

    func procurarSePreciso() {
        guard pode(sozinho: true), !ocupado, Self.vence(agora: .now, ultima: ultimaChecagem) else { return }
        procurar(manual: false)
    }

    /// Pedido da pessoa: vale mesmo desligado.
    func procurarAgora() {
        guard pode(sozinho: false), !ocupado else { return }
        procurar(manual: true)
    }

    private func procurar(manual: Bool) {
        estado = .procurando
        tarefa = Task {
            do {
                let (dados, codigo) = try await Self.baixar(Lancamento.api, api: true)
                ultimaChecagem = .now
                guard codigo == 200 else { throw Falha.http(codigo) }
                let l = try Lancamento.ler(dados)
                lancamento = Versao.maisNova(l.versao, que: Self.versaoInstalada) ? l : nil
                estado = .parado
            } catch {
                guard !Task.isCancelled else { return }
                Self.log.error("procurar: \(String(describing: error), privacy: .public)")
                // Sozinho, a falha (sem internet, por exemplo) não aparece: tenta de novo na próxima hora.
                estado = manual ? .erro(String(localized: "Não deu para procurar atualizações: \(Self.detalhe(error))")) : .parado
            }
        }
    }

    private func desligar() {
        tarefa?.cancel()
        tarefa = nil
        lancamento = nil
        if ocupado { estado = .parado }
    }

    // MARK: Instalar (só pelo clique)

    func instalar() {
        guard pode(sozinho: false), !ocupado, let l = lancamento else { return }
        estado = .baixando
        tarefa = Task {
            do {
                entrega = try await preparar(l)
            } catch {
                guard !Task.isCancelled else { return }
                Self.log.error("instalar: \(String(describing: error), privacy: .public)")
                estado = .erro(Self.motivo(error))
                return
            }
            estado = .instalando
            // O caminho de sempre: medindo, pergunta "Continuar medindo" / "Fechar mesmo assim".
            // Fechando, o terminate não volta (aoFechar abre o executor). Voltou: continuou medindo.
            NSApp.terminate(nil)
            guard let e = entrega else { return }
            entrega = nil
            await Task.detached { Instalacao.desanexar(e.pedido.montagem); Instalacao.apagarTemporario(e.pasta) }.value
            estado = .parado
        }
    }

    enum Falha: Error {
        case http(Int), grande, digest, dmg, assinatura, versao, semEscrita(String), executor
    }

    /// Baixa, confere e monta. Qualquer erro (ou o cancelamento) desmonta e apaga a pasta.
    private func preparar(_ l: Lancamento) async throws -> (pasta: URL, executor: URL, pedido: Pedido, versao: String) {
        let destino = Bundle.main.bundleURL
        guard Instalacao.podeEscrever(destino) else { throw Falha.semEscrita(destino.deletingLastPathComponent().path) }
        let pasta = FileManager.default.temporaryDirectory.appending(path: Instalacao.prefixoTemporario + UUID().uuidString, directoryHint: .isDirectory)
        var montagem: URL?
        emPreparo = (pasta, nil)
        defer { emPreparo = nil }
        do {
            try FileManager.default.createDirectory(at: pasta, withIntermediateDirectories: true)
            let (dados, codigo) = try await Self.baixar(l.dmg, api: false)
            guard codigo == 200 else { throw Falha.http(codigo) }
            // shortcut: o DMG inteiro na memória (uns 6 MB); acima de 200 MB recusa.
            guard dados.count < 200_000_000 else { throw Falha.grande }
            guard Lancamento.sha256(dados) == l.sha256 else { throw Falha.digest }
            let dmg = pasta.appending(path: "SDTally-\(l.versao).dmg")
            try dados.write(to: dmg)
            try Task.checkCancellation()
            guard let m = await Task.detached(operation: { Instalacao.anexar(dmg) }).value else { throw Falha.dmg }
            montagem = m
            emPreparo = (pasta, m)
            let novo = m.appending(path: "SDTally.app", directoryHint: .isDirectory)
            guard await Task.detached(operation: { Instalacao.assinaturaValida(novo) }).value else { throw Falha.assinatura }
            guard Instalacao.versao(novo) == l.versao, Versao.maisNova(l.versao, que: Self.versaoInstalada) else { throw Falha.versao }
            // Bytes, não copyItem: a cópia sai sem os atributos estendidos do bundle (a quarentena).
            let executor = pasta.appending(path: "SDTallyUpdater")
            try Data(contentsOf: destino.appending(path: "Contents/Helpers/SDTallyUpdater")).write(to: executor)
            try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: executor.path)
            let pedido = Pedido(destino: destino, novo: novo, pid: getpid(), montagem: m)
            // O executor recusa o que Pedido.ler recusar: melhor saber agora, com o app aberto.
            guard (try? Pedido.ler(pedido.argumentos))?.argumentos == pedido.argumentos else { throw Falha.executor }
            try Task.checkCancellation()
            return (pasta, executor, pedido, l.versao)
        } catch {
            let m = montagem
            await Task.detached { if let m { Instalacao.desanexar(m) }; Instalacao.apagarTemporario(pasta) }.value
            throw error
        }
    }

    // MARK: Rede

    /// Recusa redirecionamento para fora de `Lancamento.hosts` ou sem https.
    private nonisolated final class SoGithub: NSObject, URLSessionTaskDelegate, Sendable {
        func urlSession(_ s: URLSession, task: URLSessionTask, willPerformHTTPRedirection r: HTTPURLResponse, newRequest: URLRequest) async -> URLRequest? {
            Lancamento.permitido(newRequest.url) ? newRequest : nil
        }
    }

    /// Um GET, sem cookies nem cache. Manda só o pedido e o User-Agent com a versão do app.
    private static func baixar(_ u: URL, api: Bool) async throws -> (Data, Int) {
        guard Lancamento.permitido(u) else { throw Falha.http(0) }
        var r = URLRequest(url: u)
        if api {
            r.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
            r.setValue("2022-11-28", forHTTPHeaderField: "X-GitHub-Api-Version")
        }
        r.setValue("SDTally/\(versaoInstalada) (+https://github.com/guiselig/sdtally)", forHTTPHeaderField: "User-Agent")
        let (d, resp) = try await sessao.data(for: r, delegate: SoGithub())
        return (d, (resp as? HTTPURLResponse)?.statusCode ?? 0)
    }

    // MARK: Mensagens

    private static func detalhe(_ e: Error) -> String {
        switch e {
        case Falha.http(let c): "HTTP \(c)"
        case let u as URLError: u.localizedDescription
        default: String(describing: e)
        }
    }

    private static func motivo(_ e: Error) -> String {
        switch e {
        case Falha.digest: String(localized: "O arquivo baixado não confere com o SHA-256 publicado.")
        case Falha.dmg: String(localized: "O DMG não abriu.")
        case Falha.assinatura: String(localized: "A assinatura do app novo não é a do SDTally.")
        case Falha.versao: String(localized: "A versão no DMG não é mais nova que a instalada.")
        case Falha.semEscrita(let p): String(localized: "Sem permissão para gravar em \(p). Baixe o DMG na página de versões.")
        case Falha.http, Falha.grande, is URLError: String(localized: "O download falhou: \(detalhe(e))")
        default: String(localized: "Não deu para trocar o app: \(String(describing: e))")
        }
    }

    /// Ao abrir depois de uma instalação: o executor deixou o resultado nas preferências. Lido uma vez.
    private func lerResultado() {
        let d = UserDefaults.standard
        guard let r = Resultado(d.dictionary(forKey: Resultado.chave)) else { return }
        d.removeObject(forKey: Resultado.chave)
        guard let e = r.erro else { return }
        Self.log.error("executor: \(e.rawValue, privacy: .public)")
        estado = .erro(e == .aberto
            ? String(localized: "O SDTally não fechou a tempo. A versão instalada continua como estava.")
            : String(localized: "Não deu para instalar a versão nova. A versão instalada continua como estava."))
    }

    /// Builds de desenvolvimento do a184a39 registraram o atualizador antigo como item de início:
    /// tira o registro uma vez e fecha o processo dele, se ainda roda (ele usava a rede sozinho).
    private func tirarAtualizadorAntigo() {
        let d = UserDefaults.standard
        if !d.bool(forKey: "tirouAtualizadorAntigo") {
            d.set(true, forKey: "tirouAtualizadorAntigo")
            let s = SMAppService.loginItem(identifier: Self.idAntigo)
            if s.status != .notRegistered { try? s.unregister() }
        }
        let abertos = NSRunningApplication.runningApplications(withBundleIdentifier: Self.idAntigo)
        guard !abertos.isEmpty else { return }
        abertos.forEach { $0.terminate() }
        Task {
            try? await Task.sleep(for: .seconds(5))
            abertos.filter { !$0.isTerminated }.forEach { $0.forceTerminate() }
        }
    }
}

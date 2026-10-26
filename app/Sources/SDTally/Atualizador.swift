// Atualizador opcional, desligado por padrão: o SDTallyUpdater.app vai dormente em
// Contents/Library/LoginItems e só roda depois de a pessoa ligar em Configurações, que registra o
// item de início (SMAppService.loginItem). O app continua sem rede: quem fala com o github.com é o
// SDTallyUpdater (app/Sources/SDTallyUpdater), e só ele.
import AppKit
import ServiceManagement

enum Atualizador {
    enum Estado { case desligado, ligado, aguardandoAprovacao, indisponivel }

    static let id = "io.github.guiselig.sdtally.updater"
    /// A escolha da pessoa: aoAbrir registra de novo se ela ligou e o registro sumiu.
    private static let chave = "atualizarSozinho"
    private static var servico: SMAppService { .loginItem(identifier: id) }

    /// Fora do .app (desenvolvimento) ou num .app montado sem o atualizador.
    private static var disponivel: Bool {
        !Dev.ativo && FileManager.default.fileExists(atPath: Bundle.main.bundleURL.appending(path: "Contents/Library/LoginItems/SDTallyUpdater.app").path)
    }

    static var estado: Estado {
        guard disponivel else { return .indisponivel }
        return switch servico.status {
        case .enabled: .ligado
        case .requiresApproval: .aguardandoAprovacao
        default: .desligado
        }
    }

    struct Erro: LocalizedError {
        let errorDescription: String?
    }

    /// Registra o item de início; o macOS o abre na hora e a cada início de sessão. Desligado em
    /// Itens de Início, fica `.aguardandoAprovacao` (a UI abre Itens de Início).
    static func ligar() throws {
        guard disponivel else { throw Erro(errorDescription: String(localized: "O atualizador só funciona no SDTally.app.")) }
        UserDefaults.standard.set(true, forKey: chave)
        do {
            try servico.register()
        } catch {
            // Como no Root sem Senha: o erro vem também em "aguardando aprovação"; decide pelo status.
            guard [.enabled, .requiresApproval].contains(servico.status) else {
                UserDefaults.standard.set(false, forKey: chave)
                throw Erro(errorDescription: String(localized: "Não deu para ligar o atualizador: \(error.localizedDescription)"))
            }
        }
    }

    static func desligar() throws {
        UserDefaults.standard.set(false, forKey: chave)
        guard disponivel else { return }
        for a in NSRunningApplication.runningApplications(withBundleIdentifier: id) { a.terminate() }
        guard servico.status != .notRegistered else { return }
        do {
            try servico.unregister()
        } catch {
            if servico.status != .notRegistered {
                throw Erro(errorDescription: String(localized: "Não deu para desligar o atualizador: \(error.localizedDescription)"))
            }
        }
    }

    /// Ao abrir o app: ligado pela pessoa e sem registro (bundle trocado à mão, por exemplo), registra
    /// de novo. Já registrado, não mexe: unregister + register tiraria o item dos Itens de Início.
    static func aoAbrir() {
        guard UserDefaults.standard.bool(forKey: chave), estado == .desligado else { return }
        try? servico.register()
    }
}

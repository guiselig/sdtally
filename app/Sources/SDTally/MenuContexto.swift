// Menu de botão direito no ícone da Dock (Delegado.applicationDockMenu) e no ícone da barra de
// menus (BarraDeStatus: um monitor local pega o clique direito na janela do ícone).
//   itensContexto(barra:medindo:podeParar:soAuto:)   os itens de cada um, testados (MenuContextoTests)
//   MenuContexto.menu(barra:), MenuContexto.vigiarBarra()
import AppKit

nonisolated enum ItemContexto: Equatable {
    case abrir, novaJanela, medir, parar(ativo: Bool), configuracoes, separador, sair
}

/// Dock: Nova janela e a medição (o macOS põe Opções, Mostrar e Sair). Barra: Abrir, a medição,
/// Configurações e Sair. A medição segue o botão principal; com o Automático com root não há
/// medição manual, e pelo osascript ela não para (Parar desligado).
nonisolated func itensContexto(barra: Bool, medindo: Bool, podeParar: Bool, soAuto: Bool) -> [ItemContexto] {
    var i: [ItemContexto] = [barra ? .abrir : .novaJanela]
    if medindo { i.append(.parar(ativo: podeParar)) } else if !soAuto { i.append(.medir) }
    if barra { i += [.configuracoes, .separador, .sair] }
    return i
}

enum MenuContexto {
    static func menu(barra: Bool) -> NSMenu {
        let p = Painel.unico
        let m = NSMenu()
        m.autoenablesItems = false
        for item in itensContexto(barra: barra, medindo: p.medindo, podeParar: p.podeParar, soAuto: p.soAuto) {
            switch item {
            case .abrir: m.addItem(novoItem(String(localized: "Abrir SDTally")) { p.mostrar(p.secao ?? .sistema) })
            case .novaJanela: m.addItem(novoItem(String(localized: "Nova janela")) { p.mostrar(p.secao ?? .sistema) })
            // Abre a janela em Dados do Sistema, onde fica o cronômetro.
            case .medir: m.addItem(novoItem(String(localized: "Medição completa")) { p.medirComSenha(); p.mostrar(.sistema) })
            case .parar(let ativo): m.addItem(novoItem(String(localized: "Parar medição"), ativo: ativo) { p.parar() })
            case .configuracoes: m.addItem(novoItem(String(localized: "Configurações…"), tecla: ",") { p.mostrar(.permissoes) })
            case .separador: m.addItem(.separator())
            case .sair: m.addItem(novoItem(String(localized: "Sair do SDTally"), tecla: "q") { NSApp.terminate(nil) })
            }
        }
        return m
    }

    /// Clique direito no ícone da barra (a janela NSStatusBarWindow do app): o menu no lugar do
    /// popover, que continua no clique esquerdo.
    static func vigiarBarra() {
        NSEvent.addLocalMonitorForEvents(matching: .rightMouseDown) { e in
            guard let w = e.window, w.className.contains("NSStatusBarWindow"), let v = w.contentView else { return e }
            NSMenu.popUpContextMenu(menu(barra: true), with: e, for: v)
            return nil
        }
    }
}

/// Alvo comum dos itens: a ação de cada um vai no representedObject (NSMenuItem não se subclassifica
/// com o isolamento padrão no MainActor).
private final class AlvoMenu: NSObject {
    static let unico = AlvoMenu()
    @objc func rodar(_ i: NSMenuItem) { (i.representedObject as? Acao)?.bloco() }
}

private final class Acao {
    let bloco: () -> Void
    init(_ bloco: @escaping () -> Void) { self.bloco = bloco }
}

/// `tecla`: o atalho com ⌘ mostrado à direita, como nos menus do sistema ("Settings… ⌘,").
private func novoItem(_ titulo: String, ativo: Bool = true, tecla: String = "", acao: @escaping () -> Void) -> NSMenuItem {
    let i = NSMenuItem(title: titulo, action: #selector(AlvoMenu.rodar(_:)), keyEquivalent: tecla)
    i.target = AlvoMenu.unico
    i.representedObject = Acao(acao)
    i.isEnabled = ativo
    return i
}

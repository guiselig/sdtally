// Ícone da barra de menus pelo AppKit: o rótulo (RotuloBarra, a pill) no botão do NSStatusItem e o
// cartão (BarraMenusView) dentro de um NSMenu de verdade, que o macOS 26 desenha com Liquid Glass,
// contorno e sombra, como os menus do sistema. O MenuBarExtra(.window) era uma janela de fundo
// sólido. Rótulo e cartão são peças separadas: uma não mexe na outra. Clique direito no ícone:
// MenuContexto.vigiarBarra.
import AppKit
import SwiftUI

final class BarraDeStatus: NSObject, NSMenuDelegate {
    static let unico = BarraDeStatus()
    private var item: NSStatusItem?
    private let menu = NSMenu()
    private let linha = NSMenuItem()

    override init() {
        super.init()
        menu.delegate = self
        menu.addItem(linha)
    }

    /// Põe ou tira o ícone conforme a chave barraDeMenus (Configurações).
    func ajustar() {
        let quer = UserDefaults.standard.bool(forKey: "barraDeMenus")
        if quer, item == nil {
            let i = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
            if let b = i.button {
                let rotulo = Rotulo(rootView: AnyView(RotuloBarra().environment(Painel.unico)
                    .font(Font(NSFont.menuBarFont(ofSize: 0)))))
                rotulo.item = i
                rotulo.translatesAutoresizingMaskIntoConstraints = false
                b.addSubview(rotulo)
                NSLayoutConstraint.activate([rotulo.centerXAnchor.constraint(equalTo: b.centerXAnchor),
                                             rotulo.centerYAnchor.constraint(equalTo: b.centerYAnchor)])
                b.target = self
                b.action = #selector(clicou)
            }
            item = i
        } else if !quer, let i = item {
            NSStatusBar.system.removeStatusItem(i)
            item = nil
        }
    }

    /// O menu preso ao item só durante o clique: assim o macOS o posiciona e realça o botão como os
    /// dele, e o clique direito continua livre para o MenuContexto.
    @objc private func clicou() {
        guard let i = item else { return }
        i.menu = menu
        i.button?.performClick(nil)
        i.menu = nil
    }

    /// Cartão novo a cada abertura (o realce do mouse começa limpo), na altura do conteúdo.
    func menuWillOpen(_ menu: NSMenu) {
        // Sem o anel de foco: o primeiro item clicável ganhava foco ao abrir e parecia selecionado.
        let cartao = Cartao(rootView: AnyView(BarraMenusView().environment(Painel.unico)
            .environment(\.fecharMenu, { [weak menu] in menu?.cancelTracking() })
            .focusEffectDisabled()))
        cartao.frame.size = cartao.fittingSize
        linha.view = cartao
        Painel.unico.atualizarSeVelho()
    }
}

/// Só desenha: o clique vai para o botão do item. A largura do item segue a do rótulo.
private final class Rotulo: NSHostingView<AnyView> {
    weak var item: NSStatusItem?
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override func layout() {
        super.layout()
        let w = ceil(fittingSize.width) + 8
        if let item, item.length != w { item.length = w }
    }
}

/// O cartão cresce com o conteúdo (leitura nova com o menu aberto): o NSMenu acompanha o quadro.
private final class Cartao: NSHostingView<AnyView> {
    override func invalidateIntrinsicContentSize() {
        super.invalidateIntrinsicContentSize()
        let t = fittingSize
        if frame.size != t { setFrameSize(t) }
    }
}

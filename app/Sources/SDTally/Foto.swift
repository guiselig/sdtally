// Ganchos de desenvolvimento e fotos sem tela (Mac bloqueado, CI). Todas as SDTALLY_* só valem
// com Dev.ativo (binário cru, fora do .app); dentro do .app, Foto.env é vazio.
//   SDTALLY_FOTO=/x.png     espera a leitura rápida, desenha a janela num PNG e sai
//   SDTALLY_SECAO=<s>       resumo, sistema, armazenamento (o Disco), ranking, fazer, historico, permissoes, info,
//                           boasvindas, novidades, barra, vazios, selo ou infointeira (as cinco últimas por
//                           Foto.renderizar; vazios: V1, V5 e V12 da medição carregada; selo: popovers
//                           do selo; armazenamentointeiro, fazerinteiro, historicointeiro e permissoesinteira: a tela sem cortar; infointeira: Informações sem cortar, até "Comandos"; sistemainteiro: Dados do
//                           Sistema sem cortar)
//   SDTALLY_PAGINA=<n>      página das boas-vindas (0 a 3) com SDTALLY_SECAO=boasvindas
//   SDTALLY_ANCORA=<id>     abre o Disco rolado até a seção com esse .id (livres, cuida, recursos, volumes,
//                           recuperacao, informativo, naomedido) ou Dados do Sistema (snapshots, reiniciar, partes)
//   SDTALLY_BARRA=<nível>   ajustes, macos, volumeData ou termo:<key>: nível inicial da barra que
//                           se desdobra (Foto.barra, lido pelo A9); também abre Dados do Sistema
//   SDTALLY_ESQUEMA=escuro  tema escuro (com SDTALLY_FOTO, claro é o padrão; sem ela, o do sistema)
//   SDTALLY_COMPLETO=/x.json  carrega uma medição completa salva em vez de medir; liga o Manual
//   SDTALLY_MODO=auto       com SDTALLY_COMPLETO, fica no Automático
//   SDTALLY_RAPIDO=/x.json  usa uma leitura rápida salva (--quick --json) em vez de rodar o sdtally
//   SDTALLY_PASTA=/caminho  abre o ranking já no detalhe dessa pasta
//   SDTALLY_PDF=/x.pdf      gera o relatório redigido (Relatorio.gerarPDF, pelo SDTALLY_BIN) e sai
// Sem tela, screencapture falha; layer.render desenha a janela mesmo assim.
import AppKit
import SwiftUI

/// Modo de desenvolvimento: binário cru, sem bundle id. Liga todas as SDTALLY_*.
nonisolated enum Dev {
    static let ativo = Bundle.main.bundleIdentifier == nil
}

/// Nível da barra que se desdobra (spec-fase6 §4.8; a tela é do A9). Fica aqui, estável, porque o
/// SDTALLY_BARRA precisa dele.
nonisolated enum NivelBarra: Hashable, Sendable {
    case ajustes, macos, volumeData, termo(String)
}

enum Foto {
    static let env = env(ProcessInfo.processInfo.environment, bundleID: Bundle.main.bundleIdentifier)

    /// As SDTALLY_* só passam sem bundle id (mesma regra do Dev.ativo); pura, para o teste.
    nonisolated static func env(_ env: [String: String], bundleID: String?) -> [String: String] {
        bundleID == nil ? env : [:]
    }

    static var pastaInicial: [String] { env["SDTALLY_PASTA"].map { [$0] } ?? [] }

    /// Leitura rápida salva (SDTALLY_RAPIDO): o Painel.atualizar usa no lugar do --quick.
    static var rapidoFixo: Data? { env["SDTALLY_RAPIDO"].flatMap { FileManager.default.contents(atPath: $0) } }

    /// Seção para rolar ao abrir Armazenamento (SDTALLY_ANCORA).
    static var ancora: String? { env["SDTALLY_ANCORA"] }

    /// Nível inicial da barra que se desdobra (SDTALLY_BARRA).
    static var barra: NivelBarra? { nivelBarra(env["SDTALLY_BARRA"]) }

    /// "ajustes", "macos", "volumeData" ou "termo:<key>"; o resto dá nil.
    nonisolated static func nivelBarra(_ s: String?) -> NivelBarra? {
        switch s {
        case "ajustes": .ajustes
        case "macos": .macos
        case "volumeData": .volumeData
        case let t? where t.hasPrefix("termo:") && t.count > 6: .termo(String(t.dropFirst(6)))
        default: nil
        }
    }

    static var secaoInicial: Secao {
        if env["SDTALLY_PASTA"] != nil { return .ranking }
        if env["SDTALLY_SECAO"] == nil, barra != nil || ["snapshots", "reiniciar", "partes"].contains(ancora) { return .sistema }
        if env["SDTALLY_SECAO"] == nil, ancora != nil { return .armazenamento }
        return switch env["SDTALLY_SECAO"] {
        case "sistema": .sistema
        case "armazenamento": .armazenamento
        case "ranking": .ranking
        case "fazer": .fazer
        case "historico": .historico
        case "permissoes": .permissoes
        case "info": .info
        // "Abrir em: Última tela" só aqui, na criação do Painel: o menu Ir e a notificação escolhem depois.
        case nil where UserDefaults.standard.string(forKey: "abrirEm") == "ultima":
            Secao(rawValue: UserDefaults.standard.string(forKey: "ultimaSecao") ?? "") ?? .sistema
        default: .sistema
        }
    }

    static var escuro: Bool { env["SDTALLY_ESQUEMA"] == "escuro" }

    /// Antes da primeira janela: tema e ganchos que saem sem abrir janela.
    static func aoIniciar() {
        guard Dev.ativo else { return }
        if env["SDTALLY_ESQUEMA"] != nil || env["SDTALLY_FOTO"] != nil { NSApp.appearance = NSAppearance(named: escuro ? .darkAqua : .aqua) }
    }

    static func preparar(_ painel: Painel) {
        guard let p = env["SDTALLY_COMPLETO"], let d = FileManager.default.contents(atPath: p) else { return }
        painel.completo = try? decodificar(Completo.self, d, kind: nil)
        if painel.completo != nil { painel.completoDados = d }
        // Medição carregada é a do Manual; SDTALLY_MODO=auto mostra o Automático (vazio sem o --auto).
        painel.manual = env["SDTALLY_MODO"] != "auto"
    }

    static func tirarSePedido(_ painel: Painel) async {
        let destino = env["SDTALLY_FOTO"], pdf = env["SDTALLY_PDF"]
        guard destino != nil || pdf != nil else { return }
        for _ in 0..<60 where painel.rapido == nil && painel.rapidoFalhou == nil {
            try? await Task.sleep(for: .milliseconds(250))
        }
        try? await Task.sleep(for: .seconds(1))  // animações de entrada
        if let pdf {
            // Como o toggle do exportar, que começa ligado: redigido.
            do { try await Relatorio.gerarPDF(painel, destino: URL(fileURLWithPath: pdf), redigir: true) } catch { exit(2) }
            if destino == nil { exit(0) }
        }
        guard let destino else { return }
        switch env["SDTALLY_SECAO"] {
        case "boasvindas":
            exit(await renderizar(BoasVindasView(pagina: Int(env["SDTALLY_PAGINA"] ?? "") ?? 0).environment(painel), destino: destino) ? 0 : 2)
        case "novidades":
            exit(await renderizar(NovidadesView(versao: "1.1"), destino: destino) ? 0 : 2)
        case "barra":
            exit(await renderizar(BarraMenusView().environment(painel).frame(width: 300), destino: destino) ? 0 : 2)
        case "vazios":
            exit(await renderizar(vazios(painel).frame(width: 620), destino: destino) ? 0 : 2)
        case "selo":
            exit(await renderizar(selos(painel).frame(width: 440), destino: destino) ? 0 : 2)
        case "sistemainteiro":
            exit(await renderizar(SistemaView().environment(painel).frame(width: JanelaView.tamanho.width, height: 4000), destino: destino) ? 0 : 2)
        case "armazenamentointeiro":
            exit(await renderizar(ArmazenamentoView().environment(painel).frame(width: JanelaView.tamanho.width, height: 5200), destino: destino) ? 0 : 2)
        case "fazerinteiro":
            exit(await renderizar(ConselhosView().environment(painel).frame(width: JanelaView.tamanho.width, height: 3600), destino: destino) ? 0 : 2)
        case "historicointeiro":
            exit(await renderizar(HistoricoView().environment(painel).frame(width: JanelaView.tamanho.width, height: 1500), destino: destino) ? 0 : 2)
        case "permissoesinteira":
            exit(await renderizar(PermissoesView().environment(painel).frame(width: JanelaView.tamanho.width, height: 1300), destino: destino) ? 0 : 2)
        case "infointeira":
            exit(await renderizar(InfoView().environment(painel).frame(width: 760, height: 1300), destino: destino) ? 0 : 2)
        default: break
        }
        for _ in 0..<20 where janelaPrincipal() == nil { try? await Task.sleep(for: .milliseconds(250)) }
        exit(fotografar(janelaPrincipal()?.contentView?.superview, destino) ? 0 : 2)
    }

    /// V1, V5 e V12 com os números da medição carregada (o rápido e o SDTALLY_COMPLETO).
    private static func vazios(_ p: Painel) -> some View {
        let c = p.completo
        let tela = p.telaBoa
        return VStack(alignment: .leading, spacing: 16) {
            VazioTela(vazio: .v1(ultimo: tela?.tela.systemData, hora: tela.flatMap { hora($0.tela.loggedAt) })) {
                Button(AcaoVazio.abrirArmazenamento.rotulo) {}
            }
            VazioLinha(vazio: .v5(usadoData: p.rapido?.dataVolume.used.bytes)) {
                Button(AcaoVazio.medir.rotulo) {}
            }
            if let c { VazioLinha(vazio: .v12(c, comSenha: p.medicoes.last { $0.completo.root }?.completo)) { Button(AcaoVazio.medidorRoot.rotulo) {} } }
        }
        .padding(24)
    }

    /// Os popovers do selo, um por estado que a medição carregada tem.
    private static func selos(_ p: Painel) -> some View {
        let macos = p.rapido?.macosVersion ?? p.completo?.macosVersion ?? ""
        var ms: [Medida] = [p.rapido?.dataVolume.used ?? p.completo?.dataVolume.used].compactMap(\.self)
        if let v = p.completo?.dataVolume {
            ms += v.terms.map { Medida(bytes: $0.bytes, state: $0.state, reason: $0.reason, source: $0.source) }
            ms.append(Medida(bytes: v.unexplained.bytes, state: v.unexplained.state, reason: nil, source: "difference"))
        }
        return VStack(alignment: .leading, spacing: 12) {
            ForEach(Array(ms.enumerated()), id: \.offset) { _, m in
                VStack(alignment: .leading, spacing: 6) {
                    Text(nomeFonte(m.source)).font(.caption).foregroundStyle(.tertiary)
                    SeloPopover(selo: selo(medida: m, catalogo: p.catalogo, macos: macos))
                }
                .cartao()
            }
        }
        .padding(16)
    }

    /// layer.render pega o texto do SwiftUI; cacheDisplay deixava o detalhe sem texto.
    private static func fotografar(_ v: NSView?, _ destino: String) -> Bool {
        guard let v, let camada = v.layer,
              let rep = v.bitmapImageRepForCachingDisplay(in: v.bounds),
              let ctx = NSGraphicsContext(bitmapImageRep: rep) else { return false }
        camada.render(in: ctx.cgContext)
        return (try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: destino))) != nil
    }

    /// A janela do `Window(id: "principal")`: o SwiftUI põe o id no começo do identifier.
    /// `canBecomeMain` deixa de fora a janela do ícone da barra de menus.
    static func janelaPrincipal() -> NSWindow? {
        NSApp.windows.first { $0.isVisible && $0.identifier?.rawValue.hasPrefix("principal") == true }
            ?? NSApp.windows.first { $0.isVisible && $0.canBecomeMain }
    }

    /// Desenha uma view que não vive na janela principal (folha, janela da barra de menus) numa
    /// janela própria sem borda e fotografa como a principal. ImageRenderer não desenha os
    /// controles do AppKit (botões saem como placeholder amarelo).
    static func renderizar(_ conteudo: some View, destino: String) async -> Bool {
        let host = NSHostingView(rootView: conteudo.background(Color(nsColor: .windowBackgroundColor)))
        host.frame.size = host.fittingSize
        let w = NSWindow(contentRect: host.frame, styleMask: .borderless, backing: .buffered, defer: false)
        w.contentView = host
        w.orderFrontRegardless()
        try? await Task.sleep(for: .seconds(1))
        return fotografar(host.superview, destino)
    }

}

// Barra de menus e avisos (spec-fase5 §2.5 e §4.2). Dono: A6. API estável:
//   BarraMenusView()   lê Painel do ambiente; 330 pt de largura; no padrão dos menus da Apple
//   RotuloBarra()      o livre, o liberado automaticamente ou só o glifo (chave rotuloBarra); triângulo com aviso
//   aviso(pontos:agora:ultimo:) -> Aviso?   pura; `ultimo` vazio = só a condição, sem histerese
import Charts
import SwiftUI

/// Aviso do ícone e da notificação. Crescimento leva a hora real do ponto de comparação.
nonisolated struct Aviso: Sendable, Equatable {
    enum Tipo: String, Sendable, CaseIterable { case crescimento, poucoEspaco = "pouco_espaco" }
    let tipo: Tipo
    /// Crescimento: Δ de (usado − purgeable). Pouco espaço: livre + purgeable.
    let bytes: Int64
    /// Crescimento: hora do ponto de comparação. Pouco espaço: nil.
    let desde: Date?
    /// Pouco espaço: (livre + purgeable) / total, de 0 a 1.
    let fracao: Double?

    /// Título e corpo da notificação (§5): só números, sem caminhos.
    func textos(agora: Date = .now) -> (titulo: String, corpo: String) {
        switch tipo {
        case .crescimento:
            let d = desde ?? agora
            let h = d.formatted(.dateTime.hour().minute().locale(localApp))
            let titulo = if Calendar.current.isDate(d, inSameDayAs: agora) {
                String(localized: "O disco usou mais \(gb1(bytes)) desde hoje às \(h).")
            } else if Calendar.current.isDate(d, inSameDayAs: agora.addingTimeInterval(-86_400)) {
                String(localized: "O disco usou mais \(gb1(bytes)) desde ontem às \(h).")
            } else {
                String(localized: "O disco usou mais \(gb1(bytes)) desde \(d.formatted(.dateTime.day(.twoDigits).month(.twoDigits).locale(localApp))) às \(h).")
            }
            return (titulo, String(localized: "Abra o SDTally para ver o que cresceu."))
        case .poucoEspaco:
            return (String(localized: "Restam \(Int(((fracao ?? 0) * 100).rounded(.down))) % do disco livres, contando o que o macOS libera sozinho."),
                    String(localized: "Veja em O que fazer o que dá para liberar."))
        }
    }
}

/// Pouco espaço: (livre + purgeable) / total < 10 % no último ponto. Crescimento: (usado − purgeable)
/// do último ponto menos o do ponto mais perto de 24 h, só entre 20 e 28 h atrás, ≥ 10 GB.
/// Histerese: um tipo avisado há menos de 24 h (`ultimo[tipo]`) não volta.
nonisolated func aviso(pontos: [Ponto], agora: Date, ultimo: [Aviso.Tipo: Date]) -> Aviso? {
    guard let atual = pontos.last else { return nil }
    let bloqueado = { (t: Aviso.Tipo) in ultimo[t].map { agora.timeIntervalSince($0) < 86_400 } ?? false }
    if !bloqueado(.poucoEspaco), let total = atual.total, total > 0, let livre = atual.livre {
        let sobra = livre + (atual.purgeable ?? 0)
        let f = Double(sobra) / Double(total)
        if f < 0.10 { return Aviso(tipo: .poucoEspaco, bytes: sobra, desde: nil, fracao: f) }
    }
    guard !bloqueado(.crescimento), let u = atual.usado else { return nil }
    var perto: (d: Date, valor: Int64)?
    // Do mais novo para o mais antigo, parando antes de 28 h: lê poucas datas.
    for p in pontos.dropLast().reversed() {
        guard let d = data(p.t) else { continue }
        let idade = agora.timeIntervalSince(d)
        if idade > 28 * 3600 { break }
        guard idade >= 20 * 3600, let pu = p.usado else { continue }
        if perto.map({ abs(idade - 86_400) < abs(agora.timeIntervalSince($0.d) - 86_400) }) ?? true {
            perto = (d, pu - (p.purgeable ?? 0))
        }
    }
    guard let perto else { return nil }
    let delta = u - (atual.purgeable ?? 0) - perto.valor
    return delta >= 10_000_000_000 ? Aviso(tipo: .crescimento, bytes: delta, desde: perto.d, fracao: nil) : nil
}

/// Dados do Sistema da janela da barra: a tela boa do Ajustes ou, sem ela, o último ponto com
/// `systemData`. `em` é o carimbo da leitura.
nonisolated func sistemaBarra(tela: Tela?, pontos: [Ponto]) -> (bytes: Int64, em: String)? {
    if let t = tela, let sd = t.systemData, let em = t.loggedAt { return (sd, em) }
    guard let p = pontos.last(where: { $0.systemData != nil }), let sd = p.systemData else { return nil }
    return (sd, p.telaEm ?? p.t)
}

/// GB com 1 casa ("12,4 GB"), como nos textos do §5.
nonisolated func gb1(_ bytes: Int64) -> String {
    (Double(bytes) / 1e9).formatted(.number.precision(.fractionLength(1)).locale(localApp)) + " GB"
}

// MARK: - Rótulo

/// O que o ícone da barra mostra ("O que mostrar", em Permissões e aparência): o livre (padrão),
/// o liberado automaticamente ou só o glifo. Com aviso, o triângulo vem antes. Dados do Sistema nunca vai aqui.
struct RotuloBarra: View {
    @Environment(Painel.self) private var painel
    @Environment(\.openWindow) private var abrirJanela
    @AppStorage("rotuloBarra") private var mostrar = "livre"

    @MainActor private static let glifo: NSImage = {
        let r = ImageRenderer(content: Glifo())
        r.scale = 2
        let img = r.cgImage.map { NSImage(cgImage: $0, size: NSSize(width: 16, height: 14)) } ?? NSImage()
        img.isTemplate = true
        return img
    }()

    var body: some View {
        HStack(spacing: 4) {
            if painel.avisoAtivo != nil {
                Image(systemName: "exclamationmark.triangle")
            } else if mostrar == "icone" {
                Image(nsImage: Self.glifo)
            }
            switch mostrar {
            case "icone": EmptyView()
            case "liberado": Text(painel.purgVivo?.bytes.map(gb) ?? "—").monospacedDigit()
            default: Text((painel.livre ?? painel.rapido?.capacity?.free).map(gb) ?? "—").monospacedDigit()
            }
        }
        .onAppear { painel.abrirJanela = abrirJanela }
    }
}

private struct Glifo: View {
    var body: some View {
        TallyShape().stroke(.black, style: StrokeStyle(lineWidth: 1.6, lineCap: .round)).frame(width: 16, height: 14)
    }
}

// MARK: - Janela

struct BarraMenusView: View {
    @Environment(Painel.self) private var painel
    @Environment(\.openWindow) private var abrirJanela
    /// true quando o fundo do sistema foi escondido e o vidro é nosso (VidroDaJanela).
    @State private var vidro = false

    // No padrão dos menus da Apple (Wi-Fi, Bluetooth): título em negrito, seções com cabeçalho
    // cinza, linhas com ícone redondo e valor à direita, separadores recuados, ação no fim.
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            sistema.padding(.horizontal, 14).padding(.top, 12).padding(.bottom, 8)
            if let t = painel.telaBoa {
                BarraView(segmentos: t.segmentos, altura: 6).padding(.horizontal, 14).padding(.bottom, 10)
            }
            separador
            cabecalho(String(localized: "Espaço"))
            LinhaMenu(simbolo: "internaldrive.fill", nome: String(localized: "Livre"),
                      valor: (painel.livre ?? painel.rapido?.capacity?.free).map(gb))
            LinhaMenu(simbolo: "arrow.3.trianglepath", nome: String(localized: "Liberado automaticamente"),
                      valor: painel.purgVivo?.bytes.map(gb))
            maisOcupam
            grafico
            if let m = maisCresceu {
                separador
                cabecalho(String(localized: "Mais cresceu desde a medição anterior"))
                LinhaMenu(simbolo: "arrow.up.right", nome: m.0, valor: "+" + gb1(m.1))
            }
            if let a = painel.avisoAtivo {
                separador
                Label(a.textos().titulo, systemImage: "exclamationmark.triangle.fill")
                    .foregroundStyle(Marca.aviso)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.horizontal, 14).padding(.vertical, 6)
            }
            separador
            AcaoMenu(titulo: String(localized: "Abrir SDTally…")) {
                abrirJanela(id: "principal")
                NSApp.activate()
            }
            .padding(.bottom, 6)
        }
        .frame(width: 330)
        .modifier(FundoMenu(vidro: vidro))
        .background(VidroDaJanela(vidro: $vidro))
        .onAppear { painel.atualizarSeVelho() }
    }

    /// As 5 causas que mais ocupam o disco (as mesmas de Dados do Sistema), do automático ao vivo
    /// ou da última Medição completa; sem os dois, o aviso da primeira passada. O clique abre o app
    /// no "Ver…" da causa.
    @ViewBuilder private var maisOcupam: some View {
        if let c = painel.autoVivo ?? painel.completo {
            let fichas = c.cards ?? painel.catalogo?.cards
            let cinco = causas(top: c.top, swap: c.swap, fichas: fichas, home: c.home).prefix(5)
            if !cinco.isEmpty {
                separador
                cabecalho(String(localized: "O que mais ocupa"))
                ForEach(Array(cinco)) { x in
                    LinhaMenu(simbolo: x.id == "swap" ? "memorychip" : termo(x.termo).simbolo, nome: x.nome, valor: gb(x.bytes)) {
                        abrirJanela(id: "principal")
                        NSApp.activate()
                        painel.ver(x)?.ir()
                    }
                }
            }
        } else {
            // A primeira passada do automático ainda corre: o mesmo aviso de Dados do Sistema (SistemaView).
            separador
            cabecalho(String(localized: "O que mais ocupa"))
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text("A primeira leitura automática leva cerca de 1 minuto.")
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.horizontal, 14).padding(.vertical, 4)
        }
    }

    private var separador: some View { Divider().padding(.horizontal, 14).padding(.vertical, 5) }

    private func cabecalho(_ t: String) -> some View {
        Text(t).fontWeight(.semibold).foregroundStyle(.secondary)
            .padding(.horizontal, 14).padding(.top, 4).padding(.bottom, 2)
    }

    @ViewBuilder private var sistema: some View {
        if let s = sistemaBarra(tela: painel.telaBoa?.tela, pontos: painel.pontos) {
            HStack(alignment: .firstTextBaseline) {
                Text("Dados do Sistema").font(.headline)
                Spacer()
                Text(gb(s.bytes)).font(.headline.monospacedDigit())
            }
        } else {
            VStack(alignment: .leading, spacing: 2) {
                Text("Dados do Sistema").font(.headline)
                Text(Vazio.v23.texto).font(.caption).foregroundStyle(.secondary)
            }
        }
    }

    /// (usado − purgeable) dos últimos 7 dias, sem eixos.
    @ViewBuilder private var grafico: some View {
        let corte = Date.now.addingTimeInterval(-7 * 86_400)
        let itens = painel.pontos.suffix(7 * 24 + 24).compactMap { p -> (Date, Double)? in
            guard let d = data(p.t), d >= corte, let u = p.usado else { return nil }
            return (d, Double(u - (p.purgeable ?? 0)) / 1e9)
        }
        if itens.count >= 2 {
            let v = itens.map(\.1)
            separador
            cabecalho(String(localized: "Usado, últimos 7 dias"))
                .help("Usado menos o que é liberado automaticamente")
            Chart(itens, id: \.0) { i in
                LineMark(x: .value("Hora", i.0), y: .value("GB", i.1)).foregroundStyle(.teal)
            }
            .chartYScale(domain: (v.min()! - 1)...(v.max()! + 1))
            .chartXAxis(.hidden)
            .chartYAxis(.hidden)
            .frame(height: 44)
            .padding(.horizontal, 14).padding(.vertical, 4)
            .accessibilityLabel("Espaço usado nos últimos 7 dias")
        }
    }

    /// A linha do `top` que mais cresceu no último "O que mudou".
    private var maisCresceu: (String, Int64)? {
        let m = painel.medicoes
        guard m.count >= 2, let mud = Historico.mudancas(de: m[m.count - 2].completo, para: m[m.count - 1].completo) else { return nil }
        let home = m[m.count - 1].completo.home
        return mud.compactMap { x -> (String, Int64)? in
            guard let l = x.linha, let depois = x.depois else { return nil }
            let d = depois - (x.antes ?? 0)
            return d > 0 ? (rotulo(l, home: home), d) : nil
        }
        .max { $0.1 < $1.1 }
    }
}

/// Linha de menu com ícone num círculo, nome e valor à direita (como as redes no menu Wi-Fi).
/// Com `acao`, vira item clicável com o realce arredondado de AcaoMenu.
private struct LinhaMenu: View {
    let simbolo: String
    let nome: String
    let valor: String?
    var acao: (() -> Void)?
    @State private var sobre = false

    var body: some View {
        if let acao {
            Button(action: acao) {
                conteudo
                    .background(RoundedRectangle(cornerRadius: 8).fill(sobre ? AnyShapeStyle(.quaternary) : AnyShapeStyle(.clear)).padding(.horizontal, 5))
                    .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .onHover { sobre = $0 }
        } else {
            conteudo
        }
    }

    private var conteudo: some View {
        HStack(spacing: 10) {
            Image(systemName: simbolo)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.primary)
                .frame(width: 26, height: 26)
                .background(Circle().fill(.quaternary))
                .accessibilityHidden(true)
            Text(nome).lineLimit(1).truncationMode(.middle)
            Spacer(minLength: 8)
            Text(valor ?? "—").monospacedDigit().foregroundStyle(.secondary)
        }
        .padding(.horizontal, 14).padding(.vertical, 4)
        .accessibilityElement(children: .combine)
    }
}

/// Item de ação no fim do menu: texto à esquerda, realce arredondado ao passar o mouse.
private struct AcaoMenu: View {
    let titulo: String
    let acao: () -> Void
    @State private var sobre = false

    var body: some View {
        Button(action: acao) {
            Text(titulo)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 9).padding(.vertical, 4)
                .background(RoundedRectangle(cornerRadius: 8).fill(sobre ? AnyShapeStyle(.quaternary) : AnyShapeStyle(.clear)))
                .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .onHover { sobre = $0 }
        .padding(.horizontal, 5)
    }
}

/// Vidro com cantos grandes, como os menus do sistema no macOS 26+, quando o fundo da janela é nosso.
/// O glassEffect não segue o ajuste "Liquid Glass" de Aparência; ele fica em NSGlassTintAmount
/// (domínio global, sem API pública), e o tom vem de uma camada do fundo da janela sobre o vidro.
private struct FundoMenu: ViewModifier {
    let vidro: Bool
    @AppStorage("NSGlassTintAmount") private var tom = 0.0

    func body(content: Content) -> some View {
        if vidro, #available(macOS 26, *) {
            let t = min(max(tom, 0), 1)
            content
                .background(RoundedRectangle(cornerRadius: raioMenu)
                    .fill(Color(nsColor: .windowBackgroundColor).opacity(0.85 * t)))
                // O conteúdo dentro do vidro (não ao lado dele): o SwiftUI aplica a vibrancy do texto.
                .glassEffect(.regular, in: .rect(cornerRadius: raioMenu, style: .continuous))
                // Sem o fundo padrão do MenuBarExtra: o vidro o refletiria (fosco) e a borda dele aparece.
                .containerBackground(.clear, for: .window)
        } else {
            content
        }
    }
}

private let raioMenu: CGFloat = 18

/// Deixa a janela do MenuBarExtra transparente e esconde o fundo que o sistema põe atrás do
/// conteúdo, para o vidro de FundoMenu ser o único. Se o fundo do sistema for ancestral do
/// conteúdo (não dá para esconder sem esconder tudo), só arredonda os cantos dele.
private struct VidroDaJanela: NSViewRepresentable {
    @Binding var vidro: Bool

    func makeNSView(context: Context) -> NSView { Vista(aoEntrar: { vidro = $0 }) }
    func updateNSView(_ v: NSView, context: Context) {}

    final class Vista: NSView {
        let aoEntrar: (Bool) -> Void
        init(aoEntrar: @escaping (Bool) -> Void) { self.aoEntrar = aoEntrar; super.init(frame: .zero) }
        required init?(coder: NSCoder) { nil }

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            guard #available(macOS 26, *), let w = window, let raiz = w.contentView?.superview ?? w.contentView else { return }
            var proprio = true
            func passar(_ v: NSView) {
                if let e = v as? NSVisualEffectView {
                    if isDescendant(of: e) {
                        e.maskImage = Self.mascara
                        proprio = false
                    } else {
                        e.isHidden = true
                    }
                }
                v.subviews.forEach(passar)
            }
            passar(raiz)
            if proprio {
                w.isOpaque = false
                w.backgroundColor = .clear
                // A sombra da janela segue o retângulo dela e traz um contorno de cantos pequenos.
                w.hasShadow = false
                self.proprio = true
                NotificationCenter.default.addObserver(self, selector: #selector(mudouTamanho), name: NSWindow.didResizeNotification, object: w)
                DispatchQueue.main.async { self.limpar() }
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { self.limpar() }
            }
            DispatchQueue.main.async { self.aoEntrar(proprio) }
        }

        private var proprio = false
        @objc private func mudouTamanho(_ n: Notification) { limpar() }

        // O SwiftUI refaz as camadas de fundo quando a janela muda de tamanho (os 5 chegam).
        override func layout() {
            super.layout()
            if proprio { limpar() }
        }

        /// O fundo padrão do MenuBarExtra: as vistas antes das nossas (desfoque CABackdropLayer,
        /// camada clareadora e a do Chameleon) e o fundo da moldura. containerBackground não as tira.
        private func limpar() {
            guard let w = window, let h = w.contentView else { return }
            (h.superview ?? h).layer?.backgroundColor = .clear
            for v in h.subviews {
                if String(describing: type(of: v)).hasPrefix("AppKitPlatformViewHost") || isDescendant(of: v) { break }
                v.isHidden = true
            }
            w.invalidateShadow()
        }

        static let mascara: NSImage = {
            let r = raioMenu, lado = 2 * r + 1
            let img = NSImage(size: NSSize(width: lado, height: lado), flipped: false) { rect in
                NSBezierPath(roundedRect: rect, xRadius: r, yRadius: r).fill()
                return true
            }
            img.capInsets = NSEdgeInsets(top: r, left: r, bottom: r, right: r)
            img.resizingMode = .stretch
            return img
        }()
    }
}

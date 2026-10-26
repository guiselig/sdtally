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
    }
}

private struct Glifo: View {
    var body: some View {
        TallyShape().stroke(.black, style: StrokeStyle(lineWidth: 1.6, lineCap: .round)).frame(width: 16, height: 14)
    }
}

// MARK: - Janela

/// Fecha o NSMenu do cartão antes de uma ação (BarraDeStatus); fora do menu (fotos), não faz nada.
extension EnvironmentValues {
    @Entry var fecharMenu: () -> Void = {}
}

struct BarraMenusView: View {
    @Environment(Painel.self) private var painel

    var body: some View {
        Group {
            if Portao.unico.liberado { lista } else { semAcesso }
        }
        .frame(width: 330)
    }

    /// Sem Acesso Total: só o aviso, o botão do Ajustes e o Sair.
    private var semAcesso: some View {
        VStack(alignment: .leading, spacing: 0) {
            Label("O SDTally precisa do Acesso Total ao Disco.", systemImage: "lock.fill")
                .font(.headline).fixedSize(horizontal: false, vertical: true)
                .padding(.horizontal, 14).padding(.top, 12).padding(.bottom, 8)
            separador
            AcaoMenu(titulo: String(localized: "Abrir Privacidade e Segurança")) { painel.abrirAcessoTotal() }
            separador
            AcaoMenu(titulo: String(localized: "Sair do SDTally"), tecla: "⌘Q") { NSApp.terminate(nil) }
            Color.clear.frame(height: 6)
        }
    }

    // No padrão dos menus da Apple (Wi-Fi, Bluetooth): título em negrito, seções com cabeçalho
    // cinza, linhas com ícone redondo e valor à direita, separadores recuados, ação no fim.
    private var lista: some View {
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
            AcaoMenu(titulo: String(localized: "Abrir SDTally…")) { painel.mostrar(painel.secao ?? .sistema) }
            // Os mesmos itens do menu de botão direito do ícone (MenuContexto), menos o Abrir.
            ForEach(Array(itensContexto(barra: true, medindo: painel.medindo, podeParar: painel.podeParar, soAuto: painel.soAuto).enumerated()), id: \.offset) { _, item in
                switch item {
                case .medir: AcaoMenu(titulo: String(localized: "Medição completa")) { painel.medirComSenha(); painel.mostrar(.sistema) }
                case .parar(let ativo): AcaoMenu(titulo: String(localized: "Parar medição")) { painel.parar() }.disabled(!ativo)
                case .configuracoes: AcaoMenu(titulo: String(localized: "Configurações…"), tecla: "⌘,") { painel.mostrar(.permissoes) }
                case .separador: separador
                case .sair: AcaoMenu(titulo: String(localized: "Sair do SDTally"), tecla: "⌘Q") { NSApp.terminate(nil) }
                case .abrir, .novaJanela: EmptyView()
                }
            }
            Color.clear.frame(height: 6)
        }
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
                // Linha da execução anterior, lida do disco ao abrir: diz de quando é.
                if painel.autoVivo != nil, let d = painel.idadeAuto {
                    Text(String(localized: "Última leitura automática: \(idade(d))"))
                        .font(.caption).foregroundStyle(.secondary)
                        .padding(.horizontal, 14).padding(.bottom, 2)
                }
                ForEach(Array(cinco)) { x in
                    LinhaMenu(simbolo: x.id == "swap" ? "memorychip" : termo(x.termo).simbolo, nome: x.nome, valor: gb(x.bytes)) {
                        painel.mostrar(painel.secao ?? .sistema)
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
    @Environment(\.fecharMenu) private var fecharMenu

    var body: some View {
        if let acao {
            Button { fecharMenu(); acao() } label: {
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

/// Item de ação no fim do menu, como num menu do sistema: texto à esquerda, atalho em cinza à
/// direita e, ao passar o mouse, o realce na cor de destaque com o texto branco.
private struct AcaoMenu: View {
    let titulo: String
    var tecla: String? = nil
    let acao: () -> Void
    @State private var sobre = false
    @Environment(\.isEnabled) private var ativo
    @Environment(\.fecharMenu) private var fecharMenu

    var body: some View {
        let realce = sobre && ativo
        Button { fecharMenu(); acao() } label: {
            HStack {
                Text(titulo)
                Spacer(minLength: 8)
                if let tecla { Text(tecla).foregroundStyle(realce ? AnyShapeStyle(.white.opacity(0.8)) : AnyShapeStyle(.secondary)) }
            }
            .foregroundStyle(realce ? AnyShapeStyle(.white) : AnyShapeStyle(ativo ? .primary : .tertiary))
            .padding(.horizontal, 9).padding(.vertical, 4)
            .background(RoundedRectangle(cornerRadius: 6).fill(realce ? AnyShapeStyle(Color.accentColor) : AnyShapeStyle(.clear)))
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .onHover { sobre = $0 }
        .padding(.horizontal, 5)
    }
}

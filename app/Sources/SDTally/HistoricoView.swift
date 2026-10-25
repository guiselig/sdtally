// Histórico (spec-fase5 §4.1): período e data; no Automático, Dados do Sistema, espaço usado e pastas;
// no Manual, "O que mudou desde a última vez" e as medições manuais.
//   HistoricoView()      lê Painel do ambiente
//   GraficoLinha         uma série no tempo, valor ao passar o mouse, zoom no tempo pela rolagem
//   zoomTempo(_:limite:fracao:delta:), eixoTempo(_:)   puras, testadas (HistoricoTests)
import AppKit
import Charts
import SwiftUI

/// O período (Hora a Ano, terminando na data escolhida) nos dois modos. Automático: Dados do Sistema
/// em cima e o espaço usado embaixo, com o mesmo zoom no tempo, e as pastas que mais mudaram.
/// Manual: o que mudou entre as duas últimas medições e as medições manuais do período.
struct HistoricoView: View {
    @Environment(Painel.self) private var painel
    @State private var periodo = PeriodoHistorico.semana
    @State private var dia = Date.now
    /// Trecho do período aproximado pela rolagem sobre um gráfico; nil = o período inteiro.
    @State private var janela: ClosedRange<Date>?
    private static let desligado = Historico.pastaLeitura(env: Foto.env, bundleID: Bundle.main.bundleIdentifier) == nil

    var body: some View {
        if painel.pontos.isEmpty && painel.pontosPastas.isEmpty && painel.medicoes.isEmpty {
            ContentUnavailableView {
                Label("Histórico", systemImage: "chart.xyaxis.line")
            } description: {
                Text(Self.desligado ? String(localized: "Histórico desligado fora do .app (desenvolvimento).")
                     : String(localized: "Nenhum ponto salvo ainda: o primeiro entra na próxima hora."))
            }
        } else {
            // Hoje: até agora. Outro dia: até o fim dele.
            let fim = Calendar.current.isDateInToday(dia) ? Date.now
                : Calendar.current.date(byAdding: .second, value: -1, to: Calendar.current.startOfDay(for: dia).addingTimeInterval(86_400)) ?? dia
            let inicio = periodo.inicio(fim)
            let noPeriodo = { (t: String) in data(t).map { $0 >= inicio && $0 <= fim } ?? false }
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    // Numa linha quando cabe; senão a data desce (línguas de rótulos longos).
                    ViewThatFits(in: .horizontal) {
                        HStack { seletorPeriodo; Spacer(minLength: 12); periodoInteiro; seletorData }
                        VStack(alignment: .leading, spacing: 10) { seletorPeriodo; HStack { seletorData; periodoInteiro } }
                    }
                    if painel.manual && !painel.soAuto {
                        OQueMudou(medicoes: painel.medicoes).cartao()
                        ListaMedicoes(medicoes: painel.medicoes.filter { noPeriodo($0.completo.generatedAt) }).cartao()
                    } else {
                        let pontos = painel.pontos.filter { noPeriodo($0.t) }
                        let sistema = pontos.compactMap { p in p.systemData.flatMap { b in data(p.t).map { (d: $0, gb: Double(b) / 1e9) } } }
                        let usado = pontos.compactMap { p in p.usado.flatMap { u in data(p.t).map { (d: $0, gb: Double(u - (p.purgeable ?? 0)) / 1e9) } } }
                        VStack(alignment: .leading, spacing: 10) {
                            Text("Dados do Sistema").font(.title3.weight(.semibold))
                                .help("Dados do Sistema como a tela de Armazenamento do Ajustes mostra, só nas horas em que ela foi lida. Role sobre o gráfico para aproximar ou afastar.")
                            if sistema.isEmpty {
                                Text("Nenhuma leitura da tela do Ajustes neste período.").foregroundStyle(.secondary)
                            } else {
                                GraficoLinha(itens: sistema, nome: String(localized: "Dados do Sistema"), cor: Marca.dados, faixas: true,
                                             limite: inicio...fim, janela: $janela).frame(height: 200)
                            }
                        }
                        .cartao()
                        VStack(alignment: .leading, spacing: 10) {
                            Text("Espaço usado").font(.title3.weight(.semibold))
                                .help("Usado no disco menos o que o macOS libera sozinho, uma vez por hora (diskutil e CacheDelete). Role sobre o gráfico para aproximar ou afastar.")
                            if usado.isEmpty {
                                Text("Nenhum ponto neste período.").foregroundStyle(.secondary)
                            } else {
                                GraficoLinha(itens: usado, nome: String(localized: "Usado − liberável"), cor: .teal, area: true,
                                             limite: inicio...fim, janela: $janela).frame(height: 200)
                            }
                        }
                        .cartao()
                        PastasNoPeriodo(amostras: painel.pontosPastas.filter { noPeriodo($0.t) }, limite: inicio...fim, janela: $janela)
                            .cartao()
                    }
                }
                .padding(24)
                .frame(maxWidth: 760)
                .frame(maxWidth: .infinity)
            }
            .onChange(of: periodo) { janela = nil }
            .onChange(of: dia) { janela = nil }
        }
    }
}

extension HistoricoView {
    private var seletorPeriodo: some View {
        Picker("Período", selection: $periodo) {
            ForEach(PeriodoHistorico.allCases) { Text($0.nome).tag($0) }
        }
        .pickerStyle(.segmented).labelsHidden().fixedSize()
    }

    /// Só com zoom: volta ao período inteiro.
    @ViewBuilder private var periodoInteiro: some View {
        if janela != nil { Button("Ver o período inteiro") { janela = nil } }
    }

    private var seletorData: some View {
        DatePicker("Até", selection: $dia, in: ...Date.now, displayedComponents: .date).fixedSize()
    }
}

/// Janela do Histórico, sempre terminando no `fim` escolhido. Mês é o mês de calendário para trás.
enum PeriodoHistorico: CaseIterable, Identifiable {
    case hora, dia, semana, mes, trintaDias, ano
    var id: Self { self }

    var nome: String {
        switch self {
        case .hora: String(localized: "Hora (período)", defaultValue: "Hora")  // "Hora" já é o eixo ("Time")
        case .dia: String(localized: "Dia")
        case .semana: String(localized: "Semana")
        case .mes: String(localized: "Mês")
        case .trintaDias: String(localized: "30 dias")
        case .ano: String(localized: "Ano")
        }
    }

    func inicio(_ fim: Date) -> Date {
        let c = Calendar.current
        return switch self {
        case .hora: fim.addingTimeInterval(-3600)
        case .dia: fim.addingTimeInterval(-86_400)
        case .semana: fim.addingTimeInterval(-7 * 86_400)
        case .mes: c.date(byAdding: .month, value: -1, to: fim) ?? fim.addingTimeInterval(-30 * 86_400)
        case .trintaDias: fim.addingTimeInterval(-30 * 86_400)
        case .ano: c.date(byAdding: .year, value: -1, to: fim) ?? fim.addingTimeInterval(-365 * 86_400)
        }
    }

}

/// Marcas do eixo X para um trecho de `s` segundos: de quanto em quanto e com que formato. Segue o
/// zoom, não o período escolhido.
nonisolated func eixoTempo(_ s: TimeInterval) -> (Calendar.Component, Int, Date.FormatStyle) {
    let hora = Date.FormatStyle.dateTime.hour().minute().locale(localApp)
    let dia = Date.FormatStyle.dateTime.day(.twoDigits).month(.twoDigits).locale(localApp)
    return switch s {
    case ..<(2 * 3600): (.minute, 15, hora)
    case ..<(12 * 3600): (.hour, 2, hora)
    case ..<(36 * 3600): (.hour, 6, hora)
    case ..<(10 * 86_400): (.day, 1, dia)
    case ..<(70 * 86_400): (.day, 7, dia)
    default: (.month, 1, .dateTime.month(.abbreviated).locale(localApp))
    }
}

/// Trecho novo depois de rolar `delta` com o ponteiro em `fracao` da largura (0 = começo, 1 = fim):
/// para cima aproxima, para baixo afasta, com o ponto sob o ponteiro parado. Não sai de `limite`
/// nem fica menor que 10 min.
nonisolated func zoomTempo(_ j: ClosedRange<Date>, limite: ClosedRange<Date>, fracao: Double, delta: Double) -> ClosedRange<Date> {
    let span = j.upperBound.timeIntervalSince(j.lowerBound)
    let maximo = limite.upperBound.timeIntervalSince(limite.lowerBound)
    let novo = min(max(span * exp(-delta * 0.02), min(600, maximo)), maximo)
    let f = min(max(fracao, 0), 1)
    var a = j.lowerBound.addingTimeInterval(span * f - novo * f)
    if a < limite.lowerBound { a = limite.lowerBound }
    if a.addingTimeInterval(novo) > limite.upperBound { a = limite.upperBound.addingTimeInterval(-novo) }
    return a...a.addingTimeInterval(novo)
}

// MARK: - Pastas

/// Pastas que mais mudaram no período, pela primeira e a última amostra em que cada uma aparece
/// (ausente numa hora não é zero: só não estava entre as maiores). Clicar mostra a linha dela.
private struct PastasNoPeriodo: View {
    let amostras: [PontoPastas]
    let limite: ClosedRange<Date>
    @Binding var janela: ClosedRange<Date>?
    @State private var escolhida: String?

    var body: some View {
        let lista = variacoesPastas(amostras)
        let padroes = Dictionary(uniqueKeysWithValues: lista.map { v in
            (v.id, padraoCrescimento(amostras.compactMap { a in a.pastas[v.id].flatMap { b in data(a.t).map { (d: $0, b: b) } } }))
        })
        VStack(alignment: .leading, spacing: 10) {
            Text("Pastas").font(.title3.weight(.semibold))
                .help("Uma amostra por hora das maiores pastas do Automático, guardada por 90 dias. Uma pasta só entra na conta quando aparece em duas amostras do período.")
            if lista.isEmpty {
                Text("Sem duas amostras de pastas neste período.").foregroundStyle(.secondary)
            } else {
                ForEach(lista) { v in
                    Button { escolhida = escolhida == v.id ? nil : v.id } label: { linha(v, padroes[v.id]) }
                        .buttonStyle(.plain)
                }
                if let p = escolhida {
                    let pontos = amostras.compactMap { a in a.pastas[p].flatMap { b in data(a.t).map { (d: $0, gb: Double(b) / 1e9) } } }
                    let nome = (p as NSString).abbreviatingWithTildeInPath
                    Text(nome).font(.caption).foregroundStyle(.secondary)
                        .lineLimit(1).truncationMode(.middle).padding(.top, 6)
                    if let c = padroes[p] {
                        Text(verbatim: "\(c.etiqueta): \(c.detalhe)").font(.caption).foregroundStyle(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    GraficoLinha(itens: pontos, nome: nome, cor: .teal, pontos: true, limite: limite, janela: $janela)
                        .frame(height: 160)
                } else {
                    Text("Clique numa pasta para ver a linha dela.").font(.caption).foregroundStyle(.secondary)
                }
            }
        }
    }

    private func linha(_ v: VariacaoPasta, _ c: Crescimento?) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Image(systemName: v.delta >= 0 ? "arrow.up.right" : "arrow.down.right")
                .foregroundStyle(.secondary).frame(width: 18).accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text((v.id as NSString).abbreviatingWithTildeInPath).lineLimit(1).truncationMode(.middle)
                HStack(spacing: 6) {
                    Text(verbatim: "\(gb(v.antes)) → \(gb(v.depois))").font(.caption).foregroundStyle(.secondary)
                    // O padrão na janela; o detalhe (inclinação e de quando a quando) no balão.
                    if let c {
                        Text(c.etiqueta).font(.caption2.weight(.semibold)).foregroundStyle(.secondary)
                            .padding(.horizontal, 6).padding(.vertical, 1)
                            .background(.quaternary, in: .capsule)
                            .help(c.detalhe)
                    }
                }
            }
            Spacer(minLength: 12)
            Text((v.delta >= 0 ? "+" : "−") + gb(abs(v.delta))).monospacedDigit().fontWeight(.semibold)
        }
        .padding(.vertical, 2)
        .background(escolhida == v.id ? AnyShapeStyle(.quaternary) : AnyShapeStyle(.clear), in: .rect(cornerRadius: 6))
        .contentShape(.rect)
        .accessibilityElement(children: .combine)
    }
}

/// Uma pasta entre a primeira e a última amostra em que aparece no período.
struct VariacaoPasta: Identifiable, Equatable {
    let id: String
    let antes, depois: Int64
    var delta: Int64 { depois - antes }
}

/// As 10 maiores variações (para mais ou para menos), de 10 MB ou mais, só de pastas que aparecem
/// em duas amostras ou mais.
func variacoesPastas(_ amostras: [PontoPastas]) -> [VariacaoPasta] {
    var primeiro: [String: Int64] = [:], ultimo: [String: Int64] = [:], vezes: [String: Int] = [:]
    for a in amostras {
        for (p, b) in a.pastas {
            if primeiro[p] == nil { primeiro[p] = b }
            ultimo[p] = b
            vezes[p, default: 0] += 1
        }
    }
    return primeiro.compactMap { p, a in
        guard (vezes[p] ?? 0) >= 2, let d = ultimo[p], abs(d - a) >= 10_000_000 else { return nil }
        return VariacaoPasta(id: p, antes: a, depois: d)
    }
    .sorted { abs($0.delta) > abs($1.delta) }
    .prefix(10).map { $0 }
}

// MARK: - Gráfico

/// Uma série no tempo: linha (com área embaixo, se `area`; com pontos, se `pontos`), eixo Y do
/// trecho visível, o valor da hora mais perto ao passar o mouse e zoom no tempo pela rolagem sobre a
/// área do gráfico. `janela` é dividida entre os gráficos da tela: o zoom vale para todos.
struct GraficoLinha: View {
    let itens: [(d: Date, gb: Double)]
    let nome: String
    let cor: Color
    var area = false
    var pontos = false
    /// Leituras soltas e que pulam (a tela do Ajustes): uma por faixa de tempo, em degraus, com o
    /// mínimo e o máximo da faixa numa barra clara (agruparLeituras).
    var faixas = false
    let limite: ClosedRange<Date>
    @Binding var janela: ClosedRange<Date>?
    @State private var selecao: Date?

    var body: some View {
        let dominio = janela ?? limite
        let visiveis = itens.filter { dominio.contains($0.d) }
        let vals = (visiveis.isEmpty ? itens : visiveis).map(\.gb)
        let piso = ((vals.min() ?? 0) * 0.95 / 10).rounded(.down) * 10
        let teto = ((vals.max() ?? 1) * 1.02 / 10).rounded(.up) * 10
        let eixo = eixoTempo(dominio.upperBound.timeIntervalSince(dominio.lowerBound))
        let grupos = faixas ? agruparLeituras(itens, dominio: dominio) : []
        let perto = { (s: Date, d: Date) in abs(d.timeIntervalSince(s)) }
        let sel: GrupoLeituras? = selecao.flatMap { s in
            faixas ? grupos.min { perto(s, $0.d) < perto(s, $1.d) }
                : visiveis.min { perto(s, $0.d) < perto(s, $1.d) }.map { GrupoLeituras(id: 0, d: $0.d, gb: $0.gb, min: $0.gb, max: $0.gb, trecho: 0) }
        }
        Chart {
            ForEach(grupos) { g in
                if g.min < g.max {
                    RectangleMark(x: .value("Hora", g.d), yStart: .value("GB", g.min), yEnd: .value("GB", g.max), width: 4)
                        .foregroundStyle(cor.opacity(0.3))
                }
                LineMark(x: .value("Hora", g.d), y: .value("GB", g.gb), series: .value("Trecho", g.trecho))
                    .interpolationMethod(.stepEnd)
                    .foregroundStyle(cor)
            }
            ForEach(faixas ? [] : itens, id: \.d) { i in
                if area {
                    AreaMark(x: .value("Hora", i.d), yStart: .value("GB", piso), yEnd: .value("GB", i.gb))
                        .foregroundStyle(cor.opacity(0.15))
                }
                LineMark(x: .value("Hora", i.d), y: .value("GB", i.gb)).foregroundStyle(cor)
                if pontos {
                    PointMark(x: .value("Hora", i.d), y: .value("GB", i.gb)).foregroundStyle(cor).symbolSize(16)
                }
            }
            if let sel {
                RuleMark(x: .value("Hora", sel.d))
                    .foregroundStyle(.secondary)
                    // Dentro da área do gráfico: ela é recortada (chartPlotStyle), e o cartão em cima dela sumia.
                    .annotation(position: .top, spacing: 4, overflowResolution: .init(x: .fit(to: .plot), y: .fit(to: .plot))) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(sel.d.formatted(.dateTime.day(.twoDigits).month(.twoDigits).hour().minute().locale(localApp)))
                                .font(.caption.weight(.semibold))
                            Text(verbatim: "\(nome): \(gb(Int64(sel.gb * 1e9)))")
                            if sel.min < sel.max {
                                Text("entre \(gb(Int64(sel.min * 1e9))) e \(gb(Int64(sel.max * 1e9))) nesta faixa").foregroundStyle(.secondary)
                            }
                        }
                        .font(.caption.monospacedDigit())
                        .padding(6)
                        .background(.background.secondary, in: .rect(cornerRadius: 6))
                    }
            }
        }
        .chartYScale(domain: piso...max(teto, piso + 10))
        .chartYAxis {
            AxisMarks { v in
                AxisGridLine()
                AxisValueLabel { if let g = v.as(Double.self) { Text("\(Int(g)) GB") } }
            }
        }
        .chartXScale(domain: dominio)
        .chartXAxis {
            AxisMarks(values: .stride(by: eixo.0, count: eixo.1)) { _ in
                AxisGridLine()
                AxisValueLabel(format: eixo.2)
            }
        }
        // Com zoom, as marcas fora do trecho não passam da área do gráfico.
        .chartPlotStyle { $0.clipped() }
        .chartXSelection(value: $selecao)
        .chartOverlay { proxy in
            GeometryReader { g in
                if let f = proxy.plotFrame {
                    let r = g[f]
                    RolagemZoom { fracao, delta in
                        janela = zoomTempo(dominio, limite: limite, fracao: fracao, delta: delta)
                    }
                    .allowsHitTesting(false)
                    .frame(width: r.width, height: r.height)
                    .offset(x: r.minX, y: r.minY)
                }
            }
        }
        .environment(\.locale, localApp)
    }
}

/// Leituras de uma faixa de tempo: a última (`d`, `gb`), o mínimo e o máximo, e o trecho da linha.
nonisolated struct GrupoLeituras: Identifiable, Equatable {
    let id: Int
    let d: Date
    let gb, min, max: Double
    let trecho: Int
}

/// Leituras juntas por faixa (`partes` faixas iguais no trecho visível): a última de cada uma, com o
/// mínimo e o máximo. Um vão sem leitura maior que 6 faixas e que 2 h começa outro trecho: a linha
/// não liga leituras distantes como se o valor tivesse andado entre elas.
nonisolated func agruparLeituras(_ itens: [(d: Date, gb: Double)], dominio: ClosedRange<Date>, partes: Int = 80) -> [GrupoLeituras] {
    let span = dominio.upperBound.timeIntervalSince(dominio.lowerBound)
    guard span > 0, partes > 0 else { return [] }
    let w = span / Double(partes)
    var porFaixa: [Int: [(d: Date, gb: Double)]] = [:]
    for i in itens where dominio.contains(i.d) {
        porFaixa[Int(i.d.timeIntervalSince(dominio.lowerBound) / w), default: []].append(i)
    }
    var r: [GrupoLeituras] = []
    var trecho = 0
    var anterior: Date?
    for k in porFaixa.keys.sorted() {
        let v = porFaixa[k]!.sorted { $0.d < $1.d }
        if let a = anterior, v[0].d.timeIntervalSince(a) > Swift.max(6 * w, 7200) { trecho += 1 }
        let gbs = v.map(\.gb)
        r.append(GrupoLeituras(id: k, d: v[v.count - 1].d, gb: v[v.count - 1].gb, min: gbs.min()!, max: gbs.max()!, trecho: trecho))
        anterior = v[v.count - 1].d
    }
    return r
}

/// Pega a rolagem que cai sobre ela (monitor local, sem receber cliques nem o hover do gráfico) e
/// passa a posição do ponteiro em fração da largura e o quanto rolou; a página não rola junto.
private struct RolagemZoom: NSViewRepresentable {
    let aoRolar: (_ fracao: Double, _ delta: Double) -> Void

    func makeNSView(context: Context) -> Vista { Vista() }
    func updateNSView(_ v: Vista, context: Context) { v.aoRolar = aoRolar }

    final class Vista: NSView {
        var aoRolar: ((Double, Double) -> Void)?
        private var monitor: Any?

        override func hitTest(_ p: NSPoint) -> NSView? { nil }

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            if let monitor { NSEvent.removeMonitor(monitor) }
            monitor = nil
            guard window != nil else { return }
            monitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self] e in
                guard let self, e.window === self.window, e.scrollingDeltaY != 0 else { return e }
                // Só a parte à vista: o pedaço escondido pela rolagem da página não conta.
                let r = self.convert(self.visibleRect, to: nil)
                guard r.width > 0, r.contains(e.locationInWindow) else { return e }
                let delta = e.hasPreciseScrollingDeltas ? e.scrollingDeltaY : e.scrollingDeltaY * 8
                let fracao = (e.locationInWindow.x - self.convert(self.bounds, to: nil).minX) / self.bounds.width
                self.aoRolar?(fracao, delta)
                return nil
            }
        }
    }
}

// MARK: - O que mudou

/// As duas medições mais novas: o que mudou 100 MB ou mais, ou por que não dá para comparar.
private struct OQueMudou: View {
    @Environment(Painel.self) private var painel
    let medicoes: [Salva]

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("O que mudou desde a última vez").font(.title3.weight(.semibold))
            if medicoes.count < 2 {
                Text("Falta uma segunda medição").foregroundStyle(.secondary)
            } else {
                let a = medicoes[medicoes.count - 2].completo, b = medicoes[medicoes.count - 1].completo
                Text("De \(dia(a.generatedAt)) para \(dia(b.generatedAt))")
                    .font(.caption).foregroundStyle(.secondary)
                    .help("Diferenças de 100 MB ou mais")
                if let m = Historico.mudancas(de: a, para: b) {
                    if m.isEmpty { Text("Nada mudou 100 MB ou mais.").foregroundStyle(.secondary) }
                    ForEach(m) { linha($0, home: b.home) }
                } else if let motivo = Historico.motivo(a, b) {
                    Faixa(simbolo: "exclamationmark.triangle.fill", texto: motivo)
                }
            }
        }
    }

    private func linha(_ m: Mudanca, home: String) -> some View {
        let (detalhe, valor): (String, String) =
            if let antes = m.antes, let depois = m.depois {
                ("\(gb(antes)) → \(gb(depois))", (depois >= antes ? "+" : "−") + gb(abs(depois - antes)))
            } else if let depois = m.depois {
                (String(localized: "nova na lista"), gb(depois))
            } else {
                (String(localized: "saiu da lista"), String(localized: "era \(gb(m.antes ?? 0))"))
            }
        let simbolo = m.antes == nil ? "plus.circle" : m.depois == nil ? "minus.circle"
            : (m.depois ?? 0) >= (m.antes ?? 0) ? "arrow.up.right" : "arrow.down.right"
        return HStack(alignment: .firstTextBaseline, spacing: 10) {
            Image(systemName: simbolo).foregroundStyle(.secondary).frame(width: 18).accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text(m.linha.map { rotulo($0, home: home) } ?? nomeTermo(m.termo)).lineLimit(1).truncationMode(.middle)
                Text(detalhe).font(.caption).foregroundStyle(.secondary)
            }
            Spacer(minLength: 12)
            Text(valor).monospacedDigit().fontWeight(.semibold)
        }
        .padding(.vertical, 2)
        .accessibilityElement(children: .combine)
    }
}

/// "02/10 às 09:36" de um carimbo do sdtally.
private func dia(_ carimbo: String) -> String {
    guard let d = data(carimbo) else { return carimbo }
    return String(localized: "\(d.formatted(.dateTime.day(.twoDigits).month(.twoDigits).locale(localApp))) às \(hora(carimbo))")
}

// MARK: - Lista

/// Medições salvas, da mais nova para a mais antiga: data, modo, usado e bloqueado pelo macOS.
private struct ListaMedicoes: View {
    let medicoes: [Salva]

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Medições manuais").font(.title3.weight(.semibold))
            if medicoes.isEmpty {
                Text("Nenhuma medição neste período.").foregroundStyle(.secondary)
            } else {
                Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 8) {
                    GridRow {
                        Text("Medida em")
                        Text("Modo")
                        Text("Usado").gridColumnAlignment(.trailing)
                        Text("Bloqueado ou não medido").gridColumnAlignment(.trailing)
                        Text("sdtally")
                    }
                    .font(.caption).foregroundStyle(.secondary)
                    ForEach(medicoes.reversed()) { s in
                        let c = s.completo, u = c.dataVolume.unexplained
                        Divider()
                        GridRow {
                            Text(dia(c.generatedAt))
                            Text(ModoMedicao.nome(s.modo, root: c.root))
                                .font(.caption)
                                .padding(.horizontal, 6).padding(.vertical, 2)
                                .background(.quaternary, in: .capsule)
                            Text(c.dataVolume.used.bytes.map(gb) ?? "—")
                            Text((u.bytes.map(gb) ?? "—") + (u.percentOfUsed.map { " · " + $0.formatted(.number.precision(.fractionLength(1)).locale(localApp)) + " %" } ?? ""))
                                .help(nomeResto(c))
                            Text(c.tool.replacing("sdtally ", with: "")).font(.caption).foregroundStyle(.secondary)
                        }
                        .monospacedDigit()
                    }
                }
            }
        }
    }
}

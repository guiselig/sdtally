// Histórico (spec-fase5 §4.1): período e data, gráfico, pastas, "O que mudou desde a última vez" e
// as medições manuais.
//   HistoricoView()      lê Painel do ambiente
//   GraficoHistorico     LineMark + AreaMark, linha picotada de Dados do Sistema, chartXSelection
import Charts
import SwiftUI

/// Uma página só, nos dois modos: o período (Hora a Ano, terminando na data escolhida), o espaço
/// usado, as pastas que mais mudaram e as medições manuais do período.
struct HistoricoView: View {
    @Environment(Painel.self) private var painel
    @State private var periodo = PeriodoHistorico.semana
    @State private var dia = Date.now
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
                        HStack { seletorPeriodo; Spacer(minLength: 12); seletorData }
                        VStack(alignment: .leading, spacing: 10) { seletorPeriodo; seletorData }
                    }
                    VStack(alignment: .leading, spacing: 10) {
                        Text("Espaço usado").font(.title3.weight(.semibold))
                            .help("Usado no disco menos o que o macOS libera sozinho, uma vez por hora (diskutil e CacheDelete). Os pontos são Dados do Sistema, só onde a tela do Ajustes foi lida.")
                        let pontos = painel.pontos.filter { noPeriodo($0.t) }
                        if pontos.isEmpty {
                            Text("Nenhum ponto neste período.").foregroundStyle(.secondary)
                        } else {
                            GraficoHistorico(pontos: pontos, inicio: inicio, fim: fim, periodo: periodo).frame(height: 240)
                        }
                    }
                    .cartao()
                    PastasNoPeriodo(amostras: painel.pontosPastas.filter { noPeriodo($0.t) }, inicio: inicio, fim: fim, periodo: periodo)
                        .cartao()
                    OQueMudou(medicoes: painel.medicoes).cartao()
                    ListaMedicoes(medicoes: painel.medicoes.filter { noPeriodo($0.completo.generatedAt) }).cartao()
                }
                .padding(24)
                .frame(maxWidth: 760)
                .frame(maxWidth: .infinity)
            }
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

    /// Marcas do eixo X: de quanto em quanto e com que formato.
    var passo: (Calendar.Component, Int) {
        switch self {
        case .hora: (.minute, 15)
        case .dia: (.hour, 4)
        case .semana: (.day, 1)
        case .mes, .trintaDias: (.day, 7)
        case .ano: (.month, 1)
        }
    }

    var formato: Date.FormatStyle {
        switch self {
        case .hora, .dia: .dateTime.hour().minute().locale(localApp)
        case .semana, .mes, .trintaDias: .dateTime.day(.twoDigits).month(.twoDigits).locale(localApp)
        case .ano: .dateTime.month(.abbreviated).locale(localApp)
        }
    }
}

// MARK: - Pastas

/// Pastas que mais mudaram no período, pela primeira e a última amostra em que cada uma aparece
/// (ausente numa hora não é zero: só não estava entre as maiores). Clicar mostra a linha dela.
private struct PastasNoPeriodo: View {
    let amostras: [PontoPastas]
    let inicio, fim: Date
    let periodo: PeriodoHistorico
    @State private var escolhida: String?

    var body: some View {
        let lista = variacoesPastas(amostras)
        VStack(alignment: .leading, spacing: 10) {
            Text("Pastas").font(.title3.weight(.semibold))
                .help("Uma amostra por hora das maiores pastas do Automático, guardada por 90 dias. Uma pasta só entra na conta quando aparece em duas amostras do período.")
            if lista.isEmpty {
                Text("Sem duas amostras de pastas neste período.").foregroundStyle(.secondary)
            } else {
                ForEach(lista) { v in
                    Button { escolhida = escolhida == v.id ? nil : v.id } label: { linha(v) }
                        .buttonStyle(.plain)
                }
                if let p = escolhida {
                    let pontos = amostras.compactMap { a in a.pastas[p].flatMap { b in data(a.t).map { ($0, Double(b) / 1e9) } } }
                    Text((p as NSString).abbreviatingWithTildeInPath).font(.caption).foregroundStyle(.secondary)
                        .lineLimit(1).truncationMode(.middle).padding(.top, 6)
                    Chart(pontos, id: \.0) { d, g in
                        LineMark(x: .value("Hora", d), y: .value("GB", g)).foregroundStyle(Color.teal)
                        PointMark(x: .value("Hora", d), y: .value("GB", g)).foregroundStyle(Color.teal).symbolSize(20)
                    }
                    .chartXScale(domain: inicio...fim)
                    .chartYAxis { AxisMarks { v in AxisGridLine(); AxisValueLabel { if let g = v.as(Double.self) { Text(gb(Int64(g * 1e9))) } } } }
                    .chartXAxis { AxisMarks(values: .stride(by: periodo.passo.0, count: periodo.passo.1)) { _ in AxisGridLine(); AxisValueLabel(format: periodo.formato) } }
                    .frame(height: 160)
                    .environment(\.locale, localApp)
                } else {
                    Text("Clique numa pasta para ver a linha dela.").font(.caption).foregroundStyle(.secondary)
                }
            }
        }
    }

    private func linha(_ v: VariacaoPasta) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Image(systemName: v.delta >= 0 ? "arrow.up.right" : "arrow.down.right")
                .foregroundStyle(.secondary).frame(width: 18).accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text((v.id as NSString).abbreviatingWithTildeInPath).lineLimit(1).truncationMode(.middle)
                Text(verbatim: "\(gb(v.antes)) → \(gb(v.depois))").font(.caption).foregroundStyle(.secondary)
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

/// Linha e área de (usado − purgeable); Dados do Sistema só como ponto, onde houve tela (sem
/// interpolar). Passar o mouse mostra os valores da hora mais próxima.
struct GraficoHistorico: View {
    let pontos: [Ponto]
    let inicio, fim: Date
    let periodo: PeriodoHistorico
    @State private var selecao: Date?

    private struct Item {
        let d: Date
        let usado: Double
        let sistema: Double?
    }

    private static let usado = String(localized: "Usado − liberável"), sistema = String(localized: "Dados do Sistema")

    var body: some View {
        let itens = pontos.compactMap { p -> Item? in
            guard let d = data(p.t), let u = p.usado else { return nil }
            return Item(d: d, usado: Double(u - (p.purgeable ?? 0)) / 1e9, sistema: p.systemData.map { Double($0) / 1e9 })
        }
        let valores = itens.flatMap { [$0.usado] + ($0.sistema.map { [$0] } ?? []) }
        let piso = ((valores.min() ?? 0) * 0.95 / 10).rounded(.down) * 10
        let teto = ((valores.max() ?? 1) * 1.02 / 10).rounded(.up) * 10
        let sel = selecao.flatMap { s in itens.min { abs($0.d.timeIntervalSince(s)) < abs($1.d.timeIntervalSince(s)) } }
        Chart {
            ForEach(itens, id: \.d) { i in
                AreaMark(x: .value("Hora", i.d), yStart: .value("GB", piso), yEnd: .value("GB", i.usado))
                    .foregroundStyle(Color.teal.opacity(0.15))
                LineMark(x: .value("Hora", i.d), y: .value("GB", i.usado))
                    .foregroundStyle(by: .value("Série", Self.usado))
                if let s = i.sistema {
                    // Linha picotada: só liga as leituras da tela, não é medida contínua como o usado.
                    LineMark(x: .value("Hora", i.d), y: .value("GB", s))
                        .foregroundStyle(by: .value("Série", Self.sistema))
                        .lineStyle(StrokeStyle(lineWidth: 1.5, dash: [4, 3]))
                }
            }
            if let sel {
                RuleMark(x: .value("Hora", sel.d))
                    .foregroundStyle(.secondary)
                    .annotation(position: .top, spacing: 4, overflowResolution: .init(x: .fit(to: .chart), y: .disabled)) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(sel.d.formatted(.dateTime.day(.twoDigits).month(.twoDigits).hour().minute().locale(localApp)))
                                .font(.caption.weight(.semibold))
                            Text("\(Self.usado): \(gb(Int64(sel.usado * 1e9)))")
                            if let s = sel.sistema { Text("\(Self.sistema): \(gb(Int64(s * 1e9)))") }
                        }
                        .font(.caption.monospacedDigit())
                        .padding(6)
                        .background(.background.secondary, in: .rect(cornerRadius: 6))
                    }
            }
        }
        .chartForegroundStyleScale([Self.usado: Color.teal, Self.sistema: Marca.dados])
        .chartYScale(domain: piso...max(teto, piso + 10))
        .chartYAxis {
            AxisMarks { v in
                AxisGridLine()
                AxisValueLabel { if let g = v.as(Double.self) { Text("\(Int(g)) GB") } }
            }
        }
        .chartXScale(domain: inicio...fim)
        .chartXAxis {
            AxisMarks(values: .stride(by: periodo.passo.0, count: periodo.passo.1)) { _ in
                AxisGridLine()
                AxisValueLabel(format: periodo.formato)
            }
        }
        .chartXSelection(value: $selecao)
        .chartLegend(position: .bottom, alignment: .leading)
        .environment(\.locale, localApp)
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
                        Text("Bloqueado pelo macOS").gridColumnAlignment(.trailing)
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
                            Text(c.tool.replacing("sdtally ", with: "")).font(.caption).foregroundStyle(.secondary)
                        }
                        .monospacedDigit()
                    }
                }
            }
        }
    }
}

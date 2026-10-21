// Histórico (spec-fase5 §4.1): gráfico, "O que mudou desde a última vez" e a lista das medições.
//   HistoricoView()      lê Painel do ambiente
//   GraficoHistorico     LineMark + AreaMark, PointMark de Dados do Sistema, chartXSelection
import Charts
import SwiftUI

/// Automático: o gráfico dos pontos de hora em hora (sem senha). Manual: "O que mudou" e a lista
/// das Medições completas salvas.
struct HistoricoView: View {
    @Environment(Painel.self) private var painel
    private static let desligado = Historico.pastaLeitura(env: Foto.env, bundleID: Bundle.main.bundleIdentifier) == nil

    var body: some View {
        if painel.manual ? painel.medicoes.isEmpty : painel.pontos.isEmpty {
            ContentUnavailableView {
                Label("Histórico", systemImage: "chart.xyaxis.line")
            } description: {
                Text(Self.desligado ? String(localized: "Histórico desligado fora do .app (desenvolvimento).")
                     : painel.manual ? String(localized: "Nenhuma medição completa salva.") : String(localized: "Nenhum ponto salvo ainda: o primeiro entra na próxima hora."))
            } actions: {
                if painel.manual && !Self.desligado {
                    Button("Medir com senha") { painel.medirComSenha() }.disabled(painel.medindo)
                }
            }
        } else {
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    if !painel.manual {
                        VStack(alignment: .leading, spacing: 10) {
                            Text("Espaço usado").font(.title3.weight(.semibold))
                                .help("Usado no disco menos o que o macOS libera sozinho, uma vez por hora (diskutil e CacheDelete). Os pontos são Dados do Sistema, só onde a tela do Ajustes foi lida.")
                            GraficoHistorico(pontos: painel.pontos).frame(height: 240)
                        }
                        .cartao()
                    }
                    if painel.manual {
                        OQueMudou(medicoes: painel.medicoes).cartao()
                        ListaMedicoes(medicoes: painel.medicoes).cartao()
                    }
                }
                .padding(24)
                .frame(maxWidth: 760)
                .frame(maxWidth: .infinity)
            }
        }
    }
}

// MARK: - Gráfico

/// Linha e área de (usado − purgeable); Dados do Sistema só como ponto, onde houve tela (sem
/// interpolar). Passar o mouse mostra os valores da hora mais próxima.
struct GraficoHistorico: View {
    let pontos: [Ponto]
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
                    PointMark(x: .value("Hora", i.d), y: .value("GB", s))
                        .foregroundStyle(by: .value("Série", Self.sistema))
                        .symbolSize(50)
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
        .chartXAxis {
            AxisMarks(values: .stride(by: .day)) { _ in
                AxisGridLine()
                AxisValueLabel(format: .dateTime.day(.twoDigits).month(.twoDigits))
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
            Text("Medições salvas").font(.title3.weight(.semibold))
            if medicoes.isEmpty {
                Text("Nenhuma medição salva").foregroundStyle(.secondary)
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

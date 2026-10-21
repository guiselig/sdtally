// Snapshots, raio-X (spec-fase6 §2.2 item 10, §5.4; B3): substitui a lista simples. Dono: A10b.
// API estável:
//   SnapshotsView(c:)        a seção inteira (gráfico, tabela, presos, bloco do sistema, rodapé)
//   RaioX.presos(_:)         termo snapshots − Σ privados (fonte "difference")
//   RaioX.situacao(_:)       texto da coluna Situação pelo `verdict` (e `purgeable` no tm_reference)
//   RaioX.criadoPor(_:), RaioX.idade(_:geradoEm:)
import Charts
import SwiftUI

/// Contas e textos puros do raio-X. O app não refaz o veredito: só traduz o `verdict` do sdtally.
nonisolated enum RaioX {
    /// Presos em dois ou mais snapshots: termo `snapshots` − Σ privados. nil se o termo não foi
    /// medido, se algum privado é null ou se a diferença der negativa (leituras que não fecham).
    static func presos(_ c: Completo) -> Int64? {
        guard let t = c.dataVolume.terms.first(where: { $0.key == "snapshots" }), t.state == "measured", let termo = t.bytes,
              let s = c.informational?.snapshots else { return nil }
        let privados = s.map(\.privateBytes)
        guard !privados.contains(nil) else { return nil }
        let d = termo - privados.reduce(0) { $0 + ($1 ?? 0) }
        return d >= 0 ? d : nil
    }

    /// Rótulo curto da coluna Situação.
    static func situacao(_ s: Completo.Snapshot) -> String {
        switch s.verdict {
        case "tm_reference": s.purgeable == false ? String(localized: "Base do próximo backup · não removível") : String(localized: "Base do próximo backup")
        case "tm_stable": String(localized: "Estável")
        case "tm_recent": String(localized: "Recente (menos de 24 h)")
        case "tm_aged": String(localized: "Mais de 24 h")
        case "apple_other": String(localized: "Do macOS")
        case "third_party": String(localized: "De outro app")
        default: "—"
        }
    }

    /// O que a Situação quer dizer: tooltip da linha, não texto visível.
    static func situacaoDica(_ s: Completo.Snapshot) -> String {
        switch s.verdict {
        case "tm_reference":
            switch s.purgeable {
            case true?: String(localized: "Se faltar espaço, o macOS pode apagá-lo; o próximo backup demora mais.")
            case false?: String(localized: "Marcado como não removível.")
            case nil: ""
            }
        case "tm_stable": String(localized: "Marcado como estável pelo Time Machine.")
        case "tm_aged": String(localized: "O Time Machine costuma removê-los em 24 h (regra da Apple, não medição).")
        case "apple_other": String(localized: "Do macOS (com.apple), fora do Time Machine, no volume Data.")
        case "third_party": String(localized: "Só o app que criou gerencia.")
        default: ""
        }
    }

    /// "Time Machine", "macOS (com.apple.os)" ou o criador cru (escapado); sem criador, pelo `kind`.
    static func criadoPor(_ s: Completo.Snapshot) -> String {
        switch s.creator {
        case "com.apple.TimeMachine"?: "Time Machine"
        case let c? where c.hasPrefix("com.apple."): "macOS (\(c))"
        case let c?: visivel(c)
        case nil: s.kind == "time_machine" ? "Time Machine" : s.kind == "apple_other" ? "macOS" : String(localized: "Outro app")
        }
    }

    /// Idade = `generated_at` − `date` (§4.4). Sem data (fora do TM, ou TM sem data): "—".
    static func idade(_ s: Completo.Snapshot, geradoEm: String) -> String {
        guard s.kind == "time_machine", let d = data(s.date), let g = data(geradoEm) else { return "—" }
        let h = Int(g.timeIntervalSince(d) / 3600)
        if h < 1 { return String(localized: "menos de 1 h") }
        if h < 48 { return String(localized: "\(h) h") }
        return String(localized: "\(h / 24) dias")
    }

    /// Rótulo do eixo do gráfico: data curta no TM; nos outros, curto (o criador está na legenda).
    static func rotulo(_ s: Completo.Snapshot) -> String {
        guard s.kind == "time_machine", let d = s.date, d.count >= 16 else { return s.kind == "apple_other" ? "macOS" : s.kind == "time_machine" ? "Time Machine" : String(localized: "outro app") }
        let a = Array(d)
        return "\(String(a[8..<10]))/\(String(a[5..<7])) \(hora(d))"
    }
}

struct SnapshotsView: View {
    @Environment(Painel.self) private var painel
    let c: Completo

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Snapshots, raio-X").font(.headline)
                .help("O snapshot do sistema em uso é o macOS que está rodando, no volume System: entra na linha macOS do Ajustes, não em Dados do Sistema.")
            switch c.informational?.snapshots {
            case nil:
                let m = c.stageErrors.first { $0.stage == "snapshots" }?.error ?? String(localized: "a etapa de snapshots não rodou")
                VazioLinha(vazio: .v7(motivo: m)) {
                    Button(AcaoVazio.medirDeNovo.rotulo) { painel.medir(admin: c.root) }.disabled(painel.medindo)
                }
            case let s? where s.isEmpty:
                VazioLinha(.v6(hora: painel.manual ? (hora(c.generatedAt) as String) : nil))
            case let s?:
                grafico(s)
                tabela(s)
                Divider()
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Text("Presos em dois ou mais snapshots:")
                    NumeroView(bytes: RaioX.presos(c), source: "difference", state: RaioX.presos(c) == nil ? "not_measured" : "measured")
                }
                .help("Só saem quando todos saírem.")
            }
            // Só o plist ilegível: sem preferências é TM nunca configurado; outro motivo cru não aparece.
            if let tm = c.informational?.timeMachine, tm.state == "not_measured", tm.reason == "plist unreadable" {
                VazioLinha(.v16)
            }
            HStack {
                if let u = ajustesPermitidos["time_machine"] {
                    Button("Abrir Time Machine no Ajustes") { NSWorkspace.shared.open(u) }.buttonStyle(.link)
                }
                Spacer()
                Button("Copiar tmutil listlocalsnapshots /") {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString("tmutil listlocalsnapshots /", forType: .string)
                }
                .controlSize(.small)
                .help("A lista oficial do macOS, para rodar no Terminal")
            }
        }
        .cartao()
    }

    private func grafico(_ s: [Completo.Snapshot]) -> some View {
        let pontos = Array(s.enumerated())
        return Chart(pontos, id: \.offset) { i, x in
            BarMark(x: .value("Snapshot", String(i)), y: .value("Libera se apagar só este", Double(x.privateBytes ?? 0)))
                .foregroundStyle(by: .value("Criado por", RaioX.criadoPor(x)))
                .cornerRadius(3)
        }
        .chartXAxis {
            AxisMarks { v in
                AxisValueLabel { Text(v.as(String.self).flatMap(Int.init).map { RaioX.rotulo(s[$0]) } ?? "").font(.caption2) }
            }
        }
        .chartYAxis {
            AxisMarks { v in
                AxisGridLine()
                AxisValueLabel { Text(v.as(Double.self).map { gb(Int64($0)) } ?? "") }
            }
        }
        .chartLegend(position: .top, alignment: .leading)
        .frame(height: 160)
        .accessibilityHidden(true)  // a tabela abaixo tem os mesmos números
    }

    private func tabela(_ s: [Completo.Snapshot]) -> some View {
        Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 6) {
            GridRow {
                Text("Criado por")
                Text("Idade")
                Text("Libera se apagar só este").gridColumnAlignment(.trailing)
                Text("Cumulativo").gridColumnAlignment(.trailing)
                Text("Situação")
            }
            .font(.caption).foregroundStyle(.secondary)
            ForEach(Array(s.enumerated()), id: \.offset) { _, x in
                Divider()
                GridRow(alignment: .firstTextBaseline) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(RaioX.criadoPor(x))
                        if let d = x.date { Text(Armazenamento.dataCurta(d)).font(.caption).foregroundStyle(.secondary) }
                    }
                    Text(RaioX.idade(x, geradoEm: c.generatedAt)).monospacedDigit()
                    NumeroView(bytes: x.privateBytes, source: "fsctl_snapshot_private_size", state: x.privateBytes == nil ? "not_measured" : "measured")
                    if let b = x.cumulativeBytes { NumeroView(bytes: b, source: "fsctl_snapshot_private_size") } else { Text("—") }
                    Text(RaioX.situacao(x)).font(.caption).foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                        .help(RaioX.situacaoDica(x))
                }
            }
        }
    }
}

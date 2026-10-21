// Todo número do app passa por aqui: valor, fonte ao lado e o estado (nunca 0 no lugar de
// "não medido"). Clicar na fonte mostra a chave bruta, o motivo e os bytes exatos.
// Fase 6 (F6, spec-fase6 §5.7): o selo ao lado da fonte diz se a fonte foi conferida contra a
// Apple. API estável:
//   selo(medida:catalogo:macos:) -> Selo   pura; catálogo de Painel.catalogo (--catalog --json)
//   Selo.estado (.conferido, .margem, .medido, .parcial, .naoMedido), .simbolo, .texto, .comando
//   comandosVerificar: [String]            igual ao catalogo::COMANDOS_VERIFICAR do Rust
//   SeloPopover(selo:)                     o conteúdo do popover (Reproduzir + Copiar comando)
import AppKit
import SwiftUI

/// Os `apple_command` que o app aceita mostrar e copiar (catalogo::COMANDOS_VERIFICAR).
nonisolated let comandosVerificar = [
    "diskutil apfs list -plist",
    "/usr/bin/log show --last 1h --predicate 'subsystem == \"com.apple.settings.Storage\"'",
    "df -k /System/Volumes/Data",
    "df -k /",
    "lsof -b -w +L1",
    "sysctl vm.swapusage",
    "sysctl kern.boottime",
    "diskutil apfs listSnapshots /System/Volumes/Data",
]

nonisolated struct Selo: Sendable, Equatable {
    enum Estado: Sendable, Equatable { case conferido, margem, medido, parcial, naoMedido }
    let estado: Estado
    let texto: String
    /// "Reproduzir: <comando>" e "Copiar comando": só com comando em `comandosVerificar`.
    let comando: String?

    var simbolo: String {
        switch estado {
        case .conferido: "checkmark.seal"
        case .margem: "checkmark.circle"
        case .medido: "circle"
        case .parcial: "circle.lefthalf.filled"
        case .naoMedido: "circle.dashed"
        }
    }
}

/// Selo de um número. Sem catálogo ou com fonte fora dele: "Medido". `macos`: versão da medição.
nonisolated func selo(medida m: Medida, catalogo: Catalogo?, macos: String) -> Selo {
    let conf = catalogo?.sources[m.source]
    let cmd = conf?.appleCommand.flatMap { comandosVerificar.contains($0) ? $0 : nil }
    switch m.state {
    case "not_measured": return Selo(estado: .naoMedido, texto: m.reason.map { String(localized: "Não medido: \($0).") } ?? String(localized: "Não medido."), comando: cmd)
    case "not_available": return Selo(estado: .naoMedido, texto: String(localized: "Não existe neste macOS."), comando: cmd)
    case "partial": return Selo(estado: .parcial, texto: m.source == "difference" ? String(localized: "Teto.") : String(localized: "Piso: só o que foi medido."), comando: cmd)
    default: break
    }
    guard let conf, ["byte", "margin"].contains(conf.checked) else {
        return Selo(estado: .medido, texto: String(localized: "Medido; sem comparação com a Apple."), comando: cmd)
    }
    let maior = String(macos.split(separator: ".").first ?? "")
    guard conf.checkedOn.contains(maior) else {
        let em = conf.checkedOn.joined(separator: ", ")
        return Selo(estado: .medido, texto: String(localized: "Medido. Neste macOS (\(macos)) ainda não conferido; conferido no \(em)."), comando: cmd)
    }
    if conf.checked == "margin" {
        return Selo(estado: .margem, texto: String(localized: "Bate com a Apple com diferença pequena e explicada."), comando: cmd)
    }
    // "2026-10-01" → "01/10"
    let dia = conf.checkedAt.flatMap { c -> String? in
        let a = Array(c)
        return a.count >= 10 ? "\(String(a[8..<10]))/\(String(a[5..<7]))" : nil
    }
    let texto = dia.map { String(localized: "Conferido ao byte pelo projeto contra a Apple (macOS \(maior), \($0)).") }
        ?? String(localized: "Conferido ao byte pelo projeto contra a Apple (macOS \(maior)).")
    return Selo(estado: .conferido, texto: texto, comando: cmd)
}

/// Conteúdo do popover do selo.
struct SeloPopover: View {
    let selo: Selo

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label(selo.texto, systemImage: selo.simbolo).foregroundStyle(.secondary)
            if let c = selo.comando {
                Text("Reproduzir: \(c)").font(.callout.monospaced()).textSelection(.enabled)
                Button("Copiar comando") {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(c, forType: .string)
                }
            }
        }
        .fixedSize(horizontal: false, vertical: true)
    }
}

struct NumeroView: View {
    let bytes: Int64?
    let state: String
    var reason: String?
    let source: String
    var quando: String?
    var estilo: Font = .body
    /// Só o selo, sem o texto da fonte: a lista inteira vem da mesma fonte, dita uma vez no cabeçalho.
    var compacto = false
    @State private var detalhe = false

    init(_ m: Medida, quando: String? = nil, estilo: Font = .body) {
        bytes = m.bytes; state = m.state; reason = m.reason; source = m.source
        self.quando = quando; self.estilo = estilo
    }

    init(bytes: Int64?, source: String, state: String = "measured", reason: String? = nil,
         quando: String? = nil, estilo: Font = .body, compacto: Bool = false) {
        self.bytes = bytes; self.state = state; self.reason = reason; self.source = source
        self.quando = quando; self.estilo = estilo; self.compacto = compacto
    }

    var body: some View {
        // A fonte antes do número: os números ficam alinhados na borda direita.
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            Button { detalhe.toggle() } label: {
                HStack(spacing: 3) {
                    Image(systemName: seloAtual.simbolo).accessibilityHidden(true)
                    if !compacto { Text(legenda) }
                }
                .font(.caption).foregroundStyle(.tertiary)
            }
            .buttonStyle(.plain)
            .help(legenda)
            .popover(isPresented: $detalhe, arrowEdge: .bottom) { popover }
            Text(valor)
                .font(estilo.monospacedDigit())
                .contentTransition(.numericText(value: Double(bytes ?? 0)))
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(valor), fonte: \(nomeFonte(source))")
    }

    private var seloAtual: Selo {
        let p = Painel.unico
        return selo(medida: Medida(bytes: bytes, state: state, reason: reason, source: source), catalogo: p.catalogo,
                    macos: p.rapido?.macosVersion ?? p.completo?.macosVersion ?? "")
    }

    /// Diferença parcial (usado − termos que são pisos) é um teto: o que falta nos termos está nela.
    private var teto: Bool { state == "partial" && source == "difference" }

    private var valor: String {
        guard let b = bytes else { return "—" }
        return (teto ? "≤ " : state == "partial" ? "≥ " : "") + gb(b)
    }

    private var legenda: String {
        var partes = [nomeFonte(source)]
        switch state {
        case "partial": partes.append(teto ? String(localized: "teto: inclui o que não foi medido") : String(localized: "parcial"))
        case "not_measured": partes.append(reason.map { String(localized: "não medido: \($0)") } ?? String(localized: "não medido"))
        case "not_available": partes.append(String(localized: "não existe neste macOS"))
        default: break
        }
        if let q = quando { partes.append(q) }
        return partes.joined(separator: " · ")
    }

    private var popover: some View {
        Form {
            LabeledContent("Fonte", value: source)
            LabeledContent("Estado", value: state)
            if let b = bytes { LabeledContent("Bytes", value: bytesExatos(b)) }
            if let r = reason { LabeledContent("Motivo", value: r) }
            if let q = quando { LabeledContent("Quando", value: q) }
            Section { SeloPopover(selo: seloAtual) }
        }
        .formStyle(.grouped)
        .frame(width: 360)
        .textSelection(.enabled)
    }
}

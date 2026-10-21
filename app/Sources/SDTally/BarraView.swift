// Barras: a do Ajustes, a da conta do volume e a mini-barra das tabelas. Só geometria: as
// larguras são proporções dos bytes do JSON.
import SwiftUI

nonisolated struct Segmento: Identifiable, Sendable {
    let id: String
    let nome: String
    let bytes: Int64
    let cor: Color
    var tracejado = false
    /// Barra que se desdobra (BarraDesdobravel): fonte e estado do número, e o nível que abre.
    var fonte = "difference"
    var estado = "measured"
    var desdobra: NivelBarra? = nil
}

/// Barra segmentada proporcional, 1 pt entre segmentos e mínimo de 2 pt, como a do Ajustes.
struct BarraView: View {
    let segmentos: [Segmento]
    var altura: CGFloat = 20
    var destaque: Set<String> = []
    /// Passar o mouse e clicar num segmento (Início, B5): o id, ou nil ao sair.
    var aoPassar: ((String?) -> Void)? = nil
    var aoClicar: ((String) -> Void)? = nil
    @Environment(\.accessibilityReduceMotion) private var reduzir

    var body: some View {
        GeometryReader { geo in
            let total = max(1, segmentos.reduce(0) { $0 + max(0, $1.bytes) })
            let util = geo.size.width - CGFloat(max(0, segmentos.count - 1))
            HStack(spacing: 1) {
                ForEach(segmentos) { s in
                    Group {
                        if s.tracejado {
                            Rectangle().strokeBorder(.secondary, style: StrokeStyle(lineWidth: 1, dash: [3, 3]))
                        } else {
                            Rectangle().fill(s.cor)
                        }
                    }
                    .frame(width: max(2, util * CGFloat(max(0, s.bytes)) / CGFloat(total)))
                    .opacity(destaque.isEmpty || destaque.contains(s.id) ? 1 : 0.35)
                    .help("\(s.nome) · \(gb(s.bytes))")
                    .onHover { aoPassar?($0 ? s.id : nil) }
                    .onTapGesture { aoClicar?(s.id) }
                }
            }
        }
        .frame(height: altura)
        .clipShape(.rect(cornerRadius: 6))
        .animation(reduzir ? nil : .spring(duration: 0.4, bounce: 0), value: segmentos.map(\.bytes))
        .animation(.easeOut(duration: 0.12), value: destaque)
        .accessibilityElement()
        .accessibilityLabel(segmentos.map { "\($0.nome) \(gb($0.bytes))" }.joined(separator: ", "))
    }
}

/// Mini-barra das tabelas: ocupa (cor do termo a 35 %) e libera se apagar (cor cheia), relativas
/// ao maior da lista.
struct BarraOcupa: View {
    let bytes: Int64
    let privado: Int64?
    let maximo: Int64
    var cor: Color = termo("tree").cor

    var body: some View {
        let f = { (b: Int64) in 100 * CGFloat(max(0, b)) / CGFloat(max(1, maximo)) }
        ZStack(alignment: .leading) {
            Capsule().fill(cor.opacity(0.35)).frame(width: max(2, f(bytes)))
            if let p = privado, p > 0 { Capsule().fill(cor).frame(width: max(2, f(p))) }
        }
        .frame(width: 100, height: 6, alignment: .leading)
        .accessibilityHidden(true)
    }
}

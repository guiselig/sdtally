// Recursos baixados pelo macOS (spec-fase6 §2.2 item 9a, §5.5; A9 do PLANO): as linhas de `top[]`
// em AssetsV2, agrupadas pela ficha. Só "Ocupa": o privado delas vem 0 sem causa conhecida. Dono: A10b.
// API estável:
//   RecursosView(c:)            some a seção quando não há linha de AssetsV2 no `top`
//   Recursos.grupos(_:fichas:)  pura: grupos por `card` (o maior primeiro) e a linha dobrada
import SwiftUI

nonisolated enum Recursos {
    static let prefixo = "/System/Library/AssetsV2/com_apple_MobileAsset_"
    /// Grupo abaixo disto vai para "outros recursos".
    static let dobra: Int64 = 100_000_000

    struct Grupo: Identifiable, Sendable, Equatable {
        /// O `card`, ou o caminho quando a linha não tem ficha.
        let id: String
        let nome: String
        /// O passo da ficha (o caminho indicado); nil sem ficha.
        let passo: String?
        let bytes: Int64
        /// Alguma linha é piso (`lower_bound`).
        let piso: Bool
    }

    /// Agrupa por `card` as linhas com o prefixo de AssetsV2; o nome vem da ficha (`what.texto`, sem o
    /// ponto final) ou, sem ficha, da pasta. Grupos abaixo de 100 MB somam na linha dobrada.
    static func grupos(_ top: [LinhaTop], fichas: MapaFichas?) -> (grupos: [Grupo], outros: Int64?) {
        let ls = top.filter { $0.path?.hasPrefix(prefixo) == true }
        let porCard = Dictionary(grouping: ls) { $0.card ?? $0.path! }
        let gs = porCard.map { id, ls -> Grupo in
            let f = fichas?[ls[0].card]
            let nome = f.map { $0.what.texto.hasSuffix(".") ? String($0.what.texto.dropLast()) : $0.what.texto }
                ?? visivel(String(ls[0].path!.dropFirst(prefixo.count)))
            return Grupo(id: id, nome: nome, passo: f?.step.texto, bytes: ls.reduce(0) { $0 + $1.bytes }, piso: ls.contains(where: \.lowerBound))
        }
        .sorted { ($0.bytes, $1.id) > ($1.bytes, $0.id) }
        let pequenos = gs.filter { $0.bytes < dobra }
        return (gs.filter { $0.bytes >= dobra }, pequenos.isEmpty ? nil : pequenos.reduce(0) { $0 + $1.bytes })
    }
}

struct RecursosView: View {
    let c: Completo

    var body: some View {
        let r = Recursos.grupos(c.top, fichas: c.cards)
        if !r.grupos.isEmpty || r.outros != nil {
            VStack(alignment: .leading, spacing: 12) {
                Text("Recursos baixados pelo macOS").font(.headline)
                    .help("Gerenciados pelo macOS. Não apague dentro de AssetsV2; use o caminho indicado em cada um. Medido às \(hora(c.generatedAt)).")
                Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 10) {
                    GridRow {
                        Color.clear.frame(width: 20, height: 1)
                        Text("Recurso")
                        Text("Ocupa").gridColumnAlignment(.trailing)
                    }
                    .font(.caption).foregroundStyle(.secondary)
                    ForEach(r.grupos) { g in linha("arrow.down.circle", g.nome, g.passo, g.bytes, piso: g.piso) }
                    if let o = r.outros { linha("ellipsis.circle", String(localized: "outros recursos (abaixo de 100 MB cada)"), nil, o, piso: false) }
                }
            }
            .cartao()
        }
    }

    private func linha(_ simbolo: String, _ nome: String, _ detalhe: String?, _ bytes: Int64, piso: Bool) -> some View {
        GridRow(alignment: .firstTextBaseline) {
            Image(systemName: simbolo).foregroundStyle(.secondary).frame(width: 20)
            VStack(alignment: .leading, spacing: 2) {
                Text(nome).help(detalhe ?? "")
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            NumeroView(bytes: bytes, source: "top", state: piso ? "partial" : "measured", quando: String(localized: "às \(hora(c.generatedAt))"), compacto: true)
        }
    }
}

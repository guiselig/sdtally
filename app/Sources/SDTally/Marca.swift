// Sistema visual (spec-fase5 §3, spec-inicio §6). Só cores de sistema: nenhum hex, nenhuma cor de
// marca. Controles seguem o destaque do macOS (a pessoa escolhe em Ajustes); os dados nunca usam
// o destaque, que pode ser vermelho (G9). API:
//   Marca.dados, .aviso, .bom, Marca.nivel(_ level: String) -> (cor: Color, simbolo: String)
//   Font.heroi (usar com .tracking(-0.5)), Font.kpi
//   view.cartao(destaque: Color? = nil), view.transicaoNumero(_ valor: Double)
//   Faixa(simbolo:tom:texto:) { ação opcional }, TallyShape(tracos:), IconeApp(tamanho:)
//   FichaKPI(titulo:bytes:fonte:variacao:compacta:)
import SwiftUI

nonisolated enum Marca {
    /// Dados do Sistema (cinza, como no Ajustes): fixo, nunca o destaque do sistema, que pode ser vermelho (G9).
    static let dados = Color.gray
    /// Toda faixa de aviso, sempre com símbolo.
    static let aviso = Color.orange
    static let bom = Color.green

    /// Nível do "O que fazer" (§3.2): cor do selo e símbolo. Nível desconhecido vira `measurement`.
    /// G9 (spec-fase6 §6.1): nenhum nível usa vermelho; `do_not_touch` é `.secondary`.
    static func nivel(_ level: String) -> (cor: Color, simbolo: String) {
        switch level {
        case "system_handles": (.blue, "checkmark.circle")
        case "safe": (bom, "leaf")
        case "cost": (aviso, "clock.arrow.circlepath")
        case "check_first": (.orange, "exclamationmark.magnifyingglass")
        case "do_not_touch": (.secondary, "hand.raised")
        case "anomaly": (.indigo, "questionmark.diamond")
        default: (.secondary, "gauge.with.dots.needle.50percent")
        }
    }
}

extension Font {
    static let heroi = Font.system(size: 44, weight: .bold).monospacedDigit()
    static let kpi = Font.system(.title2, design: .rounded, weight: .semibold).monospacedDigit()
}

// MARK: - Cartão e número

private struct Cartao: ViewModifier {
    let destaque: Color?
    @Environment(\.colorSchemeContrast) private var contraste

    func body(content: Content) -> some View {
        content
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(.background.secondary, in: .rect(cornerRadius: 12))
            .overlay {
                if let destaque {
                    RoundedRectangle(cornerRadius: 12).strokeBorder(destaque, lineWidth: contraste == .increased ? 2 : 1)
                } else if contraste == .increased {
                    RoundedRectangle(cornerRadius: 12).strokeBorder(.separator, lineWidth: 1)
                }
            }
    }
}

private struct TransicaoNumero: ViewModifier {
    let valor: Double
    @Environment(\.accessibilityReduceMotion) private var reduzir
    func body(content: Content) -> some View {
        content.contentTransition(reduzir ? .opacity : .numericText(value: valor))
    }
}

extension View {
    /// Substitui o GroupBox: padding 16, fundo opaco, borda na cor de destaque.
    func cartao(destaque: Color? = nil) -> some View { modifier(Cartao(destaque: destaque)) }
    /// `.numericText`, ou `.opacity` com Reduzir Movimento.
    func transicaoNumero(_ valor: Double) -> some View { modifier(TransicaoNumero(valor: valor)) }
}

// MARK: - Faixa

/// Faixa de aviso com símbolo, texto e uma ação opcional (generaliza a FaixaAcessoTotal).
struct Faixa<Acao: View>: View {
    let simbolo: String
    var tom: Color = Marca.aviso
    let texto: String
    @ViewBuilder var acao: Acao

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: simbolo).font(.title3).foregroundStyle(tom).accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 8) {
                Text(texto).font(.callout).fixedSize(horizontal: false, vertical: true)
                acao
            }
            Spacer(minLength: 0)
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(tom.opacity(0.1), in: .rect(cornerRadius: 8))
    }
}

extension Faixa where Acao == EmptyView {
    init(simbolo: String, tom: Color = Marca.aviso, texto: String) {
        self.init(simbolo: simbolo, tom: tom, texto: texto) { EmptyView() }
    }
}

// MARK: - Glifo e ícone

/// Marca de contagem: traços verticais cortados por uma diagonal. Desenhar com `.stroke`.
nonisolated struct TallyShape: Shape {
    var tracos = 4

    func path(in r: CGRect) -> Path {
        var p = Path()
        let passo = r.width / CGFloat(tracos + 1)
        for i in 1...tracos {
            let x = r.minX + passo * CGFloat(i)
            p.move(to: CGPoint(x: x, y: r.minY + r.height * 0.1))
            p.addLine(to: CGPoint(x: x, y: r.maxY - r.height * 0.1))
        }
        p.move(to: CGPoint(x: r.minX, y: r.maxY - r.height * 0.22))
        p.addLine(to: CGPoint(x: r.maxX, y: r.minY + r.height * 0.22))
        return p
    }
}

/// Ícone do app. No .app, o do sistema (SDTally.icon, com o Liquid Glass e a variante da aparência atual).
/// Fora dele (desenvolvimento e fotos), desenhado na grade de 1024: área útil de 824, raio 185 contínuo,
/// gradiente azul do sistema, brilho de 25 %, sombra de 30 % e traços brancos. Em 64 px ou menos, três traços e a diagonal.
struct IconeApp: View {
    var tamanho: CGFloat = 96

    var body: some View {
        if Dev.ativo { desenhado } else {
            Image(nsImage: NSApp.applicationIconImage).resizable().frame(width: tamanho, height: tamanho).accessibilityHidden(true)
        }
    }

    private var desenhado: some View {
        let k = tamanho / 1024
        let forma = RoundedRectangle(cornerRadius: 185 * k, style: .continuous)
        return ZStack {
            forma.fill(Color.blue.gradient)
            forma.fill(LinearGradient(colors: [.white.opacity(0.25), .clear], startPoint: .top, endPoint: .center))
            TallyShape(tracos: tamanho <= 64 ? 3 : 4)
                .stroke(.white, style: StrokeStyle(lineWidth: (tamanho <= 64 ? 64 : 52) * k, lineCap: .round))
                .frame(width: 520 * k, height: 440 * k)
        }
        .frame(width: 824 * k, height: 824 * k)
        .shadow(color: .black.opacity(0.3), radius: 20 * k, y: 10 * k)
        .frame(width: tamanho, height: tamanho)
        .accessibilityHidden(true)
    }
}

// MARK: - Ficha

/// Número em Font.kpi, com o nome, a fonte e uma variação opcional ("+1,2 GB desde ontem").
/// Compacta (janela da barra de menus): Liquid Glass com cantos de 16 pt. O SDK do macOS 27 não
/// tem nível de transparência no Glass: só `.regular` e `.clear` (e `tint`).
private struct FundoFicha: ViewModifier {
    let compacta: Bool

    func body(content: Content) -> some View {
        if compacta, #available(macOS 26, *) {
            content.glassEffect(.regular, in: .rect(cornerRadius: 16))
        } else if compacta {
            content.background(.background.secondary, in: .rect(cornerRadius: 16))
        } else {
            content.background(.background.secondary, in: .rect(cornerRadius: 8))
        }
    }
}

struct FichaKPI: View {
    let titulo: String
    let bytes: Int64?
    /// nil: sem a linha da fonte (janela da barra de menus).
    var fonte: String?
    var variacao: String?
    var compacta = false

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            // Compacta (janela da barra, 300 pt): o título numa linha só, as duas fichas da mesma altura.
            Text(titulo).font(compacta ? .subheadline : .callout).foregroundStyle(.secondary)
                .lineLimit(compacta ? 1 : nil).minimumScaleFactor(compacta ? 0.7 : 1)
            Text(bytes.map(gb) ?? "—")
                .font(compacta ? .title2.weight(.semibold).monospacedDigit() : .kpi)
                .transicaoNumero(Double(bytes ?? 0))
            if let variacao { Text(variacao).font(.caption).foregroundStyle(.secondary) }
            if let fonte { Text(fonte).font(.caption2).foregroundStyle(.tertiary) }
        }
        // Compacta: mais alta, sem roubar largura do título.
        .padding(.horizontal, 12)
        .padding(.vertical, compacta ? 16 : 12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .modifier(FundoFicha(compacta: compacta))
        .accessibilityElement(children: .combine)
    }
}

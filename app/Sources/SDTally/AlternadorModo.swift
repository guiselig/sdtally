// Alternador Automático | Manual do cabeçalho (painel.manual), um só para o app. Fica na faixa do
// semáforo (barra de título escondida), centrado sobre o conteúdo, nas telas que mostram medição: Dados do Sistema, Pastas maiores, Histórico e O que fazer. Disco,
// Permissões e Sobre não têm. No macOS 26+, a barra de abas de vidro do iOS imitada (PilulaDeVidro).
import SwiftUI

struct AlternadorModo: View {
    @Environment(Painel.self) private var painel

    static func mostra(_ s: Secao) -> Bool { [.sistema, .ranking, .fazer].contains(s) }

    var body: some View {
        @Bindable var painel = painel
        if #available(macOS 26, *) {
            PilulaDeVidro(opcoes: [String(localized: "Automático"), String(localized: "Manual")], escolha: Binding(
                get: { painel.manual ? 1 : 0 },
                set: { i in painel.manual = i == 1 }))
                .help("Automático (⌘←): ao vivo, sem senha. Manual (⌘→): a última Medição completa.")
                .accessibilityRepresentation {
                    Picker("Medição", selection: $painel.manual) {
                        Text("Automático").tag(false)
                        Text("Manual").tag(true)
                    }
                }
        } else {
            Picker("Medição", selection: $painel.manual) {
                Text("Automático").tag(false)
                Text("Manual").tag(true)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .fixedSize()
        }
    }
}

/// A barra de abas de vidro do iOS 26, que o macOS não tem: trilho de Liquid Glass e a escolha
/// numa cápsula escura. Segurando ou arrastando, a escolha vira uma lente de vidro claro, maior
/// que o trilho, que segue o ponteiro; ao soltar, encaixa na opção mais perto, com mola.
/// shortcut: imitação (gesto + glassEffect), não o controle do sistema; trocar se a Apple trouxer o do iOS ao macOS.
@available(macOS 26, *)
private struct PilulaDeVidro: View {
    let opcoes: [String]
    @Binding var escolha: Int
    /// Molduras das opções no espaço do trilho, medidas.
    @State private var molduras: [Int: CGRect] = [:]
    /// Centro da lente enquanto segura ou arrasta; nil parado.
    @State private var lente: CGFloat?

    private let mola = Animation.spring(response: 0.35, dampingFraction: 0.72)

    var body: some View {
        HStack(spacing: 0) {
            ForEach(opcoes.indices, id: \.self) { i in
                Text(opcoes[i])
                    .fontWeight(.semibold)
                    .foregroundStyle(destacada == i ? AnyShapeStyle(.tint) : AnyShapeStyle(.primary))
                    .padding(.horizontal, 16).padding(.vertical, 7)
                    .onGeometryChange(for: CGRect.self) { $0.frame(in: .named("trilho")) } action: { molduras[i] = $0 }
            }
        }
        // As molduras e o gesto no espaço desta pilha (antes do respiro do trilho).
        .coordinateSpace(.named("trilho"))
        // Parada, a cápsula escura fica atrás do texto; a lente fica por cima e o refrata.
        .background { if lente == nil { selecao } }
        .overlay { if lente != nil { selecao.allowsHitTesting(false) } }
        .padding(3)
        .glassEffect(.regular, in: .capsule)
        .contentShape(.capsule)
        .gesture(DragGesture(minimumDistance: 0, coordinateSpace: .named("trilho"))
            .onChanged { v in withAnimation(lente == nil ? mola : .interactiveSpring(response: 0.18)) { lente = limitar(v.location.x) } }
            .onEnded { v in
                let i = maisPerto(v.location.x)
                withAnimation(mola) { lente = nil; escolha = i }
            })
    }

    /// A opção sob a lente (arrastando) ou a escolhida (parado): leva a cor de destaque.
    private var destacada: Int { lente.map(maisPerto) ?? escolha }

    @ViewBuilder private var selecao: some View {
        if let m = molduras[escolha] ?? molduras.values.first {
            let largura = lente == nil ? m.width : (molduras[maisPerto(lente!)]?.width ?? m.width)
            let centro = lente ?? m.midX
            Group {
                if lente != nil {
                    // A lente: vidro claro, maior que o trilho, sobre o texto. Só a borda é vidro
                    // (a máscara tira o miolo): o vidro do macOS desfoca, e o miolo fica transparente.
                    Capsule().fill(.clear)
                        .glassEffect(.clear.interactive(), in: .capsule)
                        .mask(Capsule().strokeBorder(lineWidth: 6))
                        .frame(width: largura + 14, height: m.height + 14)
                } else {
                    Capsule().fill(Color.black.opacity(0.35))
                        .frame(width: largura, height: m.height)
                }
            }
            .position(x: centro, y: m.midY)
        }
    }

    private func limitar(_ x: CGFloat) -> CGFloat {
        let xs = molduras.values
        guard let a = xs.map(\.midX).min(), let b = xs.map(\.midX).max() else { return x }
        return min(max(x, a), b)
    }

    private func maisPerto(_ x: CGFloat) -> Int {
        molduras.min { abs($0.value.midX - x) < abs($1.value.midX - x) }?.key ?? escolha
    }
}

/// Tela sem medição no modo do alternador. Automático: a primeira passada ainda corre. Manual:
/// nenhuma Medição completa, com o botão de medir com senha.
struct SemMedicao: View {
    @Environment(Painel.self) private var painel
    let simbolo: String

    var body: some View {
        if painel.manual {
            ContentUnavailableView {
                Label("Nenhuma medição completa", systemImage: simbolo)
            } description: {
                Text("A medição com senha alcança as pastas do sistema e de outros usuários.")
            } actions: {
                Button("Medir com senha") { painel.medirComSenha() }
                    .buttonStyle(.borderedProminent)
                    .disabled(painel.medindo)
            }
        } else {
            ContentUnavailableView {
                Label("Lendo o disco", systemImage: simbolo)
            } description: {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text("A primeira leitura automática leva cerca de 1 minuto.")
                }
            }
        }
    }
}

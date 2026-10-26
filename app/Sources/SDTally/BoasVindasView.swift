// Boas-vindas (spec-fase5 §2.3; 19/10): folha de 4 páginas, Boas-vindas, O que faz, Como faz e
// Permissões. Pede só o que o dono clicar. Dono: A4.
// API estável:
//   BoasVindasView(pagina:)  lê Painel do ambiente; "Começar" grava @AppStorage("viuBoasVindas")
// A JanelaView apresenta a folha por painel.mostrarBoasVindas.
import AppKit
import SwiftUI

struct BoasVindasView: View {
    @Environment(\.dismiss) private var fechar
    @AppStorage("viuBoasVindas") private var viu = false
    @State var pagina = 0

    private static let titulos = [String(localized: "Bem-vindo ao SDTally"), String(localized: "O que ele faz"), String(localized: "Como ele faz"), String(localized: "Permissões")]

    private static let faz: [Item] = [
        Item("square.stack.3d.up.fill", String(localized: "Explica Dados do Sistema"), String(localized: "Divide o número do Ajustes em partes com nome.")),
        Item("list.number", String(localized: "Mostra o que mais ocupa"), String(localized: "As maiores pastas do disco, com o app de cada uma.")),
        Item("checklist", String(localized: "Diz o que fazer"), String(localized: "O que o macOS libera sozinho, o que conferir e o que não mexer.")),
        Item("chart.xyaxis.line", String(localized: "Acompanha o disco"), String(localized: "Guarda o histórico e avisa quando o disco cresce.")),
    ]

    private static let como: [Item] = [
        Item("gearshape", String(localized: "Lê o Ajustes escondido"), String(localized: "Abre o Armazenamento sem mostrar a janela, lê o número e fecha.")),
        Item("magnifyingglass", String(localized: "Mede as pastas"), String(localized: "Percorre o disco quando você pede, com ou sem senha.")),
        Item("eye", String(localized: "Só lê"), String(localized: "Nunca apaga nem altera arquivos, sem rede e sem telemetria.")),
        Item("number", String(localized: "Cada número com a fonte"), String(localized: "Passe o mouse sobre um número para ver de onde ele veio.")),
    ]

    var body: some View {
        VStack(spacing: 0) {
            VStack(spacing: 10) {
                IconeApp(tamanho: pagina == 0 ? 96 : 64)
                Text(Self.titulos[pagina])
                    .font(.system(.title, design: .rounded, weight: .bold))
            }
            .frame(maxWidth: .infinity)
            .padding(.top, 28)
            conteudo
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 32).padding(.top, 28)
            Spacer(minLength: 0)
            rodape
        }
        // Altura mínima, não fixa: em tailandês a página 3 passa de 640 e cortava o rodapé.
        .frame(width: 560)
        .frame(minHeight: pagina == 3 ? 640 : 460)
        .fixedSize(horizontal: false, vertical: true)
        .animation(.smooth(duration: 0.25), value: pagina)
    }

    @ViewBuilder private var conteudo: some View {
        switch pagina {
        case 0:
            VStack(spacing: 28) {
                Text("O que tem dentro de Dados do Sistema, com o número do próprio Ajustes.")
                    .font(.title3).foregroundStyle(.secondary)
                // Pedido de estrela só aqui, sem lembrete depois (dono, 26/10). Abre no navegador: o app segue sem rede.
                VStack(spacing: 4) {
                    Text("Se o SDTally te ajudar, uma estrela no GitHub ajuda outras pessoas a encontrá-lo.")
                        .foregroundStyle(.secondary)
                    Button("Dar uma estrela no GitHub", systemImage: "star") {
                        NSWorkspace.shared.open(URL(string: "https://github.com/guiselig/sdtally")!)
                    }
                    .buttonStyle(.link)
                }
                .font(.callout)
            }
            .multilineTextAlignment(.center).frame(maxWidth: .infinity)
        case 1: lista(Self.faz)
        case 2: lista(Self.como)
        default:
            VStack(alignment: .leading, spacing: 12) {
                LinhaAcessoTotal(explicar: true)
                Divider()
                LinhaMedidorRoot(explicar: true)
                Divider()
                Group {
                    ToggleBarraDeMenus(explicar: true)
                    ToggleIniciarSessao(explicar: true)
                    ToggleNotificar(explicar: true)
                }
                .toggleStyle(.switch)
            }
        }
    }

    private func lista(_ itens: [Item]) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            ForEach(itens, id: \.simbolo) { i in
                HStack(alignment: .top, spacing: 14) {
                    Image(systemName: i.simbolo)
                        .font(.title2).foregroundStyle(.tint)
                        .frame(width: 32)
                        .accessibilityHidden(true)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(i.titulo).font(.headline)
                        Text(i.texto).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                    }
                }
            }
        }
    }

    private var rodape: some View {
        // Os pontos no centro da folha, com ou sem o Voltar.
        HStack {
            if pagina > 0 { Button("Voltar") { pagina -= 1 } }
            Spacer()
            if pagina < Self.titulos.count - 1 {
                Button("Continuar") { pagina += 1 }.keyboardShortcut(.defaultAction)
            } else {
                Button("Começar") { viu = true; fechar() }.keyboardShortcut(.defaultAction)
            }
        }
        .overlay {
            HStack(spacing: 6) {
                ForEach(Self.titulos.indices, id: \.self) { i in
                    Circle().fill(i == pagina ? AnyShapeStyle(.primary) : AnyShapeStyle(.quaternary)).frame(width: 6, height: 6)
                }
            }
            .accessibilityElement().accessibilityLabel("Página \(pagina + 1) de \(Self.titulos.count)")
        }
        .padding(20)
    }

    private struct Item {
        let simbolo, titulo, texto: String
        init(_ simbolo: String, _ titulo: String, _ texto: String) { (self.simbolo, self.titulo, self.texto) = (simbolo, titulo, texto) }
    }
}

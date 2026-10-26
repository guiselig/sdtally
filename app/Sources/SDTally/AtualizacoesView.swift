// Atualizações (dono, 26/10): o app só usa a rede para procurar e baixar versões no github.com,
// e só instala com o clique. Aqui o texto do estado e a faixa do topo da janela (versão nova e o
// aviso único de quem atualizou). A opção é ToggleAtualizacoes; a lógica, Atualizacoes.
import SwiftUI

/// Download ou instalação em andamento: os botões de instalar e procurar esperam.
func atualizacaoOcupada(_ a: Atualizacoes) -> Bool {
    switch a.estado {
    case .procurando, .baixando, .instalando: true
    default: false
    }
}

/// Uma linha para o estado; nil sem nada a dizer.
func textoAtualizacao(_ a: Atualizacoes) -> String? {
    switch a.estado {
    case .procurando: return String(localized: "Procurando versão nova…")
    case .baixando: return String(localized: "Baixando a versão nova…")
    case .instalando: return String(localized: "Instalando a versão nova…")
    case .erro(let m): return String(localized: "Não deu para atualizar: \(m)")
    case .parado:
        if let n = a.disponivel { return String(localized: "SDTally \(n.versao) disponível") }
        guard let d = a.ultimaChecagem else { return nil }
        // Só a hora quando foi hoje; senão a data junto, para não parecer uma busca recente.
        let quando = d.formatted(date: Calendar.current.isDateInToday(d) ? .omitted : .abbreviated, time: .shortened)
        return String(localized: "Você está na versão mais nova. Última busca: \(quando)")
    }
}

/// Faixa discreta embaixo do título da tela: a versão nova, ou uma vez o aviso de que o app agora
/// procura atualizações (só para quem já tinha visto as boas-vindas).
struct FaixaAtualizacao: View {
    @Environment(Painel.self) private var painel
    @AppStorage("viuBoasVindas") private var viuBoasVindas = false

    var body: some View {
        let a = Atualizacoes.unica
        if let n = a.disponivel {
            faixa("arrow.down.circle.fill") {
                Text(textoAtualizacao(a) ?? "").fixedSize(horizontal: false, vertical: true)
                Spacer(minLength: 8)
                if atualizacaoOcupada(a) {
                    ProgressView().controlSize(.small)
                } else {
                    Button("Novidades") { NSWorkspace.shared.open(n.pagina) }.buttonStyle(.link)
                    Button("Instalar") { a.instalar() }.buttonStyle(.borderedProminent).controlSize(.small)
                }
            }
        } else if viuBoasVindas && !a.jaAvisouRede {
            faixa("network") {
                Text("O SDTally agora procura atualizações no github.com. Dá para desligar em Configurações › Versões.")
                    .fixedSize(horizontal: false, vertical: true)
                Spacer(minLength: 8)
                Button("Configurações") { a.jaAvisouRede = true; painel.mostrar(.permissoes) }.buttonStyle(.link)
                Button("Fechar", systemImage: "xmark") { a.jaAvisouRede = true }
                    .labelStyle(.iconOnly).buttonStyle(.borderless)
            }
        }
    }

    private func faixa(_ simbolo: String, @ViewBuilder _ conteudo: () -> some View) -> some View {
        HStack(spacing: 10) {
            Image(systemName: simbolo).foregroundStyle(.tint).accessibilityHidden(true)
            conteudo()
        }
        .font(.callout)
        .padding(.horizontal, 12).padding(.vertical, 8)
        .background(.quaternary.opacity(0.5), in: .rect(cornerRadius: 10))
        .padding(.horizontal, 20)
    }
}

extension Atualizacoes.Estado {
    var ehErro: Bool { if case .erro = self { true } else { false } }
}

// Acesso Total ao Disco obrigatório (dono, 24/10). Sem ele o app não lê nada: nenhum sdtally e
// nenhum caminho tocado (o próprio app, ao conferir um caminho, buscar um ícone ou um nome, faria o
// macOS pedir permissão pasta por pasta). A janela e o menu da barra mostram só como liberar.
// Fora do .app (Dev: fotos, testes) não vale.
import AppKit
import SwiftUI

/// O TCC.db do sistema só abre com Acesso Total; tentar abrir não faz o macOS pedir nada.
nonisolated func temAcessoTotal() -> Bool {
    FileHandle(forReadingAtPath: "/Library/Application Support/com.apple.TCC/TCC.db") != nil
}

@Observable final class Portao {
    static let unico = Portao()
    private(set) var liberado = Dev.ativo || temAcessoTotal()

    /// Roda `aoLiberar` quando houver Acesso Total, conferindo a cada 2 s.
    func esperar(_ aoLiberar: @escaping @MainActor () -> Void) {
        guard !liberado else { return aoLiberar() }
        Task {
            while !temAcessoTotal() { try? await Task.sleep(for: .seconds(2)) }
            liberado = true
            aoLiberar()
        }
    }
}

/// A janela inteira enquanto falta o Acesso Total.
struct TelaAcessoTotal: View {
    @Environment(Painel.self) private var painel

    var body: some View {
        VStack(spacing: 20) {
            IconeApp(tamanho: 96)
            Text("Dê Acesso Total ao Disco ao SDTally").font(.system(.title, design: .rounded, weight: .bold))
                .multilineTextAlignment(.center)
            Text("O SDTally só funciona com o Acesso Total ao Disco. Sem ele, o macOS esconde pastas como Mail, Mensagens e Safari e pergunta por cada pasta protegida. O SDTally só lê: não apaga nem altera nada.")
                .foregroundStyle(.secondary).multilineTextAlignment(.center).fixedSize(horizontal: false, vertical: true)
            VStack(alignment: .leading, spacing: 8) {
                passo(1, String(localized: "Clique em Abrir Privacidade e Segurança."))
                passo(2, String(localized: "Ligue o SDTally na lista do Acesso Total ao Disco."))
                passo(3, String(localized: "Se o macOS pedir, reabra o SDTally."))
            }
            Button("Abrir Privacidade e Segurança") { painel.abrirAcessoTotal() }
                .buttonStyle(.borderedProminent).controlSize(.large)
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text("Esperando o Acesso Total ao Disco…").foregroundStyle(.secondary)
            }
        }
        .padding(48)
        .frame(maxWidth: 520)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(nsColor: .windowBackgroundColor))
    }

    private func passo(_ n: Int, _ texto: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Image(systemName: "\(n).circle.fill").foregroundStyle(.secondary).accessibilityHidden(true)
            Text(texto).fixedSize(horizontal: false, vertical: true)
        }
    }
}

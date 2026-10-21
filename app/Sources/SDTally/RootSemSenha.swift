// Root sem Senha (o helper do SMAppService): o aviso antes de instalar e o estado "só Automático".
// Com ele instalado, nenhuma medição pede senha nem Touch ID: é para isso que ele existe. Só a
// troca em Configurações > Avançado pede o Touch ID.
import LocalAuthentication
import SwiftUI

/// Touch ID, com a senha da conta como alternativa (.deviceOwnerAuthentication). true se a pessoa
/// confirmou; false se cancelou ou o Mac não tem como pedir. `motivo` completa "SDTally quer <motivo>".
@MainActor func confirmarDono(_ motivo: String) async -> Bool {
    let c = LAContext()
    var erro: NSError?
    guard c.canEvaluatePolicy(.deviceOwnerAuthentication, error: &erro) else { return false }
    return (try? await c.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: motivo)) ?? false
}

/// "Instalar Root sem Senha?" antes de registrar o helper. Os botões de instalar só ligam
/// `painel.perguntarInstalar`; o alerta fica na janela (JanelaView).
struct AvisoInstalarRoot: ViewModifier {
    @Environment(Painel.self) private var painel

    func body(content: Content) -> some View {
        @Bindable var painel = painel
        content.alert("Instalar o Root sem Senha?", isPresented: $painel.perguntarInstalar) {
            Button("Instalar") { painel.instalarHelper() }
            Button("Cancelar", role: .cancel) {}
        } message: {
            Text(Self.aviso)
        }
    }

    static let aviso = String(localized: """
    Serve para o SDTally não pedir a senha de administrador a cada medição. O macOS pede a sua aprovação uma vez, em Itens de Início; depois disso, medir não pede senha.

    O app só usa o root para medir: lê o tamanho das pastas, nunca o conteúdo, não apaga nem altera nada e não usa a rede.

    Dá para remover a qualquer hora em Configurações.
    """)
}

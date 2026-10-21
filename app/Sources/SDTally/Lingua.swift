// Idioma do app (Configurações > Aparência): o do sistema (padrão) ou uma das traduções do
// Localizable.xcstrings. Grava AppleLanguages só no domínio do app, como o Ajustes > Idioma e
// Região > Aplicativos; vale ao reabrir, porque o SwiftUI e o Bundle.main leem a língua na abertura.
import AppKit
import SwiftUI

enum Lingua {
    /// Domínio do app no UserDefaults (no binário solto do `swift build`, o nome do processo).
    static var dominio: String { Bundle.main.bundleIdentifier ?? ProcessInfo.processInfo.processName }

    /// A escolhida no app; "" = a do sistema. Lê só o domínio do app: `stringArray(forKey:)`
    /// cairia na lista global do sistema.
    static var escolhida: String {
        (UserDefaults.standard.persistentDomain(forName: dominio)?["AppleLanguages"] as? [String])?.first ?? ""
    }

    static func escolher(_ codigo: String) {
        if codigo.isEmpty { UserDefaults.standard.removeObject(forKey: "AppleLanguages") }
        else { UserDefaults.standard.set([codigo], forKey: "AppleLanguages") }
    }

    /// Árabe e hebraico: o AppKit só espelha a janela com estas duas chaves gravadas no domínio do
    /// app (com o -AppleLanguages ou o AppleLanguages do app ele traduz, mas fica da esquerda para a
    /// direita; o register(defaults:) ele não lê). Gravadas na abertura, valem já nesta.
    static func ajustarDirecao() {
        let d = UserDefaults.standard
        if Locale.Language(identifier: linguaDoApp).characterDirection == .rightToLeft {
            d.set(true, forKey: "AppleTextDirection")
            d.set(true, forKey: "NSForceRightToLeftWritingDirection")
        } else {
            d.removeObject(forKey: "AppleTextDirection")
            d.removeObject(forKey: "NSForceRightToLeftWritingDirection")
        }
    }

    /// As traduções que o app tem, cada uma no próprio nome ("Deutsch", "日本語"), em ordem alfabética.
    static let opcoes: [(codigo: String, nome: String)] = Bundle.main.localizations
        .filter { $0 != "Base" }
        .map { c in
            let l = Locale(identifier: c)
            return (c, (l.localizedString(forIdentifier: c) ?? c).capitalized(with: l))
        }
        .sorted { $0.nome.localizedStandardCompare($1.nome) == .orderedAscending }

    /// Reabre o app: um shell espera este processo sair e abre o .app de novo. Só no .app;
    /// o binário solto do `swift build` não tem pacote para o `open`.
    static var podeReabrir: Bool { Bundle.main.bundleURL.pathExtension == "app" }

    static func reabrir() {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/bin/sh")
        p.arguments = ["-c", "while kill -0 \(ProcessInfo.processInfo.processIdentifier) 2>/dev/null; do sleep 0.2; done; /usr/bin/open \"$0\"",
                       Bundle.main.bundleURL.path]
        try? p.run()
        NSApp.terminate(nil)
    }
}

struct PickerLingua: View {
    @State private var escolha = Lingua.escolhida
    @State private var perguntar = false

    var body: some View {
        Picker("Idioma", selection: $escolha) {
            Text("Idioma do sistema").tag("")
            Divider()
            ForEach(Lingua.opcoes, id: \.codigo) { Text($0.nome).tag($0.codigo) }
        }
        .onChange(of: escolha) { _, novo in
            Lingua.escolher(novo)
            perguntar = true
        }
        .alert("Reabrir o SDTally?", isPresented: $perguntar) {
            if Lingua.podeReabrir { Button("Reabrir") { Lingua.reabrir() } }
            Button(Lingua.podeReabrir ? "Depois" : "OK", role: .cancel) {}
        } message: {
            Text("O novo idioma vale a partir da próxima vez que o SDTally abrir.")
        }
    }
}

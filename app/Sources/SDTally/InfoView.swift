// Informações (spec-fase5 §2.4 e §5): versão, caminhos, Mac e "Comandos" (para agentes de IA)
// (textos da spec-fase6 §5.8).
// Dono: A2. API estável:
//   InfoView()           lê Painel do ambiente
import AppKit
import SDTallyXPC
import SwiftUI

struct InfoView: View {
    @Environment(Painel.self) private var painel

    var body: some View {
        let info = Bundle.main.infoDictionary ?? [:]
        let bin = Executor.binario
        VStack(spacing: 0) {
        // Fora do Form: só o conteúdo, sem o cartão (fundo e contorno) de uma seção.
        VStack(spacing: 8) {
            IconeApp(tamanho: 96)
            Text("SDTally").font(.system(.largeTitle, design: .rounded, weight: .bold))
            Text("Só lê · Sem rede · Sem telemetria").foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.top, 8)
        Form {
            Section {
                if Dev.ativo {
                    LabeledContent("Versão", value: String(localized: "Fora do .app (desenvolvimento)"))
                } else {
                    LabeledContent("Versão", value: info["CFBundleShortVersionString"] as? String ?? "—")
                }
            }
            Section {
                LabeledContent("Repositório") {
                    VStack(alignment: .trailing, spacing: 2) {
                        Link("github.com/guiselig/sdtally", destination: urlRepositorio)
                    }
                }
                LabeledContent("Licença", value: "GPL-3.0-or-later · © 2026 Guilherme Selig")
                LabeledContent("Assinatura", value: HelperXPC.timeProprio().map { String(localized: "Apple Development · time \($0)") } ?? "ad hoc")
                    .help(HelperXPC.timeProprio() == nil ? String(localized: "O Root sem Senha não funciona com assinatura ad hoc.") : "")
            }
            Section("Caminhos") {
                caminho(String(localized: "App"), Bundle.main.bundleURL.path)
                caminho("sdtally", bin)
                caminho(String(localized: "Root sem Senha"), helper, ausente: Dev.ativo ? String(localized: "Fora do .app (desenvolvimento)") : "—")
                caminho(String(localized: "Pasta de dados"), Historico.pastaLeitura(env: Foto.env, bundleID: Bundle.main.bundleIdentifier)?.path)
            }
            Section("Mac") {
                // operatingSystemVersionString vem em inglês ("Version 27.0 (Build …)"): mesmos dados pelo sysctl.
                LabeledContent("macOS", value: "\(Self.sysctl("kern.osproductversion")) (\(Self.sysctl("kern.osversion")))")
                LabeledContent("Processador", value: Self.sysctl("machdep.cpu.brand_string"))
                LabeledContent("Modelo", value: Self.sysctl("hw.model"))
            }
            Section {
                let s = "\"\(bin ?? "sdtally")\""
                comando(String(localized: "Leitura rápida"), "\(s) --quick --json --redact names")
                comando(String(localized: "Última medição"), "\(s) --last --json --redact names")
                comando("MCP", "claude mcp add sdtally -- \(s) --mcp")
                comando(String(localized: "Catálogo fontes"), "\(s) --catalog --json")
                comando(String(localized: "Verificar"), "\(s) --verify")
            } header: {
                Text("Comandos")
                    .help("Pelo MCP, seu nome de usuário e os nomes de pastas pessoais são trocados antes de sair do sdtally (`--redact off` desliga). O SDTally não manda nada pela rede, mas o cliente de IA manda. A IA nunca mede como administrador nem executa os passos de O que fazer; lê a última medição completa feita no app.")
            }
        }
        .formStyle(.grouped)
        // Sem o fundo próprio do Form (mais escuro): a cor da janela, igual ao cabeçalho.
        .scrollContentBackground(.hidden)
        }
    }

    private var helper: String? {
        let p = Bundle.main.bundleURL.appending(path: "Contents/MacOS/SDTallyHelper").path
        return FileManager.default.fileExists(atPath: p) ? p : nil
    }

    private func caminho(_ nome: String, _ p: String?, ausente: String = "—") -> some View {
        LabeledContent(nome) {
            Text(p.map { ($0 as NSString).abbreviatingWithTildeInPath } ?? ausente)
                .lineLimit(1).truncationMode(.middle)
                .help(p ?? "")
        }
        .contextMenu { if let p { MenuArquivo(caminho: p) } }
    }

    /// Título numa linha, comando e Copiar na de baixo, em todos (o LabeledContent punha os curtos
    /// numa linha só).
    private func comando(_ nome: String, _ c: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(nome)
            HStack {
                Text(c).font(.system(.callout, design: .monospaced)).textSelection(.enabled)
                    .foregroundStyle(.secondary)
                    .lineLimit(1).truncationMode(.middle).help(c)
                Spacer(minLength: 8)
                BotaoCopiar(texto: c)
            }
        }
    }

    private static func sysctl(_ nome: String) -> String {
        var n = 0
        guard sysctlbyname(nome, nil, &n, nil, 0) == 0, n > 0 else { return "—" }
        var b = [UInt8](repeating: 0, count: n)
        guard sysctlbyname(nome, &b, &n, nil, 0) == 0 else { return "—" }
        return String(decoding: b.prefix { $0 != 0 }, as: UTF8.self)
    }
}

/// "Copiar" que vira "Copiado", em vermelho, por 1,5 s depois do clique. Os dois rótulos ocupam
/// o mesmo lugar: o botão não muda de largura.
private struct BotaoCopiar: View {
    let texto: String
    @State private var copiado = false

    var body: some View {
        // "Copiado!" por 3 s no botão vermelho do sistema (proeminente); a largura é a do maior texto.
        let rotulo = ZStack {
            Text("Copiado!").hidden()
            Text(copiado ? String(localized: "Copiado!") : String(localized: "Copiar"))
        }
        Group {
            if copiado {
                Button(action: copiar) { rotulo }
                    .buttonStyle(.borderedProminent)
                    .tint(.red)  // g9: cor do Ajustes (pedido do dono: botão vermelho ao copiar)
            } else {
                Button(action: copiar) { rotulo }
            }
        }
        .accessibilityLabel(copiado ? String(localized: "Copiado") : String(localized: "Copiar"))
    }

    private func copiar() {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(texto, forType: .string)
        withAnimation(.easeOut(duration: 0.15)) { copiado = true }
        Task {
            try? await Task.sleep(for: .seconds(3))
            withAnimation(.easeOut(duration: 0.15)) { copiado = false }
        }
    }
}

// Novidades da versão (dono, 26/10): folha "Bem-vindo ao SDTally 1.1" na primeira abertura depois
// de atualizar, uma vez por versão maior.menor. O texto vem no app e traduzido: buscar as notas no
// GitHub usaria a rede fora das atualizações e viria só em inglês.
import SwiftUI

nonisolated enum Novidades {
    /// Chave com a última versão (maior.menor) cujas novidades a pessoa já viu, ou que instalou do zero.
    static let chave = "ultimaVersaoNovidades"

    struct Item: Sendable {
        let simbolo, titulo, texto: String
    }

    /// Versões de correção (1.1.1) usam a lista da 1.1 e não mostram de novo: a chave guarda só maior.menor.
    static func itens(_ versao: String) -> [Item]? {
        switch versao {
        case "1.1": [
            Item(simbolo: "arrow.down.circle", titulo: String(localized: "Atualizações pelo próprio app"),
                 texto: String(localized: "Procura uma versão nova no github.com uma vez por dia e avisa na janela, na barra de menus e em Configurações › Versões. Só instala quando você clica: confere a assinatura e o SHA-256, troca o app e reabre, sem o “Abrir Mesmo Assim”.")),
            Item(simbolo: "network", titulo: String(localized: "Internet só para atualizações"),
                 texto: String(localized: "Nada sobre o seu disco é enviado, e dá para desligar em Configurações › Versões. O CLI e o helper root nunca acessam a internet e são atualizados junto com o app.")),
            Item(simbolo: "menubar.rectangle", titulo: String(localized: "Barra de menus com o menu do macOS"),
                 texto: String(localized: "O card da barra de menus agora é um menu nativo, com o mesmo vidro, borda e sombra dos menus do sistema.")),
        ]
        default: nil
        }
    }

    /// "1.1.0" vira "1.1".
    static func maiorMenor(_ versao: String) -> String {
        versao.split(separator: ".").prefix(2).joined(separator: ".")
    }

    /// Mostra quem já usava o app (viu as boas-vindas), numa versão com lista e mais nova que a
    /// última vista. Sem nada gravado e com as boas-vindas vistas, a pessoa veio de uma versão anterior.
    static func deveMostrar(versaoApp: String, vista: String?, viuBoasVindas: Bool,
                            temLista: (String) -> Bool = { itens($0) != nil }) -> Bool {
        let atual = maiorMenor(versaoApp)
        guard viuBoasVindas, temLista(atual) else { return false }
        guard let vista else { return true }
        return vista.compare(atual, options: .numeric) == .orderedAscending
    }

    static var versaoApp: String { maiorMenor(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "") }

    /// Grava a versão atual como vista: ao mostrar a folha e no Começar das boas-vindas.
    static func marcarVista() { UserDefaults.standard.set(versaoApp, forKey: chave) }

    /// O que mostrar nesta abertura, ou nil. Nunca fora do .app (Dev: fotos, testes).
    @MainActor static func paraMostrar(viuBoasVindas: Bool) -> String? {
        let v = versaoApp
        guard !Dev.ativo, deveMostrar(versaoApp: v, vista: UserDefaults.standard.string(forKey: chave), viuBoasVindas: viuBoasVindas)
        else { return nil }
        return v
    }
}

struct NovidadesView: View {
    let versao: String
    @Environment(\.dismiss) private var fechar

    var body: some View {
        VStack(spacing: 0) {
            VStack(spacing: 8) {
                IconeApp(tamanho: 64)
                Text("Bem-vindo ao SDTally \(versao)").font(.system(.title, design: .rounded, weight: .bold))
                Text("O que há de novo").foregroundStyle(.secondary)
            }
            .padding(.top, 28)
            // Altura fixa: a lista rola se o texto (ou a língua) passar dela.
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    ForEach(Novidades.itens(versao) ?? [], id: \.simbolo) { i in
                        HStack(alignment: .top, spacing: 14) {
                            Image(systemName: i.simbolo).font(.title2).foregroundStyle(.tint).frame(width: 32)
                                .accessibilityHidden(true)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(i.titulo).font(.headline)
                                Text(i.texto).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                            }
                        }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(20)
            }
            .frame(height: 300)
            .background(.quaternary.opacity(0.5), in: .rect(cornerRadius: 12))
            .padding(.horizontal, 32).padding(.top, 20)
            HStack {
                Spacer()
                Button("Continuar") { fechar() }.keyboardShortcut(.defaultAction)
            }
            .padding(20)
        }
        .frame(width: 560)
        .onAppear { Novidades.marcarVista() }
    }
}

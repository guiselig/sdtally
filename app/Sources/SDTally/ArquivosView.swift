// Maiores arquivos e Arquivos duplicados (só na medição completa; docs/json-schema.md,
// `largest_files` e `duplicates`). O app nunca apaga: só "Mostrar no Finder" e "Copiar caminho".
import AppKit
import SwiftUI

/// Nota de tamanho: o número é o alocado; esparso e comprimido dizem o lógico. nil no resto.
nonisolated func notaTamanho(_ a: Completo.ArquivoGrande) -> String? {
    guard let l = a.logicalBytes else { return nil }
    switch a.kindOfSize {
    case "sparse": return String(localized: "esparso, aparece como \(gb(l))")
    case "compressed": return String(localized: "comprimido, \(gb(l)) sem compressão")
    default: return nil
    }
}

/// Caminho para a tela: "~" no lugar da home, caracteres invisíveis à mostra.
nonisolated func rotuloArquivo(_ p: String, home: String) -> String {
    visivel(p.hasPrefix(home + "/") ? "~" + p.dropFirst(home.count) : p)
}

struct MaioresArquivos: View {
    let arquivos: [Completo.ArquivoGrande]
    let home: String

    var body: some View {
        Bloco(titulo: String(localized: "Maiores arquivos"), dica: String(localized: "Espaço alocado no disco pelo APFS")) {
            let visiveis = arquivos.prefix(5), resto = arquivos.dropFirst(5)
            VStack(alignment: .leading, spacing: 6) {
                ForEach(Array(visiveis.enumerated()), id: \.offset) { _, a in linha(a) }
                if !resto.isEmpty {
                    DisclosureGroup("e mais \(resto.count)") {
                        VStack(alignment: .leading, spacing: 6) {
                            ForEach(Array(resto.enumerated()), id: \.offset) { _, a in linha(a) }
                        }
                    }
                    .font(.callout)
                }
            }
        }
    }

    private func linha(_ a: Completo.ArquivoGrande) -> some View {
        LinhaArquivo(caminho: a.path, home: home, nota: notaTamanho(a)) {
            Text(gb(a.bytes)).monospacedDigit()
        }
    }
}

struct ArquivosDuplicados: View {
    let d: Completo.Duplicados
    let home: String

    var body: some View {
        Bloco(titulo: String(localized: "Arquivos duplicados"), fonte: String(localized: "conteúdo, byte a byte")) {
            VStack(alignment: .leading, spacing: 12) {
                if d.groups.isEmpty {
                    Text("Nenhuma cópia idêntica de 100 MB ou mais.").foregroundStyle(.secondary)
                } else {
                    ForEach(Array(d.groups.enumerated()), id: \.offset) { _, g in
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Cópias: \(g.copies) · \(gb(g.bytesEach)) cada · \(gb(g.wastedBytes)) a mais")
                                .font(.callout.weight(.semibold)).monospacedDigit()
                            ForEach(g.paths, id: \.self) { p in
                                LinhaArquivo(caminho: p, home: home, nota: nil) { EmptyView() }
                            }
                        }
                    }
                    Label("Antes de remover uma cópia, confira qual delas os apps usam.", systemImage: "info.circle")
                        .font(.callout).foregroundStyle(.secondary)
                }
                if d.state == "partial" {
                    Label("Comparação parcial: pode haver mais cópias além destas.", systemImage: "clock")
                        .font(.callout).foregroundStyle(.secondary).help(d.reason ?? "")
                }
            }
        }
    }
}

/// Ícone, caminho (com nota opcional), valor e os botões de só leitura.
private struct LinhaArquivo<Valor: View>: View {
    let caminho: String
    let home: String
    let nota: String?
    @ViewBuilder var valor: Valor

    var body: some View {
        let abs = absoluto(caminho, home: home)
        HStack(spacing: 8) {
            Image(nsImage: NSWorkspace.shared.icon(forFile: abs)).resizable().frame(width: 16, height: 16)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 0) {
                Text(rotuloArquivo(caminho, home: home)).font(.callout.monospaced()).lineLimit(1).truncationMode(.middle)
                if let nota { Text(nota).font(.caption).foregroundStyle(.secondary) }
            }
            .help(abs)
            Spacer(minLength: 8)
            valor
            Button("Mostrar no Finder") { NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: abs)]) }
                .controlSize(.small)
        }
        .contextMenu { MenuArquivo(caminho: abs) }
    }
}

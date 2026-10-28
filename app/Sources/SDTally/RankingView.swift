// Pastas maiores: o ranking inteiro da medição do alternador (automática ou completa). Duplo clique numa pasta abre
// o detalhe dela (`sdtally <caminho> --json`, sem senha), e assim por diante.
import AppKit
import SwiftUI

struct RankingView: View {
    @Environment(Painel.self) private var painel
    // Aqui e não em Ranking: sobrevivem a entrar numa pasta e voltar.
    @State private var filtro = ""
    @State private var ordem = [KeyPathComparator(\LinhaTop.bytes, order: .reverse)]

    /// Sem NavigationStack: ele poria uma barra de ferramentas (voltar e título) na janela. `painel.pilha`
    /// continua sendo o caminho; a última pasta é a tela, e o "voltar" é o botão do RamoView ou ⌘[.
    var body: some View {
        if let caminho = painel.pilha.last {
            RamoView(caminho: caminho).id(caminho)
        } else if let c = painel.dados {
            Ranking(c: c, filtro: $filtro, ordem: $ordem)
        } else {
            SemMedicao(simbolo: "list.number")
        }
    }
}

private struct Ranking: View {
    @Environment(Painel.self) private var painel
    let c: Completo
    @Binding var filtro: String
    @Binding var ordem: [KeyPathComparator<LinhaTop>]
    @State private var apelidoDe: String?

    var body: some View {
        let maximo = c.top.map(\.bytes).max() ?? 1
        let linhas = c.top
            .filter { l in filtro.isEmpty || ([l.path ?? nomeTermo(l.term)] + [l.path.flatMap { painel.apelidos[$0] }].compactMap { $0 })
                .contains { $0.localizedCaseInsensitiveContains(filtro) } }
            .sorted(using: ordem)
        Table(linhas, sortOrder: $ordem) {
            TableColumn("Nome") { l in NomeLinha(l: l, home: c.home, apelido: l.path.flatMap { painel.apelidos[$0] }) }
                .width(min: 150, ideal: 170)
            TableColumn("Ocupa", value: \.bytes) { l in
                HStack(spacing: 8) {
                    Text((l.lowerBound ? "≥ " : "") + gb(l.bytes)).monospacedDigit().frame(minWidth: 72, alignment: .trailing)
                    BarraOcupa(bytes: l.bytes, privado: l.privateBytes, maximo: maximo, cor: termo(l.term).cor)
                }
                .help("Espaço alocado no disco pelo APFS")
            }
            .width(min: 180, ideal: 180)
            TableColumn("Libera se apagar", value: \.livre) { l in
                Text(l.privateBytes.map(gb) ?? "—").monospacedDigit().foregroundStyle(.secondary)
                    .help(l.privateBytes == nil ? String(localized: "Só a travessia mede") : String(localized: "O que voltaria ao livre se só isto fosse apagado hoje"))
            }
            .width(min: 100, ideal: 100)
            TableColumn("Fonte") { l in
                Text(nomeFonte(l.source)).font(.caption).foregroundStyle(.secondary).help(l.source)
            }
            .width(min: 80, ideal: 100)
        }
        .contextMenu(forSelectionType: LinhaTop.ID.self) { ids in
            if let l = linha(ids), let p = l.path {
                if l.folded != true { MenuApelido(caminho: p, editando: $apelidoDe); Divider() }
                MenuArquivo(caminho: absoluto(p, home: c.home))
            }
        } primaryAction: { ids in
            if let l = linha(ids), let p = l.path, l.term == "tree", l.folded != true {
                painel.pilha.append(absoluto(p, home: c.home))
            }
        }
        // Sem o fundo próprio da tabela (mais escuro): a cor da janela, igual ao cabeçalho.
        .scrollContentBackground(.hidden)
        .safeAreaInset(edge: .top, spacing: 0) { faixas }
        .navigationTitle(Secao.ranking.titulo)
        .editorApelido($apelidoDe)
    }

    /// O filtro vive no conteúdo (sem barra de ferramentas na janela).
    private var campoFiltro: some View {
        HStack(spacing: 6) {
            Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
            TextField("Filtrar por caminho", text: $filtro).textFieldStyle(.plain)
            if !filtro.isEmpty {
                Button { filtro = "" } label: { Image(systemName: "xmark.circle.fill") }
                    .buttonStyle(.plain).foregroundStyle(.secondary).help("Limpar o filtro")
            }
        }
        .padding(.horizontal, 8).padding(.vertical, 5)
        .background(.quaternary, in: .rect(cornerRadius: 7))
    }

    @ViewBuilder private var faixas: some View {
        VStack(alignment: .leading, spacing: 6) {
            campoFiltro
            if let s = c.top.first(where: { $0.term == "snapshots" }), let n = s.count, n > 0 {
                Label("\(n) snapshots locais · \(gb(s.bytes))", systemImage: "clock.arrow.circlepath")
                    .help("Enquanto existirem, apagar um arquivo pode liberar menos do que ele ocupa: compare Ocupa com Libera se apagar.")
            }
            Text(painel.manual ? String(localized: "\(c.top.count) linhas · medido às \(hora(c.generatedAt))") : String(localized: "\(c.top.count) linhas"))
                .foregroundStyle(.secondary)
                .help("Do volume de dados inteiro, inclusive o que o Ajustes põe em outras linhas. Duplo clique numa pasta abre o que há dentro.")
        }
        .font(.callout)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 16).padding(.vertical, 8)
        .background(Color(nsColor: .windowBackgroundColor))  // a cor da janela, a mesma do alternador em cima
    }

    private func linha(_ ids: Set<LinhaTop.ID>) -> LinhaTop? {
        guard let id = ids.first else { return nil }
        return c.top.first { $0.id == id }
    }
}

private extension LinhaTop {
    /// Para ordenar: sem medida vai para o fim.
    var livre: Int64 { privateBytes ?? -1 }
}

/// Ícone do Finder + caminho com "~"; com apelido, ele em cima e o caminho embaixo, em cinza.
private struct NomeLinha: View {
    let l: LinhaTop
    let home: String
    let apelido: String?

    var body: some View {
        if let p = l.path {
            let abs = absoluto(p, home: home)
            HStack(spacing: 6) {
                Image(nsImage: NSWorkspace.shared.icon(forFile: abs)).resizable().frame(width: 16, height: 16)
                VStack(alignment: .leading, spacing: 0) {
                    // O caminho como o JSON traz ("~/Library"): a pasta inteira, sem nome solto e pai cinza.
                    if let apelido {
                        Text(verbatim: visivel(apelido)).lineLimit(1).truncationMode(.tail)
                        Text(rotulo(l, home: home)).font(.caption2).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                    } else {
                        Text(rotulo(l, home: home)).lineLimit(1).truncationMode(.middle)
                    }
                    if let o = l.owner {
                        Text(nomeDono(o)).font(.caption2).foregroundStyle(.secondary).lineLimit(1).truncationMode(.tail)
                            .help(fonteDono(o.source))
                    }
                }
            }
            .help(abs)
        } else {
            Label(rotulo(l, home: home), systemImage: termo(l.term).simbolo)
        }
    }
}

struct MenuArquivo: View {
    let caminho: String
    var body: some View {
        Button("Mostrar no Finder") { NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: caminho)]) }
        Button("Copiar caminho") {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(caminho, forType: .string)
        }
    }
}

/// "Dar apelido…" e, com apelido, "Tirar apelido" (Apelidos). `caminho` como no JSON ("~/…"); o
/// campo abre no editorApelido da tela, por `editando`.
struct MenuApelido: View {
    @Environment(Painel.self) private var painel
    let caminho: String
    @Binding var editando: String?
    var body: some View {
        Button("Dar apelido…") { editando = caminho }
        if painel.apelidos[caminho] != nil {
            Button("Tirar apelido") { painel.darApelido(nil, para: caminho) }
        }
    }
}

extension View {
    /// Alerta com o campo do apelido da pasta em `editando` (caminho do JSON); fechar volta a nil.
    func editorApelido(_ editando: Binding<String?>) -> some View { modifier(EditorApelido(editando: editando)) }
}

private struct EditorApelido: ViewModifier {
    @Environment(Painel.self) private var painel
    @Binding var editando: String?
    @State private var texto = ""
    func body(content: Content) -> some View {
        content
            .alert("Apelido da pasta", isPresented: Binding(get: { editando != nil }, set: { if !$0 { editando = nil } }),
                   presenting: editando) { p in
                TextField("Apelido", text: $texto)
                Button("Salvar") { painel.darApelido(texto, para: p) }
                Button("Cancelar", role: .cancel) {}
            } message: { p in
                Text(verbatim: visivel(p))
            }
            .onChange(of: editando) { _, p in texto = p.flatMap { painel.apelidos[$0] } ?? "" }
    }
}

/// Um nível de pastas de um caminho, medido na hora sem senha. Voltar cancela a leitura.
struct RamoView: View {
    @Environment(Painel.self) private var painel
    let caminho: String
    @State private var ramo: Ramo?
    @State private var erro: String?
    @State private var tentativa = 0
    private var nome: String { FileManager.default.displayName(atPath: caminho) }

    var body: some View {
        Group {
            if let r = ramo {
                tabela(r)
            } else if let e = erro {
                ContentUnavailableView {
                    Label("Não deu para ler \(nome)", systemImage: "exclamationmark.triangle")
                } description: {
                    Text(e)
                } actions: {
                    Button("Tentar de novo") { erro = nil; tentativa += 1 }
                }
            } else {
                TimelineView(.periodic(from: .now, by: 1)) { _ in
                    ProgressView("Lendo \(nome)…")
                }
            }
        }
        // Erro e carregando ocupam a tela toda: o Voltar fica no topo, não no meio.
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .navigationTitle(nome)
        .scrollContentBackground(.hidden)
        // Sem barra de ferramentas na janela, o "voltar" do NavigationStack vive aqui.
        .safeAreaInset(edge: .top, spacing: 0) {
            HStack(spacing: 8) {
                Button { painel.pilha.removeLast() } label: { Label("Voltar", systemImage: "chevron.left") }
                    .help("Voltar (⌘[)")
                Text(nome).font(.headline).lineLimit(1).truncationMode(.middle)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 16).padding(.vertical, 8)
            .background(Color(nsColor: .windowBackgroundColor))  // a cor da janela, a mesma do alternador em cima
        }
        .task(id: "\(caminho)|\(tentativa)") {
            do { ramo = try await painel.ramo(caminho) } catch { if !Task.isCancelled { erro = error.localizedDescription } }
        }
    }

    private func tabela(_ r: Ramo) -> some View {
        let maximo = r.children.map(\.bytes).max() ?? 1
        return Table(r.children) {
            TableColumn("Nome") { f in
                HStack(spacing: 6) {
                    Image(nsImage: NSWorkspace.shared.icon(forFile: f.path)).resizable().frame(width: 16, height: 16)
                    Text(f.looseFiles ? String(localized: "Arquivos soltos nesta pasta") : f.name).lineLimit(1).truncationMode(.middle)
                    if f.protected { Image(systemName: "lock").font(.caption2).foregroundStyle(.secondary).help("Pasta protegida: medida por dir-stats, sem abrir") }
                }
            }
            .width(min: 150, ideal: 170)
            TableColumn("Ocupa") { f in
                HStack(spacing: 8) {
                    Text(gb(f.bytes)).monospacedDigit().frame(minWidth: 72, alignment: .trailing)
                    BarraOcupa(bytes: f.bytes, privado: f.privateBytes, maximo: maximo)
                }
            }
            .width(min: 180, ideal: 180)
            TableColumn("Libera se apagar") { f in
                Text(f.privateBytes.map(gb) ?? "—").monospacedDigit().foregroundStyle(.secondary)
            }
            .width(min: 100, ideal: 100)
            TableColumn("Fonte") { f in Text(nomeFonte(f.source)).font(.caption).foregroundStyle(.secondary) }
                .width(min: 80, ideal: 100)
        }
        .contextMenu(forSelectionType: Ramo.Filho.ID.self) { ids in
            if let f = filho(ids, r) { MenuArquivo(caminho: f.path) }
        } primaryAction: { ids in
            if let f = filho(ids, r), !f.looseFiles, !f.protected { painel.pilha.append(f.path) }
        }
        .safeAreaInset(edge: .bottom, spacing: 0) { rodape(r) }
    }

    private func rodape(_ r: Ramo) -> some View {
        let mais = r.total.lowerBound ? "≥ " : ""
        var partes = [String(localized: "Total \(mais)\(gb(r.total.bytes))"),
                      String(localized: "\(r.total.files.formatted(.number.locale(localApp))) arquivos")]
        if r.interrupted { partes.append(String(localized: "interrompido")) }
        if r.protectedWithoutSize > 0 { partes.append(String(localized: "\(r.protectedWithoutSize) pastas sem permissão e sem dir-stats: não medidas")) }
        if r.readErrors > 0 { partes.append(String(localized: "\(r.readErrors) erros de leitura")) }
        return Text(partes.joined(separator: " · "))
            .font(.callout).foregroundStyle(.secondary).monospacedDigit()
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 16).padding(.vertical, 8)
            .background(Color(nsColor: .windowBackgroundColor))  // a cor da janela, a mesma do alternador em cima
    }

    private func filho(_ ids: Set<Ramo.Filho.ID>, _ r: Ramo) -> Ramo.Filho? {
        guard let id = ids.first else { return nil }
        return r.children.first { $0.id == id }
    }
}

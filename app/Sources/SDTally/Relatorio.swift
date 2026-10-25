// Exportar (spec-fase5 §4.3, spec-fase6 §5.8). O NSSavePanel tem o toggle "Trocar nomes pessoais",
// que começa ligado. Desligado, o JSON são os bytes crus do sdtally, sem transformar; ligado, é o
// que o `sdtally --redact names --stdin` devolve para eles, e o PDF sai desse JSON redigido.
// PDF: uma página por seção, 595 pt de largura (a altura cresce com o conteúdo), tema claro, sem autor.
// API estável:
//   Relatorio.exportarJSON(_ painel: Painel)   NSSavePanel; chama gravarJSON
//   Relatorio.exportarPDF(_ painel: Painel)    NSSavePanel; chama gerarPDF
//   Relatorio.gravarJSON(_:em:redigir:bin:) async throws    sem redigir, painel.completoDados sem mudar nada
//   Relatorio.gerarPDF(_:destino:pontos:redigir:bin:) async throws   páginas de RelatorioView, sem autor
//   RelatorioView(painel:pagina:pontos:completo:)   uma página de 595 pt, tema claro
// Só VStack, Grid e formas: o ImageRenderer não desenha controles do AppKit.
import AppKit
import Charts
import SwiftUI
import UniformTypeIdentifiers

enum Relatorio {
    static let aviso = String(localized: "O arquivo traz seu nome de usuário e nomes de pastas da sua home (por exemplo, ~/Documents). Revise antes de compartilhar.")
    static let avisoRedigido = String(localized: "Seu nome de usuário e os nomes de pastas pessoais saem trocados. Pastas conhecidas e nomes de apps mantêm o nome.")
    static let rotuloRedigir = String(localized: "Trocar nomes pessoais (recomendado para compartilhar)")

    static func exportarJSON(_ painel: Painel) {
        guard painel.completoDados != nil, let e = escolher(.json) else { return }
        Task {
            do { try await gravarJSON(painel, em: e.url, redigir: e.redigir) } catch { NSAlert(error: error).runModal() }
        }
    }

    /// Sem redigir, os bytes do stdout, sem decodificar nem reformatar. `bin` nil: o sdtally embutido.
    static func gravarJSON(_ painel: Painel, em url: URL, redigir: Bool = false, bin: String? = nil) async throws {
        guard var dados = painel.completoDados else { throw CocoaError(.fileWriteUnknown) }
        if redigir { dados = try await Executor.redigir(dados, bin: bin) }
        try dados.write(to: url, options: .atomic)
    }

    static func exportarPDF(_ painel: Painel) {
        guard painel.completo != nil, let e = escolher(.pdf) else { return }
        Task {
            do { try await gerarPDF(painel, destino: e.url, redigir: e.redigir) } catch { NSAlert(error: error).runModal() }
        }
    }

    /// Nome "SDTally AAAA-MM-DD", em Downloads, com o toggle e o aviso do §5.8. Não abre o arquivo depois.
    private static func escolher(_ tipo: UTType) -> (url: URL, redigir: Bool)? {
        let p = NSSavePanel()
        p.nameFieldStringValue = "SDTally " + Date.now.formatted(Date.ISO8601FormatStyle(timeZone: .current).year().month().day())
        p.allowedContentTypes = [tipo]
        p.directoryURL = FileManager.default.urls(for: .downloadsDirectory, in: .userDomainMask).first
        let t = ToggleRedigir(p)
        guard p.runModal() == .OK, let url = p.url else { return nil }
        return (url, t.ligado)
    }

    /// `pontos` nil lê o pontos.jsonl de Historico.pastaLeitura (os testes passam os da fixture).
    /// Com `redigir`, as páginas saem do JSON redigido pelo sdtally, nunca de painel.completo.
    static func gerarPDF(_ painel: Painel, destino: URL, pontos: [Ponto]? = nil, redigir: Bool = false, bin: String? = nil) async throws {
        guard var c = painel.completo else { throw CocoaError(.fileWriteUnknown) }
        if redigir {
            guard let dados = painel.completoDados else { throw CocoaError(.fileWriteUnknown) }
            c = try decodificar(Completo.self, await Executor.redigir(dados, bin: bin), kind: nil)
        }
        let pts = pontos ?? Historico.pastaLeitura(env: Foto.env, bundleID: Bundle.main.bundleIdentifier).map(Historico.lerPontos) ?? []
        // Só o criador: sem autor, título nem assunto.
        let info = [kCGPDFContextCreator as String: "SDTally"] as CFDictionary
        guard let pdf = CGContext(destino as CFURL, mediaBox: nil, info) else { throw CocoaError(.fileWriteUnknown) }
        NSAppearance(named: .aqua)?.performAsCurrentDrawingAppearance {
            for n in 1...(pts.isEmpty ? 4 : 5) {
                let r = ImageRenderer(content: RelatorioView(painel: painel, pagina: n, pontos: pts, completo: c).environment(\.colorScheme, .light))
                r.render { tamanho, desenhar in
                    var caixa = CGRect(origin: .zero, size: tamanho)
                    pdf.beginPage(mediaBox: &caixa)
                    desenhar(pdf)
                    pdf.endPage()
                }
            }
        }
        pdf.closePDF()
    }
}

/// Toggle do accessory do NSSavePanel: começa ligado e troca o `message` junto.
private final class ToggleRedigir: NSObject {
    private let painel: NSSavePanel
    private let botao = NSButton(checkboxWithTitle: Relatorio.rotuloRedigir, target: nil, action: nil)
    var ligado: Bool { botao.state == .on }

    init(_ p: NSSavePanel) {
        painel = p
        super.init()
        botao.state = .on
        botao.target = self
        botao.action = #selector(mudou)
        let pilha = NSStackView(views: [botao])
        pilha.edgeInsets = NSEdgeInsets(top: 10, left: 20, bottom: 10, right: 20)
        pilha.frame.size = pilha.fittingSize
        p.accessoryView = pilha
        mudou()
    }

    @objc private func mudou() { painel.message = ligado ? Relatorio.avisoRedigido : Relatorio.aviso }
}

struct RelatorioView: View {
    let painel: Painel
    var pagina = 1
    var pontos: [Ponto] = []
    /// A medição das páginas (a redigida, no exportar); nil: painel.completo.
    var completo: Completo? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            if let c = completo ?? painel.completo {
                switch pagina {
                case 1: Capa(c: c, telaRapida: painel.telaBoa)
                case 2: PaginaArmazenamento(c: c)
                case 3: Maiores(c: c)
                case 4: Passos(c: c)
                default: PaginaHistorico(pontos: pontos)
                }
            }
            Spacer(minLength: 0)
            Text("SDTally · só lê, sem rede · página \(pagina) de \(pontos.isEmpty ? 4 : 5)")
                .font(.caption2).foregroundStyle(.tertiary)
        }
        .padding(40)
        .frame(width: 595, alignment: .topLeading)
        .frame(minHeight: 842, alignment: .top)
        .background(.white)
        .foregroundStyle(.black)
    }
}

// MARK: - Peças

private func titulo(_ s: String) -> some View { Text(s).font(.title2.weight(.semibold)) }
/// Subtítulo junto do conteúdo; o espaço maior fica entre as seções.
private func secao<C: View>(_ s: String, @ViewBuilder _ c: () -> C) -> some View {
    VStack(alignment: .leading, spacing: 8) { Text(s).font(.headline); c() }
}
private func valor(_ b: Int64?, _ state: String = "measured") -> String {
    b.map { (state == "partial" ? "≥ " : "") + gb($0) } ?? "—"
}

/// Nome, valor e fonte, alinhados numa Grid de três colunas.
private func linha(_ nome: String, _ v: String, _ fonte: String = "", cor: Color? = nil, tracejado: Bool = false, forte: Bool = false) -> some View {
    GridRow {
        HStack(spacing: 6) {
            if tracejado {
                Circle().strokeBorder(.secondary, style: StrokeStyle(lineWidth: 1, dash: [2, 2])).frame(width: 8, height: 8)
            } else if let cor { Circle().fill(cor).frame(width: 8, height: 8) }
            Text(nome).lineLimit(1).truncationMode(.middle)
        }
        Text(v).monospacedDigit().fontWeight(forte ? .semibold : .regular).gridColumnAlignment(.trailing)
        Text(fonte).font(.caption).foregroundStyle(.secondary)
    }
}

private func tabela<C: View>(@ViewBuilder _ c: () -> C) -> some View {
    Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 16, verticalSpacing: 5) { c() }.font(.callout)
}

// MARK: - 1. Capa: cabeçalho, barra do Ajustes e a conta

private struct Capa: View {
    let c: Completo
    let telaRapida: TelaLida?
    /// Em bitmap: no contexto de PDF, os gradientes do IconeApp saíam pretos.
    private static let icone: NSImage? = {
        let r = ImageRenderer(content: IconeApp(tamanho: 48))
        r.scale = 4
        return r.nsImage
    }()

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            if let i = Self.icone { Image(nsImage: i).resizable().frame(width: 48, height: 48) }
            VStack(alignment: .leading, spacing: 2) {
                Text("SDTally").font(.system(.title, design: .rounded, weight: .bold))
                Text("Relatório de armazenamento").foregroundStyle(.secondary)
            }
            Spacer()
            VStack(alignment: .trailing, spacing: 2) {
                Text("Medido em \(Armazenamento.dataCurta(c.generatedAt))")
                Text("macOS \(c.macosVersion ?? "?") · \(c.tool)")
                Text(c.root ? String(localized: "como administrador") : String(localized: "sem senha de administrador"))
            }
            .font(.caption).foregroundStyle(.secondary)
        }
        if !c.complete {
            Faixa(simbolo: "exclamationmark.triangle", texto: String(localized: "Medição incompleta: o que não abriu fica bloqueado pelo macOS."))
        }
        // A tela que veio com a medição; sem ela, a última lida pelo app.
        let tela = c.settingsScreen.screen.flatMap { $0.state == "not_measured" ? nil : $0 } ?? telaRapida?.tela
        let macos = c.settingsScreen.screen?.state == "not_measured" ? telaRapida?.macos ?? c.settingsScreen.macos : c.settingsScreen.macos
        if let t = tela {
            VStack(alignment: .leading, spacing: 10) {
                HStack(alignment: .firstTextBaseline) {
                    Text("Como o Ajustes divide o disco").font(.headline)
                    Spacer()
                    if let u = t.used, let cap = t.capacity { Text("\(gb(u)) de \(gb(cap)) usados").monospacedDigit().foregroundStyle(.secondary) }
                }
                let segs = segmentos(t, macos: macos)
                BarraView(segmentos: segs, altura: 16)
                Grid(alignment: .leading, horizontalSpacing: 24, verticalSpacing: 4) {
                    ForEach(Array(stride(from: 0, to: segs.count, by: 2)), id: \.self) { i in
                        GridRow {
                            ForEach(segs[i..<min(i + 2, segs.count)]) { s in
                                HStack(spacing: 6) {
                                    Circle().fill(s.cor).frame(width: 8, height: 8)
                                    Text(s.nome)
                                    Text(gb(s.bytes)).monospacedDigit().foregroundStyle(.secondary)
                                }
                            }
                        }
                    }
                }
                .font(.callout)
                Text(hora(t.loggedAt).map { String(localized: "Como no Ajustes às \($0) · log do Ajustes e do serviço; macOS pelo diskutil") }
                    ?? String(localized: "Como no Ajustes · log do Ajustes e do serviço; macOS pelo diskutil"))
                    .font(.caption).foregroundStyle(.secondary)
            }
            if let sd = t.systemData {
                VStack(alignment: .leading, spacing: 10) {
                    Text("Dados do Sistema").font(.title3.weight(.semibold))
                    Text(gb(sd)).font(.heroi).tracking(-0.5)
                    Text("A Apple não mede este número: ela subtrai. É o que sobra do usado depois do macOS e das outras linhas.")
                        .font(.callout).foregroundStyle(.secondary)
                    tabela {
                        linha(String(localized: "Usado"), valor(t.used), String(localized: "log do Ajustes"))
                        linha("− macOS", valor(macos.bytes), nomeFonte(macos.source))
                        linha(String(localized: "− outras linhas"), valor(t.categoriesTotal), String(localized: "log do serviço"))
                        Divider()
                        linha(String(localized: "= Dados do Sistema"), gb(sd), String(localized: "diferença"), forte: true)
                    }
                }
                .cartao(destaque: Marca.dados.opacity(0.5))
            }
        }
        let u = c.dataVolume.unexplained
        HStack(alignment: .top, spacing: 8) {
            FichaKPI(titulo: String(localized: "Livre"), bytes: Armazenamento.principal(c.containers ?? [])?.free, fonte: nomeFonte("diskutil_apfs_list"))
            FichaKPI(titulo: String(localized: "O macOS libera sozinho"), bytes: c.informational?.purgeable?.bytes, fonte: nomeFonte("cachedelete"), variacao: String(localized: "já dentro do usado"))
            FichaKPI(titulo: nomeResto(c), bytes: u.bytes, fonte: nomeFonte("difference"),
                     variacao: u.percentOfUsed.map { String(localized: "\($0.formatted(.number.precision(.fractionLength(1)).locale(localApp))) % do usado") })
        }
    }

    private func segmentos(_ t: Tela, macos: Medida) -> [Segmento] {
        var s = (t.categories ?? []).filter { $0.bytes > 0 }.map {
            let k = categoria($0.id)
            return Segmento(id: $0.id, nome: k.nome, bytes: $0.bytes, cor: k.cor)
        }
        if let sd = t.systemData { s.append(Segmento(id: "sd", nome: String(localized: "Dados do Sistema"), bytes: sd, cor: Marca.dados)) }
        if let m = macos.bytes { s.append(Segmento(id: "macos", nome: "macOS", bytes: m, cor: .gray.opacity(0.5))) }
        if let a = t.available { s.append(Segmento(id: "livre", nome: String(localized: "Disponível"), bytes: a, cor: Color(nsColor: .quaternaryLabelColor))) }
        return s
    }
}

// MARK: - 2. Armazenamento

private struct PaginaArmazenamento: View {
    let c: Completo

    var body: some View {
        titulo(String(localized: "Armazenamento"))
        let cs = c.containers ?? []
        if let p = Armazenamento.principal(cs) {
            secao(String(localized: "Container do disco interno")) {
                tabela {
                    linha(String(localized: "Capacidade"), valor(p.ceiling), "diskutil")
                    linha(String(localized: "Livre"), valor(p.free), "diskutil")
                    Divider()
                    ForEach(Array(p.volumes.enumerated()), id: \.offset) { _, v in
                        linha(nomePapel(v.roles.first).nome, valor(v.capacityInUse), v.inSettingsMacos ? String(localized: "entra no macOS da tela") : "")
                    }
                    Divider()
                    linha(String(localized: "macOS da tela"), gb(Armazenamento.macosDaTela(cs)), String(localized: "soma dos volumes marcados"), forte: true)
                    if let s = c.swap?.total {
                        linha(String(localized: "Swap (no VM)"), gb(s), nomeFonte("sysctl_vm_swapusage"))
                        linha(String(localized: "Resto do VM"), valor(Armazenamento.restoVM(cs, swap: c.swap)), String(localized: "diferença"))
                    }
                }
            }
        }
        let v = c.dataVolume
        let segs = v.terms.compactMap { t in t.bytes.map { Segmento(id: t.key, nome: nomeTermo(t.key), bytes: $0, cor: termo(t.key).cor) } }
            + (v.unexplained.bytes.flatMap { $0 > 0 ? [Segmento(id: "sem", nome: nomeResto(c), bytes: $0, cor: .clear, tracejado: true)] : nil } ?? [])
        secao(String(localized: "Conta do volume de dados")) {
            BarraView(segmentos: segs, altura: 12)
            tabela {
                linha(String(localized: "Usado no volume"), valor(v.used.bytes, v.used.state), nomeFonte(v.used.source), forte: true)
                ForEach(v.terms) { t in
                    linha(nomeTermo(t.key), valor(t.bytes, t.state), t.bytes == nil ? String(localized: "não medido") : nomeFonte(t.source), cor: termo(t.key).cor)
                }
                linha(String(localized: "Bloqueado pelo macOS"), valor(v.unexplained.bytes), String(localized: "diferença"), tracejado: true)
            }
        }
        let partes = Armazenamento.partes(c.top)
        if !partes.isEmpty {
            secao(String(localized: "Partes do sistema no volume Data")) {
                tabela {
                    ForEach(partes) { p in linha(p.nome, (p.piso ? "≥ " : "") + gb(p.bytes), p.prefixo) }
                }
            }
        }
        if let pg = c.informational?.purgeable, let b = pg.bytes {
            secao(String(localized: "O que o macOS libera sozinho")) {
                tabela {
                    linha(String(localized: "Total"), gb(b), nomeFonte(pg.source), forte: true)
                    ForEach(Array((pg.topServices ?? []).filter { $0.bytes > 0 }.enumerated()), id: \.offset) { _, s in
                        linha(nomeServico(s.name), gb(s.bytes))
                    }
                }
            }
        }
    }
}

// MARK: - 3. As 25 maiores

private struct Maiores: View {
    let c: Completo

    var body: some View {
        titulo(String(localized: "O que mais pesa"))
        Text("As 25 maiores linhas da medição. “Libera se apagar” desconta o que é compartilhado com clones e snapshots.")
            .font(.callout).foregroundStyle(.secondary)
        Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 12, verticalSpacing: 5) {
            GridRow {
                Text("#"); Text("Pasta"); Text("Ocupa").gridColumnAlignment(.trailing); Text("Libera").gridColumnAlignment(.trailing)
            }
            .font(.caption.weight(.semibold)).foregroundStyle(.secondary)
            Divider()
            ForEach(Array(c.top.prefix(25).enumerated()), id: \.offset) { i, l in
                GridRow {
                    Text("\(i + 1)").foregroundStyle(.secondary).monospacedDigit()
                    HStack(spacing: 6) {
                        Circle().fill(termo(l.term).cor).frame(width: 7, height: 7)
                        Text(rotulo(l, home: c.home)).lineLimit(1).truncationMode(.middle)
                    }
                    Text((l.lowerBound ? "≥ " : "") + gb(l.bytes)).monospacedDigit()
                    Text(l.privateBytes.map(gb) ?? "—").monospacedDigit().foregroundStyle(.secondary)
                }
            }
        }
        .font(.callout)
    }
}

// MARK: - 4. O que fazer

private struct Passos: View {
    let c: Completo

    var body: some View {
        titulo(String(localized: "O que fazer"))
        Text(rodapeConselhos).font(.callout).foregroundStyle(.secondary)
        let a = ordenarConselhos(c.advice ?? [])
        if a.isEmpty { Text("Nenhuma recomendação nesta medição.").foregroundStyle(.secondary) }
        ForEach(Array(a.enumerated()), id: \.offset) { _, item in Item(c: c, item: item) }
    }

    private struct Item: View {
        let c: Completo
        let item: Conselho

        var body: some View {
            let n = Marca.nivel(item.level)
            let liberavel = ["safe", "cost", "check_first", "anomaly"].contains(item.level) && item.id != "deleted_open"
            let cs = caminhosPrincipais(item.paths, top: c.top)
            VStack(alignment: .leading, spacing: 4) {
                HStack(alignment: .firstTextBaseline) {
                    Image(systemName: n.simbolo).foregroundStyle(n.cor)
                    Text(conselhosPT[item.id]?.titulo ?? item.term.map(nomeTermo) ?? String(localized: "Recomendação do sdtally")).font(.headline)
                    Text(nomeNivel(item.level)).font(.caption.weight(.semibold)).foregroundStyle(n.cor)
                    Spacer()
                    if liberavel {
                        Text(item.privateBytes.map { String(localized: "libera \(gb($0))") } ?? String(localized: "libera ≤ \(gb(item.bytes))")).monospacedDigit().fontWeight(.semibold)
                    } else if item.bytes > 0 {
                        Text(gb(item.bytes)).monospacedDigit()
                    }
                }
                Text(textoMostrado(item, percentual: c.dataVolume.unexplained.percentOfUsed))
                    .font(.callout).fixedSize(horizontal: false, vertical: true)
                if !item.paths.isEmpty {
                    Text(cs.visiveis.joined(separator: " · ") + (cs.resto.isEmpty ? "" : " · " + String(localized: "e mais \(cs.resto.count)")))
                        .font(.caption.monospaced()).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                }
                if let cmd = comandoCopiavel(item) { Text("Comando: \(cmd)").font(.caption.monospaced()) }
            }
            .padding(.leading, 10)
            .overlay(alignment: .leading) { Rectangle().fill(n.cor).frame(width: 3) }
        }
    }
}

// MARK: - 5. Histórico

private struct PaginaHistorico: View {
    let pontos: [Ponto]

    var body: some View {
        let ps = pontos.compactMap { p in data(p.t).map { (quando: $0, p: p) } }
        titulo(String(localized: "Histórico"))
        if let a = ps.first, let b = ps.last {
            Text("De \(Armazenamento.dataCurta(a.p.t)) a \(Armazenamento.dataCurta(b.p.t)). A linha contínua é o usado menos o que o macOS libera sozinho; a picotada é Dados do Sistema, ligando só as leituras da tela.")
                .font(.callout).foregroundStyle(.secondary)
        }
        Chart {
            ForEach(Array(ps.enumerated()), id: \.offset) { _, x in
                if let u = x.p.usado {
                    LineMark(x: .value("Quando", x.quando), y: .value("GB", Double(u - (x.p.purgeable ?? 0)) / 1e9), series: .value("s", "usado"))
                        .foregroundStyle(termo("tree").cor)
                }
                if let sd = x.p.systemData {
                    LineMark(x: .value("Quando", x.quando), y: .value("GB", Double(sd) / 1e9), series: .value("s", "sistema"))
                        .foregroundStyle(Marca.dados)
                        .lineStyle(StrokeStyle(lineWidth: 1.5, dash: [4, 3]))
                }
            }
        }
        .chartYScale(domain: .automatic(includesZero: false))
        .chartYAxisLabel("GB")
        .environment(\.locale, localApp)
        .frame(height: 260)
        HStack(spacing: 16) {
            HStack(spacing: 6) { Rectangle().fill(termo("tree").cor).frame(width: 14, height: 2); Text("Usado − libera sozinho") }
            HStack(spacing: 6) { Circle().fill(Marca.dados).frame(width: 8, height: 8); Text("Dados do Sistema (Ajustes)") }
        }
        .font(.caption)
    }
}

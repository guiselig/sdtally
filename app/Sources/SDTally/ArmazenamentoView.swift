// Disco (antes Dados do Sistema e Armazenamento; spec-inicio §7.2, spec-fase5 §2.4): primeiro o disco
// inteiro (físico, com o nome do volume), depois o que explica Dados do Sistema, cada número com a fonte. Dono: A3.
// API estável:
//   ArmazenamentoView()   lê Painel do ambiente; só os containers com internal == true
//   DonutDisco            SectorMark do container (Data, macOS da tela, livre)
//   Armazenamento.*       contas puras (macOS da tela, resto do VM, partes do sistema)
import Charts
import SwiftUI

/// Contas puras da tela. O app não mede: só soma e agrupa o que o sdtally devolveu.
nonisolated enum Armazenamento {
    static func internos(_ cs: [Conteiner]) -> [Conteiner] { cs.filter { $0.internal == true } }

    /// O container interno com o volume Data de boot.
    static func principal(_ cs: [Conteiner]) -> Conteiner? { cs.first { $0.internal == true && $0.holdsData } }

    /// Containers do disco interno que não guardam o Data nem são o ISC (a cópia de recuperação).
    static func outrosInternos(_ cs: [Conteiner]) -> [Conteiner] { internos(cs).filter { !$0.isc && !$0.holdsData } }

    /// Σ capacity_in_use dos volumes que entram no macOS da tela (= settings_screen.macos).
    static func macosDaTela(_ cs: [Conteiner]) -> Int64 {
        internos(cs).flatMap(\.volumes).filter(\.inSettingsMacos).reduce(0) { $0 + ($1.capacityInUse ?? 0) }
    }

    static func vm(_ cs: [Conteiner]) -> Int64? {
        principal(cs)?.volumes.first { $0.roles.contains("VM") }?.capacityInUse
    }

    /// VM − swap (kernelcore e outros). nil sem as duas leituras ou se der negativo: diskutil e
    /// sysctl leem em momentos diferentes, e o swap muda no meio.
    static func restoVM(_ cs: [Conteiner], swap: Swap?) -> Int64? {
        guard let v = vm(cs), let s = swap?.total, v >= s else { return nil }
        return v - s
    }

    struct Parte: Identifiable, Sendable {
        let prefixo: String
        let nome: String
        let bytes: Int64
        /// Alguma linha é piso (`lower_bound`).
        let piso: Bool
        var id: String { prefixo }
    }

    /// Partes do sistema no volume Data (§2.4, item 9), na ordem da tela.
    static let grupos: [(prefixo: String, nome: String)] = [
        ("/private/var/db", String(localized: "Bancos de dados do sistema")),
        ("/private/var/folders", String(localized: "Temporários e caches por usuário")),
        ("/private/var/vm", String(localized: "Imagem de repouso")),
        ("/Library", String(localized: "Biblioteca do Mac")),
        ("/System", String(localized: "Sistema (lado Data)")),
        ("~/Library/Caches", String(localized: "Caches")),
        ("~/Library/Developer", String(localized: "Desenvolvedor")),
        ("~/Library/Containers", String(localized: "Containers dos apps")),
        ("~/Library/Application Support", String(localized: "Suporte dos apps")),
        ("~/.Trash", String(localized: "Lixo")),
    ]

    /// Soma as linhas do `top` por prefixo, por componente inteiro. Linha dobrada conta quando
    /// está dentro do prefixo; a ancestral (ex.: "demais itens de ~/Library") nunca. Grupo vazio sai.
    static func partes(_ top: [LinhaTop]) -> [Parte] {
        grupos.compactMap { g in
            let ls = top.filter { l in l.path.map { $0 == g.prefixo || $0.hasPrefix(g.prefixo + "/") } ?? false }
            guard !ls.isEmpty else { return nil }
            return Parte(prefixo: g.prefixo, nome: g.nome, bytes: ls.reduce(0) { $0 + $1.bytes }, piso: ls.contains(where: \.lowerBound))
        }
    }

    /// "02/10 às 08:12" de "2026-10-02 08:12".
    static func dataCurta(_ c: String) -> String {
        guard c.count >= 16 else { return c }
        return String(localized: "\(diaCurto(c)) às \(hora(c))")
    }

    /// "02/10" de "2026-10-02 08:12"; texto curto demais volta como veio.
    static func diaCurto(_ c: String) -> String {
        let a = Array(c)
        guard a.count >= 16 else { return c }
        return "\(String(a[8..<10]))/\(String(a[5..<7]))"
    }
}

/// Nome do volume de boot, como o Finder mostra ("Macintosh HD").
let nomeVolume = (try? URL(fileURLWithPath: "/").resourceValues(forKeys: [.volumeLocalizedNameKey]).volumeLocalizedName) ?? "Macintosh HD"

struct ArmazenamentoView: View {
    @Environment(Painel.self) private var painel

    var body: some View {
        // Seções com .id(<âncora>) (spec-fase6 §2.2): SDTALLY_ANCORA rola até uma delas.
        ScrollViewReader { proxy in
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                let r = painel.rapidoVivo
                let c = painel.completo
                Text(nomeVolume).font(.title2.weight(.semibold))
                // Físico: o rápido é mais novo; sem ele, o que veio na medição completa.
                let cs = r?.containers ?? c?.containers
                let swap = r?.containers != nil ? painel.swapVivo : c?.swap
                let macos = (r?.settingsScreen ?? c?.settingsScreen)?.macos
                if let cs {
                    Fisico(cs: cs, dados: painel.usoVivo ?? c?.dataVolume.used, macos: macos)
                } else if r == nil && c == nil {
                    ProgressView("Lendo o disco…").frame(maxWidth: .infinity)
                } else {
                    Bloco(titulo: String(localized: "Disco interno"), fonte: nomeFonte("statfs")) { Heroi() }
                }
                LivresView().id("livres")
                BarraDesdobravel().id("barra")
                if let cs { Containers(cs: cs, swap: swap, macos: macos) }
                if let ia = c?.settingsScreen.appleIntelligence ?? r?.settingsScreen.appleIntelligence { AppleIntelligence(ia: ia) }
                // Partes do sistema, snapshots, apagados abertos e reiniciar ficam em Dados do Sistema (SistemaView).
                if let c { RecursosView(c: c).id("recursos") }
                if let l = c?.largestFiles, !l.isEmpty, let c { MaioresArquivos(arquivos: l, home: c.home).id("maiores") }
                if let d = c?.duplicates, let c { ArquivosDuplicados(d: d, home: c.home).id("duplicados") }
                if let c {
                    InformativoBloco(c: c).id("informativo")
                    EstadoMedicao(c: c)
                }
                // Sem medição completa salva, o automático já traz a Lixeira e os metadados do APFS.
                NaoMedido(informativo: (c ?? painel.autoVivo)?.informational).id("naomedido")
            }
            .padding(24)
            .frame(maxWidth: 760)
            .frame(maxWidth: .infinity)
        }
        .task(id: painel.rapido?.generatedAt) {
            guard let a = painel.rolarPara ?? Foto.ancora ?? Foto.barra.map({ _ in "barra" }) else { return }
            painel.rolarPara = nil
            try? await Task.sleep(for: .milliseconds(300))
            proxy.scrollTo(a, anchor: .top)
        }
        }
    }
}

// MARK: - Peças

/// Cartão com título; o conteúdo vem abaixo. Sem subtexto: o que explicaria a conta vai na `dica`
/// (tooltip do título), nunca em texto visível.
struct Bloco<Conteudo: View>: View {
    let titulo: String
    var dica: String?
    /// Texto pequeno à direita do título (a fonte do número logo abaixo).
    var fonte: String?
    @ViewBuilder var conteudo: Conteudo

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline) {
                Text(titulo).font(.headline).help(dica ?? "")
                if let fonte {
                    Spacer()
                    Text(fonte).font(.caption).foregroundStyle(.tertiary)
                }
            }
            conteudo
        }
        .cartao()
    }
}

/// Linha de grade: símbolo, nome (com detalhe opcional) e o valor à direita.
struct Linha<Valor: View>: View {
    let simbolo: String
    let nome: String
    var detalhe: String?
    /// Tooltip do nome: o que explicaria a linha fica aqui, não em texto visível.
    var dica: String?
    var recuo = false
    var tom: Color?
    /// Texto do botão (i) ao lado do nome.
    var info: String?
    @ViewBuilder var valor: Valor

    var body: some View {
        GridRow(alignment: .firstTextBaseline) {
            // Recuada, o símbolo vai para a coluna do nome: a primeira coluna não alarga.
            if recuo { Color.clear.frame(width: 20, height: 1) } else { icone }
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                if recuo { icone }
                VStack(alignment: .leading, spacing: 2) {
                    HStack(alignment: .firstTextBaseline, spacing: 4) {
                        Text(nome).help(dica ?? "")
                        if let info { BotaoInfo(nome: nome, texto: info) }
                    }
                    if let detalhe { Text(detalhe).font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true) }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            valor.gridColumnAlignment(.trailing)
        }
    }

    private var icone: some View {
        Image(systemName: simbolo).foregroundStyle(tom.map(AnyShapeStyle.init) ?? AnyShapeStyle(.secondary)).frame(width: 20)
    }
}

/// (i) que abre um balão com a explicação.
struct BotaoInfo: View {
    let nome: String
    let texto: String
    @State private var aberto = false

    var body: some View {
        Button { aberto.toggle() } label: { Image(systemName: "info.circle") }
            .buttonStyle(.borderless)
            .accessibilityLabel("O que é \(nome)")
            .popover(isPresented: $aberto, arrowEdge: .bottom) {
                Text(texto).font(.callout).fixedSize(horizontal: false, vertical: true).padding(14).frame(width: 320)
            }
    }
}

func grade<C: View>(@ViewBuilder _ c: () -> C) -> some View {
    Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 10) { c() }
}

// MARK: - 1 a 5: disco, donut, volumes, VM, ISC

private struct Fisico: View {
    let cs: [Conteiner]
    let dados: Medida?
    let macos: Medida?

    var body: some View {
        let p = Armazenamento.principal(cs)
        Bloco(titulo: String(localized: "Disco interno"), dica: String(localized: "Um container APFS com vários volumes; todos dividem o mesmo espaço livre."), fonte: nomeFonte("statfs")) {
            // O livre grande, o ocupado e a manchete (antes o cartão do disco no Início).
            Heroi()
            if let p {
                Divider()
                DonutDisco(fatias: [
                    .init(id: "data", nome: String(localized: "Dados"), bytes: dados?.bytes, fonte: dados?.source ?? "attr_vol_spaceused", cor: .blue),
                    .init(id: "macos", nome: String(localized: "macOS da tela"), bytes: macos?.bytes, fonte: macos?.source ?? "diskutil_capacity_in_use", cor: .gray),
                    .init(id: "livre", nome: String(localized: "Livre"), bytes: p.free, fonte: "diskutil_apfs_list", cor: Color(nsColor: .quaternaryLabelColor)),
                ], total: p.ceiling)
            }
        }
    }
}

/// Os volumes do container principal, o outro container do disco interno e o ISC.
private struct Containers: View {
    let cs: [Conteiner]
    let swap: Swap?
    let macos: Medida?

    var body: some View {
        let p = Armazenamento.principal(cs)
        if let p { Volumes(c: p, swap: swap, cs: cs, macos: macos).id("volumes") }
        // Recuperação e Atualização ficam noutro container do mesmo disco; sem ele a soma do macOS da tela não fecha.
        ForEach(Array(Armazenamento.outrosInternos(cs).enumerated()), id: \.offset) { _, c in
            Bloco(titulo: String(localized: "Outro container do disco interno"), dica: String(localized: "Guarda a cópia de recuperação do macOS. O Ajustes soma estes volumes na linha macOS, não em Dados do Sistema.")) {
                grade {
                    Linha(simbolo: "shippingbox", nome: String(localized: "Tamanho")) { NumeroView(bytes: c.ceiling, source: "diskutil_apfs_list") }
                    ForEach(Array(c.volumes.enumerated()), id: \.offset) { _, v in
                        let n = nomePapel(v.roles.first)
                        Linha(simbolo: n.simbolo, nome: n.nome) {
                            HStack(spacing: 8) {
                                if v.inSettingsMacos { SeloMacos() }
                                NumeroView(bytes: v.capacityInUse, source: "diskutil_capacity_in_use").frame(minWidth: 120, alignment: .trailing)
                            }
                        }
                    }
                }
            }
            .id("recuperacao")
        }
        ForEach(Array(Armazenamento.internos(cs).filter(\.isc).enumerated()), id: \.offset) { _, c in
            Bloco(titulo: String(localized: "Container ISC (xART, Hardware)"), dica: String(localized: "Container à parte, usado pelo Mac para iniciar com segurança. O Ajustes não mostra.")) {
                grade {
                    Linha(simbolo: "shippingbox", nome: String(localized: "Tamanho")) { NumeroView(bytes: c.ceiling, source: "diskutil_apfs_list") }
                    ForEach(Array(c.volumes.enumerated()), id: \.offset) { _, v in
                        let n = nomePapel(v.roles.first)
                        Linha(simbolo: n.simbolo, nome: n.nome) { NumeroView(bytes: v.capacityInUse, source: "diskutil_capacity_in_use") }
                    }
                }
            }
        }
    }
}

/// Donut do container: Data, macOS da tela e livre, com a legenda e a fonte de cada fatia.
struct DonutDisco: View {
    struct Fatia: Identifiable {
        let id: String
        let nome: String
        let bytes: Int64?
        let fonte: String
        let cor: Color
    }
    let fatias: [Fatia]
    let total: Int64?

    var body: some View {
        // Gráfico e legenda centralizados no cartão.
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 32) { grafico; legenda }
            VStack(spacing: 16) { grafico; legenda }
        }
        .frame(maxWidth: .infinity)
        .accessibilityElement(children: .combine)
    }

    private var grafico: some View {
        Chart(fatias.filter { ($0.bytes ?? 0) > 0 }) { f in
            SectorMark(angle: .value("Bytes", Double(f.bytes ?? 0)), innerRadius: .ratio(0.62), angularInset: 1.5)
                .foregroundStyle(f.cor)
                .cornerRadius(3)
        }
        .chartLegend(.hidden)
        .frame(width: 180, height: 180)
        .overlay {
            VStack(spacing: 2) {
                Text(total.map(gb) ?? "—").font(.headline.monospacedDigit())
                Text("no container").font(.caption).foregroundStyle(.secondary)
            }
        }
        .accessibilityHidden(true)
    }

    private var legenda: some View {
        VStack(alignment: .leading, spacing: 10) {
            ForEach(fatias) { f in
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Circle().fill(f.cor).frame(width: 10, height: 10)
                    Text(f.nome).frame(minWidth: 110, alignment: .leading)
                    NumeroView(bytes: f.bytes, source: f.fonte)
                }
            }
        }
    }
}

private struct SeloMacos: View {
    var body: some View {
        Text("entra no macOS da tela")
            .font(.caption2)
            .padding(.horizontal, 6).padding(.vertical, 2)
            .background(.quaternary, in: .capsule)
            .fixedSize()
    }
}

private struct Volumes: View {
    let c: Conteiner
    let swap: Swap?
    let cs: [Conteiner]
    let macos: Medida?

    var body: some View {
        Bloco(titulo: String(localized: "Volumes do container"), dica: String(localized: "Dividem o espaço livre com o Dados. O Ajustes soma os que têm o selo na linha macOS, não em Dados do Sistema.")) {
            grade {
                ForEach(Array(c.volumes.enumerated()), id: \.offset) { _, v in
                    let n = nomePapel(v.roles.first)
                    Linha(simbolo: n.simbolo, nome: n.nome) {
                        HStack(spacing: 8) {
                            if v.inSettingsMacos { selo }
                            // Largura mínima: o selo fica alinhado, seja qual for o número.
                            NumeroView(bytes: v.capacityInUse, source: "diskutil_capacity_in_use")
                                .frame(minWidth: 120, alignment: .trailing)
                        }
                    }
                    if v.roles.contains("VM") { dentroDoVM }
                }
                Divider()
                Linha(simbolo: "sum", nome: String(localized: "macOS da tela"), dica: String(localized: "Soma dos volumes com o selo")) {
                    if let m = macos { NumeroView(m) } else { NumeroView(bytes: Armazenamento.macosDaTela(cs), source: "diskutil_capacity_in_use") }
                }
            }
        }
    }

    private var selo: some View { SeloMacos() }

    @ViewBuilder private var dentroDoVM: some View {
        Linha(simbolo: "arrow.left.arrow.right", nome: String(localized: "Arquivos de swap"), dica: String(localized: "Memória que foi parar no disco. Fechar apps pesados ou reiniciar devolve o espaço."), recuo: true) {
            NumeroView(bytes: swap?.total, source: "sysctl_vm_swapusage", state: swap?.total == nil ? "not_measured" : "measured")
        }
        let resto = Armazenamento.restoVM(cs, swap: swap)
        Linha(simbolo: "ellipsis.circle", nome: String(localized: "Resto do VM (kernelcore e outros)"), recuo: true) {
            if resto == nil, Armazenamento.vm(cs) != nil, swap?.total != nil {
                Text("—").help("Leituras em momentos diferentes")
            } else {
                NumeroView(bytes: resto, source: "difference", state: resto == nil ? "not_measured" : "measured")
            }
        }
    }
}

// MARK: - 7: Apple Intelligence (6 e 8: BarraDesdobravel.swift)

private struct AppleIntelligence: View {
    let ia: IA

    var body: some View {
        Bloco(titulo: String(localized: "Apple Intelligence (informativo)"), dica: String(localized: "Os modelos ficam no volume de dados e entram em Dados do Sistema. Não some com o resto.")) {
            grade {
                Linha(simbolo: "sparkles", nome: String(localized: "Pela regra do Ajustes"), dica: String(localized: "O mesmo número que o Ajustes mostra no botão (i) de macOS; o sdtally refaz a conta")) {
                    NumeroView(bytes: ia.bytes, source: ia.source, state: ia.state, reason: ia.reason)
                }
                if let f = ia.physical {
                    Linha(simbolo: "internaldrive", nome: String(localized: "No disco")) { NumeroView(f) }
                }
            }
        }
    }
}

// MARK: - 9: partes do sistema

struct PartesDoSistema: View {
    @Environment(Painel.self) private var painel
    let c: Completo

    var body: some View {
        let ps = Armazenamento.partes(c.top)
        Bloco(titulo: String(localized: "Partes do sistema no volume Data"), dica: String(localized: "Soma das linhas de Pastas maiores, no volume de dados, não no volume selado do sistema.") + (painel.manual ? " " + String(localized: "Medido às \(hora(c.generatedAt)).") : "")) {
            if ps.isEmpty {
                Text("Nenhuma linha da medição cai nessas pastas.").foregroundStyle(.secondary)
            } else {
                grade {
                    ForEach(ps) { p in
                        let dica = p.prefixo == "/private/var/vm" ? String(localized: "Cópia da memória que o Mac guarda para acordar; o macOS controla o tamanho.") : nil
                        Linha(simbolo: "folder", nome: p.nome, detalhe: p.prefixo, dica: dica) {
                            HStack(spacing: 8) {
                                NumeroView(bytes: p.bytes, source: "top", state: p.piso ? "partial" : "measured", quando: painel.manual ? String(localized: "às \(hora(c.generatedAt))") : nil, compacto: true)
                                Button { abrir(p.prefixo) } label: { Image(systemName: "chevron.right.circle") }
                                    .buttonStyle(.borderless)
                                    .help("Ver o que tem dentro")
                                    .accessibilityLabel("Ver o que tem dentro de \(p.nome)")
                            }
                        }
                    }
                }
            }
        }
    }

    private func abrir(_ prefixo: String) {
        painel.secao = .ranking
        painel.pilha = [absoluto(prefixo, home: c.home)]
    }
}

// MARK: - 11 e 12: apagados, purgeable (10, snapshots: SnapshotsView.swift; 9a: RecursosView.swift)

struct ApagadosAbertos: View {
    let i: Completo.Informativo
    let root: Bool

    var body: some View {
        // nil: a lista não foi medida e o bloco some; [] medida e vazia: V8.
        if let ps = i.deletedOpenProcesses {
            Bloco(titulo: String(localized: "Apagados ainda abertos, por processo"), dica: String(localized: "Arquivos apagados que um processo ainda segura. Os 10 maiores; o espaço volta quando o processo fecha.")) {
                if ps.isEmpty {
                    VazioLinha(.v8(root: root))
                } else {
                    grade {
                        ForEach(Array(ps.enumerated()), id: \.offset) { _, p in
                            Linha(simbolo: "app.dashed", nome: p.name, detalhe: p.files == 1 ? String(localized: "1 arquivo") : String(localized: "\(p.files.formatted(.number.locale(localApp))) arquivos")) {
                                VStack(alignment: .trailing, spacing: 2) {
                                    NumeroView(bytes: p.bytes, source: "lsof_proc_pidfdinfo", state: p.bytes == nil ? "not_measured" : "measured", reason: p.bytes == nil ? String(localized: "alocado ilegível") : nil)
                                    // Fora de qualquer soma: explica a ordem (max(bytes, lógico)).
                                    if let l = p.logicalBytesWithoutAllocated, l > 0 {
                                        HStack(spacing: 4) {
                                            NumeroView(bytes: l, source: "lsof_proc_pidfdinfo")
                                            Text("lógico (alocado ilegível)").font(.caption).foregroundStyle(.secondary)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// MARK: - 13 a 15: informativo, estado, o que falta

private struct InformativoBloco: View {
    let c: Completo

    var body: some View {
        if let i = c.informational {
            Bloco(titulo: String(localized: "Informativo (não somar)"), dica: String(localized: "Já está dentro dos números acima; mostrado à parte.")) {
                grade {
                    if let r = i.repeatedFullClones {
                        Linha(simbolo: "doc.on.doc", nome: String(localized: "Clones completos repetidos"), detalhe: arquivos(r.files)) { NumeroView(bytes: r.bytes, source: "getattrlistbulk") }
                    }
                    if let r = i.repeatedHardlinks {
                        Linha(simbolo: "link", nome: String(localized: "Hardlinks repetidos"), detalhe: arquivos(r.files)) { NumeroView(bytes: r.bytes, source: "getattrlistbulk") }
                    }
                    if let d = i.dataless {
                        Linha(simbolo: "icloud", nome: String(localized: "Só na nuvem (não baixado)"), detalhe: String(localized: "\(arquivos(d.files)) · 0 no disco")) {
                            Text(String(localized: "\(gb(d.apparentBytes)) aparentes")).monospacedDigit()
                        }
                    }
                    if let m = i.apfsMetadata {
                        Linha(simbolo: "internaldrive", nome: String(localized: "Metadados do APFS"),
                              dica: String(localized: "Árvores vivas do volume; as árvores dos snapshots não entram.")) {
                            NumeroView(bytes: m.bytes, source: m.source, state: m.state, reason: m.reason)
                        }
                    }
                    if let l = i.trash {
                        Linha(simbolo: "trash", nome: String(localized: "Lixo há mais de \(l.thresholdDays) dias"), detalhe: textoLixo(l), dica: String(localized: "Vale a data em que cada item entrou no Lixo, não a de modificação; só o Lixo desta conta no volume Data.")) {
                            NumeroView(bytes: l.older.bytes, source: l.older.source, state: l.older.state, reason: l.older.reason)
                        }
                    }
                    if let g = i.graftRootsSkipped {
                        Linha(simbolo: "arrow.triangle.branch", nome: String(localized: "Raízes de enxerto puladas")) { Text("\(g)").monospacedDigit() }
                    }
                    if let s = i.scanState {
                        Linha(simbolo: "magnifyingglass", nome: String(localized: "Travessia")) { Text(s == "measured" || s == "complete" ? String(localized: "completa") : s == "partial" ? String(localized: "parcial") : s) }
                    }
                }
            }
        }
    }

    private func arquivos(_ n: Int64) -> String { n == 1 ? String(localized: "1 arquivo") : String(localized: "\(n.formatted(.number.locale(localApp))) arquivos") }
}

/// "3 de 12 itens do Lixo (4,56 GB no total)". Sem medida, o motivo já vem no número (nil).
nonisolated func textoLixo(_ l: Completo.Lixo) -> String? {
    guard let n = l.older.items, let t = l.total.items, let b = l.total.bytes else { return nil }
    func itens(_ x: Int64) -> String { x == 1 ? String(localized: "1 item") : String(localized: "\(x.formatted(.number.locale(localApp))) itens") }
    var s = String(localized: "\(n) de \(itens(t)) (\(gb(b)) no total)")
    if let u = l.undated, u.items > 0 { s += " · " + String(localized: "\(itens(u.items)) sem data de entrada ficam fora") }
    return s
}

private struct EstadoMedicao: View {
    let c: Completo

    var body: some View {
        Bloco(titulo: String(localized: "Estado da medição"), dica: c.complete ? String(localized: "Todas as etapas rodaram.") : String(localized: "O que faltou fica em Bloqueado pelo macOS.")) {
            Text(c.complete ? String(localized: "Completa") : String(localized: "Incompleta"))
            if !c.incompleteReasons.isEmpty || !c.stageErrors.isEmpty {
                grade {
                    ForEach(c.incompleteReasons, id: \.self) { r in
                        Linha(simbolo: "exclamationmark.triangle", nome: motivo(r), tom: Marca.aviso) { EmptyView() }
                    }
                    ForEach(c.stageErrors, id: \.stage) { e in
                        Linha(simbolo: "xmark.octagon", nome: String(localized: "Etapa \(e.stage)"), detalhe: e.error) { EmptyView() }
                    }
                }
                .textSelection(.enabled)
            }
        }
    }

    private func motivo(_ r: String) -> String {
        switch r {
        case "no_root": String(localized: "Sem senha de administrador (\(c.deniedFolders) pastas não abriram)")
        case "no_full_disk_access": String(localized: "Sem Acesso Total ao Disco")
        case "interrupted": String(localized: "Interrompida: os números são um piso")
        case "stage_failed": String(localized: "Alguma etapa da coleta falhou (abaixo)")
        default: r
        }
    }
}

private struct NaoMedido: View {
    /// nil sem medição completa; `trash`/`apfsMetadata` nil em medição de um sdtally antigo.
    let informativo: Completo.Informativo?

    var body: some View {
        Bloco(titulo: String(localized: "O que ainda não é medido"), dica: String(localized: "Aqui, memória é espaço em disco. A RAM aparece no Monitor de Atividade; a ponte entre as duas é o swap, no volume VM.")) {
            VStack(alignment: .leading, spacing: 6) {
                item(String(localized: "Livros e iCloud Drive nas categorias"), String(localized: "Livros pede o tamanho de cada livro; iCloud Drive, o banco do File Provider."))
                if informativo?.apfsMetadata == nil {
                    item(String(localized: "Metadados do APFS"), String(localized: "Saem da medição completa, pelos contadores do próprio APFS, sem senha."))
                }
                item(String(localized: "Metadados dos snapshots do APFS"), String(localized: "O contador cobre só as árvores vivas do volume; as de cada snapshot não têm contador."))
                item(String(localized: "Dentro de Preboot e Recovery"), String(localized: "O diskutil só dá o total de cada volume, e o sdtally só percorre o volume Data."))
                item(String(localized: "Dono das pastas sem app declarado"), String(localized: "Ferramentas de linha de comando e pastas do sistema não dizem no disco de qual app são."))
                if informativo?.trash == nil {
                    item(String(localized: "Lixo acima de 30 dias"), String(localized: "Sai da medição completa, pela data em que cada item entrou no Lixo."))
                }
                item(String(localized: "Lixo de volumes externos"), String(localized: "O sdtally só percorre o volume Data, então só o Lixo da sua conta nele é medido."))
                item(String(localized: "Conteúdo de cada snapshot"), String(localized: "Viria de comparar cada snapshot com o disco; hoje só se mede quanto ele segura."))
                item(String(localized: "Limiar do aviso de pouco espaço"), String(localized: "O macOS não grava nas preferências."))
            }
        }
    }

    private func item(_ titulo: String, _ motivo: String) -> some View {
        Label(titulo, systemImage: "circle.dashed").help(motivo)
    }
}

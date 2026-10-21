// Barra que se desdobra (spec-fase6 §4.8, A1), em dois lugares: no Disco, "Como o Ajustes divide"
// (raiz .ajustes; o macOS desdobra no lugar, Dados do Sistema não desdobra); em Dados do Sistema,
// a conta do volume Data (raiz .volumeData) → termo(k), numa pilha (Painel.pilhaBarra; vazia = a
// raiz). Crossfade entre níveis, sem morph de largura: as somas
// de dois níveis não são iguais (nota do volumeData). Dono: A9.
import SwiftUI

/// Pilha de partida de um nível (SDTALLY_BARRA): o caminho até ele, sem o `ajustes` da raiz.
nonisolated func pilhaInicial(_ n: NivelBarra?) -> [NivelBarra] {
    switch n {
    // O macOS desdobra no lugar (BarraDesdobravel.abertos), não na pilha.
    case nil, .ajustes?, .macos?, .volumeData?: []
    // O volumeData é a raiz da barra no menu Dados do Sistema (SistemaView).
    case let .termo(k)?: [.termo(k)]
    }
}

/// Categorias, Dados do Sistema, macOS e Disponível, como a barra do Ajustes.
nonisolated func segmentosAjustes(_ t: Tela, macos: Medida?) -> [Segmento] {
    var s = (t.categories ?? []).filter { $0.bytes > 0 }.map {
        let c = categoria($0.id)
        return Segmento(id: $0.id, nome: c.nome, bytes: $0.bytes, cor: c.cor, fonte: "storage_management_log")
    }
    if let sd = t.systemData { s.append(Segmento(id: "sd", nome: String(localized: "Dados do Sistema"), bytes: sd, cor: Marca.dados)) }
    if let m = macos, let b = m.bytes {
        s.append(Segmento(id: "macos", nome: "macOS", bytes: b, cor: .gray.opacity(0.5), fonte: m.source, estado: m.state, desdobra: .macos))
    }
    if let a = t.available {
        s.append(Segmento(id: "livre", nome: String(localized: "Disponível"), bytes: a, cor: Color(nsColor: .quaternaryLabelColor), fonte: "settings_ui_log"))
    }
    return s
}

/// As partes de um nível. nil: falta a fonte (sem tela do Ajustes, V1; sem containers; sem a
/// medição completa no volumeData e no termo, V5). `tela`: a última tela boa (Painel.telaBoa),
/// que vale mais que a do rápido, que só tem a tela com o painel do Ajustes aberto.
nonisolated func segmentos(_ n: NivelBarra, rapido: Rapido?, completo: Completo?, tela: Tela? = nil)
    -> (titulo: String, partes: [Segmento], soma: Int64?, nota: String?)? {
    let t = tela ?? [rapido?.settingsScreen.screen, completo?.settingsScreen.screen].compactMap(\.self)
        .first { $0.state != "not_measured" && $0.systemData != nil }
    let macos = (rapido?.settingsScreen ?? completo?.settingsScreen)?.macos
    switch n {
    case .ajustes:
        guard let t else { return nil }
        let p = segmentosAjustes(t, macos: macos)
        return (String(localized: "Como o Ajustes divide"), p, p.reduce(0) { $0 + $1.bytes }, nil)

    case .macos:
        guard let cs = rapido?.containers ?? completo?.containers else { return nil }
        let tons = [0.85, 0.65, 0.5, 0.38, 0.28, 0.2]
        let p = Armazenamento.internos(cs).flatMap(\.volumes).filter(\.inSettingsMacos).enumerated().compactMap { i, v in
                v.capacityInUse.map {
                    Segmento(id: "\(i)", nome: nomePapel(v.roles.first).nome, bytes: $0, cor: .gray.opacity(tons[i % tons.count]),
                             fonte: "diskutil_capacity_in_use")
                }
            }
        return (String(localized: "macOS: volumes do disco interno"), p, macos?.bytes ?? p.reduce(0) { $0 + $1.bytes },
                String(localized: "Os volumes que o Ajustes soma na linha macOS. O swap fica no VM."))

    case .volumeData:
        guard let c = completo else { return nil }
        let v = c.dataVolume
        var p = v.terms.compactMap { t -> Segmento? in
            guard let b = t.bytes, b > 0 else { return nil }
            return Segmento(id: t.key, nome: nomeTermo(t.key), bytes: b, cor: termo(t.key).cor, fonte: t.source, estado: t.state, desdobra: .termo(t.key))
        }
        if let b = v.unexplained.bytes, b > 0 {
            p.append(Segmento(id: "sem", nome: String(localized: "Bloqueado pelo macOS"), bytes: b, cor: .clear, tracejado: true, estado: v.unexplained.state))
        }
        let w = valorOuNaoMedido(c.informational?.purgeable?.bytes ?? rapido?.purgeable.bytes)
        let x = v.used.bytes
        let nota: String
        if let x, let t, let sd = t.systemData, let cat = t.categoriesTotal {
            // Horas diferentes (a medição pode ter dias): as duas à vista, e diferença só quando cabe.
            let d = x - cat - sd
            let gera: String = hora(c.generatedAt)
            let soma = t.loggedAt.map { String(localized: "O volume Data usa \(gb(x)) (medido às \(gera)). As categorias e Dados do Sistema do Ajustes (lido às \(hora($0))) somam \(gb(cat + sd)).") }
                ?? String(localized: "O volume Data usa \(gb(x)) (medido às \(gera)). As categorias e Dados do Sistema do Ajustes somam \(gb(cat + sd)).")
            nota = soma + " " + (d >= 0 ? String(localized: "A diferença é \(gb(d)); o purgeable medido é \(w).") : String(localized: "O purgeable medido é \(w)."))
        } else {
            nota = String(localized: "O volume Data usa \(valorOuNaoMedido(x)). Sem a tela do Ajustes, não há o outro total para comparar; o purgeable medido é \(w).")
        }
        return (String(localized: "Conta do volume Data"), p, x, nota)

    case let .termo(k):
        guard let c = completo else { return nil }
        let total = c.dataVolume.terms.first { $0.key == k }?.bytes
        let linhas: [(id: String, nome: String, bytes: Int64, fonte: String)] = switch k {
        case "snapshots":
            (c.informational?.snapshots ?? []).enumerated().compactMap { i, s in
                s.privateBytes.map { ("s\(i)", nomeSnapshot(s), $0, "fsctl_snapshot_private_size") }
            }
        case "deleted_open":
            (c.informational?.deletedOpenProcesses ?? []).enumerated().compactMap { i, p in
                p.bytes.map { ("p\(i)", visivel(p.name), $0, "lsof_proc_pidfdinfo") }
            }
        default:
            c.top.filter { $0.term == k }.map { ($0.id, visivel(rotulo($0, home: c.home)), $0.bytes, $0.source) }
        }
        let soma = linhas.reduce(0) { $0 + $1.bytes }
        let base = total ?? soma
        let cor = termo(k).cor
        let ordem = linhas.sorted { $0.bytes > $1.bytes }
        let menores = ordem.filter { Double($0.bytes) < Double(base) * 0.005 }
        let juntar = menores.count >= 2
        var p = ordem.filter { !juntar || Double($0.bytes) >= Double(base) * 0.005 }.enumerated().map { i, l in
            Segmento(id: l.id, nome: l.nome, bytes: l.bytes, cor: cor.opacity(i.isMultiple(of: 2) ? 1 : 0.6), fonte: l.fonte)
        }
        if juntar {
            p.append(Segmento(id: "menores", nome: String(localized: "menores (\(menores.count))"), bytes: menores.reduce(0) { $0 + $1.bytes }, cor: cor.opacity(0.35), fonte: "top"))
        }
        if let total, total - soma > 0 {
            let nome = k == "snapshots" ? String(localized: "Presos em dois ou mais snapshots") : String(localized: "Fora das linhas listadas")
            p.append(Segmento(id: "resto", nome: nome, bytes: total - soma, cor: .clear, tracejado: true))
        }
        let nota = k == "snapshots" ? String(localized: "Cada snapshot pelo que só ele segura. Os blocos presos em dois ou mais só saem quando todos saírem.") : nil
        return (String(localized: "\(nomeTermo(k)) no volume Data"), p, base, nota)
    }
}

/// "Time Machine · 01/10 às 18:05", ou quem criou (reverse-DNS) quando não é do Time Machine.
nonisolated func nomeSnapshot(_ s: Completo.Snapshot) -> String {
    let quem = s.kind == "time_machine" ? "Time Machine" : s.creator.map(visivel) ?? (s.kind == "apple_other" ? String(localized: "Do macOS") : String(localized: "De outro app"))
    return quem + (s.date.map { " · " + Armazenamento.dataCurta($0) } ?? "")
}

struct BarraDesdobravel: View {
    @Environment(Painel.self) private var painel
    /// O nível sem pilha: .ajustes no Disco, .volumeData em Dados do Sistema.
    var raiz: NivelBarra = .ajustes
    /// A medição dos níveis do volume Data; nil = a Medição completa (painel.completo).
    var fonte: Completo? = nil
    /// `fonte` é o autoVivo: a legenda diz "ao vivo" e não chama de incompleto o que o auto não mede.
    var aoVivo = false
    private var c: Completo? { fonte ?? painel.completo }
    @Environment(\.accessibilityReduceMotion) private var reduzir
    @Namespace private var ns
    @State private var destaque: Set<String> = []
    /// Partes desdobradas no lugar (hoje só o macOS): a linha cresce para baixo, sem trocar de nível.
    @State private var abertos: Set<NivelBarra> = Foto.barra == .macos ? [.macos] : []

    var body: some View {
        let nivel = painel.pilhaBarra.last ?? raiz
        VStack(alignment: .leading, spacing: 12) {
            nivelView(nivel)
                .id(nivel)
                .transition(.opacity)
        }
        .animation(reduzir ? nil : .easeInOut(duration: 0.2), value: painel.pilhaBarra)
        .cartao()
        .focusable()
        .focusEffectDisabled()
        .onExitCommand { voltar() }
    }

    @ViewBuilder private func nivelView(_ nivel: NivelBarra) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            if let s = segmentos(nivel, rapido: painel.rapidoVivo, completo: c, tela: painel.telaBoa?.tela) {
                cabecalho(s.titulo, legenda: legenda(nivel), dica: s.nota)
                if s.partes.isEmpty {
                    Text(vazioTermo(nivel)).foregroundStyle(.secondary)
                } else {
                    BarraView(segmentos: s.partes, altura: 24, destaque: destaque)
                        .matchedGeometryEffect(id: "barra", in: ns, properties: .position)
                    if nivel == .volumeData, let w = c?.informational?.purgeable?.bytes ?? painel.purgVivo?.bytes,
                       let soma = s.soma, soma > 0 {
                        Colchete(fracao: Double(w) / Double(soma), rotulo: String(localized: "Purgeable medido: \(gb(w))"))
                    }
                    linhas(s.partes, nivel: nivel)
                }
            } else {
                cabecalho(nivel == .ajustes ? String(localized: "Como o Ajustes divide") : nivel == .macos ? String(localized: "macOS: volumes do disco interno") : String(localized: "Conta do volume Data"), legenda: nil, dica: nil)
                semFonte(nivel)
            }
        }
    }

    /// `legenda`: hora e modo da leitura. `dica`: a conta por extenso, só no tooltip do título.
    private func cabecalho(_ titulo: String, legenda: String?, dica: String?) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 8) {
                if !painel.pilhaBarra.isEmpty {
                    Button { voltar() } label: { Label("Voltar", systemImage: "chevron.left") }
                        .buttonStyle(.borderless)
                        .help("Voltar um nível (Esc ou ⌘[)")
                }
                Text(titulo).font(.headline).help(dica ?? "")
            }
            if let legenda {
                Text(legenda).font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    /// Legenda da barra: uma linha por parte; a que desdobra é um botão.
    private func linhas(_ partes: [Segmento], nivel: NivelBarra) -> some View {
        Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 10) {
            ForEach(partes) { s in
                GridRow(alignment: .firstTextBaseline) {
                    Group {
                        if s.tracejado {
                            RoundedRectangle(cornerRadius: 2).strokeBorder(.secondary, style: StrokeStyle(lineWidth: 1, dash: [2, 2]))
                        } else {
                            RoundedRectangle(cornerRadius: 2).fill(s.cor)
                        }
                    }
                    .frame(width: 10, height: 10)
                    nome(s, nivel: nivel).frame(maxWidth: .infinity, alignment: .leading)
                    NumeroView(bytes: s.bytes, source: s.fonte, state: s.estado, quando: quando(nivel), compacto: nivel == .ajustes && s.fonte == "storage_management_log").gridColumnAlignment(.trailing)
                }
                .contentShape(.rect)
                .onHover { destaque = $0 ? [s.id] : [] }
                if let d = s.desdobra, abertos.contains(d), let sub = segmentos(d, rapido: painel.rapidoVivo, completo: c, tela: painel.telaBoa?.tela) {
                    ForEach(sub.partes) { f in
                        GridRow(alignment: .firstTextBaseline) {
                            Color.clear.frame(width: 10, height: 1)
                            HStack(spacing: 8) {
                                RoundedRectangle(cornerRadius: 2).fill(f.cor).frame(width: 10, height: 10)
                                Text(f.nome)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            NumeroView(bytes: f.bytes, source: f.fonte, state: f.estado).gridColumnAlignment(.trailing)
                        }
                        .foregroundStyle(.secondary)
                    }
                }
            }
        }
    }

    @ViewBuilder private func nome(_ s: Segmento, nivel: NivelBarra) -> some View {
        let texto = VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 4) {
                Text(s.nome).lineLimit(2).truncationMode(.middle)
                if let d = s.desdobra {
                    Image(systemName: "chevron.right").font(.caption).foregroundStyle(.tertiary)
                        .rotationEffect(.degrees(abertos.contains(d) ? 90 : 0))
                }
            }
            if let d = detalhe(s, nivel: nivel) {
                Text(d).font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            }
        }
        .help(ajuda(s, nivel: nivel))
        if let d = s.desdobra {
            Button { alternar(d) } label: { texto.contentShape(.rect) }
                .buttonStyle(.plain)
                .help(abertos.contains(d) ? String(localized: "Recolher \(s.nome)") : String(localized: "Desdobrar \(s.nome)"))
                .accessibilityAction(named: abertos.contains(d) ? String(localized: "Recolher \(s.nome)") : String(localized: "Desdobrar \(s.nome)")) { alternar(d) }
        } else {
            texto
        }
    }

    /// Só fato curto e visível; a explicação de cada parte vai em `ajuda` (tooltip).
    private func detalhe(_ s: Segmento, nivel: NivelBarra) -> String? {
        switch (nivel, s.id) {
        case (.volumeData, "sem"):
            c.flatMap { $0.deniedFolders > 0 ? String(localized: "\($0.deniedFolders) pastas não abriram") : nil }
        case (.volumeData, "deleted_open"):
            c?.top.first(where: { $0.term == "deleted_open" })?.topProcess.map { String(localized: "Maior: \(visivel($0.name))") }
        default: nil
        }
    }

    private func ajuda(_ s: Segmento, nivel: NivelBarra) -> String {
        switch (nivel, s.id) {
        case (.ajustes, "sd"): String(localized: "O que sobra do usado depois do macOS e das outras linhas.")
        case (.ajustes, "macos"): String(localized: "Os volumes que o Ajustes soma na linha macOS. O swap fica no VM.")
        case (.volumeData, "sem"): String(localized: "O que o volume diz estar ocupado e nenhuma pasta legível explica: metadados do APFS e pastas que não abriram.")
        case (.volumeData, let k): termo(k).frase
        default: ""
        }
    }

    private func legenda(_ nivel: NivelBarra) -> String? {
        switch nivel {
        case .ajustes:
            guard let t = painel.telaBoa?.tela else { return nil }
            let agora = painel.rapido?.settingsScreen.screen.map { $0.state != "not_measured" } ?? false
            let fonte: String? = t.categories?.isEmpty == false ? String(localized: "categorias: \(nomeFonte("storage_management_log"))") : nil
            let base = if let h = hora(t.loggedAt) {
                agora ? String(localized: "Lido às \(h)") : String(localized: "Armazenamento do Ajustes fechado · última leitura às \(h)")
            } else {
                agora ? String(localized: "Lido agora") : String(localized: "Armazenamento do Ajustes fechado")
            }
            return [base, fonte].compactMap { $0 }.joined(separator: " · ")
        case .macos: return nil
        case .volumeData, .termo:
            guard let c else { return nil }
            // O auto nunca tem senha nem xattr: "incompleto" ali não é falha.
            if aoVivo { return String(localized: "Ao vivo · sem senha") }
            let arquivo: String? = Foto.env["SDTALLY_COMPLETO"] != nil ? String(localized: "de arquivo") : nil
            let modo = c.root ? String(localized: "como administrador") : String(localized: "sem senha")
            let fim = c.complete ? String(localized: "completo") : String(localized: "incompleto")
            return [String(localized: "Medido às \(hora(c.generatedAt))"), modo, arquivo, fim].compactMap { $0 }.joined(separator: " · ")
        }
    }

    private func quando(_ nivel: NivelBarra) -> String? {
        switch nivel {
        case .volumeData, .termo: aoVivo ? nil : c.map { String(localized: "às \(hora($0.generatedAt))") }
        default: nil
        }
    }

    private func vazioTermo(_ nivel: NivelBarra) -> String {
        guard case let .termo(k) = nivel, let c else { return String(localized: "Nenhum.") }
        return nenhum(fonte: c.dataVolume.terms.first { $0.key == k }?.source ?? "top", hora: aoVivo ? nil : (hora(c.generatedAt) as String))
    }

    @ViewBuilder private func semFonte(_ nivel: NivelBarra) -> some View {
        switch nivel {
        case .ajustes:
            // A ação de cada vazio (§5.6): V1 abre o Ajustes, V2 nenhuma, V3 vai para Permissões.
            let v = vazioAjustes(painel.rapido?.settingsScreen.screen, ultimo: painel.pontos.last { $0.systemData != nil })
            VazioLinha(vazio: v) {
                switch v.acao {
                case .abrirArmazenamento?: Button(AcaoVazio.abrirArmazenamento.rotulo) { painel.abrirArmazenamento() }
                case .permissoes?: Button(AcaoVazio.permissoes.rotulo) { painel.secao = .permissoes }
                default: EmptyView()
                }
            }
        case .macos:
            Text("Não medido").foregroundStyle(.secondary)
        case .volumeData, .termo:
            // Sem botão: a senha fica no Calcular da barra lateral (spec-inicio §5).
            VazioLinha(.v5(usadoData: painel.usoVivo?.bytes))
        }
    }

    private func alternar(_ d: NivelBarra) {
        withAnimation(reduzir ? nil : .easeInOut(duration: 0.2)) {
            if abertos.contains(d) { abertos.remove(d) } else { abertos.insert(d) }
        }
    }

    private func voltar() {
        if !painel.pilhaBarra.isEmpty { painel.pilhaBarra.removeLast() }
    }
}

/// Colchete do purgeable: só o tamanho, sem posição (está espalhado dentro do usado).
private struct Colchete: View {
    let fracao: Double
    let rotulo: String

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            GeometryReader { g in
                let w = g.size.width * min(1, max(0, fracao))
                Path { p in
                    p.move(to: CGPoint(x: 0.5, y: 0))
                    p.addLine(to: CGPoint(x: 0.5, y: 6))
                    p.addLine(to: CGPoint(x: w - 0.5, y: 6))
                    p.addLine(to: CGPoint(x: w - 0.5, y: 0))
                }
                .stroke(.secondary, lineWidth: 1)
            }
            .frame(height: 7)
            .accessibilityHidden(true)
            Text(rotulo).font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                .help("Já está dentro do usado, sem lugar fixo na barra; o colchete só mostra o tamanho.")
        }
    }
}

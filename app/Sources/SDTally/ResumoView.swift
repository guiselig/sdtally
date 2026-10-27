// Sem tela de Início: as peças do antigo Início (spec-inicio §2 a §5) ficam aqui e aparecem no
// topo de Dados do Sistema (CartaoInicio): a faixa do Acesso Total (B0), o cartão Dados do
// Sistema (B3) com "O que mais ocupa o disco" (B5) dentro e, no fim, a linha de medição (B4), o
// único lugar da senha na tela; depois, o que vem depois da lista (B6). B1 e B2 (Heroi) ficam no Disco.
// As contas puras ficam em Causas.swift.
import AppKit
import SwiftUI

/// O que era o Início, no topo de Dados do Sistema: a faixa do Acesso Total, o cartão Dados do
/// Sistema e o que vem depois da lista.
struct CartaoInicio: View {
    @Environment(Painel.self) private var painel

    var body: some View {
        let c = painel.dados
        let fichas = c?.cards ?? painel.catalogo?.cards
        let cs = c.map { causas(top: $0.top, swap: $0.swap, fichas: fichas, home: $0.home, apelidos: painel.apelidos) } ?? []
        if painel.rapido?.fullDiskAccess == false {
            Faixa(simbolo: "lock.trianglebadge.exclamationmark", texto: String(localized: "Sem Acesso Total ao Disco")) {
                Button("Abrir Permissões") { painel.secao = .permissoes }
            }
        }
        CartaoSistema(causas: cs, fichas: fichas)
        DepoisDaLista(causas: cs)
    }
}

/// Estado vazio da tela do Ajustes (V1, V2, V3); `ultimo`: o último Dados do Sistema do Histórico.
nonisolated func vazioAjustes(_ t: Tela?, ultimo: Ponto?) -> Vazio {
    if t?.state == "partial" { return .v2 }
    if let r = t?.reason, r.localizedCaseInsensitiveContains("permission") || r.localizedCaseInsensitiveContains("not read") { return .v3 }
    return .v1(ultimo: ultimo?.systemData, hora: ultimo.map { hora($0.telaEm ?? $0.t) })
}

/// Linha de ranking em texto: caminho, "demais itens em", ou o termo.
nonisolated func rotulo(_ l: LinhaTop, home: String) -> String {
    if let p = l.path { return l.folded == true ? String(localized: "Demais itens em \(p)") : p }
    var s = nomeTermo(l.term)
    if let n = l.count { s += " (\(n))" }
    if let p = l.topProcess { s += String(localized: " · maior entre os medidos: \(p.name) \(gb(p.bytes))") }
    return s
}

extension TelaLida {
    /// Segmentos da barra do Ajustes: categorias, Dados do Sistema, macOS e Disponível.
    var segmentos: [Segmento] { segmentosAjustes(tela, macos: macos) }
}

extension Painel {
    /// O link "Ver…" de uma causa (§3 B5) e o destino dele, que também vale para o clique numa parte
    /// do cartão e num item de "Em seguida": snapshots e swap em Dados do Sistema, o conselho em O
    /// que fazer, a pasta em Pastas maiores. nil sem destino.
    func ver(_ c: Causa) -> (rotulo: String, ir: @MainActor () -> Void)? {
        if c.termo == "snapshots" { return (String(localized: "Ver os snapshots ›"), { self.abrir(.sistema, rolando: "snapshots") }) }
        if c.id == "swap" { return (String(localized: "Vale a pena reiniciar? ›"), { self.abrir(.sistema, rolando: "reiniciar") }) }
        if let a = conselho(de: c, em: completo?.advice ?? []) { return (String(localized: "Ver em O que fazer ›"), { self.abrir(.fazer, rolando: a.id) }) }
        // Pasta de outra conta: o SDTally não desce nela (ficha other_users).
        guard let p = c.caminho, c.card != "other_users" else { return nil }
        let abs = absoluto(p, home: completo?.home ?? "")
        return (String(localized: "Ver a pasta ›"), {
            self.pilha = [abs]
            self.secao = .ranking
        })
    }

    /// Vai para a seção e rola até a âncora (o .id do bloco ou do cartão); a seção limpa `rolarPara`.
    func abrir(_ s: Secao, rolando a: String) {
        rolarPara = a
        secao = s
    }
}

// MARK: - B1 e B2: o Livre e a manchete (no primeiro cartão do Disco)

/// Fica no primeiro cartão do Disco (Fisico, em ArmazenamentoView), acima do donut.
struct Heroi: View {
    @Environment(Painel.self) private var painel

    var body: some View {
        let c = painel.completo
        let fichas = c?.cards ?? painel.catalogo?.cards
        let cs = c.map { causas(top: $0.top, swap: $0.swap, fichas: fichas, home: $0.home, apelidos: painel.apelidos) } ?? []
        let manchete = manchete(advice: c?.advice, tm: c?.informational?.timeMachine,
                                snapshots: c?.advice?.first { $0.id == "tm_backup_stuck" }?.bytes,
                                causas: cs, usado: c?.dataVolume.used.bytes)
        VStack(alignment: .leading, spacing: 12) {
            if let r = painel.rapido, let c = r.capacity {
                livre(r, c)
            } else if let f = painel.rapidoFalhou {
                Text(nomeVolume).font(.headline)
                Text("—").font(.heroi)
                Text("Não deu para ler o disco: \(f.msg)").foregroundStyle(Marca.aviso).textSelection(.enabled)
                Button("Tentar de novo") { Task { await painel.atualizar() } }
            } else {
                VStack(alignment: .leading, spacing: 8) {
                    Text(nomeVolume).font(.headline)
                    Text("00,00 GB livres").font(.heroi)
                    Text("de 000,00 GB · 00 % ocupado")
                }
                .redacted(reason: .placeholder)
            }
            if let m = manchete { linha(m, fichas: fichas) }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// Volume, fonte e hora; o livre do statfs (no tom de aviso com pouco espaço, a conta do
    /// Aviso.poucoEspaco); o ocupado; e "Hoje", só com pontos do Histórico.
    @ViewBuilder private func livre(_ r: Rapido, _ c: Rapido.Capacidade) -> some View {
        // A fonte (statfs) fica na linha do título do cartão (Bloco); aqui, só o aviso de falha.
        if let f = painel.rapidoFalhou {
            Label("Não atualizado desde \(hora(r.generatedAt))", systemImage: "exclamationmark.triangle")
                .font(.caption).foregroundStyle(Marca.aviso).help(f.msg)
        }
        HStack {
            NumeroLivre(capacidade: c)
            Spacer()
            // O ícone do próprio volume, o mesmo do Finder (interno, externo, rede).
            Image(nsImage: Self.icone)
                .resizable()
                .frame(width: 64, height: 64)
                .accessibilityHidden(true)
        }
    }

    /// Lido uma vez: o NSWorkspace monta o ícone a cada chamada.
    private static let icone = NSWorkspace.shared.icon(forFile: "/")

    private func linha(_ m: Manchete, fichas: MapaFichas?) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Image(systemName: m.simbolo)
                .foregroundStyle(m.simbolo == "clock.arrow.circlepath" ? Marca.aviso : Color.secondary)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 8) {
                Text(m.texto).fixedSize(horizontal: false, vertical: true)
                // O botão vem da ficha (R6), só com o painel que a manchete pede.
                if let a = m.ajustes, let b = botoesAjustes(fichas?["tm_backup_stuck"]).first(where: { $0.url == ajustesPermitidos[a] }) {
                    Button(b.rotulo) { NSWorkspace.shared.open(b.url) }
                }
            }
        }
    }
}

/// O número grande do livre: só ele lê o `livre` do laço de 1 s, então só ele redesenha quando o
/// número muda (o resto do cartão e o ícone ficam).
private struct NumeroLivre: View {
    @Environment(Painel.self) private var painel
    let capacidade: Rapido.Capacidade

    var body: some View {
        // O livre do statfs de 1 s; o total não muda.
        let cap = Rapido.Capacidade(total: capacidade.total, free: painel.livre ?? capacidade.free)
        VStack(alignment: .leading, spacing: 4) {
            let total = Double(max(1, cap.total))
            let pouco = Double(cap.free + (painel.purgVivo?.bytes ?? 0)) / total < 0.10
            let livres = Text("\(gb(cap.free)) livres")
                .font(.heroi).tracking(-0.5)
                .foregroundStyle(pouco ? Marca.aviso : Color.primary)
                .transicaoNumero(Double(cap.free))
            let de = Text("de \(gb(cap.total)) · \(Int((100 * (1 - Double(cap.free) / total)).rounded())) % ocupado")
                .foregroundStyle(.secondary)
            ViewThatFits(in: .horizontal) {
                HStack(alignment: .firstTextBaseline, spacing: 10) { livres; de }
                VStack(alignment: .leading, spacing: 4) { livres; de }
            }
            if let p = painel.pontos.first(where: { data($0.t).map(Calendar.current.isDateInToday) ?? false }),
               let antes = p.livre, abs(antes - cap.free) >= 1_000_000_000 {
                Button("Hoje: \(gb(antes)) às \(hora(p.t)) → \(gb(cap.free)) agora ›") { painel.secao = .historico }
                    .buttonStyle(.link)
            }
        }
    }
}

// MARK: - B3 e B4: o cartão Dados do Sistema

private struct CartaoSistema: View {
    @Environment(Painel.self) private var painel
    let causas: [Causa]
    let fichas: MapaFichas?

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            NumeroSistema()
            ContaNoCartao().padding(.top, 10)
            // Só com medição (a automática ou a completa, conforme o alternador).
            if painel.dados != nil || !painel.manual { MaisOcupa(causas: causas, fichas: fichas).padding(.top, 15) }
            // No fim do cartão: o Bloqueado pelo macOS, ou os botões de calcular.
            // No Manual sem medição, o cartão do topo (Dados do Sistema) já tem o botão e o cronômetro.
            if painel.manual && painel.dados == nil {} else { LinhaMedicao().padding(.top, 15) }
        }
        .cartao(destaque: Marca.dados.opacity(0.5))
    }
}

/// A conta do Ajustes (usado − macOS − outras linhas = Dados do Sistema) direto no cartão.
private struct ContaNoCartao: View {
    @Environment(Painel.self) private var painel

    var body: some View {
        if let l = painel.telaBoa, l.tela.systemData != nil {
            Conta(lida: l, livre: painel.rapido?.capacity?.free, purg: painel.purgVivo?.bytes, destaque: .constant([]), cabecalho: false)
        }
    }
}

/// Coluna do número: o Dados do Sistema da tela do Ajustes, o último lido ou o estado vazio.
private struct NumeroSistema: View {
    @Environment(Painel.self) private var painel

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(spacing: 8) {
                // O mesmo símbolo da barra lateral.
                Image(systemName: Secao.sistema.simbolo).font(.title3).accessibilityHidden(true)
                Text("Dados do Sistema").font(.title3.weight(.semibold))
                    .help("Não é uma pasta: é o que o Ajustes não põe em nenhuma outra linha. A Apple não mede, subtrai.")
            }
            estado
        }
    }

    @ViewBuilder private var estado: some View {
        if let l = painel.telaBoa, let sd = l.tela.systemData {
            lido(l, sd)
        } else if painel.aguardandoAjustes {
            esperando
        } else {
            switch vazioAjustes(painel.rapido?.settingsScreen.screen, ultimo: nil) {
            case .v2:
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text(Vazio.v2.texto).fixedSize(horizontal: false, vertical: true)
                }
            case .v3:
                Text(Vazio.v3.texto).fixedSize(horizontal: false, vertical: true)
            default:
                if let u = sistemaBarra(tela: nil, pontos: painel.pontos) {
                    Text("Último lido: \(gb(u.bytes)) em \(Armazenamento.dataCurta(u.em))").foregroundStyle(.secondary)
                } else if !painel.manual && painel.dados == nil {
                    // Abrindo do zero: a primeira passada automática no lugar do "Não lido" (o
                    // cartão "Lendo o disco" de baixo some enquanto isso).
                    VStack(alignment: .leading, spacing: 4) {
                        HStack(spacing: 8) {
                            ProgressView().controlSize(.small)
                            Text("Lendo o disco").font(.headline)
                        }
                        Text("A primeira leitura automática leva cerca de 1 minuto.").foregroundStyle(.secondary)
                    }
                } else if !NSRunningApplication.runningApplications(withBundleIdentifier: Painel.idAjustes).isEmpty {
                    // Com o Ajustes aberto pelo dono, o app não o abre escondido (lerAjustes).
                    Text("Não lido: o Ajustes está aberto. Ao fechá-lo, o SDTally lê sozinho.").foregroundStyle(.secondary)
                } else {
                    Text("Não lido")
                }
            }
        }
    }

    /// O número com a hora da leitura ao lado (no tom de aviso depois de 15 min).
    @ViewBuilder private func lido(_ l: TelaLida, _ sd: Int64) -> some View {
        if let em = l.tela.loggedAt, let d = data(em) {
            TimelineView(.periodic(from: .now, by: 30)) { ctx in
                let velho = ctx.date.timeIntervalSince(d) > 15 * 60
                let i = idade(d, agora: ctx.date)
                let comDia = ctx.date.timeIntervalSince(d) >= 6 * 3600
                VStack(alignment: .leading, spacing: 4) {
                    HStack(alignment: .firstTextBaseline, spacing: 10) {
                        Text(gb(sd)).font(.heroi).tracking(-0.5).transicaoNumero(Double(sd)).fixedSize()
                        // Leitura de mais de 6 h: a idade já traz dia e hora ("de 05/10 às 18:49").
                        Text(comDia ? String(localized: "no Ajustes \(i)") : String(localized: "no Ajustes às \(hora(em)) · \(i)"))
                            .font(.caption).foregroundStyle(velho ? Marca.aviso : Color.secondary)
                    }
                    if let a = Historico.leituraAnterior(painel.pontos, telaEm: em), let antes = a.systemData {
                        desde(sd - antes, em: data(a.telaEm ?? a.t))
                    }
                    if painel.aguardandoAjustes {
                        esperando
                    } else if velho {
                        Button("Ler de novo do Ajustes") { painel.lerAjustes() }.buttonStyle(.link)
                    }
                }
            }
        } else {
            Text(gb(sd)).font(.heroi).tracking(-0.5).transicaoNumero(Double(sd))
        }
    }

    private var esperando: some View {
        HStack(spacing: 8) {
            ProgressView().controlSize(.small)
            Text("Esperando o Ajustes…")
        }
    }
}

// MARK: - A conta (popover do ⓘ)

/// Dados do Sistema = usado − macOS − outras linhas. A Apple não mede: subtrai.
private struct Conta: View {
    let lida: TelaLida
    var livre: Int64?
    var purg: Int64?
    @Binding var destaque: Set<String>
    /// Título e número grande; fora no cartão, que já os mostra.
    var cabecalho = true
    @State private var categorias = false

    var body: some View {
        let t = lida.tela
        VStack(alignment: .leading, spacing: 10) {
            if cabecalho {
                Text("Dados do Sistema").font(.title3.weight(.semibold))
                Text(t.systemData.map(gb) ?? "—")
                    .font(.heroi).tracking(-0.5)
                    .transicaoNumero(Double(t.systemData ?? 0))
                    .help(notaDisponivel ?? "")
            }
            ViewThatFits(in: .horizontal) {
                HStack(alignment: .top, spacing: 8) { fichas(t) }
                Grid(alignment: .leading, horizontalSpacing: 8, verticalSpacing: 8) {
                    GridRow { ficha(t.used, String(localized: "usado"), String(localized: "log do Ajustes"), ids: []); ficha(lida.macos.bytes, "macOS", nomeFonte(lida.macos.source), ids: ["macos"]) }
                    GridRow { fichaOutras(t); ficha(t.systemData, String(localized: "Dados do Sistema"), String(localized: "diferença"), ids: ["sd"], forte: true) }
                }
            }
        }
    }

    /// O Ajustes ora conta o que o macOS libera sozinho como disponível, ora dentro do usado
    /// (purgavelNoAjustes).
    private var notaDisponivel: String? {
        guard let disponivel = purgavelNoAjustes(tela: lida.tela, livre: livre, purg: purg),
              let y = lida.tela.available, let livre, let purg else { return nil }
        if !disponivel {
            return String(localized: "Nesta abertura do Ajustes, o que o macOS libera sozinho (\(gb(purg)), CacheDelete) está dentro de Dados do Sistema, não em Disponível: o Disponível da tela é só o livre.")
        }
        return String(localized: "O Ajustes conta \(gb(y - livre)) além do livre como Disponível (diferença entre o Disponível da tela e o livre do statfs); o macOS libera sozinho \(gb(purg)) (CacheDelete). O que não entra em Disponível fica em Dados do Sistema.")
    }

    @ViewBuilder private func fichas(_ t: Tela) -> some View {
        ficha(t.used, String(localized: "usado"), String(localized: "log do Ajustes"), ids: [])
        sinal("−")
        ficha(lida.macos.bytes, "macOS", nomeFonte(lida.macos.source), ids: ["macos"])
        sinal("−")
        fichaOutras(t)
        sinal("=")
        ficha(t.systemData, String(localized: "Dados do Sistema"), String(localized: "diferença"), ids: ["sd"], forte: true)
    }

    private func fichaOutras(_ t: Tela) -> some View {
        Button { categorias.toggle() } label: {
            ficha(t.categoriesTotal, String(localized: "outras linhas ›"), String(localized: "log do serviço"), ids: Set((t.categories ?? []).map(\.id)))
        }
        .buttonStyle(.plain)
        .help("Ver as linhas uma a uma, para conferir com a tela")
        .popover(isPresented: $categorias, arrowEdge: .bottom) {
            Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 6) {
                ForEach(t.categories ?? []) { c in
                    GridRow {
                        Text(categoria(c.id).nome)
                        Text(gb(c.bytes)).monospacedDigit().gridColumnAlignment(.trailing)
                    }
                }
                Divider()
                GridRow {
                    Text("Total").bold()
                    Text(t.categoriesTotal.map(gb) ?? "—").monospacedDigit().bold()
                }
            }
            .padding()
        }
    }

    private func ficha(_ b: Int64?, _ nome: String, _ fonte: String, ids: Set<String>, forte: Bool = false) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(b.map(gb) ?? "—")
                .font(.body.monospacedDigit().weight(forte ? .semibold : .regular))
                .contentTransition(.numericText(value: Double(b ?? 0)))
            Text(nome).font(.caption)
            Text(b == nil ? String(localized: "não medido") : fonte).font(.caption2).foregroundStyle(.tertiary)
        }
        .padding(8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.quaternary.opacity(forte ? 0.8 : 0.4), in: .rect(cornerRadius: 8))
        .onHover { dentro in destaque = dentro ? ids : [] }
    }

    private func sinal(_ s: String) -> some View {
        Text(s).font(.title3).foregroundStyle(.tertiary).padding(.top, 8)
    }
}

/// "Desde a última leitura": Dados do Sistema contra a abertura anterior do Ajustes no Histórico.
private func desde(_ d: Int64, em: Date?) -> some View {
    let mudou = abs(d) >= Historico.limite
    let valor = mudou ? (d > 0 ? "+" : "−") + gb(abs(d)) : String(localized: "sem mudança")
    let texto = em.map { String(localized: "Desde a última leitura (\(quando($0))): \(valor)") } ?? String(localized: "Desde a última leitura: \(valor)")
    return Label(texto, systemImage: !mudou ? "equal" : d > 0 ? "arrow.up.right" : "arrow.down.right")
        .font(.caption.monospacedDigit())
        .help("Dados do Sistema da tela do Ajustes contra a abertura anterior, guardada no Histórico")
}

// MARK: - B4: linha de medição

/// O único lugar da senha na tela (§5): o estado da medição completa e, quando cabe, um botão.
private struct LinhaMedicao: View {
    @Environment(Painel.self) private var painel
    /// A oferta do medidor root aparece uma vez, depois de uma medição com senha.
    @AppStorage("ofertaMedidor") private var ofertaMedidor = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // No Automático, sem botões de calcular: a medição com senha começa no cartão de cima
            // (CartaoTempoReal). Fica só a frase do que a leitura sem senha não alcança.
            if !painel.manual {
                // Lido como root (Automático com root), a frase do "sem senha" não vale.
                if let c = painel.dados, !c.root {
                    Text(Vazio.v12(c, comSenha: painel.medicoes.last { $0.completo.root }?.completo).texto)
                        .fixedSize(horizontal: false, vertical: true)
                }
            } else {
            switch painel.medicao {
            case .medindo(let desde, let admin, _):
                HStack(spacing: 10) {
                    ProgressView().controlSize(.small)
                    TimelineView(.periodic(from: desde, by: 1)) { ctx in
                        let t = decorrido(desde, ctx.date)
                        Text(admin ? String(localized: "Calculando como administrador… \(t)") : String(localized: "Calculando… \(t)")).monospacedDigit()
                    }
                    Spacer()
                    if painel.podeParar { Button("Parar") { painel.parar() } }
                }
            case .senhaCancelada:
                Text("Senha cancelada: nada foi medido")
                HStack {
                    botaoSenha
                    Button("Agora não") { painel.esquecerErro() }
                }
            case .falhou(let msg):
                Label(msg, systemImage: "exclamationmark.triangle").textSelection(.enabled)
                HStack {
                    botaoSenha
                    Button("Copiar detalhes") {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(msg, forType: .string)
                    }
                }
            case .parada:
                parada
            }
            }
        }
        .font(.callout)
        .animation(.smooth(duration: 0.2), value: painel.medindo)
    }

    @ViewBuilder private var parada: some View {
        if let c = painel.dados {
            if c.root {
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Text(fraseSemExplicacao(c))
                    InfoBloqueado(c: c)
                }
                if !restoReduzivel(c) {
                    Text("Pastas protegidas até do administrador e mudanças durante a medição. Nenhuma permissão reduz isto.")
                        .foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                }
            } else {
                HStack(alignment: .firstTextBaseline, spacing: 12) {
                    Text(Vazio.v12(c, comSenha: painel.medicoes.last { $0.completo.root }?.completo).texto)
                        .fixedSize(horizontal: false, vertical: true)
                    Spacer(minLength: 0)
                    botaoSenha
                }
            }
            oferta
        } else {
            HStack {
                Spacer()
                botaoSenha
            }
        }
    }

    /// Uma vez, depois de medir com senha: o medidor root dispensa a senha nas próximas.
    @ViewBuilder private var oferta: some View {
        if painel.modoCompleto == .senha, painel.helper != .enabled, !ofertaMedidor {
            HStack(spacing: 8) {
                if painel.helper == .requiresApproval {
                    Text("Aguardando aprovação em Itens de Início").foregroundStyle(.secondary)
                    Button("Abrir Itens de Início") { painel.abrirItensDeInicio() }
                } else {
                    Button("Instalar Root sem Senha…") { painel.perguntarInstalar = true }
                    Button("Agora não") { ofertaMedidor = true }
                }
            }
            if let e = painel.helperErro {
                Label(e, systemImage: "exclamationmark.triangle").font(.caption).foregroundStyle(Marca.aviso).textSelection(.enabled)
            }
        }
    }

    /// O único botão de senha da tela (§5; o grep do §10 conta este `medir`). Com o medidor root
    /// instalado, mede como administrador sem pedir a senha.
    @ViewBuilder private var botaoSenha: some View {
        let falhou = if case .falhou = painel.medicao { true } else { false }
        Button(falhou ? String(localized: "Tentar de novo") : painel.helperAtivo ? AcaoVazio.medidorRoot.rotulo : AcaoVazio.medir.rotulo) { painel.medirComSenha() }
            .buttonStyle(.borderedProminent)
            .disabled(painel.medindo)
            .help("Pastas do sistema e de outros usuários só abrem como administrador; sem a senha, ficam bloqueadas pelo macOS.")
    }

    private func decorrido(_ a: Date, _ b: Date) -> String {
        let s = max(0, Int(b.timeIntervalSince(a)))
        return String(format: "%d:%02d", s / 60, s % 60)
    }
}

// MARK: - B5: o que mais ocupa o disco

/// A barra do disco e as 5 maiores causas, que são a legenda dela: mesma cor e o mesmo destaque
/// ao passar o mouse. O clique abre o detalhe de uma linha por vez.
private struct MaisOcupa: View {
    @Environment(Painel.self) private var painel
    let causas: [Causa]
    let fichas: MapaFichas?
    @State private var destaque: Set<String> = []

    var body: some View {
        let c = painel.dados
        let cinco = Array(causas.prefix(10))  // as 10 maiores: o "Em seguida" entrou na lista
        // Do mesmo instante das causas; sem medição completa, o rápido.
        let segs = segmentosDisco(cinco, top: c?.top ?? [], containers: c != nil ? c?.containers : painel.rapido?.containers)
        VStack(alignment: .leading, spacing: 10) {
            cabecalho(c)
            if !segs.isEmpty {
                BarraView(segmentos: segs, altura: 12, destaque: destaque,
                          aoPassar: { destaque = $0.map { [$0] } ?? [] },
                          aoClicar: { id in if let x = cinco.first(where: { $0.id == id }) { painel.ver(x)?.ir() } })
                legenda(segs.suffix(3), resto: segs.count > 3)
            }
            // No Automático, até a primeira passada terminar (~1 min).
            if c == nil && (painel.medindo || !painel.manual) {
                esqueleto
            } else if let c {
                if cinco.isEmpty {
                    Text(painel.manual ? String(localized: "Nenhuma pasta passou de 1 GB (medido às \(hora(c.generatedAt))).") : String(localized: "Nenhuma pasta passou de 1 GB.")).foregroundStyle(.secondary)
                } else {
                    // A cor de cada linha é a da barra; o swap fica no macOS, cinza.
                    let semSwap = cinco.filter { $0.id != "swap" }.map(\.id)
                    VStack(alignment: .leading, spacing: 2) {
                        ForEach(cinco) { x in linha(x, c, cor: semSwap.firstIndex(of: x.id).map(corDisco) ?? .gray) }
                    }
                        .opacity(painel.medindo ? 0.5 : 1)
                }
            } else {
                // A senha fica só em B4.
                Text("Não medido").foregroundStyle(.secondary)
            }
        }
    }

    /// Título, hora e modo da medição; avisos de idade (mais de 24 h) e de medição interrompida.
    @ViewBuilder private func cabecalho(_ c: Completo?) -> some View {
        let d = c.flatMap { data($0.generatedAt) }
        let velho = d.map { Date.now.timeIntervalSince($0) > 24 * 3600 } ?? false
        let interrompida = c.map(Historico.interrompida) ?? false
        HStack(alignment: .firstTextBaseline) {
            Text("O que mais ocupa o disco").font(.title3.weight(.semibold))
            Spacer()
            // No Automático, sem hora nem modo: é ao vivo.
            if let c, painel.manual {
                let modo = c.complete ? String(localized: "completo") : interrompida ? String(localized: "interrompida") : c.incompleteReasons.contains("no_root") ? String(localized: "sem senha") : c.incompleteReasons.contains("no_full_disk_access") ? String(localized: "sem Acesso Total") : String(localized: "etapa com falha")
                let medido = d.map { String(localized: "Medido \(quando($0)) · \(modo)") } ?? String(localized: "Medido · \(modo)")
                Text(medido)
                    .font(.caption).foregroundStyle(velho || interrompida ? Marca.aviso : Color.secondary)
            }
        }
        // Era a contagem no rodapé da barra lateral; a lista fica em Dados do Sistema > Estado da medição.
        if let n = c?.stageErrors.count, n > 0 {
            Label(String(localized: "\(n) etapas com erro"), systemImage: "xmark.octagon")
                .font(.callout).foregroundStyle(Marca.aviso)
        }
    }

    /// Só os três segmentos que não são linhas da lista, sem números: o livre do diskutil não é o
    /// do herói (statfs).
    private func legenda(_ base: ArraySlice<Segmento>, resto: Bool) -> some View {
        HStack(spacing: 14) {
            ForEach(Array(zip(base, [resto ? String(localized: "Resto do volume de dados") : String(localized: "Volume de dados"), "macOS", String(localized: "Livre")])), id: \.0.id) { s, nome in
                HStack(spacing: 5) {
                    RoundedRectangle(cornerRadius: 2).fill(s.cor).frame(width: 10, height: 10)
                    Text(nome)
                }
                .onHover { destaque = $0 ? [s.id] : [] }
            }
        }
        .font(.caption).foregroundStyle(.secondary)
    }

    /// Uma causa numa linha só: ícone na cor do segmento da barra, nome e caminho, tamanho e o
    /// símbolo do nível da ficha (o nome do nível no .help). O clique leva ao "Ver…" da causa.
    private func linha(_ c: Causa, _ m: Completo, cor: Color) -> some View {
        let a = conselho(de: c, em: m.advice ?? [])
        let f = fichas?[a?.card] ?? fichas?[c.card]
        let n = f.map { (Marca.nivel($0.level), nomeNivel($0.level)) }
        let extras = [c.dono.flatMap { $0 == c.nome ? nil : $0 },
                      c.pastas > 1 ? String(localized: "\(c.pastas) pastas") : c.caminho.map(visivel).flatMap { $0 == c.nome ? nil : $0 }].compactMap { $0 }
        let extra = extras.isEmpty ? nil : extras.joined(separator: " · ")
        let ver = painel.ver(c)
        return Button { ver?.ir() } label: {
            HStack(spacing: 8) {
                IconeSecao(simbolo: c.id == "swap" ? "memorychip" : termo(c.termo).simbolo, cor: cor)
                Text(c.nome).lineLimit(1).truncationMode(.middle).layoutPriority(1)
                if let extra { Text("· " + extra).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle) }
                Spacer(minLength: 8)
                Text(gb(c.bytes)).monospacedDigit().frame(width: 80, alignment: .trailing)
                ZStack {
                    if let n { Image(systemName: n.0.simbolo).foregroundStyle(n.0.cor).help(n.1) }
                }
                .frame(width: 24)
            }
            .lineLimit(1)
            .frame(height: 32)
            .padding(.horizontal, 6)
            .background(Color.secondary.opacity(destaque.contains(c.id) ? 0.1 : 0), in: .rect(cornerRadius: 6))
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        // O swap fica no macOS da barra.
        .onHover { destaque = $0 ? (c.id == "swap" ? [c.id, "macos"] : [c.id]) : [] }
        .accessibilityElement(children: .combine)
        .accessibilityLabel([c.nome, gb(c.bytes), n?.1].compactMap { $0 }.joined(separator: ", "))
        .help(f?.what.texto ?? "")
    }

    /// Medindo pela primeira vez: 10 linhas no lugar da lista.
    private var esqueleto: some View {
        VStack(alignment: .leading, spacing: 2) {
            ForEach(0..<10, id: \.self) { _ in
                HStack(spacing: 8) {
                    IconeSecao(simbolo: "folder", cor: .gray)
                    Text("Pasta medida no disco")
                    Spacer()
                    Text("00,00 GB")
                }
                .frame(height: 32)
                .padding(.horizontal, 6)
            }
        }
        .redacted(reason: .placeholder)
    }
}

// MARK: - B6: depois da lista

/// O link para todas as pastas, "Seguro" (o primeiro passo seguro fora das 10 linhas) e "O que
/// mudou" desde a medição anterior. As causas 6 a 10 ("Em seguida") entraram na lista. Cada linha só aparece com conteúdo; sem contagem nem soma (G9).
private struct DepoisDaLista: View {
    @Environment(Painel.self) private var painel
    let causas: [Causa]

    var body: some View {
        let s = seguro(advice: painel.dados?.advice ?? [], cinco: Array(causas.prefix(10)))
        Grid(alignment: .topLeading, horizontalSpacing: 12, verticalSpacing: 6) {
            if causas.count > 10 {
                GridRow {
                    Color.clear.frame(width: 0, height: 0)
                    Button("Todas as pastas (⌘4) ›") {
                        painel.pilha = []
                        painel.secao = .ranking
                    }
                    .buttonStyle(.link)
                }
            }
            if let s {
                let titulo = "\(tituloConselho(s.id) ?? s.term.map(nomeTermo) ?? String(localized: "Recomendação do sdtally")) · \(gb(s.bytes)) ›"
                GridRow {
                    etiqueta(String(localized: "Seguro"))
                    Button(titulo) { painel.abrir(.fazer, rolando: s.id) }
                        .buttonStyle(.link).lineLimit(1).help(titulo)
                }
            }
            if let m = mudou {
                GridRow {
                    etiqueta(String(localized: "Desde a medição"))
                    Button(m.curto) { painel.secao = .historico }
                        .buttonStyle(.link).lineLimit(1).help(m.completo)
                }
            }
        }
        .font(.callout)
    }

    private func etiqueta(_ t: String) -> some View {
        Text(t).fontWeight(.semibold).foregroundStyle(.secondary).gridColumnAlignment(.leading)
    }

    /// Um item por causa: só o nome (último componente se for caminho) e o tamanho, numa linha só;
    /// o nome inteiro fica no .help. Sem "Ver…" (painel.ver nil), vira texto.
    @ViewBuilder private func itens(_ cs: [Causa]) -> some View {
        ForEach(cs) { c in
            let texto = "\(Self.nomeCurto(c.nome)) \(gb(c.bytes))"
            Group {
                if let destino = painel.ver(c) {
                    Button(texto) { destino.ir() }.buttonStyle(.link)
                } else {
                    Text(texto)
                }
            }
            .lineLimit(1).fixedSize().help("\(c.nome) \(gb(c.bytes))")
        }
    }

    /// "Application Support" de "~/Library/Application Support"; nome que não é caminho fica como está.
    static func nomeCurto(_ n: String) -> String {
        n.hasPrefix("~") || n.hasPrefix("/") ? (n as NSString).lastPathComponent : n
    }

    /// As 3 maiores mudanças entre as duas últimas medições salvas, só linhas do `top` (o termo
    /// "Arquivos e pastas" fica no Histórico: já soma as pastas); nil se não se comparam ou nada mudou.
    private var mudou: (curto: String, completo: String)? {
        let ms = painel.medicoes
        guard ms.count >= 2, let antes = ms.dropLast().last?.completo, let agora = ms.last?.completo,
              let mud = Historico.mudancas(de: antes, para: agora)?.filter({ $0.linha != nil }), !mud.isEmpty else { return nil }
        // Caminho curto (último componente) na linha; o inteiro no .help.
        let itens = mud.prefix(3).map { x -> (curto: String, completo: String) in
            let d = (x.depois ?? 0) - (x.antes ?? 0)
            let delta = " " + (d > 0 ? "+" : "−") + gb(abs(d))
            let nome = visivel(x.linha.map { rotulo($0, home: agora.home) } ?? nomeTermo(x.termo))
            return (Self.nomeCurto(nome) + delta, nome + delta)
        }
        let dia = Armazenamento.dataCurta(antes.generatedAt)
        let listaCurta = itens.map(\.curto).joined(separator: " · ")
        let listaCompleta = itens.map(\.completo).joined(separator: " · ")
        return (String(localized: "de \(dia): \(listaCurta) ›"), String(localized: "de \(dia): \(listaCompleta)"))
    }
}

/// ⓘ do Bloqueado pelo macOS: deixa claro que é bloqueio do sistema, não falha da medição.
private struct InfoBloqueado: View {
    let c: Completo
    @State private var aberto = false

    var body: some View {
        Button { aberto.toggle() } label: { Image(systemName: "info.circle") }
            .buttonStyle(.borderless)
            .accessibilityLabel("O que é Bloqueado pelo macOS")
            .popover(isPresented: $aberto, arrowEdge: .bottom) {
                let e = explicarBloqueado(c)
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(e.texto, id: \.self) { Text($0).fixedSize(horizontal: false, vertical: true) }
                    if !e.pastas.isEmpty {
                        ScrollView {
                            VStack(alignment: .leading, spacing: 2) {
                                ForEach(e.pastas, id: \.self) { Text($0).font(.caption.monospaced()).textSelection(.enabled) }
                            }
                        }
                        .frame(maxHeight: 160)
                    }
                }
                .font(.callout)
                .padding(14)
                .frame(width: 380)
            }
    }
}

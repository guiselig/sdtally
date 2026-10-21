// Protótipo (Fase 4): explica o "Dados do Sistema" do Ajustes com os números do sdtally embutido.
// Só lê. Rápido sem senha a cada 180 s (30 min em segundo plano); medição completa sob demanda.
// Janela principal e ícone na barra de menus (BarraMenus.swift), os dois sobre Painel.unico.
import AppKit
import SwiftUI
import UserNotifications

@main
struct SDTallyApp: App {
    @NSApplicationDelegateAdaptor private var delegado: Delegado
    @AppStorage("barraDeMenus") private var barraDeMenus = true
    private let painel = Painel.unico

    init() {
        UserDefaults.standard.register(defaults: ["barraDeMenus": true])
        Lingua.ajustarDirecao()
    }

    var body: some Scene {
        Window("SDTally", id: "principal") {
            JanelaView().environment(painel).windowFullScreenBehavior(.disabled).background(NaTelaPrincipal())
        }
        // Largura fixa e altura mínima de JanelaView.tamanho (como o Ajustes); a altura cresce até a tela. Sem tela cheia.
        .windowResizability(.contentSize)
        // Abre na altura toda da tela principal, grudada na borda esquerda (como Globe+Ctrl+←); sem restaurar o quadro anterior, que ganharia do placement.
        .restorationBehavior(.disabled)
        .defaultWindowPlacement { _, _ in
            // Só o tamanho: com monitor externo, o SwiftUI escolhe a tela e ignora a posição pedida
            // noutra. Quem põe na tela principal é o NaTelaPrincipal, ao abrir.
            let h = (telaPrincipal()?.visibleFrame.height).map { max($0, JanelaView.tamanho.height) } ?? JanelaView.tamanho.height
            return WindowPlacement(size: CGSize(width: JanelaView.tamanho.width, height: h))
        }
        // Sem barra de título nem de ferramentas: o semáforo fica sobre a barra lateral.
        .windowStyle(.hiddenTitleBar)
        // Com o ícone, o início de sessão mostra só ele. Fora do .app a janela abre sempre (fotos).
        .defaultLaunchBehavior(barraDeMenus && !Dev.ativo ? .suppressed : .automatic)
        .commands { Menus(painel: painel) }

        MenuBarExtra(isInserted: $barraDeMenus) {
            BarraMenusView().environment(painel)
        } label: {
            RotuloBarra().environment(painel)
        }
        .menuBarExtraStyle(.window)
    }
}

/// Menus do app (spec-fase5 §2.2). Exportar e "medições salvas" chamam Relatorio e Historico.
private struct Menus: Commands {
    let painel: Painel
    @Environment(\.openWindow) private var abrirJanela

    var body: some Commands {
        CommandGroup(replacing: .appInfo) {
            Button("Sobre o SDTally") { ir(.info) }
        }
        // Janela única: abre a "principal" ou só a traz para a frente.
        CommandGroup(replacing: .newItem) {
            Button("Nova janela") { ir(painel.secao ?? .sistema) }.keyboardShortcut("n")
        }
        CommandGroup(replacing: .appSettings) {
            Button("Ajustes…") { ir(.permissoes) }.keyboardShortcut(",")
            Button("Boas-vindas e dicas") { boasVindas() }
        }
        CommandGroup(replacing: .importExport) {
            Button("Exportar PDF…") { Relatorio.exportarPDF(painel) }
                .keyboardShortcut("e")
                .disabled(painel.completo == nil)
            Button("Exportar JSON…") { Relatorio.exportarJSON(painel) }
                .keyboardShortcut("e", modifiers: [.command, .option])
                .disabled(painel.completoDados == nil)
            Divider()
            Button("Mostrar medições salvas no Finder") {
                if let p = Self.pastaDados { NSWorkspace.shared.activateFileViewerSelecting([p]) }
            }
            .disabled(Self.pastaDados == nil)
        }
        CommandMenu("Medição") {
            Button(painel.helperAtivo ? String(localized: "Calcular como administrador") : String(localized: "Calcular com senha de administrador…")) { painel.medirComSenha() }
                .keyboardShortcut("r")
                .disabled(painel.medindo || painel.soAuto)
            if painel.helperAtivo {
                Button("Calcular com senha, sem o Root sem Senha…") { painel.medir(admin: true, semHelper: true) }
                    .disabled(painel.medindo)
            }
            Button("Parar") { painel.parar() }
                .keyboardShortcut(".")
                .disabled(!painel.podeParar)
            Divider()
            // O alternador do cabeçalho (AlternadorModo), só nas telas que o têm.
            let comModo = AlternadorModo.mostra(painel.secao ?? .sistema) && !painel.soAuto
            Button("Automático") { withAnimation(.smooth(duration: 0.3)) { painel.manual = false } }
                .keyboardShortcut(.leftArrow)
                .disabled(!comModo || !painel.manual)
            Button("Manual") { withAnimation(.smooth(duration: 0.3)) { painel.manual = true } }
                .keyboardShortcut(.rightArrow)
                .disabled(!comModo || painel.manual)
            Divider()
            Button("Atualizar a leitura do armazenamento") { Task { await painel.atualizar(Set(ParteRapido.allCases)) } }
                .keyboardShortcut("r", modifiers: [.command, .shift])
            Button("Abrir Armazenamento no Ajustes") { painel.abrirArmazenamento() }
                .keyboardShortcut("a", modifiers: [.command, .option])
            Divider()
            if painel.helper == .notRegistered || painel.helper == .notFound {
                Button("Instalar Root sem Senha…") { painel.perguntarInstalar = true }
            } else {
                Button("Remover Root sem Senha") { painel.removerHelper() }
            }
        }
        CommandMenu("Ir") {
            ForEach(Array(Secao.barra.enumerated()), id: \.element) { i, s in
                Button(s.titulo) { ir(s) }.keyboardShortcut(KeyEquivalent(Character("\(i + 1)")))
            }
            Divider()
            // Pastas maiores: sobe uma pasta. Dados do Sistema: um nível da barra que se desdobra.
            let barra = painel.secao == .sistema && !painel.pilhaBarra.isEmpty
            Button("Voltar") { if barra { painel.pilhaBarra.removeLast() } else { painel.pilha.removeLast() } }
                .keyboardShortcut("[")
                .disabled(!barra && (painel.secao != .ranking || painel.pilha.isEmpty))
        }
        CommandGroup(replacing: .help) {
            Button("Boas-vindas e dicas") { boasVindas() }
            Button("Abrir repositório no navegador") { NSWorkspace.shared.open(urlRepositorio) }
        }
    }

    private static let pastaDados = Historico.pastaLeitura(env: Foto.env, bundleID: Bundle.main.bundleIdentifier)

    /// Vai para a seção e traz a janela para a frente (reabre se foi fechada).
    private func ir(_ s: Secao) {
        painel.secao = s
        abrirJanela(id: "principal")
        NSApp.activate()
    }

    private func boasVindas() {
        abrirJanela(id: "principal")
        NSApp.activate()
        painel.mostrarBoasVindas = true
    }
}

/// Dono do laço de leitura (Painel.iniciar) e do ciclo de vida: com o ícone na barra de menus,
/// fechar a janela não fecha o app. Fora de um .app (binário cru com SDTALLY_BIN), vira app
/// normal para ter Dock e janela na frente, e nunca toca nas notificações.
final class Delegado: NSObject, NSApplicationDelegate, UNUserNotificationCenterDelegate {
    /// Aberto pelo início de sessão: lido no evento de abertura, que só existe até aqui.
    private var aoLogin = false
    func applicationWillFinishLaunching(_ notification: Notification) {
        Foto.aoIniciar()
        Self.ajustarDock(janela: false)
        let e = NSAppleEventManager.shared().currentAppleEvent
        aoLogin = e?.paramDescriptor(forKeyword: keyAEPropData)?.enumCodeValue == keyAELaunchedAsLogInItem
    }
    func applicationDidFinishLaunching(_ notification: Notification) {
        if Dev.ativo {
            NSApp.setActivationPolicy(.regular)
            NSApp.activate()
        } else {
            UNUserNotificationCenter.current().delegate = self
        }
        Painel.unico.iniciar()
        abrirSeFoiAMao()
        // Ícone da barra desligado sem janela aberta: o app ficaria sem Dock e sem ícone. Abre a janela.
        NotificationCenter.default.addObserver(forName: UserDefaults.didChangeNotification, object: nil, queue: .main) { _ in
            Task { @MainActor in
                guard !Dev.ativo, !UserDefaults.standard.bool(forKey: "barraDeMenus"), NSApp.activationPolicy() == .accessory else { return }
                Painel.unico.mostrar(Painel.unico.secao ?? .sistema)
            }
        }
        NSWorkspace.shared.notificationCenter.addObserver(forName: NSWorkspace.didWakeNotification, object: nil, queue: .main) { _ in
            Task { @MainActor in await Painel.unico.atualizar() }
        }
        // A tela do Ajustes só muda com o Ajustes aberto: lê pelo aviso de abrir e fechar, sem ritmo fixo.
        for (nome, abriu) in [(NSWorkspace.didLaunchApplicationNotification, true), (NSWorkspace.didTerminateApplicationNotification, false)] {
            NSWorkspace.shared.notificationCenter.addObserver(forName: nome, object: nil, queue: .main) { n in
                guard (n.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication)?.bundleIdentifier == Painel.idAjustes else { return }
                Task { @MainActor in Painel.unico.ajustesMudou(abriu: abriu) }
            }
        }
    }
    func applicationDidBecomeActive(_ notification: Notification) {
        Painel.unico.atualizarHelper()  // aprovação em Ajustes volta com o app ativo
        Task { await Painel.unico.atualizar() }
    }
    func applicationWillTerminate(_ notification: Notification) {
        Painel.unico.fecharAjustes()
        Painel.unico.pararEscuta()
    }
    /// Com o ícone na barra, a janela não abre sozinha (.suppressed), para o início de sessão
    /// mostrar só o ícone. Aberto à mão (Finder, Dock, Spotlight), a janela abre.
    private func abrirSeFoiAMao() {
        guard !Dev.ativo, !aoLogin, UserDefaults.standard.bool(forKey: "barraDeMenus") else { return }
        let p = Painel.unico
        if p.abrirJanela != nil { p.mostrar(p.secao ?? .sistema) } else { p.abrirAoAparecer = true }
    }
    /// Com o ícone na barra e sem a janela: sem Dock nem menus (.accessory). Com a janela, ou sem o
    /// ícone: app normal (.regular), e a janela vem para a frente.
    static func ajustarDock(janela: Bool) {
        guard !Dev.ativo else { return }
        let p: NSApplication.ActivationPolicy = janela || !UserDefaults.standard.bool(forKey: "barraDeMenus") ? .regular : .accessory
        if NSApp.activationPolicy() != p { NSApp.setActivationPolicy(p) }
        if janela { NSApp.activate() }
    }
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if !flag { Painel.unico.mostrar(Painel.unico.secao ?? .sistema) }
        return true
    }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        !UserDefaults.standard.bool(forKey: "barraDeMenus")
    }

    /// Clique na notificação: abre o Histórico.
    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse) async {
        await MainActor.run { Painel.unico.mostrar(.historico) }
    }
}

/// Barra lateral (spec-inicio §1): os 6 de `barra`, na ordem dos atalhos ⌘1 a ⌘6. Permissões e
/// Sobre ficam fora da lista (⚙ e ⓘ no rodapé), mas no enum: o menu e o Foto usam. O raw value
/// guarda a "Última tela". Só o símbolo, sem caixa colorida.
enum Secao: String, Hashable, CaseIterable {
    case sistema, fazer, armazenamento, ranking, historico, permissoes, info

    /// Sem Início: Dados do Sistema é a primeira tela.
    static let barra: [Secao] = [.sistema, .armazenamento, .ranking, .historico, .fazer]

    var titulo: String {
        switch self {
        case .sistema: String(localized: "Dados de Sistema")
        case .fazer: String(localized: "Recomendações")
        case .armazenamento: String(localized: "Disco")
        case .ranking: String(localized: "Caminhos")
        case .historico: String(localized: "Histórico")
        case .permissoes: String(localized: "Configurações")
        case .info: String(localized: "Informações")
        }
    }

    var simbolo: String {
        switch self {
        case .sistema: "square.stack.3d.up.fill"
        case .fazer: "checklist"
        case .armazenamento: "internaldrive.fill"
        case .ranking: "list.number"
        case .historico: "chart.xyaxis.line"
        case .permissoes: "gearshape"
        case .info: "info.circle"
        }
    }
}

/// Quadrado de 20 pt com o símbolo branco, como no Ajustes.
struct IconeSecao: View {
    let simbolo: String
    let cor: Color

    var body: some View {
        Image(systemName: simbolo)
            .font(.system(size: 11, weight: .semibold))
            .foregroundStyle(.white)
            .frame(width: 20, height: 20)
            .background(cor.gradient, in: .rect(cornerRadius: 5, style: .continuous))
            .accessibilityHidden(true)
    }
}

let urlRepositorio = URL(string: "https://github.com/guiselig/sdtally")!
let urlVersoes = URL(string: "https://github.com/guiselig/sdtally/releases")!

/// Barra lateral: só ícones (o nome de cada um vai no `.help`), sempre. De cima para baixo: as
/// cinco seções, o Medir e o rodapé (Github, Informações e Configurações). As seções são `List`
/// de barra lateral com a seleção nativa do sistema; Github e Medir são ações, botões simples.
private struct BarraLateral: View {
    @Environment(Painel.self) private var painel

    var body: some View {
        VStack(spacing: 4) {
            lista(Secao.barra)
            // Com o Automático com root, não há medição manual (nem a divisória que a separava).
            if !painel.soAuto {
                Divider().padding(.horizontal, 6).padding(.vertical, 6)
                medir
            }
            Spacer(minLength: 0)
            botao("Github", nome: "Github") { LogoGithub().frame(width: ladoRedondo, height: ladoRedondo) } acao: { NSWorkspace.shared.open(urlRepositorio) }
            // A List tem margem própria em cima: sem ela, o Github ficava mais longe do Informações
            // do que o Informações do Configurações.
            lista([.info, .permissoes]).padding(.top, -15)
        }
        // 24 pt a mais que o conteúdo: o primeiro ícone longe do semáforo.
        .padding(.top, JanelaView.topoSemaforo - JanelaView.alturaTitulo + 24)
        .padding(.bottom, 12)
        .frame(width: JanelaView.larguraBarra)
        .frame(maxHeight: .infinity)
    }

    /// Duas listas (as cinco de cima e as duas do rodapé) com a mesma seleção, `painel.secao`:
    /// cada uma só marca as suas. Altura fixa, sem rolagem: cabem inteiras.
    private func lista(_ secoes: [Secao]) -> some View {
        let selecao = Binding<Secao?>(
            get: { painel.secao.flatMap { secoes.contains($0) ? $0 : nil } },
            set: { if let s = $0 { painel.secao = s } })
        return List(secoes, id: \.self, selection: selecao) { s in
            Simbolo(nome: s.simbolo, redondo: s == .info || s == .permissoes, escala: s == .sistema ? 1.11 : 1)
                .frame(maxWidth: .infinity)
                .frame(height: passoBarra - 8)
                .help(s == .permissoes ? String(localized: "Configurações (⌘,)") : s.titulo)
                .accessibilityLabel(s.titulo)
                .tag(s)
        }
        .listStyle(.sidebar)
        .scrollContentBackground(.hidden)
        .scrollDisabled(true)
        // A List rola até a linha escolhida quando o conteúdo passa da altura dela, e os ícones
        // subiam uns pontos fora da primeira linha: 30 pt de folga embaixo, por cima do que vem depois.
        .frame(height: CGFloat(secoes.count) * passoBarra + 12 + 30)
        .frame(height: CGFloat(secoes.count) * passoBarra + 12, alignment: .top)
    }

    private func botao(_ ajuda: String, nome: String, @ViewBuilder icone: () -> some View, acao: @escaping () -> Void) -> some View {
        Button(action: acao) { Celula(icone: icone) }
            .buttonStyle(.plain)
            .focusEffectDisabled()
            .help(ajuda)
            .accessibilityLabel(nome)
    }

    /// Medir (ação que antes ficava na barra de ferramentas): o menu tem com e sem senha. Medindo,
    /// vira um indicador de progresso e, se dá para interromper, aparece o Parar acima dele.
    @ViewBuilder private var medir: some View {
        if painel.podeParar {
            botao(String(localized: "Parar a medição (⌘.)"), nome: String(localized: "Parar")) {
                Simbolo(nome: "stop.circle", redondo: true)
            } acao: { painel.parar() }
        }
        if painel.medindo {
            Celula { ProgressView().controlSize(.small) }
                .help("Calculando…")
                .accessibilityLabel("Calculando")
        } else {
            // Um clique já pede a senha e mede (e liga o Manual, onde fica o cronômetro).
            Button { painel.medirComSenha() } label: {
                Celula { Simbolo(nome: "gauge.with.dots.needle.50percent", redondo: true).foregroundStyle(.tint) }
            }
            .buttonStyle(.plain)
            .help(painel.helperAtivo ? String(localized: "Medir como administrador (⌘R)") : String(localized: "Medir com senha (⌘R)"))
            .accessibilityLabel("Medir com senha")
        }
    }
}

private let larguraCelula: CGFloat = 44
/// Todo ícone da barra cabe na mesma caixa: no mesmo tamanho de fonte, os símbolos saem com tamanhos diferentes à vista.
private let ladoIcone: CGFloat = 20
/// Altura da caixa: a da casinha (mais larga que alta) nesse lado; os redondos ficam com esse diâmetro.
private let alturaIcone: CGFloat = 18

/// Redondos (círculo, engrenagem, logo) parecem menores na mesma caixa: ganham uma caixa quadrada maior.
private let ladoRedondo: CGFloat = 20
/// Distância entre os ícones da barra lateral (linha da List e célula dos botões).
private let passoBarra: CGFloat = 45

private struct Simbolo: View {
    let nome: String
    var redondo = false
    /// Ajuste à vista de um símbolo que, na caixa comum, parece menor que os vizinhos.
    var escala: CGFloat = 1
    var body: some View {
        Image(systemName: nome).resizable().scaledToFit()
            .frame(width: (redondo ? ladoRedondo : ladoIcone) * escala, height: (redondo ? ladoRedondo : alturaIcone) * escala)
    }
}

/// Célula de 44 × 38 pt dos botões de ação (Github, Medir): o ícone no centro.
private struct Celula<Icone: View>: View {
    let icone: Icone
    init(@ViewBuilder icone: () -> Icone) { self.icone = icone() }

    var body: some View {
        icone.frame(width: larguraCelula, height: passoBarra).contentShape(.rect)
    }
}

/// Fundo da barra lateral: o material "sidebar" do sistema.
private struct FundoBarra: NSViewRepresentable {
    func makeNSView(context: Context) -> NSVisualEffectView {
        let v = NSVisualEffectView()
        v.material = .sidebar
        v.blendingMode = .behindWindow
        // Sempre a cor chapada da janela inativa (pedido do dono): sem o vidro que deixa o fundo vazar.
        v.state = .inactive
        return v
    }
    func updateNSView(_ v: NSVisualEffectView, context: Context) {}
}

/// A tela principal (a da barra de menus, em Ajustes > Telas), não importa onde está o ponteiro.
@MainActor func telaPrincipal() -> NSScreen? { NSScreen.screens.first }

/// Ao entrar na janela, põe ela na tela principal, na altura toda e na borda esquerda.
private struct NaTelaPrincipal: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView { Vista() }
    func updateNSView(_ v: NSView, context: Context) {}

    final class Vista: NSView {
        override func viewDidMoveToWindow() {
            guard let w = window, let v = telaPrincipal()?.visibleFrame else { return }
            // Depois do placement do SwiftUI, que viria por cima.
            DispatchQueue.main.async { w.setFrame(NSRect(x: v.minX, y: v.minY, width: w.frame.width, height: v.height), display: true) }
        }
    }
}

struct JanelaView: View {
    /// Janela inteira: largura fixa, a do Ajustes (740 pt, medida no macOS 27), e altura mínima. Sem barra de título
    /// (`.hiddenTitleBar`): o conteúdo vai até o topo.
    static let tamanho = CGSize(width: 740, height: 567)
    /// A janela soma a altura da barra de título (28 pt no macOS 26 e 27, medida) ao conteúdo, mesmo escondida.
    static let alturaTitulo: CGFloat = 28
    /// Do verde do semáforo até a divisória, a mesma distância (10 pt) da borda até o vermelho.
    static let larguraBarra: CGFloat = 72
    /// Faixa de cima, onde o semáforo fica sobre o conteúdo: nada é desenhado nela.
    static let topoSemaforo: CGFloat = 36

    @Environment(Painel.self) private var painel
    @Environment(\.openWindow) private var abrirJanela
    @AppStorage("viuBoasVindas") private var viuBoasVindas = false
    /// Lida por Foto.secaoInicial com "Abrir em: Última tela" (Permissões e aparência).
    @AppStorage("ultimaSecao") private var ultimaSecao = Secao.sistema.rawValue

    var body: some View {
        @Bindable var painel = painel
        HStack(spacing: 0) {
            BarraLateral()
            Color.clear.frame(width: 1)  // o lugar da divisória, que o fundo desenha
            // O VStack: um Divider solto (o do rodapé de O que fazer) segue o eixo da pilha em volta, e viraria vertical.
            // Cabeçalho desde o topo da janela (por baixo da barra de título escondida): o título da
            // tela e, embaixo, o alternador. Troca de tela curta: a nova entra num fade com um leve deslize.
            VStack(spacing: 0) {
                cabecalho
                VStack(spacing: 0) {
                    detalhe
                        .id(painel.secao)
                        .transition(.opacity.combined(with: .offset(y: 8)))
                }
                .animation(.easeOut(duration: 0.18), value: painel.secao)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            .ignoresSafeArea(.container, edges: .top)
        }
        // O fundo vai até o topo da janela, por baixo do semáforo; o conteúdo fica abaixo da barra de título.
        .background {
            HStack(spacing: 0) {
                FundoBarra().frame(width: Self.larguraBarra)
                Divider()
                Color(nsColor: .windowBackgroundColor)
            }
            .ignoresSafeArea()
        }
        .modifier(AvisoInstalarRoot())
        .sheet(isPresented: $painel.mostrarBoasVindas) { BoasVindasView().environment(painel) }
        .onAppear {
            Delegado.ajustarDock(janela: true)
            if !viuBoasVindas && Foto.env["SDTALLY_FOTO"] == nil { painel.mostrarBoasVindas = true }
        }
        .onDisappear { Delegado.ajustarDock(janela: false) }
        .onChange(of: painel.secao) { _, s in if let s { ultimaSecao = s.rawValue } }
        .frame(width: Self.tamanho.width).frame(minHeight: Self.tamanho.height - Self.alturaTitulo, maxHeight: .infinity)
        .task {
            painel.abrirJanela = abrirJanela
            Foto.preparar(painel)
            await Foto.tirarSePedido(painel)
        }
    }

    /// 14 pt do topo até o título, 10 pt até o alternador e 14 pt até o conteúdo.
    private var cabecalho: some View {
        let s = painel.secao ?? .sistema
        return VStack(spacing: 10) {
            Text(s.titulo).font(.title3.weight(.semibold))
            if AlternadorModo.mostra(s) && !painel.soAuto { AlternadorModo() }
        }
        .frame(maxWidth: .infinity)
        .padding(.top, 14).padding(.bottom, 14)
    }

    @ViewBuilder private var detalhe: some View {
        let s = painel.secao ?? .sistema
        switch s {
        case .fazer: ConselhosView().navigationTitle(s.titulo)
        case .sistema: SistemaView().navigationTitle(s.titulo)
        case .armazenamento: ArmazenamentoView().navigationTitle(s.titulo)
        case .ranking: RankingView()
        case .historico: HistoricoView().navigationTitle(s.titulo)
        case .permissoes: PermissoesView().navigationTitle(s.titulo)
        case .info: InfoView().navigationTitle(s.titulo)
        }
    }
}

// Permissões e aparência (spec-fase5 §2.4 e §5; spec-inicio §6): Acesso Total ao Disco, medidor
// root, conta de administrador, "Comportamento" (A6), "Atualização automática", "Aparência" e
// "Avançado" (o Automático com root).
// Dono: A2. API estável:
//   PermissoesView()     lê Painel do ambiente
//   LinhaPermissao       estado, nome, para que serve, o que se perde sem ela e o botão
//   ehAdmin(_ uid:)      cópia da do helper (SDTallyHelper não muda nesta fase)
import ServiceManagement
import SwiftUI
import UserNotifications

/// Membro do grupo "admin". Cópia da função do helper, que fica com a dele.
nonisolated func ehAdmin(_ uid: uid_t) -> Bool {
    guard let pw = getpwuid(uid), let admin = getgrnam("admin") else { return false }
    var grupos = [Int32](repeating: 0, count: 256)
    var n = Int32(grupos.count)
    guard getgrouplist(pw.pointee.pw_name, Int32(bitPattern: pw.pointee.pw_gid), &grupos, &n) != -1 else { return false }
    return grupos.prefix(Int(n)).contains(Int32(bitPattern: admin.pointee.gr_gid))
}

struct PermissoesView: View {
    @Environment(Painel.self) private var painel
    /// Lido por Foto.secaoInicial (a seção ao abrir: "inicio" ou "ultima").
    @AppStorage("abrirEm") private var abrirEm = "inicio"
    private let admin = ehAdmin(getuid())

    /// O app tem Acesso Total e a última medição completa rodou sem ele: o caso que a faixa explica.
    /// O contrário (desligado agora, ligado antes) já aparece no estado e no selo da barra lateral.
    nonisolated static func acessoDivergente(app: Bool?, medicao: Bool?) -> Bool {
        app == true && medicao == false
    }

    var body: some View {
        Form {
            Section {
                LinhaAcessoTotal()
                LinhaMedidorRoot()
                LinhaPermissao(
                    nome: String(localized: "Conta de administrador"), ok: admin,
                    estados: [admin ? String(localized: "Sim") : String(localized: "Não")],
                    serve: String(localized: "A senha, o Root sem Senha e a leitura do registro do Ajustes exigem uma conta de administrador."),
                    semEla: nil
                ) { EmptyView() }
            }
            Section("Comportamento") {
                ToggleBarraDeMenus()
                PickerRotuloBarra()
                ToggleNotificar()
                ToggleIniciarSessao()
            }
            Section("Atualização automática") {
                RitmosView()
            }
            Section("Aparência") {
                PickerLingua()
                Picker("Abrir em", selection: $abrirEm) {
                    Text("Dados do Sistema").tag("inicio")
                    Text("Última tela").tag("ultima")
                }
            }
            Section("Avançado") {
                PickerAutoRoot()
            }
            Section("Versões") {
                LinhaAtualizador()
            }
        }
        .formStyle(.grouped)
        // Sem o fundo próprio do Form (mais escuro): a cor da janela, igual ao cabeçalho.
        .scrollContentBackground(.hidden)
    }
}

/// Uma permissão: símbolo do estado, nome, estados e o botão. Para que serve e o que se perde sem
/// ela só aparecem no tooltip.
/// `ok` nil = neutro (indeterminado, não instalado ou fora do .app).
struct LinhaPermissao<Botao: View>: View {
    let nome: String
    let ok: Bool?
    let estados: [String]
    let serve: String
    let semEla: String?
    /// Mostra o "para que serve" embaixo do nome (boas-vindas) em vez de só no tooltip.
    var explicar = false
    @ViewBuilder var botao: Botao

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: ok == true ? "checkmark.circle.fill" : ok == false ? "exclamationmark.triangle.fill" : "minus.circle")
                .font(.title2)
                .foregroundStyle(ok == true ? Marca.bom : ok == false ? Marca.aviso : .secondary)
                .frame(width: 28)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 4) {
                Text(nome).font(.headline)
                ForEach(estados, id: \.self) { Text($0).font(.callout) }
                // Nas boas-vindas o botão vai embaixo do texto: ao lado, cortava em de, el, hu e uk.
                if explicar {
                    Text(serve).font(.callout).foregroundStyle(.secondary)
                    botao.fixedSize().padding(.top, 4)
                }
            }
            .fixedSize(horizontal: false, vertical: true)
            .help(semEla.map { String(localized: "\(serve) Sem ela: \($0)") } ?? serve)
            if !explicar {
                Spacer(minLength: 12)
                botao.fixedSize()
            }
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .contain)
    }
}

/// Acesso Total ao Disco: estado neste app e na última medição, e o botão do Ajustes.
/// Em Permissões e nas boas-vindas.
struct LinhaAcessoTotal: View {
    @Environment(Painel.self) private var painel
    var explicar = false

    var body: some View {
        let app = painel.rapido?.fullDiskAccess
        let neste: String = switch app {
        case true?: String(localized: "Neste app: ativo")
        case false?: String(localized: "Neste app: desativado")
        case nil: String(localized: "Neste app: indeterminado")
        }
        var estados = [neste]
        if let c = painel.completo {
            let modo = ModoMedicao.nome(painel.modoCompleto, root: c.root)
            let ultima: String = switch c.fullDiskAccess {
            case true?: String(localized: "Na última medição completa (\(modo)): ativo")
            case false?: String(localized: "Na última medição completa (\(modo)): desativado")
            case nil: String(localized: "Na última medição completa (\(modo)): indeterminado")
            }
            estados.append(ultima)
        }
        return VStack(alignment: .leading, spacing: 8) {
            LinhaPermissao(
                nome: String(localized: "Acesso Total ao Disco"), ok: app, estados: estados,
                serve: String(localized: "Deixa o SDTally ler pastas que o macOS esconde até do administrador, como Mail, Mensagens e Safari."),
                semEla: String(localized: "o SDTally não mede nada e mostra só os passos para liberar."),
                explicar: explicar
            ) {
                Button("Abrir Privacidade e Segurança") { painel.abrirAcessoTotal() }
            }
            if PermissoesView.acessoDivergente(app: app, medicao: painel.completo?.fullDiskAccess) {
                Faixa(simbolo: "exclamationmark.triangle.fill", texto: String(localized: "A última medição completa rodou sem o Root sem Senha em Acesso Total ao Disco"))
            }
        }
    }
}

/// Medidor root (SMAppService): estado e o botão de instalar, aprovar ou remover.
struct LinhaMedidorRoot: View {
    @Environment(Painel.self) private var painel
    var explicar = false

    var body: some View {
        let (estado, ok): (String, Bool?) =
            if Dev.ativo { (String(localized: "Fora do .app (desenvolvimento)"), nil) }
            else if painel.helper == .enabled { (String(localized: "Ativo"), true) }
            else if painel.helper == .requiresApproval { (String(localized: "Aguardando aprovação em Itens de Início"), false) }
            else { (String(localized: "Não instalado"), nil) }
        return LinhaPermissao(
            nome: String(localized: "Root sem Senha"), ok: ok, estados: [estado] + (painel.helperErro.map { [$0] } ?? []),
            serve: String(localized: "Mede como administrador sem pedir a senha a cada vez. É um serviço do próprio SDTally, aprovado uma vez em Itens de Início."),
            semEla: String(localized: "a medição completa pede a senha de administrador e não dá para parar no meio."),
            explicar: explicar
        ) {
            Group {
                switch painel.helper {
                case .enabled: Button("Remover") { painel.removerHelper() }
                case .requiresApproval: Button("Abrir Itens de Início") { painel.abrirItensDeInicio() }
                default: Button("Instalar…") { painel.perguntarInstalar = true }
                }
            }
            .disabled(painel.medindo || Dev.ativo)
        }
    }
}

/// Configurações › Versões: a opção e, com ela desligada, "Verificar atualizações" à mão (a página de
/// versões no navegador) ou pela internet (uma busca agora). Embaixo, o estado e Instalar (dono, 26/10).
struct LinhaAtualizador: View {
    var body: some View {
        let a = Atualizacoes.unica
        ToggleAtualizacoes()
        if !a.ligado {
            LabeledContent("Verificar atualizações") {
                HStack {
                    Button("Manualmente") { NSWorkspace.shared.open(urlVersoes) }
                        .help(String(localized: "Abre a página de versões no GitHub, no navegador."))
                    Button("Pela internet") { a.procurarAgora() }
                        .disabled(atualizacaoOcupada(a))
                        .help(String(localized: "Procura uma versão nova no github.com agora."))
                }
            }
        }
        if let t = textoAtualizacao(a) {
            HStack {
                Text(t).font(.callout).foregroundStyle(a.estado.ehErro ? AnyShapeStyle(Marca.aviso) : AnyShapeStyle(.secondary))
                    .textSelection(.enabled)
                Spacer()
                if a.disponivel != nil && !atualizacaoOcupada(a) { Button("Instalar") { a.instalar() } }
            }
        }
    }
}

/// Procurar e baixar versões novas no github.com: o único uso de rede do app (dono, 26/10).
struct ToggleAtualizacoes: View {
    var explicar = false
    static let serve = String(localized: "O SDTally só se conecta à internet para isso: procurar uma versão nova no github.com e baixá-la quando você clicar em Instalar. Nada sobre o seu disco é enviado.")

    var body: some View {
        @Bindable var a = Atualizacoes.unica
        Toggle(isOn: $a.ligado) {
            if explicar {
                Rotulo(String(localized: "Procurar atualizações automaticamente"), Self.serve, explicar, simbolo: "arrow.down.circle")
            } else {
                // Em Configurações, a frase explícita de rede fica embaixo (dono, 26/10: dizer no app).
                Text("Procurar atualizações automaticamente")
                Text("O app só se conecta à internet para procurar e baixar versões novas no github.com. O CLI e o helper root nunca acessam a internet; vêm dentro do app e são atualizados junto com ele.")
            }
        }
        .help(Self.serve)
    }
}

/// Ícone na barra de menus: o app segue em segundo plano com a janela fechada.
struct ToggleBarraDeMenus: View {
    @AppStorage("barraDeMenus") private var barraDeMenus = true
    var explicar = false
    static let serve = String(localized: "Fica em segundo plano com a janela fechada: mostra o espaço livre e avisa quando o disco cresce muito.")

    var body: some View {
        Toggle(isOn: $barraDeMenus) { Rotulo(String(localized: "Ícone na barra de menus"), Self.serve, explicar, simbolo: "menubar.rectangle") }
            .help(Self.serve)
    }
}

/// O que o ícone da barra de menus mostra (RotuloBarra lê a mesma chave).
struct PickerRotuloBarra: View {
    @AppStorage("barraDeMenus") private var barraDeMenus = true
    @AppStorage("rotuloBarra") private var mostrar = "livre"

    var body: some View {
        Picker("Na barra de menus, mostrar", selection: $mostrar) {
            Text("Espaço livre").tag("livre")
            Text("Liberado automaticamente").tag("liberado")
            Text("Só o ícone").tag("icone")
        }
        .disabled(!barraDeMenus)
    }
}

/// Notificação: pede a autorização do macOS só ao ligar, e nunca fora do .app.
struct ToggleNotificar: View {
    @AppStorage("notificar") private var notificar = false
    var explicar = false
    static let serve = String(localized: "Notifica quando o disco cresce 10 GB em um dia ou fica com menos de 10 % livre. Sem ela, o aviso aparece só no ícone.")

    var body: some View {
        Toggle(isOn: $notificar) { Rotulo(String(localized: "Avisar com notificação"), Self.serve, explicar, simbolo: "bell") }
            .help(Self.serve)
            .onChange(of: notificar) { _, ligou in
                guard ligou, !Dev.ativo else { return }
                Task {
                    let ok = (try? await UNUserNotificationCenter.current().requestAuthorization(options: [.alert])) ?? false
                    if !ok { notificar = false }
                }
            }
    }
}

/// Abrir ao iniciar sessão (SMAppService.mainApp): só existe no .app; fora dele fica desligado.
struct ToggleIniciarSessao: View {
    var explicar = false
    @State private var iniciarSessao = !Dev.ativo && SMAppService.mainApp.status == .enabled
    @State private var erroSessao: String?
    static let serve = String(localized: "Deixa o ícone da barra de menus pronto depois de ligar o Mac, sem abrir a janela. Aparece em Itens de Início.")

    var body: some View {
        Toggle(isOn: Binding(get: { iniciarSessao }, set: abrirAoIniciar)) {
            Rotulo(String(localized: "Abrir ao iniciar sessão"), Dev.ativo ? String(localized: "Fora do .app (desenvolvimento).") : Self.serve, explicar, simbolo: "power")
            if let erroSessao { Text(erroSessao).foregroundStyle(Marca.aviso) }
        }
        .help(Dev.ativo ? String(localized: "Fora do .app (desenvolvimento).") : Self.serve)
        .disabled(Dev.ativo)
    }

    private func abrirAoIniciar(_ ligar: Bool) {
        guard !Dev.ativo else { return }
        erroSessao = nil
        do {
            if ligar { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
        } catch {
            erroSessao = error.localizedDescription
        }
        iniciarSessao = SMAppService.mainApp.status == .enabled
    }
}

/// Nome do toggle. Nas boas-vindas, no desenho da LinhaPermissao: símbolo, nome, o porquê
/// embaixo e o interruptor encostado à direita.
private struct Rotulo: View {
    let nome, serve, simbolo: String
    let explicar: Bool
    init(_ nome: String, _ serve: String, _ explicar: Bool, simbolo: String) {
        (self.nome, self.serve, self.explicar, self.simbolo) = (nome, serve, explicar, simbolo)
    }

    var body: some View {
        if explicar {
            HStack(alignment: .top, spacing: 12) {
                Image(systemName: simbolo).font(.title2).foregroundStyle(.secondary).frame(width: 28).accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 4) {
                    Text(nome).font(.headline)
                    Text(serve).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        } else {
            Text(nome)
        }
    }
}

/// O Automático com ou sem root (painel.autoComRoot; mudar reabre o automático sozinho). Com root
/// só com o Root sem Senha ativo: sem ele a opção fica desativada, com o aviso. Ligar pede antes o
/// "Você tem certeza?" e depois o Touch ID; desligar pede só o Touch ID. Medir não pede nada.
private struct PickerAutoRoot: View {
    @Environment(Painel.self) private var painel
    @State private var perguntar = false

    var body: some View {
        let ativo = painel.helper == .enabled
        // Sem o Root sem Senha, "Com root" não vale (o automático roda sem root): não aparece
        // marcado nem dá para escolher, mesmo se o radio deixar clicar.
        let escolha = Binding(get: { painel.autoComRoot && ativo }, set: { novo in
            guard novo != (painel.autoComRoot && ativo), !novo || ativo else { return }
            if novo { perguntar = true } else { trocar(false) }
        })
        VStack(alignment: .leading, spacing: 6) {
            Picker("Automático", selection: escolha) {
                Text("Sem root").tag(false)
                Text("Com root").tag(true).disabled(!ativo)
            }
            .pickerStyle(.radioGroup)
            .help("Com root, o automático entra nas pastas do sistema e de outros usuários, pelo Root sem Senha, sem pedir senha. A passada leva mais (~40 s contra ~30 s).")
            if !ativo {
                Text("Instale o Root sem Senha, no topo desta tela, para usar o Automático com root.")
                    .font(.caption).foregroundStyle(.secondary)
            } else if let r = painel.autoVivo?.root {
                // O estado vem da linha mostrada, não da escolha: a troca leva uma passada.
                Text(r ? String(localized: "A leitura na tela é com root.") : String(localized: "A leitura na tela é sem root."))
                    .font(.caption).foregroundStyle(.secondary)
            }
        }
        .alert("Você tem certeza?", isPresented: $perguntar) {
            Button("Ativar com root") {
                trocar(true)
            }
            Button("Cancelar", role: .cancel) {}
        } message: {
            Text(Self.aviso)
        }
    }

    /// A troca, nos dois sentidos, só depois do Touch ID (ou da senha da conta).
    private func trocar(_ comRoot: Bool) {
        Task {
            guard await confirmarDono(comRoot ? String(localized: "ativar o Automático com root") : String(localized: "voltar ao Automático sem root")) else { return }
            if comRoot { painel.manual = false }
            painel.autoComRoot = comRoot
        }
    }

    static let aviso = String(localized: """
    O que faz: o Automático passa a ler o disco inteiro como root, pelo Root sem Senha, o tempo todo e sem pedir senha. Entra nas pastas do sistema e de outros usuários do Mac, e a conta fica igual à da Medição completa. O alternador Automático | Manual some: o Automático já mede tudo.

    Riscos: um processo com root fica rodando enquanto o app estiver aberto. O SDTally só lê tamanhos, nunca o conteúdo, e não apaga nada; ainda assim, um erro num processo root tem mais alcance. Quem usar esta sessão vê o tamanho das pastas de outros usuários. Cada passada gasta mais CPU e bateria (~40 s contra ~30 s).

    Recomendações: use só no seu Mac, com uma conta só sua. Na bateria, prefira o Automático sem root. Para conferir de vez em quando, a Medição completa basta. Dá para voltar a qualquer hora em Configurações > Avançado.
    """)
}

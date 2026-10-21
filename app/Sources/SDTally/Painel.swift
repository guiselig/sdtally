// Estado do app, instância única (Painel.unico). Rápido (sem senha, ~0,2 s e ~0,12 s de CPU
// quando o painel do Ajustes está fechado) ao abrir, ao voltar ao app, ao acordar, a cada 30 s com
// o app ativo e a cada 30 min em segundo plano; completo (minutos, com ou sem senha) só sob demanda. Nunca dois do mesmo tipo ao mesmo tempo. O app não calcula
// números: só guarda o que o sdtally devolveu.
import AppKit
import CryptoKit
import DiskArbitration
import SDTallyXPC
import ServiceManagement
import SwiftUI
import UserNotifications

/// Uma linha do `sdtally --watch`: só a tela e o macOS.
nonisolated struct LinhaDaEscuta: Decodable {
    let settingsScreen: Ajustes
}

/// Partes da leitura rápida (`sdtally --quick --parts`), cada uma com o seu ritmo (`Painel.intervalo`).
enum ParteRapido: String, CaseIterable, Sendable {
    case meta, volumes, swap, uso, purgeable, tela
}

enum Medicao {
    case parada
    /// `cancelavel`: sem senha e pelo helper dá para parar; pelo osascript, não.
    case medindo(desde: Date, admin: Bool, cancelavel: Bool)
    case senhaCancelada
    case falhou(String)
}

/// Última tela do Ajustes medida: o log só tem a linha com o painel aberto, e a janela de 1 h
/// do --quick a perde depois. Fica à mostra com a hora dela, nunca como "agora".
struct TelaLida {
    let tela: Tela
    let macos: Medida
}

@Observable
final class Painel {
    /// A do app: o Delegado inicia o laço, a janela e a barra de menus leem a mesma.
    static let unico = Painel()

    var rapido: Rapido?
    var telaBoa: TelaLida?
    /// Livre do volume de dados pelo statfs do próprio app, a cada 1 s (`vigiarTempoReal`): ~1 µs por
    /// leitura, contra ~0,12 s de CPU do rápido. É o mesmo `f_bavail × f_bsize` do sdtally.
    var livre: Int64?
    /// Uso do volume Data (getattrlist) e swap (sysctl) do laço de 1 s. Propriedades próprias, fora
    /// do `rapido`: quando um número muda, só redesenha quem lê esse número (`usoVivo`, `swapVivo`,
    /// `purgVivo`, `rapidoVivo`), não toda tela que lê o `rapido`.
    var usoAgora: Int64?
    var swapAgora: Swap?
    /// Purgeable lido no app (CacheDelete por serviço); nil depois de uma leitura do sdtally, que é
    /// tão nova quanto ele.
    var purgAgora: Purgavel?
    /// Usado de cada volume APFS montado (dispositivo → bytes), do laço de 1 s: troca o
    /// `capacity_in_use`, o livre do container e o macOS da tela da leitura rápida (`aplicar`).
    var volumesAgora: [String: Int64]?
    /// Livre para downloads opcionais (NSURL Opportunistic), lido junto do purgeable (`lerPurgeavel`).
    var opcionalAgora: Int64?
    @ObservationIgnored private var sessaoDA: DASession?
    @ObservationIgnored private var discosDesde = Date.distantFuture
    @ObservationIgnored private var tarefaDisco: Task<Void, Never>?
    @ObservationIgnored var purgLidoEm: Date?
    @ObservationIgnored var lendoPurg = false
    /// Ficha de cada serviço do CacheDelete, da última leitura do sdtally (o catálogo casa pelo nome).
    @ObservationIgnored var fichaDoServico: [String: String] = [:]
    /// stdin do `sdtally --watch` (a tela chega pelo `log stream`). nil: sem escuta, e o laço lê a
    /// tela como antes, a cada `Ritmo.telaAberta` com o Ajustes aberto.
    @ObservationIgnored var escuta: FileHandle?
    /// `logged_at` da última tela vista (`notarTela`).
    @ObservationIgnored var telaNotada: String?
    var rapidoFalhou: (msg: String, em: Date)?
    /// Medição automática (`sdtally --auto`): o relatório da árvore viva, sem root nem xattr,
    /// refeito a cada mudança de 10 MB ou a cada minuto com mudança. Separado de `completo`
    /// (a medição completa); nil até a primeira passada (~40 s) terminar.
    var autoVivo: Completo?
    var autoEstado: AutoEstado?
    /// `autoVivo` é a última linha da execução anterior (lida do disco ao abrir), não a de agora:
    /// fica true até a primeira linha do automático novo.
    var autoDoDisco = false
    /// Amostras por hora das pastas do automático (Historico.PontoPastas), de até 90 dias.
    var pontosPastas: [PontoPastas] = []
    /// Quando a última linha do automático foi gravada (no máximo uma a cada 5 min).
    @ObservationIgnored private var autoGravadoEm: Date?
    /// Bytes crus da linha que gerou `autoVivo`.
    @ObservationIgnored var autoDados: Data?
    /// Fecha o automático aberto (stdin do `sdtally --auto` local ou a conexão com o helper);
    /// nil sem automático.
    @ObservationIgnored var pararAutoAtual: (() -> Void)?
    /// Cada abertura do automático ganha um número: aviso de saída de uma abertura velha não
    /// derruba a nova.
    @ObservationIgnored private var autoGeracao = 0
    /// "Automático com root" (boas-vindas e menu). Só vale com o Root sem Senha aprovado
    /// (`helper == .enabled`); sem ele, ou se o helper falhar, o automático roda sem root.
    /// `autoVivo?.root` diz como a linha mostrada foi medida.
    /// Os botões de instalar o Root sem Senha pedem antes o aviso (AvisoInstalarRoot).
    var perguntarInstalar = false
    var autoComRoot = UserDefaults.standard.bool(forKey: "autoComRoot") {
        didSet {
            guard autoComRoot != oldValue else { return }
            UserDefaults.standard.set(autoComRoot, forKey: "autoComRoot")
            reiniciarAuto()
        }
    }
    var completo: Completo?
    /// O alternador Automático | Manual do cabeçalho, um só para o app: false mostra `autoVivo`,
    /// true a Medição completa (`completo`). Uma medição com senha que termina liga o Manual.
    var manual = false
    /// O "Medir com senha" do Automático liga o Manual na hora (o cronômetro fica lá); se a senha
    /// for cancelada, volta ao Automático.
    @ObservationIgnored var voltarAoAuto = false
    /// A medição que as telas com o alternador mostram.
    var dados: Completo? { manual ? completo : autoVivo }
    /// `sdtally --catalog --json`, lido uma vez por execução (selo da NumeroView); nil se falhou.
    var catalogo: Catalogo?
    /// Bytes crus do stdout que gerou `completo`: o "Exportar JSON" grava isto sem transformar.
    var completoDados: Data?
    /// Como `completo` rodou (helper, senha ou sem root); nil quando veio de arquivo sem o xattr.
    var modoCompleto: ModoMedicao?
    /// Histórico (Historico.swift): medições salvas, da mais antiga para a mais nova, e pontos.
    var medicoes: [Salva] = []
    var pontos: [Ponto] = []
    /// Folha de boas-vindas (abre na primeira vez e pelo menu).
    var mostrarBoasVindas = false
    var medicao = Medicao.parada
    var secao: Secao? = Foto.secaoInicial
    var pilha: [String] = Foto.pastaInicial
    /// Barra que se desdobra em Armazenamento (BarraDesdobravel): vazia = nível `ajustes`.
    var pilhaBarra: [NivelBarra] = pilhaInicial(Foto.barra)
    /// Âncora para rolar ao abrir Dados do Sistema ou O que fazer (links do Início); limpa depois de rolar.
    var rolarPara: String?
    /// Até quando consultar a cada 5 s esperando o Ajustes gravar a tela (depois de "Abrir Armazenamento").
    var aguardarAjustesAte: Date?
    /// Quando o app abriu o Ajustes oculto (lerAjustes); nil sem ele aberto pelo app.
    private var abriuAjustesEm: Date?
    /// Dados do Sistema da tela anterior desde a abertura oculta: fecha quando dois seguidos batem.
    private var ultimoSD: Int64?
    /// Última tentativa de lerAjustes: sem tela nova, espera `Ritmo.ajustesEscondido` antes de tentar de novo.
    private var tentouAjustesEm: Date?
    /// Helper root (LaunchDaemon via SMAppService): aprovado uma vez, mede como root sem senha.
    var helper: SMAppService.Status = .notRegistered
    var helperErro: String?
    private let servicoHelper = SMAppService.daemon(plistName: HelperXPC.plist)
    private var reregistrando = false
    private var ramos: [String: Ramo] = [:]
    private var lendo = false
    private var tarefaMedicao: Task<Void, Never>?
    /// O laço de leitura: um só, criado em iniciar() e nunca cancelado ao fechar a janela.
    private(set) var tarefaVigia: Task<Void, Never>?
    private var lidoEm: Date?
    /// Quando cada parte do rápido foi lida pela última vez.
    private var parteLidaEm: [ParteRapido: Date] = [:]
    /// O último JSON do rápido já juntado: as partes não relidas ficam com o valor anterior.
    @ObservationIgnored var jsonRapido: Data?
    /// Quando começou o último rápido que deu certo (o `--since` do próximo).
    private var inicioDaUltimaLeitura: Date?
    /// Abre a janela principal de fora do SwiftUI (clique na notificação); vem do rótulo da barra.
    @ObservationIgnored var abrirJanela: OpenWindowAction?
    /// Só no .app; o binário cru nunca grava histórico.
    private let pastaGravacao = Historico.pastaGravacao(bundleID: Bundle.main.bundleIdentifier)
    /// Pontos só são anotados depois de lidos do disco: antes, reescrever o arquivo os perderia.
    private var historicoLido = false

    static let urlArmazenamento = URL(string: "x-apple.systempreferences:com.apple.settings.Storage")!
    nonisolated static let idAjustes = "com.apple.systempreferences"
    /// Idade da tela a partir da qual o laço relê o Ajustes oculto: cada leitura custa ~27 s de CPU.
    /// Limite do Ajustes oculto aberto: a tela estabiliza em ~30 s.
    nonisolated static let ajustesAbertoAte: TimeInterval = 60
    static let urlAcessoTotal = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")!
    static let prompt = String(localized: "O SDTally vai ler o disco como administrador para medir as pastas do sistema. Nada é apagado nem alterado.")

    var aguardandoAjustes: Bool { (aguardarAjustesAte ?? .distantPast) > .now }

    /// Idempotente: chamado no applicationDidFinishLaunching; a segunda chamada não cria outro laço.
    func iniciar() {
        iniciar { [self] in
            Self.fecharAjustesOrfao()
            atualizarHelper()
            await carregarCatalogo()
            await carregarHistorico()
            // Com leitura fixa (foto), o livre fica o dela.
            if Foto.rapidoFixo == nil { Task { await vigiarTempoReal() } }
            escutarTela()
            escutarAuto()
            if Foto.rapidoFixo == nil { vigiarDiscos() }
            await vigiar()
        }
    }

    /// Livre, uso e swap a cada 1 s, com o app ativo ou em segundo plano (ícone da barra de menus),
    /// lidos no próprio app: statfs, getattrlist e sysctl somam ~2 µs. O custo medido é o redesenho
    /// (0,76 % de CPU publicando todo valor, 0,30 % só quando o número mostrado muda), por isso só
    /// publica quando muda o centésimo de GB.
    func vigiarTempoReal() async {
        while !Task.isCancelled {
            if Ritmo.tempoReal.valor > 0 {
                if let l = Self.livreDoDisco(), Self.mudou(l, livre) { livre = l }
                if let u = Self.usoDoVolume(), Self.mudou(u, usoAgora) { usoAgora = u }
                if let w = Self.swapAgora(), Self.mudou(w.used, swapAgora?.used) { swapAgora = w }
                if let v = Self.volumesMontados(), Self.mudouVolumes(v, volumesAgora) { volumesAgora = v }
            } else if livre != nil || usoAgora != nil || swapAgora != nil || volumesAgora != nil {
                // Desligado: as telas voltam ao valor da leitura rápida, sem número parado do laço.
                (livre, usoAgora, swapAgora, volumesAgora) = (nil, nil, nil, nil)
            }
            lerPurgeavel()
            // Desligado: só confere de novo a cada 5 s se voltou a ligar.
            let s = Ritmo.tempoReal.valor
            try? await Task.sleep(for: .seconds(s > 0 ? s : 5), tolerance: .milliseconds(500))
        }
    }

    var usoVivo: Medida? {
        usoAgora.map { Medida(bytes: $0, state: "measured", reason: nil, source: "attr_vol_spaceused") } ?? rapido?.dataVolume.used
    }
    var swapVivo: Swap? { swapAgora ?? rapido?.swap }
    var purgVivo: Purgavel? { purgAgora ?? rapido?.purgeable }

    /// A leitura rápida com uso, swap e purgeable do app: para quem mostra esses números.
    var rapidoVivo: Rapido? {
        guard var r = rapido else { return nil }
        Self.aplicar(&r, uso: usoAgora, swap: swapAgora, purg: purgAgora, volumes: volumesAgora)
        return r
    }

    nonisolated static func aplicar(_ r: inout Rapido, uso: Int64?, swap: Swap?, purg: Purgavel?, volumes: [String: Int64]? = nil) {
        if let u = uso { r.dataVolume.used = Medida(bytes: u, state: "measured", reason: nil, source: "attr_vol_spaceused") }
        if let w = swap { r.swap = w }
        if let p = purg { r.purgeable = p }
        guard let vs = volumes, var cs = r.containers else { return }
        // Pela diferença: o livre do container guarda a sobra que o diskutil vê (metadados) e os
        // volumes desmontados (Recovery) ficam com o valor do diskutil.
        var dMacos: Int64 = 0
        for i in cs.indices {
            for j in cs[i].volumes.indices {
                guard let d = cs[i].volumes[j].device, let novo = vs[d], let velho = cs[i].volumes[j].capacityInUse else { continue }
                cs[i].volumes[j].capacityInUse = novo
                cs[i].free = cs[i].free.map { $0 - (novo - velho) }
                if cs[i].volumes[j].inSettingsMacos { dMacos += novo - velho }
            }
        }
        r.containers = cs
        let m = r.settingsScreen.macos
        if dMacos != 0, let b = m.bytes { r.settingsScreen.macos = Medida(bytes: b + dMacos, state: m.state, reason: m.reason, source: m.source) }
    }

    /// Usado de cada volume APFS montado, pelo getattrlist: o mesmo número do `diskutil`, byte a
    /// byte (19/10), ~0,15 ms para os 8 deste Mac. O "/" é o snapshot selado do Sistema
    /// (disk3s1s1) e conta como o volume dele (disk3s1); snapshots do Time Machine não são `/dev/`.
    nonisolated static func volumesMontados() -> [String: Int64]? {
        var m: UnsafeMutablePointer<statfs>?
        let n = getmntinfo(&m, MNT_NOWAIT)
        guard n > 0, let m else { return nil }
        func texto<T>(_ t: inout T) -> String { withUnsafeBytes(of: &t) { String(decoding: $0.prefix { $0 != 0 }, as: UTF8.self) } }
        var r: [String: Int64] = [:]
        for i in 0..<Int(n) {
            var s = m[i]
            guard texto(&s.f_fstypename) == "apfs" else { continue }
            let de = texto(&s.f_mntfromname).dropFirst("/dev/".count)
            guard texto(&s.f_mntfromname).hasPrefix("/dev/"), let x = de.range(of: #"^disk\d+s\d+"#, options: .regularExpression),
                  r[String(de[x])] == nil, let u = usoDoVolume(texto(&s.f_mntonname)) else { continue }
            r[String(de[x])] = u
        }
        return r
    }

    /// Volume novo ou que sumiu, ou algum andou 10 MB.
    nonisolated static func mudouVolumes(_ novo: [String: Int64], _ velho: [String: Int64]?) -> Bool {
        guard let velho, Set(novo.keys) == Set(velho.keys) else { return true }
        return novo.contains { mudou($0.value, velho[$0.key]) }
    }

    /// Disco ou volume montado, desmontado ou novo: o diskutil relê os containers (o usado dos
    /// montados já vem do laço de 1 s). Ao registrar, o DA avisa uma vez por disco que já existe:
    /// esses 5 s iniciais ficam de fora (a primeira leitura já tem tudo).
    private func vigiarDiscos() {
        guard sessaoDA == nil, let s = DASessionCreate(kCFAllocatorDefault) else { return }
        DARegisterDiskAppearedCallback(s, nil, { _, _ in Task { @MainActor in Painel.unico.discoMudou() } }, nil)
        DARegisterDiskDisappearedCallback(s, nil, { _, _ in Task { @MainActor in Painel.unico.discoMudou() } }, nil)
        DARegisterDiskDescriptionChangedCallback(s, nil, [kDADiskDescriptionVolumePathKey] as CFArray,
                                                 { _, _, _ in Task { @MainActor in Painel.unico.discoMudou() } }, nil)
        DASessionSetDispatchQueue(s, .main)
        sessaoDA = s
        discosDesde = .now.addingTimeInterval(5)
    }

    /// Junta uma rajada de avisos (um disco externo traz vários volumes) numa leitura só.
    func discoMudou() {
        guard Date.now > discosDesde else { return }
        tarefaDisco?.cancel()
        tarefaDisco = Task { [weak self] in
            try? await Task.sleep(for: .seconds(3))
            guard !Task.isCancelled else { return }
            await self?.atualizar([.volumes])
        }
    }

    /// Purgeable no próprio app: 10 s ativo, 1 min em segundo plano, fora do Pouca Energia. A consulta
    /// (~4 ms de CPU, XPC ao deleted) roda fora da main e nunca duas ao mesmo tempo: se o serviço
    /// travar, a próxima só sai quando ela voltar.
    private func lerPurgeavel() {
        let intervalo = (NSApp.isActive ? Ritmo.purgAtivo : Ritmo.purgFundo).valor
        guard intervalo > 0, !lendoPurg, !Self.poupar(), Date.now.timeIntervalSince(purgLidoEm ?? .distantPast) >= intervalo else { return }
        lendoPurg = true
        let fichas = fichaDoServico
        Task {
            let (p, o) = await Task.detached(priority: .utility) { (Self.purgeavelAgora(fichas: fichas), Self.lerOpcional()) }.value
            lendoPurg = false
            purgLidoEm = .now
            // Como os outros números: só publica quando muda o centésimo de GB.
            if let p, Self.mudou(p.bytes, purgAgora?.bytes) { purgAgora = p }
            if let o, Self.mudou(o, opcionalAgora) { opcionalAgora = o }
        }
    }

    /// NSURL Opportunistic do volume Data: 10–34 ms esperando o `deleted`, ~5–8 ms de CPU (19/10).
    /// URL nova a cada vez: o resourceValues guarda cache por URL e devolveria o valor velho.
    nonisolated static func lerOpcional() -> Int64? {
        (try? URL(fileURLWithPath: "/System/Volumes/Data").resourceValues(forKeys: [.volumeAvailableCapacityForOpportunisticUsageKey]))?
            .volumeAvailableCapacityForOpportunisticUsage
    }

    /// As três linhas do cartão Livre, ao vivo: o livre do laço de 1 s; Livre + Purgeable = livre +
    /// purgeable do app (o Important da Apple é essa soma, byte a byte, medido em 19/10); o
    /// opcional junto do purgeable. nil até o laço ler o livre.
    var livresVivos: ValoresVolume? {
        guard let l = livre else { return nil }
        return ValoresVolume(livre: l, importante: purgVivo?.bytes.map { l + $0 }, opcional: opcionalAgora, em: .now)
    }

    private nonisolated(unsafe) static let itemizado: (@convention(c) (CFDictionary) -> Unmanaged<CFDictionary>?)? = {
        // Pelo install-name, como o sdtally (src/sys/cachedelete.rs). Só consulta, nunca purge.
        guard let h = dlopen("/System/Library/PrivateFrameworks/CacheDelete.framework/Versions/A/CacheDelete", RTLD_LAZY),
              let f = dlsym(h, "CacheDeleteCopyItemizedPurgeableSpaceWithInfo") else { return nil }
        return unsafeBitCast(f, to: (@convention(c) (CFDictionary) -> Unmanaged<CFDictionary>?).self)
    }()

    /// CacheDeleteCopyItemizedPurgeableSpaceWithInfo no volume Data, urgência 3, igual ao sdtally:
    /// total = soma dos serviços; chaves CACHE_DELETE_* são totais do próprio serviço e ficam fora.
    nonisolated static func purgeavelAgora(fichas: [String: String]) -> Purgavel? {
        guard let f = itemizado else { return nil }
        let info = ["CACHE_DELETE_VOLUME": "/System/Volumes/Data", "CACHE_DELETE_URGENCY_LIMIT": 3] as CFDictionary
        guard let d = f(info)?.takeRetainedValue() as? [String: Any] else { return nil }
        return purgavel(d, fichas: fichas)
    }

    nonisolated static func purgavel(_ d: [String: Any], fichas: [String: String]) -> Purgavel? {
        let servicos = d.filter { !$0.key.hasPrefix("CACHE_DELETE_") }.compactMap { k, v in (v as? NSNumber).map { (k, $0.int64Value) } }
        // Sem nenhum serviço não é "0 purgeable" (mesma regra do sdtally).
        guard !servicos.isEmpty else { return nil }
        let top = servicos.filter { $0.1 > 0 }.sorted { ($0.1, $1.0) > ($1.1, $0.0) }
            .map { ServicoPurgavel(name: $0.0, bytes: $0.1, card: fichas[$0.0]) }
        return Purgavel(bytes: servicos.reduce(0) { $0 + $1.1 }, state: "measured", reason: nil, source: "cachedelete", topServices: top)
    }

    /// Mudou o que a tela mostra? Compara em centésimos de GB (10 MB).
    /// shortcut: abaixo de 1 GB o gb() mostra décimos de MB e a variação menor que 10 MB fica para a
    /// próxima mudança; trocar por comparar o texto formatado se isso aparecer.
    nonisolated static func mudou(_ novo: Int64?, _ velho: Int64?) -> Bool {
        guard let n = novo else { return false }
        guard let v = velho else { return true }
        return n / 10_000_000 != v / 10_000_000
    }

    nonisolated static func usoDoVolume(_ caminho: String = "/System/Volumes/Data") -> Int64? {
        var al = attrlist(bitmapcount: u_short(ATTR_BIT_MAP_COUNT), reserved: 0, commonattr: 0,
                          volattr: attrgroup_t(ATTR_VOL_INFO) | attrgroup_t(ATTR_VOL_SPACEUSED), dirattr: 0, fileattr: 0, forkattr: 0)
        var buf = [UInt8](repeating: 0, count: 16)
        // Resposta: u_int32 com o tamanho e o off_t do ATTR_VOL_SPACEUSED.
        let r = buf.withUnsafeMutableBytes { getattrlist(caminho, &al, $0.baseAddress, 16, 0) }
        guard r == 0 else { return nil }
        return buf.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 4, as: Int64.self) }
    }

    nonisolated static func swapAgora() -> Swap? {
        var x = xsw_usage()
        var n = MemoryLayout<xsw_usage>.size
        guard sysctlbyname("vm.swapusage", &x, &n, nil, 0) == 0 else { return nil }
        return Swap(total: Int64(x.xsu_total), used: Int64(x.xsu_used))
    }

    nonisolated static func livreDoDisco(_ caminho: String = "/System/Volumes/Data") -> Int64? {
        var st = statfs()
        guard statfs(caminho, &st) == 0 else { return nil }
        return Int64(st.f_bavail) * Int64(st.f_bsize)
    }

    /// Separado para o teste (vigiarUmaVez) não ler nem podar a pasta real.
    func iniciar(_ laco: @escaping @MainActor () async -> Void) {
        guard tarefaVigia == nil else { return }
        tarefaVigia = Task { await laco() }
    }

    /// Ritmo de cada parte. Tela: 5 s esperando o Ajustes, só sem a escuta (`sdtally --watch`).
    /// Volumes: o usado dos montados vem do laço de 1 s; o diskutil só quando um disco ou volume
    /// aparece, some ou monta (`vigiarDiscos`), e na leitura inteira. Uso e swap: no laço de 1 s do
    /// app (`vigiarTempoReal`); o rápido só na leitura inteira.
    /// Purgeable: no app (`lerPurgeavel`, ~4 ms contra ~20 ms do sdtally), 10 s ativo e 1 min em
    /// segundo plano; o rápido só na leitura inteira. Acesso Total: só ao abrir e ao voltar ao app
    /// (`atualizar()` lê tudo menos a tela).
    nonisolated static func intervalo(_ p: ParteRapido, aguardando: Bool, ativo: Bool) -> TimeInterval {
        switch p {
        case .tela: aguardando ? Ritmo.telaAberta.valor : .infinity
        case .volumes, .uso, .swap, .purgeable, .meta: .infinity
        }
    }

    /// As partes cujo intervalo passou desde a última leitura (nunca lida = vencida).
    nonisolated static func vencidas(_ lidas: [ParteRapido: Date], aguardando: Bool, ativo: Bool, agora: Date = .now) -> Set<ParteRapido> {
        Set(ParteRapido.allCases.filter { agora.timeIntervalSince(lidas[$0] ?? .distantPast) >= intervalo($0, aguardando: aguardando, ativo: ativo) })
    }

    /// Junta o JSON parcial (`--parts`) com o anterior: chave nova substitui a velha; dentro de
    /// `settings_screen`, uma por uma (a tela e o macOS vêm de partes diferentes).
    nonisolated static func juntar(_ velho: Data?, _ novo: Data) throws -> Data {
        guard let velho, var v = try JSONSerialization.jsonObject(with: velho) as? [String: Any],
              let n = try JSONSerialization.jsonObject(with: novo) as? [String: Any] else { return novo }
        for (k, x) in n {
            if k == "settings_screen", let a = v[k] as? [String: Any], let b = x as? [String: Any] {
                v[k] = a.merging(b) { $1 }
            } else {
                v[k] = x
            }
        }
        return try JSONSerialization.data(withJSONObject: v)
    }

    /// Até quando a leitura anterior ainda vale como ponto de partida do `--since`: acima disso o
    /// rápido lê a hora inteira do log (volta de segundo plano, de repouso), como antes.
    nonisolated static let incrementalAte: TimeInterval = 120

    /// Argumentos do rápido. `--since` (início da leitura anterior que deu certo) faz o sdtally ler o
    /// log da tela só dali em diante e só com o painel do Ajustes aberto: ~0,1 s de CPU em vez de ~1 s
    /// e 180 MB do `log show`. Sem leitura recente, ou com ela velha, lê a hora inteira.
    /// Sem `partes` (ou com todas), sem `--parts`.
    nonisolated static func argsRapido(desde: Date?, agora: Date = .now, partes: Set<ParteRapido> = Set(ParteRapido.allCases)) -> [String] {
        var a = ["--quick", "--json"]
        if let d = desde, (0..<incrementalAte).contains(agora.timeIntervalSince(d)) { a += ["--since", String(Int(d.timeIntervalSince1970))] }
        if partes.count < ParteRapido.allCases.count { a += ["--parts", ParteRapido.allCases.filter(partes.contains).map(\.rawValue).joined(separator: ",")] }
        return a
    }

    /// Pouca Energia ou calor sério/crítico: o laço pula (a primeira leitura e a espera do Ajustes, não).
    nonisolated static func poupar(_ p: ProcessInfo = .processInfo) -> Bool {
        p.isLowPowerModeEnabled || [.serious, .critical].contains(p.thermalState)
    }

    /// Acorda a cada 5 s para que "Abrir Armazenamento" passe a consultar a cada 5 s sem esperar.
    func vigiar() async {
        while !Task.isCancelled {
            // Com a escuta viva a tela chega sozinha: o laço não a lê.
            let v = Self.vencidas(parteLidaEm, aguardando: aguardandoAjustes && escuta == nil, ativo: NSApp.isActive)
            // Durante a medição completa o rápido só disputa disco e CPU; ao terminar, medir() atualiza.
            if !medindo, !v.isEmpty, rapido == nil || aguardandoAjustes || !Self.poupar() {
                await atualizar(v)
            }
            if let a = abriuAjustesEm, Date.now.timeIntervalSince(a) > Self.ajustesAbertoAte { fecharAjustes() }
            if Self.releAjustes(tela: [data(telaBoa?.tela.loggedAt), tentouAjustesEm].compactMap { $0 }.max(), ativo: NSApp.isActive, poupar: Self.poupar()) { lerAjustes() }
            try? await Task.sleep(for: .seconds(5))
        }
    }

    /// Ao abrir a janela da barra de menus: lê de novo se a última leitura passou de 60 s.
    func atualizarSeVelho() {
        guard !medindo, Date.now.timeIntervalSince(lidoEm ?? .distantPast) > 60 else { return }
        Task { await atualizar() }
    }

    /// Tudo menos a tela: com o app aberto, a tela só muda com o Ajustes aberto, e isso chega pelo
    /// aviso de abrir e fechar (`ajustesMudou`). Lida aqui, ao voltar ao app depois de 2 min, ela leria
    /// a hora inteira do log: ~1,5 s de CPU e 177 MB a cada troca de app.
    nonisolated static let semTela = Set(ParteRapido.allCases).subtracting([.tela])

    /// Sem argumento, tudo menos a tela (voltar ao app, acordar, depois de calcular). A primeira
    /// leitura é sempre inteira.
    func atualizar(_ pedidas: Set<ParteRapido> = semTela) async {
        guard !lendo else { return }
        lendo = true
        // A primeira leitura é inteira: o JSON juntado precisa de todas as chaves.
        let partes = jsonRapido == nil ? Set(ParteRapido.allCases) : pedidas
        let inicio = Date.now
        defer {
            lendo = false
            lidoEm = .now
            for p in partes { parteLidaEm[p] = inicio }
        }
        do {
            let dados: Data
            if let f = Foto.rapidoFixo {
                dados = f
            } else {
                let novo = try await Executor.rodar(Self.argsRapido(desde: partes.contains(.tela) ? inicioDaUltimaLeitura : nil, agora: inicio, partes: partes))
                // jsonRapido depois do await: a escuta pode ter juntado uma tela nova enquanto rodava.
                dados = try Self.juntar(jsonRapido, novo)
            }
            let r = try decodificar(Rapido.self, dados, kind: "quick")
            jsonRapido = dados
            if partes.contains(.purgeable) {
                // Leitura do sdtally: tão nova quanto a do app, e traz a ficha de cada serviço.
                purgAgora = nil
                for s in r.purgeable.topServices ?? [] { if let c = s.card { fichaDoServico[s.name] = c } }
            }
            let novaTela = r.settingsScreen.screen.flatMap { $0.state == "not_measured" ? nil : $0 }
            if let t = novaTela, t.loggedAt != telaBoa?.tela.loggedAt { notarTela(t) }
            withAnimation(.smooth(duration: 0.35)) {
                rapido = r
                // Mesma linha do log relida: não reatribui (toda atribuição redesenha quem lê a tela).
                if let t = novaTela, t.loggedAt != telaBoa?.tela.loggedAt { telaBoa = TelaLida(tela: t, macos: r.settingsScreen.macos) }
            }
            if rapidoFalhou != nil { rapidoFalhou = nil }
            if partes.contains(.tela) { inicioDaUltimaLeitura = inicio }
            // Uso, swap e purgeable vêm do laço do app, não do JSON juntado.
            anotar(rapidoVivo ?? r)
        } catch {
            rapidoFalhou = (error.localizedDescription, .now)
        }
    }

    /// A cada tela nova: o Ajustes que o dono abriu já mostrou o número (para de esperar); o
    /// escondido fecha quando duas telas seguidas ficam a menos de 100 MB (o primeiro valor do
    /// serviço sai antes de as extensões terminarem: 202,9 GB e depois 197,1 GB).
    private func notarTela(_ t: Tela) {
        guard t.loggedAt != telaNotada else { return }
        telaNotada = t.loggedAt
        if abriuAjustesEm == nil {
            aguardarAjustesAte = nil
        } else {
            if let a = ultimoSD, let b = t.systemData, abs(a - b) < 100_000_000 { fecharAjustes() }
            ultimoSD = t.systemData
        }
    }

    /// Abre o `sdtally --watch` (sem foto nem leitura fixa). Se ele cair, o laço volta a ler a tela
    /// e a escuta tenta de novo em 1 min.
    func escutarTela() {
        guard escuta == nil, Foto.rapidoFixo == nil, Foto.env["SDTALLY_FOTO"] == nil else { return }
        escuta = try? Executor.escutar(
            linha: { d in Task { @MainActor [weak self] in self?.aplicarEscuta(d) } },
            saiu: { Task { @MainActor [weak self] in self?.escutaSaiu() } })
    }

    private func escutaSaiu() {
        guard escuta != nil else { return }  // fechada pelo app
        escuta = nil
        Task { [weak self] in
            try? await Task.sleep(for: .seconds(60))
            self?.escutarTela()
        }
    }

    func pararEscuta() {
        let e = escuta
        escuta = nil
        try? e?.close()
        pararAuto()
    }

    func pararAuto() {
        let a = pararAutoAtual
        pararAutoAtual = nil
        autoGeracao += 1
        a?()
    }

    /// Troca de modo (ou o helper acabou de ser aprovado): fecha o automático e abre de novo.
    /// O último `autoVivo` fica na tela até a primeira linha do novo.
    func reiniciarAuto() {
        pararAuto()
        escutarAuto()
    }

    /// Abre o `sdtally --auto` (sem foto nem leitura fixa): pelo helper com `autoComRoot` e o
    /// helper aprovado, senão local, sem root. Se ele cair, tenta de novo em 1 min.
    func escutarAuto() {
        guard pararAutoAtual == nil, Foto.rapidoFixo == nil, Foto.env["SDTALLY_FOTO"] == nil else { return }
        autoGeracao += 1
        let g = autoGeracao
        let linha: @Sendable (Data) -> Void = { [weak self] d in
            // Decodifica fora da main: a linha é o relatório inteiro.
            guard let c = try? decodificar(Completo.self, d, kind: "auto") else { return }
            let e = (try? decodificar(LinhaAuto.self, d, kind: "auto"))?.auto
            Task { @MainActor in
                guard let self, self.autoGeracao == g else { return }
                self.aplicarAuto(d, c, e)
            }
        }
        let saiu: @Sendable () -> Void = { [weak self] in Task { @MainActor in self?.autoSaiu(g) } }
        guard autoComRoot, helper == .enabled, !Dev.ativo else { return abrirAutoLocal(linha: linha, saiu: saiu) }
        pararAutoAtual = {}  // vaga tomada enquanto o helper responde
        Task {
            do {
                let c = try await Executor.escutarPeloHelper(linha: linha, saiu: saiu)
                if autoGeracao == g { pararAutoAtual = { c.invalidate() } } else { c.invalidate() }
            } catch {
                Executor.log.error("automático com root falhou, segue sem root: \(error.localizedDescription, privacy: .public)")
                guard autoGeracao == g else { return }
                abrirAutoLocal(linha: linha, saiu: saiu)
            }
        }
    }

    private func abrirAutoLocal(linha: @escaping @Sendable (Data) -> Void, saiu: @escaping @Sendable () -> Void) {
        guard let h = try? Executor.escutar(["--auto"], linha: linha, saiu: saiu) else { pararAutoAtual = nil; return }
        pararAutoAtual = { try? h.close() }
    }

    private func autoSaiu(_ g: Int) {
        guard g == autoGeracao, pararAutoAtual != nil else { return }  // velho, ou fechado pelo app
        pararAutoAtual = nil
        Task { [weak self] in
            try? await Task.sleep(for: .seconds(60))
            guard self?.autoGeracao == g else { return }
            self?.escutarAuto()
        }
    }

    /// Uma linha do `sdtally --auto`: o sdtally já só emite com mudança, então toda linha publica.
    func aplicarAuto(_ linha: Data, _ c: Completo, _ e: AutoEstado?) {
        autoDados = linha
        autoVivo = c
        autoEstado = e
        autoDoDisco = false
        guard historicoLido, let pasta = pastaGravacao else { return }
        if autoGravadoEm.map({ Date.now.timeIntervalSince($0) >= 300 }) ?? true {
            autoGravadoEm = .now
            try? Historico.gravarUltimoAuto(linha, em: pasta)
        }
        if let p = Historico.proximoPontoPastas(c, depois: pontosPastas) {
            pontosPastas = Historico.recentes(pontosPastas + [p])
            try? Historico.gravarPontosPastas(pontosPastas, em: pasta)
        }
    }

    /// Uma tela do `sdtally --watch`. Fechar o Ajustes escondido vê toda tela; o app só redesenha
    /// se o número andou 10 MB (o painel repete a linha a cada ~5 s com bytes a mais ou a menos)
    /// ou se a tela mostrada passou de 1 min (a hora ao lado do número).
    func aplicarEscuta(_ linha: Data) {
        // Só a linha (poucos KB) antes de juntar e decodificar o rápido inteiro: a cada ~5 s com o
        // painel aberto, quase sempre sem mudança de 10 MB.
        guard let t = (try? decodificar(LinhaDaEscuta.self, linha, kind: "quick"))?.settingsScreen.screen,
              t.state != "not_measured" else { return }
        notarTela(t)
        let velha = telaBoa?.tela
        guard t.state != velha?.state || Self.mudou(t.used, velha?.used) || Self.mudou(t.systemData, velha?.systemData)
                || data(velha?.loggedAt).map({ Date.now.timeIntervalSince($0) > 60 }) ?? true,
              let dados = try? Self.juntar(jsonRapido, linha),
              let r = try? decodificar(Rapido.self, dados, kind: "quick") else { return }
        jsonRapido = dados
        withAnimation(.smooth(duration: 0.35)) {
            rapido = r
            telaBoa = TelaLida(tela: t, macos: r.settingsScreen.macos)
        }
        anotar(rapidoVivo ?? r)
    }

    /// Uma vez por execução: as fichas e as fontes não mudam enquanto o binário é o mesmo.
    func carregarCatalogo() async {
        guard catalogo == nil, let d = try? await Executor.rodar(["--catalog", "--json"]) else { return }
        catalogo = try? decodificar(Catalogo.self, d, kind: "catalog")
    }

    func abrirArmazenamento() {
        NSWorkspace.shared.open(Self.urlArmazenamento)
        aguardarAjustesAte = .now.addingTimeInterval(120)
    }

    /// Relê o Ajustes oculto: boas-vindas vistas (onde o aviso está), sem foto nem leitura fixa e
    /// tela ausente ou com mais de `Ritmo.ajustesEscondido` (app ativo) ou `Ritmo.ajustesFundo`
    /// (segundo plano; desligado por padrão). Desligado: nunca.
    static func releAjustes(tela: Date?, ativo: Bool, poupar: Bool, agora: Date = .now,
                                        viu: Bool = UserDefaults.standard.bool(forKey: "viuBoasVindas"),
                                        foto: Bool = Foto.env["SDTALLY_FOTO"] != nil || Foto.env["SDTALLY_RAPIDO"] != nil) -> Bool {
        let a = (ativo ? Ritmo.ajustesEscondido : Ritmo.ajustesFundo).valor
        return a > 0 && viu && !foto && !poupar && agora.timeIntervalSince(tela ?? .distantPast) > a
    }

    /// Abre o painel Armazenamento oculto e sem ativar (o `open -g -j`): o serviço calcula e grava a
    /// tela no log, o laço lê a cada 5 s e fecha. Com o Ajustes já aberto (o dono pode estar usando
    /// ou ter deixado aberto de propósito), não faz nada: nem abre, nem navega, nem fecha.
    func lerAjustes() {
        guard abriuAjustesEm == nil else { return }
        tentouAjustesEm = .now
        guard NSRunningApplication.runningApplications(withBundleIdentifier: Self.idAjustes).isEmpty else { return }
        aguardarAjustesAte = .now.addingTimeInterval(Self.ajustesAbertoAte)
        let c = NSWorkspace.OpenConfiguration()
        c.activates = false
        c.hides = true
        c.addsToRecentItems = false
        abriuAjustesEm = .now
        ultimoSD = nil
        NSWorkspace.shared.open(Self.urlArmazenamento, configuration: c) { app, _ in
            guard let pid = app?.processIdentifier else { return }
            Task { @MainActor in UserDefaults.standard.set(Int(pid), forKey: Self.chavePidAjustes) }
        }
    }

    /// pid do Ajustes que o app abriu escondido, até fechá-lo (`fecharAjustesOrfao`).
    nonisolated static let chavePidAjustes = "ajustesEscondidoPid"

    /// O app morreu (kill, crash) com o Ajustes escondido aberto: ninguém o fecharia, e o app novo
    /// não abre outro com ele aberto. Ao iniciar, fecha o que ficou, só se for o mesmo processo e
    /// ainda estiver escondido.
    static func fecharAjustesOrfao(_ d: UserDefaults = .standard) {
        let pid = d.integer(forKey: chavePidAjustes)
        d.removeObject(forKey: chavePidAjustes)
        guard pid > 0, let a = NSRunningApplication(processIdentifier: pid_t(pid)),
              a.bundleIdentifier == idAjustes, a.isHidden, !a.isActive else { return }
        a.terminate()
    }

    /// O dono abriu ou fechou o Ajustes. Abriu: lê a tela a cada 5 s por 1 min (o tempo de o painel
    /// Armazenamento calcular, se ele for lá). Fechou: lê uma vez. O Ajustes escondido já espera sozinho.
    func ajustesMudou(abriu: Bool) {
        guard abriuAjustesEm == nil else { return }
        if abriu {
            aguardarAjustesAte = .now.addingTimeInterval(Self.ajustesAbertoAte)
        } else {
            aguardarAjustesAte = nil
            if escuta == nil { Task { await atualizar([.tela]) } }
        }
    }

    /// Fecha só o Ajustes que segue oculto: se o dono o trouxe para a frente, fica aberto.
    func fecharAjustes() {
        guard abriuAjustesEm != nil else { return }
        for a in NSRunningApplication.runningApplications(withBundleIdentifier: Self.idAjustes) where a.isHidden && !a.isActive {
            a.terminate()
        }
        abriuAjustesEm = nil
        ultimoSD = nil
        aguardarAjustesAte = nil
        UserDefaults.standard.removeObject(forKey: Self.chavePidAjustes)
    }

    func abrirAcessoTotal() { NSWorkspace.shared.open(Self.urlAcessoTotal) }

    var medindo: Bool { if case .medindo = medicao { true } else { false } }
    var podeParar: Bool { if case .medindo(_, _, true) = medicao { true } else { false } }
    var helperAtivo: Bool { helper == .enabled }

    /// `semHelper`: força a senha pelo osascript mesmo com o helper ativo (item do menu).
    func medir(admin: Bool, semHelper: Bool = false) {
        guard !medindo else { return }
        let peloHelper = admin && helperAtivo && !semHelper
        let modo: ModoMedicao = peloHelper ? .helper : admin ? .senha : .semRoot
        medicao = .medindo(desde: .now, admin: admin, cancelavel: !admin || peloHelper)
        tarefaMedicao = Task {
            do {
                let dados = peloHelper
                    ? try await Executor.rodarPeloHelper()
                    : admin
                    ? try await Executor.rodarComoAdmin(["--json"], prompt: Self.prompt)
                    : try await Executor.rodar(["--json"])
                let c = try decodificar(Completo.self, dados, kind: nil)
                withAnimation(.smooth(duration: 0.5)) {
                    completo = c
                    medicao = .parada
                    if admin { manual = true }
                }
                completoDados = dados
                modoCompleto = modo
                ramos = [:]
                guardar(dados, c, modo)
            } catch ErroSdtally.senhaCancelada {
                medicao = .senhaCancelada
                if voltarAoAuto { manual = false }
            } catch {
                medicao = .falhou(error.localizedDescription)
            }
            voltarAoAuto = false
            await atualizar()
        }
    }

    /// A medição com senha de qualquer botão: vai para Dados do Sistema e liga o Manual na hora
    /// (o cronômetro fica lá); se a senha for cancelada, volta ao modo de antes.
    func medirComSenha() {
        guard !medindo else { return }
        voltarAoAuto = !manual
        secao = .sistema
        withAnimation(.smooth(duration: 0.3)) { manual = true }
        medir(admin: true)
    }

    /// Sem senha ou pelo helper: SIGINT, e o sdtally devolve o parcial marcado "interrompido".
    func parar() {
        guard podeParar else { return }
        tarefaMedicao?.cancel()
    }

    // MARK: Histórico

    /// Lê medições e pontos; sem medição na memória, restaura a mais nova (o rodapé mostra a data).
    func carregarHistorico() async {
        guard let pasta = Historico.pastaLeitura(env: Foto.env, bundleID: Bundle.main.bundleIdentifier) else { return }
        if let g = pastaGravacao { try? Historico.podar(url: g) }
        let (m, p) = await Historico.ler(pasta)
        let (ultimo, pp) = await Historico.lerAuto(pasta)
        medicoes = m
        pontos = p
        pontosPastas = pp
        historicoLido = true
        if autoVivo == nil, let d = ultimo, let c = try? decodificar(Completo.self, d, kind: "auto") {
            autoVivo = c
            autoDados = d
            autoDoDisco = true
        }
        if completo == nil, let s = m.last, let d = try? Data(contentsOf: s.url) {
            completo = s.completo
            completoDados = d
            modoCompleto = s.modo
        }
    }

    private func guardar(_ dados: Data, _ c: Completo, _ modo: ModoMedicao) {
        guard let pasta = pastaGravacao, let url = try? Historico.gravar(dados, generatedAt: c.generatedAt, modo: modo, em: pasta) else { return }
        try? Historico.podar(url: pasta)
        medicoes.append(Salva(url: url, completo: c, modo: modo))
        medicoes.removeAll { !FileManager.default.fileExists(atPath: $0.url.path) }  // as podadas
    }

    private func anotar(_ r: Rapido) {
        guard historicoLido, let pasta = pastaGravacao, let p = Historico.proximoPonto(r, depois: pontos) else { return }
        pontos = Historico.recentes(pontos + [p])
        try? Historico.gravarPontos(pontos, em: pasta)
        avisar()
    }

    // MARK: Avisos

    /// O aviso que o ícone e a janela da barra mostram: só a condição, sem histerese.
    var avisoAtivo: Aviso? { aviso(pontos: pontos, agora: .now, ultimo: [:]) }

    /// Com ponto novo: registra o aviso (um de cada tipo a cada 24 h) e notifica se o dono pediu.
    private func avisar() {
        let d = UserDefaults.standard
        var ultimo: [Aviso.Tipo: Date] = [:]
        for t in Aviso.Tipo.allCases { ultimo[t] = d.object(forKey: "ultimoAviso." + t.rawValue) as? Date }
        guard let a = aviso(pontos: pontos, agora: .now, ultimo: ultimo) else { return }
        d.set(Date.now, forKey: "ultimoAviso." + a.tipo.rawValue)
        guard d.bool(forKey: "notificar"), !Dev.ativo else { return }
        let (titulo, corpo) = a.textos()
        let c = UNMutableNotificationContent()
        c.title = titulo
        c.body = corpo
        UNUserNotificationCenter.current().add(UNNotificationRequest(identifier: a.tipo.rawValue, content: c, trigger: nil))
    }

    /// Vai para a seção e traz a janela para a frente (reabre se foi fechada).
    func mostrar(_ s: Secao) {
        secao = s
        abrirJanela?(id: "principal")
        NSApp.activate()
    }

    // MARK: Helper root

    func atualizarHelper() {
        let antes = helper
        helper = servicoHelper.status
        // Helper aprovado ou removido com "automático com root" escolhido: troca o caminho.
        if autoComRoot, antes != helper, antes == .enabled || helper == .enabled { reiniciarAuto() }
        // Executável do helper mudou (outro build): a Apple pede registrar de novo, senão o
        // launchd pode não subir a versão nova.
        // Nunca no meio de uma medição (o launchd derrubaria o helper que está medindo) nem fora do .app.
        guard helper == .enabled, !reregistrando, !medindo, !Dev.ativo, let h = Self.hashHelper(),
              h != UserDefaults.standard.string(forKey: "hashHelper") else { return }
        reregistrando = true  // .task e didBecomeActive chamam quase juntos ao abrir
        Task {
            defer { reregistrando = false }
            try? await servicoHelper.unregister()
            instalarHelper()
        }
    }

    /// Registra o LaunchDaemon. A primeira vez o macOS pede aprovação (com senha de admin) na
    /// notificação "Itens em segundo plano" ou em Ajustes > Geral > Itens de Início.
    func instalarHelper() {
        guard !medindo, !Dev.ativo else { return }
        helperErro = nil
        do {
            try servicoHelper.register()
        } catch {
            // O erro 1 ("Operation not permitted") vem tanto em "aguardando aprovação" quanto em
            // falha de verdade: decide pelo status.
            if ![.requiresApproval, .enabled].contains(servicoHelper.status) { helperErro = error.localizedDescription }
        }
        helper = servicoHelper.status
        if helper == .enabled || helper == .requiresApproval { UserDefaults.standard.set(Self.hashHelper(), forKey: "hashHelper") }
        if helper == .requiresApproval { SMAppService.openSystemSettingsLoginItems() }
    }

    func removerHelper() {
        guard !medindo, !Dev.ativo else { return }
        Task {
            do { try await servicoHelper.unregister() } catch { helperErro = error.localizedDescription }
            UserDefaults.standard.removeObject(forKey: "hashHelper")
            helper = servicoHelper.status
        }
    }

    func abrirItensDeInicio() { SMAppService.openSystemSettingsLoginItems() }

    private static func hashHelper() -> String? {
        let url = Bundle.main.bundleURL.appending(path: "Contents/MacOS/SDTallyHelper")
        guard let d = try? Data(contentsOf: url) else { return nil }
        return SHA256.hash(data: d).map { String(format: "%02x", $0) }.joined()
    }

    func esquecerErro() { if case .senhaCancelada = medicao { medicao = .parada } }

    /// Detalhe de pasta, em cache até a próxima medição completa.
    func ramo(_ caminho: String) async throws -> Ramo {
        if let r = ramos[caminho] { return r }
        let r = try await decodificar(Ramo.self, Executor.rodar([caminho, "--json"]), kind: "branch")
        if !r.interrupted { ramos[caminho] = r }
        return r
    }
}

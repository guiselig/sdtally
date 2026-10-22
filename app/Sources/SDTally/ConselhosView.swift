// O que fazer (spec-fase5 §4.4 e §6): advice do sdtally traduzido pelo id. Dono: A4.
// Fase 6 (A11, spec-fase6 §2.3): o cartão lê a ficha (`cards[card]`); sem ficha, volta ao
// `textoConselho(id)` da fase 5. API estável:
//   ConselhosView()                      lê Painel do ambiente
//   CartaoConselho(conselho:home:ficha:) um item (piso: alguma linha com lower_bound)
//   textoConselho(_ id: String) -> String?   pt-BR; nil = id desconhecido (mostra o text)
//   tituloConselho(_ id: String) -> String?  fase 5 + ids novos da fase 6
//   comandosPermitidos: Set<String>      mesma lista do Rust; fora dela, sem "Copiar comando"
//   ajustesPermitidos: [String: URL]     painéis do Ajustes com botão; alvo fora dele, sem botão
//   numerosCartao, linhaEvidencia, textoSozinho, textoRef, linkRelato, acaoNaoConferido: puras
// Ordem dos níveis e cores: Marca.nivel (Marca.swift). Nada aqui apaga: só Finder, Ajustes e área
// de transferência; link de relato é texto com "Copiar link", nunca aberto.
import AppKit
import SwiftUI

/// Os únicos comandos que o app oferece para copiar (src/conselho.rs, COMANDOS). Protege contra
/// JSON adulterado: comando fora daqui nem aparece.
nonisolated let comandosPermitidos: Set<String> = [
    "brew cleanup --dry-run",
    "npm cache clean --force",
    "go clean -cache",
    "pip cache purge",
    "xcrun simctl delete unavailable",
    "xcrun simctl runtime list",
    "ollama list",
    "tmutil listlocalsnapshots /",
]

/// Os comandos de "Ver antes" que o app oferece para copiar (src/conselho.rs, SIMULACOES, coluna
/// `leitura`): só simulam ou listam. Fora daqui, o JSON adulterado não ganha botão.
nonisolated let leiturasPermitidas: Set<String> = [
    "brew cleanup --dry-run",
    "npm cache ls",
    "go clean -cache -n",
    "pip cache info",
    "xcrun simctl list devices",
    "xcrun simctl runtime list",
    "ollama list",
    "tmutil listlocalsnapshots /",
]

/// Título e passo em pt-BR de cada regra do §6.
nonisolated let conselhosPT: [String: (titulo: String, texto: String)] = [
    "grant_full_disk_access": (String(localized: "Dar Acesso Total ao Disco"), String(localized: "Sem o Acesso Total ao Disco, Mail, Mensagens, Safari e outras pastas protegidas ficam bloqueadas pelo macOS. Ative em Configurações (⌘,) e meça de novo.")),
    "measure_as_admin": (String(localized: "Calcular como administrador"), String(localized: "As pastas do sistema só abrem com a senha de administrador ou o Root sem Senha; sem elas, o que não abriu fica bloqueado pelo macOS.")),
    "time_machine_snapshots": (String(localized: "Snapshots locais do Time Machine"), String(localized: "Nada a fazer: o macOS apaga esses snapshots sozinho quando falta espaço. Para ver a lista, use o comando abaixo no Terminal.")),
    "other_snapshots": (String(localized: "Snapshots de outro app"), String(localized: "Snapshot criado por outro app, como um de backup ou de clonagem. Gerencie pelo app que o criou.")),
    "purgeable_auto": (String(localized: "Espaço que o macOS libera sozinho"), String(localized: "Nada a fazer: o macOS libera esse espaço quando precisar.")),
    "deleted_open": (String(localized: "Apagados ainda abertos"), String(localized: "Reiniciar o Mac libera esse espaço.")),
    "swap_large": (String(localized: "Swap grande"), String(localized: "O Mac está usando o disco como memória. Fechar apps pesados ou reiniciar devolve o espaço.")),
    "apple_intelligence_models": (String(localized: "Modelos do Apple Intelligence"), String(localized: "Ficam dentro de Dados do Sistema e o macOS gerencia o tamanho. Não apague a pasta.")),
    "trash": (String(localized: "Lixo"), String(localized: "Esvazie o Lixo no Finder. O espaço volta quando o Lixo é esvaziado.")),
    "logs": (String(localized: "Logs dos apps"), String(localized: "Mova para o Lixo pelo Finder; os apps criam de novo. O espaço volta quando o Lixo é esvaziado.")),
    "homebrew_cache": (String(localized: "Cache do Homebrew"), String(localized: "Rode brew cleanup --dry-run para ver o que sairia. Sem o --dry-run, ele também remove versões antigas das fórmulas instaladas.")),
    "npm_cache": (String(localized: "Cache do npm"), String(localized: "Rode npm cache clean --force; o npm baixa os pacotes de novo quando precisar.")),
    "go_build_cache": (String(localized: "Cache de build do Go"), String(localized: "Rode go clean -cache; o próximo build recompila.")),
    "pip_cache": (String(localized: "Cache do pip"), String(localized: "Rode pip cache purge; o pip baixa os pacotes de novo quando precisar.")),
    "updater_caches": (String(localized: "Instaladores de atualização"), String(localized: "Instaladores já aplicados: mova para o Lixo pelo Finder. O espaço volta quando o Lixo é esvaziado.")),
    "xcode_derived_data": (String(localized: "DerivedData do Xcode"), String(localized: "Mova para o Lixo pelo Finder; o próximo build recompila tudo. O espaço volta quando o Lixo é esvaziado.")),
    "simulators": (String(localized: "Simuladores do Xcode"), String(localized: "Rode xcrun simctl delete unavailable para tirar os que não servem mais; o resto, em Xcode > Devices and Simulators.")),
    "simulator_runtimes": (String(localized: "Runtimes dos simuladores"), String(localized: "Veja quais existem com xcrun simctl runtime list e remova os que não usa em Xcode > Settings > Components.")),
    "playwright_browsers": (String(localized: "Navegadores do Playwright"), String(localized: "Só se você não usa o Playwright: mova para o Lixo pelo Finder. O espaço volta quando o Lixo é esvaziado.")),
    "ollama_models": (String(localized: "Modelos do Ollama"), String(localized: "Veja os modelos com ollama list e remova pelo próprio Ollama os que não usa.")),
    "cinebench_assets": (String(localized: "Arquivos do Cinebench"), String(localized: "Cenas do benchmark, que baixam de novo quando ele roda: mova para o Lixo pelo Finder. O espaço volta quando o Lixo é esvaziado.")),
    "cloudkit_cache": (String(localized: "Cache do CloudKit"), String(localized: "Não apague com o iCloud sincronizando; em geral, deixe como está.")),
    "claude_vm_bundles": (String(localized: "Máquina virtual do app Claude"), String(localized: "Feche o app Claude antes: é a máquina virtual dele, e mexer com ela em uso a corrompe.")),
    "preferences_anomaly": (String(localized: "Preferências grandes demais"), String(localized: "Anomalia: a pasta Preferences deveria ter poucos KB. Procure o app dono antes de mexer.")),
    "icloud_drive": (String(localized: "iCloud Drive"), String(localized: "Não apague: é o iCloud Drive, e os arquivos sumiriam de todos os aparelhos. Para liberar espaço, use “Otimizar Armazenamento do Mac”.")),
    "cloud_storage": (String(localized: "Google Drive, Dropbox e afins"), String(localized: "São dados reais; libere espaço pelo app do provedor.")),
    "system_folders": (String(localized: "Pastas do sistema"), String(localized: "Não mexer: é do sistema; o macOS precisa dessas pastas.")),
]

/// Passo em pt-BR; nil = id desconhecido (quem chama mostra o `text` do sdtally).
nonisolated func textoConselho(_ id: String) -> String? { conselhosPT[id]?.texto }

/// Títulos dos ids novos da fase 6 e das fichas do Início (o passo vem da ficha). `user_folder` não
/// entra: o nome é o da pasta no Finder.
nonisolated let titulosFase6: [String: String] = [
    "restart_worth": String(localized: "Vale a pena reiniciar"),
    "tm_backup_stuck": String(localized: "Backup do Time Machine sem registro novo"),
    "chronod_datavault": String(localized: "Cópias apagadas que o chronod segura"),
    "spotlight_index_large": String(localized: "Índice do Spotlight grande"),
    "failed_smb_mount": String(localized: "Dados gravados dentro de /Volumes"),
    "asset_voices": String(localized: "Vozes do sistema"),
    "asset_xcode": String(localized: "Documentação e ferramentas do Xcode"),
    "asset_sim_runtime": String(localized: "Runtimes de simulador baixados pelo macOS"),
    "asset_os_update": String(localized: "Atualização do macOS baixada"),
    "os_update_snapshot": String(localized: "Snapshot de atualização do macOS"),
    "other_users": String(localized: "Outra conta deste Mac"),
    "container_vm": String(localized: "Máquina virtual de contêineres"),
    "whatsapp_data": String(localized: "Dados do WhatsApp"),
    "browser_cache": String(localized: "Cache do navegador"),
]

/// Título do cartão e nome da ficha em "O que não deu para conferir".
nonisolated func tituloConselho(_ id: String) -> String? { conselhosPT[id]?.titulo ?? titulosFase6[id] }

/// Painéis do Ajustes com botão (spec-fase6 §4.8). Conferidos no macOS 27 pelo Info.plist de cada
/// .appex em /System/Library/ExtensionKit/Extensions: o CFBundleIdentifier, com
/// `allowsXAppleSystemPreferencesURLScheme = true`. `storage` é o Painel.urlArmazenamento.
nonisolated let ajustesPermitidos: [String: URL] = [
    "storage": URL(string: "x-apple.systempreferences:com.apple.settings.Storage")!,
    "time_machine": URL(string: "x-apple.systempreferences:com.apple.Time-Machine-Settings.extension")!,
    "spotlight": URL(string: "x-apple.systempreferences:com.apple.Spotlight-Settings.extension")!,
]
private nonisolated let nomesAjustes = ["storage": String(localized: "Armazenamento"), "time_machine": "Time Machine", "spotlight": "Spotlight"]

/// Botões de Ajustes da ficha: só os alvos de `ajustesPermitidos`.
nonisolated func botoesAjustes(_ f: Ficha?) -> [(rotulo: String, url: URL)] {
    (f?.acoes ?? []).compactMap {
        guard case let .ajustes(t) = $0, let u = ajustesPermitidos[t] else { return nil }
        return (String(localized: "Abrir \(nomesAjustes[t] ?? t) no Ajustes"), u)
    }
}

/// "O macOS cuida sozinho?" da ficha.
nonisolated func textoSozinho(_ f: Ficha) -> String {
    let t = f.selfCleansText?.texto
    switch f.selfCleans {
    case "yes": return t.map { String(localized: "Sim: \($0)") } ?? String(localized: "Sim")
    case "when": return t.map { String(localized: "Quando: \($0)") } ?? String(localized: "Depende")
    case "no": return String(localized: "Não")
    default: return String(localized: "Não informado")
    }
}

/// Uma referência da ficha, como texto (nada é aberto).
nonisolated func textoRef(_ r: RefFicha) -> String {
    switch r.kind {
    case "internal": String(localized: "Projeto: \(r.ref)")
    case "measured": String(localized: "Medido pelo projeto: \(r.ref)")
    case "apple": String(localized: "Apple: \(r.ref)")
    case "report": String(localized: "Relato: \(r.ref)")
    default: r.ref
    }
}

/// Link de relato que pode ir para "Copiar link": só https.
nonisolated func linkRelato(_ r: RefFicha) -> String? {
    r.kind == "report" && r.ref.hasPrefix("https://") ? r.ref : nil
}

/// "Texto conferido no macOS 27" (o `verified_on` "27.0" sai pela versão principal).
nonisolated func textoConferido(_ f: Ficha) -> String {
    let v = f.verifiedOn.split(separator: ".").first.map(String.init) ?? f.verifiedOn
    return String(localized: "Texto conferido no macOS \(v)")
}

/// Nome de uma chave de `evidence`.
nonisolated func nomeEvidencia(_ key: String) -> String {
    switch key {
    case "private_bytes": String(localized: "Espaço privado")
    case "bytes": String(localized: "Ocupa")
    case "files_without_allocated": String(localized: "Arquivos sem tamanho alocado legível")
    case "logical_bytes_without_allocated": String(localized: "Tamanho lógico (alocado ilegível)")
    case "launchservices_dv_files": String(localized: "Arquivos do banco do LaunchServices")
    default: key
    }
}

/// Uma linha de "Por que apareceu": valor, limite e fonte.
nonisolated func linhaEvidencia(_ e: Evidencia) -> String {
    let bytes = e.key.contains("bytes")
    func f(_ n: Int64) -> String { bytes ? gb(n) : n.formatted(.number.locale(localApp)) }
    return String(localized: "\(nomeEvidencia(e.key)): \(f(e.value)), limite \(f(e.limit)); fonte: \(nomeFonte(e.source))")
}

/// Números do cartão: rótulo, valor e nota. "Libera se apagar" só onde apagar é o passo (nível e
/// ação da ficha) e o privado não veio 0 (AssetsV2, spec-fase6 §0); `bytes_logical` diz "lógico".
/// Backup parado: o comando só lista e o passo é o backup terminar (R1), nunca apagar o snapshot.
nonisolated func numerosCartao(_ c: Conselho, ficha: Ficha?, piso: Bool) -> (rotulo: String, valor: String, nota: String?)? {
    if c.bytesLogical == true { return (String(localized: "Lógico, fora da soma"), gb(c.bytes), String(localized: "tamanho lógico do lsof")) }
    let apagavel = ficha?.acoes.contains { switch $0 { case .finder, .appDono, .comando: true; default: false } } ?? true
    let p = piso ? "≥ " : ""
    if ["safe", "cost", "check_first", "anomaly"].contains(c.level), !["deleted_open", "tm_backup_stuck"].contains(c.id), apagavel, c.privateBytes != 0 {
        return (String(localized: "Libera se apagar"), c.privateBytes.map(gb) ?? "≤ " + gb(c.bytes), String(localized: "Ocupa \(p)\(gb(c.bytes))"))
    }
    guard c.bytes > 0 else { return nil }
    return (c.id == "measure_as_admin" ? String(localized: "Bloqueado pelo macOS") : String(localized: "Ocupa"), p + gb(c.bytes), nil)
}

/// O botão de cada motivo de `advice_unchecked` (V18).
nonisolated func acaoNaoConferido(_ reason: String) -> AcaoVazio? {
    switch reason {
    case "term not measured": .medir
    case "no lsof", "plist unreadable", "tm status unreadable": .medirDeNovo
    default: nil
    }
}

/// Comando que pode ir para "Copiar comando": só o que está na lista fixa.
nonisolated func comandoCopiavel(_ c: Conselho) -> String? {
    c.command.flatMap { comandosPermitidos.contains($0) ? $0 : nil }
}

/// "Ver antes": a simulação ou listagem do comando, só quando existe, é da lista fixa e difere do
/// próprio comando (que, se já só lê, não precisa de outra linha).
nonisolated func verAntes(_ c: Conselho) -> SimulacaoComando? {
    guard let cmd = comandoCopiavel(c), let d = c.dryRun, d.command != cmd, leiturasPermitidas.contains(d.command) else { return nil }
    return d
}

/// R6: o app a reabrir só quando exatamente um app comum casa com o `process` (o p_comm vem
/// cortado em 15 caracteres; aí vale o começo do nome do executável).
nonisolated func appParaReabrir(_ process: String?, apps: [(executavel: String, nome: String)]) -> String? {
    guard let p = process, !p.isEmpty else { return nil }
    let casam = apps.filter { $0.executavel == p || (p.count >= 15 && $0.executavel.hasPrefix(p)) }
    return casam.count == 1 ? casam[0].nome : nil
}

/// Texto que o cartão mostra: o passo da ficha (sem ficha, o pt-BR do id), com o app da R6 e o
/// percentual do Bloqueado pelo macOS.
nonisolated func textoMostrado(_ c: Conselho, ficha: Ficha? = nil, percentual: Double? = nil, apps: [(executavel: String, nome: String)] = []) -> String {
    let base = ficha?.step.texto ?? textoConselho(c.id) ?? c.text
    switch c.id {
    case "deleted_open":
        if let nome = appParaReabrir(c.process, apps: apps) {
            return String(localized: "Feche e abra de novo o \(nome). O espaço volta quando ele soltar os arquivos apagados.")
        }
    case "measure_as_admin":
        if let p = percentual {
            let pct = p.formatted(.number.precision(.fractionLength(0...1)).locale(localApp))
            return String(localized: "Hoje \(pct) % do usado está bloqueado pelo macOS.") + " " + base
        }
    default: break
    }
    return base
}

/// Os `limite` maiores caminhos pelos bytes das linhas do `top` (empate: ordem do sdtally) e o
/// resto, que a tela recolhe em "e mais N". Até `limite`, nada muda.
nonisolated func caminhosPrincipais(_ paths: [String], top: [LinhaTop], limite: Int = 5) -> (visiveis: [String], resto: [String]) {
    guard paths.count > limite else { return (paths, []) }
    let peso = Dictionary(top.compactMap { l in l.path.map { ($0, l.bytes) } }, uniquingKeysWith: +)
    let ordem = paths.enumerated().sorted { (peso[$0.element] ?? 0, -$0.offset) > (peso[$1.element] ?? 0, -$1.offset) }.map(\.element)
    return (Array(ordem.prefix(limite)), Array(ordem.dropFirst(limite)))
}

nonisolated let ordemNiveis = ["measurement", "system_handles", "safe", "cost", "check_first", "anomaly", "do_not_touch"]

/// Nível na ordem do §4.4; dentro dele, o que mais libera primeiro. Nível desconhecido vai ao fim.
nonisolated func ordenarConselhos(_ a: [Conselho]) -> [Conselho] {
    func n(_ c: Conselho) -> Int { ordemNiveis.firstIndex(of: c.level) ?? ordemNiveis.count }
    return a.sorted { (n($0), -($0.privateBytes ?? $0.bytes)) < (n($1), -($1.privateBytes ?? $1.bytes)) }
}

nonisolated func nomeNivel(_ level: String) -> String {
    switch level {
    case "measurement": String(localized: "Medição")
    case "system_handles": String(localized: "O macOS cuida")
    case "safe": String(localized: "Seguro")
    case "cost": String(localized: "Tem custo")
    case "check_first": String(localized: "Confira antes")
    case "anomaly": String(localized: "Anomalia")
    case "do_not_touch": String(localized: "Não mexa")
    default: level
    }
}

/// O que acontece se seguir o passo, por nível.
nonisolated func custoNivel(_ level: String) -> String {
    switch level {
    case "measurement": String(localized: "Nada se perde; a medição fica mais completa.")
    case "system_handles": String(localized: "Nada a fazer: o macOS cuida sozinho.")
    case "safe": String(localized: "Nada se perde: regenera ou é descartável.")
    case "cost": String(localized: "Regenera, mas custa tempo ou banda.")
    case "check_first": String(localized: "Depende do estado na hora; confira antes.")
    case "anomaly": String(localized: "Não deveria ocupar tanto; descubra o dono antes.")
    default: String(localized: "Dados reais ou do sistema: apagar faz falta.")
    }
}

let rodapeConselhos = String(localized: "O SDTally não apaga nada. Cada passo é seu, no Finder, no Terminal ou no app dono. Depois, meça de novo: o Histórico mostra quanto voltou.")

struct ConselhosView: View {
    @Environment(Painel.self) private var painel

    var body: some View {
        if let c = painel.dados, let todos = c.advice, !todos.isEmpty || c.adviceUnchecked?.isEmpty == false || !causasNaoVoltou(c.notFreed).isEmpty {
            // measure_as_admin fica de fora: a linha de medição do Início já é esse passo (spec-inicio §5).
            let a = todos.filter { $0.id != "measure_as_admin" }
            // Os links do Início (rolarPara) e SDTALLY_ANCORA=<id do advice> ou nao_conferido rolam até lá.
            ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    // R7
                    if let d = data(c.generatedAt), Date.now.timeIntervalSince(d) > 24 * 3600 {
                        Faixa(simbolo: "clock.badge.exclamationmark", texto: String(localized: "Medido \(idade(d))"))
                    }
                    // V17
                    if !a.contains(where: { ["safe", "cost", "check_first"].contains($0.level) }) {
                        VazioLinha(vazio: .v17) {
                            Button(AcaoVazio.verArmazenamento.rotulo) { painel.secao = .sistema }
                        }
                    }
                    let ordenados = ordenarConselhos(a)
                    // Já ordenados por nível: cada nível aparece uma vez, na ordem.
                    let niveis = ordenados.map(\.level).reduce(into: [String]()) { if !$0.contains($1) { $0.append($1) } }
                    ForEach(niveis, id: \.self) { nivel in
                        let n = Marca.nivel(nivel)
                        Label(nomeNivel(nivel), systemImage: n.simbolo)
                            .font(.headline).foregroundStyle(n.cor)
                            .padding(.top, 8)
                        ForEach(ordenados.filter { $0.level == nivel }, id: \.id) { item in
                            CartaoConselho(conselho: item, home: c.home, ficha: c.cards?[item.card],
                                           piso: c.top.contains { $0.lowerBound && $0.path.map(item.paths.contains) == true })
                            .id(item.id)
                        }
                    }
                    PorQueNaoVoltou(causas: causasNaoVoltou(c.notFreed)).id("nao_voltou")
                    if let nc = c.adviceUnchecked, !nc.isEmpty {
                        NaoConferidos(itens: nc, cards: c.cards, root: c.root).id("nao_conferido")
                    }
                }
                .padding(24)
                .frame(maxWidth: 760)
                .frame(maxWidth: .infinity)
            }
            .task(id: c.generatedAt) {
                guard let a = painel.rolarPara ?? Foto.ancora else { return }
                painel.rolarPara = nil
                try? await Task.sleep(for: .milliseconds(300))
                proxy.scrollTo(a, anchor: .top)
            }
            }
        } else if painel.manual || painel.dados != nil {
            VazioTela(vazio: .v20) {
                Button("Medir com senha") { painel.medirComSenha() }
                    .disabled(painel.medindo)
            }
        } else {
            SemMedicao(simbolo: "checklist")
        }
    }
}

/// Um item: faixa na cor do nível, título, números, caminhos e a ficha na ordem do spec-fase6
/// §2.3 (o que é, de quem é, cuida sozinho, Libera/Ocupa no topo, o que se perde, como agir,
/// fonte, conferido). Sem ficha, as seções da fase 5 (passo, se fizer, origem, comando).
/// R3: só "Mostrar no Finder" e painéis de `ajustesPermitidos`, nunca abrir arquivo.
struct CartaoConselho: View {
    @Environment(Painel.self) private var painel
    @Environment(\.colorSchemeContrast) private var contraste
    let conselho: Conselho
    let home: String
    var ficha: Ficha?
    var piso = false

    var body: some View {
        let c = conselho
        let n = Marca.nivel(c.level)
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline, spacing: 12) {
                VStack(alignment: .leading, spacing: 4) {
                    if c.kind == "signature" {
                        Label("Padrão reconhecido", systemImage: "text.magnifyingglass")
                            .font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                    }
                    Text(tituloConselho(c.id) ?? c.term.map(nomeTermo) ?? String(localized: "Recomendação do sdtally")).font(.headline)
                        .help(dicaCartao)
                    Label(nomeNivel(c.level), systemImage: n.simbolo)
                        .font(.caption.weight(.semibold)).foregroundStyle(n.cor)
                }
                Spacer()
                numeros
            }
            if !c.paths.isEmpty { caminhos }
            Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 12, verticalSpacing: 6) {
                if let f = ficha {
                    linha(String(localized: "O que se perde"), f.loses.texto)
                    linha(String(localized: "Como agir"), texto)
                } else {
                    linha(String(localized: "Passo"), texto)
                }
                if let cmd = comandoCopiavel(c) {
                    GridRow {
                        Text("Comando").foregroundStyle(.secondary)
                        HStack {
                            Text(cmd).font(.callout.monospaced()).textSelection(.enabled)
                            Button("Copiar comando") { copiar(cmd) }.controlSize(.small)
                        }
                    }
                }
                if let d = verAntes(c) {
                    GridRow {
                        Text("Ver antes").foregroundStyle(.secondary)
                        HStack {
                            Text(d.command).font(.callout.monospaced()).textSelection(.enabled)
                            Button("Copiar comando") { copiar(d.command) }.controlSize(.small)
                        }
                        .help("Só mostra o que o comando acima faria. Fonte: \(d.doc)")
                    }
                }
                // 6. Como agir (botões), antes da fonte (spec-fase6 §2.3).
                if ficha != nil, temAcao {
                    GridRow {
                        Color.clear.gridCellUnsizedAxes([.horizontal, .vertical])
                        HStack { acao }
                    }
                }
                // Só o relato com link ganha linha (botão Copiar link); as outras fontes vão no tooltip do título.
                if let f = ficha {
                    ForEach(Array(f.refs.compactMap(linkRelato).enumerated()), id: \.offset) { _, l in
                        GridRow {
                            Text("Relato").foregroundStyle(.secondary)
                            HStack {
                                Text(visivel(l)).lineLimit(1).truncationMode(.middle).textSelection(.enabled)
                                Button("Copiar link") { copiar(l) }.controlSize(.small)
                            }
                        }
                    }
                }
            }
            .font(.callout)
            if ficha == nil, temAcao { HStack { acao } }
        }
        .padding(16)
        .padding(.leading, 3)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background {
            ZStack(alignment: .leading) {
                Rectangle().fill(.background.secondary)
                Rectangle().fill(n.cor).frame(width: 3)
            }
            .clipShape(.rect(cornerRadius: 12))
        }
        .overlay {
            if contraste == .increased { RoundedRectangle(cornerRadius: 12).strokeBorder(.separator, lineWidth: 1) }
        }
    }

    private var texto: String {
        let apps = conselho.id == "deleted_open"
            ? NSWorkspace.shared.runningApplications.filter { $0.activationPolicy == .regular }.compactMap { a in
                a.executableURL.map { ($0.lastPathComponent, a.localizedName ?? $0.lastPathComponent) }
            }
            : []
        return textoMostrado(conselho, ficha: ficha, apps: apps)
    }

    /// Tooltip do título: o que o cartão não mostra (por que apareceu, de quem é, origem, conferência).
    private var dicaCartao: String {
        var l: [String] = []
        if let ev = conselho.evidence, !ev.isEmpty { l.append(String(localized: "Por que apareceu: \(ev.map(linhaEvidencia).joined(separator: "; "))")) }
        if let f = ficha {
            l.append(String(localized: "O que é: \(f.what.texto)"))
            l.append(String(localized: "De quem é: \(f.owner.texto)"))
            l.append(String(localized: "O macOS cuida sozinho? \(textoSozinho(f))"))
            l += f.refs.map { String(localized: "Fonte: \(visivel(textoRef($0)))") }
            l.append(textoConferido(f))
        } else {
            l.append(String(localized: "Se fizer: \(custoNivel(conselho.level))"))
            l.append(String(localized: "Origem: \(origem)"))
        }
        return l.joined(separator: "\n")
    }

    private var origem: String {
        if let p = conselho.process { return String(localized: "\(nomeTermo(conselho.term ?? "deleted_open")) · processo \(p)") }
        if conselho.term == "snapshots" { return String(localized: "Snapshots locais (APFS)") }
        if let t = conselho.term { return conselho.paths.isEmpty ? nomeTermo(t) : String(localized: "\(nomeTermo(t)) · Pastas maiores") }
        return conselho.paths.isEmpty ? String(localized: "Medição completa") : String(localized: "Caminhos")
    }

    @ViewBuilder private var numeros: some View {
        if let m = numerosCartao(conselho, ficha: ficha, piso: piso) {
            VStack(alignment: .trailing, spacing: 2) {
                Text(m.rotulo).font(.caption).foregroundStyle(.secondary)
                Text(m.valor).font(.kpi)
                if let nota = m.nota { Text(nota).font(.caption.monospacedDigit()).foregroundStyle(.secondary) }
            }
            .accessibilityElement(children: .combine)
        }
    }

    /// Mais de 5 caminhos (o system_folders real traz dezenas): os 5 maiores e "e mais N" recolhido.
    private var caminhos: some View {
        let cs = caminhosPrincipais(conselho.paths, top: painel.dados?.top ?? [])
        return VStack(alignment: .leading, spacing: 4) {
            ForEach(cs.visiveis, id: \.self, content: caminho)
            if !cs.resto.isEmpty {
                DisclosureGroup("e mais \(cs.resto.count)") {
                    VStack(alignment: .leading, spacing: 4) { ForEach(cs.resto, id: \.self, content: caminho) }
                }
                .font(.callout)
            }
        }
    }

    private func caminho(_ p: String) -> some View {
        let abs = absoluto(p, home: home)
        return HStack(spacing: 8) {
            Image(systemName: "folder").foregroundStyle(.secondary).accessibilityHidden(true)
            Text(p).font(.callout.monospaced()).lineLimit(1).truncationMode(.middle).help(abs)
            Spacer(minLength: 8)
            Button("Mostrar no Finder") { NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: abs)]) }
            Button("Copiar caminho") { copiar(abs) }
        }
        .controlSize(.small)
    }

    private var temAcao: Bool {
        conselho.id == "grant_full_disk_access" || !botoesAjustes(ficha).isEmpty
    }

    @ViewBuilder private var acao: some View {
        switch conselho.id {
        case "grant_full_disk_access":
            Button("Abrir Permissões") { painel.secao = .permissoes }
        default: EmptyView()
        }
        ForEach(botoesAjustes(ficha), id: \.url) { b in Button(b.rotulo) { NSWorkspace.shared.open(b.url) } }
    }

    private func linha(_ rotulo: String, _ valor: String) -> some View {
        GridRow {
            Text(rotulo).foregroundStyle(.secondary)
            Text(valor).fixedSize(horizontal: false, vertical: true)
        }
    }

    private func copiar(_ s: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(s, forType: .string)
    }
}

/// "O que não deu para conferir" (V18): uma linha por `advice_unchecked`, o motivo bruto só no
/// popover e um botão por ação distinta.
/// Causas medidas de "apaguei e o espaço não voltou" (`not_freed`, src/voltou.rs): só as que têm
/// texto e número; JSON sem a chave ou adulterado não mostra nada.
nonisolated func causasNaoVoltou(_ l: [NaoVoltou]?) -> [NaoVoltou] {
    (l ?? []).filter { $0.bytes > 0 && !$0.title.texto.isEmpty && !$0.text.texto.isEmpty }
}

/// "Snapshots locais (8)", "Clones APFS (82.000 arquivos)": o título, com a contagem quando a fonte tem.
nonisolated func tituloNaoVoltou(_ n: NaoVoltou) -> String {
    guard let q = n.count else { return n.title.texto }
    let num = q.formatted(.number.locale(localApp))
    let conta = n.id == "local_snapshot" ? num : q == 1 ? String(localized: "1 arquivo") : String(localized: "\(num) arquivos")
    return "\(n.title.texto) (\(conta))"
}

private struct PorQueNaoVoltou: View {
    let causas: [NaoVoltou]

    var body: some View {
        if !causas.isEmpty {
            Label("Por que o espaço pode não voltar", systemImage: "questionmark.circle")
                .font(.headline).foregroundStyle(.secondary)
                .padding(.top, 8)
            VStack(alignment: .leading, spacing: 12) {
                ForEach(causas, id: \.id) { n in
                    VStack(alignment: .leading, spacing: 2) {
                        HStack(alignment: .firstTextBaseline) {
                            Text(tituloNaoVoltou(n)).font(.subheadline.weight(.semibold))
                            Spacer()
                            Text((n.bytesFloor == true ? "≥ " : "") + gb(n.bytes)).monospacedDigit()
                        }
                        Text("Fonte: \(nomeFonte(n.source))").font(.caption).foregroundStyle(.secondary)
                    }
                    .help(n.text.texto)
                }
            }
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(.background.secondary, in: .rect(cornerRadius: 12))
        }
    }
}

private struct NaoConferidos: View {
    @Environment(Painel.self) private var painel
    let itens: [NaoConferido]
    let cards: MapaFichas?
    let root: Bool

    var body: some View {
        let acoes = itens.compactMap { acaoNaoConferido($0.reason) }.reduce(into: [AcaoVazio]()) { if !$0.contains($1) { $0.append($1) } }
        Label(Vazio.v18.titulo, systemImage: Vazio.v18.simbolo)
            .font(.headline).foregroundStyle(.secondary)
            .padding(.top, 8)
        VStack(alignment: .leading, spacing: 8) {
            ForEach(Array(itens.enumerated()), id: \.offset) { _, u in
                LinhaNaoConferido(item: u, nome: tituloConselho(u.card ?? u.id) ?? tituloConselho(u.id) ?? visivel(u.id))
            }
            if !acoes.isEmpty {
                HStack {
                    ForEach(acoes, id: \.rotulo) { a in
                        Button(a.rotulo) { painel.medir(admin: a == .medir || root) }.disabled(painel.medindo)
                    }
                }
                .controlSize(.small)
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.background.secondary, in: .rect(cornerRadius: 12))
    }
}

private struct LinhaNaoConferido: View {
    let item: NaoConferido
    let nome: String
    @State private var aberto = false

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            Text(linhaNaoConferido(nome: nome, reason: visivel(item.reason))).font(.callout)
                .fixedSize(horizontal: false, vertical: true)
            Button { aberto.toggle() } label: { Image(systemName: "info.circle") }
                .buttonStyle(.borderless)
                .accessibilityLabel("Motivo (bruto)")
                .popover(isPresented: $aberto) {
                    Text("Motivo (bruto): \(visivel(item.reason))").font(.callout.monospaced()).padding(12)
                }
        }
    }
}

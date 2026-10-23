// Formas do JSON do sdtally (docs/json-schema.md). Chaves em snake_case, decodificadas com
// convertFromSnakeCase. Tudo opcional que o schema permite ser null.
// Fase 6 (spec-fase6 §3, §4.8): toda chave nova é opcional (`var … = nil`), para arquivo antigo e
// para o init membro a membro dos testes. API estável para os marcos A9–A12:
//   Completo.bootTime, .cards, .adviceUnchecked; Rapido.bootTime, .cards
//   Conselho.kind ("rule"|"signature"), .card, .evidence [Evidencia], .bytesLogical
//   card: String? em LinhaTop, ServicoPurgavel, Conteiner.Volume, Termo, Snapshot, ProcessoApagado
//   Completo.Snapshot.purgeable, .cumulativeBytes, .creator, .tmRole, .verdict
//   Completo.ProcessoApagado.bytes (Int64?: null sem alocado legível), .filesWithoutAllocated,
//     .logicalBytesWithoutAllocated, .launchservicesDvFiles
//   Completo.Informativo.timeMachine: TimeMachine? (destinations: [TimeMachine.Destino]?)
//   MapaFichas (cards[id] → Ficha; ficha sem campo obrigatório é descartada), fichas[id]
//   Ficha.acoes: [AcaoFicha] (kind desconhecido é ignorado), Ficha.refs: [RefFicha]
//   Catalogo (sdtally --catalog --json, kind "catalog"): .cards, .sources[source] → Conferencia
//   Completo.Informativo.trash (Lixo por idade), .apfsMetadata (nós das árvores vivas); nil em medição antiga
import Foundation

nonisolated struct Medida: Decodable, Sendable {
    let bytes: Int64?
    let state: String
    let reason: String?
    let source: String
}

nonisolated struct Categoria: Decodable, Sendable, Identifiable {
    let id: String
    let bytes: Int64
}

nonisolated struct Servico: Decodable, Sendable {
    let loggedAt: String?
    let total, available, purgeable, otherVolumes, used, system, other: Int64?
}

/// A tela do Ajustes pelo próprio log (`settings_screen.screen`).
nonisolated struct Tela: Decodable, Sendable {
    let state: String
    let reason: String?
    let loggedAt: String?
    let used, available, capacity, systemData, categoriesTotal: Int64?
    let categories: [Categoria]?
    let service: Servico?
}

/// Apple Intelligence (`settings_screen.apple_intelligence`): Medida + `physical`.
nonisolated struct IA: Decodable, Sendable {
    let bytes: Int64?
    let state: String
    let reason: String?
    let source: String
    let physical: Medida?
}

/// `settings_screen`. O `system_data` daqui (log do serviço) não é decodificado de propósito:
/// não é o número da tela (research/mapa-de-fontes.md §6).
nonisolated struct Ajustes: Decodable, Sendable {
    /// Troca pelo do laço de 1 s (`Painel.aplicar`, volumes montados).
    var macos: Medida
    let screen: Tela?
    let appleIntelligence: IA?
}

nonisolated struct ServicoPurgavel: Decodable, Sendable {
    let name: String
    let bytes: Int64
    var card: String? = nil
}

nonisolated struct Purgavel: Decodable, Sendable {
    let bytes: Int64?
    let state: String
    let reason: String?
    let source: String
    let topServices: [ServicoPurgavel]?
}

nonisolated struct ErroEtapa: Decodable, Sendable {
    let stage: String
    let error: String
}

/// `containers[]` (diskutil apfs list): sem nome de volume, que pode ser pessoal.
nonisolated struct Conteiner: Decodable, Sendable {
    struct Volume: Decodable, Sendable {
        let roles: [String]
        /// Montado: troca pelo do laço de 1 s (`Painel.aplicar`).
        var capacityInUse: Int64?
        /// Entra no macOS da tela (disco interno, fora o Data e o ISC).
        let inSettingsMacos: Bool
        var card: String? = nil
        /// `disk3s5`: o app acha o volume montado por ele. Ausente em JSON antigo.
        var device: String? = nil
    }
    /// nil quando o diskutil não diz o tipo do disco: tratado como não interno.
    let `internal`: Bool?
    /// Container ISC (xART, Hardware): o Ajustes não mostra.
    let isc: Bool
    let holdsData: Bool
    let ceiling: Int64?
    var free: Int64?
    var volumes: [Volume]
}

/// `swap` (sysctl vm.swapusage, sem root). Os swapfiles ficam no volume VM.
nonisolated struct Swap: Decodable, Sendable {
    let total: Int64?
    let used: Int64?
}

/// `advice[]` (src/conselho.rs). `text` em inglês; o app traduz pelo `id`.
nonisolated struct Conselho: Decodable, Sendable {
    let id: String
    /// measurement, system_handles, safe, cost, check_first, do_not_touch, anomaly.
    let level: String
    let bytes: Int64
    /// "Libera se apagar"; null usa `bytes` com "≤".
    let privateBytes: Int64?
    let paths: [String]
    let term: String?
    let process: String?
    let command: String?
    let text: String
    /// "rule" ou "signature" (fase 6); nil em arquivo antigo.
    var kind: String? = nil
    var card: String? = nil
    /// Só nos `signature`: o número medido, o limite e a fonte ("Por que apareceu").
    var evidence: [Evidencia]? = nil
    /// `true` só no `chronod_datavault`: `bytes` lógicos, fora de qualquer soma.
    var bytesLogical: Bool? = nil
    /// `true` só no `tm_backup_stuck` com snapshot de outro tipo: `bytes` é um piso.
    var bytesFloor: Bool? = nil
    /// O comando que só mostra o que `command` faria (ou só lista) e a fonte; nil em arquivo antigo.
    var dryRun: SimulacaoComando? = nil
}

/// `advice[].dry_run` (src/conselho.rs, SIMULACOES).
nonisolated struct SimulacaoComando: Decodable, Sendable, Equatable {
    let command: String
    let doc: String
}

/// `advice[].evidence[]`.
nonisolated struct Evidencia: Decodable, Sendable, Equatable {
    let key: String
    let value: Int64
    let limit: Int64
    let source: String
}

/// `not_freed[]` (src/voltou.rs): causa medida de apagar algo e o espaço não voltar. `title` e `text`
/// já vêm em pt e en; só existe a causa que tem número medido.
nonisolated struct NaoVoltou: Decodable, Sendable, Equatable {
    let id: String
    let title: TextoFicha
    let text: TextoFicha
    let bytes: Int64
    let bytesFloor: Bool?
    let count: Int64?
    let source: String
}

/// `advice_unchecked[]`: detector que não deu para olhar. `reason` em inglês (spec-fase6 §5.6).
nonisolated struct NaoConferido: Decodable, Sendable {
    let id: String
    let card: String?
    let reason: String
}

/// `sdtally --quick --json`: o que muda a toda hora, ~3–4 s.
nonisolated struct Rapido: Decodable, Sendable {
    struct Volume: Decodable, Sendable { var used: Medida }
    struct Capacidade: Decodable, Sendable { let total: Int64; let free: Int64 }

    let generatedAt: String
    let tool: String?
    /// Option no Rust: null não derruba a leitura inteira.
    let macosVersion: String?
    let home: String
    let root: Bool
    let fullDiskAccess: Bool?
    let interrupted: Bool
    var settingsScreen: Ajustes
    /// Uso e swap: o laço de 1 s do Painel (`vigiarTempoReal`) troca pelos valores lidos no app.
    var dataVolume: Volume
    let capacity: Capacidade?
    /// Também troca pelo do app (`Painel.purgeavelAgora`, a cada 10 s / 1 min).
    var purgeable: Purgavel
    let stageErrors: [ErroEtapa]
    var containers: [Conteiner]?
    var swap: Swap?
    var bootTime: String? = nil
    var cards: MapaFichas? = nil
}

nonisolated struct Termo: Decodable, Sendable, Identifiable {
    let key: String
    let bytes: Int64?
    let state: String
    let reason: String?
    let source: String
    var card: String? = nil
    var id: String { key }
}

/// `top[].owner`: o app dono da linha, quando o sdtally achou fonte (container, Info.plist, grupo, nome).
/// `name` nil: o id é conhecido, mas nenhum app instalado tem esse id. `source` diz como se soube.
nonisolated struct DonoApp: Decodable, Sendable, Equatable {
    let bundleId: String
    let name: String?
    let source: String
}

nonisolated struct LinhaTop: Decodable, Sendable, Identifiable {
    struct Processo: Decodable, Sendable { let name: String; let bytes: Int64 }
    let path: String?
    let term: String
    let bytes: Int64
    let privateBytes: Int64?
    let lowerBound: Bool
    let count: Int?
    let topProcess: Processo?
    let source: String
    /// Baldes pequenos dobrados: `path` é a pasta pai ("demais itens de …").
    let folded: Bool?
    var card: String? = nil
    /// Provedor de `FileProvider/<uuid>` ("iCloud Drive", "File Provider"...), grau B. nil nas outras.
    var label: String? = nil
    /// App dono da pasta (`owner`), só quando há fonte; nil em JSON sem a chave.
    var owner: DonoApp? = nil
    var id: String { "\(path ?? "-")|\(term)|\(folded == true)" }
}

/// `sdtally --json`: a conta inteira do volume Data (travessia; minutos).
nonisolated struct Completo: Decodable, Sendable {
    struct Inexplicado: Decodable, Sendable {
        let bytes: Int64?
        let percentOfUsed: Double?
        let state: String
        /// Σ termos − usado, quando as partes passaram do usado (então `bytes` é nil).
        let overlapBytes: Int64?

        enum CodingKeys: String, CodingKey { case bytes, percentOfUsed, state, overlapBytes }

        /// Medição salva por um sdtally antigo traz o resto negativo: aqui vira não medido, nunca
        /// um número negativo na tela.
        init(from d: Decoder) throws {
            let c = try d.container(keyedBy: CodingKeys.self)
            let b = try c.decodeIfPresent(Int64.self, forKey: .bytes)
            let o = try c.decodeIfPresent(Int64.self, forKey: .overlapBytes)
            if let b, b < 0 {
                (bytes, percentOfUsed, state, overlapBytes) = (nil, nil, "not_measured", o ?? -b)
            } else {
                (bytes, percentOfUsed, state, overlapBytes) = (b, try c.decodeIfPresent(Double.self, forKey: .percentOfUsed), try c.decode(String.self, forKey: .state), o)
            }
        }
    }
    struct Volume: Decodable, Sendable {
        let used: Medida
        let terms: [Termo]
        let unexplained: Inexplicado
    }

    struct Contagem: Decodable, Sendable { let files: Int64; let bytes: Int64 }
    struct Dataless: Decodable, Sendable { let files: Int64; let apparentBytes: Int64; let foldersNotOpened: Int64 }
    /// `informational.snapshots[]`: `kind` ∈ time_machine, apple_other, other; `date` só no TM.
    /// Fase 6: `verdict` ∈ tm_reference, tm_stable, tm_recent, tm_aged, apple_other, third_party.
    struct Snapshot: Decodable, Sendable {
        let kind: String
        let date: String?
        let privateBytes: Int64?
        var purgeable: Bool? = nil
        var cumulativeBytes: Int64? = nil
        var creator: String? = nil
        /// "reference", "stable" ou nil.
        var tmRole: String? = nil
        var verdict: String? = nil
        var card: String? = nil
    }
    /// `informational.deleted_open_processes[]`: os 10 maiores por max(bytes, lógico).
    struct ProcessoApagado: Decodable, Sendable {
        let name: String
        /// Alocado medido; nil quando nenhum arquivo dele teve alocado legível.
        let bytes: Int64?
        let files: Int64
        var filesWithoutAllocated: Int64? = nil
        /// SIZE do lsof dos sem alocado: lógico, fora de qualquer soma.
        var logicalBytesWithoutAllocated: Int64? = nil
        var launchservicesDvFiles: Int64? = nil
        var card: String? = nil
    }
    /// `informational.time_machine` (plist do TM chave por chave + `tmutil status`).
    /// Datas "AAAA-MM-DD HH:MM:SS" em hora local.
    struct TimeMachine: Decodable, Sendable {
        struct Destino: Decodable, Sendable {
            let referenceAt, stableAt, lastCompletedAt, lastAttemptAt: String?
            let attemptsSinceCompleted: Int?
            let resultRaw: Int64?
        }
        let state: String
        let reason: String?
        let source: String
        let running: Bool?
        let destinations: [Destino]?
    }
    /// `informational.trash`: o `~/.Trash` por idade (data de entrada no Lixo, `ATTR_CMN_ADDEDTIME`).
    /// `older` é o que entrou há mais de `thresholdDays`; `items` nil quando não mediu.
    struct Lixo: Decodable, Sendable {
        struct Parte: Decodable, Sendable {
            let bytes: Int64?
            let state: String
            let reason: String?
            let source: String
            let items: Int64?
        }
        let thresholdDays: Int
        let total: Parte
        let older: Parte
        /// Itens sem data de entrada (ficam fora de `older`); nil quando não mediu.
        let undated: SemData?
        let source: String
    }
    struct SemData: Decodable, Sendable { let items: Int64; let bytes: Int64 }
    /// `informational.apfs_metadata`: Medida + nós das árvores vivas do volume Data e o bloco.
    struct MetadadosAPFS: Decodable, Sendable {
        struct Nos: Decodable, Sendable { let omap, fsroot, extentref: Int64 }
        let bytes: Int64?
        let state: String
        let reason: String?
        let source: String
        let nodes: Nos?
        let blockSize: Int64?
    }
    struct Informativo: Decodable, Sendable {
        let purgeable: Purgavel?
        let repeatedFullClones, repeatedHardlinks: Contagem?
        let graftRootsSkipped: Int?
        let scanState: String?
        let dataless: Dataless?
        let snapshots: [Snapshot]?
        let deletedOpenProcesses: [ProcessoApagado]?
        var timeMachine: TimeMachine? = nil
        /// Fases do "O que ainda não é medido": nil em medição salva por um sdtally antigo.
        var trash: Lixo? = nil
        var apfsMetadata: MetadadosAPFS? = nil
        var protectedWithoutSize: SemTamanho? = nil
    }
    /// `informational.protected_without_size`: pastas negadas que nem o dir-stats mediu.
    struct SemTamanho: Decodable, Sendable {
        struct Pasta: Decodable, Sendable { let path: String; let errno: Int32? }
        let count: Int
        let folders: [Pasta]
    }

    let generatedAt: String
    let tool: String
    /// Option no Rust: null não derruba a leitura inteira.
    let macosVersion: String?
    let home: String
    let complete: Bool
    let incompleteReasons: [String]
    let root: Bool
    let fullDiskAccess: Bool?
    let deniedFolders: Int
    let settingsScreen: Ajustes
    let dataVolume: Volume
    let top: [LinhaTop]
    let informational: Informativo?
    let stageErrors: [ErroEtapa]
    let containers: [Conteiner]?
    let swap: Swap?
    let advice: [Conselho]?
    var bootTime: String? = nil
    var cards: MapaFichas? = nil
    var adviceUnchecked: [NaoConferido]? = nil
    var notFreed: [NaoVoltou]? = nil
}

// MARK: - Fase 6: fichas e catálogo (spec-fase6 §3.4)

/// Código BCP 47 da língua em que o app está ("pt-BR", "fr", "zh-Hans"). Sem .lproj no
/// Bundle.main (testes, binário solto) fica no pt-BR, a língua-fonte.
nonisolated let linguaDoApp: String = Bundle.main.localizations.count > 1
    ? Bundle.main.preferredLocalizations.first ?? "pt-BR" : "pt-BR"

/// Texto de uma ficha: objeto língua → texto (`{"pt": …, "en": …, "fr": …}`). "pt" é o pt-BR;
/// "en" sempre vem; língua sem tradução não vem (nunca string vazia).
nonisolated struct TextoFicha: Decodable, Sendable, Equatable {
    let porLingua: [String: String]

    init(pt: String, en: String) { porLingua = ["pt": pt, "en": en] }
    init(from decoder: Decoder) throws { porLingua = try decoder.singleValueContainer().decode([String: String].self) }

    /// Na língua do app: o código exato, depois cortando subtags pela direita ("zh-Hant-HK",
    /// "zh-Hant", "zh"), depois "en". Exceções (docs/json-schema.md, "Texto traduzido"):
    /// es das Américas vai ao "es-419" (cortar daria o da Espanha), "no"/"nn" ao "nb", pt-BR ao "pt".
    var texto: String { escolher(linguaDoApp) }

    func escolher(_ lingua: String) -> String {
        var candidatos: [String] = []
        var partes = lingua.replacingOccurrences(of: "_", with: "-").split(separator: "-").map(String.init)
        while !partes.isEmpty { candidatos.append(partes.joined(separator: "-")); partes.removeLast() }
        switch candidatos.last {
        case "es" where candidatos.count > 1 && !candidatos[0].hasSuffix("-ES"):
            candidatos.insert("es-419", at: candidatos.count - 1)
        case "no", "nn": candidatos.append("nb")
        default: break
        }
        for c in candidatos + ["en", "pt"] { if let t = porLingua[c] { return t } }
        return ""
    }
}

/// Ação da ficha já validada: `kind` desconhecido (inclusive `restart`, `log_out` e `none`, que o CLI
/// ainda emite e o app não mostra), `settings` sem `target` e `command` sem
/// comando não viram ação. O alvo de `settings` ainda passa por `ajustesPermitidos` (A11).
nonisolated enum AcaoFicha: Sendable, Equatable {
    case finder, ajustes(String), appDono, comando(String)

    init?(kind: String, target: String?, command: String?) {
        switch (kind, target, command) {
        case ("finder", _, _): self = .finder
        case ("settings", let t?, _): self = .ajustes(t)
        case ("owner_app", _, _): self = .appDono
        case ("command", _, let c?): self = .comando(c)
        default: return nil
        }
    }
}

/// `refs[]`: `kind` ∈ internal, apple, report, measured.
nonisolated struct RefFicha: Decodable, Sendable {
    let kind: String
    let ref: String
}

/// `cards[id]`. Campo obrigatório ausente faz a ficha inteira ser descartada (MapaFichas).
nonisolated struct Ficha: Decodable, Sendable {
    private struct AcaoBruta: Decodable, Sendable {
        let kind: String
        let target: String?
        let command: String?
    }
    let what, owner, loses, step: TextoFicha
    /// yes, when, no, unknown.
    let selfCleans: String
    let selfCleansText: TextoFicha?
    let level: String
    private let actions: [AcaoBruta]
    let refs: [RefFicha]
    let verifiedOn: String

    var acoes: [AcaoFicha] { actions.compactMap { AcaoFicha(kind: $0.kind, target: $0.target, command: $0.command) } }
}

/// `cards` (no completo, no rápido e no catálogo): id → ficha. Ficha que não decodifica sai.
nonisolated struct MapaFichas: Decodable, Sendable {
    private struct Talvez: Decodable, Sendable {
        let ficha: Ficha?
        init(from decoder: Decoder) throws { ficha = try? Ficha(from: decoder) }
    }
    let todas: [String: Ficha]

    init(_ todas: [String: Ficha]) { self.todas = todas }

    init(from decoder: Decoder) throws {
        todas = try decoder.singleValueContainer().decode([String: Talvez].self).compactMapValues(\.ficha)
    }

    subscript(id: String?) -> Ficha? { id.flatMap { todas[$0] } }
}

/// `sources[source]` do catálogo. `checked` ∈ byte, margin, no, n/a.
nonisolated struct Conferencia: Decodable, Sendable {
    let checked: String
    let checkedOn: [String]
    let checkedAt: String?
    let appleCommand: String?
}

/// `sdtally --catalog --json` (kind `catalog`): lido uma vez por execução (Painel.catalogo).
nonisolated struct Catalogo: Decodable, Sendable {
    let cards: MapaFichas
    let sources: [String: Conferencia]
}

/// `sdtally <caminho> --json`: um nível de pastas.
nonisolated struct Ramo: Decodable, Sendable {
    struct Filho: Decodable, Sendable, Identifiable {
        let name: String
        let path: String
        let looseFiles: Bool
        let bytes: Int64
        let privateBytes: Int64?
        let protected: Bool
        let source: String
        var id: String { path + (looseFiles ? "|soltos" : "") }
    }
    struct Total: Decodable, Sendable {
        let bytes: Int64
        let lowerBound: Bool
        let privateBytes: Int64?
        let files: Int64
        let folders: Int64
    }
    let root: String
    let display: String
    let complete: Bool
    let interrupted: Bool
    let children: [Filho]
    let total: Total
    let protectedWithoutSize: Int64
    let readErrors: Int64
}

private nonisolated struct Cabecalho: Decodable {
    let schemaVersion: Int
    let kind: String?
}

/// Confere `schema_version` e `kind` antes de decodificar: versão desconhecida é recusada,
/// nunca adivinhada. O JSON completo não tem `kind`.
nonisolated func decodificar<T: Decodable>(_ tipo: T.Type, _ dados: Data, kind: String?) throws -> T {
    let d = JSONDecoder()
    d.keyDecodingStrategy = .convertFromSnakeCase
    let c: Cabecalho
    do { c = try d.decode(Cabecalho.self, from: dados) } catch { throw ErroSdtally.json(String(describing: error)) }
    guard c.schemaVersion == 1, c.kind == kind else { throw ErroSdtally.incompativel(c.schemaVersion) }
    do { return try d.decode(tipo, from: dados) } catch { throw ErroSdtally.json(String(describing: error)) }
}

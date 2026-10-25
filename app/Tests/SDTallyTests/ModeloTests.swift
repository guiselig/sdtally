// Decode das fixtures (sintéticas, home /Users/exemplo) e as contas que o app só mostra.
import Foundation
import Testing
@testable import SDTally
import SDTallyXPC

private func fixture(_ nome: String) throws -> Data {
    let url = try #require(Bundle.module.url(forResource: nome, withExtension: "json", subdirectory: "Fixtures"))
    return try Data(contentsOf: url)
}

@Test func quickFechaComATela() throws {
    let r = try decodificar(Rapido.self, fixture("quick"), kind: "quick")
    let t = try #require(r.settingsScreen.screen)
    let used = try #require(t.used), mac = try #require(r.settingsScreen.macos.bytes)
    #expect(used - mac - (t.categoriesTotal ?? 0) == t.systemData)
}

@Test func completoFechaComOUsado() throws {
    let c = try decodificar(Completo.self, fixture("completo"), kind: nil)
    let topo = c.top.reduce(Int64(0)) { $0 + $1.bytes }
    #expect(topo + (c.dataVolume.unexplained.bytes ?? 0) == c.dataVolume.used.bytes)
    #expect(c.top.contains { $0.folded == true })
    #expect(c.dataVolume.terms.first { $0.key == "large_xattrs" }?.bytes == nil)
}

@Test func ramoDecodifica() throws {
    let r = try decodificar(Ramo.self, fixture("ramo"), kind: "branch")
    #expect(r.children.contains { $0.looseFiles })
}

@Test func versaoOuTipoErradoSaoRecusados() throws {
    var j = try JSONSerialization.jsonObject(with: fixture("quick")) as! [String: Any]
    j["schema_version"] = 2
    #expect(throws: ErroSdtally.self) { try decodificar(Rapido.self, JSONSerialization.data(withJSONObject: j), kind: "quick") }
    #expect(throws: ErroSdtally.self) { try decodificar(Ramo.self, fixture("quick"), kind: "branch") }
}

@Test func formatoComoOAjustes() {
    #expect(gb(172_126_034_512) == "172,13 GB")
    #expect(gb(968_900_000) == "968,9 MB")
    #expect(absoluto("~/Music", home: "/Users/exemplo") == "/Users/exemplo/Music")
}

@Test func aspasSobrevivemAoShell() throws {
    let s = "a'b $c d ção"
    let p = Process()
    p.executableURL = URL(fileURLWithPath: "/bin/sh")
    p.arguments = ["-c", "printf %s " + Executor.aspas(s)]
    let out = Pipe()
    p.standardOutput = out
    try p.run()
    p.waitUntilExit()
    #expect(String(decoding: out.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self) == s)
}

@Test func requisitoDoHelperSoComTeamID() {
    #expect(HelperXPC.requisito(identifier: HelperXPC.idApp, time: "ABCDE12345")
        == "identifier \"io.github.guiselig.sdtally\" and anchor apple generic and certificate leaf[subject.OU] = \"ABCDE12345\"")
    // Ad hoc não tem time; texto que não é Team ID não entra no requisito.
    #expect(HelperXPC.requisito(identifier: HelperXPC.idApp, time: "") == nil)
    #expect(HelperXPC.requisito(identifier: HelperXPC.idApp, time: "AB\" or tru") == nil)
    #expect(HelperXPC.requisito(identifier: HelperXPC.idApp, time: "abcde12345") == nil)
}

/// Com o CDHash da build: só aquele sdtally; texto que não é 40 hex não entra no requisito.
@Test func requisitoComCDHash() {
    let h = String(repeating: "a1", count: 20)
    #expect(HelperXPC.requisito(identifier: HelperXPC.idCLI, time: "ABCDE12345", cdhash: h)?.hasSuffix(" and cdhash H\"\(h)\"") == true)
    #expect(HelperXPC.requisito(identifier: HelperXPC.idCLI, time: "ABCDE12345", cdhash: "a1") == nil)
    #expect(HelperXPC.requisito(identifier: HelperXPC.idCLI, time: "ABCDE12345", cdhash: String(h.dropLast(2)) + "\" o") == nil)
}

/// O shell do root pelo osascript, rodado aqui sem root com um script assinado ad hoc no papel do
/// sdtally: copia, confere a assinatura da cópia e roda a cópia; requisito que não bate ou binário
/// trocado depois de assinado não roda nada.
@Test func scriptAdminConfereACopia() throws {
    let dir = FileManager.default.temporaryDirectory.appending(path: "sdtally-admin-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: dir) }
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    let bin = dir.appending(path: "eco").path
    try Data("#!/bin/sh\necho \"$@\"\n".utf8).write(to: URL(fileURLWithPath: bin))
    try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: bin)
    func rodar(_ exe: String, _ args: [String]) throws -> (Int32, String) {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: exe)
        p.arguments = args
        let out = Pipe()
        p.standardOutput = out
        p.standardError = FileHandle.nullDevice
        try p.run()
        let d = out.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        return (p.terminationStatus, String(decoding: d, as: UTF8.self))
    }
    func admin(_ req: String?) throws -> (Int32, String) {
        try rodar("/bin/sh", ["-c", Executor.scriptAdmin(bin: bin, args: ["a'b", "$c d"], usuario: "x'y", uid: 501, requisito: req)])
    }
    #expect(try rodar("/usr/bin/codesign", ["--sign", "-", "--identifier", "io.teste.eco", bin]).0 == 0)
    #expect(try admin("identifier \"io.teste.eco\"") == (0, "a'b $c d\n"))
    #expect(try admin(nil) == (0, "a'b $c d\n"))
    #expect(try admin(HelperXPC.requisito(identifier: HelperXPC.idCLI, time: "ABCDE12345")) == (1, ""))
    // Trocado depois de assinado (o que a cópia evita entre a senha e o exec): não roda.
    try Data("#!/bin/sh\necho trocado\n".utf8).write(to: URL(fileURLWithPath: bin))
    #expect(try admin("identifier \"io.teste.eco\"") == (1, ""))
}


// MARK: - Chaves da fase 5 (containers, swap, snapshots, apagados por processo, advice)

private func macosDosVolumes(_ cs: [Conteiner]) -> Int64 {
    cs.filter { $0.internal == true }.flatMap(\.volumes).filter(\.inSettingsMacos).reduce(Int64(0)) { $0 + ($1.capacityInUse ?? 0) }
}

private func vm(_ cs: [Conteiner]) -> Int64? {
    cs.first { $0.internal == true && $0.holdsData }?.volumes.first { $0.roles == ["VM"] }?.capacityInUse
}

@Test func chavesNovasDoRapido() throws {
    let r = try decodificar(Rapido.self, fixture("quick"), kind: "quick")
    let cs = try #require(r.containers)
    #expect(macosDosVolumes(cs) == r.settingsScreen.macos.bytes)
    #expect(cs.contains { $0.isc } && cs.contains { $0.internal != true })
    let swap = try #require(r.swap?.total), v = try #require(vm(cs))
    #expect(v - swap > 0)
}

@Test func vmMenorQueOSwap() throws {
    let r = try decodificar(Rapido.self, fixture("quick-vm-negativo"), kind: "quick")
    let cs = try #require(r.containers)
    let v = try #require(vm(cs)), swap = try #require(r.swap?.total)
    #expect(v - swap < 0)
}

@Test func chavesNovasDoCompleto() throws {
    let c = try decodificar(Completo.self, fixture("completo"), kind: nil)
    let cs = try #require(c.containers)
    #expect(macosDosVolumes(cs) == c.settingsScreen.macos.bytes)
    #expect(c.swap?.total != nil)
    let a = try #require(c.advice)
    #expect(Set(a.map(\.level)) == ["measurement", "system_handles", "safe", "cost", "check_first", "do_not_touch", "anomaly"])
    #expect(a.contains { $0.privateBytes == nil } && a.contains { $0.command != nil })
    let i = try #require(c.informational)
    let s = try #require(i.snapshots)
    #expect(Set(s.map(\.kind)) == ["time_machine", "apple_other", "other"])
    #expect(s.allSatisfy { ($0.date != nil) == ($0.kind == "time_machine") })
    #expect(i.deletedOpenProcesses?.isEmpty == false)
    #expect(i.purgeable?.topServices?.count == 3)
}

@Test func donoDoTopNaoQuebraEPodeFaltar() throws {
    let c = try decodificar(Completo.self, fixture("completo"), kind: nil)
    let vm = try #require(c.top.first { $0.path == "~/Library/Application Support/Claude/vm_bundles" })
    #expect(vm.owner == DonoApp(bundleId: "com.exemplo.Claude", name: "Claude", source: "folder_app_name"))
    #expect(nomeDono(try #require(vm.owner)) == "Claude")
    // Id do container sem app instalado: o texto diz o que se sabe, não inventa nome.
    let m = try #require(c.top.first { $0.path == "~/Library/Containers/com.exemplo.Mensageiro" })
    let dono = try #require(m.owner)
    #expect(dono.name == nil && nomeDono(dono) == "Container de com.exemplo.Mensageiro")
    // Sem a chave (medição antiga ou linha sem fonte): nil, nunca texto.
    #expect(c.top.filter { $0.owner == nil }.count == c.top.count - 2)
}

@Test func injetadoTemComandoForaDaLista() throws {
    let c = try decodificar(Completo.self, fixture("completo-injetado"), kind: nil)
    #expect(c.advice?.contains { $0.command?.hasPrefix("rm ") == true } == true)
}

@Test func chavesNovasAusentesViramNil() throws {
    var j = try JSONSerialization.jsonObject(with: fixture("quick")) as! [String: Any]
    j["containers"] = nil
    j["swap"] = nil
    let r = try decodificar(Rapido.self, JSONSerialization.data(withJSONObject: j), kind: "quick")
    #expect(r.containers == nil && r.swap == nil)
    let velha = try decodificar(Completo.self, Data(contentsOf: dados.appending(path: "Medicoes/2026-09-28-1015.json")), kind: nil)
    #expect(velha.advice == nil && velha.containers == nil && velha.swap == nil)
    #expect(velha.informational?.snapshots == nil && velha.informational?.deletedOpenProcesses == nil)
}

private var dados: URL { Bundle.module.resourceURL!.appending(path: "Fixtures/dados") }

@Test func pastaDeDadosDaFixture() throws {
    let medicoes = try FileManager.default.contentsOfDirectory(at: dados.appending(path: "Medicoes"), includingPropertiesForKeys: nil)
    let cs = try medicoes.map { try decodificar(Completo.self, Data(contentsOf: $0), kind: nil) }
    #expect(cs.count == 3 && Set(cs.map(\.tool)).count == 2)
    let linhas = try String(contentsOf: dados.appending(path: "pontos.jsonl"), encoding: .utf8).split(separator: "\n")
    #expect(linhas.count >= 7 * 24)
    #expect(try linhas.allSatisfy { (try JSONSerialization.jsonObject(with: Data($0.utf8)) as? [String: Any])?["t"] is String })
}

@Test func pastaDeLeitura() {
    #expect(Historico.pastaLeitura(env: ["SDTALLY_DADOS": "/tmp/x"], bundleID: nil)?.path == "/tmp/x")
    #expect(Historico.pastaLeitura(env: [:], bundleID: nil) == nil)
    // Dentro do .app, SDTALLY_DADOS não vale.
    #expect(Historico.pastaLeitura(env: ["SDTALLY_DADOS": "/tmp/x"], bundleID: "io.github.guiselig.sdtally")?.path.hasSuffix("Library/Application Support/SDTally") == true)
}

@Test func nivelConhecidoEDesconhecido() {
    #expect(Marca.nivel("safe").simbolo == "leaf")
    #expect(Marca.nivel("inventado").simbolo == Marca.nivel("measurement").simbolo)
}

// O Rust publica internal: null quando o diskutil não diz o tipo do disco; o resto do JSON tem de ler.
@Test func internalNuloNaoDerrubaODecode() throws {
    var j = try JSONSerialization.jsonObject(with: fixture("quick")) as! [String: Any]
    var cs = j["containers"] as! [[String: Any]]
    cs[0]["internal"] = NSNull()
    j["containers"] = cs
    let r = try decodificar(Rapido.self, try JSONSerialization.data(withJSONObject: j), kind: "quick")
    #expect(r.containers?.first?.internal == nil)
}

// MARK: - Fase 6 (spec-fase6 §3, §4.8)

@Test func completoAssinaturasDecodifica() throws {
    let c = try decodificar(Completo.self, fixture("completo-assinaturas"), kind: nil)
    let a = try #require(c.advice)
    let ids = Set(a.map(\.id))
    for id in ["asset_voices", "asset_xcode", "asset_sim_runtime", "asset_os_update", "spotlight_index_large",
               "failed_smb_mount", "restart_worth", "tm_backup_stuck", "chronod_datavault"] {
        #expect(ids.contains(id), "\(id)")
    }
    // Suprimidas: tm_backup_stuck tira time_machine_snapshots; restart_worth tira swap_large e deleted_open.
    #expect(ids.isDisjoint(with: ["time_machine_snapshots", "swap_large", "deleted_open"]))
    #expect(a.allSatisfy { $0.card == $0.id && ($0.kind == "signature") == ($0.evidence != nil) })
    #expect(a.first { $0.id == "chronod_datavault" }?.bytesLogical == true)
    #expect(a.filter { $0.bytesLogical == true }.count == 1)
    let fichas = try #require(c.cards)
    for id in Set(a.compactMap(\.card)).union(c.top.compactMap(\.card)) { #expect(fichas[id] != nil, "\(id)") }
    #expect(c.adviceUnchecked?.map(\.reason) == ["tm status unreadable", "no lsof"])
    let i = try #require(c.informational)
    #expect(i.timeMachine?.running == false && i.timeMachine?.destinations?.first?.attemptsSinceCompleted == 6)
    #expect(i.snapshots?.first { $0.tmRole == "reference" }?.verdict == "tm_reference")
    let chronod = try #require(i.deletedOpenProcesses?.first)
    #expect(chronod.name == "chronod" && chronod.bytes == nil && chronod.launchservicesDvFiles == 247)
    #expect(c.bootTime == "2026-09-28 08:15")
}

@Test func completoVaziosDecodifica() throws {
    let c = try decodificar(Completo.self, fixture("completo-vazios"), kind: nil)
    let i = try #require(c.informational)
    #expect(i.snapshots?.isEmpty == true && i.deletedOpenProcesses?.isEmpty == true)
    #expect(i.timeMachine?.state == "not_measured" && i.timeMachine?.destinations == nil && i.timeMachine?.running == nil)
    #expect(c.advice?.isEmpty == true && c.adviceUnchecked?.isEmpty == true)
    #expect(c.bootTime == nil && c.containers == nil && c.swap?.total == 0)
    #expect(c.settingsScreen.screen?.state == "not_measured")
    let topo = c.top.reduce(Int64(0)) { $0 + $1.bytes }
    #expect(topo + (c.dataVolume.unexplained.bytes ?? 0) == c.dataVolume.used.bytes)
}

@Test func cardsNasLinhasDoCompletoEDoRapido() throws {
    let c = try decodificar(Completo.self, fixture("completo"), kind: nil)
    #expect(c.advice?.allSatisfy { $0.kind == "rule" && $0.card == $0.id } == true)
    #expect(c.informational?.purgeable?.topServices?.first?.card == "svc_time_machine")
    #expect(c.dataVolume.terms.first { $0.key == "snapshots" }?.card == "term_snapshots")
    #expect(c.containers?.first?.volumes.first?.card == "vol_system")
    #expect(c.informational?.snapshots?.allSatisfy { $0.creator != nil && $0.verdict != nil } == true)
    let r = try decodificar(Rapido.self, fixture("quick"), kind: "quick")
    #expect(r.bootTime != nil && r.cards?["svc_time_machine"] != nil)
}

@Test func catalogoDecodifica() throws {
    let cat = try decodificar(Catalogo.self, fixture("catalogo"), kind: "catalog")
    #expect(cat.cards.todas.count >= 50)
    // Todo apple_command do catálogo está na lista fixa do app (igual à do Rust).
    #expect(cat.sources.values.compactMap(\.appleCommand).allSatisfy(comandosVerificar.contains))
    let alvos = Set(cat.cards.todas.values.flatMap(\.acoes).compactMap { if case .ajustes(let t) = $0 { t } else { nil } })
    #expect(alvos == ["storage", "time_machine", "spotlight"])
    #expect(cat.cards["time_machine_snapshots"]?.acoes == [.comando("tmutil listlocalsnapshots /")])
    #expect(throws: ErroSdtally.self) { try decodificar(Catalogo.self, fixture("catalogo"), kind: nil) }
}

@Test func acaoDesconhecidaIgnoradaEFichaIncompletaDescartada() throws {
    var j = try JSONSerialization.jsonObject(with: fixture("catalogo")) as! [String: Any]
    var cards = j["cards"] as! [String: Any]
    var f = cards["trash"] as! [String: Any]
    f["actions"] = [["kind": "teleport", "target": NSNull(), "command": NSNull()],
                    ["kind": "settings", "target": NSNull(), "command": NSNull()],
                    ["kind": "finder", "target": NSNull(), "command": NSNull()]]
    cards["trash"] = f
    var g = cards["logs"] as! [String: Any]
    g["what"] = nil
    cards["logs"] = g
    j["cards"] = cards
    let cat = try decodificar(Catalogo.self, JSONSerialization.data(withJSONObject: j), kind: "catalog")
    #expect(cat.cards["trash"]?.acoes == [.finder])
    #expect(cat.cards["logs"] == nil && cat.cards["homebrew_cache"] != nil)
}

/// O CLI ainda emite `restart`, `log_out` e `none`: a ficha decodifica inteira, sem essas ações.
@Test func acoesRemovidasDoAppNaoQuebramADecodificacao() throws {
    let base = try JSONSerialization.jsonObject(with: fixture("catalogo")) as! [String: Any]
    let original = try decodificar(Catalogo.self, JSONSerialization.data(withJSONObject: base), kind: "catalog").cards["trash"]
    for kind in ["restart", "log_out", "none"] {
        var j = base
        var cards = j["cards"] as! [String: Any]
        var f = cards["trash"] as! [String: Any]
        f["actions"] = [["kind": kind, "target": NSNull(), "command": NSNull()]]
        cards["trash"] = f
        j["cards"] = cards
        let ficha = try #require(decodificar(Catalogo.self, JSONSerialization.data(withJSONObject: j), kind: "catalog").cards["trash"])
        #expect(ficha.acoes.isEmpty, "\(kind)")
        #expect(ficha.owner.texto == original?.owner.texto && ficha.step.texto == original?.step.texto && ficha.level == original?.level, "\(kind)")
    }
}

@Test func arquivoAntigoSemChavesDaFase6() throws {
    let velha = try decodificar(Completo.self, Data(contentsOf: dados.appending(path: "Medicoes/2026-09-28-1015.json")), kind: nil)
    #expect(velha.cards == nil && velha.bootTime == nil && velha.adviceUnchecked == nil)
}

@Test func visivelEscapaInvisiveis() {
    #expect(visivel("Ignore\u{202E}x\u{E0041}") == "Ignore\\u{202E}x\\u{E0041}")
    #expect(visivel("a\u{0}b\u{200B}\u{2028}\u{061C}\u{FEFF}") == "a\\u{0000}b\\u{200B}\\u{2028}\\u{061C}\\u{FEFF}")
    #expect(visivel("~/Música çã 😀") == "~/Música çã 😀")
}

@Test func nivelDaBarraPeloAmbiente() {
    #expect(Foto.nivelBarra("ajustes") == .ajustes && Foto.nivelBarra("volumeData") == .volumeData)
    #expect(Foto.nivelBarra("termo:snapshots") == .termo("snapshots"))
    #expect(Foto.nivelBarra("termo:") == nil && Foto.nivelBarra("x") == nil && Foto.nivelBarra(nil) == nil)
}

/// Medição sem Acesso Total (13/10): Σ termos passou do usado e o resto saiu em -14,68 GB.
/// Nenhum caminho mostra negativo: o sdtally novo manda null + overlap_bytes, e a medição salva por
/// um antigo (bytes negativo) é lida como não medida.
@MainActor @Test func restoNegativoNuncaChegaATela() throws {
    func completo(_ unexplained: [String: Any], acessoTotal: Bool = false) throws -> Completo {
        var j = try JSONSerialization.jsonObject(with: fixture("completo")) as! [String: Any]
        var dv = j["data_volume"] as! [String: Any]
        dv["unexplained"] = unexplained
        j["data_volume"] = dv
        j["root"] = true
        j["full_disk_access"] = acessoTotal
        return try decodificar(Completo.self, JSONSerialization.data(withJSONObject: j), kind: nil)
    }
    let novo = try completo(["bytes": NSNull(), "percent_of_used": NSNull(), "state": "not_measured", "overlap_bytes": 14_680_000_000, "reason": "x", "source": "difference"])
    let antigo = try completo(["bytes": -14_680_000_000, "percent_of_used": -3.3, "state": "partial", "source": "difference"])
    for c in [novo, antigo] {
        let u = c.dataVolume.unexplained
        #expect(u.bytes == nil && u.percentOfUsed == nil && u.state == "not_measured" && u.overlapBytes == 14_680_000_000)
        let f = fraseSemExplicacao(c)
        #expect(f == "Bloqueado pelo macOS: não medido")
        #expect(explicarBloqueado(c).texto.contains { $0.contains("passam do usado em 14,68 GB") })
        #expect(!f.contains("-14") && !f.contains("−"))
        #expect(Vazio.v12(c, comSenha: nil).texto == "Bloqueado pelo macOS: não medido, sem senha. Medir com senha pode reduzir isto.")
    }
    // Resto normal não muda.
    let ok = try completo(["bytes": 6_000_000_000, "percent_of_used": 1.5, "state": "partial", "source": "difference"])
    #expect(fraseSemExplicacao(ok) == "Bloqueado pelo macOS: 6,00 GB (1,5 % do usado)")
    // Com root e Acesso Total nenhuma permissão reduz o resto: "Não medido".
    let tudo = try completo(["bytes": 6_000_000_000, "percent_of_used": 1.5, "state": "partial", "source": "difference"], acessoTotal: true)
    #expect(!restoReduzivel(tudo) && restoReduzivel(ok))
    #expect(nomeResto(tudo) == "Não medido" && nomeResto(ok) == "Bloqueado pelo macOS")
    #expect(fraseSemExplicacao(tudo) == "Não medido: 6,00 GB (1,5 % do usado)")
}

@Test func lixoPorIdadeEMetadadosDoAPFS() throws {
    let c = try decodificar(Completo.self, fixture("completo"), kind: nil)
    let l = try #require(c.informational?.trash)
    #expect(l.thresholdDays == 30 && l.older.items == 3 && l.older.bytes == 1_230_000_000 && l.total.items == 12)
    #expect(l.older.source == "getattrlist_addedtime" && l.undated?.items == 0)
    let m = try #require(c.informational?.apfsMetadata)
    #expect(m.bytes == 1_654_865_920 && m.blockSize == 4096 && m.nodes?.fsroot == 391_006 && m.source == "apfs_user_client")
    // Sem os dois campos (medição de um sdtally antigo) o decode segue.
    var j = try JSONSerialization.jsonObject(with: fixture("completo")) as! [String: Any]
    var i = j["informational"] as! [String: Any]
    i["trash"] = nil
    i["apfs_metadata"] = nil
    j["informational"] = i
    let antigo = try decodificar(Completo.self, JSONSerialization.data(withJSONObject: j), kind: nil)
    #expect(antigo.informational?.trash == nil && antigo.informational?.apfsMetadata == nil)
}

@Test func textoDoLixoSoTemOsNumeros() throws {
    let c = try decodificar(Completo.self, fixture("completo"), kind: nil)
    let lixo = try #require(c.informational?.trash)
    let t = try #require(textoLixo(lixo))
    #expect(t.contains("3 de 12 itens") && t.contains("4,56 GB") && !t.contains("modificação"))
    // Não medido: sem número inventado no texto.
    let nm = Completo.Lixo(thresholdDays: 30, total: .init(bytes: nil, state: "not_measured", reason: "x", source: "getattrlist_addedtime", items: nil),
                           older: .init(bytes: nil, state: "not_measured", reason: "x", source: "getattrlist_addedtime", items: nil), undated: nil, source: "getattrlist_addedtime")
    #expect(textoLixo(nm) == nil)
}

@Test func explicacaoDoBloqueado() throws {
    var j = try #require(JSONSerialization.jsonObject(with: fixture("completo")) as? [String: Any])
    j["root"] = true
    var v = try #require(j["data_volume"] as? [String: Any])
    var termos = try #require(v["terms"] as? [[String: Any]])
    termos[1]["state"] = "partial"; termos[1]["reason"] = "65 objects with unreadable xattr"
    v["terms"] = termos; j["data_volume"] = v
    var inf = try #require(j["informational"] as? [String: Any])
    var procs = try #require(inf["deleted_open_processes"] as? [[String: Any]])
    procs[0]["files_without_allocated"] = 418
    inf["deleted_open_processes"] = procs
    inf["protected_without_size"] = ["count": 2, "folders": [["path": "/private/var/db/x", "errno": 1], ["path": "/Library/y", "errno": NSNull()]]]
    j["informational"] = inf
    let c = try decodificar(Completo.self, JSONSerialization.data(withJSONObject: j), kind: nil)
    let e = explicarBloqueado(c)
    #expect(e.texto.first?.contains("nem o administrador") == true)
    #expect(e.texto.contains("2 pastas protegidas sem tamanho: nem o APFS informou."))
    #expect(e.texto.contains("418 arquivos apagados ainda abertos sem tamanho legível."))
    #expect(e.texto.contains("65 objetos com atributos estendidos ilegíveis."))
    #expect(e.pastas == ["/private/var/db/x", "/Library/y"])
    // Medição antiga, sem o campo: nada inventado.
    let antigo = explicarBloqueado(try decodificar(Completo.self, fixture("completo"), kind: nil))
    #expect(antigo.pastas.isEmpty && !antigo.texto.contains { $0.contains("pastas protegidas sem tamanho") })
}

/// A linha do `sdtally --auto` é o relatório completo com `kind` "auto" e o bloco `auto`; o
/// histórico (kind nil) não a aceita como medição completa.
@Test func linhaAutoDecodifica() throws {
    var v = try #require(JSONSerialization.jsonObject(with: fixture("completo")) as? [String: Any])
    v["kind"] = "auto"
    v["auto"] = ["last_pass_seconds": 40.5, "passes": 1, "events": 10, "rereads": 3, "folders_in_memory": 458_000, "other_sources_age_seconds": 12]
    let d = try JSONSerialization.data(withJSONObject: v)
    let c = try decodificar(Completo.self, d, kind: "auto")
    #expect(c.top.isEmpty == false)
    #expect((try? decodificar(Completo.self, d, kind: nil)) == nil)
}

/// Uma amostra por hora, sem as linhas dobradas; gravar e ler devolvem o mesmo; 90 dias.
@Test func pontosPastasDoAutomatico() throws {
    var v = try #require(JSONSerialization.jsonObject(with: fixture("completo")) as? [String: Any])
    v["kind"] = "auto"
    v["generated_at"] = "2026-10-21 10:00:00"
    let c = try decodificar(Completo.self, try JSONSerialization.data(withJSONObject: v), kind: "auto")
    let p = try #require(Historico.proximoPontoPastas(c, depois: []))
    let esperadas = c.top.filter { $0.folded != true && $0.path != nil }
    #expect(!p.pastas.isEmpty && p.pastas.count <= esperadas.count)
    #expect(Historico.proximoPontoPastas(c, depois: [p]) == nil)
    let depois = PontoPastas(t: carimboComFuso("2026-10-21 11:00:00"), pastas: p.pastas)
    v["generated_at"] = "2026-10-21 11:00:00"
    let c2 = try decodificar(Completo.self, try JSONSerialization.data(withJSONObject: v), kind: "auto")
    #expect(Historico.proximoPontoPastas(c2, depois: [p]) == depois)
    let dir = FileManager.default.temporaryDirectory.appending(path: "sdtally-pastas-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: dir) }
    try Historico.gravarPontosPastas([p, depois], em: dir)
    #expect(Historico.lerPontosPastas(dir) == [p, depois])
    let velho = PontoPastas(t: "2026-01-01 00:00:00", pastas: [:])
    #expect(Historico.recentes([velho, depois], agora: data("2026-10-21 12:00:00")!) == [depois])
}

/// macos_version é Option no Rust: null não derruba a leitura.
@Test func macosVersionNulo() throws {
    for (nome, kind) in [("quick", "quick"), ("completo", nil)] as [(String, String?)] {
        var v = try #require(JSONSerialization.jsonObject(with: fixture(nome)) as? [String: Any])
        v["macos_version"] = NSNull()
        let d = try JSONSerialization.data(withJSONObject: v)
        if kind == nil { #expect(try decodificar(Completo.self, d, kind: nil).macosVersion == nil) }
        else { #expect(try decodificar(Rapido.self, d, kind: "quick").macosVersion == nil) }
    }
}

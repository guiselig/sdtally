// A6: barra de menus e avisos. Funções puras, mais o laço único do Painel.
import Foundation
import Testing
@testable import SDTally

private let agora = data("2026-10-03 12:00:00")!
private let gb10: Int64 = 10_000_000_000

/// Ponto `horas` antes de `agora`, no formato do pontos.jsonl.
private func ponto(_ horas: Double, usado: Int64, purgeable: Int64 = 0, livre: Int64 = 400_000_000_000,
                   total: Int64 = 1_000_000_000_000, systemData: Int64? = nil, telaEm: String? = nil) -> Ponto {
    let f = DateFormatter()
    f.locale = Locale(identifier: "en_US_POSIX")
    f.dateFormat = "yyyy-MM-dd HH:mm:ss"
    return Ponto(t: f.string(from: agora.addingTimeInterval(-horas * 3600)), total: total, usado: usado, livre: livre,
                 purgeable: purgeable, systemData: systemData, telaEm: telaEm)
}

@MainActor @Test func vigiarUmaVez() async {
    let p = Painel()
    var n = 0
    p.iniciar { n += 1 }
    let t = p.tarefaVigia
    p.iniciar { n += 1 }
    #expect(t != nil && p.tarefaVigia == t)
    await t?.value
    #expect(n == 1)
}

@Test func intervaloDoRapido() {
    #expect(Painel.intervalo(.tela, aguardando: false) == .infinity)
    #expect(Painel.intervalo(.tela, aguardando: false) == .infinity)
    #expect(Painel.intervalo(.tela, aguardando: true) == 5)
    #expect(Painel.intervalo(.volumes, aguardando: true) == .infinity)
    #expect(Painel.intervalo(.purgeable, aguardando: false) == .infinity)
    #expect(Painel.intervalo(.uso, aguardando: false) == .infinity)
    #expect(Painel.intervalo(.swap, aguardando: false) == .infinity)
    #expect(Painel.intervalo(.meta, aguardando: true) == .infinity)
}

/// Cada parte no seu ritmo: nunca lida vence (menos Acesso Total, uso, swap e purgeable, que só o
/// atualizar() lê; os três últimos vêm do laço do app).
@Test func partesVencidas() {
    #expect(Painel.vencidas([:], aguardando: false, agora: agora).isEmpty)
    let lidas = Dictionary(uniqueKeysWithValues: ParteRapido.allCases.map { ($0, agora.addingTimeInterval(-61)) })
    #expect(Painel.vencidas(lidas, aguardando: false, agora: agora).isEmpty)
    #expect(Painel.vencidas(lidas, aguardando: true, agora: agora) == [.tela])
    #expect(Painel.vencidas(lidas, aguardando: false, agora: agora).isEmpty)
    // Voltar ao app e acordar não leem a tela (sem --since leria a hora inteira do log).
    #expect(!Painel.semTela.contains(.tela) && Painel.semTela.count == ParteRapido.allCases.count - 1)
}

/// O JSON parcial substitui só as chaves que trouxe; em settings_screen, uma por uma.
@Test func juntarRapidoParcial() throws {
    let velho = Data(#"{"a":1,"b":2,"settings_screen":{"macos":10,"screen":20}}"#.utf8)
    let novo = Data(#"{"b":3,"settings_screen":{"screen":21}}"#.utf8)
    let j = try JSONSerialization.jsonObject(with: Painel.juntar(velho, novo)) as? [String: Any]
    #expect(j?["a"] as? Int == 1 && j?["b"] as? Int == 3)
    let ss = j?["settings_screen"] as? [String: Any]
    #expect(ss?["macos"] as? Int == 10 && ss?["screen"] as? Int == 21)
    #expect(try Painel.juntar(nil, novo) == novo)
}

/// `--since` só com leitura anterior recente (< 120 s): senão o sdtally lê a hora inteira do log.
@Test func argsDoRapidoIncremental() {
    let sem = ["--quick", "--json"]
    #expect(Painel.argsRapido(desde: nil, agora: agora) == sem)
    #expect(Painel.argsRapido(desde: agora.addingTimeInterval(-30), agora: agora) == sem + ["--since", String(Int(agora.timeIntervalSince1970) - 30)])
    #expect(Painel.argsRapido(desde: agora.addingTimeInterval(-119), agora: agora).count == 4)
    // Voltou de segundo plano (30 min) ou relógio andou para trás: hora inteira.
    #expect(Painel.argsRapido(desde: agora.addingTimeInterval(-1800), agora: agora) == sem)
    #expect(Painel.argsRapido(desde: agora.addingTimeInterval(5), agora: agora) == sem)
    // --parts na ordem do enum; com todas, sem a opção.
    #expect(Painel.argsRapido(desde: nil, agora: agora, partes: [.purgeable, .uso]) == sem + ["--parts", "uso,purgeable"])
    #expect(Painel.argsRapido(desde: nil, agora: agora, partes: Set(ParteRapido.allCases)) == sem)
}

@Test func avisoComHisterese() throws {
    let pts = [ponto(24, usado: 500_000_000_000), ponto(0, usado: 500_000_000_000 + gb10)]
    let a = try #require(aviso(pontos: pts, agora: agora, ultimo: [:]))
    #expect(a.tipo == .crescimento && a.bytes == gb10)
    #expect(a.desde == data(pts[0].t))  // a hora real do ponto
    #expect(aviso(pontos: pts, agora: agora, ultimo: [.crescimento: agora.addingTimeInterval(-3600)]) == nil)
    #expect(aviso(pontos: pts, agora: agora, ultimo: [.crescimento: agora.addingTimeInterval(-25 * 3600)]) == a)
    // A histerese é por tipo: um pouco espaço recente não segura o crescimento.
    #expect(aviso(pontos: pts, agora: agora, ultimo: [.poucoEspaco: agora]) == a)
    #expect(a.textos(agora: agora).titulo == "O disco usou mais 10,0 GB desde ontem às 12:00.")
}

@Test func semPontoNaJanelaNaoAvisa() {
    let atual = ponto(0, usado: 600_000_000_000)
    #expect(aviso(pontos: [ponto(19, usado: 500_000_000_000), atual], agora: agora, ultimo: [:]) == nil)
    #expect(aviso(pontos: [ponto(29, usado: 500_000_000_000), atual], agora: agora, ultimo: [:]) == nil)
    #expect(aviso(pontos: [atual], agora: agora, ultimo: [:]) == nil)
    // Dois na janela: vale o mais perto de 24 h.
    let a = aviso(pontos: [ponto(27, usado: 500_000_000_000), ponto(23, usado: 595_000_000_000), atual], agora: agora, ultimo: [:])
    #expect(a == nil)  // contra o de 23 h, só 5 GB
}

@Test func poucoEspacoContaPurgeable() throws {
    let total: Int64 = 1_000_000_000_000
    // 5 % livre + 10 % liberável = 15 %: sem aviso.
    #expect(aviso(pontos: [ponto(0, usado: 1, purgeable: 100_000_000_000, livre: 50_000_000_000, total: total)], agora: agora, ultimo: [:]) == nil)
    let a = try #require(aviso(pontos: [ponto(0, usado: 1, purgeable: 30_000_000_000, livre: 50_000_000_000, total: total)], agora: agora, ultimo: [:]))
    #expect(a.tipo == .poucoEspaco && a.bytes == 80_000_000_000)
    #expect(a.textos().titulo == "Restam 8 % do disco livres, contando o que o macOS libera sozinho.")
}

@Test func crescimentoDescontaPurgeable() {
    // Usado cresceu 12 GB, mas 5 GB disso o macOS libera sozinho: 7 GB, sem aviso.
    let pts = [ponto(24, usado: 500_000_000_000, purgeable: 20_000_000_000),
               ponto(0, usado: 512_000_000_000, purgeable: 25_000_000_000)]
    #expect(aviso(pontos: pts, agora: agora, ultimo: [:]) == nil)
    // Liberável caiu 3 GB: 15 GB contam.
    let pts2 = [ponto(24, usado: 500_000_000_000, purgeable: 20_000_000_000),
                ponto(0, usado: 512_000_000_000, purgeable: 17_000_000_000)]
    #expect(aviso(pontos: pts2, agora: agora, ultimo: [:])?.bytes == 15_000_000_000)
}

@Test func sistemaDoUltimoPonto() throws {
    let pts = [ponto(30, usado: 1, systemData: 100_000_000_000, telaEm: "2026-10-02 06:00:00"),
               ponto(5, usado: 1, systemData: 145_990_000_000, telaEm: "2026-10-03 07:00:00"),
               ponto(0, usado: 1)]
    let s = try #require(sistemaBarra(tela: nil, pontos: pts))
    #expect(s.bytes == 145_990_000_000 && s.em == "2026-10-03 07:00:00")
    #expect(sistemaBarra(tela: nil, pontos: [ponto(0, usado: 1)]) == nil)
    let dec = JSONDecoder()
    dec.keyDecodingStrategy = .convertFromSnakeCase
    let tela = try dec.decode(Tela.self, from: Data(#"{"state":"measured","logged_at":"2026-10-03 11:00:00.1-0300","system_data":150000000000}"#.utf8))
    #expect(sistemaBarra(tela: tela, pontos: pts)?.bytes == 150_000_000_000)
}

/// Ajustes oculto: só com o app ativo, boas-vindas vistas, sem foto, sem poupar e tela velha (> 600 s).
@MainActor @Test func releituraDoAjustesOculto() {
    let velha = agora.addingTimeInterval(-601), nova = agora.addingTimeInterval(-60)
    #expect(Painel.releAjustes(tela: velha, ativo: true, poupar: false, agora: agora, viu: true, foto: false))
    #expect(Painel.releAjustes(tela: nil, ativo: true, poupar: false, agora: agora, viu: true, foto: false))
    #expect(!Painel.releAjustes(tela: nova, ativo: true, poupar: false, agora: agora, viu: true, foto: false))
    #expect(!Painel.releAjustes(tela: velha, ativo: false, poupar: false, agora: agora, viu: true, foto: false))
    #expect(!Painel.releAjustes(tela: velha, ativo: true, poupar: true, agora: agora, viu: true, foto: false))
    #expect(!Painel.releAjustes(tela: velha, ativo: true, poupar: false, agora: agora, viu: false, foto: false))
    #expect(!Painel.releAjustes(tela: velha, ativo: true, poupar: false, agora: agora, viu: true, foto: true))
}

/// O livre de 1 s sai do statfs do volume de dados, como o `capacity.free` do sdtally.
@Test func livreDoDiscoPeloStatfs() {
    let l = Painel.livreDoDisco()
    #expect(l != nil && l! > 0)
    #expect(Painel.livreDoDisco("/caminho/que/nao/existe") == nil)
}

@Test func tempoRealPublicaSoQuandoATelaMuda() {
    #expect(Painel.mudou(45_000_000_000, nil))
    #expect(!Painel.mudou(nil, 45_000_000_000))
    #expect(!Painel.mudou(45_000_000_000, 45_009_999_999), "mesmo centésimo de GB")
    #expect(Painel.mudou(45_010_000_000, 45_009_999_999))
    let u = Painel.usoDoVolume(), w = Painel.swapAgora()
    #expect((u ?? 0) > 0)
    #expect(w?.total != nil && (w?.used ?? -1) >= 0)
}

@Test func purgeavelNoApp() {
    // Mesma regra do sdtally: CACHE_DELETE_* fora, soma dos serviços, ficha pelo nome.
    let d: [String: Any] = ["CACHE_DELETE_TOTAL_AVAILABLE": NSNumber(value: 99), "com.apple.TMCacheDelete": NSNumber(value: 9_000), "com.apple.x": NSNumber(value: 0), "com.apple.y": NSNumber(value: 500)]
    let p = Painel.purgavel(d, fichas: ["com.apple.TMCacheDelete": "svc_time_machine"])
    #expect(p?.bytes == 9_500)
    #expect(p?.topServices?.map(\.name) == ["com.apple.TMCacheDelete", "com.apple.y"])
    #expect(p?.topServices?.first?.card == "svc_time_machine" && p?.topServices?.last?.card == nil)
    #expect(Painel.purgavel(["CACHE_DELETE_TOTAL_AVAILABLE": NSNumber(value: 1)], fichas: [:]) == nil, "sem serviço não é zero")
    // A chamada real responde com serviços (CacheDelete do macOS).
    #expect((Painel.purgeavelAgora(fichas: [:])?.bytes ?? -1) >= 0)
}

@Test func ritmosEditaveis() throws {
    let d = try #require(UserDefaults(suiteName: "sdtally.teste.ritmos"))
    d.removePersistentDomain(forName: "sdtally.teste.ritmos")
    #expect(Ritmo.valor(.tempoReal, d) == 1 && Ritmo.valor(.purgAtivo, d) == 10 && Ritmo.valor(.ajustesEscondido, d) == 600)
    #expect(Ritmo.valor(.ajustesFundo, d) == 0, "Ajustes escondido em segundo plano: desligado por padrão")
    d.set(5.0, forKey: Ritmo.tempoReal.chave)
    #expect(Ritmo.valor(.tempoReal, d) == 5)
    d.set(7.0, forKey: Ritmo.tempoReal.chave)
    #expect(Ritmo.valor(.tempoReal, d) == 1, "fora das opções volta ao padrão")
    #expect(Ritmo.allCases.allSatisfy { $0.opcoes.contains($0.padrao) })
    #expect(Ritmo.rotulo(0) == "Desligado" && Ritmo.rotulo(10) == "10 s" && Ritmo.rotulo(600) == "10 min" && Ritmo.rotulo(3600) == "1 h")
    d.removePersistentDomain(forName: "sdtally.teste.ritmos")
}

/// Tela do `sdtally --watch`: entra no rápido; repetida com menos de 10 MB de diferença e há menos
/// de 1 min, não redesenha; andou 10 MB, entra.
@MainActor @Test func escutaDaTela() throws {
    let url = try #require(Bundle.module.url(forResource: "quick", withExtension: "json", subdirectory: "Fixtures"))
    let p = Painel()
    p.jsonRapido = try Data(contentsOf: url)
    let f = DateFormatter()
    f.dateFormat = "yyyy-MM-dd HH:mm:ss.SSSSSSZ"
    let linha = { (used: Int64, s: Double) in
        Data(#"{"schema_version":1,"kind":"quick","settings_screen":{"macos":{"bytes":1,"reason":null,"source":"diskutil_capacity_in_use","state":"measured"},"screen":{"state":"measured","logged_at":"\#(f.string(from: .now.addingTimeInterval(s)))","used":\#(used),"available":1,"capacity":2,"system_data":\#(used / 2)}}}"#.utf8)
    }
    p.aplicarEscuta(linha(400_000_000_000, 0))
    #expect(p.telaBoa?.tela.used == 400_000_000_000 && p.rapido?.settingsScreen.screen?.used == 400_000_000_000)
    let primeira = p.telaBoa?.tela.loggedAt
    p.aplicarEscuta(linha(400_005_000_000, 5))
    #expect(p.telaBoa?.tela.loggedAt == primeira, "5 MB: não redesenha")
    p.aplicarEscuta(linha(400_020_000_000, 10))
    #expect(p.telaBoa?.tela.used == 400_020_000_000)
    p.aplicarEscuta(Data("não é json".utf8))
    #expect(p.telaBoa?.tela.used == 400_020_000_000)
}

/// Volumes montados pelo getattrlist trocam o usado, o livre do container e o macOS da tela pela
/// diferença; desmontados (sem leitura) ficam com o do diskutil.
@Test func volumesVivos() throws {
    let url = try #require(Bundle.module.url(forResource: "quick", withExtension: "json", subdirectory: "Fixtures"))
    var r = try decodificar(Rapido.self, Data(contentsOf: url), kind: "quick")
    var cs = try #require(r.containers)
    let i = try #require(cs.firstIndex { $0.holdsData })
    let j = try #require(cs[i].volumes.firstIndex { $0.inSettingsMacos && $0.capacityInUse != nil })
    let k = try #require(cs[i].volumes.firstIndex { !$0.inSettingsMacos && $0.capacityInUse != nil })
    cs[i].volumes[j].device = "disk9s1"
    cs[i].volumes[k].device = "disk9s5"
    r.containers = cs
    let (v1, v2, livre, macos) = (cs[i].volumes[j].capacityInUse!, cs[i].volumes[k].capacityInUse!, cs[i].free, r.settingsScreen.macos.bytes)
    Painel.aplicar(&r, uso: nil, swap: nil, purg: nil, volumes: ["disk9s1": v1 + 100, "disk9s5": v2 + 1_000, "disk1s1": 5])
    let n = try #require(r.containers)[i]
    #expect(n.volumes[j].capacityInUse == v1 + 100 && n.volumes[k].capacityInUse == v2 + 1_000)
    #expect(n.free == livre.map { $0 - 1_100 })
    #expect(r.settingsScreen.macos.bytes == macos.map { $0 + 100 })
    #expect(Painel.mudouVolumes(["a": 1], nil) && Painel.mudouVolumes(["a": 1, "b": 2], ["a": 1]))
    #expect(!Painel.mudouVolumes(["a": 5_000_000], ["a": 1]) && Painel.mudouVolumes(["a": 20_000_000], ["a": 1]))
    // Os montados deste Mac: o Data aparece com o mesmo usado do getattrlist direto.
    let m = try #require(Painel.volumesMontados())
    #expect(m.values.contains { abs($0 - (Painel.usoDoVolume() ?? 0)) < 100_000_000 })
}

/// Cartão Livre ao vivo: Livre + Purgeable é a soma dos dois do app; sem purgeable, não medido.
@MainActor @Test func livresAoVivo() {
    let p = Painel()
    #expect(p.livresVivos == nil)
    p.livre = 1_000
    #expect(p.livresVivos?.livre == 1_000 && p.livresVivos?.importante == nil)
    p.purgAgora = Painel.purgavel(["com.apple.x": NSNumber(value: 500)], fichas: [:])
    #expect(p.livresVivos?.importante == 1_500)
    #expect((Painel.lerOpcional() ?? -1) >= 0)
}

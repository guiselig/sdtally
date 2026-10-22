// Histórico de medições (spec-fase5 §4.1). Tipos e funções puras são nonisolated.
// Pasta: Medicoes/AAAA-MM-DD-HHMM(-n).json com o stdout cru de cada medição completa (o modo vai num
// xattr, para o arquivo continuar idêntico ao stdout) e pontos.jsonl com um Ponto por linha.
// Fixture: Tests/SDTallyTests/Fixtures/dados.
import Foundation

/// Como a medição completa rodou. O JSON só diz `root`; helper ou senha fica no xattr do arquivo.
nonisolated enum ModoMedicao: String, Sendable {
    case helper, senha, semRoot = "sem_root"

    static let xattr = "io.github.guiselig.sdtally.modo"

    /// Nome no selo e em Permissões. Sem modo salvo (medição antiga ou de arquivo), só o `root`.
    static func nome(_ m: ModoMedicao?, root: Bool) -> String {
        switch m {
        case .helper: String(localized: "Root sem Senha")
        case .senha: String(localized: "com senha")
        case .semRoot: String(localized: "sem root")
        case nil: root ? String(localized: "como administrador") : String(localized: "sem root")
        }
    }
}

/// Um ponto por hora do rápido (ou por tela nova do Ajustes). `usado` é do container interno.
nonisolated struct Ponto: Codable, Sendable, Equatable {
    let t: String
    let total, usado, livre, purgeable, systemData: Int64?
    let telaEm: String?
}

/// Uma amostra por hora das pastas do automático (o `top` da linha, sem as dobradas): a linha do
/// tempo por pasta do Histórico. Pasta que não estava no `top` naquela hora não aparece (ausente
/// não é zero).
nonisolated struct PontoPastas: Codable, Sendable, Equatable {
    let t: String
    let pastas: [String: Int64]
}

/// Uma medição salva, já decodificada.
nonisolated struct Salva: Sendable, Identifiable {
    let url: URL
    let completo: Completo
    let modo: ModoMedicao?
    var id: URL { url }
}

/// Diferença de um termo ou de uma linha do `top` entre duas medições. `antes` ou `depois` nil:
/// a linha está numa só das medições (ausente não é zero).
nonisolated struct Mudanca: Sendable, Identifiable {
    let id: String
    /// nil = termo da conta (`termo` é a chave).
    let linha: LinhaTop?
    let termo: String
    let antes, depois: Int64?

    /// Para ordenar e filtrar: a linha ausente conta pelo valor da presente.
    var tamanho: Int64 { abs((depois ?? 0) - (antes ?? 0)) }
}

nonisolated enum Historico {
    private static var fm: FileManager { .default }

    /// Com bundle id: ~/Library/Application Support/SDTally. Fora do .app (bundleID nil), só
    /// SDTALLY_DADOS, e essa pasta só é lida, nunca gravada. Nos outros casos, nil.
    static func pastaLeitura(env: [String: String], bundleID: String?) -> URL? {
        if bundleID == nil { return env["SDTALLY_DADOS"].map { URL(fileURLWithPath: $0, isDirectory: true) } }
        return fm.homeDirectoryForCurrentUser.appending(path: "Library/Application Support/SDTally", directoryHint: .isDirectory)
    }

    /// Só no .app, fora do Dev.ativo. O xctest tem bundle id: teste nenhum pede a pasta real.
    static func pastaGravacao(bundleID: String?, dev: Bool = Dev.ativo) -> URL? {
        guard bundleID != nil, !dev else { return nil }
        return pastaLeitura(env: [:], bundleID: bundleID)
    }

    // MARK: Gravar

    static func nomeValido(_ nome: String) -> Bool {
        nome.wholeMatch(of: /\d{4}-\d{2}-\d{2}-\d{4}(-\d+)?\.json/) != nil
    }

    /// "2026-10-02 09:36[:ss]" → "2026-10-02-0936"; carimbo estranho usa a hora de agora.
    static func nomeArquivo(_ generatedAt: String, agora: Date = .now) -> String {
        let n = String(generatedAt.prefix(16)).replacing(" ", with: "-").replacing(":", with: "")
        if nomeValido(n + ".json") { return n }
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd-HHmm"
        return f.string(from: agora)
    }

    /// Pasta 0700 (criada ou já existente).
    private static func criar(_ dir: URL) throws {
        try fm.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        try fm.setAttributes([.posixPermissions: 0o700], ofItemAtPath: dir.path)
    }

    private static func escrever(_ dados: Data, _ url: URL) throws {
        try dados.write(to: url, options: .atomic)
        try fm.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
    }

    /// Grava o stdout cru em Medicoes/ (colisão de nome ganha -2, -3…) e o modo no xattr.
    @discardableResult
    static func gravar(_ dados: Data, generatedAt: String, modo: ModoMedicao?, em pasta: URL) throws -> URL {
        try criar(pasta)
        let dir = pasta.appending(path: "Medicoes", directoryHint: .isDirectory)
        try criar(dir)
        let base = nomeArquivo(generatedAt)
        var url = dir.appending(path: base + ".json")
        var n = 2
        while (try? fm.attributesOfItem(atPath: url.path)) != nil {  // lstat: symlink quebrado também ocupa o nome
            url = dir.appending(path: "\(base)-\(n).json")
            n += 1
        }
        try escrever(dados, url)
        if let modo { _ = setxattr(url.path, ModoMedicao.xattr, modo.rawValue, modo.rawValue.utf8.count, 0, XATTR_NOFOLLOW) }
        return url
    }

    static func gravarPontos(_ pontos: [Ponto], em pasta: URL) throws {
        try criar(pasta)
        let e = JSONEncoder()
        var d = Data()
        for p in pontos {
            d.append(try e.encode(p))
            d.append(0x0A)
        }
        try escrever(d, pasta.appending(path: "pontos.jsonl"))
    }

    /// 100 medições ou 50 MB (fica sempre a mais nova) e pontos de até 90 dias. Só remove de
    /// Medicoes/ arquivo comum com nome AAAA-MM-DD-HHMM(-n).json: o único removeItem do target.
    static func podar(url pasta: URL, agora: Date = .now) throws {
        let chaves: Set<URLResourceKey> = [.isSymbolicLinkKey, .isRegularFileKey, .fileSizeKey]
        let dir = pasta.appending(path: "Medicoes", directoryHint: .isDirectory)
        let arquivos = ((try? fm.contentsOfDirectory(at: dir, includingPropertiesForKeys: Array(chaves))) ?? [])
            .compactMap { u -> (URL, Int64)? in
                guard nomeValido(u.lastPathComponent), let v = try? u.resourceValues(forKeys: chaves),
                      v.isSymbolicLink == false, v.isRegularFile == true else { return nil }
                return (u, Int64(v.fileSize ?? 0))
            }
            // ponytail: ordem pelo nome (que vem do generated_at); "-2" de uma colisão no mesmo minuto pode sair trocado.
            .sorted { $0.0.lastPathComponent > $1.0.lastPathComponent }
        var soma: Int64 = 0
        for (i, (u, tamanho)) in arquivos.enumerated() {
            soma += tamanho
            if i >= 100 || (i > 0 && soma > 50_000_000) { try fm.removeItem(at: u) }
        }
        let pontos = lerPontos(pasta)
        let ficam = recentes(pontos, agora: agora)
        if ficam.count != pontos.count { try gravarPontos(ficam, em: pasta) }
        let pp = lerPontosPastas(pasta)
        let ficamPP = recentes(pp, agora: agora)
        if ficamPP.count != pp.count { try gravarPontosPastas(ficamPP, em: pasta) }
    }

    /// Amostras de pastas de até 90 dias.
    static func recentes(_ pontos: [PontoPastas], agora: Date = .now) -> [PontoPastas] {
        let corte = agora.addingTimeInterval(-90 * 86_400)
        return pontos.filter { data($0.t).map { $0 >= corte } ?? false }
    }

    /// Pontos de até 400 dias (o filtro Ano do Histórico; uma linha por hora, ~1 MB por ano).
    static func recentes(_ pontos: [Ponto], agora: Date = .now) -> [Ponto] {
        let corte = agora.addingTimeInterval(-400 * 86_400)
        return pontos.filter { data($0.t).map { $0 >= corte } ?? false }
    }

    // MARK: Ler

    /// Medições (da mais antiga para a mais nova por `generated_at`, não pelo nome) e pontos.
    @concurrent static func ler(_ pasta: URL) async -> (medicoes: [Salva], pontos: [Ponto]) {
        (lerMedicoes(pasta), lerPontos(pasta))
    }

    /// Pula symlink, arquivo de mais de 20 MB, corrompido ou de outro schema (como o `--last`).
    static func lerMedicoes(_ pasta: URL) -> [Salva] {
        let chaves: Set<URLResourceKey> = [.isSymbolicLinkKey, .isRegularFileKey, .fileSizeKey]
        let dir = pasta.appending(path: "Medicoes", directoryHint: .isDirectory)
        guard let urls = try? fm.contentsOfDirectory(at: dir, includingPropertiesForKeys: Array(chaves)) else { return [] }
        return urls.compactMap { u -> Salva? in
            guard u.pathExtension == "json", let v = try? u.resourceValues(forKeys: chaves),
                  v.isSymbolicLink == false, v.isRegularFile == true, (v.fileSize ?? .max) <= 20_000_000,
                  let d = try? Data(contentsOf: u), let c = try? decodificar(Completo.self, d, kind: nil) else { return nil }
            return Salva(url: u, completo: c, modo: lerModo(u))
        }
        .sorted { $0.completo.generatedAt < $1.completo.generatedAt }
    }

    static func lerModo(_ url: URL) -> ModoMedicao? {
        var buf = [UInt8](repeating: 0, count: 32)
        let n = getxattr(url.path, ModoMedicao.xattr, &buf, buf.count, 0, XATTR_NOFOLLOW)
        guard n > 0 else { return nil }
        return ModoMedicao(rawValue: String(decoding: buf.prefix(n), as: UTF8.self))
    }

    /// Linha que não decodifica é pulada.
    static func lerPontos(_ pasta: URL) -> [Ponto] {
        guard let d = try? Data(contentsOf: pasta.appending(path: "pontos.jsonl")) else { return [] }
        let dec = JSONDecoder()
        return d.split(separator: 0x0A).compactMap { try? dec.decode(Ponto.self, from: Data($0)) }
    }

    // MARK: Automático

    /// A última linha do `sdtally --auto`: o app abre já com números (os da última vez) enquanto
    /// a passada nova (~40 s) não termina.
    static func gravarUltimoAuto(_ dados: Data, em pasta: URL) throws {
        try criar(pasta)
        try escrever(dados, pasta.appending(path: "auto.json"))
    }

    /// Última linha do automático e amostras de pastas; sem symlink nem arquivo de mais de 20 MB.
    @concurrent static func lerAuto(_ pasta: URL) async -> (ultimo: Data?, pontos: [PontoPastas]) {
        let u = pasta.appending(path: "auto.json")
        let v = try? u.resourceValues(forKeys: [.isSymbolicLinkKey, .fileSizeKey])
        let ok = v?.isSymbolicLink == false && (v?.fileSize ?? .max) <= 20_000_000
        return (ok ? try? Data(contentsOf: u) : nil, lerPontosPastas(pasta))
    }

    static func gravarPontosPastas(_ pontos: [PontoPastas], em pasta: URL) throws {
        try criar(pasta)
        let e = JSONEncoder()
        e.outputFormatting = .sortedKeys
        var d = Data()
        for p in pontos {
            d.append(try e.encode(p))
            d.append(0x0A)
        }
        try escrever(d, pasta.appending(path: "pastas.jsonl"))
    }

    /// Linha que não decodifica é pulada.
    static func lerPontosPastas(_ pasta: URL) -> [PontoPastas] {
        guard let d = try? Data(contentsOf: pasta.appending(path: "pastas.jsonl")) else { return [] }
        let dec = JSONDecoder()
        return d.split(separator: 0x0A).compactMap { try? dec.decode(PontoPastas.self, from: Data($0)) }
    }

    /// A amostra de agora, ou nil: no máximo uma por hora.
    static func proximoPontoPastas(_ c: Completo, depois pontos: [PontoPastas], agora: Date = .now) -> PontoPastas? {
        let pastas = Dictionary(c.top.compactMap { l in l.folded == true ? nil : l.path.map { ($0, l.bytes) } }, uniquingKeysWith: +)
        let p = PontoPastas(t: carimboComFuso(c.generatedAt, agora: agora), pastas: pastas)
        guard !pastas.isEmpty else { return nil }
        guard let u = pontos.last, let a = data(u.t), let b = data(p.t) else { return p }
        return b.timeIntervalSince(a) >= 3600 ? p : nil
    }

    // MARK: Pontos

    /// O ponto de agora, ou nil: no máximo um por hora, mais um a cada tela do Ajustes com
    /// `logged_at` novo (só esse traz `systemData`). `usado` = ceiling − free do container interno.
    static func proximoPonto(_ r: Rapido, depois pontos: [Ponto], agora: Date = .now) -> Ponto? {
        let ultimaTela = pontos.last { $0.telaEm != nil }?.telaEm
        var tela: (em: String, sd: Int64)?
        if let t = r.settingsScreen.screen, t.state != "not_measured", let sd = t.systemData,
           let em = t.loggedAt.map({ String($0.prefix(19)) }), em != ultimaTela {
            tela = (em, sd)
        }
        let c = r.containers?.first { $0.internal == true && $0.holdsData }
        let usado: Int64? = if let teto = c?.ceiling, let livre = c?.free { teto - livre } else { nil }
        let p = Ponto(t: carimboComFuso(r.generatedAt, agora: agora), total: r.capacity?.total, usado: usado, livre: r.capacity?.free,
                      purgeable: r.purgeable.bytes, systemData: tela?.sd, telaEm: tela?.em)
        guard tela == nil, let u = pontos.last, let a = data(u.t), let b = data(p.t) else { return p }
        return b.timeIntervalSince(a) >= 3600 ? p : nil
    }

    /// O último ponto com Dados do Sistema de uma abertura do Ajustes anterior a `telaEm`.
    static func leituraAnterior(_ pontos: [Ponto], telaEm: String?) -> Ponto? {
        guard let atual = telaEm.map({ String($0.prefix(19)) }) else { return nil }
        return pontos.last { $0.systemData != nil && ($0.telaEm ?? "") < atual }
    }

    // MARK: Comparação

    static let limite: Int64 = 100_000_000
    static let textoModos = String(localized: "Sem comparação: modos diferentes (com e sem senha).")
    static let textoVersoes = String(localized: "Sem comparação: versões diferentes do sdtally.")
    static let textoInterrompida = String(localized: "Sem comparação: uma das medições foi interrompida.")

    static func interrompida(_ c: Completo) -> Bool { c.incompleteReasons.contains("interrupted") }

    /// Por que duas medições não se comparam; nil = comparáveis.
    static func motivo(_ a: Completo, _ b: Completo) -> String? {
        if interrompida(a) || interrompida(b) { return textoInterrompida }
        if a.root != b.root || a.fullDiskAccess != b.fullDiskAccess || a.complete != b.complete { return textoModos }
        if a.tool != b.tool { return textoVersoes }
        return nil
    }

    static func comparavel(_ a: Completo, _ b: Completo) -> Bool { motivo(a, b) == nil }

    /// Termos e linhas do `top` (chave path|term|folded) com |Δ| ≥ 100 MB, do maior para o menor.
    /// nil quando as duas não são comparáveis.
    static func mudancas(de a: Completo, para b: Completo) -> [Mudanca]? {
        guard comparavel(a, b) else { return nil }
        var m: [Mudanca] = []
        let termosA = Dictionary(a.dataVolume.terms.compactMap { t in t.bytes.map { (t.key, $0) } }, uniquingKeysWith: { x, _ in x })
        for t in b.dataVolume.terms {
            if let depois = t.bytes, let antes = termosA[t.key] {
                m.append(Mudanca(id: "termo|" + t.key, linha: nil, termo: t.key, antes: antes, depois: depois))
            }
        }
        let la = Dictionary(a.top.map { ($0.id, $0) }, uniquingKeysWith: { x, _ in x })
        let lb = Dictionary(b.top.map { ($0.id, $0) }, uniquingKeysWith: { x, _ in x })
        for (k, l) in lb { m.append(Mudanca(id: k, linha: l, termo: l.term, antes: la[k]?.bytes, depois: l.bytes)) }
        for (k, l) in la where lb[k] == nil { m.append(Mudanca(id: k, linha: l, termo: l.term, antes: l.bytes, depois: nil)) }
        return m.filter { $0.tamanho >= limite }.sorted { ($0.tamanho, $0.id) > ($1.tamanho, $1.id) }
    }
}

/// "às 09:36" se for de hoje; senão "em 28/09 às 09:29".
nonisolated func quando(_ d: Date, agora: Date = .now) -> String {
    let h = d.formatted(.dateTime.hour().minute().locale(localApp))
    if Calendar.current.isDate(d, inSameDayAs: agora) { return String(localized: "às \(h)") }
    return String(localized: "em \(d.formatted(.dateTime.day(.twoDigits).month(.twoDigits).locale(localApp))) às \(h)")
}

// A parte do atualizador sem rede e sem interface: ler o release da API do GitHub, comparar versões,
// conferir o SHA-256 e a assinatura do app novo e trocar o bundle (testada em SDTallyUpdaterTests).
import CryptoKit
import Foundation
import Security

nonisolated enum Versao {
    /// "1.2.3", "v1.2" ou "1.2.3-beta.1": os três números (faltando, 0) e o sufixo de pré-lançamento.
    static func partes(_ s: String) -> (numeros: [Int], pre: String?)? {
        let t = s.hasPrefix("v") ? s.dropFirst() : Substring(s)
        let nucleo = t.split(separator: "-", maxSplits: 1, omittingEmptySubsequences: false)
        let n = nucleo[0].split(separator: ".", omittingEmptySubsequences: false)
        guard (1...3).contains(n.count), n.allSatisfy({ !$0.isEmpty && $0.allSatisfy { $0.isASCII && $0.isNumber } }),
              nucleo.count == 1 || !nucleo[1].isEmpty else { return nil }
        let numeros = n.compactMap { Int($0) }
        guard numeros.count == n.count else { return nil }
        return (numeros + Array(repeating: 0, count: 3 - numeros.count), nucleo.count == 2 ? String(nucleo[1]) : nil)
    }

    /// `a` é mais nova que `b`. Versão que não se lê nunca é mais nova.
    static func maisNova(_ a: String, que b: String) -> Bool {
        guard let x = partes(a), let y = partes(b) else { return false }
        if x.numeros != y.numeros { return y.numeros.lexicographicallyPrecedes(x.numeros) }
        switch (x.pre, y.pre) {
        case (nil, nil), (_?, nil): return false
        case (nil, _?): return true
        // shortcut: pré-lançamentos comparados como texto ("beta.10" < "beta.9"); o publicar.sh não publica pré-lançamento.
        case let (p?, q?): return q < p
        }
    }
}

/// O release mais recente, já conferido: imutável, com o DMG e o SHA-256 dele.
nonisolated struct Lancamento: Equatable, Sendable {
    let versao: String
    let dmg: URL
    /// Hex minúsculo, 64 caracteres.
    let sha256: String
    let pagina: URL

    static let api = URL(string: "https://api.github.com/repos/guiselig/sdtally/releases/latest")!
    static let versoes = URL(string: "https://github.com/guiselig/sdtally/releases")!
    /// Só estes hosts, por https. Os dois últimos são para onde o github.com redireciona o download.
    static let hosts: Set<String> = ["api.github.com", "github.com", "objects.githubusercontent.com", "release-assets.githubusercontent.com"]

    static func permitido(_ u: URL?) -> Bool {
        guard let u, u.scheme == "https", let h = u.host() else { return false }
        return hosts.contains(h)
    }

    enum Recusa: Error, Equatable {
        case json, versao, naoImutavel, semDMG, semDigest, urlFora
    }

    private struct API: Decodable {
        let tag_name: String
        let immutable: Bool?
        let html_url: String?
        let assets: [Asset]
        struct Asset: Decodable {
            let name: String
            let browser_download_url: String
            let digest: String?
        }
    }

    /// Lê a resposta de /releases/latest. Sem `immutable: true` ou sem o digest sha256 do
    /// SDTally-<versão>.dmg, recusa: o atualizador não faz nada.
    static func ler(_ dados: Data) throws(Recusa) -> Lancamento {
        guard let r = try? JSONDecoder().decode(API.self, from: dados) else { throw .json }
        let v = r.tag_name.hasPrefix("v") ? String(r.tag_name.dropFirst()) : r.tag_name
        guard Versao.partes(v) != nil else { throw .versao }
        guard r.immutable == true else { throw .naoImutavel }
        guard let a = r.assets.first(where: { $0.name == "SDTally-\(v).dmg" }) else { throw .semDMG }
        guard let d = a.digest, d.hasPrefix("sha256:"), let h = hex(d.dropFirst(7)) else { throw .semDigest }
        guard let u = URL(string: a.browser_download_url), permitido(u), u.host() == "github.com",
              u.path().hasPrefix("/guiselig/sdtally/releases/download/") else { throw .urlFora }
        let p = r.html_url.flatMap(URL.init(string:)).flatMap { permitido($0) && $0.host() == "github.com" ? $0 : nil }
        return Lancamento(versao: v, dmg: u, sha256: h, pagina: p ?? versoes)
    }

    private static func hex<S: StringProtocol>(_ s: S) -> String? {
        let t = s.lowercased()
        return t.count == 64 && t.allSatisfy(\.isHexDigit) ? t : nil
    }

    static func sha256(_ dados: Data) -> String {
        SHA256.hash(data: dados).map { String(format: "%02x", $0) }.joined()
    }

    /// Vai no userInfo da notificação: o clique em Instalar funciona mesmo com o atualizador reaberto.
    var info: [String: String] {
        ["versao": versao, "dmg": dmg.absoluteString, "sha256": sha256, "pagina": pagina.absoluteString]
    }

    init(versao: String, dmg: URL, sha256: String, pagina: URL) {
        (self.versao, self.dmg, self.sha256, self.pagina) = (versao, dmg, sha256, pagina)
    }

    init?(info: [String: String]) {
        guard let v = info["versao"], Versao.partes(v) != nil, let d = info["dmg"].flatMap(URL.init(string:)), Self.permitido(d),
              let h = info["sha256"].flatMap(Self.hex), let p = info["pagina"].flatMap(URL.init(string:)), Self.permitido(p) else { return nil }
        self.init(versao: v, dmg: d, sha256: h, pagina: p)
    }
}

nonisolated enum Instalacao {
    static let idApp = "io.github.guiselig.sdtally"
    /// Designated requirement que o SDTally.app novo tem de cumprir: o identifier e o time do montar.sh.
    static let requisito = #"identifier "io.github.guiselig.sdtally" and anchor apple generic and certificate leaf[subject.OU] = "5DFAL23UXC""#

    static func info(_ app: URL) -> [String: Any]? {
        guard let d = try? Data(contentsOf: app.appending(path: "Contents/Info.plist")) else { return nil }
        return try? PropertyListSerialization.propertyList(from: d, format: nil) as? [String: Any]
    }

    static func versao(_ app: URL) -> String? { info(app)?["CFBundleShortVersionString"] as? String }

    /// O SDTally.app que contém o atualizador (.../SDTally.app/Contents/Library/LoginItems/SDTallyUpdater.app),
    /// onde quer que ele esteja; nil fora de um SDTally.app.
    static func appDono(_ atualizador: URL) -> URL? {
        var u = atualizador.standardizedFileURL
        for _ in 0..<4 { u.deleteLastPathComponent() }
        return info(u)?["CFBundleIdentifier"] as? String == idApp ? u : nil
    }

    /// Validação estrita, todas as arquiteturas e o código aninhado, contra `requisito`.
    static func assinaturaValida(_ app: URL, requisito: String = requisito) -> Bool {
        var codigo: SecStaticCode?
        var req: SecRequirement?
        guard SecStaticCodeCreateWithPath(app as CFURL, [], &codigo) == errSecSuccess, let codigo,
              SecRequirementCreateWithString(requisito as CFString, [], &req) == errSecSuccess, let req else { return false }
        let f = SecCSFlags(rawValue: kSecCSStrictValidate | kSecCSCheckAllArchitectures | kSecCSCheckNestedCode)
        return SecStaticCodeCheckValidity(codigo, f, req) == errSecSuccess
    }

    /// A troca é um rename na pasta de `app`: sem escrita nela, não dá.
    static func podeEscrever(_ app: URL) -> Bool {
        FileManager.default.isWritableFile(atPath: app.deletingLastPathComponent().path)
    }

    /// Copia `novo` para uma pasta temporária no mesmo volume de `destino` e troca os dois de uma vez
    /// (replaceItemAt). O bundle antigo do SDTally sai; qualquer erro antes da troca deixa `destino` como estava.
    static func trocar(_ destino: URL, por novo: URL) throws {
        let fm = FileManager.default
        let pasta = try fm.url(for: .itemReplacementDirectory, in: .userDomainMask, appropriateFor: destino, create: true)
        defer { apagarTemporario(pasta) }
        let copia = pasta.appending(path: destino.lastPathComponent)
        try fm.copyItem(at: novo, to: copia)
        _ = try fm.replaceItemAt(destino, withItemAt: copia)
    }

    /// Só as pastas temporárias do próprio atualizador: o download e a da troca.
    static func apagarTemporario(_ u: URL) {
        let p = u.standardizedFileURL.path
        let tmp = FileManager.default.temporaryDirectory.standardizedFileURL.path
        guard p.hasPrefix(tmp.hasSuffix("/") ? tmp : tmp + "/") || p.contains("/TemporaryItems/") else { return }
        try? FileManager.default.removeItem(at: u)
    }
}

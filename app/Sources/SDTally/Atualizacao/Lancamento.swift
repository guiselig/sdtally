// O release mais recente do GitHub, lido e conferido aqui sem buscar nada (testado em SDTallyTests): imutável,
// com o DMG e o SHA-256 dele, por https e só nos hosts da lista. Quem busca é Atualizacoes.
import CryptoKit
import Foundation
import SDTallyInstalacao

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

    /// Guardado nas preferências: a versão nova continua à vista depois de reabrir o app.
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

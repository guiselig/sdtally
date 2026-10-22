// Contrato entre o app e o helper root (LaunchDaemon registrado pelo app com SMAppService).
// O helper só mede o volume inteiro (`sdtally --json`) ou abre o automático (`sdtally --auto`),
// sempre com argumentos fixos, para o usuário da conexão; app e helper
// só aceitam o outro lado se ele estiver assinado pelo mesmo time (certificado emitido pela Apple).
import Foundation
import Security

public enum HelperXPC {
    /// Label do LaunchDaemon e nome do serviço Mach.
    public static let servico = "io.github.guiselig.sdtally.helper"
    public static let plist = "io.github.guiselig.sdtally.helper.plist"
    public static let idApp = "io.github.guiselig.sdtally"
    public static let idHelper = "io.github.guiselig.sdtally.helper"
    public static let idCLI = "io.github.guiselig.sdtally.cli"

    /// Chave do Info.plist embutido do helper com o CDHash do sdtally da mesma build (montar.sh).
    public static let chaveCDHash = "SDTallyCLICDHash"

    /// Requisito de assinatura: identifier fixo, certificado da Apple e o mesmo time; com `cdhash`,
    /// só aquele binário (um sdtally antigo assinado pelo mesmo time não passa).
    /// nil se o time não tiver a forma de um Team ID ou o cdhash não for 40 hex (evita injetar
    /// texto no requisito).
    public static func requisito(identifier: String, time: String, cdhash: String? = nil) -> String? {
        guard time.count == 10, time.allSatisfy({ $0.isASCII && ($0.isUppercase || $0.isNumber) }) else { return nil }
        let r = "identifier \"\(identifier)\" and anchor apple generic and certificate leaf[subject.OU] = \"\(time)\""
        guard let cdhash else { return r }
        guard cdhash.count == 40, cdhash.allSatisfy({ $0.isASCII && $0.isHexDigit }) else { return nil }
        return r + " and cdhash H\"\(cdhash)\""
    }

    /// Time da assinatura do próprio processo; nil com assinatura ad hoc. Valida o código em
    /// execução contra o arquivo antes de ler: se o executável foi trocado em disco depois de
    /// subir, o time lido seria o do arquivo novo (achado da revisão de 02/10).
    public static func timeProprio() -> String? {
        assinaturaPropria()?[kSecCodeInfoTeamIdentifier as String] as? String
    }

    /// Info.plist embutido no próprio executável, como a assinatura o vê (coberto por ela).
    public static func plistProprio() -> [String: Any]? {
        assinaturaPropria()?[kSecCodeInfoPList as String] as? [String: Any]
    }

    private static func assinaturaPropria() -> [String: Any]? {
        var eu: SecCode?
        guard SecCodeCopySelf([], &eu) == errSecSuccess, let eu,
              SecCodeCheckValidity(eu, [], nil) == errSecSuccess else { return nil }
        var estatico: SecStaticCode?
        guard SecCodeCopyStaticCode(eu, [], &estatico) == errSecSuccess, let estatico else { return nil }
        var info: CFDictionary?
        guard SecCodeCopySigningInformation(estatico, SecCSFlags(rawValue: kSecCSSigningInformation), &info) == errSecSuccess else { return nil }
        return info as? [String: Any]
    }
}

/// Interface XPC do helper. Respostas: (JSON do sdtally, nil) ou (nil, mensagem de erro).
@objc public protocol HelperProtocolo {
    func medir(reply: @escaping @Sendable (Data?, String?) -> Void)
    func parar()
    /// Automático com root: sobe `sdtally --auto` e devolve a ponta de leitura do stdout dele
    /// (uma linha JSON por mudança), ou (nil, erro). Vive até a conexão cair (o app a invalida para parar).
    func iniciarAuto(reply: @escaping @Sendable (FileHandle?, String?) -> Void)
}

// Registro do app (dono, 02/11): o que o app fez de importante, num arquivo que a pessoa abre em
// Configurações › Avançado e pode anexar a uma issue. Só local, nunca sai do Mac. Em inglês, como as issues;
// a pasta pessoal vira "~". Também vai ao log do sistema em .notice, que o macOS guarda (.info não).
import Foundation
import os

nonisolated enum Registro {
    static let pasta = FileManager.default.homeDirectoryForCurrentUser.appending(path: "Library/Logs/SDTally")
    static let arquivo = pasta.appending(path: "SDTally.log")
    /// Passando disto, o arquivo vira SDTally.1.log (o anterior se perde): no máximo ~2 MB no disco.
    static let limite = 1_000_000
    /// Pid desta sessão; ao abrir, se ainda estiver gravado, a anterior não fechou normalmente.
    static let chaveAberto = "registroPidAberto"

    private static let fila = DispatchQueue(label: "io.github.guiselig.sdtally.registro")
    private static let log = Logger(subsystem: "io.github.guiselig.sdtally", category: "registro")
    private static let casa = FileManager.default.homeDirectoryForCurrentUser.path

    static func anotar(_ categoria: String, _ texto: String) {
        let t = semCasa(texto)
        log.notice("\(categoria, privacy: .public): \(t, privacy: .public)")
        let linha = "\(Date.now.formatted(.iso8601.year().month().day().dateSeparator(.dash).time(includingFractionalSeconds: false).timeSeparator(.colon))) [\(categoria)] \(t)\n"
        fila.async { gravar(linha) }
    }

    /// Espera a fila: para a última linha antes de sair.
    static func esvaziar() { fila.sync {} }

    static func semCasa(_ s: String) -> String { s.replacingOccurrences(of: casa, with: "~") }

    /// Na abertura: anota a sessão anterior que não fechou (travou, forçada a sair, Mac desligou) e esta.
    static func aoAbrir(versao: String, d: UserDefaults = .standard) {
        let pid = ProcessInfo.processInfo.processIdentifier
        if let antes = d.object(forKey: chaveAberto) as? Int, anteriorNaoFechou(antes, atual: pid) {
            anotar("app", "previous session (pid \(antes)) did not quit normally: it hung, was force quit, crashed or the Mac shut down")
        }
        d.set(Int(pid), forKey: chaveAberto)
        let mac = ProcessInfo.processInfo.operatingSystemVersion
        anotar("app", "started SDTally \(versao) (pid \(pid)) on macOS \(mac.majorVersion).\(mac.minorVersion).\(mac.patchVersion)")
    }

    /// O mesmo pid é a mesma sessão (o aoAbrir rodou duas vezes), não uma que caiu.
    static func anteriorNaoFechou(_ gravado: Int, atual: Int32) -> Bool { gravado != Int(atual) }

    /// No fim da saída normal: a próxima abertura não acusa nada.
    static func aoFechar(d: UserDefaults = .standard) {
        anotar("app", "quit")
        d.removeObject(forKey: chaveAberto)
        esvaziar()
    }

    private static func gravar(_ linha: String) {
        let fm = FileManager.default
        try? fm.createDirectory(at: pasta, withIntermediateDirectories: true)
        if let t = (try? fm.attributesOfItem(atPath: arquivo.path))?[.size] as? Int, t > limite {
            let velho = pasta.appending(path: "SDTally.1.log")
            try? fm.removeItem(at: velho)
            try? fm.moveItem(at: arquivo, to: velho)
        }
        guard let h = try? FileHandle(forWritingTo: arquivo) else {
            try? Data(linha.utf8).write(to: arquivo)
            return
        }
        defer { try? h.close() }
        _ = try? h.seekToEnd()
        try? h.write(contentsOf: Data(linha.utf8))
    }
}

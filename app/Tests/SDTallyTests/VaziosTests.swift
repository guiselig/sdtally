// Estados vazios (spec-fase6 §5.6): todo texto passa no G9 e o não medido nunca vira "0".
import Foundation
import Testing
@testable import SDTally

private let todos: [Vazio] = [
    .v1(ultimo: 221_266_053_536, hora: "09:29"), .v1(ultimo: nil, hora: nil), .v2, .v3, .v5(usadoData: nil), .v5(usadoData: 410_000_000_000),
    .v6(hora: "09:36"), .v7(motivo: "no root"), .v8(root: false), .v8(root: true), .v9,
    .v12(semExplicacao: 41_000_000_000, porcento: 10, dataComSenha: "28/09", bytesComSenha: 2_000_000_000),
    .v12(semExplicacao: nil, porcento: nil, dataComSenha: nil, bytesComSenha: nil),
    .v16, .v17, .v18, .v20, .v23,
]

/// scripts/g9-proibidas.txt, a mesma lista do scripts/g9.sh (grep -F, sensível a maiúsculas).
private func proibidas() throws -> [String] {
    let raiz = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
    return try String(contentsOf: raiz.appending(path: "scripts/g9-proibidas.txt"), encoding: .utf8)
        .split(separator: "\n").map(String.init).filter { !$0.isEmpty }
}

@Test func todoTextoPassaNoG9() throws {
    let p = try proibidas()
    #expect(p.contains("lixo") && p.contains("junk"))
    let textos = todos.flatMap { [$0.titulo, $0.texto, $0.acao?.rotulo ?? ""] }
        + ["term not measured", "no lsof", "plist unreadable", "tm status unreadable"].map(motivoNaoConferido)
        + [nenhum(fonte: "fsctl_snapshot_private_size", hora: "09:36")]
    for t in textos {
        for x in p { #expect(!t.contains(x), "\(t) contém \(x)") }
    }
}

@Test func naoMedidoNuncaDaZero() {
    #expect(valorOuNaoMedido(nil) == "não medido")
    #expect(valorOuNaoMedido(0) == "0 B")  // zero medido é zero
    #expect(Vazio.v5(usadoData: nil).texto.hasSuffix("agora: não medido (volume APFS)."))
    let v12 = Vazio.v12(semExplicacao: nil, porcento: nil, dataComSenha: nil, bytesComSenha: nil).texto
    #expect(v12 == "Bloqueado pelo macOS: não medido, sem senha. Medir com senha pode reduzir isto.")
    for v in todos { #expect(!v.texto.contains(" 0 ") && !v.texto.contains(": 0"), "\(v.texto)") }
}

@Test func textosComNumeros() {
    #expect(Vazio.v1(ultimo: 221_266_053_536, hora: "09:29").texto.hasSuffix("Último lido: 221,27 GB às 09:29."))
    #expect(!Vazio.v1(ultimo: nil, hora: nil).texto.contains("Último lido"))
    #expect(Vazio.v1(ultimo: 221_266_053_536, hora: nil).texto.hasSuffix("Último lido: 221,27 GB."))
    #expect(Vazio.v12(semExplicacao: 41_000_000_000, porcento: 10, dataComSenha: "28/09", bytesComSenha: 2_000_000_000).texto
        == "Bloqueado pelo macOS: 41,00 GB (10 %), sem senha. Medir com senha pode reduzir isto. Com senha, em 28/09: 2,00 GB.")
    #expect(Vazio.v16.texto == "Não deu para ler as preferências do Time Machine.")
    #expect(Vazio.v8(root: true).texto == "Nenhum app segurando arquivo apagado (medido).")
    #expect(nenhum(fonte: "fsctl_snapshot_private_size", hora: "09:36") == "Nenhum (snapshots (APFS), medido às 09:36).")
    #expect(Vazio.v1(ultimo: nil, hora: nil).acao == .abrirArmazenamento && Vazio.v9.acao == nil)
}

@Test func motivosDeAdviceUnchecked() {
    #expect(linhaNaoConferido(nome: "Time Machine parado", reason: "plist unreadable") == "Time Machine parado: o estado do Time Machine não pôde ser lido")
    #expect(motivoNaoConferido("motivo novo") == "motivo novo")
}

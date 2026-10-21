// A7: o PDF sai sem autor, com as páginas das seções, e o JSON exportado é o stdout sem mudança.
// A12: com o toggle ligado, o JSON e o PDF saem do `sdtally --redact names --stdin` (o do cargo).
import Foundation
import PDFKit
import Testing
@testable import SDTally

private func fixture(_ caminho: String) throws -> URL {
    let base = try #require(Bundle.module.url(forResource: "Fixtures", withExtension: nil))
    return base.appending(path: caminho)
}

@MainActor private func painelDaFixture() throws -> Painel {
    let p = Painel()
    let d = try Data(contentsOf: fixture("completo.json"))
    p.completo = try decodificar(Completo.self, d, kind: nil)
    p.completoDados = d
    return p
}

@MainActor @Test func pdfSemAutor() async throws {
    let painel = try painelDaFixture()
    let destino = FileManager.default.temporaryDirectory.appending(path: "sdtally-\(UUID().uuidString).pdf")
    defer { try? FileManager.default.removeItem(at: destino) }
    let pontos = Historico.lerPontos(try fixture("dados"))
    #expect(!pontos.isEmpty)
    try await Relatorio.gerarPDF(painel, destino: destino, pontos: pontos)
    let doc = try #require(PDFDocument(url: destino))
    #expect(doc.pageCount >= 4)
    #expect(doc.pageCount == 5)  // com pontos, entra o Histórico
    let atributos = doc.documentAttributes ?? [:]
    #expect(atributos[PDFDocumentAttribute.authorAttribute] == nil)
    #expect(atributos[PDFDocumentAttribute.creatorAttribute] as? String == "SDTally")
    #expect(doc.page(at: 0)?.bounds(for: .mediaBox).width == 595)
    #expect(doc.page(at: 2)?.string?.contains("O que mais pesa") == true)

    // Sem pontos, sem a página do Histórico.
    try await Relatorio.gerarPDF(painel, destino: destino, pontos: [])
    #expect(PDFDocument(url: destino)?.pageCount == 4)
}

@MainActor @Test func jsonExportadoIgualAFixture() async throws {
    let painel = try painelDaFixture()
    let destino = FileManager.default.temporaryDirectory.appending(path: "sdtally-\(UUID().uuidString).json")
    defer { try? FileManager.default.removeItem(at: destino) }
    try await Relatorio.gravarJSON(painel, em: destino)  // toggle desligado: o que exportarJSON chama depois do NSSavePanel
    #expect(try Data(contentsOf: destino) == Data(contentsOf: fixture("completo.json")))
}

/// O sdtally do cargo: SDTALLY_BIN, $CARGO_TARGET_DIR/debug, $TMPDIR/sdt-cargo/debug (critério 1) ou target/debug.
private func sdtallyDoCargo() -> String? {
    let env = ProcessInfo.processInfo.environment
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
    return [env["SDTALLY_BIN"], env["CARGO_TARGET_DIR"].map { $0 + "/debug/sdtally" },
            NSTemporaryDirectory() + "sdt-cargo/debug/sdtally", repo.appending(path: "target/debug/sdtally").path]
        .compactMap { $0 }.first { FileManager.default.isExecutableFile(atPath: $0) }
}

@MainActor @Test(.enabled(if: sdtallyDoCargo() != nil, "pulado: sem o sdtally do cargo; rode cargo build ou SDTALLY_BIN=..."))
func exportRedigidoSemHome() async throws {
    let bin = try #require(sdtallyDoCargo())
    let painel = try painelDaFixture()
    // Uma pasta pessoal fora das conhecidas, para a troca aparecer.
    let cru = String(decoding: try #require(painel.completoDados), as: UTF8.self)
        .replacingOccurrences(of: "\"~/Music\"", with: "\"~/Projeto Secreto\"")
    painel.completoDados = Data(cru.utf8)
    let destino = FileManager.default.temporaryDirectory.appending(path: "sdtally-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: destino.appendingPathExtension("json")); try? FileManager.default.removeItem(at: destino.appendingPathExtension("pdf")) }

    try await Relatorio.gravarJSON(painel, em: destino.appendingPathExtension("json"), redigir: true, bin: bin)
    let json = try String(contentsOf: destino.appendingPathExtension("json"), encoding: .utf8)
    #expect(!json.contains("/Users/exemplo"))
    #expect(!json.contains("Projeto Secreto"))
    #expect(json.contains("~/<pasta do usuário>"))
    let r = try decodificar(Completo.self, Data(json.utf8), kind: nil)
    #expect(r.home == "/Users/<usuário>")
    #expect(r.top.map(\.bytes) == painel.completo?.top.map(\.bytes))  // sem fusão: os números não mudam

    try await Relatorio.gerarPDF(painel, destino: destino.appendingPathExtension("pdf"), pontos: [], redigir: true, bin: bin)
    let doc = try #require(PDFDocument(url: destino.appendingPathExtension("pdf")))
    let texto = (0..<doc.pageCount).compactMap { doc.page(at: $0)?.string }.joined(separator: "\n")
    #expect(texto.contains("pasta do usuário"))
    #expect(!texto.contains("Projeto Secreto"))
    #expect(!texto.contains("/Users/exemplo"))
}

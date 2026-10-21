// Regra do app: sem subtextos (docs/spec-inicio.md). Frases explicativas removidas não voltam.
import Foundation
import Testing

@Test func fraseExplicativaRemovidaNaoVoltaParaATela() throws {
    let fontes = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().appending(path: "Sources/SDTally")
    let arquivos = try FileManager.default.contentsOfDirectory(at: fontes, includingPropertiesForKeys: nil).filter { $0.pathExtension == "swift" }
    #expect(arquivos.count > 10)
    let removidas = ["O SDTally mediu estas partes", "As partes não somam", "Por que a senha?", "Pode fechar o Ajustes depois",
                     "Enquanto existirem, apagar um arquivo pode liberar menos", "Duplo clique numa pasta abre",
                     "Só o que esta medição mediu", "Texto conferido no macOS \\(", "Para não digitar a senha"]
    for a in arquivos {
        let s = try String(contentsOf: a, encoding: .utf8)
        for f in removidas where a.lastPathComponent != "Relatorio.swift" {
            // Dentro de .help( ou de comentário (///) a frase pode existir; como Text( visível, não.
            for l in s.split(separator: "\n") where l.contains(f) && l.contains("Text(") && !l.contains(".help(") {
                Issue.record("\(a.lastPathComponent): \(l)")
            }
        }
    }
}

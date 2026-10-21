// Livre (spec-fase6 §4.8, §5.2): três linhas da mesma leitura, sem selo "confere", e os valores
// do volume nunca lidos no laço de leitura.
import Foundation
import Testing
@testable import SDTally

private let as1105 = Calendar.current.date(from: DateComponents(year: 2026, month: 10, day: 2, hour: 11, minute: 5))!

@Test func livresTresLinhasDaMesmaLeitura() throws {
    let v = ValoresVolume(livre: 100_000_000_000, importante: 147_000_000_000, opcional: 140_000_000_000, em: as1105)
    let ls = livres(volume: v)
    #expect(ls.map(\.nome) == ["Livre no disco", "Livre + Purgeable", "Livre para downloads opcionais"])
    #expect(ls.map(\.bytes) == [100_000_000_000, 147_000_000_000, 140_000_000_000])
    // Só downloads opcionais tem o (i).
    #expect(ls.map { $0.info != nil } == [false, false, true])
    // Leitura que falhou: não medido, nunca 0.
    #expect(livres(volume: ValoresVolume(livre: nil, importante: 1, opcional: nil, em: as1105)).map(\.bytes) == [nil, 1, nil])
}

@Test func livresSemSeloConfere() throws {
    for l in livres(volume: ValoresVolume(livre: 1, importante: 1, opcional: 1, em: as1105)) {
        for x in ["confere", "bate", "toler", "igual"] { #expect(!(l.nome + (l.info ?? "")).lowercased().contains(x), "\(l.nome)") }
    }
}

/// O laço (Painel.vigiar/atualizar) nunca chama a leitura dos três valores do volume: só a
/// LivresView. O Painel lê só o Opportunistic, junto do purgeable (`lerOpcional`); o Important é
/// livre + purgeable, sem NSURL.
@Test func valoresDoVolumeForaDoLaco() throws {
    let fontes = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().appending(path: "Sources/SDTally")
    let arquivos = try FileManager.default.contentsOfDirectory(at: fontes, includingPropertiesForKeys: nil).filter { $0.pathExtension == "swift" }
    #expect(arquivos.count > 10)
    for a in arquivos {
        var s = try String(contentsOf: a, encoding: .utf8)
        if a.lastPathComponent == "Painel.swift" { s = s.replacingOccurrences(of: "volumeAvailableCapacityForOpportunisticUsage", with: "") }
        if a.lastPathComponent != "LivresView.swift" {
            #expect(!s.contains("ValoresVolume.ler("), "\(a.lastPathComponent)")
            #expect(!s.contains("volumeAvailableCapacity"), "\(a.lastPathComponent)")
        }
    }
}

@Test func horaAusenteNaoViraInterrogacao() throws {
    #expect(hora(nil) == nil)
    #expect(hora(Optional("2026-10-02")) == nil)
    #expect(hora("2026-10-02 09:29:42.302128-0300") == "09:29")
}

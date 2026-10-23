import Foundation
import Testing
@testable import SDTally

private func serie(_ gbs: [Double], horas: Double = 6) -> [(d: Date, b: Int64)] {
    gbs.enumerated().map { (d: Date(timeIntervalSince1970: Double($0.offset) * horas * 3600), b: Int64($0.element * 1e9)) }
}

@Test func cresceSempre() {
    // +0,5 GB a cada 6 h por 3 dias: constante, 2 GB por dia.
    let c = padraoCrescimento(serie((0..<13).map { 10 + Double($0) * 0.5 }))
    #expect(c.padrao == .cresceSempre)
    #expect(abs((c.porDia ?? 0) - 2e9) < 1e6)
    #expect(c.amostras == 13)
}

@Test func crescePicos() {
    // Parado, um degrau de 5 GB, parado, outro de 4 GB.
    let c = padraoCrescimento(serie([10, 10, 10, 15, 15, 15, 15, 19, 19, 19, 19, 19, 19]))
    #expect(c.padrao == .crescePicos)
}

@Test func estavelEEncolhe() {
    #expect(padraoCrescimento(serie([20, 20.03, 19.98, 20.02, 20.01, 19.99, 20.04])).padrao == .estavel)
    #expect(padraoCrescimento(serie([30, 29, 28, 27.5, 26, 25, 24])).padrao == .encolhe)
}

@Test func dadosInsuficientes() {
    // Menos de 6 amostras, ou 6 em menos de 24 h: sem padrão nem inclinação.
    let poucas = padraoCrescimento(serie([1, 2, 3, 4, 5]))
    #expect(poucas.padrao == .insuficiente && poucas.porDia == nil && poucas.amostras == 5)
    #expect(padraoCrescimento(serie([1, 2, 3, 4, 5, 6], horas: 1)).padrao == .insuficiente)
}

// Registro do app: pasta pessoal escondida e sessão anterior que não fechou.
import Foundation
import Testing
@testable import SDTally

@Test func registroTrocaAPastaPessoal() {
    let casa = FileManager.default.homeDirectoryForCurrentUser.path
    #expect(Registro.semCasa("falhou em \(casa)/Library/Caches") == "falhou em ~/Library/Caches")
    #expect(Registro.semCasa("sem caminho") == "sem caminho")
}

@Test func registroSessaoAnterior() {
    #expect(Registro.anteriorNaoFechou(41, atual: 42))
    #expect(!Registro.anteriorNaoFechou(42, atual: 42))
}

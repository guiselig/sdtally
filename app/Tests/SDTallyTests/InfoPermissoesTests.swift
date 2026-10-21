// A2: ehAdmin local, SDTALLY_BIN só fora do .app, caminho de pasta absoluto e a faixa do
// Acesso Total divergente.
import Foundation
import Testing
@testable import SDTally

@Test func ehAdminUsuarioInexistente() {
    #expect(ehAdmin(99999) == false)
}

@Test func binarioIgnoraEnvNoBundle() {
    let env = ["SDTALLY_BIN": "/tmp/falso"]
    #expect(Executor.binario(env: env, bundleID: "io.github.guiselig.sdtally") != "/tmp/falso")
    #expect(Executor.binario(env: env, bundleID: nil) == "/tmp/falso")
}

/// Com bundle id (o .app), nenhuma SDTALLY_* chega ao Foto.env. O swift test roda sem bundle id,
/// por isso o teste usa a função pura.
@Test func sdtallyNaoValeComBundle() {
    let env = ["SDTALLY_SECAO": "info", "SDTALLY_COMPLETO": "/tmp/x.json", "SDTALLY_PDF": "/tmp/r.pdf"]
    #expect(Foto.env(env, bundleID: "io.github.guiselig.sdtally").isEmpty)
    #expect(Foto.env(env, bundleID: nil) == env)
}

@Test func caminhoSemBarraRecusado() async {
    for args in [["Library", "--json"], ["~/Library", "--json"], ["", "--json"]] {
        await #expect {
            _ = try await Executor.rodar(args)
        } throws: { e in
            if case .caminhoRelativo = e as? ErroSdtally { true } else { false }
        }
    }
}

@Test func acessoTotalDivergenteMostraFaixa() {
    #expect(PermissoesView.acessoDivergente(app: true, medicao: false))
    #expect(!PermissoesView.acessoDivergente(app: true, medicao: true))
    #expect(!PermissoesView.acessoDivergente(app: nil, medicao: false))
    #expect(!PermissoesView.acessoDivergente(app: true, medicao: nil))
}

// Maiores arquivos e duplicados: decodificação aditiva e a nota de esparso/comprimido.
import Foundation
import Testing
@testable import SDTally

private func fixture(_ nome: String) throws -> Data {
    let url = try #require(Bundle.module.url(forResource: nome, withExtension: "json", subdirectory: "Fixtures"))
    return try Data(contentsOf: url)
}

@Test func maioresArquivosEDuplicadosDecodificam() throws {
    var j = try #require(JSONSerialization.jsonObject(with: fixture("completo")) as? [String: Any])
    j["largest_files"] = [
        ["path": "/Users/x/vm/Docker.raw", "bytes": 12_000_000_000, "private_bytes": 12_000_000_000, "logical_bytes": 64_000_000_000, "kind_of_size": "sparse"],
        ["path": "/Users/x/a.bin", "bytes": 200_000_000, "private_bytes": 0, "logical_bytes": NSNull(), "kind_of_size": NSNull()],
    ]
    j["duplicates"] = ["state": "partial", "reason": "comparison stopped at the 60 s limit", "source": "content",
                       "groups": [["bytes_each": 1_000_000_000, "copies": 3, "wasted_bytes": 2_000_000_000, "paths": ["/a", "/b", "/c"]]],
                       "candidates": 9, "bytes_read": 123]
    let c = try decodificar(Completo.self, JSONSerialization.data(withJSONObject: j), kind: nil)
    let l = try #require(c.largestFiles)
    #expect(l.count == 2 && l[0].kindOfSize == "sparse" && l[1].logicalBytes == nil)
    let d = try #require(c.duplicates)
    #expect(d.state == "partial" && d.groups.first?.wastedBytes == 2_000_000_000 && d.groups.first?.paths.count == 3)
    // Medição antiga, sem os campos: nil, sem erro.
    #expect(try decodificar(Completo.self, fixture("completo"), kind: nil).duplicates == nil)
}

@Test func notaDeEsparsoEComprimido() {
    let esparso = Completo.ArquivoGrande(path: "/a", bytes: 1, logicalBytes: 64_000_000_000, kindOfSize: "sparse")
    #expect(notaTamanho(esparso) == "esparso, aparece como \(gb(64_000_000_000))")
    let comp = Completo.ArquivoGrande(path: "/a", bytes: 1, logicalBytes: 30_000_000_000, kindOfSize: "compressed")
    #expect(notaTamanho(comp) == "comprimido, \(gb(30_000_000_000)) sem compressão")
    #expect(notaTamanho(Completo.ArquivoGrande(path: "/a", bytes: 1, logicalBytes: 5, kindOfSize: nil)) == nil)
    #expect(notaTamanho(Completo.ArquivoGrande(path: "/a", bytes: 1, kindOfSize: "sparse")) == nil)
    #expect(rotuloArquivo("/Users/x/a.bin", home: "/Users/x") == "~/a.bin")
    #expect(rotuloArquivo("/Users/xy/a.bin", home: "/Users/x") == "/Users/xy/a.bin")
}

@Test func duplicadosTemTitulo() { #expect(tituloConselho("duplicate_files") == "Arquivos duplicados") }

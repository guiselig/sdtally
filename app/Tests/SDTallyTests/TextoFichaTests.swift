// Texto das fichas por língua: código exato, depois cortando subtags, depois "en"; exceções es-419 e nb.
import Foundation
import Testing
@testable import SDTally

@Test func textoFichaEscolheALingua() throws {
    let json = #"{"pt": "Dados do Sistema", "en": "System Data", "zh-Hans": "系统数据", "fr": "Données système"}"#
    let t = try JSONDecoder().decode(TextoFicha.self, from: Data(json.utf8))
    #expect(t.escolher("pt-BR") == "Dados do Sistema")
    #expect(t.escolher("pt-PT") == "Dados do Sistema")
    #expect(t.escolher("zh-Hans") == "系统数据")
    #expect(t.escolher("fr-CA") == "Données système")
    #expect(t.escolher("de") == "System Data")
    #expect(t.texto == "Dados do Sistema")
    let u = try JSONDecoder().decode(TextoFicha.self, from: Data(#"{"pt": "a", "en": "b", "es": "c", "es-419": "d", "nb": "e", "zh-Hant": "f"}"#.utf8))
    #expect(u.escolher("es-MX") == "d")
    #expect(u.escolher("es-ES") == "c")
    #expect(u.escolher("es") == "c")
    #expect(u.escolher("no") == "e")
    #expect(u.escolher("zh-Hant-HK") == "f")
    #expect(u.escolher("zh-Hans") == "b")  // nos testes não há .lproj no Bundle.main: pt-BR
}

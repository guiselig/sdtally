// Livre (spec-fase6 §2.2 item 1a, §4.8, §5.2; B2): três linhas ao vivo. Dono: A10a.
// API estável:
//   Painel.livresVivos              os valores, ao vivo (livre do laço de 1 s, purgeable, Opportunistic)
//   livres(volume:)                 pura: as três linhas
//   LivresView()                    só mostra: não lê o volume
import Foundation
import SwiftUI

/// Os três "livres" do volume, montados ao vivo pelo Painel (`livresVivos`).
nonisolated struct ValoresVolume: Sendable, Equatable {
    let livre: Int64?
    let importante: Int64?
    let opcional: Int64?
    let em: Date
}

nonisolated struct LinhaLivre: Sendable, Equatable {
    let nome: String
    /// Texto do botão (i) ao lado do nome; nil sem botão.
    var info: String? = nil
    let bytes: Int64?
    let fonte: String
}

/// As três linhas. Sem selo "confere" nem diferenças: o purgeable por serviço fica em
/// "Liberado automaticamente".
nonisolated func livres(volume v: ValoresVolume) -> [LinhaLivre] {
    [
        .init(nome: String(localized: "Livre no disco"), bytes: v.livre, fonte: "statfs"),
        // O Important da Apple byte a byte, mas montado no app: livre + purgeable.
        .init(nome: String(localized: "Livre + Purgeable"), bytes: v.importante, fonte: "statfs + CacheDelete"),
        .init(nome: String(localized: "Livre para downloads opcionais"),
              info: String(localized: "O espaço que o macOS deixa os apps usarem para o que não é essencial: pré-carregar conteúdo, downloads automáticos e caches. Sai menor que o livre porque o macOS guarda uma reserva para o que é importante."),
              bytes: v.opcional, fonte: "NSURL"),
    ]
}

struct LivresView: View {
    @Environment(Painel.self) private var painel

    var body: some View {
        Bloco(titulo: String(localized: "Livre"), dica: String(localized: "Ao vivo. O Finder não publica de onde tira o “Disponível”.")) {
            if let v = painel.livresVivos {
                grade {
                    ForEach(Array(livres(volume: v).enumerated()), id: \.offset) { _, l in
                        Linha(simbolo: "circle.dashed", nome: l.nome, info: l.info) {
                            NumeroView(bytes: l.bytes, source: l.fonte, state: l.bytes == nil ? "not_measured" : "measured")
                        }
                    }
                }
            } else {
                ProgressView().frame(maxWidth: .infinity)
            }
            CuidaSozinhoView().id("cuida")
        }
    }
}

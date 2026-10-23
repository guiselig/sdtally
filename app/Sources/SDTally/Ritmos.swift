// Ritmo de cada rodada automática, editável nos Ajustes do app (PermissoesView, "Atualização automática").
//   Ritmo.valor   o intervalo em segundos (UserDefaults "ritmo.<caso>"; 0 = desligado)
//   RitmosView    uma linha por rodada, com as opções de cada uma
import SwiftUI

nonisolated enum Ritmo: String, CaseIterable, Identifiable, Sendable {
    case tempoReal, purgAtivo, purgFundo, ajustesEscondido, ajustesFundo

    var id: String { rawValue }
    var chave: String { "ritmo." + rawValue }

    /// Os valores medidos em 19/10 (spec-inicio.md): o que custa é o redesenho, não a leitura.
    var padrao: TimeInterval {
        switch self {
        case .tempoReal: 1
        case .purgAtivo: 10
        case .purgFundo: 60
        case .ajustesEscondido: 600
        // ~58 s de CPU e ~400 MB do macOS por abertura (medido em 19/10): em segundo plano, só se pedir.
        case .ajustesFundo: 0
        }
    }

    var titulo: String {
        switch self {
        case .tempoReal: String(localized: "Livre, uso e swap")
        case .purgAtivo: String(localized: "O macOS libera sozinho, com o app aberto")
        case .purgFundo: String(localized: "O macOS libera sozinho, em segundo plano")
        case .ajustesEscondido: String(localized: "Abrir o Ajustes escondido, com o app aberto")
        case .ajustesFundo: String(localized: "Abrir o Ajustes escondido, em segundo plano")
        }
    }

    /// 0 = desligado.
    var opcoes: [TimeInterval] {
        switch self {
        case .tempoReal: [1, 2, 5, 10, 30, 0]
        case .purgAtivo: [5, 10, 30, 60, 0]
        case .purgFundo: [30, 60, 300, 600, 0]
        case .ajustesEscondido: [300, 600, 1800, 3600, 0]
        case .ajustesFundo: [600, 1800, 3600, 0]
        }
    }

    var valor: TimeInterval { Self.valor(self, UserDefaults.standard) }

    static func valor(_ r: Ritmo, _ d: UserDefaults) -> TimeInterval {
        guard let v = d.object(forKey: r.chave) as? Double, r.opcoes.contains(v) else { return r.padrao }
        return v
    }

    static func rotulo(_ s: TimeInterval) -> String {
        switch s {
        case 0: String(localized: "Desligado")
        case ..<60: String(localized: "\(Int(s)) s")
        case ..<3600: String(localized: "\(Int(s / 60)) min")
        default: String(localized: "\(Int(s / 3600)) h")
        }
    }
}

struct RitmosView: View {
    var body: some View {
        ForEach(Ritmo.allCases) { LinhaRitmo(ritmo: $0) }
    }
}

private struct LinhaRitmo: View {
    let ritmo: Ritmo
    @AppStorage private var valor: Double

    init(ritmo: Ritmo) {
        self.ritmo = ritmo
        _valor = AppStorage(wrappedValue: ritmo.padrao, ritmo.chave)
    }

    var body: some View {
        Picker(ritmo.titulo, selection: $valor) {
            // O padrão de cada ritmo leva "(Padrão)" ao lado.
            ForEach(ritmo.opcoes, id: \.self) {
                let r = Ritmo.rotulo($0)
                Text($0 == ritmo.padrao ? String(localized: "\(r) (Padrão)") : r).tag($0)
            }
        }
    }
}

// Padrão de crescimento de uma pasta pelas amostras por hora do automático (PontoPastas): só o que
// foi medido na janela, com a inclinação e de quando a quando; sem previsão de quando o disco enche.
//   padraoCrescimento(_:)   pura, testada (CrescimentoTests)
//   Crescimento.etiqueta, .detalhe   os textos da linha da pasta no Histórico
import Foundation

nonisolated enum PadraoCrescimento: Equatable {
    case cresceSempre, crescePicos, estavel, encolhe, insuficiente
}

nonisolated struct Crescimento: Equatable {
    let padrao: PadraoCrescimento
    /// Inclinação medida (mínimos quadrados), em bytes por dia; nil com dados insuficientes.
    let porDia: Double?
    let de, ate: Date?
    let amostras: Int
}

/// Mínimo: 6 amostras cobrindo 24 h ou mais. Estável: o último menos o primeiro abaixo de 100 MB e de
/// 2 % do tamanho. Subindo: "em picos" quando os 10 % maiores passos (pelo menos 1) fazem 60 % ou
/// mais da subida; senão "sempre". Uma hora em que a pasta não estava entre as maiores não entra:
/// ausente não é zero.
nonisolated func padraoCrescimento(_ serie: [(d: Date, b: Int64)]) -> Crescimento {
    let s = serie.sorted { $0.d < $1.d }
    guard s.count >= 6, let p = s.first, let u = s.last, u.d.timeIntervalSince(p.d) >= 86_400 else {
        return Crescimento(padrao: .insuficiente, porDia: nil, de: s.first?.d, ate: s.last?.d, amostras: s.count)
    }
    let xs = s.map { $0.d.timeIntervalSince(p.d) / 86_400 }, ys = s.map { Double($0.b) }
    let n = Double(s.count), mx = xs.reduce(0, +) / n, my = ys.reduce(0, +) / n
    let sxy = zip(xs, ys).reduce(0) { $0 + ($1.0 - mx) * ($1.1 - my) }
    let sxx = xs.reduce(0) { $0 + ($1 - mx) * ($1 - mx) }
    let inclinacao = sxx > 0 ? sxy / sxx : 0
    let delta = Double(u.b - p.b)
    let padrao: PadraoCrescimento
    if abs(delta) < max(100_000_000, 0.02 * Double(max(p.b, u.b))) {
        padrao = .estavel
    } else if delta < 0 {
        padrao = .encolhe
    } else {
        let passos = zip(s, s.dropFirst()).map { Double($1.b - $0.b) }
        let sobe = passos.filter { $0 > 0 }.sorted(by: >)
        let k = max(1, Int((Double(passos.count) * 0.1).rounded(.up)))
        let total = sobe.reduce(0, +)
        padrao = total > 0 && sobe.prefix(k).reduce(0, +) / total >= 0.6 ? .crescePicos : .cresceSempre
    }
    return Crescimento(padrao: padrao, porDia: inclinacao, de: p.d, ate: u.d, amostras: s.count)
}

extension Crescimento {
    var etiqueta: String {
        switch padrao {
        case .cresceSempre: String(localized: "Cresce sempre")
        case .crescePicos: String(localized: "Cresce em picos")
        case .estavel: String(localized: "Estável")
        case .encolhe: String(localized: "Encolhe")
        case .insuficiente: String(localized: "Dados insuficientes")
        }
    }

    /// A inclinação medida e a janela; com dados insuficientes, quantas amostras havia.
    var detalhe: String {
        guard let porDia, let de, let ate else {
            return String(localized: "Menos de 6 amostras em 24 h para dizer um padrão (amostras: \(amostras)).")
        }
        let f = Date.FormatStyle.dateTime.day(.twoDigits).month(.twoDigits).hour().minute().locale(localApp)
        let taxa = (porDia >= 0 ? "+" : "−") + gb(Int64(abs(porDia)))
        return String(localized: "\(taxa) por dia, medido de \(de.formatted(f)) a \(ate.formatted(f)) (amostras: \(amostras)).")
    }
}

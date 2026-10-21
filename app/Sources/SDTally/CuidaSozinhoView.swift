// Liberado automaticamente, antes "O que o macOS resolve sozinho" (spec-fase6 §2.2 item 12, §4.8, §5.3; B4): absorve o "por
// serviço" do purgeable. Dono: A10a. API estável:
//   cuidaSozinho(completo:rapido:lixeiraAuto:) -> (titulo, linhas)   pura
//   backupParado(_:)        `tm_backup_stuck` em advice
//   lixeiraAutomatica()        FXRemoveOldTrashItems do Finder; ausente dá nil ("não definido")
//   CuidaSozinhoView()      a segunda parte do cartão Livre, sem Bloco próprio
import Foundation
import SwiftUI

nonisolated struct LinhaCuida: Sendable, Equatable {
    let simbolo: String
    let nome: String
    var detalhe: String? = nil
    /// Linha sem número quando `fonte` é nil.
    var bytes: Int64? = nil
    var fonte: String? = nil
    var estado = "measured"
}

nonisolated func backupParado(_ c: Completo?) -> Bool {
    c?.advice?.contains { $0.id == "tm_backup_stuck" } ?? false
}

/// `true`/`false` pela preferência do Finder; nil quando nunca foi definida.
nonisolated func lixeiraAutomatica() -> Bool? {
    (CFPreferencesCopyAppValue("FXRemoveOldTrashItems" as CFString, "com.apple.finder" as CFString) as? NSNumber)?.boolValue
}

private nonisolated let servicoTM = "com.apple.TMCacheDelete"

/// O título é o `purgeable.bytes`, nunca somado aos snapshots (o serviço do Time Machine é a
/// própria linha de snapshots). Com o backup parado, o Time Machine sai do título e fica à parte.
nonisolated func cuidaSozinho(completo c: Completo?, rapido r: Rapido?, lixeiraAuto: Bool?) -> (titulo: Int64?, linhas: [LinhaCuida]) {
    let parado = backupParado(c)
    let p = r?.purgeable ?? c?.informational?.purgeable
    let servicos = (p?.topServices ?? []).filter { $0.bytes > 0 }
    let tm = servicos.first { $0.name == servicoTM }
    let titulo = p?.bytes.map { $0 - (parado ? tm?.bytes ?? 0 : 0) }
    let fichas = r?.cards ?? c?.cards
    let snaps = (c?.informational?.snapshots ?? []).filter { $0.kind == "time_machine" }

    var ls: [LinhaCuida] = servicos.filter { $0.name != servicoTM || snaps.isEmpty }.map {
        LinhaCuida(simbolo: "gearshape", nome: nomeServico($0.name), detalhe: fichas?[$0.card]?.what.texto, bytes: $0.bytes, fonte: p?.source)
    }
    // A linha de snapshots carrega os bytes do serviço do Time Machine (são os mesmos).
    if let (antigo, data) = snaps.compactMap({ s in s.date.map { (s, $0) } }).min(by: { $0.1 < $1.1 }) {
        let base = snaps.first { $0.tmRole == "reference" } ?? antigo
        let detalhe = !parado ? String(localized: "O Time Machine costuma removê-los em 24 h (regra da Apple, não medição).")
            : base.purgeable == true ? String(localized: "O mais antigo é a base do próximo backup. Se faltar espaço, o macOS pode apagá-lo; aí o próximo backup demora mais.")
            : String(localized: "O mais antigo é a base do próximo backup.")
        // `detalhe` das linhas é tooltip (Linha.dica), nunca texto visível.
        ls.append(LinhaCuida(simbolo: "clock.arrow.circlepath",
                             nome: String(localized: "Snapshots do Time Machine: \(snaps.count), o mais antigo de \(Armazenamento.dataCurta(data))"),
                             detalhe: detalhe, bytes: tm?.bytes, fonte: tm == nil ? nil : p?.source))
    }
    if let s = r?.swap ?? c?.swap {
        ls.append(LinhaCuida(simbolo: "arrow.left.arrow.right", nome: String(localized: "Swap"), detalhe: String(localized: "Sai ao reiniciar e volta com o uso."),
                             bytes: s.total, fonte: "sysctl_vm_swapusage", estado: s.total == nil ? "not_measured" : "measured"))
    }
    if let t = c?.dataVolume.terms.first(where: { $0.key == "deleted_open" }) {
        ls.append(LinhaCuida(simbolo: "doc.badge.ellipsis", nome: String(localized: "Arquivos apagados que apps ainda seguram"), detalhe: String(localized: "Sai quando o app fecha."),
                             bytes: t.bytes, fonte: t.source, estado: t.state))
    }
    let lixeira = lixeiraAuto.map { $0 ? String(localized: "Lixo automático (remover após 30 dias): ligado") : String(localized: "Lixo automático (remover após 30 dias): desligado") }
        ?? String(localized: "Lixo automático (remover após 30 dias): não definido")
    ls.append(LinhaCuida(simbolo: "trash", nome: lixeira,
                         detalhe: String(localized: "Quanto já passou de 30 dias: não medido.")))
    let atualizacao = (c?.top ?? []).filter { $0.card == "asset_os_update" }
    if !atualizacao.isEmpty {
        ls.append(LinhaCuida(simbolo: "arrow.down.circle", nome: String(localized: "Atualização do macOS baixada"),
                             bytes: atualizacao.reduce(0) { $0 + $1.bytes }, fonte: "top"))
    }
    return (titulo, ls)
}

struct CuidaSozinhoView: View {
    @Environment(Painel.self) private var painel
    @State private var lixeiraAuto: Bool?

    var body: some View {
        let c = painel.completo
        if let p = painel.purgVivo ?? c?.informational?.purgeable {
            let (titulo, linhas) = cuidaSozinho(completo: c, rapido: painel.rapidoVivo, lixeiraAuto: lixeiraAuto)
            // Parte do cartão Livre (LivresView): sem Bloco próprio, abaixo de uma divisória.
            Divider()
            VStack(alignment: .leading, spacing: 12) {
                Text("Liberado automaticamente").font(.headline)
                    .help("Não pede nada de você. O purgeable e os snapshots não se somam: o serviço do Time Machine é a própria linha de snapshots.")
                if let titulo { Text(gb(titulo)).font(.kpi) } else { Text(p.reason.map { String(localized: "Não medido: \($0)") } ?? String(localized: "Não medido")).foregroundStyle(.secondary) }
                grade {
                    ForEach(Array(linhas.enumerated()), id: \.offset) { _, l in
                        Linha(simbolo: l.simbolo, nome: l.nome, dica: l.detalhe) {
                            if let f = l.fonte { NumeroView(bytes: l.bytes, source: f, state: l.estado) }
                        }
                    }
                }
            }
            .onAppear { lixeiraAuto = lixeiraAutomatica() }
        }
    }
}

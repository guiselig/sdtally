// Vale a pena reiniciar? (spec-fase6 §2.2 item 11a, §4.8, §5.5; A11). Dono: A10a. API estável:
//   reiniciar(completo:bootTime:agora:) -> CartaoReinicio   pura
//   ReiniciarView(c:)
import Foundation
import SwiftUI

nonisolated struct LinhaReinicio: Sendable, Equatable {
    let nome: String
    var detalhe: String? = nil
    let bytes: Int64?
    let fonte: String
    var estado = "measured"
    /// Medido, mas sem número que faça sentido (VM − swap negativo): mostra "—".
    var traco = false
}

nonisolated struct CartaoReinicio: Sendable, Equatable {
    /// Coluna 1: o que sai ao reiniciar.
    let libera: [LinhaReinicio]
    /// Coluna 2: lógico pelo lsof, fora de qualquer soma. nil: a lista de apagados não foi medida.
    let podeLiberar: [LinhaReinicio]?
    /// Coluna 3: o que reiniciar não muda.
    let naoMuda: [LinhaReinicio]
    let rodape: String
}

/// `bootTime`: o do rápido, mais novo; sem ele, o da medição completa.
nonisolated func reiniciar(completo c: Completo, bootTime: String? = nil, agora: Date = .now) -> CartaoReinicio {
    let termo = { (k: String) in c.dataVolume.terms.first { $0.key == k } }
    var libera = [LinhaReinicio(nome: String(localized: "Swap"), detalhe: String(localized: "Sai da linha macOS do Ajustes"), bytes: c.swap?.total,
                                fonte: "sysctl_vm_swapusage", estado: c.swap?.total == nil ? "not_measured" : "measured")]
    if let t = termo("deleted_open") {
        libera.append(LinhaReinicio(nome: String(localized: "Apagados ainda abertos"), detalhe: String(localized: "Sai de Dados do Sistema"), bytes: t.bytes, fonte: t.source, estado: t.state))
    }
    let pode = c.informational?.deletedOpenProcesses.map { ps in
        ps.compactMap { p in
            (p.logicalBytesWithoutAllocated ?? 0) > 0
                ? LinhaReinicio(nome: visivel(p.name), detalhe: String(localized: "Lógicos pelo lsof; o alocado não é legível nessa pasta do sistema"),
                                bytes: p.logicalBytesWithoutAllocated, fonte: "lsof_proc_pidfdinfo")
                : nil
        }
    }
    var naoMuda: [LinhaReinicio] = []
    if let l = c.top.first(where: { $0.path == "/private/var/vm" }) {
        naoMuda.append(LinhaReinicio(nome: String(localized: "Imagem de repouso"), detalhe: "/private/var/vm", bytes: l.bytes, fonte: l.source))
    }
    if let cs = c.containers {
        let resto = Armazenamento.restoVM(cs, swap: c.swap)
        let medido = Armazenamento.vm(cs) != nil && c.swap?.total != nil
        naoMuda.append(LinhaReinicio(nome: String(localized: "Resto do VM (kernelcore e outros)"), bytes: resto, fonte: "difference",
                                     estado: medido ? "measured" : "not_measured", traco: medido && resto == nil))
    }
    if let t = termo("snapshots") {
        naoMuda.append(LinhaReinicio(nome: nomeTermo("snapshots"), bytes: t.bytes, fonte: t.source, estado: t.state))
    }
    let ligado = (data(bootTime ?? c.bootTime)).map { d -> String in
        let n = Int(agora.timeIntervalSince(d) / 86_400)
        return n < 1 ? String(localized: "Ligado há menos de 1 dia") : String(localized: "Ligado há \(n) dias")
    } ?? ""
    return CartaoReinicio(libera: libera, podeLiberar: pode, naoMuda: naoMuda,
                          rodape: ligado.trimmingCharacters(in: CharacterSet(charactersIn: ". ")))
}

struct ReiniciarView: View {
    @Environment(Painel.self) private var painel
    let c: Completo

    var body: some View {
        let k = reiniciar(completo: c, bootTime: painel.rapido?.bootTime)
        Bloco(titulo: String(localized: "Vale a pena reiniciar?"), dica: String(localized: "Salve o trabalho aberto antes. O swap volta com o uso. Depois de reiniciar, meça de novo: o Histórico mostra quanto voltou.")) {
            coluna(String(localized: "Libera ao reiniciar"), "arrow.uturn.backward.circle", k.libera)
            if c.swap?.total == 0 { VazioLinha(.v9) }
            if let ps = k.podeLiberar, ps.isEmpty {
                subtitulo(String(localized: "Pode liberar (tamanho lógico)"))
                Text(nenhum(fonte: "lsof_proc_pidfdinfo", hora: painel.manual ? (hora(c.generatedAt) as String) : nil)).foregroundStyle(.secondary)
            } else {
                coluna(String(localized: "Pode liberar (tamanho lógico)"), "questionmark.circle", k.podeLiberar ?? [
                    LinhaReinicio(nome: String(localized: "Apagados por processo"), bytes: nil, fonte: "lsof_proc_pidfdinfo", estado: "not_measured"),
                ])
            }
            coluna(String(localized: "Não muda"), "equal.circle", k.naoMuda)
            if !k.rodape.isEmpty { Text(k.rodape).font(.caption).foregroundStyle(.secondary) }
        }
    }

    private func subtitulo(_ s: String) -> some View {
        Text(s).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary)
    }

    @ViewBuilder private func coluna(_ titulo: String, _ simbolo: String, _ ls: [LinhaReinicio]) -> some View {
        subtitulo(titulo)
        grade {
            ForEach(Array(ls.enumerated()), id: \.offset) { _, l in
                Linha(simbolo: simbolo, nome: l.nome, dica: l.detalhe) {
                    if l.traco {
                        Text("—").help("Leituras em momentos diferentes")
                    } else {
                        NumeroView(bytes: l.bytes, source: l.fonte, state: l.estado)
                    }
                }
            }
        }
    }
}

// Dados do Sistema (a primeira tela): tudo o que explica o Dados do Sistema, conforme o
// alternador Automático | Manual (painel.dados). No Automático, ao vivo pelo autoVivo (sdtally
// --auto, sem senha), com o cartão de tempo real no topo; sem ele ainda, o aviso da primeira
// passada (~1 min). No Manual, a última Medição completa. De cima para baixo: o antigo Início
// (CartaoInicio), a conta do volume Data (desdobra por termo), as partes do sistema, os
// snapshots, os apagados ainda abertos e o "Vale a pena reiniciar?".
import SwiftUI

struct SistemaView: View {
    @Environment(Painel.self) private var painel

    var body: some View {
        let c = painel.dados
        // Seções com .id(<âncora>): os links do Início e SDTALLY_ANCORA rolam até elas.
        ScrollViewReader { proxy in
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if !painel.soAuto {
                    // No Manual, enquanto mede, o cartão diz que está medindo (com o cronômetro) no
                    // lugar do convite para medir com senha.
                    if painel.manual && painel.medindo {
                        CartaoTempoReal(simbolo: "hourglass", titulo: String(localized: "Medindo o disco"),
                                        texto: String(localized: "A medição completa percorre o disco arquivo por arquivo e leva alguns minutos. Os números novos aparecem quando ela terminar."))
                    } else if !painel.manual {
                        CartaoTempoReal()
                    } else if c == nil {
                        CartaoTempoReal(simbolo: "lock.open", titulo: String(localized: "Nenhuma medição completa"),
                                        texto: String(localized: "A medição com senha entra nas pastas do sistema e de outros usuários e mede arquivo por arquivo: valores mais completos e precisos que os do Automático."))
                    }
                }
                CartaoInicio()
                if let c {
                    // No Automático, até a primeira linha nova, os números são os da execução anterior.
                    if !painel.manual, let d = painel.idadeAuto {
                        Faixa(simbolo: "clock", texto: String(localized: "Números da última leitura automática (\(idade(d))). Os de agora chegam quando a primeira passada terminar, em cerca de 1 minuto."))
                    }
                    BarraDesdobravel(raiz: .volumeData, fonte: c, aoVivo: !painel.manual).id("barra")
                    PartesDoSistema(c: c).id("partes")
                    if let i = c.informational {
                        SnapshotsView(c: c).id("snapshots")
                        ApagadosAbertos(i: i, root: c.root)
                        ReiniciarView(c: c).id("reiniciar")
                    }
                }
            }
            .padding(24)
            .frame(maxWidth: 760)
            .frame(maxWidth: .infinity)
        }
        .task(id: c != nil) {
            guard c != nil, let a = painel.rolarPara ?? Foto.ancora ?? Foto.barra.map({ _ in "barra" }) else { return }
            painel.rolarPara = nil
            try? await Task.sleep(for: .milliseconds(300))
            proxy.scrollTo(a, anchor: .top)
        }
        }
    }
}

/// Topo do Automático: os números são ao vivo; a medição com senha (Manual) é mais completa.
/// O botão leva ao Manual na hora, onde corre o cronômetro; senha cancelada volta ao Automático.
/// No Manual sem medição, o mesmo cartão com o texto do que a medição com senha traz.
private struct CartaoTempoReal: View {
    @Environment(Painel.self) private var painel
    var simbolo = "dot.radiowaves.left.and.right"
    var titulo = String(localized: "Valores atualizados em tempo real")
    var texto = String(localized: "Para valores mais completos e maior precisão, faça uma medição manual.")

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            Image(systemName: simbolo).font(.title3).foregroundStyle(.secondary)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text(titulo).font(.headline)
                Text(texto)
                    .foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 8)
            acao
        }
        .cartao()
    }

    @ViewBuilder private var acao: some View {
        switch painel.medicao {
        case .medindo(let desde, _, _):
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                TimelineView(.periodic(from: desde, by: 1)) { ctx in
                    let s = max(0, Int(ctx.date.timeIntervalSince(desde)))
                    Text(String(localized: "Medindo… \(s / 60):\(String(format: "%02d", s % 60))")).monospacedDigit()
                }
                if painel.podeParar { Button("Parar") { painel.parar() } }
            }
        case .falhou(let msg):
            VStack(alignment: .trailing, spacing: 4) {
                botao(String(localized: "Tentar de novo"))
                Text(msg).font(.caption).foregroundStyle(Marca.aviso).lineLimit(2).textSelection(.enabled)
            }
        case .senhaCancelada, .parada:
            botao(String(localized: "Medir com senha"))
        }
    }

    private func botao(_ t: String) -> some View {
        Button(t) { painel.medirComSenha() }
            .buttonStyle(.borderedProminent)
            .help(painel.helperAtivo ? String(localized: "Mede como administrador pelo Root sem Senha, sem pedir a senha.") : String(localized: "Pastas do sistema e de outros usuários só abrem como administrador."))
    }
}

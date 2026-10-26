// SDTallyUpdater: o executor da atualização. Sem rede, sem interface, sem notificação e sem
// relógio: vai em SDTally.app/Contents/Helpers e só roda depois do clique em Instalar, com o DMG
// já baixado, conferido e montado pelo app (app/Sources/SDTally/Atualizacao). O app o copia para a
// pasta temporária da instalação antes de abrir, porque o bundle onde ele mora vai ser trocado.
//   SDTallyUpdater <SDTally.app> <DMG montado>/SDTally.app <pid do SDTally> <DMG montado>
// Espera o SDTally fechar, confere de novo, troca (Instalacao.executar), desmonta o DMG, apaga a
// própria pasta temporária e abre o SDTally. Falhou: o app antigo fica como estava e abre de novo;
// o motivo vai em Resultado, que o app lê ao abrir.
import Foundation
import SDTallyInstalacao

let pasta = URL(filePath: CommandLine.arguments[0]).deletingLastPathComponent()
let pedido: Pedido
do {
    pedido = try Pedido.ler(Array(CommandLine.arguments.dropFirst()))
} catch {
    // Sem um destino conferido, não abre nada.
    Resultado(versao: "", erro: .argumentos).gravar()
    exit(64)
}
let versao = Instalacao.versao(pedido.novo) ?? ""
let erro = Instalacao.executar(destino: pedido.destino, novo: pedido.novo, pid: pedido.pid)
Instalacao.desanexar(pedido.montagem)
Resultado(versao: versao, erro: erro).gravar()
// Só a pasta desta instalação (o DMG e esta cópia do executor), nunca outra.
if pasta.lastPathComponent.hasPrefix(Instalacao.prefixoTemporario) { Instalacao.apagarTemporario(pasta) }
// O SDTally que não fechou a tempo continua aberto: não abre outro.
if erro != .aberto { _ = Instalacao.lancarSolto(URL(filePath: "/usr/bin/open"), [pedido.destino.path]) }
exit(erro == nil ? 0 : 1)

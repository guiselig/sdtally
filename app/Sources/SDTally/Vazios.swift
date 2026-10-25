// Estados vazios (spec-fase6 §5.6, G3). Textos finais em pt-BR, puros e testados contra o G9.
// API estável para os marcos A9–A12:
//   Vazio (.v1 … .v23): .titulo, .texto, .simbolo, .acao: AcaoVazio?
//   VazioTela(vazio) { ação }   tela inteira (ContentUnavailableView)
//   VazioLinha(vazio) { ação }  em linha, com a Faixa
//   nenhum(fonte:hora:) -> String          lista vazia medida: "Nenhum (fonte, medido às HH:MM)."
//   valorOuNaoMedido(_ bytes: Int64?) -> String   nunca "0" no lugar de "não medido"
//   motivoNaoConferido(_ reason: String) -> String   `advice_unchecked[].reason` em pt
//   linhaNaoConferido(nome:reason:) -> String       "<nome da ficha>: <motivo em pt>"
// Quem mostra decide o que a ação faz (abrir Ajustes, medir, ir para Permissões): aqui só o rótulo.
import SwiftUI

/// O que o botão de um estado vazio faz; o rótulo é fixo.
nonisolated enum AcaoVazio: Sendable, Equatable {
    case abrirArmazenamento, permissoes, medir, medirDeNovo, verArmazenamento, medidorRoot

    var rotulo: String {
        switch self {
        case .abrirArmazenamento: String(localized: "Abrir Armazenamento no Ajustes")
        case .permissoes: String(localized: "Permissões")
        case .medir: String(localized: "Calcular com senha…")
        case .medirDeNovo: String(localized: "Calcular de novo")
        case .verArmazenamento: String(localized: "Ver em Dados do Sistema")
        case .medidorRoot: String(localized: "Calcular como administrador")
        }
    }
}

nonisolated enum Vazio: Sendable, Equatable {
    /// `screen.state == not_measured`; `ultimo`/`hora`: o último Dados do Sistema lido, se houver.
    case v1(ultimo: Int64?, hora: String?)
    /// `screen.state == partial`.
    case v2
    /// `log show` falhou sem admin.
    case v3
    /// Só o rápido: `usadoData` = `data_volume.used.bytes` do rápido.
    case v5(usadoData: Int64?)
    /// `snapshots == []`, medido às `hora`.
    /// `hora` nil no Automático: é ao vivo, sem hora de medição.
    case v6(hora: String?)
    /// `snapshots == null`.
    case v7(motivo: String)
    /// `deleted_open_processes == []`.
    case v8(root: Bool)
    /// `swap.total == 0`.
    case v9
    /// `incomplete_reasons ∋ no_root`: Bloqueado pelo macOS de hoje e, se houver, a medição com senha do histórico.
    case v12(semExplicacao: Int64?, porcento: Double?, dataComSenha: String?, bytesComSenha: Int64?)
    /// `time_machine.state == not_measured` com `reason == "plist unreadable"`.
    case v16
    /// Sem itens `safe`, `cost` ou `check_first`.
    case v17
    /// `advice_unchecked != []`: título da seção; as linhas saem de `linhaNaoConferido`.
    case v18
    /// O que fazer sem medição completa.
    case v20
    /// Barra de menus sem tela e sem `Ponto`.
    case v23

    var titulo: String {
        switch self {
        case .v1: String(localized: "O Ajustes não está em Armazenamento")
        case .v2: String(localized: "O Ajustes ainda está calculando")
        case .v3: String(localized: "Precisa de uma conta de administrador")
        case .v5: String(localized: "Só a leitura rápida")
        case .v6: String(localized: "Nenhum snapshot local")
        case .v7: String(localized: "Snapshots não medidos")
        case .v8: String(localized: "Nenhum apagado ainda aberto")
        case .v9: String(localized: "Sem swap agora")
        case .v12: String(localized: "Medido sem senha")
        case .v16: String(localized: "Estado do Time Machine não lido")
        case .v17: String(localized: "Nada passou de 1 GB")
        case .v18: String(localized: "O que não deu para conferir")
        case .v20: String(localized: "Falta a medição completa")
        case .v23: String(localized: "Dados do Sistema ainda não lido")
        }
    }

    var texto: String {
        switch self {
        case let .v1(ultimo, h):
            ultimo.map { u in
                h.map { String(localized: "Último lido: \(gb(u)) às \($0).") } ?? String(localized: "Último lido: \(gb(u)).")
            } ?? String(localized: "Ainda não lido.")
        case .v2: String(localized: "O Ajustes ainda está calculando.")
        case .v3: String(localized: "Só uma conta de administrador lê o Ajustes.")
        case let .v5(u): String(localized: "Usado do volume Data agora: \(valorOuNaoMedido(u)) (volume APFS).")
        case let .v6(h): h.map { String(localized: "Nenhum snapshot local (medido às \($0)).") } ?? String(localized: "Nenhum snapshot local.")
        case let .v7(m): String(localized: "Não medido: \(m).")
        case let .v8(root):
            root ? String(localized: "Nenhum app segurando arquivo apagado (medido).")
                : String(localized: "Nenhum app segurando arquivo apagado (medido). Sem senha: só seus processos.")
        case .v9: String(localized: "Sem swap agora.")
        case let .v12(b, p, data, antes):
            ([
                p.map { String(localized: "Bloqueado pelo macOS: \(valorOuNaoMedido(b)) (\($0.formatted(.number.precision(.fractionLength(0...1)).locale(localApp))) %), sem senha.") }
                    ?? String(localized: "Bloqueado pelo macOS: \(valorOuNaoMedido(b)), sem senha."),
                String(localized: "Medir com senha pode reduzir isto."),
                data.flatMap { d in antes.map { String(localized: "Com senha, em \(d): \(gb($0)).") } },
            ] as [String?]).compactMap { $0 }.joined(separator: " ")
        case .v16: String(localized: "Não deu para ler as preferências do Time Machine.")
        case .v17: String(localized: "Nada passou de 1 GB.")
        case .v18: String(localized: "O que não deu para conferir")
        case .v20: String(localized: "Sem medição completa.")
        case .v23: String(localized: "Dados do Sistema não lido.")
        }
    }

    var simbolo: String {
        switch self {
        case .v1, .v2, .v23: "gearshape"
        case .v3: "person.badge.key"
        case .v5, .v20: "gauge.with.dots.needle.33percent"
        case .v6: "clock.arrow.circlepath"
        case .v7, .v16: "circle.dashed"
        case .v8: "doc.badge.ellipsis"
        case .v9: "memorychip"
        case .v12: "lock.open"
        case .v17: "checkmark.circle"
        case .v18: "questionmark.circle"
        }
    }

    var acao: AcaoVazio? {
        switch self {
        case .v1: .abrirArmazenamento
        case .v3: .permissoes
        default: nil
        }
    }
}

extension Vazio {
    /// V12 de uma medição sem senha; `comSenha`: a medição com senha mais nova do histórico.
    static func v12(_ c: Completo, comSenha r: Completo?) -> Vazio {
        let u = c.dataVolume.unexplained
        return .v12(semExplicacao: u.bytes, porcento: u.percentOfUsed,
                    dataComSenha: r.map { Armazenamento.diaCurto($0.generatedAt) },
                    bytesComSenha: r?.dataVolume.unexplained.bytes)
    }
}

/// Lista vazia **medida**: diz "Nenhum", com a fonte e a hora.
/// `hora` nil no Automático: é ao vivo, sem hora de medição.
nonisolated func nenhum(fonte: String, hora h: String?) -> String {
    h.map { String(localized: "Nenhum (\(nomeFonte(fonte)), medido às \($0)).") } ?? String(localized: "Nenhum (\(nomeFonte(fonte))).")
}

/// O resto só é "bloqueado" quando uma permissão o reduziria (sem root ou sem Acesso Total). Com os
/// dois, nenhuma permissão reduz: pastas protegidas até do administrador, partes parciais e o que
/// mudou durante a medição viram "Não medido" (feedback do Reddit; dono, 24/10).
nonisolated func restoReduzivel(_ c: Completo) -> Bool { !c.root || c.fullDiskAccess != true }

nonisolated func nomeResto(_ c: Completo) -> String {
    restoReduzivel(c) ? String(localized: "Bloqueado pelo macOS") : String(localized: "Não medido")
}

/// B4: a frase do Bloqueado pelo macOS (ou do Não medido, com root e Acesso Total). Nunca negativa: se as partes passaram do usado (dir-stats das
/// pastas protegidas conta o que a travessia já contou), o número não existe e a frase diz isso.
nonisolated func fraseSemExplicacao(_ c: Completo) -> String {
    let u = c.dataVolume.unexplained
    // A sobreposição (partes medidas acima do usado) fica no ⓘ, em explicarBloqueado.
    let v = valorOuNaoMedido(u.bytes)
    let p = u.percentOfUsed.map { $0.formatted(.number.precision(.fractionLength(1)).locale(localApp)) }
    guard restoReduzivel(c) else {
        return p.map { String(localized: "Não medido: \(v) (\($0) % do usado)") } ?? String(localized: "Não medido: \(v)")
    }
    return p.map { String(localized: "Bloqueado pelo macOS: \(v) (\($0) % do usado)") } ?? String(localized: "Bloqueado pelo macOS: \(v)")
}

/// O ⓘ ao lado do Bloqueado pelo macOS: por que o número existe e o que ficou sem tamanho.
nonisolated func explicarBloqueado(_ c: Completo) -> (texto: [String], pastas: [String]) {
    var t = [c.root
        ? String(localized: "O macOS protege parte do disco (Proteção da Integridade do Sistema e cofres de dados): nem o administrador lê essas pastas. É o que sobra do usado depois de somar tudo o que deu para medir.")
        : String(localized: "Sem a senha, as pastas do sistema e de outros usuários não abrem. Com a senha, boa parte disso passa a ter nome; o resto o macOS bloqueia até para o administrador.")]
    let u = c.dataVolume.unexplained
    if u.bytes == nil, let o = u.overlapBytes, o > 0 {
        t.append(String(localized: "Desta vez não dá para medir: as partes medidas passam do usado em \(gb(o)), então o que sobra sairia negativo. Acontece quando clones e hardlinks contam mais de uma vez, ou quando o disco muda durante a medição."))
    }
    let inf = c.informational
    let n = { (v: Int64) in v.formatted(.number.locale(localApp)) }
    if let s = inf?.protectedWithoutSize, s.count > 0 {
        t.append(s.count == 1 ? String(localized: "1 pasta protegida sem tamanho: nem o APFS informou.") : String(localized: "\(s.count) pastas protegidas sem tamanho: nem o APFS informou."))
    }
    let semAlocado = (inf?.deletedOpenProcesses ?? []).reduce(Int64(0)) { $0 + ($1.filesWithoutAllocated ?? 0) }
    if semAlocado > 0 { t.append(String(localized: "\(n(semAlocado)) arquivos apagados ainda abertos sem tamanho legível.")) }
    if let x = c.dataVolume.terms.first(where: { $0.key == "large_xattrs" }), x.state == "partial",
       let q = x.reason?.split(separator: " ").first.flatMap({ Int64($0) }), q > 0 {
        t.append(String(localized: "\(n(q)) objetos com atributos estendidos ilegíveis."))
    }
    t.append(String(localized: "Metadados dos snapshots, que o macOS não informa."))
    return (t, inf?.protectedWithoutSize?.folders.map { visivel($0.path) } ?? [])
}

/// O que não foi medido nunca aparece como "0".
nonisolated func valorOuNaoMedido(_ bytes: Int64?) -> String {
    bytes.map(gb) ?? String(localized: "não medido")
}

/// `advice_unchecked[].reason` em pt; motivo desconhecido aparece cru.
nonisolated func motivoNaoConferido(_ reason: String) -> String {
    switch reason {
    case "term not measured": String(localized: "a parte da conta não foi medida")
    case "no lsof": String(localized: "a lista de apagados não rodou")
    case "plist unreadable": String(localized: "o estado do Time Machine não pôde ser lido")
    case "tm status unreadable": String(localized: "não deu para saber se o Time Machine está fazendo backup agora")
    default: reason
    }
}

/// Linha de "O que não deu para conferir" (V18).
nonisolated func linhaNaoConferido(nome: String, reason: String) -> String {
    String(localized: "\(nome): \(motivoNaoConferido(reason))")
}

/// Tela inteira vazia. A ação vem de quem chama (o rótulo está em `vazio.acao?.rotulo`).
struct VazioTela<Acao: View>: View {
    let vazio: Vazio
    @ViewBuilder var acao: Acao

    var body: some View {
        ContentUnavailableView {
            Label(vazio.titulo, systemImage: vazio.simbolo)
        } description: {
            Text(vazio.texto)
        } actions: {
            acao
        }
    }
}

/// Estado vazio em linha, com a Faixa (tom neutro: vazio não é aviso).
struct VazioLinha<Acao: View>: View {
    let vazio: Vazio
    @ViewBuilder var acao: Acao

    var body: some View {
        Faixa(simbolo: vazio.simbolo, tom: .secondary, texto: vazio.texto) { acao }
    }
}

extension VazioTela where Acao == EmptyView {
    init(_ vazio: Vazio) { self.init(vazio: vazio) { EmptyView() } }
}

extension VazioLinha where Acao == EmptyView {
    init(_ vazio: Vazio) { self.init(vazio: vazio) { EmptyView() } }
}

# Spec final: Início e barra lateral do SDTally (fase 6, depois da fase 5)

Base: a proposta "resposta", que levou os 3 votos, com as ideias boas de "ajustes" e "saude" que os juízes apontaram. Os problemas que os juízes viram estão resolvidos no texto abaixo.

Histórico: registro do desenho da fase 6; o código atual manda onde divergir.

**Marcas usadas no texto**
- **‡**: depende do detector do Time Machine corrigido (M2).
- **†**: depende de `top[].settings_line` no CLI (M5).
- **§**: depende de fichas novas (M7).

Até esses marcos ficarem prontos, o que eles alimentam fica **escondido**. Nunca aparece "?".

---

## Regra do app: sem subtextos (vale sobre tudo o que está abaixo e nas outras specs)

O dono pediu: as coisas se explicam sozinhas, sem textões. Em nenhuma tela do app há parágrafo ou linha cinza que explique como algo funciona ou por que o número é assim. Fica: título, número, nome curto, botão, estado curto ("Medindo…", "Não medido", "Medido há 3 dias") e o aviso que muda o que a pessoa faz, numa frase curta. Informação que não pode sumir (fonte, hora, parcial/≥, de onde vem a conta) vai para o `.help` do próprio título, número ou linha, ou fica no ⓘ que já existia; `Bloco` e `Linha` têm `dica:` para isso, e `Bloco` não tem mais legenda. Os textos longos que seguem neste documento descrevem o desenho antigo: onde disserem "texto fixo", "legenda", "rodapé" ou "parágrafo", leia "tooltip". O texto da CLI (`src/`) e o relatório em PDF não mudam. Mantêm-se os testes G9, `nunca_apaga` e "sem ?".

Onde foi cada coisa do Início: "Não é uma pasta…" → `.help` do título Dados do Sistema; "O SDTally mediu estas partes", "As partes não somam…" → saíram; "Ao todo, as pastas medidas…" → `.help` de "Ver todas as partes ›"; a nota dos snapshots (o macOS libera sozinho X) → `.help` da parte; "Por que a senha?" → `.help` do botão de medir; "Se faltar espaço…" → "O macOS libera sozinho até X ›"; manchete R1: "Time Machine sem backup concluído desde 01/10 (11 tentativas). Snapshots: 49,62 GB."; manchete R2: "Maior pedaço: X, 145,18 GB (39,7 % do usado)."

---

## 0. Decisões em uma linha cada

1. **Número principal: o Livre (statfs).** É o único que existe sempre. Nos dois Macs reais, o número do Ajustes estava sem leitura na maior parte do tempo.
2. **Manchete de uma frase**, logo abaixo do Livre, escolhida por regra fixa. A ideia veio da "saude" e da "ajustes". Mostra só o que o CLI detectou ou mediu.
3. **Cartão "Dados do Sistema" logo em segundo.** Diz o que é, mostra o número do Ajustes e as partes medidas. Termina na linha de medição, que é o **único lugar da senha**.
4. **"O que mais ocupa o disco":** uma barra e 5 linhas, cada uma numa linha só. A lista serve de legenda da barra (mesma cor, mesmo destaque ao passar o mouse). O detalhe abre ao clicar.
5. **"Libera se apagar" sai do Início.** Fica só em O que fazer e em Pastas maiores. Motivos:
   - o `private_bytes` vem 0 na VM do Claude, no My Drive e em AssetsV2;
   - mostrar "dividido com snapshots" empurra a pessoa a apagar snapshot.
6. **Barra lateral só de ícones, com 5 itens.** Permissões e Sobre viram ícones ⚙ e ⓘ no rodapé, junto do Github; Medir fica abaixo das seções, depois de uma divisória. A seção "Armazenamento" passa a se chamar **Dados do Sistema** e muda de ordem.
7. **Só cores de sistema (revisão posterior).** Sem magenta, sem hex, sem gradiente, sem opção "cor dos botões": os controles seguem o destaque do macOS. Dados do Sistema é cinza (`Marca.dados`), como no Ajustes, e nunca usa o destaque, que pode ser vermelho (G9). Vale para toda tela nova.
8. **CLI:** corrigir o `tm_parado` e criar `top[].settings_line` com as regras da Frente P. Fichas novas entram com refs conferidas (R6).

---

## 1. Barra lateral final

| ⌘ | Título | `Secao` (o case não muda) | SF Symbol |
|---|---|---|---|
| ⌘1 | **Início** | `.resumo` | `house.fill` |
| ⌘2 | **O que fazer** | `.fazer` | `checklist` |
| | *Detalhes* (Section) | | |
| ⌘3 | **Dados do Sistema** | `.armazenamento` | `square.stack.3d.up.fill` |
| ⌘4 | **Pastas maiores** | `.ranking` | `list.number` |
| ⌘5 | **Histórico** | `.historico` | `chart.xyaxis.line` |

**Revisão (janela fixa, barra só de ícones, sem barra de ferramentas).** Os itens são só o SF Symbol, sem a caixa colorida do `IconeSecao` (que continua nos cartões do Início); `Secao.cor` saiu. Cada item é um botão simples de 44 × 38 pt (símbolo de 15 pt no centro; o nome vai no `.help` e no `accessibilityLabel`), com 4 pt entre eles. A seleção é uma pílula na cor do texto (`Color.primary` a 12 %), não a cor de destaque do sistema: o `List` com `.tint` pintava o item selecionado de magenta sólido.

**Janela.** Como a do Ajustes: largura fixa e altura livre. `JanelaView.tamanho` = 740 × 567 pt (a largura do Ajustes, medida no macOS 27; 567 é a altura mínima, a máxima é a tela), **com** a barra de título, que não existe: `.windowStyle(.hiddenTitleBar)`, `.windowResizability(.contentSize)` e `.windowFullScreenBehavior(.disabled)`. A janela soma 28 pt de barra de título ao conteúdo mesmo escondida (`JanelaView.alturaTitulo`, medida no macOS 26 e 27), então o `.frame` do conteúdo tem 539 pt de altura mínima. Não há `NavigationSplitView`, barra de ferramentas nem título visível: o conteúdo é um `HStack` (barra de 72 pt, divisória de 1 pt, detalhe de 667 pt) e o fundo (material `.sidebar` do sistema na barra, `windowBackgroundColor` no detalhe, `.ignoresSafeArea()`) vai até o topo da janela, por baixo do semáforo. O semáforo (x de 10 a 61,5 pt, y até 22 pt) fica sobre a barra lateral, com a mesma folga de 10 pt da borda ao vermelho e do verde à divisória, e nada é desenhado nos primeiros 36 pt (`JanelaView.topoSemaforo`): os ícones e o detalhe começam abaixo. O detalhe vai dentro de um `VStack`: um `Divider` solto (o do rodapé de O que fazer) segue o eixo da pilha em volta e viraria vertical num `HStack`. Os 667 pt do detalhe cobrem os cartões de 560 pt; o conteúdo mais comprido rola dentro do painel. A tabela de Pastas maiores (e a do detalhe de uma pasta) tem larguras ideais que somam 560 pt; os nomes compridos cortam no meio.

**Medir fica logo abaixo das 5 seções**, depois de uma divisória recuada (não encosta nas bordas). **Rodapé da barra (de cima para baixo).** **Github**, **Sobre** (`info.circle` → `.info`) e **Permissões e aparência** (`gearshape` → `.permissoes`, `.help("Permissões e aparência (⌘,)")`). Github é só a logo oficial (Invertocat, `LogoGithub`: o path de `mark-github-16` do Primer Octicons 19.40.0, MIT, com os arcos do SVG convertidos em curvas), 19 pt como os outros ícones redondos, na cor do texto, e abre `urlRepositorio` no navegador com `NSWorkspace.shared.open`, a mesma ação de Ajuda > Abrir repositório no navegador, só no clique. Sobre e Permissões ganham a pílula quando a seção está aberta.

**Medir** (antes na barra de ferramentas). Um `Menu` sem ação principal: o clique abre "Medir com senha de administrador…" (ou "Medir como administrador" com o medidor root) e "Medir sem senha"; `.help` diz que ⌘R mede com senha. Medindo, vira um `ProgressView` e, se a medição pode ser interrompida, o botão **Parar** (`stop.circle`) aparece logo acima dele.

**O texto de estado e o popover `DetalhesMedicao` saíram.** Cada dado do popover tem lugar fixo em outra tela: data e modo da medição no cabeçalho de "O que mais ocupa o disco" (Início) e na legenda de Dados do Sistema; Acesso Total e modo em Permissões e aparência; versão do sdtally e do macOS em Sobre; pastas que não abriram, "Sem explicação" e a hora da tela do Ajustes em Dados do Sistema; etapas com erro em "Estado da medição" (Dados do Sistema) e, como aviso de uma linha, no cabeçalho do Início. "Medindo…" aparece no Medir da barra lateral e na linha de medição do cartão.

**`enum Secao: String, Hashable, CaseIterable`**
- Ordem: `resumo, fazer, armazenamento, ranking, historico, permissoes, info`.
- `static let barra: [Secao] = [.resumo, .fazer, .armazenamento, .ranking, .historico]`.
- Títulos de `permissoes` e `info`: "Permissões e aparência" e "Sobre o SDTally".
- Os dois saem da `List`, mas continuam no enum, porque o menu e o `Foto` usam.
- O raw value String serve para "Abrir em: Última tela" (§6).

**Menu Ir**
- `ForEach(Secao.barra.enumerated())` gera ⌘1 a ⌘5.
- ⌘, continua levando a `.permissoes`, e "Sobre o SDTally" continua no menu do app.

**Sem barra de ferramentas.** Não há `.toolbar` na janela e o nome da seção não aparece (`navigationTitle` fica só como título da janela para o sistema e a acessibilidade).
- Sai o `ToolbarItem` "Abrir Armazenamento".
- O Medir foi para a barra lateral (ver o Rodapé). O filtro de Pastas maiores (`.searchable`) virou um campo `TextField` no topo do conteúdo da tela, e o "voltar" do detalhe de uma pasta virou o botão "Voltar" do `RamoView` (sem `NavigationStack`: `Painel.pilha.last` é a tela; ⌘[ continua).

**O "!" do Acesso Total** sai da barra lateral e vira a faixa B0 do Início.

**Por que esses nomes**
- "Armazenamento" batia com "Abrir Armazenamento no Ajustes", que é outra coisa.
- "Dados do Sistema" põe o nome do produto na barra lateral.
- A tela muda de ordem para merecer o nome (§7.2).

---

## 2. Wireframe (janela 1200×800, Mac do dono)

Estado real em 05/10:
- medição completa das 18:39, sem senha (`meu-mac.json`);
- leitura rápida das 19:05 (livre 24,07 GB, purgeable 51,55 GB);
- tela do Ajustes lida às 18:49 (Dados do Sistema 176,45 GB).

O conteúdo usa `maxWidth 820`, `padding 24` e `spacing 20`.

```
┌─────────────────────┬──────────────────────────────────────────────────────────────────────────────┐
│ ● ● ●            ◫  │ Início                                                       [⟳ Medir ▾]     │
├─────────────────────┼──────────────────────────────────────────────────────────────────────────────┤
│ [⌂] Início       ⌘1 │ ╭ B1 ─ cartão simples, sem gradiente ──────────────────────────────────────╮  │
│ [✓] O que fazer  ⌘2 │ │ Macintosh HD                                         statfs · 19:05     │  │
│                     │ │ 24,07 GB livres                                         ╱┃┃┃┃ (Tally   │  │
│ DETALHES            │ │ de 494,38 GB · 95 % ocupado                                  10 %)      │  │
│ [▦] Dados do        │ │ B2 ⟲ O Time Machine não conclui um backup desde 01/10 (11 tentativas    │  │
│     Sistema      ⌘3 │ │    depois). Os snapshots que ele guarda no Mac somam 49,62 GB: o macOS  │  │
│ [≡] Pastas maiores  │ │    apaga esses snapshots sozinho se faltar espaço, mas o certo é o      │  │
│                  ⌘4 │ │    backup terminar. ‡              [Abrir Time Machine no Ajustes]      │  │
│ [~] Histórico    ⌘5 │ │ Se faltar espaço, o macOS libera sozinho até 51,55 GB ›                 │  │
│                     │ ╰─────────────────────────────────────────────────────────────────────────╯  │
│                     │ ╭ B3 ─ borda cinza (Marca.dados 50 %) ─────────────────────────────────────╮  │
│                     │ │ [▦] Dados do Sistema            │ O SDTally mediu estas partes:         │  │
│                     │ │ 176,45 GB  ⓘ                    │ ▮ Snapshots locais do Time   49,62 GB │  │
│                     │ │ no Ajustes às 18:49 · há 16 min │   Machine (9)                         │  │
│                     │ │ Ler de novo do Ajustes          │   o macOS libera sozinho 47,00 GB;    │  │
│                     │ │                                 │   na leitura das 18:49, em Disponível │  │
│                     │ │ Não é uma pasta. É o que o      │ ▮ Cache do CloudKit †        20,64 GB │  │
│                     │ │ Ajustes não põe em nenhuma      │ ▮ Modelos do Apple Intelligence 17,50 │  │
│                     │ │ outra linha: a Apple não mede,  │ ▮ Pastas do sistema †         6,85 GB │  │
│                     │ │ subtrai.                        │ As partes não somam o número do       │  │
│                     │ │                                 │ Ajustes.     Ver todas as partes ›    │  │
│                     │ │ ┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄ │  │
│                     │ │ B4 ◌ Sem senha, as pastas do sistema caem em Sem explicação: hoje       │  │
│                     │ │      14,29 GB (3,3 %).                           [Medir com senha…]     │  │
│                     │ ╰─────────────────────────────────────────────────────────────────────────╯  │
│                     │ B5 O que mais ocupa o disco                  medido às 18:39 · sem senha     │
│                     │ ████████▓▓▓▓▓▓▓▒▒▒▒▒▒▒░░░░░░░▚▚▚▚······························▓▓▓▓  ░░     │
│                     │ ▫ resto do volume de dados   ■ macOS   □ livre                               │
│                     │ [◷] Snapshots locais do Time Machine (9)  49,62 GB ● Dados do Sistema  ⊙‡  ⌄ │
│                     │ [▢] ~/.colima/_lima                       41,20 GB ● Documentos†         ⌄   │
│                     │ [▢] iCloud Drive · 2 pastas               39,52 GB ● várias linhas†  ✋  ⌄   │
│                     │ [▢] Música · ~/Music                      38,26 GB ● várias linhas†      ⌄   │
│                     │ [▢] Cache do CloudKit                     20,64 GB ● Dados do Sistema† ⊙ ⌄   │
│                     │ B6 Em seguida: Modelos do Apple Intelligence 17,50 GB · Máquina virtual do   │
│                     │ app Claude 12,74 GB · Imagens 10,44 GB · Swap (memória no disco) 9,18 GB     │
│                     │                                                     Todas as pastas (⌘4) ›   │
│ ─────────────────── │                                                                              │
│ Medido às 18:39 ·   │                                                                              │
│ sem senha     ⚙  ⓘ  │                                                                              │
└─────────────────────┴──────────────────────────────────────────────────────────────────────────────┘
```

**Linha aberta (clique na primeira linha da lista, Mac do dono):**
```
[◷] Snapshots locais do Time Machine (9)  49,62 GB  ● Dados do Sistema  ⊙ Confira antes ‡   ⌃
    O Time Machine tentou fazer backup várias vezes sem registrar um novo; enquanto isso, o
    snapshot de base fica no disco.                                         ← ficha.what
    O Time Machine tentou de novo várias vezes desde o último backup registrado, sem registrar
    outro, e o snapshot de base segue no disco. Veja o destino em Ajustes > Time Machine.
                                                                            ← textoPasso
    Último backup concluído: 01/10 às 20:29 · 11 tentativas depois; a última hoje às 16:42.
    O macOS libera sozinho 47,00 GB destes (CacheDelete). O Ajustes ora mostra essa parte em
    Disponível, ora em Dados do Sistema; na leitura das 18:49, em Disponível.
    [Abrir Time Machine no Ajustes]                                    Ver os snapshots ›
```

**Sem M2, M5 e M7:**
- a manchete não aparece;
- os selos e tags marcados com † somem;
- o cartão Dados do Sistema mostra só Snapshots e Apple Intelligence;
- o selo dos snapshots fica "O macOS cuida" (`time_machine_snapshots`).

---

## 3. Blocos

Arquivo: `ResumoView.swift`, que é reescrito. As funções puras vão para o novo `Causas.swift`, `nonisolated` e testáveis.

### B0. Faixa de Acesso Total (condicional, é o único bloco acima do herói)
- **Quando:** `painel.rapido?.fullDiskAccess == false`.
- **Peça:** `Faixa(simbolo: "lock.trianglebadge.exclamationmark", texto: textoConselho("grant_full_disk_access")!)`.
  - O texto vira "…Ative em Permissões (⌘,) e meça de novo."
  - Botão "Abrir Permissões" → `painel.secao = .permissoes`.

### B1. Herói "Livre" (`.cartao()`, sem gradiente)

| Peça | Fonte no Modelo | Texto |
|---|---|---|
| Nome do volume | `URLResourceValues.volumeLocalizedName` (como no `CartaoAjustes.volume` de hoje) | "Macintosh HD" |
| Fonte e hora, à direita | `rapido.generatedAt` | "statfs · 19:05" (`.caption`, `.tertiary`) |
| Número | `rapido.capacity.free` | "24,07 GB livres" (`Font.heroi`, `.tracking(-0.5)`, `.transicaoNumero`) |
| Cor do número | `(free + purgeable.bytes) / total < 0,10`, a mesma conta do `Aviso.poucoEspaco` | `.primary`; abaixo do limite, `Marca.aviso` |
| Subtítulo | `capacity.total`, `1 − free/total` arredondado | "de 494,38 GB · 95 % ocupado" |
| "Hoje" (só no .app) | primeiro `Ponto` de hoje (`Ponto.t`, `.livre`) contra `capacity.free`, se \|Δ\| ≥ 1 GB | "Hoje: 35,90 GB às 11:00 → 24,07 GB agora ›" → `.historico` |
| Libera sozinho | — | Saiu do herói (18/10, pedido do dono); o purgeable fica em Dados do Sistema ("O que o macOS resolve sozinho"). |
| Ícone do volume | `NSWorkspace.shared.icon(forFile: "/")` | O ícone do próprio disco (o do Finder), 64×64, à direita, `accessibilityHidden`. "de X · Y % ocupado" fica na mesma linha do livre (`ViewThatFits`; empilha se não couber). Fonte do número: SF Pro, sem `.rounded`. O livre vem do `capacity` (statfs) do rápido, que roda a cada 30 s com o app ativo (`Painel.intervalo`) e custa ~0,2 s de tempo, ~0,12 s de CPU e ~11 MB com o painel do Ajustes fechado (`--since`, ver docs/json-schema.md). |

### B2. Manchete (dentro do herói; no máximo uma; regra fixa; vale a primeira que bater)

```swift
nonisolated struct Manchete: Equatable { let simbolo: String; let texto: String; let ajustes: String? }
nonisolated func manchete(advice: [Conselho]?, tm: Completo.TimeMachine?, snapshots: Int64?,
                          causas: [Causa], usado: Int64?) -> Manchete?
```

**R1. Backup parado ‡**
- **Quando:**
  - `advice` contém `tm_backup_stuck`;
  - existe um destino com `attemptsSinceCompleted ≥ 2` e `lastCompletedAt != nil`;
  - `snapshots` (= `bytes` do `tm_backup_stuck`: o termo `snapshots`, ou a soma dos privados do TM quando há snapshot de outro tipo, §9.2) ≥ 1 GB.
- **Texto:** "O Time Machine não conclui um backup desde {DD/MM} ({n} tentativas depois). Os snapshots que ele guarda no Mac somam {gb}: o macOS apaga esses snapshots sozinho se faltar espaço, mas o certo é o backup terminar." Com `bytes_floor: true` (snapshot de outro tipo, `bytes` é piso): "somam pelo menos {gb}".
- **Símbolo:** `clock.arrow.circlepath`, em `Marca.aviso`.
- **Botão:** `ajustes` = `botoesAjustes(fichas["tm_backup_stuck"]).first`, que dá "Abrir Time Machine no Ajustes". Nunca apagar snapshot, nem `tmutil`, nem exclusões.

**R2. Um pedaço grande**
- **Quando:** a primeira causa que não é swap tem `bytes × 5 > usado`, ou seja, mais de 20 % de `dataVolume.used`.
- **Texto:** "O maior pedaço do disco é {nome}: {gb}, {pct} % do usado." Quando a linha do Ajustes é conhecida e não é `mixed`, acrescenta " No Ajustes, fica em {nomeLinha}." †
- **Símbolo:** `chart.bar.fill`, em `.secondary`.
- **Botão:** nenhum.

**Senão:** sem manchete, e nada ocupa o lugar dela.

O G9 já foi conferido: as duas frases não têm "Seu Mac está", contagem nem palavra da lista proibida.

### B3. Cartão "Dados do Sistema" (`.cartao(destaque: Marca.dados.opacity(0.5))`)

Usa `ViewThatFits(in: .horizontal)`: com largura sobrando, `HStack(alignment: .top)` em duas colunas (esquerda com mínimo de 300, direita com mínimo de 320); sem largura, empilha.

**Coluna esquerda: o número**

Cabeçalho: `IconeSecao("square.stack.3d.up.fill", Marca.dados)` + "Dados do Sistema" (`.title3.semibold`). Fora do pt, vem também "No Ajustes: “System Data”", que já existe.

| Estado | Condição | O que mostra |
|---|---|---|
| Lido | `painel.telaBoa` com `systemData` | "176,45 GB" em `Font.heroi` (`.primary`), 16 pt abaixo do título (ícone = símbolo da barra lateral). A `Conta` (4 fichas, `cabecalho: false`) fica direto no cartão, em largura total, acima do tracejado (19/10: o ⓘ saiu). Legenda: "no Ajustes às 18:49 · há 16 min" (`TimelineView` de 30 s; passa a `Marca.aviso` acima de 15 min). Abaixo, `desde()`, usando `Historico.leituraAnterior`. Acima de 15 min, aparece o link "Ler de novo do Ajustes". |
| Não lido, com um ponto salvo | `sistemaBarra(tela: nil, pontos:)` | "Último lido: 176,45 GB em 05/10 às 18:49" (`.secondary`) + [Ler do Ajustes] |
| Nada | sem tela e sem ponto (V1) | "O número do Ajustes ainda não foi lido." + [Ler do Ajustes] |
| Esperando | `painel.aguardandoAjustes` | spinner + "Esperando o Ajustes calcular…" + "Pode fechar o Ajustes depois; o número fica aqui com a hora dele." |
| V2 | `screen.state == "partial"` | spinner + o texto do V2 |
| V3 | `vazioAjustes(...) == .v3` | o texto do V3, sem botão |

- - B1 (19/10): o livre vem do statfs do próprio app a cada 1 s (`Painel.vigiarLivre`, ~1 µs), ativo ou em segundo plano. O rápido é lido por partes (`--parts`, `Painel.intervalo`): tela 5 s esperando o Ajustes, 30 s ativo, 30 min em segundo plano; volumes 10 min; uso, swap e purgeable 1 min ativo e 5 min em segundo plano; Acesso Total ao abrir e ao voltar ao app.
- [Ler do Ajustes] = `AcaoVazio.lerAjustes`, que chama `painel.lerAjustes()`: abre o painel Armazenamento oculto e sem ativar (`NSWorkspace.OpenConfiguration` com `activates = false`, `hides = true`, o mesmo que `open -g -j`), lê a cada 5 s e fecha quando dois Dados do Sistema seguidos ficam a menos de 100 MB, ou em 60 s. Só fecha se o Ajustes seguir oculto. Com o Ajustes já aberto (o dono pode estar usando), não faz nada. O laço relê sozinho com o app ativo, boas-vindas vistas (o aviso está lá) e a tela com mais de 10 min (~27 s de CPU por leitura).
- B1 (19/10): abaixo do livre, as linhas da última tela do Ajustes (categorias ≥ 1 MB, macOS e Dados do Sistema), da maior para a menor, em 3 colunas lidas em Z.
- Sem texto fixo: "Não é uma pasta. É o que o Ajustes não põe em nenhuma outra linha: a Apple não mede, subtrai." fica no `.help` do título.

**Coluna direita: as partes**
- **Fonte:** `causas(...)` filtrado por `linha == "system_data"`, até 4, cada parte com `bytes ≥ 1 GB`.
- **Linha de cada parte:** `▮` em `Marca.dados`, com opacidade 1 e 0,7 alternadas + nome (`.lineLimit(1)`, `.truncationMode(.middle)`) + `gb(bytes)`.
- **Parte de snapshots:** ganha uma subnota com `informational.purgeable.topServices[com.apple.TMCacheDelete].bytes` e `purgavelNoAjustes()`:
  - "o macOS libera sozinho 47,00 GB destes; na leitura das 18:49, em Disponível";
  - ou "…, em Dados do Sistema";
  - ou, sem tela lida, "…; o Ajustes ora mostra essa parte em Disponível, ora em Dados do Sistema".
- **Rodapé da coluna:** só "Ver todas as partes ›" (o aviso de que as partes não somam o número do Ajustes saiu; a soma das pastas medidas fica no `.help` do link), que faz `pilhaBarra = [.volumeData]`, `rolarPara = "barra"` e `secao = .armazenamento`.
- **Sem medição completa:** "As partes saem da medição completa." (`.secondary`).
- **Clique numa parte:** mesmo destino do link "Ver…" da linha correspondente na lista (§3, B5).

**Embaixo, na largura toda:** separador tracejado (`StrokeStyle(dash: [3,3])`) e a **linha de medição (B4)**.

`purgavelNoAjustes` sai do `notaDisponivel` atual:

```swift
/// true: o purgeable está em Disponível nesta leitura; false: dentro de Dados do Sistema; nil: sem tela.
nonisolated func purgavelNoAjustes(tela: Tela?, livre: Int64?, purg: Int64?) -> Bool?
```

### B4. Linha de medição (o único lugar da senha; `CartaoMedicao` e `HelperLinha` fundidos em `LinhaMedicao`)

| Estado | Texto | Botões |
|---|---|---|
| Nunca mediu (`completo == nil`, `.parada`) | "Ainda não medido" | [Medir com senha…] (`borderedProminent`, com o porquê da senha no `.help`) [Medir sem senha] |
| Sem root (`incompleteReasons ∋ no_root`) | `Vazio.v12(c, comSenha:).texto`, reaproveitado: "Sem senha, as pastas do sistema caem em Sem explicação: hoje 14,29 GB (3,3 %)." + "Na medição com senha de DD/MM, eram Z.", se houver | [Medir com senha…], ou [Medir como administrador] com `helperAtivo` |
| Com root | "Sem explicação: 1,45 GB (0,4 % do usado). Medido como administrador." | nenhum |
| Sem Acesso Total, com root | "Sem explicação: X (Y % do usado)." | nenhum (a faixa B0 cuida) |
| Medindo | "Lendo o disco… 1:12 · só leitura" (`TimelineView` de 1 s). Sem `cancelavel`: "A medição como administrador não pode ser parada; termina sozinha." | [Parar], só se `podeParar` |
| Senha cancelada | "Sem a senha, a medição fica incompleta: as pastas que só o administrador lê ficam em Sem explicação." | [Medir sem senha] [Agora não] |
| Falhou | a mensagem (`.textSelection`) | [Tentar de novo] (o mesmo botão de senha) [Copiar detalhes] |
| Oferta do medidor (uma vez) | depois de `modoCompleto == .senha`, com `helper != .enabled` e `@AppStorage("ofertaMedidor") == false`: "Para não digitar a senha a cada medição:" + [Instalar medidor root…] [Agora não]. `.requiresApproval`: "Aguardando aprovação em Ajustes > Geral > Itens de Início." + [Abrir Itens de Início] | — |

**Regra de código:** um único `@ViewBuilder var botaoSenha` chama `painel.medir(admin: true)`, e todas as linhas da tabela o usam. É isso que torna a regra verificável por grep (§10).

### B5. "O que mais ocupa o disco"

**Cabeçalho**
- À esquerda: "O que mais ocupa o disco" (`.title3.semibold`).
- À direita: "medido às 18:39 · sem senha" (`.caption`, `.secondary`).
- Medição com mais de 24 h: "Medido há 2 dias. Meça de novo antes de agir." (`idadeMedicao`), em `Marca.aviso`.
- `interrupted`: "Medição interrompida: podem faltar pastas.", em `Marca.aviso`.

**Barra:** `BarraView(segmentos:destaque:)`, que já existe, com altura 12 e sem total próprio.

Base: `Armazenamento.principal(completo.containers)`, do mesmo instante das causas.

| Segmento | Bytes | Cor | Fonte (só no `.help`) |
|---|---|---|---|
| Cada uma das 5 causas, menos o swap | `causa.bytes` | `corLinha(causa)` (abaixo), opacidade 1 e 0,7 alternadas | a da linha |
| Resto do volume de dados | `capacityInUse` do volume Data − Σ das causas mostradas | `Color.secondary.opacity(0.25)` | "diferença" |
| macOS | Σ `capacityInUse` dos volumes do container principal com `inSettingsMacos` | `.gray.opacity(0.5)` | diskutil |
| Livre | `principal.free` | `quaternaryLabelColor` | diskutil |

- **Legenda:** só "▫ resto do volume de dados ■ macOS □ livre", **sem números**. A lista é a legenda das causas. O Livre do diskutil (21,07) é diferente do statfs do herói, por isso não aparece escrito.
- **Sem medição completa:** três segmentos tirados de `rapido.containers` (volume de dados, macOS, livre).

**Linhas:** só as 5 primeiras causas. Altura de 32. É um `Button`, e o clique abre ou fecha o detalhe; só um fica aberto por vez.

```
[IconeSecao(termo.simbolo | "memorychip", corLinha)] nome · caminho curto (cinza, se o nome ≠ caminho)
                       gb(bytes)  ● nomeLinha (escondido se nil)  selo (escondido sem ficha)  ⌄
```

- **Selo:** `Marca.nivel(ficha.level)` + `nomeNivel(level)` ("O macOS cuida", "Confira antes", "Não mexa"…).
- **Hover:** destaca a linha e o segmento (`@State destaque: Set<String>`). O clique no segmento abre a linha.
- **Acessibilidade:** `.accessibilityElement(children: .combine)`, com o rótulo "nome, 49,62 GB, no Ajustes Dados do Sistema, Confira antes".

**Detalhe aberto**, na ordem:
1. `ficha.what.pt`, em `.secondary`.
2. O passo: `textoPasso(conselho, fichas:)` ou `ficha.step.pt`.
3. Os fatos medidos, só onde existem:
   - **Snapshots:** "Último backup concluído: {dataCurta(lastCompletedAt)} · {n} tentativas depois; a última {quando(lastAttemptAt)}." Só com `n ≥ 2` e `running == false`. Mais a frase do purgeable da parte B3.
   - **Swap:** "{gb(swap.total)} reservados, {gb(swap.used)} em uso · ligado há {N} dias." (de `bootTime`) "Fica na linha macOS do Ajustes."
4. Botões, no máximo 2:
   - `botoesAjustes(ficha).first`;
   - "Mostrar no Finder" para `termo == "tree"` com caminho. A regra R3 permite, porque só revela.
5. Link à direita, um só:

| Causa | Link | Ação |
|---|---|---|
| Com conselho | "Ver em O que fazer ›" | `rolarPara = conselho.id`, `secao = .fazer` |
| Snapshots | "Ver os snapshots ›" | `rolarPara = "snapshots"`, `secao = .armazenamento` |
| Swap | "Vale a pena reiniciar? ›" | `rolarPara = "reiniciar"`, `secao = .armazenamento` |
| Pasta | "Ver a pasta ›" | `pilha = [absoluto(caminho, home:)]`, `secao = .ranking` |

**Lista vazia**
- **Sem medição completa:** "A lista sai da medição completa. Para começar, use o cartão Dados do Sistema, acima." Sem botão, porque a senha fica só em B4.
- **Sem nenhuma causa ≥ 1 GB:** "Nenhuma pasta passou de 1 GB (medido às HH:MM)."

**Funções (em `Causas.swift`)**

```swift
nonisolated struct Causa: Identifiable, Sendable, Equatable {
    let id: String        // card, caminho, termo ou "swap"
    let nome: String
    let caminho: String?  // o da maior linha do grupo
    let termo: String     // tree, snapshots, protected_folders, deleted_open, large_xattrs, swap
    let card: String?
    let bytes: Int64
    let pastas: Int       // linhas agrupadas ("iCloud Drive · 2 pastas")
    let linha: String?    // settings_line, a provisória, ou nil
}
nonisolated func causas(top: [LinhaTop], swap: Swap?, fichas: MapaFichas?, home: String) -> [Causa]
nonisolated func conselho(de c: Causa, em advice: [Conselho]) -> Conselho?
nonisolated func nomeLinha(_ l: String?) -> (nome: String, cor: Color)?
```

**`causas`**
1. Descarta as linhas com `folded == true`.
2. Agrupa por `card` quando não é nil. Linha sem ficha fica sozinha, com a chave `path ?? term`.
3. Soma os `bytes` do grupo. `caminho` e `termo` vêm da maior linha.
4. `linha`:
   - um único `settingsLine` no grupo: esse valor;
   - mais de um: `"mixed"`;
   - nenhum: `linhaProvisoria`.
5. Acrescenta o swap: id `"swap"`, nome "Swap (memória no disco)", `bytes = swap.used`, linha `"macos"`.
6. Fica só o que tem pelo menos 1 GB. Ordena por `bytes` decrescente e, no empate, por `id`.

**Nome da causa**
- Termos `snapshots`, `deleted_open` e `large_xattrs`: `nomeTermo(termo)`, mais " ({count})" quando houver contagem.
- Os outros, na ordem:
  1. `tituloConselho(card)`;
  2. `fichas[card].what.pt` sem o ponto final (como o `Recursos.grupos` já faz);
  3. pasta direta da home (`~/X`): `FileManager.displayName(atPath: absoluto)`, que dá "Música", "Imagens", "Mesa" ou "Filmes";
  4. o resto: `visivel(caminho)`.

**`linhaProvisoria(termo:card:)`** (fica com o comentário `ponytail:` "sai quando todo JSON tiver settings_line"):
- `snapshots` e `deleted_open` → `"system_data"`;
- card `apple_intelligence_models` → `"system_data"` (Frente M);
- o resto → nil.

`protected_folders` **não** entra aqui: sem root, a home de outra conta cai nesse termo.

**`conselho(de:)`**
1. `advice` com o mesmo `card`.
2. Senão, `paths` que contêm o caminho.
3. Senão, o mesmo `term`, só para os termos que não são `tree` nem `protected_folders`. Com isso, snapshots aponta para `tm_backup_stuck` ou `time_machine_snapshots`.
4. Para o swap: `restart_worth` antes de `swap_large`.

**Ficha da linha:** `fichas[conselho?.card] ?? fichas[causa.card]`.

**`nomeLinha`**
- `"system_data"` → ("Dados do Sistema", `Marca.dados`);
- `"mixed"` → ("várias linhas", `.secondary`);
- `"macos"` → ("macOS", `.gray`);
- id da Apple → `categoria(id)`;
- nil → nil.

**`corLinha(causa)`:** `nomeLinha(causa.linha)?.cor` ou, sem linha, `termo(causa.termo).cor`.

### B6. Depois da lista (cada linha só aparece quando tem conteúdo)
- **"Em seguida:"** as 4 primeiras causas depois da 5ª, sem a do "Seguro": "nome gb" separados por " · ", cada um clicável com o mesmo destino do "Ver…". À direita, "Todas as pastas (⌘4) ›". Sem contagem e sem soma (G9).
- **"Seguro:"** o primeiro `advice` que cumpre tudo isto:
  - `level == "safe"`;
  - `bytesLogical != true`;
  - `privateBytes ≥ 1 GB`;
  - o card não está nas 5 linhas.

  Texto: "Seguro: {tituloConselho} · {gb(bytes)} ›" → O que fazer, na âncora dele. Esse item sai do "Em seguida".
- **"O que mudou":** `Historico.mudancas(de:para:)` entre as duas últimas `painel.medicoes`, as 3 primeiras. Texto: "Desde a medição de {dataCurta}: {rótulo} +{gb1} · … ›" → Histórico. Com `nil` (não comparáveis) ou lista vazia, não aparece nada.

---

## 4. Estados do Início

| Estado | Condição | O que muda |
|---|---|---|
| Primeira abertura | `completo == nil`, `medicoes` vazio | A folha de boas-vindas abre por cima (já existe). B1 sai do rápido em cerca de 3 s; antes disso, esqueleto `.redacted`. B3 mostra o número ou o V1, e a coluna direita diz "As partes saem…". B4 fica em "Nunca mediu". B5 vazio, com a barra de três segmentos. Sem manchete. |
| Medindo, já com medição | `.medindo` | B4 mostra o progresso. As partes de B3 e a lista B5 ficam com `.opacity(0.5)`. B1 e B2 não mudam. |
| Medindo pela primeira vez | `.medindo`, `completo == nil` | B5 mostra 5 linhas de esqueleto `.redacted`. |
| Sem root | `no_root` | B4 com o V12 e o botão. O resto igual. |
| Sem Acesso Total | `fullDiskAccess == false` | Aparece a faixa B0. |
| Sem registro do Ajustes | V1, V2, V3 | Só a coluna esquerda do B3 muda (§3, B3). Nunca há faixa no topo. |
| Ajustes lido há mais de 15 min | `loggedAt` | A legenda do número passa a `Marca.aviso` e aparece o link "Ler de novo do Ajustes". |
| Medição com mais de 24 h | `generatedAt` | Legenda de B5 em `Marca.aviso`. A manchete continua. |
| Medição interrompida | `interrupted` | Legenda de B5 em `Marca.aviso`. |
| Senha cancelada | `.senhaCancelada` | B4 (tabela). |
| Erro na medição | `.falhou` | B4 (tabela). |
| Erro na leitura rápida, com valor anterior | `rapidoFalhou`, `rapido != nil` | B1 mantém o número, e a legenda vira "Não atualizado desde HH:MM" em `Marca.aviso`, com a mensagem no `.help`. |
| Erro na leitura rápida, nunca leu | `rapidoFalhou`, `rapido == nil` | B1 mostra "—" + "Não deu para ler o disco: {msg}" + [Tentar de novo] (`Task { await painel.atualizar() }`). |
| Sem histórico | `pontos` vazio ou fora do .app | Somem "Hoje:", "desde a última leitura" e "O que mudou", sem nada no lugar. |
| Arquivo antigo | sem `cards` ou sem `settings_line` | Os nomes vêm do termo ou do caminho. Selos e tags somem, menos as provisórias. |

---

## 5. Senha: um lugar só

- **Na tela:** só em B4, no fim do cartão Dados do Sistema. Tem um único `botaoSenha`.
- **Fora da tela (comandos globais, sem texto próprio na tela):**
  - o Medir da barra lateral (⌘R);
  - "Medir sem senha" no menu do botão e em ⌥⌘R;
  - o menu Medição, que não muda e continua com "Instalar/Remover medidor root".
- **Saem:**
  - os botões do `CartaoMedicao`;
  - a `HelperLinha`, que fica só como oferta única em B4 e em Permissões;
  - o V12 solto;
  - o "Medir de novo…" do `DetalhesMedicao`;
  - o cartão `measure_as_admin` em O que fazer: `ConselhosView` filtra `id == "measure_as_admin"`, porque B4 já é esse passo.
- **Ficam como estão:** os estados vazios de outras telas, que só aparecem sem medição completa e têm um botão cada (Pastas maiores, O que fazer V20, "Volume de dados" em Dados do Sistema). Cada tela continua com um lugar só.

---

## 6. Cor, personalidade e personalização

**Tokens** (`Marca.swift`; só cores de sistema, que já trocam com o tema)

| Token | Valor | Onde |
|---|---|---|
| `Marca.dados` | `.gray` | Só Dados do Sistema (▮ das partes, borda do B3 a 50 %, segmento e ponto da linha "Dados do Sistema", ícone ⌘3); o número fica em `.primary`. **Controles:** `.tint` padrão do sistema (abaixo). |
| `Marca.violeta` | removido | Era o quadrado do Início; sem a caixa colorida na barra, ficou sem uso. |
| `Marca.cor`, `Marca.gradiente` | removidos | Eram o magenta e o fundo do herói |
| `Marca.aviso` | `.orange` | Símbolo da manchete R1, legendas velhas, erro do rápido, Livre abaixo do limite de pouco espaço |
| `categoria()` | cores do Ajustes | Linhas que não são Dados do Sistema. A reserva `.pink` **vira `.gray`**, para não parecer magenta. Entra `"com.apple.STMExtension.iOSFiles": ("Arquivos do iOS", .cyan)`. |
| `IconeSecao` | quadrado de 20 pt | Também nas linhas de B5, na cor da linha do Ajustes (ideia da "saude"). É o que dá vida sem parecer alarme. |
| `TallyShape` | `Color.primary` a 0,08 | Glifo no herói |

**Regras**
- **Só cores de sistema:** nenhum hex, nenhuma cor de marca (o `scripts/g9.sh` falha com hex `0x……`, `srgbRed` ou `NSColor(name:`). Dados do Sistema nunca usa o destaque do sistema (pode ser vermelho). Roxo (`.purple`) só onde o Ajustes usa (Outros usuários); termos e níveis usam `.indigo`.
- O estado nunca é magenta, e nada é vermelho, menos o `.red` de Aplicativos, que já tem o comentário `// g9: cor do Ajustes`.

**`.tint`**
- **Revisão:** o `.tint` da `JanelaView` saiu; botão `borderedProminent`, toggles, pickers e progresso seguem o destaque do sistema.
- **Origem do vermelho atual:** o destaque do sistema deste Mac, `AppleAccentColor = 0` (vermelho).
- **Botões `.link`:** continuam azuis, porque o estilo usa `linkColor`; na foto de teste o link ficou azul.
- **Seleção da barra lateral:** não confirmei se segue o `.tint`. A janela da foto saiu inativa e a seleção, cinza.
  - Critério P1: conferir com a janela ativa.
  - Se a seleção seguir o destaque do sistema, aceitar: é escolha da pessoa em Ajustes > Aparência.

**Personalização (Permissões e aparência > seção "Aparência", `@AppStorage`)**
1. ~~"Cor dos botões"~~ (removida: `corMarca` não existe mais).
2. "Abrir em":
   - Valores: "Início" (padrão) ou "Última tela" (`abrirEm`).
   - `JanelaView.onChange(of: painel.secao)` grava `ultimaSecao` (rawValue).
   - `Foto.secaoInicial` aplica a escolha uma vez, na criação do Painel, quando `abrirEm == "ultima"` e `Foto.env["SDTALLY_SECAO"] == nil` (o menu Ir e a notificação escolhem depois, sem passar por ela).

**Personalidade nos textos:** frase curta, afirmativa e com número. O conselho vem como "o certo é…", "confira antes", "fica em…". Nada de "limpar".

---

## 7. O que sai do Resumo atual, e o que muda nas outras telas

### 7.1 Do Resumo atual para…

| Hoje | Vai para |
|---|---|
| `CartaoAjustes`: barra das categorias e V1 cinza no topo | Número e estados V1, V2, V3 numa coluna do B3. A barra das categorias fica em Dados do Sistema > barra, nível `ajustes` (já existe). |
| `Conta` (usado − macOS − outras linhas, 4 fichas) e `notaDisponivel` | Direto no B3, abaixo do número e das partes (mesmo struct, sem cabeçalho). O `notaDisponivel` vira o `purgavelNoAjustes` + o texto de hoje. |
| `Fichas`: Livre | B1 |
| `Fichas`: O macOS libera sozinho | Linha do B1 (mesmo destino: âncora `cuida`) |
| `Fichas`: Sem explicação | B4 |
| `ProximosPassos` (`measure_as_admin`; `asset_xcode` com "0 B libera se apagar"; chronod) | Cada causa leva o próprio passo no detalhe. A lista inteira fica em O que fazer. `measure_as_admin` vira B4. "Libera se apagar" sai do Início. **Apagar** `proximosPassos()` e os testes dela (3 em `ResumoTests`, `proximosPassosSemNadaAFazer` em `ArmazenamentoTests`). `textoPasso` fica. |
| `VazioLinha` V12 + botão | O texto do V12 em B4 |
| `BarraTermos` "O que ocupa o volume de dados" | Dados do Sistema > barra, nível `volumeData` (já existe). "Arquivos e pastas", "Atributos estendidos grandes" e "Apagados ainda abertos" só aparecem lá. |
| `CartaoMedicao` + `HelperLinha` | `LinhaMedicao` (B4) |
| Barra de ferramentas: "Abrir Armazenamento" | [Ler do Ajustes] no B3 + menu Medição (⌥⌘A) |
| Rodapé: barra, Livre, libera sozinho | B1 |
| Seção App + "!" | ⚙ ⓘ no rodapé + faixa B0 |

### 7.2 Outras telas

| Tela ou arquivo | Mudança |
|---|---|
| **Dados do Sistema** (`ArmazenamentoView`) | O título vem de `Secao.titulo`. **Só muda a ordem:** `BarraDesdobravel().id("barra")` → `AppleIntelligence` → (com medição) `PartesDoSistema`, `RecursosView`, `SnapshotsView`, `ApagadosAbertos`, `ReiniciarView`, ou o `Bloco` "Volume de dados" → `CuidaSozinhoView().id("cuida")` → `Text("O disco inteiro").font(.title2.weight(.semibold))` → `Fisico` (Disco interno, donut, `LivresView().id("livres")`, volumes) → `InformativoBloco` → `EstadoMedicao` → `NaoMedido`. As âncoras ficam iguais. |
| **O que fazer** (`ConselhosView`) | Filtra `measure_as_admin`. A rolagem passa a `painel.rolarPara ?? Foto.ancora`, limpando `rolarPara` depois (o `.id(item.id)` já existe). Na V17, o botão vira `AcaoVazio.verArmazenamento` com o rótulo novo. |
| **Pastas maiores** (`RankingView`) | `navigationTitle("Pastas maiores")` e o comentário do topo |
| **Permissões e aparência** (`PermissoesView`) | Título novo e a `Section("Aparência")` (§6) |
| **Sobre o SDTally** (`InfoView`) | Título |
| `Vazios.swift` | `.verArmazenamento` → "Ver em Dados do Sistema". V17 → "…O que o macOS cuida sozinho está em Dados do Sistema." |
| `BoasVindasView`, `conselhosPT["grant_full_disk_access"]`, `src/catalogo.rs:131`, `Fixtures/catalogo.json` | "Permissões (⌘6)" → "Permissões (⌘,)" |
| `Foto.swift` | `SDTALLY_RAPIDO=/x.json` (só com `Dev.ativo`): `static var rapidoFixo: Data?`. Os valores de `SDTALLY_SECAO` não mudam (`resumo` abre o Início; `armazenamento` abre Dados do Sistema). |
| `Painel.atualizar` | `let dados = if let f = Foto.rapidoFixo { f } else { try await Executor.rodar(["--quick","--json"]) }` |
| `Modelo.swift` | `LinhaTop`: `var settingsLine: String? = nil`, `var settingsLineGrade: String? = nil` |
| PDF (`Relatorio`) e barra de menus | Sem mudança nesta fase |

---

## 8. Como fica nos dois Macs (números reais)

### 8.1 Mac do dono

Dados:
- `meu-mac.json`: 18:39, sem root, `denied_folders` 252;
- leitura rápida às 19:05;
- Ajustes lido às 18:49 (foto `fotos/sintese-tint.png`).

| Bloco | Hoje (JSON atual) | Com M2, M5 e M7 |
|---|---|---|
| B0 | não aparece (`full_disk_access: true`) | igual |
| B1 | "24,07 GB livres · de 494,38 GB · 95 % ocupado"; "libera sozinho até 51,55 GB" (47,00 GB do TMCacheDelete). Às 18:39 eram 21,83 GB e 51,28 GB. Livre + purgeable = 15,3 % do disco, então o número fica na cor normal. | igual |
| "Hoje" | não aparece: `~/Library/Application Support/SDTally` não existe (conferido) | aparece quando o .app tiver gravado pontos |
| B2 | **não aparece**: `tm_backup_stuck` não está no `advice`, e a maior causa tem 11,4 % do usado (abaixo de 20 %) | R1: "O Time Machine não conclui um backup desde 01/10 (11 tentativas depois). Os snapshots que ele guarda no Mac somam 49,62 GB: …" + [Abrir Time Machine no Ajustes]. A última tentativa foi às 16:42. |
| B3, número | "176,45 GB · no Ajustes às 18:49" | igual |
| B3, partes | Snapshots locais do Time Machine (9) 49,62 GB ("o macOS libera sozinho 47,00 GB destes; na leitura das 18:49, em Disponível": Disponível 76,04 GB contra livre 24,07 GB) · Modelos do Apple Intelligence 17,50 GB | Snapshots 49,62 · Cache do CloudKit 20,64 · Modelos do Apple Intelligence 17,50 · Pastas do sistema 6,85 |
| B4 | "Sem senha, as pastas do sistema caem em Sem explicação: hoje 14,29 GB (3,3 %)." + [Medir com senha…] | igual |
| B5 (5 linhas) | 1. Snapshots locais do Time Machine (9), 49,62 GB · Dados do Sistema · O macOS cuida. 2. `~/.colima/_lima` 41,20. 3. iCloud Drive · 2 pastas 39,52 · Não mexa. 4. Música · `~/Music` 38,26. 5. Cache do CloudKit 20,64 · Confira antes | tags: Dados do Sistema‡ (selo Confira antes) · Documentos (A) · várias linhas (B) · várias linhas (A) · Dados do Sistema (A). Selos § de Colima e Música. |
| B6 | Em seguida: Modelos do Apple Intelligence 17,50 · Máquina virtual do app Claude 12,74 · Imagens 10,44 · Swap (memória no disco) 9,18. Não há "Seguro" (o único item seguro, o chronod, tem bytes lógicos). Não há "O que mudou". | igual |

**O que o Início conta ao dono** (só com números medidos):
- **Por que o disco encheu:**
  - o Time Machine não termina o backup desde 01/10, e os snapshots seguram 49,62 GB;
  - o macOS pode liberar 47,00 GB disso sozinho, sem precisar apagar nada à mão.
- **O que é o Dados do Sistema aqui (176,45 GB às 18:49):**
  - nessa leitura, o purgeable estava em Disponível, então os snapshots **não** entram nos 176,45 GB;
  - entram o cache do CloudKit (20,64 GB) e os modelos do Apple Intelligence (17,50 GB);
  - pela Frente P (grau B), entra também quase todo o iCloud Drive baixado: a linha "iCloud Drive" do Ajustes marcou 258,5 MB, contra 37,03 GB medidos em `com~apple~CloudDocs`. Fica como observação; a tela não mostra essa conta (§9).
- **A Frente P confere com o Ajustes deste Mac:**
  - Música: 36,43 GB no Ajustes contra 38,26 GB em `~/Music`. A diferença, 1,8 GB, cai em Dados do Sistema, como a Frente P previu.
  - Documentos: 72,63 GB no Ajustes, o que comporta `~/.colima` (41,20 GB). Isso mostra que a pasta não é Dados do Sistema.

### 8.2 Mac do colega

Fonte: `sdc-relatorio-2026-10-05-1445.json`, com root e sem log do Ajustes. O app não abre esse formato; a fixture do M8 faz a conversão.

| Bloco | Hoje, com a fixture | Com M5 e M7 |
|---|---|---|
| B1 | "99,66 GB livres · de 494,33 GB · 80 % ocupado"; "libera sozinho até 1,74 GB" (`CACHE_DELETE_TOTAL_AVAILABLE`) | igual |
| B2 | R2: "O maior pedaço do disco é /Users/<outro>: 145,18 GB, 39,7 % do usado." | "O maior pedaço do disco é Outra conta deste Mac: 145,18 GB, 39,7 % do usado. No Ajustes, fica em Outros usuários." |
| B3 | "O número do Ajustes ainda não foi lido." + [Ler do Ajustes]. Partes: Modelos do Apple Intelligence 7,36 GB | Partes: `~/Library/Group Containers/group.net.whatsapp.WhatsApp.shared/Message` 8,23 · Apple Intelligence 7,36 · `~/Library/Caches/Google/Chrome` 4,99 · `/private/tmp` 4,57 |
| B4 | "Sem explicação: 1,45 GB (0,4 % do usado). Medido como administrador." Sem botão. | igual |
| B5 | 1. /Users/<outro> 145,18. 2. Mesa 78,84. 3. Filmes 18,16. 4. Máquina virtual do app Claude 11,21 · Confira antes. 5. `…/WhatsApp.shared/Message` 8,23 | tags: Outros usuários (B) · várias linhas (B; "Mesa e Documentos" do iCloud não medido) · várias linhas · Aplicativos (A) · Dados do Sistema (A) |
| B6 | Seguro: Cache do npm · 7,78 GB › (o `npm_cache` é seguro e libera 7,78 GB). Em seguida: `~/Library/Application Support/Google/Chrome` 7,92 · Modelos do Apple Intelligence 7,36 · `~/Library/Caches/Google/Chrome` 4,99 · `/private/tmp` 4,57 (o npm sai daqui e o próximo, `/private/tmp`, entra) | igual |
| Time Machine | não aparece: nenhum snapshot ocupa espaço. "Nenhum backup concluído registrado" fica em Dados do Sistema (bloco de snapshots, V6). | igual |

**O que o Início conta ao colega:** o disco está cheio por causa de arquivos de pessoas (outra conta com 145 GB, Mesa com 79 GB), não de Dados do Sistema. A única ação segura é o cache do npm.

---

## 9. Dados que faltam e pendências no CLI (nada disso foi inventado na tela)

**1. `top[].settings_line` + `settings_line_grade` (M5)**
- Novo `src/linhas.rs`, chamado dentro de `catalogo::anexar`, que todas as saídas já usam.
- O valor é o **id da categoria da Apple** (o mesmo que `categoria()` lê), ou `"system_data"`, ou `"mixed"`, ou `null`. O grau é `"A"` ou `"B"`.
- O `--last` continua cru, e o app usa a linha provisória.

**Regras** (vale a primeira que bater; o caminho no formato do `top`):

| Caminho ou termo | Linha | Grau |
|---|---|---|
| termo `snapshots` | `system_data` | B |
| termo `deleted_open` | `system_data` | A |
| termo `large_xattrs` | null | — |
| exatamente `~`, `~/Library`, `~/Library/Developer`, `~/Library/Application Support/MobileSync`, `/Users`; linha `folded` de um destes | `mixed` | A |
| `~/.Trash` | Trash | A |
| `~/Library/Application Support/MobileSync/Backup` | iOSFiles | A |
| `~/Library/Application Support/MobileSync/*` | `system_data` | A |
| `~/Library/Application Support/*` | Applications | A |
| `~/Library/Mail/*` | Mail | A |
| `~/Library/Messages/*` | `mixed` | A |
| `~/Library/Developer/Xcode/{DerivedData,Archives,Installs,* DeviceSupport}`, `~/Library/Developer/CoreSimulator/Caches` | Developer | A |
| `~/Library/Containers/*`, `~/Library/Mobile Documents/*` | `mixed` | B |
| `~/Library/iTunes/* Software Updates` | iOSFiles | A |
| outro `~/Library/*` (Caches, Group Containers, CloudStorage…) | `system_data` | A |
| `~/Music/Music/Media.localized/*` | Music | B |
| `~/Pictures/Photos Library.photoslibrary/*` | Photos | B |
| `~/Music`, `~/Movies`, `~/Pictures` e o que está dentro | `mixed` | A |
| `~/Desktop`, `~/Documents` | `mixed` | B |
| `~/Applications`, `/Applications/*` | Applications | A |
| outro `~/*` (pastas ocultas, Downloads, Public…) | Documents | A |
| `/Users/Shared`, `/Users/Deleted Users` | OtherUsers | A |
| `/Users/*` | OtherUsers | B |
| `/Library/Developer/{CommandLineTools,CoreSimulator}` | Developer | A |
| `/Library/Application Support/GarageBand/Instrument Library`, `/Library/Application Support/Logic/{Sampler Instruments,EXS Factory Samples,Alchemy Samples,Ultrabeat Samples,Channel Strip Settings,Plug-In Settings}`, `/Library/Audio/{Impulse Responses,Apple Loops}/Apple` | GarageBand | A |
| o resto (`/Library`, `/private`, `/opt`, `/System/Library/AssetsV2`, `/.DocumentRevisions-V100`…) | `system_data` | A |

**2. Detector do Time Machine (M2)**
- **Defeito:** em `src/conselho.rs:296`, o `tm_parado` exige `private_bytes ≥ 1 GB` no snapshot de referência. No dono ele vale 0, e o detector não dispara, mesmo com 11 tentativas e `running: false`. O critério 5 do spec-fase6 dizia que deveria disparar.
- **Correção:**
  - tirar o `*p >= GB` dos candidatos;
  - exigir `terms[snapshots].bytes ≥ GB`;
  - `bytes` = `terms[snapshots].bytes`;
  - `privado` = `Some(p)` da referência. Com 0, o app já mostra "Ocupa".
  - O texto en passa de "The base snapshot holds {p}" para "Local Time Machine snapshots hold {total}".
  - O termo é a faixa de todos os snapshots, de qualquer tipo. Com algum snapshot que não é do TM (outro app, atualização do macOS), o portão de 1 GB e o `bytes` usam a soma de `private_bytes` dos snapshots do TM (um piso): o item leva `bytes_floor: true` e o texto en diz "hold at least {total}".
- Atualizar a linha `tm_backup_stuck` do spec-fase6 §6.2.

**3. Fichas novas (M7)**
- Todas entram com a `Casa::Caminho` que as liga às linhas, e com refs conferidas (R6). **Os passos ficam como rascunho até a conferência.**

| Ficha | Caminhos | Nível | Rascunho |
|---|---|---|---|
| `other_users` | `/Users/*` | `check_first` | "Pasta de outra conta, a Compartilhada ou uma conta apagada. Quem usa a pasta, ou um administrador em Ajustes > Usuários e Grupos, decide o que fica." Sem ação; não desce na pasta. |
| `user_folder` | `~/Desktop`, `~/Documents`, `~/Movies`, `~/Music`, `~/Pictures`, `~/Downloads` | `check_first` | "Pasta sua. Veja o que pesa em Pastas maiores antes de mexer." Ação: Finder. |
| `container_vm` | `~/.colima`, `~/.lima`, `~/Library/Containers/com.docker.docker`, `~/Library/Group Containers/HUAQ24HBR6.dev.orbstack` | `check_first` | "Reduza pelo próprio programa, apagando lá dentro o que não usa: contêineres, imagens, volumes ou máquinas. Apagar a pasta por fora apaga a máquina inteira, com tudo dentro. No Colima, o espaço volta ao Mac quando a máquina reinicia." Ação: app dono. |
| `browser_cache` | `~/Library/Caches/{Google/Chrome,com.apple.Safari,Firefox}` | **conferir** | **Conferir** |
| `whatsapp_data` | `~/Library/Group Containers/group.net.whatsapp.WhatsApp.shared` | `check_first` | **Conferir:** liberar pelo armazenamento do próprio WhatsApp. |
| `private_tmp` | `/private/tmp` | **conferir** | Precisa de pesquisa sobre a limpeza automática. Sem ficha até lá. |

**4. Relatório do colega**
- O formato `sdc-coleta` não abre no app.
- **Para a foto**, uma fixture convertida à mão fica no scratch, **sem commit** (são dados de outra pessoa, mesmo anonimizados). Commit só se o dono aprovar.

| Campo do coletor | Campo no Modelo |
|---|---|
| `volume_data.container_{livre,total}_statfs` | `capacity` |
| `volume_data.usado_attr_vol_spaceused` | `data_volume.used` |
| `conta.nao_atribuido` | `unexplained` |
| `arvore.baldes` | `top` (`alocado` → `bytes`, `privado` → `private_bytes`) |
| `diagnostico.conselhos` | `advice` |
| `diagnostico.{time_machine,swap,boot_time}` | os campos de mesmo nome |
| `purgeable.totais.CACHE_DELETE_TOTAL_AVAILABLE` | `purgeable.bytes` |
| `painel.apple_intelligence_reproduzido.ocupacao_fisica_bytes` | linha com card `apple_intelligence_models` |

- Card à mão: `claude_vm_bundles`, `npm_cache`.
- O importador de verdade fica para quando for pedido.

**5. Ficaram de fora, de propósito, até alguém pedir**
- "Fontes: ao passar o mouse / sempre": mexe em todas as telas. No Início, a fonte já fica no `.help` e na legenda.
- "Na barra de menus: Dados do Sistema": vai contra a decisão registrada no `RotuloBarra`.
- Sparkline no herói: o Histórico e a barra de menus já têm o gráfico.
- Guardar a `Tela` inteira no Histórico: o `Ponto.systemData` já dá o "Último lido".
- Dividir o iCloud Drive entre a linha dele e Dados do Sistema: precisa medir o que pode sair do Mac (FPFS).
- Ordem dos blocos e temas.

---

## 10. Critérios de aceite

**Comandos**
- `swift test --package-path app --scratch-path "$TMPDIR/sdt-test"`
- `CARGO_TARGET_DIR="$TMPDIR/sdt-cargo" cargo test`
- `scripts/g9.sh` sai com 0.

**Fotos**
- Binário cru: `SDTALLY_BIN=<release>/sdtally`.
- `$S` = o scratch.
- `meu-mac-m2.json` = `sdtally --json` sem sudo, com o binário do M2, no Mac do dono (só lê).

| # | Variáveis | Conferir na foto |
|---|---|---|
| P1 | `SDTALLY_COMPLETO=$S/meu-mac-m2.json`, `SDTALLY_SECAO=resumo` | (a) Barra lateral: Início, O que fazer, *Detalhes*, Dados do Sistema, Pastas maiores, Histórico. Só ícones; do alto: Medir, Github (só a logo), ⓘ, ⚙, sem texto de estado. Sem Permissões e sem Informações na lista. (b) Sem barra de ferramentas nem nome da seção; o semáforo sobre a barra lateral. (c) "GB livres" grande; "% ocupado". (d) Manchete com "não conclui um backup desde 01/10" e [Abrir Time Machine no Ajustes]. (e) Cartão com borda cinza, partes e **um** botão "Medir com senha…" (sem "Não é uma pasta" visível). (f) 5 linhas, cada uma numa linha só, na ordem Snapshots, `~/.colima/_lima`, iCloud Drive, Música, Cache do CloudKit, e "Em seguida:". (g) **Não aparecem:** "Medir sem senha", "Instalar medidor root", "Arquivos e pastas", "Atributos estendidos", "Apagados ainda abertos", "libera se apagar", "Próximos passos", "0 B". (h) Com a janela ativa: o botão principal e a seleção seguem o destaque do sistema; nada magenta. |
| P2 | P1 + `SDTALLY_ESQUEMA=escuro` | Número de Dados do Sistema em `#F06BD0`, fundo do herói visível e sem bloco claro |
| P3 | `SDTALLY_COMPLETO=$S/colega-completo.json`, `SDTALLY_RAPIDO=$S/colega-rapido.json` | "99,66 GB livres", "80 % ocupado", manchete "O maior pedaço do disco é … 145,18 GB, 39,7 % do usado", B4 "Medido como administrador" sem botão, "Seguro: Cache do npm" |
| P4 | `SDTALLY_RAPIDO=$S/rapido-agora.json`, sem `SDTALLY_COMPLETO` | B4 "Ainda não medido" com os dois botões (sem "Por que a senha?"). B5 com o texto e **sem** botão. Barra com três segmentos. |
| P5 | P4 com `full_disk_access: false` no JSON do rápido | Faixa B0 acima do herói, e mais nada acima dele |
| P6 | `SDTALLY_SECAO=armazenamento` + `SDTALLY_COMPLETO` | Título "Dados do Sistema"; primeiro bloco é a barra que se desdobra; "O disco inteiro" vem depois de "O que o macOS resolve sozinho" |
| P7 | `SDTALLY_SECAO=permissoes` | Título "Permissões e aparência"; seção "Aparência" só com "Abrir em" |
| P8 | `SDTALLY_SECAO=fazer` + `SDTALLY_COMPLETO=meu-mac-m2.json` | Sem o cartão "Medir como administrador". "Backup do Time Machine sem registro novo" presente. "Snapshots locais do Time Machine" (`time_machine_snapshots`) ausente. |

**Testes Swift** (`ResumoTests.swift` reescrito, `G9Tests` e `ConselhosTests`)
- `secoesDaBarra`: `Secao.barra.map(\.titulo) == ["Início","O que fazer","Dados do Sistema","Pastas maiores","Histórico"]` e `Secao.allCases.count == 7`.
- `causasDoDono`: `LinhaTop` montadas no teste com os bytes do dono, sem nome de usuário. Espera:
  - a ordem e os bytes `[49_615_863_808, 41_20…, 39_52… (2 pastas, card icloud_drive), 38_26…, 20_64…]`;
  - o swap na 9ª posição, com 9_181_462_528;
  - linha `folded` e linha com menos de 1 GB fora.
- `causaUsaSettingsLine`: `settingsLine` vence a provisória; duas linhas diferentes no grupo dão `"mixed"`; `protected_folders` sem `settingsLine` dá `nil`.
- `mancheteBackup`: valores do dono com `tm_backup_stuck` dão o texto exato da R1 e `ajustes == "time_machine"`; sem o conselho, e com 11,4 % do usado, dá `nil`.
- `manchetePedacoGrande`: 145_180_639_232 de 365_807_153_152 dá "…: 145,18 GB, 39,7 % do usado."; com `linha` = OtherUsers, termina em " No Ajustes, fica em Outros usuários."
- `conselhoDaCausa`: snapshots pega `tm_backup_stuck` antes de `time_machine_snapshots`; swap pega `restart_worth` antes de `swap_large`; pasta pega pelo `paths`.
- `nomeDaLinha`: `system_data`, `mixed`, `macos`, id da Apple, `nil`; e `categoria("x").cor == .gray`.
- `purgavelNoAjustes`: Disponível 76,04 com livre 24,07 e purgeable 51,55 dá `true`; Disponível igual ao livre dá `false`; sem tela dá `nil`.
- Apagar os 4 testes de `proximosPassos`. Os testes de `vazioAjustes`, `textoPasso` e `idadeMedicao` ficam.

**Testes Rust**
- `tm_parado_referencia_sem_privado`: referência com privado 0, 11 tentativas, `running: false`, termo de 49,6 GB. Dispara com `bytes` = termo e suprime `time_machine_snapshots`.
- `tm_parado_nao_sem_snapshots`: termo com menos de 1 GB não dispara.
- Os testes `tm_backup_stuck_*` de hoje continuam passando.
- `linha_ajustes_tabela`: pelo menos 22 casos da tabela do §9, incluindo `~/.colima/_lima` → Documents, `~/Library/Caches/CloudKit/com.apple.bird` → `system_data`, `/Users/x` → OtherUsers B, `~/Library` (exato) → `mixed`.
- `json_forma`: toda linha de `top[]` tem as chaves `settings_line` e `settings_line_grade` (null é permitido).
- `catalogo`: nenhum `Txt` contém "⌘6".

**Grep**
- `grep -c 'medir(admin: true' app/Sources/SDTally/ResumoView.swift` → `1`.
- `grep -c 'instalarHelper()' app/Sources/SDTally/ResumoView.swift` → `1`.
- `grep -n 'abrirArmazenamento()' app/Sources/SDTally/SDTallyApp.swift` → só o item do menu Medição.
- `grep -rn '⌘6\|⌘7' app/Sources src app/Tests/SDTallyTests/Fixtures` → vazio.
- `git diff --stat` não toca `app/Sources/SDTallyHelper`, `app/Sources/SDTallyXPC` nem `src/bin/sdc-coleta.rs`, e não entra dependência nova.

---

## 11. Arquivos e ordem dos marcos

| Marco | O quê | Arquivos | Pronto quando |
|---|---|---|---|
| **M1** Barra lateral e cor | `Secao` (ordem, títulos, símbolos, cores, raw String, `barra`), `List` com 5 itens, `Rodape` (⚙ ⓘ Github), barra de ferramentas sem "Abrir Armazenamento", menu Ir com `prefix(5)`, `.tint`, `categoria` com `.gray` e iOSFiles, "(⌘,)", rótulos dos `Vazios` | `SDTallyApp.swift`, `Marca.swift`, `Formato.swift`, `Vazios.swift`, `BoasVindasView.swift`, `ConselhosView.swift` (texto), `RankingView.swift`, `src/catalogo.rs`, `Fixtures/catalogo.json` | `secoesDaBarra`, g9, grep do ⌘6, foto de qualquer seção |
| **M2** Detector do Time Machine | `tm_parado` + 2 testes + linha do spec-fase6 | `src/conselho.rs`, `docs/spec-fase6.md` | `cargo test`; `meu-mac-m2.json` tem `tm_backup_stuck`; foto P8 |
| **M3** Início, parte 1 | Apagar os blocos antigos; B0, B1, B2, B3 (número, texto, popover da `Conta`, coluna das partes com a linha provisória), B4 (`LinhaMedicao`); `Causas.swift` (`causas`, `manchete`, `nomeLinha`, `linhaProvisoria`, `purgavelNoAjustes`); `SDTALLY_RAPIDO` | `ResumoView.swift`, `Causas.swift` (novo), `Foto.swift`, `Painel.swift`, `ResumoTests.swift`, `ArmazenamentoTests.swift` | testes de `manchete`, `purgavelNoAjustes` e `nomeLinha`; grep da senha; fotos P1 (a–e), P4, P5 |
| **M4** Início, parte 2 | B5 (barra, linhas, detalhe, navegação) e B6; `rolarPara` em O que fazer; filtro de `measure_as_admin` | `ResumoView.swift`, `Causas.swift`, `ConselhosView.swift`, `ResumoTests.swift` | `causasDoDono`, `conselhoDaCausa`; fotos P1 (f–h), P2, P8 |
| **M5** Linha do Ajustes | `src/linhas.rs` + a chamada em `anexar`; `json-schema.md`; `LinhaTop.settingsLine`; tags, partes do B3 e sufixo da R2 passam a usar o campo | `src/linhas.rs` (novo), `src/catalogo.rs`, `src/lib.rs`, `docs/json-schema.md`, `Modelo.swift`, testes | `linha_ajustes_tabela`, `json_forma`, `causaUsaSettingsLine`; P1 com as tags † |
| **M6** Dados do Sistema e aparência | Nova ordem da `ArmazenamentoView` + "O disco inteiro"; `Section("Aparência")`; "Abrir em" | `ArmazenamentoView.swift`, `PermissoesView.swift`, `SDTallyApp.swift` | fotos P6 e P7 |
| **M7** Fichas novas | As 6 fichas do §9.3 com refs conferidas (R6) e `Casa::Caminho` | `src/catalogo.rs`, `Fixtures/catalogo.json` | `tests/catalogo.rs`; P1 e P3 com nome e selo de Colima, Música, outra conta e WhatsApp |
| **M8** Aceite nos dois Macs | `colega-completo.json` e `colega-rapido.json` no scratch (§9.4); rodar P1 a P8 | nenhum no repositório | todos os critérios do §10 |

**Dependências**
- M2, M5 e M7 são do CLI e podem andar em paralelo com M1, M3 e M4.
- Cada marco do app funciona sem os do CLI: o que depende deles fica escondido.
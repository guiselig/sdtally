# SPEC: SDTally fase 5 (app + CLI), versão final

Histórico: registro da fase 5; o código atual manda onde divergir (ex.: o app lê a cada 30 s com a janela ativa, não 60 s).

> Ajustes do dono depois do desenho: sem regra de caches do JetBrains (pedido explícito de não sugerir); critério 1 vale localmente enquanto o CI estiver sem cobrança.

Os caminhos abaixo são relativos à raiz do repositório.

**O que conferi no código:**
- Do spec anterior, continua valendo:
  - `Secao { resumo, ranking }`, `JanelaView`, `Rodape` e o popover `Sobre`;
  - `Painel.vigiar()` dentro do `.task` da janela, e `applicationShouldTerminateAfterLastWindowClosed → true`;
  - `Executor.binario`, que aceita `SDTALLY_BIN` sempre;
  - `Painel.atualizarHelper()`: registra de novo quando o hash do helper muda e não olha `medindo`;
  - `Foto.tirarSePedido`;
  - o helper: `ehAdmin` privado em `SDTallyHelper/main.swift:63`, `reqApp`, só `medir`/`parar`, `reqCLI`;
  - `coletar_rapido` e `volumes.rs`.
- Conferido nesta rodada:
  - O `Package.swift` liga `SDTallyHelper` a `SDTallyXPC`. Mexer no `SDTallyXPC` muda o executável do helper.
  - `report/mod.rs:309`: a linha dobrada publica `path` sem o "/(demais)", com `folded: true`.
  - `report/mod.rs:266/455`: `take(3)` nos serviços.
  - `volumes.rs:57-86`: já lê `Internal` e o ISC; todo volume do disco interno que não é Data nem ISC entra no macOS da tela, inclusive o VM.
  - `snapshots.rs:50`: o tipo é "TimeMachine" ou "outro".
  - `montar.sh:34`: `@BUILD@` é a própria versão, sem o sufixo.
  - `research/frentes/G-metadados-apfs.md:213`: o sleepimage fica no volume Data e já aparece na travessia; o volume VM guarda os swapfiles e o kernelcore.

## 0. Conflitos entre as propostas e a escolha

| Tema | Escolha | Por quê |
|---|---|---|
| Onde fica o MCP | No Rust, com `sdtally --mcp`. Não fica no binário do app. | Nenhum processo novo chega ao helper root, e não precisa de FFI nem de Touch ID. O dado de root vem da última medição que o app salvou. |
| Subcomando ou flag | Flags `--mcp`, `--last` e `--history`. | `sdtally mcp` seria lido como uma pasta chamada "mcp". |
| Motor do "O que fazer" | Rust (`src/conselho.rs`, chave `advice`). O app traduz pelo `id`. | A CLI, a IA e o PDF recebem as mesmas regras, testadas com `cargo test`. A R6 (nome do processo) fica no app. |
| Comandos no "O que fazer" | Lista fixa de comandos oficiais, igual no Rust e no Swift. Nunca `rm`, `sudo` ou `tmutil delete`, e nunca um caminho medido dentro do comando. O app só oferece "Copiar comando" se o comando estiver na lista dele. | A ferramenta oficial confere o estado na hora em que roda. A lista no Swift protege contra JSON adulterado. |
| Formato do histórico | Stdout cru de cada medição completa, mais `pontos.jsonl`. A mais nova é a de `generated_at` maior. | Não exige `Encodable` no `Completo`, e `--last` lê o mesmo arquivo. |
| Retenção | 100 medições ou 50 MB. Pontos: 1 por hora, guardados por 90 dias. | Não cresce sem limite. |
| Cadência em segundo plano | A cada 30 min. Pula em Modo de Pouca Energia ou com `thermalState` em `[.serious, .critical]`. | O rápido custa cerca de 3 s de CPU. Com o app ativo, continua a cada 60 s. |
| Aviso de crescimento | (usado do container − purgeable) contra um ponto de 20 a 28 h atrás. Sem ponto nessa janela, não há aviso. Pouco espaço: (livre + purgeable) / total < 10 %. O ícone muda sempre; a notificação é opcional e começa desligada. | Sem alarme por intervalo de dias. O livre segue a mesma conta do Ajustes. |
| Modo de desenvolvimento | `Dev.ativo = Bundle.main.bundleIdentifier == nil` liga **todas** as `SDTALLY_*`. Dentro do .app, nenhuma vale. | Fecha de uma vez as portas de `SDTALLY_BIN`, `SDTALLY_COMPLETO`, `SDTALLY_DADOS` e companhia. |
| Helper root | `SDTallyHelper` e `SDTallyXPC` **não mudam** nesta fase. | Qualquer mudança troca o hash do helper e pode pedir nova aprovação em Itens de Início. |
| Ajustes | Sem cena `Settings`. ⌘, abre Permissões, que tem a seção "Comportamento". | Uma janela a menos, e dá para fotografar. |
| Nome da tela | "Informações" na barra lateral e "Sobre o SDTally" no menu. O popover passa a "Detalhes da medição". | Evita dois "Sobre". |
| Boas-vindas | Uma página só. Sem TipKit: as dicas viram `.help` e legendas. | Menos estado. |
| Gráfico do container | Donut (`SectorMark`) no topo de Armazenamento. O resto continua com `BarraView`. | O dono pediu Swift Charts. A `BarraView` já resolve as outras barras. |
| Gráfico do histórico | Linha de (usado − purgeable), com pontos de Dados do Sistema só onde houve leitura da tela, sem interpolar. | Interpolar seria estimar. |
| Exportação e MCP anônimos | Ficam fora. A v1 avisa que o arquivo traz o nome de usuário e nomes de pastas. | Exigiria um `--anonymize-stdin` (§8). |
| Chave nova dos volumes | `containers`, com `internal`, `roles` e sem nome de volume. | O nome pode ser pessoal. |
| Swap | `swap {total, used}` por `sysctl vm.swapusage`, sem root. O sleepimage **não** entra no VM: aparece no Data, pela linha `/private/var/vm` do `top`. | Divide o volume VM em swap e resto. O sleepimage já é medido pela travessia. |
| Grupos da barra lateral | Resumo; **Disco**: Armazenamento, O que mais pesa, O que fazer, Histórico; **App**: Permissões, Informações. | Separa os dados do disco dos do app. |
| Abrir medição antiga, ⌘O, "Copiar diagnóstico", Markdown | Ficam fora (§8). | Ninguém pediu. |
| Commit na tela Informações | Entra, e a linha "build" sai: hoje ela repete a versão. | O commit ajuda no relato de bug. |

## 1. Critério de sucesso

Considero feito quando:

1. Estes comandos passam localmente (o CI do GitHub está parado por cobrança; quando voltar, o job `app` no `macos-15` precisa passar também):
   - `CARGO_TARGET_DIR="$TMPDIR/sdt-cargo" cargo test`
   - `swift test --package-path app --scratch-path "$TMPDIR/sdt-test"`
2. `sdtally --quick --json` e `sdtally --json` trazem `containers` (com `internal` e sem nome de volume) e `swap`. Num teste e no Mac do dono vale a invariante: Σ `capacity_in_use` dos volumes com `in_settings_macos` = `settings_screen.macos.bytes`.
3. `sdtally --json` traz `advice` gerado só pelas regras do §6. Os testes provam:
   - todo `command` pertence exatamente à lista fixa;
   - nenhum `command` contém `rm `, `sudo`, `tmutil delete`, `deletelocalsnapshots`, `-rf`, `|`, `;`, `&&`, `$(`, crase ou `>`;
   - nenhum `text` contém `rm `, `sudo`, `tmutil delete`, `deletelocalsnapshots` ou `-rf`;
   - uma linha `folded` conta quando está dentro do prefixo da regra e nunca quando é ancestral dele;
   - os limites usam `private_bytes`.
4. Há fotos revisadas, tiradas com o binário cru e as fixtures sintéticas: as 7 seções no claro, Resumo e Armazenamento no escuro, as boas-vindas, a janela da barra de menus e a página 1 do PDF.
5. Armazenamento mostra cada parte do §2.4 com a fonte, inclusive "Partes do sistema no volume Data". A seção "O que ainda não é medido" lista só o que de fato falta.
6. Histórico:
   - duas medições da fixture geram "O que mudou";
   - uma medição com root contra uma sem root mostra "modos diferentes";
   - duas versões diferentes do `tool` mostram "versões diferentes do sdtally".
7. Com a barra de menus ligada, fechar a janela não fecha o app. Só existe um laço de leitura (teste `vigiarUmaVez`), e no início de sessão só o ícone aparece.
8. O JSON exportado é idêntico, byte a byte, ao stdout do sdtally. O PDF não tem autor.
9. `printf '%s\n' '<initialize>' '<notifications/initialized>' '<ping>' '<tools/list>' | sdtally --mcp` devolve exatamente 3 linhas: a notificação não recebe resposta, e `tools/list` traz 3 ferramentas, cada uma com `inputSchema`. Com euid 0, `--mcp`, `--last` e `--history` recusam. O `HelperProtocolo`, o `SDTallyXPC` e o `SDTallyHelper` não mudam (`git diff` vazio nos dois diretórios).
10. Um grep no CI reprova o job se houver:
    - em `app/Sources/SDTally`, `removeItem`, `trashItem`, `recycle(`, `moveItem`, `unlink`, `rmdir(` ou `removefile` fora de `Historico.swift`;
    - em `app/Sources`, `URLSession`, `NWConnection`, `NWListener` ou `socket(`;
    - em `src/`, `TcpStream`, `UdpSocket`, `remove_file` ou `remove_dir`.

## 2. Arquitetura de informação

### 2.1 Barra lateral (`Secao` em `SDTallyApp.swift`)

`enum Secao: Hashable { case resumo, armazenamento, ranking, fazer, historico, permissoes, info }`

| Grupo | Item | Atalho | Símbolo (`IconeSecao`) | Cor do quadrado | Selo |
|---|---|---|---|---|---|
| (sem título) | Resumo | ⌘1 | `gauge.with.dots.needle.50percent` | Disco | Armazenamento | ⌘2 | `internaldrive` | `.blue` | — |
| Disco | O que mais pesa | ⌘3 | `list.number` | `.indigo` | — |
| Disco | O que fazer | ⌘4 | `checklist` | `.green` | `.badge(n)`, com n = itens `safe` + `cost` |
| Disco | Histórico | ⌘5 | `chart.xyaxis.line` | `.teal` | — |
| App | Permissões | ⌘6 | `lock.shield` | `.gray` | `.badge("!")` se `rapido.fullDiskAccess == false` |
| App | Informações | ⌘7 | `info.circle` | `.gray` | — |

- O `Rodape` continua e ganha uma `BarraView(altura: 4)` com usado e livre de `rapido.capacity`. A hora traz a data quando não é de hoje: "Medido em 28/09 às 09:29".
- O popover `Sobre` passa a se chamar `DetalhesMedicao`, com o título "Detalhes da medição".

### 2.2 Menus (`.commands`)

- **SDTally**, com `CommandGroup(replacing: .appInfo)` e `.appSettings`:
  - "Sobre o SDTally" vai para `.info` e traz a janela para a frente;
  - "Ajustes…" ⌘, vai para `.permissoes`;
  - "Boas-vindas e dicas".
- **Arquivo**, com `CommandGroup(replacing: .importExport)`. Os dois "Exportar" ficam desligados sem medição completa:
  - "Exportar PDF…" ⌘E;
  - "Exportar JSON…" ⌥⌘E;
  - "Mostrar medições salvas no Finder" (desligado com `Historico.pasta == nil`).
- **Medição**: os itens atuais, mais "Abrir Armazenamento no Ajustes" ⌥⌘A.
- **Ir** (`CommandMenu`, substitui o `CommandGroup(before: .sidebar)`):
  - ⌘1 a ⌘7;
  - "Voltar" ⌘[, desligado quando `secao != .ranking` ou quando a pilha está vazia.
- **Ajuda**, com `CommandGroup(replacing: .help)`: "Boas-vindas e dicas" e "Abrir repositório no navegador" (`NSWorkspace.open`, só no clique).

### 2.3 Boas-vindas

- É um `.sheet` na `JanelaView`, controlado por `@AppStorage("viuBoasVindas")`, e reabre pelo menu.
- Com `SDTALLY_FOTO`, nunca aparece. A exceção é `SDTALLY_SECAO=boasvindas`, que a renderiza com `Foto.renderizar`.
- Não pede permissão nenhuma. Os textos estão no §5.

### 2.4 Telas

**Resumo** (`ResumoView.swift`), na ordem:
1. `CartaoAjustes`: o segmento `"sd"` passa a usar `Marca.cor`.
2. `Conta` com `.cartao(destaque: Marca.cor)`. O número vem em `Font.heroi` e, abaixo dele, "Desde a última leitura" (A5).
3. Três `FichaKPI`:
   - Livre (`capacity.free`);
   - O macOS libera sozinho (`purgeable.bytes`);
   - Sem explicação (`unexplained.percent_of_used`), com a idade da medição ("há 2 dias").
4. "Próximos passos": até 3 `advice` dos níveis `measurement`, `safe`, `cost` e `check_first`, ordenados por `private_bytes` (ou `bytes`), e o link "Ver todos (⌘4)". Com a medição de mais de 24 h, aparece a `Faixa` da R7.
5. Barra compacta dos termos, colorida, com o link "Ver tudo em Armazenamento (⌘2)".
6. `CartaoMedicao`.

`Medido` e `Informativo` saem do Resumo e vão para Armazenamento.

**Armazenamento** (`ArmazenamentoView.swift`, novo). Vai do físico para o detalhe, e cada número passa pela `NumeroView`, que mostra a fonte. Só entram os `containers` com `internal == true`.

| # | Parte | Fonte real | Estado |
|---|---|---|---|
| 1 | Disco interno: capacidade e livre | `capacity.total/free` (statfs). Do container: `containers[holds_data].ceiling/free` | `containers` é novo (R1) |
| 2 | Donut do container: Data, macOS da tela, livre | `data_volume.used`, `settings_screen.macos`, `containers[].free` | Novo |
| 3 | Volumes do container, um por linha: System (selado, só leitura), Preboot, Recovery, VM, Update, com o selo "entra no macOS da tela" | `containers[].volumes[] {roles, capacity_in_use, in_settings_macos}` | Novo. Já coletado em `volumes.rs` |
| 4 | Dentro do VM: arquivos de swap e resto do VM (kernelcore e outros) | `swap.total` (sysctl). O resto é `capacity_in_use(VM) − swap.total`, com a fonte "diferença". Se der negativo, mostra "—" com "leituras em momentos diferentes" | **Medição nova** (R1), sem root |
| 5 | Container ISC (xART, Hardware), com "o Ajustes não mostra" | `containers[isc == true]` | Novo |
| 6 | Como o Ajustes divide: categorias, Dados do Sistema, macOS, Disponível, com "Lido às HH:MM" ou "Não lido na última hora" | `settings_screen.*`, `logged_at`, `state` | Existe. Recebe o estado que saiu de Permissões |
| 7 | Apple Intelligence (informativo, dentro de Dados do Sistema) | `settings_screen.apple_intelligence` + `physical` | Existe |
| 8 | Conta do volume Data termo a termo, com Sem explicação tracejado e as pastas negadas | `data_volume.terms`, `.unexplained`, `denied_folders` | Existe, agora colorido |
| 9 | **Partes do sistema no volume Data**: soma das linhas do `top` por grupo, com link para o detalhe | Grupos por prefixo de `top[].path`: `/private/var/db`, `/private/var/folders`, `/private/var/vm` (imagem de repouso), `/Library`, `/System` (lado Data), `~/Library/Caches`, `~/Library/Developer`, `~/Library/Containers`, `~/Library/Application Support`, `~/.Trash`. Só no completo, sem medição nova | Novo (só no app) |
| 10 | Snapshots, um por um | `informational.snapshots [{kind ∈ time_machine/apple_other/other, date (só TM), private_bytes}]` | Novo (R1) |
| 11 | Apagados-abertos por processo | `informational.deleted_open_processes [{name, bytes, files}]` (os 10 maiores) | Novo (R1) |
| 12 | O que o macOS libera sozinho, por serviço | `purgeable.top_services`, todos acima de 0 | Alterado (R1) |
| 13 | Informativo: clones, hardlinks, só na nuvem, raízes de enxerto, `scan_state` | `informational.*` | Existe |
| 14 | Estado da medição | `incomplete_reasons`, `stage_errors` | Existe |
| 15 | **O que ainda não é medido** (texto fixo) | ver abaixo | — |

O que ainda não é medido, e de onde viria:
- **Categorias sem abrir o Ajustes**: a Frente Q (`research/experimentos/frenteQ`) vira `src/sources/categorias.rs`. Sete categorias saem da travessia com `STMSize`. Fotos depende de um framework privado.
- **Metadados do APFS**: seletor 0x1e do `AppleAPFSUserClient` (Frente G). Hoje caem em Sem explicação.
- **Dentro de Preboot e Recovery**: o diskutil só dá o total.
- **Dados do Sistema por linha** (Fase 2) e **dono de cada pasta por app** (Frente C).
- Nota fixa: "Aqui, memória é espaço em disco. A RAM aparece no Monitor de Atividade; a ponte entre as duas é o swap, no volume VM."

**O que mais pesa**: a estrutura não muda. A `BarraOcupa` recebe a cor do termo: "Ocupa" a 35 % e "Libera se apagar" em cor cheia.

**O que fazer** (`ConselhosView.swift`): ver §4.4 e §6.

**Histórico** (`HistoricoView.swift`): ver §4.1.

**Permissões** (`PermissoesView.swift`). Uma `LinhaPermissao` por item, com o estado, para que serve, o que se perde sem ela e um botão:

| Linha | Estado lido de | Botão |
|---|---|---|
| Acesso Total ao Disco | Dois estados: "neste app" (`rapido.fullDiskAccess`; nil = "Indeterminado") e "na última medição completa (modo X)" (`completo.fullDiskAccess`, `completo.root`). Se divergirem, aparece uma `Faixa` de aviso | Abrir Privacidade e Segurança |
| Medidor root | `painel.helper`: os 4 estados de `SMAppService.Status`. Com `Dev.ativo`: "Fora do .app (desenvolvimento)" | Instalar…, Remover ou Abrir Itens de Início. Desligado durante a medição e fora do .app |
| Conta de administrador | `ehAdmin(getuid())`, cópia própria no target do app (o helper fica com a dele) | — |

Também ficam em Permissões:
- a seção "Comportamento" (A6): Ícone na barra de menus (`@AppStorage("barraDeMenus")`, ligado por padrão), Avisar com notificação (`@AppStorage("notificar")`, desligado; ligar pede `requestAuthorization` só fora do `Dev.ativo`) e Abrir ao iniciar sessão (`SMAppService.mainApp`, desligado, só no .app);
- o bloco fixo "O SDTally nunca".

**Informações** (`InfoView.swift`): `IconeApp` de 96 pt, o nome em `.rounded` bold e um `Form(.grouped)` com:
- Versão `CFBundleShortVersionString`. Com `Dev.ativo`: "Fora do .app (desenvolvimento)" e `rapido.tool`.
- sdtally: `rapido.tool`. Formato do JSON: schema 1.
- Repositório: o link `github.com/guiselig/sdtally` ("privado por enquanto"), que só abre no clique. Licença MIT, © 2026 Guilherme Selig.
- Assinatura: "Apple Development · time XXXXXXXXXX" (`HelperXPC.timeProprio()`, que já existe), ou "ad hoc (o medidor root não funciona)".
- Caminhos, com `abbreviatingWithTildeInPath` e o `MenuArquivo`: o app, o sdtally (`Executor.binario`), o helper e a pasta de dados.
- Mac: `operatingSystemVersionString`, `machdep.cpu.brand_string` e `hw.model`.
- "Só lê · Sem rede · Sem telemetria".
- Seção "Para agentes de IA" (§5).

### 2.5 Barra de menus (`MenuBarExtra(isInserted:content:label:)`, `.menuBarExtraStyle(.window)`)

- **Rótulo**: o glifo `TallyShape` como `NSImage` template e o **livre**. Com aviso ativo, o glifo dá lugar a `exclamationmark.triangle`. Dados do Sistema nunca vai no rótulo.
- **Janela** (300 pt, `BarraMenusView`):
  - "Dados do Sistema 145,99 GB · como no Ajustes às 09:29 (há 3 h)". Vem de `telaBoa`; sem ela, do último `Ponto` com `systemData`, com a data e a hora dele.
  - `BarraView(altura: 8)` das categorias, quando houver tela.
  - Duas `FichaKPI` compactas: Livre e O macOS libera sozinho.
  - Gráfico pequeno dos 7 dias de pontos.
  - "Mais cresceu: ~/Library/Developer +4,1 GB", tirado do último "O que mudou".
  - A linha do aviso ativo.
  - Botões: Medir · Ler do Ajustes · Abrir SDTally (`openWindow(id: "principal")` + `NSApp.activate()`) · Sair.
- Abrir a janela da barra de menus atualiza o rápido se a última leitura tiver mais de 60 s.

## 3. Sistema visual

### 3.1 Tokens (`app/Sources/SDTally/Marca.swift`, novo)

**Regra (revisão "só cores de sistema"):** o app não tem cor de marca nem hex. Só cores de sistema (`.blue`, `.orange`, `.gray`…) e semânticas (`.primary`, `.secondary`, `.separator`…); os controles seguem o destaque do macOS (`.tint` padrão). Não há `.xcassets`. Os hex e o gradiente magenta desta tabela foram removidos; a coluna "Valor" abaixo é a atual.

| Token | Valor | Uso |
|---|---|---|
| `Marca.dados` | `.gray` | Dados do Sistema (número em `.primary`; ícone, ▮, segmento e ponto em cinza, como no Ajustes). Nunca o destaque do sistema, que pode ser vermelho (G9) |
| `Marca.aviso` | `.orange` | Toda faixa de aviso, sempre com símbolo |
| `Marca.bom` | `.green` | Permissão ok, sempre com símbolo |
| `Marca.cor`, `Marca.gradiente`, `Marca.dinamica` | removidos | Eram o magenta e o gradiente da marca |
| `IconeApp` | `Color.blue.gradient` | Ícone do app (fixo, para o `.icns` sair igual em qualquer Mac) |
| `Marca.nivel(_:) -> (Color, String)` | ver §3.2 | | Mapa nível → cor e símbolo, usado por Resumo, O que fazer e PDF |

Tipografia, forma e materiais:
- `Font.heroi = .system(size: 44, weight: .bold, design: .rounded).monospacedDigit()` com `.tracking(-0.5)`.
- `Font.kpi = .system(.title2, design: .rounded, weight: .semibold).monospacedDigit()`.
- Cantos de 12 (cartões), 8 (fichas) e 6 (barras). Espaçamentos de 8, 12, 16 e 24.
- Cartões opacos (`.background.secondary`). Material só no que flutua.
- Com `colorSchemeContrast == .increased`, borda de 1 pt e tracejado de 1,5 pt.

### 3.2 Cor por termo (campo `cor` em `termo(_:)`, `Formato.swift`)

| Termo | Cor de sistema |
|---|---|
| `tree` | `.teal` |
| `large_xattrs` | `.green` |
| `snapshots` | `.orange` |
| `deleted_open` | `.indigo` |
| `protected_folders` | `.yellow` |
| Sem explicação | tracejado `.secondary` | |
| macOS e volumes | cinzas do sistema, um símbolo por volume (VM: `memorychip`) | |

- Nenhum termo usa o cinza de Dados do Sistema nem vermelho; vizinhos têm cores distintas. `.purple` fica fora (no escuro parece magenta).
- O texto fica sempre na cor de texto; a cor vai só na marca ao lado. A legenda com nome fica sempre visível.
- O purgeable é um colchete `.secondary` com `arrow.uturn.backward.circle`.
- Na implementação, rodar o validador de novo com o `windowBackgroundColor` real.

Níveis (só no selo e no símbolo):

| Nível | Cor | Símbolo |
|---|---|---|
| `system_handles` | `.blue` | `checkmark.circle` |
| `safe` | `Marca.bom` | `leaf` |
| `cost` | `Marca.aviso` | `clock.arrow.circlepath` |
| `check_first` | `.orange` | `exclamationmark.magnifyingglass` |
| `do_not_touch` | `.red` | `hand.raised` |
| `anomaly` | `.purple` | `questionmark.diamond` |
| `measurement` | `.secondary` | `gauge.with.dots.needle.50percent` |

### 3.3 Componentes novos

| Componente | Arquivo | Responsabilidade |
|---|---|---|
| `.cartao(destaque:)` | Marca.swift | Substitui o `GroupBox`: padding 16, fundo opaco, borda no destaque |
| `.transicaoNumero(_:)` | Marca.swift | `.numericText`, ou `.opacity` com Reduzir Movimento |
| `Faixa(simbolo:tom:texto:acao:)` | Marca.swift | Generaliza a `FaixaAcessoTotal` |
| `TallyShape` | Marca.swift | Quatro traços e uma diagonal |
| `IconeApp(tamanho:)` | Marca.swift | Grade de 1024, área útil de 824, raio 185 contínuo, gradiente, brilho de 25 %, sombra de 30 % e traços brancos. Em 64 px ou menos, três traços e a diagonal |
| `FichaKPI` | Marca.swift | Número em `Font.kpi` com fonte e variação |
| `IconeSecao` | SDTallyApp.swift | Quadrado de 20 pt com símbolo branco |
| `DonutDisco` | ArmazenamentoView.swift | `SectorMark` com `innerRadius: .ratio(0.62)` e `angularInset: 1.5` |
| `GraficoHistorico` | HistoricoView.swift | `LineMark` + `AreaMark`, `PointMark` de Dados do Sistema e `chartXSelection` com `RuleMark` |
| `LinhaPermissao` | PermissoesView.swift | Estado, nome, para que serve, o que se perde sem ela e o botão |
| `CartaoConselho` | ConselhosView.swift | Faixa de 3 pt na cor do nível, título, caminhos, "Libera se apagar" em destaque e "Ocupa" abaixo, as quatro seções, "Mostrar no Finder", "Copiar caminho" e "Copiar comando" (só se o `command` estiver em `comandosPermitidos`) |
| `BarraMenusView` | BarraMenus.swift | Conteúdo da `MenuBarExtra` |
| `RelatorioView` | Relatorio.swift | Páginas do PDF só com `VStack` e `Grid` |

A `BarraView` não é reescrita.

### 3.4 Ícone do app

- Com `Dev.ativo`, `SDTALLY_ICONE=/pasta` renderiza o iconset (16 a 1024 px, @1x e @2x) com `ImageRenderer` e sai. Fica em `Foto.swift`.
- O `montar.sh`:
  - roda isso no binário cru;
  - chama `iconutil -c icns` e copia o resultado para `Contents/Resources/SDTally.icns`;
  - põe `CFBundleIconFile` no Info.plist.
- Nenhum binário entra no repositório.

## 4. Funções

### 4.1 Histórico (`Historico.swift`, novo)

Todos os tipos e funções puras (`Ponto`, `Mudanca`, `Aviso`, `mudancas`, `aviso`, `pasta`) são `nonisolated`, como no `Modelo.swift`.

**Pastas.** Leitura e gravação seguem regras diferentes:
- **Leitura**: `pastaLeitura(env:bundleID:)`. Com `Dev.ativo` e `SDTALLY_DADOS` definido, lê dessa pasta e **só lê**. Com bundle id, lê de `~/Library/Application Support/SDTally/`. Nos outros casos, devolve `nil`.
- **Gravação**: `pastaGravacao(bundleID:)`. Só com bundle id e fora do `Dev.ativo`. Gravar e podar recebem a URL como parâmetro, e os testes usam uma pasta temporária. O xctest tem bundle id, por isso nenhum teste chama `pastaGravacao`.
- A pasta tem 0700, os arquivos 0600, e a gravação é `.atomic`.

**Arquivos.**
- `Medicoes/AAAA-MM-DD-HHMM.json`: o stdout cru de cada medição completa. Em colisão de nome, acrescenta `-2`. Arquivo corrompido ou de outro schema é pulado. A ordem vem de `generated_at`, não do nome.
- `pontos.jsonl`: `nonisolated struct Ponto: Codable { t: String; total, usado, livre, purgeable, systemData: Int64?; telaEm: String? }`.
  - `usado` = `ceiling − free` do container interno com `holds_data`.
  - `livre` e `total` vêm do statfs.
  - Grava no máximo um por hora, ou quando chega uma tela com `loggedAt` novo. Só esse caso preenche `systemData`.

**Retenção** (`podar(url:)`):
- 100 medições ou 50 MB, e pontos de até 90 dias (o `pontos.jsonl` é reescrito).
- Só remove arquivos de `Medicoes/` cujo nome casa com `^\d{4}-\d{2}-\d{2}-\d{4}(-\d+)?\.json$` e que não são symlink. É o único `removeItem` do target.

**Restaurar.** Ao iniciar, se `completo == nil`, carrega a mais nova. O rodapé mostra a data.

**Comparação.**
- `comparavel(a, b)` exige `root`, `fullDiskAccess`, `complete` e `tool` iguais, e nenhuma das duas `interrupted`.
- `mudancas(de:para:) -> [Mudanca]?`:
  - devolve `nil` se não forem comparáveis;
  - compara cada termo e as linhas do `top` pela chave `path|term|folded`;
  - entram as diferenças com |Δ| ≥ 100 MB;
  - uma linha presente numa só das medições aparece como "nova na lista" ou "saiu da lista (abaixo do limite ou apagada)", nunca como 0.
- A leitura dos N arquivos roda numa função `@concurrent`.

**Tela.**
- `GraficoHistorico`.
- Lista das medições: data, selo do modo, usado, sem explicação.
- "O que mudou desde a última vez".
- Sem par comparável, por modo: "As duas últimas medições foram feitas em modos diferentes (com e sem senha); parte da diferença seria o que não abriu."
- Sem par comparável, por versão: "As duas últimas medições são de versões diferentes do sdtally; a divisão das pastas pode ter mudado."

### 4.2 Barra de menus e avisos (`BarraMenus.swift`, novo; `Painel.swift`; `SDTallyApp.swift`)

- **Laço de leitura.** O `Painel` passa a ser uma instância única, acessível ao `Delegado`. `Painel.iniciar()` é idempotente (flag `vigiando`) e cria uma `Task` guardada no `Painel` que roda `vigiar()`. É chamado em `Delegado.applicationDidFinishLaunching`, não no `.task` do rótulo. Fechar a janela não cancela o laço.
- **Cadência**: 5 s enquanto aguarda o Ajustes; 60 s com o app ativo; 1800 s em segundo plano. Pula quando `isLowPowerModeEnabled` ou `[.serious, .critical].contains(thermalState)`. A medição completa continua só por clique.
- **Janela e ciclo de vida.**
  - `applicationShouldTerminateAfterLastWindowClosed` devolve `!barraDeMenus`. O padrão `barraDeMenus = true` fica em `UserDefaults.register`.
  - O `Window(id: "principal")` usa `.defaultLaunchBehavior(barraDeMenus ? .suppressed : .automatic)`. No início de sessão aparece só o ícone, e a janela abre pelo menu ou pelo Dock.
- **`aviso(pontos:agora:ultimo:) -> Aviso?`** (pura):
  - crescimento: (usado − purgeable) agora menos o mesmo valor do ponto mais próximo de 24 h **dentro de 20 a 28 h atrás** ≥ 10 GB. Sem ponto nessa janela, não avisa. O aviso leva a hora real do ponto;
  - pouco espaço: (livre + purgeable) / total < 10 %;
  - histerese: um aviso de cada tipo a cada 24 h (`ultimoAviso.<tipo>`).
- **Notificação** (`UNUserNotificationCenter`): só com `notificar == true` e fora do `Dev.ativo`. O texto traz só números, sem caminhos. Clicar abre o Histórico.

### 4.3 Exportar (`Relatorio.swift`, novo)

- **JSON**: grava `painel.completoDados` sem nenhuma transformação.
- **PDF**:
  - `RelatorioView` em tema claro, uma página por seção, com 595 pt de largura;
  - gerado com `ImageRenderer.render` num `CGContext(url:mediaBox:auxiliaryInfo:)` que leva só `kCGPDFContextCreator = "SDTally"`;
  - páginas: (1) cabeçalho, barra do Ajustes e conta; (2) Armazenamento; (3) as 25 maiores; (4) O que fazer; (5) Histórico, se houver pontos.
- **`NSSavePanel`**: nome `SDTally AAAA-MM-DD`, pasta Downloads, aviso no `message` (§5). O arquivo não abre sozinho depois de salvo.

### 4.4 "O que fazer" (Rust `src/conselho.rs`; app `ConselhosView.swift`)

**Rust.**
- `pub fn conselhos(c: &Conta, extra: &Extra) -> Vec<Conselho>`, pura. `Extra` traz os snapshots por tipo e o swap.
- Sai em `advice`, só no `--json` completo:
  `{"id","level","bytes","private_bytes"|null,"paths":[..],"term"|null,"process"|null,"command"|null,"text"}`.
  O `text` é em inglês. Quando há `command`, termina em "Ask the user before running."
- As regras são dados: um `const` com prefixo, nível, comando e limite.
  - O casamento é por componente inteiro.
  - `*` só no fim do último componente.
  - Uma linha casa se o `path` for igual ao prefixo ou estiver dentro dele. Vale também para as linhas `folded`: `~/Library/Logs` dobrada casa com `logs`, e `~/Library/Caches` dobrada não casa com `homebrew_cache`. Linha ancestral nunca casa.
  - Só entram termos `measured` ou `partial`.
  - Várias linhas da mesma regra viram um item só, com Σ `bytes` e Σ `private_bytes`.
- **Limite e destaque**: Σ `private_bytes` ("Libera se apagar"). Quando `private_bytes` é null, usa `bytes`, e o app mostra "≤".
- **Lista fixa de comandos**: `brew cleanup --dry-run`, `npm cache clean --force`, `go clean -cache`, `pip cache purge`, `xcrun simctl delete unavailable`, `xcrun simctl runtime list`, `ollama list`, `tmutil listlocalsnapshots /`. Nenhum leva caminho medido.
  - **Dry-run conferido (Fase 3):** cada um tem em `conselho::SIMULACOES` o comando que só mostra ou lista (`brew cleanup --dry-run`, `npm cache ls`, `go clean -cache -n`, `pip cache info`, `xcrun simctl list devices`; os outros três já só leem) e a fonte. Só o `go clean` tem simulação própria; no npm, no pip e no `simctl delete` a "simulação" é a listagem. `advice[].dry_run` leva isso e o app mostra "Ver antes". `tests/dryrun.rs` reprova comando novo sem linha.

**App.**
- `textoConselho(_ id:)` em pt-BR. Para um `id` desconhecido, mostra o `text`.
- `comandosPermitidos`: a mesma lista do Rust. Fora dela, o botão "Copiar comando" não aparece.
- Ordem: `measurement`, `system_handles`, `safe`, `cost`, `check_first`, `anomaly`, `do_not_touch`; dentro do nível, por `private_bytes` (ou `bytes`).
- **R6** (`deleted_open`): só diz "Feche e abra de novo o <app>" quando exatamente um `runningApplications` com `activationPolicy == .regular` tem `executableURL?.lastPathComponent` igual ao `process`, ou começando por ele quando `process` tem 15 caracteres ou mais (o `p_comm` vem cortado). Nos outros casos: "Reiniciar o Mac libera esse espaço."
- **R7**: com a medição de mais de 24 h, aparece a faixa "Medido <idade>. Meça de novo antes de agir." Linha com `lower_bound` mostra "≥".
- **R3**: só "Mostrar no Finder" (`activateFileViewerSelecting`). Nunca `NSWorkspace.open` num caminho medido.
- Rodapé fixo (§5).

### 4.5 Acesso por IA (Rust)

- **Guarda comum**: `--last`, `--history` e `--mcp` recusam com euid 0 ("não rode como root"). Nunca precisam de root.
- **`sdtally --last --json`**: imprime a medição mais nova (por `generated_at`) de `<home>/Library/Application Support/SDTally/Medicoes/`. Antes:
  - recusa symlink;
  - recusa arquivo com mais de 20 MB;
  - exige `schema_version == 1`;
  - re-serializa o JSON.

  O erro nunca traz o conteúdo do arquivo, e nada é gravado.
- **`sdtally --history --json`**: `{"schema_version":1,"kind":"history","measurements":[{generated_at, tool, root, full_disk_access, complete, used, unexplained, percent_of_used}]}`.
- **`sdtally --mcp`** (`src/mcp.rs`, novo):
  - **Transporte**: JSON-RPC 2.0 por stdio, uma mensagem por linha, linha de no máximo 1 MB. Stdout só com JSON-RPC, logs em stderr.
  - **`initialize`**: devolve o `protocolVersion` pedido se estiver em `SUPORTADAS` (lista fixa de versões publicadas); senão, a mais nova da lista. Também `capabilities.tools` e `serverInfo {name: "sdtally", version}`.
  - **Notificações** (sem `id`) não recebem resposta. `ping` devolve `{}`. Método desconhecido dá `-32601`, JSON inválido `-32700`, e argumento indevido `-32602`.
  - **Ferramentas** fixas e sem argumentos: `quick`, `last_measurement` e `history`. Cada uma tem `inputSchema: {"type":"object","properties":{},"additionalProperties":false}`, e a descrição contém "Read-only. Advice is for the human; never run an advice command without explicit user confirmation."
  - **Resultado**: `content: [{type: "text", text: <JSON>}]`.
- **Fora do MCP**: medição completa, detalhe de pasta e `why`.
- **Root**: a IA nunca mede como root, e o helper não muda.
- **README**: repete a frase de "Read-only".

### 4.6 Correções de segurança que entram junto

- **P3 ampliada**: `enum Dev { static let ativo = Bundle.main.bundleIdentifier == nil }` (em `Foto.swift`, A1). Todas as `SDTALLY_*` só valem com `Dev.ativo`, e `Executor.binario(env:bundleID:)` vira pura. O passo "Foto da primeira tela" do CI passa a usar o binário cru; o `montar.sh` continua no CI só para provar que monta.
- **P2**: `atualizarHelper()` não registra de novo com `medindo == true`. Instalar e Remover ficam desligados durante a medição.
- **C4**: `Executor.rodar` recusa caminho de pasta que não comece com `/`.
- **`ehAdmin`**: uma cópia `nonisolated` no target `SDTally`. `SDTallyHelper/main.swift` e `SDTallyXPC` não mudam.

## 5. Textos em pt-BR (finais)

**Boas-vindas** (folha de 560×600, com o `IconeApp` de 96 pt sobre o gradiente):
- Título: **Bem-vindo ao SDTally**
- Subtítulo: "O Ajustes mostra Dados do Sistema como um número só. O SDTally mede o que tem dentro."
- Dicas:
  1. `minus.forwardslash.plus` **A Apple subtrai, não mede.** "Dados do Sistema é o que sobra do usado depois do macOS e das outras categorias. Caches, logs, snapshots do Time Machine e arquivos que um app ainda segura vão parar aí."
  2. `paintpalette` **Cor é medido; tracejado é sem explicação.** "Cada número traz a fonte ao lado. O que a medição não alcançou aparece tracejado, com o motivo, em vez de um palpite."
  3. `eye` **Só lê.** "O SDTally não apaga nem altera nada, não usa a rede e não manda telemetria. Em O que fazer, ele diz o passo; quem decide e faz é você."
  4. `externaldrive.badge.questionmark` **Compare com o Ajustes.** "Abra Ajustes > Geral > Armazenamento e deixe a tela aberta alguns segundos: o SDTally lê a conta do próprio Ajustes e mostra o mesmo número, com a hora."
- Linha final: "Para medir as pastas do sistema, o SDTally usa o Acesso Total ao Disco e a senha de administrador (ou o medidor root). Os dois são opcionais; veja em Permissões (⌘6)."
- Botões: "Abrir Armazenamento no Ajustes" e **"Começar"** (padrão).

**Dicas no contexto:**
- Ranking: "Dê dois cliques numa pasta para ver o que tem dentro."
- Conta: "Passe o mouse num número da conta para ver a parte dele na barra."
- Fonte: "Clique na fonte para ver a chave bruta, o motivo e os bytes exatos."
- VM: "Swap é memória que foi parar no disco. Fechar apps pesados ou reiniciar devolve o espaço."
- Imagem de repouso (Partes do sistema): "Cópia da memória que o Mac guarda para acordar; o macOS controla o tamanho."
- Histórico: "Depois de seguir um passo de O que fazer, meça de novo: aqui aparece quanto voltou."
- (Removido: a regra "app sem subtextos", em spec-inicio.md, tirou este rodapé do app; fica só no relatório em PDF.) Rodapé de O que fazer: "O SDTally não apaga nada. Cada passo é seu, no Finder, no Terminal ou no app dono. Depois, meça de novo: o Histórico mostra quanto voltou."
- Todo passo com "Lixo pelo Finder" termina em "O espaço volta quando o Lixo é esvaziado."

**Permissões:**
- **Acesso Total ao Disco**
  - Para que serve: "Deixa o SDTally ler pastas que o macOS esconde até do administrador, como Mail, Mensagens e Safari."
  - Sem ele: "Essas pastas caem em Sem explicação e a medição sai incompleta. Durante a medição, o macOS também pode perguntar por Documentos, Mesa e Downloads."
  - Estados: "Neste app: ativo/desativado/indeterminado" e "Na última medição completa (com senha/medidor root/sem root): ativo/desativado".
  - Divergência: "O medidor root tem a própria entrada em Acesso Total ao Disco; a última medição completa rodou sem ele."
- **Medidor root**
  - Para que serve: "Mede como administrador sem pedir a senha a cada vez. É um serviço do próprio SDTally, aprovado uma vez em Itens de Início."
  - Sem ele: "A medição completa pede a senha de administrador e não dá para parar no meio."
  - Estados: "Ativo" / "Aguardando aprovação em Itens de Início" / "Não instalado" / "Fora do .app (desenvolvimento)".
- **Conta de administrador**
  - Para que serve: "A senha, o medidor root e a leitura do registro do Ajustes exigem uma conta de administrador."
  - Estados: "Sim" / "Não. Peça a um administrador para medir."
- **Ícone na barra de menus**: "Mostra o espaço livre sem abrir a janela e avisa quando o disco cresce muito."
- **Avisar com notificação**: "Manda uma notificação quando o disco cresce 10 GB em um dia ou fica com menos de 10 % livre. Sem ela, o aviso aparece só no ícone."
- **Abrir ao iniciar sessão**: "Deixa o ícone da barra de menus pronto depois de ligar o Mac, sem abrir a janela."
- **"O SDTally nunca"**: "apaga ou altera arquivos seus · usa a rede · manda telemetria · pede permissão sem você clicar".

**Informações → Para agentes de IA:**
- "Agentes de IA, como o Claude Code e o Claude Desktop, podem ler as medições pelo sdtally. A resposta inclui seu nome de usuário e nomes de pastas da sua home, e vai para o provedor da IA que você usa: o SDTally não manda nada pela rede, mas o cliente de IA manda."
- Comandos, cada um com o botão "Copiar":
  - `"<sdtally>" --quick --json`
  - `"<sdtally>" --last --json`
  - `claude mcp add sdtally -- "<sdtally>" --mcp`
- Nota: "Pela IA, o sdtally nunca mede como administrador nem executa os passos de O que fazer; a última medição completa que você fez no app é a que ela lê."

**Notificações:**
- "O disco usou mais 12,4 GB desde ontem às 09:00." / "Abra o SDTally para ver o que cresceu." A hora é a do ponto real.
- "Restam 8 % do disco livres, contando o que o macOS libera sozinho." / "Veja em O que fazer o que dá para liberar."

**Exportar** (`message`): "O arquivo traz seu nome de usuário e nomes de pastas da sua home (por exemplo, ~/Documents). Revise antes de compartilhar."

**Estados vazios:**
- "Nenhuma medição salva ainda. Cada medição completa entra aqui."
- "As recomendações saem da medição completa." (botão "Medir com senha de administrador…")
- "Histórico desligado fora do .app (desenvolvimento)."

## 6. Regras do "O que fazer"

**Limite padrão**: Σ `private_bytes` ≥ 1 GB (ou `bytes` quando null, mostrado com "≤"). Nos itens `do_not_touch`, ≥ 5 GB, só para responder "posso apagar isso?". Nos termos fora de `tree`, vale a medida indicada na própria regra. Nenhuma regra e nenhum botão apaga nada.

| id | Condição sobre o dado medido | Recomendação (pt, resumida) | Nível | Fonte |
|---|---|---|---|---|
| `grant_full_disk_access` | `full_disk_access == false` | Dar Acesso Total ao Disco (link para Permissões) | measurement | raiz |
| `measure_as_admin` | `!root` e `unexplained.percent_of_used > 2` | Medir como administrador: hoje X % do usado está sem explicação | measurement | `root`, `unexplained` |
| `time_machine_snapshots` | Σ `private_bytes` dos snapshots `time_machine` ≥ 1 GB | Nada a fazer: o macOS apaga sozinho quando falta espaço. Para ver: `tmutil listlocalsnapshots /` | system_handles | `fsctl_snapshot_private_size` |
| `other_snapshots` | Σ `private_bytes` dos `kind == other` ≥ 1 GB | Snapshot de outro app (backup ou clonagem); gerencie no app que criou | check_first | idem |
| `purgeable_auto` | `purgeable.bytes` ≥ 5 GB | Nada a fazer: o macOS libera quando precisar | system_handles | `cachedelete` |
| `deleted_open` | termo `deleted_open` ≥ 1 GB; `process` = `top_process.name` | Feche e abra de novo o app (R6), ou reinicie o Mac | safe | `lsof_proc_pidfdinfo` |
| `swap_large` | `swap.total` ≥ 8 GB | O Mac está usando o disco como memória; fechar apps pesados ou reiniciar devolve | system_handles | `sysctl_vm_swapusage` |
| `apple_intelligence_models` | `apple_intelligence.physical` ≥ 5 GB | Modelos do Apple Intelligence; o macOS gerencia, não apague a pasta | system_handles | `fsctl_dir_stats` |
| `trash` | linhas em `~/.Trash` | Esvaziar o Lixo | safe | top |
| `logs` | `~/Library/Logs` | Lixo pelo Finder; os apps recriam | safe | top + docs/04 Seguro |
| `homebrew_cache` | `~/Library/Caches/Homebrew` | `brew cleanup --dry-run` para ver. Sem `--dry-run`, ele também remove versões antigas das fórmulas instaladas | safe | idem |
| `npm_cache` | `~/.npm/_cacache` | `npm cache clean --force` | safe | idem |
| `go_build_cache` | `~/Library/Caches/go-build` | `go clean -cache` | safe | idem |
| `pip_cache` | `~/Library/Caches/pip` | `pip cache purge` | safe | idem |
| `updater_caches` | `~/Library/Caches/*.ShipIt`, `*-updater`, `node-gyp` | Instaladores já aplicados: Lixo pelo Finder | safe | idem |
| `xcode_derived_data` | `~/Library/Developer/Xcode/DerivedData` | Apagar no Finder; o próximo build recompila tudo | cost | docs/04 Custo |
| `simulators` | `~/Library/Developer/CoreSimulator/Devices` | `xcrun simctl delete unavailable`; o resto, em Xcode > Devices and Simulators | cost | idem |
| `simulator_runtimes` | `/Library/Developer/CoreSimulator` | Veja com `xcrun simctl runtime list`; remova em Xcode > Settings > Components | cost | idem |
| `playwright_browsers` | `~/Library/Caches/ms-playwright` | Só se não usa Playwright; Lixo pelo Finder | cost | idem |
| `ollama_models` | `~/.ollama/models` | Veja com `ollama list` e remova pelo próprio Ollama | cost | idem |
| `cinebench_assets` | `~/Library/Preferences/Maxon/Cinebench_*` | Assets do benchmark; baixam de novo ao rodar | cost | idem |
| `cloudkit_cache` | `~/Library/Caches/CloudKit` | Não apague com o iCloud sincronizando; em geral, deixe | check_first | docs/04 Condicional |
| `claude_vm_bundles` | `~/Library/Application Support/Claude/vm_bundles` | Feche o app Claude antes; é a máquina virtual dele | check_first | idem |
| `preferences_anomaly` | linha em `~/Library/Preferences/<x>` ≥ 1 GB, fora `Cinebench_*` | Anomalia: Preferences deveria ter KB; procure o app dono | anomaly | docs/04 Anomalias |
| `icloud_drive` | Σ `~/Library/Mobile Documents/*` ≥ 5 GB | Não apague: é o iCloud Drive e sumiria de todos os aparelhos. Use "Otimizar Armazenamento do Mac" | do_not_touch | docs/04 Proibido |
| `cloud_storage` | Σ `~/Library/CloudStorage/*` ≥ 5 GB | Dados reais; libere pelo app do provedor | do_not_touch | idem |
| `system_folders` | linha em `/System`, `/private/var/db` ou `/private/var/vm`, ou termo `protected_folders` ≥ 5 GB | Não mexer: é do sistema e não é lixo | do_not_touch | idem |

Ficam fora, porque a profundidade dos baldes não alcança: `~/Library/Containers/<app>/Data/Library/Caches` e `…/workspaceStorage` (§8). Os snapshots `apple_other` aparecem em Armazenamento, sem recomendação.

## 7. Marcos

As fotos são tiradas sempre com o **binário cru**. Nunca com um .app, nem com o `montar.sh` no caminho padrão.

```
export CARGO_TARGET_DIR="$TMPDIR/sdt-cargo"
cargo build --bin sdtally
swift build --package-path app --scratch-path "$TMPDIR/sdt"
F=app/Tests/SDTallyTests/Fixtures
SDTALLY_BIN="$CARGO_TARGET_DIR/debug/sdtally" SDTALLY_COMPLETO=$F/completo.json SDTALLY_DADOS=$F/dados \
SDTALLY_SECAO=<secao> [SDTALLY_ESQUEMA=escuro] SDTALLY_FOTO="$TMPDIR/<secao>.png" "$TMPDIR/sdt/debug/SDTally"
```

`SDTALLY_DADOS` só lê. O binário cru nunca grava histórico.

Há duas trilhas que não compartilham arquivo: **Rust** (R1 → R2 → R3) e **Swift** (A1, depois {A2 ∥ A3 ∥ A4}, depois A5 → A6, e A7 depois de A3 + A4). O A1 entrega as fixtures finais e o mapa de níveis, e nenhum marco seguinte mexe nelas.

| Marco | Arquivos | Compila e testa | Prova |
|---|---|---|---|
| **R1** JSON aditivo | `src/sys/mod.rs` (`swap_usage()` via `sysctlbyname("vm.swapusage")`); `src/coleta.rs` (`Rapido` e `Coleta` guardam `volumes.json`, swap e snaps); `src/sources/snapshots.rs` (`kind` ∈ {time_machine, apple_other, other} pelo prefixo; `date` só no TM); `src/report/mod.rs` (`containers_json(&Value)` pura, com `internal`/`isc`/`holds_data` e sem nome; `swap`; `informational.snapshots`; `deleted_open_processes`; `top_services` sem `take(3)`); `docs/json-schema.md`; `tests/relatorio.rs` | `cargo test`: `json_forma` (`containers` sem `name`, com `internal`); invariante do macOS da tela; `kind` nos 3 valores, sem nome | `sdtally --quick --json \| jq '.containers,.swap'` |
| **R2** Conselhos | `src/conselho.rs`, `src/lib.rs`, `src/bin/sdtally.rs` (`v["advice"]`), `docs/json-schema.md`, `tests/conselho.rs` | `cargo test`: `so_de_medido`; `dobrado_dentro_conta`; `dobrado_ancestral_nao`; `limite_usa_privado` (5 GB alocados e 0,2 GB privados não passam); `comandos_na_lista`; `sem_texto_perigoso`; `componente_inteiro`; `system_folders_sem_library` | `sdtally --json \| jq .advice` |
| **R3** IA | `src/bin/sdtally.rs` (`--last`, `--history`, `--mcp`), `src/mcp.rs`, `src/historico.rs` (guardas e ordem por `generated_at`), `README.md`, `tests/mcp.rs` | `cargo test`: `initialize` (versão suportada ecoa; desconhecida vira a mais nova; `serverInfo`); notificação sem resposta; `ping`; `-32601`; `tools/list` = 3 com `inputSchema` e a frase "Read-only"; argumento dá `-32602`; todo stdout é JSON-RPC; symlink, 21 MB e schema 2 são recusados sem vazar conteúdo; `recusaComoRoot` | `printf … \| sdtally --mcp` |
| **A1** Base | `Marca.swift` (tokens, `Marca.nivel`, `TallyShape`, `IconeApp`, `.cartao`, `.transicaoNumero`, `Faixa`, `FichaKPI`); `Modelo.swift` (campos novos opcionais, `nonisolated`); fixtures finais `quick.json`, `quick-vm-negativo.json`, `completo.json` (`containers`, `swap`, `advice` em todos os níveis, linhas `folded`), `completo-injetado.json` (`command` fora da lista) e `dados/{Medicoes/3 arquivos (um com outro tool), pontos.jsonl}`, tudo com `/Users/exemplo`; `SDTallyApp.swift` (7 seções, grupos, `IconeSecao`, selos, menus chamando stubs, sheet); esqueletos das 6 telas novas; `Historico.swift` (só `pastaLeitura`); `Relatorio.swift` (stubs); `Painel.swift` (`completoDados`); `Foto.swift` (`Dev.ativo`; 7 seções mais `boasvindas`/`barra`; `SDTALLY_ESQUEMA`, `SDTALLY_ICONE`, `SDTALLY_PDF`; janela por `identifier` com prefixo `principal`; `renderizar`); `.github/workflows/ci.yml` (foto pelo binário cru; fotos `armazenamento`/`permissoes`/`info`; grep do §1.10) | `swift test` (`ModeloTests`): decode das chaves novas e nil quando ausentes | 7 fotos claras, 1 escura |
| **A2** Informações, Permissões, segurança | `InfoView.swift`, `PermissoesView.swift` (com `ehAdmin` local; "Comportamento" como esqueleto desligado), `Executor.swift` (`binario(env:bundleID:)`, C4), `Painel.swift` (P2), `app/montar.sh` (ícone), `app/Info.plist` (`CFBundleIconFile`). Não toca `SDTallyHelper` nem `SDTallyXPC` | `InfoPermissoesTests`: `ehAdmin(99999) == false`, `binarioIgnoraEnvNoBundle`, caminho sem `/` recusado, Acesso Total divergente mostra faixa | fotos `info` e `permissoes`; iconset em `$TMPDIR` |
| **A3** Armazenamento e Resumo | `ArmazenamentoView.swift` (com "Partes do sistema"), `ResumoView.swift`, `Formato.swift` (`cor`, `nomePapel(role)`, `nomeFonte`), `BarraView.swift`, `RankingView.swift` | `ArmazenamentoTests`: Σ dos volumes = `macos.bytes`; VM − swap negativo vira nil; só `internal`; agrupamento das partes; próximos passos sem `system_handles` | fotos `resumo` e `armazenamento`, claro e escuro |
| **A4** O que fazer e boas-vindas | `ConselhosView.swift` (textos, `comandosPermitidos`, R3, R6, R7), `BoasVindasView.swift` | `ConselhosTests`: `id` desconhecido mostra `text`; `comandoInjetadoSemBotao`; `apagadosAbertosSoNomeiaApp`; `apagadosAbertosNomeCortado` | fotos `fazer` e `boasvindas` |
| **A5** Histórico | `Historico.swift`, `HistoricoView.swift`, `Painel.swift` (guardar, restaurar, anotar ponto), `ResumoView.swift` ("desde a última leitura"), `SDTallyApp.swift` (data no `Rodape`), `Tests/HistoricoTests.swift` | `HistoricoTests` (pasta temporária, sem `@MainActor`): 0600/0700; retenção; `pastaGravacao` nil com `Dev.ativo`; `SDTALLY_DADOS` só lê; corrompido pulado; ordem por `generated_at`; `naoComparaModos`; `naoComparaVersoes`; `ausenteNaoEZero` | foto `historico` |
| **A6** Barra de menus e avisos | `BarraMenus.swift`, `SDTallyApp.swift` (cena, `Delegado`, `.defaultLaunchBehavior`), `Painel.swift` (instância única, `iniciar`, cadência, notificação), `PermissoesView.swift` ("Comportamento", dono dos toggles) | `BarraMenusTests`: `vigiarUmaVez`; `avisoComHisterese`; `semPontoNaJanelaNaoAvisa`; `poucoEspacoContaPurgeable`; `crescimentoDescontaPurgeable`; `sistemaDoUltimoPonto` | foto `barra` |
| **A7** Exportar | `Relatorio.swift` | `RelatorioTests`: `pdfSemAutor` (`pageCount ≥ 4`); JSON exportado igual aos bytes da fixture | `SDTALLY_PDF=$TMPDIR/r.pdf`, página 1 via `sips` |

Paralelismo:
- R1, R2 e R3 são sequenciais.
- A2, A3 e A4 rodam juntos depois de A1, com arquivos de teste separados.
- A5 vem depois de A2 e A3. A6 vem depois de A5.
- A7 roda em paralelo com A5 e A6, depois de A3 e A4. A página de histórico do PDF entra quando A5 estiver pronto.
- A seção de IA de Informações só é conferida de ponta a ponta depois de R3.

A conferir no Mac durante os marcos:
- se `Window(id: "principal")` gera o `NSWindow.identifier` com esse prefixo (A1);
- se `log show` sem admin falha (A2);
- se o daemon do helper tem entrada própria em Acesso Total ao Disco e se o `full_disk_access` da medição via helper reflete isso (A2);
- se `.defaultLaunchBehavior` com valor condicional suprime a janela no início de sessão (A6);
- quais `protocolVersion` o Claude Code e o Claude Desktop mandam, para preencher `SUPORTADAS` (R3).

## 8. Fora do escopo, e quando entraria

| Item | Quando entraria |
|---|---|
| Apagar ou limpar pelo app (Fase 3) | Só se o dono reabrir a decisão |
| Exportação e MCP anônimos (`--anonymize-stdin` com `anon::Ctx::new`) | Quando o dono quiser publicar ou não quiser nomes indo para o provedor da IA |
| Medição root pela IA | Só com pedido explícito e revisão do requisito do helper |
| Mudanças no helper ou no `SDTallyXPC` | Numa fase que já exija nova aprovação do helper |
| Abrir medição antiga, ⌘O, "Ignorar" item, `sdtally diff` | Quando o histórico mostrar que faz falta |
| TipKit, Markdown, "Copiar diagnóstico" | A pedido |
| "O que fazer" e `containers` no texto da CLI | Quando alguém usar a CLI em texto e pedir |
| Conselhos para `Containers/*/Caches` e `workspaceStorage` | Com regra sobre o detalhe de pasta |
| Categorias sem o Ajustes, metadados do APFS, dentro de Preboot e Recovery, Fase 2, Frente C | Fases de pesquisa do PLANO |
| RAM física | Só se o dono pedir |
| `.accessory`, inglês, notarização | Fase 5 do PLANO, ou a pedido |

Fase de desenho: nenhum arquivo foi criado, alterado ou apagado, e nenhum build rodou.

## Mudanças após a crítica

**Aceitas:**
1. O sleepimage saiu do VM. O VM ficou dividido em swap e "resto do VM (diferença)", com "—" quando der negativo; há fixture e teste do caso negativo (itens 1 e 30). Fui além da crítica e tirei `sleepimage_bytes`: o `/private/var/vm` do `top` já mede o sleepimage no Data, então não há medição nova.
2. "swap" saiu da dica 1 das boas-vindas (2).
3. As linhas `folded` contam quando estão dentro do prefixo da regra, nunca quando são ancestrais, com testes (3).
4. O teste de comandos virou duas checagens: a lista fixa, mais um conjunto de substrings proibidas (4).
5. A frase "Read-only… never run…" entrou nas ferramentas MCP e no README, e "Ask the user before running." no `text` com comando (5).
6. `comandosPermitidos` no Swift, com a fixture `completo-injetado.json` (6).
7. `system_folders` não pega mais `/Library`, e entrou `simulator_runtimes` com `xcrun simctl runtime list` (7).
8. `brew cleanup --dry-run`, com o aviso sobre o Cellar (8).
9. Limite e destaque por `private_bytes`, com "≤" quando for null (9).
10. A R6 aceita nome cortado e exige um único app que case (10).
11. Aviso só com ponto entre 20 e 28 h, hora real na notificação, pouco espaço com (livre + purgeable) e crescimento pelo container (11).
12. Pastas como funções puras. Gravação só com bundle e fora do `Dev.ativo`, e os testes usam URL temporária (12).
13. `SDTALLY_DADOS` só lê (13).
14. `Dev.ativo` liga todas as `SDTALLY_*`, e a foto do CI usa o binário cru (14).
15. `thermalState` com `contains` (15).
16. Tipos e funções puras `nonisolated` (16).
17. `iniciar()` no `Delegado` e `.defaultLaunchBehavior` (17).
18. Dois estados de Acesso Total ao Disco e a divergência; entrou na lista "a conferir" (18).
19. "pede permissão sem você clicar", e o aviso do TCC na linha do Acesso Total (19).
20. O bloco "Partes do sistema no volume Data", tirado do `top` (20).
21. A regra `trash` e o aviso de esvaziar o Lixo (21).
22. Próximos passos sem "Nada a fazer", e R7 no Resumo (22).
23. Dados do Sistema na barra de menus vem do último `Ponto` quando falta `telaBoa` (23).
24. Data no rodapé e idade na ficha (24).
25. Arquivos de teste por marco, fixtures finais no A1, `Marca.nivel` e o gancho `SDTALLY_PDF` no A1, e o A6 como dono de "Comportamento" (25).
26. O helper não muda (26).
27. O MCP completo: notificações, `ping`, códigos de erro, `serverInfo`, `inputSchema` e lista `SUPORTADAS` (27).
28. `--last`, `--history` e `--mcp` recusam rodar como root (28).
29. O aviso de exportação e o texto para IA citam o nome de usuário (29).
31. `--scratch-path` e `CARGO_TARGET_DIR` no critério (31).
32. Grep do CI ampliado (32).
33. `kind` com três valores (33).
34. Campo `internal` (34).
35. `tool` igual na comparação e ordem por `generated_at`, também no Rust (35).
36. "Voltar" só no ranking (36).
37. (a) A linha "build" saiu de Informações. (b) "Registro do Ajustes" saiu de Permissões e o estado foi para a linha 6 de Armazenamento.

**Recusadas ou ajustadas:**
- (4) A checagem de metacaracteres de shell (`>`, `|`, `;`…) vale só para `command`, não para `text`. Motivo: textos legítimos como "Xcode > Settings > Components" reprovariam.
- (5) Recusei a alternativa de tirar o `command` da saída do MCP. Motivo: `sdtally --json` já o expõe a quem tem shell, e a frase na descrição da ferramenta cobre o risco. O "Ask the user before running." vai só nos itens com comando; nos de nível `system_handles` ("nada a fazer") não faz sentido.
- (26) Recusei a variante de mover `ehAdmin` para o `SDTallyXPC`. Motivo: o helper liga esse target, então o executável dele mudaria do mesmo jeito. A cópia fica no target do app.
- (37c) `--history` fica, como a própria crítica admite.
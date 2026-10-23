# JSON do `sdtally --json` (schema_version 1)

Só stdout; progresso e erros vão para stderr. Chaves em inglês. Bytes são inteiros (B) ou `null`,
nunca 0 no lugar de "não medido". Código: `src/report/mod.rs` (`json`); teste da forma em
`tests/relatorio.rs` (`json_forma`). Mudança incompatível sobe `schema_version`.

## Medida

Todo número vem neste formato:

```json
{"bytes": 123 | null, "state": "measured|partial|not_measured|not_available", "reason": "..." | null, "source": "<fonte>"}
```

- `partial`: `bytes` é um piso (só o que foi medido); o que faltou está em `unexplained`.
- `not_measured`: `bytes` é `null`; `reason` diz por quê. `not_available`: a fonte não existe nesta
  versão do macOS (ex.: Apple Intelligence no 15.0).
- `source`: `getattrlistbulk` (travessia), `listxattr`, `fsctl_snapshot_private_size`,
  `lsof_proc_pidfdinfo`, `fsctl_dir_stats`, `attr_vol_spaceused`, `difference`,
  `diskutil_capacity_in_use`, `dir_stats_like_settings`, `storage_management_log`, `cachedelete`; e, em
  `system_data.reproduced.categories[]`, as da Fase 2 (abaixo).

## Raiz

| Chave | Tipo | O quê |
|---|---|---|
| `schema_version` | int | 1 |
| `home` | string | home da pessoa medida (com sudo, a do `SUDO_USER`): resolve o `~` dos caminhos |
| `tool`, `generated_at`, `macos_version` | string | versão do sdtally, hora local `AAAA-MM-DD HH:MM`, versão do macOS |
| `generated_at_epoch` | int | o mesmo instante em segundos desde a época (UTC); a hora local não tem fuso. Só na medição completa (`sdtally --json`); arquivo salvo antes dela não tem a chave |
| `complete` | bool | `false` se houver qualquer motivo abaixo |
| `incomplete_reasons` | [string] | `interrupted`, `no_root`, `no_full_disk_access`, `stage_failed` (alguma etapa da coleta registrou erro; ver `stage_errors`) |
| `root`, `full_disk_access` | bool (FDA pode ser `null`) | modo em que rodou |
| `denied_folders` | int | pastas que a travessia não abriu (EPERM/EACCES) |
| `containers` | [objeto] ou `null` | containers APFS, abaixo; `null` se o `diskutil` falhou |
| `swap` | objeto ou `null` | `{"total", "used", "source": "sysctl_vm_swapusage"}`: arquivos de swap no volume VM, sem root; `null` se o sysctl falhou |
| `settings_screen` | objeto | `macos` (Medida), `apple_intelligence` (Medida + `physical`: Medida), `system_data` (Medida + `logged_at`, hora do log; é o número do `StorageManagementService`, **não o da tela**: a tela tira dele a diferença entre o macOS dela e o `SYS` do serviço, ver research/mapa-de-fontes.md §6; `not_measured` se o log não tem registro), `screen` (a tela pelo próprio log, abaixo) |
| `data_volume.used` | Medida | usado do volume Data |
| `data_volume.terms` | [Medida + `key`] | somáveis, nesta ordem: `tree`, `large_xattrs`, `snapshots`, `deleted_open`, `protected_folders`, `apfs_metadata` (nós das árvores vivas do APFS × bloco, piso; `source: apfs_user_client`, ver "Lixo por idade e metadados do APFS"; `not_measured` se o seletor falhou ou a etapa não rodou) |
| `data_volume.unexplained` | objeto | `bytes` = used − Σ terms medidos, `percent_of_used`, `state` (`partial` se algum termo não é `measured`), `source: difference`. Nunca negativo: se Σ terms passa de `used` (dir-stats de pasta protegida conta clone em dobro e bytes que a travessia já contou), `bytes` e `percent_of_used` são `null`, `state: not_measured`, `reason` diz por quê e `overlap_bytes` = Σ terms − used |
| `top` | [objeto] | ranking inteiro, do maior para o menor (o texto mostra só `--top N`) |
| `informational` | objeto | não somar: `purgeable` (Medida + `top_services` [{name, bytes}], todos os serviços acima de 0, do maior para o menor), `snapshots` e `deleted_open_processes` (abaixo), `repeated_full_clones`, `repeated_hardlinks` e `repeated_shared_blocks` ({files, bytes}; o último: arquivos com blocos divididos com outro, por clone parcial, e os bytes que a árvore descontou para contar cada bloco uma vez, pela união das faixas físicas `F_LOG2PHYS_EXT`; o arquivo de FILEID menor fica com os blocos), `graft_roots_skipped` (int), `scan_state` (`measured` ou `partial`, da travessia que contou os repetidos), `dataless` ({files, apparent_bytes, folders_not_opened}); `null` sem travessia; `trash` e `apfs_metadata` ("Lixo por idade e metadados do APFS", abaixo; `null` se a etapa não rodou; o valor de `apfs_metadata` já está somado em `data_volume.terms`), `protected_without_size` (pastas negadas sem dir-stats, abaixo; `null` sem a etapa) |
| `system_data` | objeto | `by_line` (Σ do `top` por linha do Ajustes) e `reproduced` (as 15 categorias reproduzidas), ver "Fase 2: categorias do Ajustes"; só no completo |
| `stage_errors` | [objeto] | `{"stage", "error"}` de cada erro registrado numa etapa da coleta (erro em inglês) |
| `stage_seconds` | objeto | segundos que cada etapa da coleta levou, pelo nome interno: `meta`, `volumes`, `swap`, `volume_data`, `snapshots`, `purgeable`, `apagados_abertos`, `lixo`, `apfs_metadata`, `arvore`, `dirstats_negadas`, `painel`, `tela_log`, e `categories` (a leitura dos fatos da Fase 2, depois da coleta) (etapa que não rodou, por exemplo após cancelar, não aparece). Medida do próprio sdtally, sem fonte do sistema |
| `advice` | [objeto] | "O que fazer" (abaixo); só no `--json` completo |
| `not_freed` | [objeto] | "Por que o espaço pode não voltar" (abaixo); só no `--json` completo; `[]` sem causa medida |

## Linha de `top`

`{"path": "~/Music" | null, "folded": bool, "term": "<key do termo>", "bytes": int, "private_bytes": int | null,
"lower_bound": bool, "count": int | null, "top_process": {"name", "bytes"} | null, "source": "<fonte>"}`

- `path` é `null` nas linhas de termo único (snapshots, apagados-abertos, xattrs, metadados do APFS); home como `~`.
- `folded`: baldes pequenos dobrados no pai; `path` é a pasta pai ("demais itens de ~/Library").
- `private_bytes`: "libera se apagar"; só a travessia mede (`null` em dir-stats e nos termos únicos).
- `count`: quantidade de snapshots. `top_process`: processo que mais segura apagados-abertos.
- Σ `top[].bytes` + `unexplained.bytes` = `used.bytes`, exceto com `unexplained.overlap_bytes`: aí Σ `top[].bytes` = `used.bytes` + `overlap_bytes`.

## `containers`

Um por container APFS, na ordem do `diskutil apfs list`. Sem nome de volume nem identificador de
disco (o nome pode ser pessoal).

| Chave | O quê |
|---|---|
| `internal` | `true` no disco interno, `false` em externo, `null` se o `diskutil info` não disse |
| `isc` | container ISC (xART, Hardware, iSCPreboot): o Ajustes não mostra |
| `holds_data` | guarda o volume Data de boot |
| `ceiling`, `free` | `CapacityCeiling` e `CapacityFree` do container |
| `volumes` | [{`roles` ([string], ex.: `System`, `Preboot`, `Recovery`, `VM`, `Update`, `Data`; vazio sem papel), `capacity_in_use` (int ou `null`; fonte `diskutil_capacity_in_use`), `in_settings_macos` (bool), `device` (string, ex.: `disk3s5`; o app acha o volume montado por ele)}] |

Os volumes que não são o Data (System, Preboot, Recovery, Update, VM) são outros volumes do mesmo
container: dividem o espaço livre com o Data, mas o Ajustes os soma na linha **macOS**, nunca em Dados
do Sistema (`in_settings_macos: true`). O Data de boot e o container ISC têm `false`. Só o total de
cada volume é medido; o que há dentro de Preboot e Recovery fica como "não medido" (o `diskutil` só dá
o total, e o sdtally só percorre o volume Data).

Invariante: Σ `capacity_in_use` dos volumes com `in_settings_macos` = `settings_screen.macos.bytes`
(é a mesma marca que soma o macOS da tela). O resto do VM além do swap é
`capacity_in_use(VM) − swap.total`; lidos em momentos diferentes, pode dar negativo.

## `informational.snapshots` e `informational.deleted_open_processes`

- `snapshots`: [{`kind`, `date`, `private_bytes`}] do volume Data de boot, do mais antigo ao mais
  novo; `null` se a etapa não rodou. `kind` pelo prefixo do nome: `time_machine`
  (`com.apple.TimeMachine.`), `apple_other` (outro `com.apple.`, ex.: atualização) ou `other`. `date`
  só no `time_machine`: `AAAA-MM-DD HH:MM:SS` do nome, hora local; `null` nos outros. O nome não sai.
  `private_bytes`: blocos só desse snapshot (`null` se o fsctl falhou).
- `deleted_open_processes`: os 10 processos que mais seguram apagados-abertos, do maior para o
  menor; `null` sem lsof. [{`name`, `bytes` (alocado medido; `null` se nenhum arquivo dele teve
  alocado legível), `files`, `files_without_allocated`, `logical_bytes_without_allocated` (SIZE do
  lsof desses, lógico, fora da soma)}]. Sem sudo, só processos do próprio usuário.

## `advice` ("O que fazer")

Gerado só pelas regras de `src/conselho.rs` (spec da fase 5, §6), a partir do que foi medido.
Nenhuma regra apaga nada; o app traduz pelo `id`.

`{"id", "level", "bytes", "private_bytes" | null, "paths": [string], "term" | null, "process" | null, "command" | null, "text"}`

- `level`: `measurement`, `system_handles`, `safe`, `cost`, `check_first`, `anomaly`, `do_not_touch`.
- Regras de caminho: casam as linhas de `top` (com `folded` também) cujo `path` é igual ao prefixo
  da regra ou está dentro dele, por componente inteiro; linha ancestral nunca casa. Cada linha entra
  só na primeira regra que casa. Só termos `measured` ou `partial`.
- `bytes` e `private_bytes`: Σ das linhas da regra. `private_bytes` é `null` se alguma linha não
  tem privado (dir-stats); aí o limite usa `bytes` (o app mostra "≤"). Limite: Σ ≥ 1 GB, ou 5 GB em
  `do_not_touch`; fora da árvore, o da própria regra.
- `paths`: as pastas das linhas (sem repetir); vazio nas regras fora da árvore.
- `term`: o termo das linhas quando é um só (`tree`, `protected_folders`, `snapshots`,
  `deleted_open`); `null` se mistura termos ou não vem de termo. `process`: em `deleted_open`, o
  `top_process.name`.
- `bytes` em `grant_full_disk_access` e `measure_as_admin`: o `unexplained.bytes`.
- `command`: só um destes, nunca com caminho medido: `brew cleanup --dry-run`,
  `npm cache clean --force`, `go clean -cache`, `pip cache purge`, `xcrun simctl delete unavailable`,
  `xcrun simctl runtime list`, `ollama list`, `tmutil listlocalsnapshots /`. Com `command`, o `text`
  termina em "Ask the user before running."
- `text`: em inglês.
- `dry_run`: `{"command", "doc"}` ou `null` (sem `command`). `command` é o que roda antes só para ver
  (simulação ou listagem; igual ao `command` do item quando ele já só lê) e `doc`, onde isso está
  documentado. Vem de `conselho::SIMULACOES`, uma linha por comando da lista fixa; o teste
  `tests/dryrun.rs` reprova comando sem linha, sem fonte ou cuja leitura mude o disco.

## `largest_files` (só na medição completa)

Os maiores arquivos que a travessia leu: de 100 MB ou mais, no máximo 30, do maior para o menor
(empate: caminho). `[{"path", "bytes" (alocado), "private_bytes" (o que apagar libera: 0 num clone
que divide tudo)}]`. Caminho sem o `/System/Volumes/Data` da frente. Pasta protegida (sem root) não
entra. No `--auto` sai `null`: o modo vivo guarda pasta, não arquivo. Com redação (`--redact names`,
MCP), `path` vira o balde da pasta do arquivo, como `top[].path`: o nome do arquivo não sai.

## `not_freed` ("Por que o espaço pode não voltar")

Gerado por `src/voltou.rs` só a partir da `Conta` medida, no `--json` completo (o texto da CLI tem
o mesmo bloco, depois de "Informativo"). Só existe a causa com número medido de 1 GB ou mais; o que
não foi medido some, nunca vira zero. Nada aqui manda apagar: explica por que apagar algo pode
devolver menos do que o esperado. Do maior para o menor (empate: `id`).

`{"id", "title": {pt, en}, "text": {pt, en}, "bytes", "bytes_floor", "count" | null, "source"}`

| `id` | número | `count` | `source` |
|---|---|---|---|
| `local_snapshot` | termo `snapshots` (snapshots locais ainda seguram blocos apagados) | snapshots | `fsctl_snapshot_private_size` |
| `deleted_open` | termo `deleted_open` (apagado que um app mantém aberto) | nulo | `lsof_proc_pidfdinfo` |
| `shared_clone` | `repeated_full_clones.bytes` (cópias que dividem blocos) | arquivos | `getattrlistbulk` |
| `cloud_only` | `dataless.apparent_bytes` (só na nuvem: tamanho aparente, não ocupa disco) | arquivos | `getattrlistbulk` |
| `purgeable` | `informational.purgeable.bytes` (Disponível = livre + purgeable) | nulo | `cachedelete` |
| `trash` | Σ das linhas de `top` em `~/.Trash` (ocupa até esvaziar) | nulo | `getattrlistbulk` |

`bytes_floor: true` quando o termo saiu `partial` ou a travessia foi interrompida. `title` e `text`
são texto traduzido (abaixo).

**Texto traduzido** (`title`/`text` daqui e os campos `{pt, en, …}` das fichas): objeto do código
BCP 47 para o texto. `pt` é pt-BR e `en` vem sempre; as outras línguas (`ar ca cs da de el es
es-419 fi fr he hi hr hu id it ja ko ms nb nl pl pt-PT ro ru sk sl sv th tr uk vi zh-Hans
zh-Hant`, as localizações do macOS) vêm de `src/traducoes/<código>.json`. Língua sem tradução não
aparece (nunca texto vazio). O JSON não depende de `--lang`. Escolha no app: código exato, depois
cortar subtags da direita (`zh-Hant-HK`, `zh-Hant`, `zh`), depois `en`; `es-US` vai para
`es-419`, `no`/`nn` para `nb`.

## `settings_screen.screen` (a tela do Ajustes pelo próprio log)

Com o painel Ajustes > Geral > Armazenamento aberto, o Ajustes grava a cada ~5 s
`Sum of categories: X, available: Y, capacity: Z` e o `StorageManagementService` grava um bloco
`StorageLogInvestigation - <chave>: <bytes>`. Lido da última hora (`log show --last 1h`, ~1,2 s e 180 MB) ou, com `--quick --since` (abaixo), só do incremento.

| Chave | O quê |
|---|---|
| `state` | `measured`; `partial` (linha da tela sem bloco do serviço ou sem macOS); `not_measured` (painel não aberto na janela lida; `reason` diz) |
| `logged_at` | hora da linha da tela |
| `used` | X: o "usado" do cabeçalho da tela, ao byte |
| `available`, `capacity` | Y e Z da mesma linha |
| `system_data` | Dados do Sistema como a tela: X − Σ categorias das extensões (Used − System − Other do serviço) − macOS da tela. Bate com a tela dentro do ciclo de 5 s (02/10: 145,99 e 147,71 GB na tela, 145,99 e 147,70 calculados) |
| `categories_total` | Σ `categories` (Used − System − Other do serviço); `used − categories_total − macos = system_data` |
| `categories` | [{`id`, `bytes`}] das extensões (ex.: `com.apple.STMExtension.Applications`), do maior para o menor |
| `service` | o bloco do serviço usado: `logged_at`, `total`, `available`, `purgeable`, `other_volumes`, `used`, `system`, `other` |
| `source` | `settings_ui_log` |

macOS da tela (`settings_screen.macos`): Σ CapacityInUse dos containers do disco interno, menos o
Data e menos o container ISC (xART, Hardware, iSCPreboot), que o Ajustes não mostra.

## `sdtally --quick --json` (kind `quick`)

Só o que muda a toda hora, sem travessia (~1,5 s; ~0,2 s e ~0,12 s de CPU com `--since` e o painel fechado): `schema_version`, `kind: "quick"`, `tool`,
`generated_at` (com segundos), `macos_version`, `home`, `root`, `full_disk_access`, `interrupted`,
`settings_screen` (`macos`, `screen`), `data_volume.used`, `containers` e `swap` (como no completo),
`capacity` ({`total`, `free`, `source: statfs`}), `purgeable` (Medida + `top_services`, todos acima
de 0), `stage_errors` e `stage_seconds` (como no completo).

`--parts <lista>` (só com `--quick`; 19/10): lê só as partes pedidas, separadas por vírgula, cada
uma com o seu ritmo no app. As chaves das outras **somem** do JSON (o app junta com a leitura anterior);
as da raiz que não custam nada (`schema_version`, `kind`, `tool`, `generated_at`, `macos_version`,
`boot_time`, `home`, `root`, `interrupted`, `stage_errors`, `stage_seconds`) saem sempre.

| Parte | Chaves | Custo |
|---|---|---|
| `meta` | `full_disk_access` | ~0 |
| `volumes` | `containers`, `settings_screen.macos` | ~0,19 s |
| `swap` | `swap` | ~0 |
| `uso` | `data_volume`, `capacity` | ~0 |
| `purgeable` | `purgeable` | ~0,02 s |
| `tela` | `settings_screen.screen` | ~0 com o painel fechado; aberto, lê os volumes também (a tela precisa do macOS) |

Nome desconhecido ou lista vazia: sai com erro. Sem `--parts`, todas.

`--since <segundos desde a época>` (só com `--quick`; o app passa o início da leitura anterior que deu
certo, se tiver menos de 120 s): leitura incremental da tela do Ajustes.
- Painel Ajustes > Geral > Armazenamento **aberto agora** (processo `Storage.appex` ou
  `StorageManagementService` vivo, por `proc_listallpids`): lê o log de `--since` − 10 s até agora
  (`log show --last <n>s`; a folga junta o bloco do serviço, que sai ~2 s antes da linha da tela, com
  a linha). `--since` no futuro ou a mais de ~1 h atrás: a hora inteira. Custa ~0,3 s de CPU e ~110 MB.
- Painel **fechado**: não lê o log (fechado, nada novo é gravado). `screen.state` é `not_measured`,
  `reason` é `Settings Storage pane closed; screen log skipped (--since)` (não contém "not read", que é
  log ilegível) e `stage_seconds` não tem `tela_log`. O app mantém a última tela boa.
- Sem `--since`: a hora inteira, sempre (primeira leitura, volta de segundo plano, `--mcp`).

Limite conhecido: painel aberto e fechado dentro de um único intervalo entre duas leituras (30 s) não
é visto; a hora inteira do log só é relida na primeira leitura e depois de >120 s sem leitura.

`containers[].internal` e o tamanho do disco vêm do DiskArbitration (o mesmo `Internal`/`TotalSize` do
`diskutil info`, conferido por teste); o resto de `containers` segue do `diskutil apfs list`.

## `sdtally --auto` (kind `auto`)

Processo vivo, uma linha JSON compacta por mudança; acaba quando o stdin fecha. Faz uma
passada inteira (a mesma leitura de pasta da medição completa, sem xattr) e depois relê só as pastas
que o FSEvents avisar. Guarda o total de cada pasta, nunca arquivo por arquivo. Aviso perdido (o
macOS diz) refaz a passada.

Cada linha é o relatório completo (as mesmas chaves do `sdtally --json`, com a árvore viva no lugar
da travessia) com `kind` `"auto"` e mais o bloco `auto` ({`last_pass_seconds`, `passes`, `events`,
`rereads`, `folders_in_memory`, `other_sources_age_seconds`}). Diferenças para a medição completa:
`root` true só quando roda como root (pelo helper do app, opção "automático com root": abre
as pastas protegidas e lista os apagados abertos de todos os processos); xattr não medido (o termo sai não medido e o relatório, incompleto); as outras
fontes (volumes, snapshots, purgeable, apagados abertos, lixo, logs do painel, fatos das categorias)
são lidas antes da passada e relidas a cada 10 min (`other_sources_age_seconds`), e a árvore entra
na hora de cada linha; `stage_seconds.categories` não sai.

Uma linha sai logo depois da passada e, depois, a cada mudança de 10 MB ou mais no alocado da árvore,
a cada minuto se mudou menos, e a cada releitura das outras fontes.

`growing_now` ("Crescendo agora"): lista de janelas, hoje 5 min e 1 h, cada uma
{`window_seconds`, `covered_seconds` (quanto da janela o processo já viu: começa em 0 ao abrir),
`net_bytes` (soma de tudo que mudou na janela, pode ser negativa), `top` [{`path`, `bytes`}]}. Vem
do que cada releitura mudou no alocado simples (sem repetidos nem blocos divididos; hardlink e
clone não entram). A soma de uma pasta inclui as de baixo, e o `top` mostra a pasta mais funda com
1 MB ou mais (a mãe só aparece quando o crescimento está espalhado em filhas abaixo do mínimo), no
máximo 10, da que mais cresceu para a que menos cresceu. Só crescimento entra no `top`. Uma passada
inteira (aviso perdido) não conta como crescimento. Com redação, cada `path` vira o balde, como
`top[].path`.

Custo medido (Mac M4, 458 mil pastas, 19–20/10/2026, com o dono usando o Mac): passada de ~40 s e
~53 s de CPU, primeira linha ~60 s depois de abrir; depois, 145–180 MB, pico de 185 MB; cada linha
custa 45–300 ms e tem ~190 KB; releitura das outras fontes ~16–20 s de relógio e ~4 s de CPU a cada
10 min; mudança vista em ~1 s. Pasta que custa 5 ms ou mais para reler (cache com dezenas de
milhares de arquivos) entra num orçamento de 5 ms de releitura por segundo e espera 20× o que
custou: o número dela pode atrasar. Repetidos e blocos divididos são refeitos no máximo a cada 5 min.
Como root (sonda da passada, 20/10/2026): 471 mil pastas, 2,32 M arquivos, +11,5 GB de árvore, 86
pastas negadas (252 sem root), passada de ~41 s; RAM igual (169 MB depois da passada, pico 188 MB).

## `sdtally <caminho> --json` (kind `branch`)

`root` (absoluto), `display` (com `~`), `depth`, `home`, `complete`, `interrupted`, `children`
[{`name` (relativo; vazio = arquivos soltos na raiz), `path` (absoluto), `loose_files`, `bytes`
(alocado único), `private_bytes` (libera se apagar; `null` em pasta protegida), `protected`,
`source`}], `total` ({`bytes`, `lower_bound`, `private_bytes`, `files`, `folders`,
`dataless_files`, `dataless_apparent_bytes`}), `protected_without_size`, `read_errors`,
`other_volume_entries`.

`total.lower_bound` é `true` e `complete` é `false` quando alguma pasta ou entrada não foi lida
por motivo que não seja EPERM/EACCES (essas vão para dir-stats) nem ENOENT/ENOTDIR (sumiram durante
a medição: contam em `read_errors`, mas não deixam nada sem medir). No relatório completo, o termo
`tree` sai `partial` pelo mesmo critério (`reason` "<n> pastas/entradas não lidas"), e dir-stats
0 sai como não medido ("dir-stats 0 (não mantido)").

## `sdtally why <caminho> --json` (kind `item`)

`path`, `display`, `type` (`file`, `folder`, `symlink`, `other`), `attributes` ({`logical_bytes`,
`allocated_bytes`, `private_bytes`, `file_id`, `clone_id`, `ext_flags`, `ext_flag_names`, `flags`,
`flag_names`, `device`, `links`} ou `null` com `attributes_error`), `lstat` ({`size`,
`blocks_bytes`, `links`, `device`, `flags`, `flag_names`}), `xattrs` ({`count`, `bytes`, `error`}),
`graft_root` e `dir_stats_bytes` (só pasta; `null` + `*_error` quando não se aplica ou falhou).

## `--last` e `--history`

`sdtally --last --json` imprime, re-serializada, a medição completa mais nova (por
`generated_at_epoch`; arquivo sem ela, pelo mtime; empate, por `generated_at`) de
`~/Library/Application Support/SDTally/Medicoes/`. Arquivo symlink, com mais de 20 MB, com
`schema_version` ≠ 1 ou com `kind` é pulado com aviso em stderr, sem o conteúdo.

`sdtally --history --json`: `{"schema_version": 1, "kind": "history", "measurements": [...]}`, da
mais antiga para a mais nova, cada uma com `generated_at`, `tool`, `root`, `full_disk_access`,
`complete`, `used` (`data_volume.used.bytes`), `unexplained` e `percent_of_used`
(`data_volume.unexplained`).

## `--changes` (kind `changes`)

`sdtally --changes --json` (e a ferramenta MCP `what_changed`): o que mudou entre as duas medições
completas mais novas da mesma pasta, na mesma ordem do `--last`. As mesmas regras do "O que mudou"
do app (`Historico.mudancas`). Menos de duas medições válidas: erro em stderr, saída 2, como o
`--last` sem medição. Recusa root.

| Chave | O quê |
|---|---|
| `from`, `to` | `{generated_at, generated_at_epoch, file}` da penúltima e da última; `generated_at_epoch` é `null` em arquivo antigo; `file` é só o nome do arquivo |
| `span_seconds` | `to` − `from` pela chave de ordem (época; sem ela, mtime) |
| `comparable`, `reason` | `false` com `reason`: `interrupted` (uma delas tem `interrupted` em `incomplete_reasons`), `different_modes` (`root`, `full_disk_access` ou `complete` diferentes), `different_versions` (`tool` diferente); senão `true` e `null`. Não comparáveis: `terms` e `folders` vazios |
| `used` | `{from, to, delta}` de `data_volume.used.bytes`; lado ausente = `null`, e o `delta` também |
| `system_data` | `{from, to, delta}` de `settings_screen.screen.system_data`; só quando as duas têm |
| `terms` | `[{key, from, to, delta}]` de `data_volume.terms`, só termos com `bytes` nas duas |
| `folders` | `[{path, term, folded, from, to, delta}]` das linhas do `top`, pela chave `path`+`term`+`folded` (a primeira de cada chave vale); linha só de um lado tem o outro `null`, nunca `0`, e `delta` = o valor dela (negativo se saiu). Até 30 |

`terms` e `folders`: só |`delta`| ≥ 100 MB (10^8), do maior para o menor. Com redação
(`--redact names`, ou o padrão do MCP), `folders[].path` passa pelo mesmo balde de `top[].path`,
linha por linha, sem juntar linhas, e `file` pelo mesmo filtro de nomes.

## Fase 6: catálogo

Tudo aditivo; `schema_version` continua 1. As fichas vêm de `src/catalogo.rs` (`CATALOGO`), em
dados compilados, sem versão própria (a versão é a do `tool`).

**`card`** (id da ficha ou `null`) entra em:

| Onde | Casa por |
|---|---|
| `top[]` | `path`, por componente inteiro como as regras (linha `folded` igual); sem `path`, pelo `term` |
| `informational.purgeable.top_services[]` (completo) e `purgeable.top_services[]` (rápido) | `name` igual |
| `containers[].volumes[]` | algum dos `roles` |
| `informational.deleted_open_processes[]` | `name` igual, ou prefixo quando o `name` tem 15+ caracteres |
| `informational.snapshots[]` | `kind`: `time_machine` → `time_machine_snapshots`, `other` → `other_snapshots`, `apple_other` → `null` |
| `data_volume.terms[]` | `key` |
| raiz do `why --json` | `display` |
| `advice[]` | sempre a ficha do próprio `id` |

A ordem do catálogo é a prioridade: a primeira ficha que casa vence (`cinebench_assets` antes de
`preferences_anomaly`; `asset_*` e `apple_intelligence_models` antes de `system_folders`).

**`advice[]`** ganha `kind` (`rule` ou `signature`, ver "Fase 6: regras e detectores") e `card`. O `text` das
regras de `REGRAS` sai do `step.en` da ficha (mais " Ask the user before running." quando há
`command`).

**`cards`** (raiz, no completo, no rápido e no `why`): `{id: Ficha}` só com as fichas citadas em
algum `card`. `--last` e `--history` saem crus, sem `card` nem `cards`. Arquivo antigo sem uma das
listas: a lista é ignorada.

**`top[].settings_line`** e **`top[].settings_line_grade`** (também do `anexar`, em toda linha de
`top`; `--last` sai sem eles): a linha do Ajustes > Armazenamento em que a linha cai, pelas regras
da Frente P (`src/linhas.rs`, spec-inicio §9.1). Vale a primeira regra que casa; o `path` casa por
componente inteiro, e a linha `folded` pelo `path` do pai. Com nomes reais, a travessia dá balde
próprio às bibliotecas de mídia (`~/Music/<a>/<b>`, `~/Movies/<a>/<b>`, `~/Pictures/*.photoslibrary`):
Música, TV e Fotos pelo balde da biblioteca (B), o resto da biblioteca em Dados do Sistema (Documentos
a exclui) e o resto de `~/Music`, `~/Movies` e `~/Pictures` em Documentos.

- `settings_line`: id da categoria da Apple (o mesmo de `settings_screen.screen.categories[].id`,
  ex.: `com.apple.STMExtension.Documents`), `system_data` (Dados do Sistema), `mixed` (a pasta
  junta várias linhas: `~`, `~/Library`, `~/Desktop`, `~/Documents`, containers, iCloud Drive...) ou
  `null` (`large_xattrs`).
- `settings_line_grade`: `A` (regra lida no código da Apple), `B` (hipótese: snapshots, outra
  conta, containers e iCloud Drive (`~/Library/Containers/*`, `~/Library/Mobile Documents/*`),
  "Mesa e Documentos" do iCloud, bibliotecas de Música e Fotos) ou `null` junto com a linha.

**`top[].label`** (opcional, do `anexar`, só no completo e só com `home` na raiz): nome do provedor
da linha `~/Library/Application Support/FileProvider/<uuid>` não dobrada, lido do xattr
`com.apple.file-provider-domain-id` da própria pasta (`<bundle>/<domínio>`, sem seguir link). Grau
B: xattr observado no macOS 26 e 27, sem documentação da Apple. Valores fechados, pelo bundle:
`iCloud Drive` (`com.apple.CloudDocs.*`), `Google Drive` (`com.google.drivefs.*`), `OneDrive`
(`com.microsoft.OneDrive*`), `Dropbox` (`com.getdropbox.*`); outro bundle ou sem xattr,
`File Provider`. O domínio (traz a conta), o id e o bundle cru nunca saem; por isso a redação não
mexe no campo. Outras linhas: sem a chave.

**`top[].owner`** (opcional, do `anexar`, só no completo e só com `home` na raiz): o app dono da
linha, `{"bundle_id": "<id>", "name": "<nome do app>" | null, "source": "<fonte>"}` (`src/owners.rs`,
Frente C). Só sai quando há fonte no disco; sem fonte a chave não existe (nunca `null`, nunca
"desconhecido"). Linha `folded` e linha sem `path` nunca têm dono. `bundle_id` só tem `A-Z a-z 0-9 . - _`
(até 200); id fora disso não vira dono. `name` é o nome do arquivo `.app` sem a extensão (o do
Finder) e fica `null` quando o container diz o id, mas nenhum app instalado tem esse id (app removido
ou fora das pastas de apps). Fontes, da mais firme para a mais fraca:

| `source` | Caminho da linha | Prova |
|---|---|---|
| `app_bundle` | dentro de `/Applications` ou `~/Applications`, no primeiro `*.app` do caminho | `Contents/Info.plist` desse `.app` (`CFBundleIdentifier`) |
| `container` | `~/Library/Containers/<x>` | xattr `com.apple.containermanager.identifier` da pasta (o id que o containermanagerd gravou). Id `com.apple.*` sem app instalado: sem dono (serviço do sistema) |
| `group_container` | `~/Library/Group Containers/<g>` | o id do grupo está no xattr da pasta **e** exatamente um app instalado o declara em `com.apple.security.application-groups`, nos entitlements da assinatura do executável principal (lidos do Mach-O, sem `codesign`). Grupo de vários apps (Office): sem dono |
| `folder_bundle_id` | `~/Library/{Application Support,Caches,Logs,HTTPStorages,WebKit}/<x>` | `<x>` é igual ao bundle id de um app instalado |
| `icloud_container` | `~/Library/Mobile Documents/<x>` | `<x>` (`iCloud~a~b~c` ou `<TIME>~a~b~c`, `~` = `.`) é igual ao bundle id de um app instalado. Só o nome da pasta; o conteúdo não é aberto |
| `folder_app_name` | as mesmas pastas de `folder_bundle_id` | `<x>` (ou `<x> <subpasta>`, como `Google/Chrome`) é igual, sem maiúsculas, ao nome do arquivo, ao `CFBundleDisplayName` ou ao `CFBundleName` de **um único** app instalado. A mais fraca: nome igual não prova posse; dois apps com o nome, e nenhum dono |

Apps instalados = os `*.app` de `/Applications`, `/Applications/Utilities`, `/System/Applications`
(e `Utilities`), `/System/Cryptexes/App/System/Applications` e `~/Applications` (um nível; app de iOS
no Mac, pelo `WrappedBundle`). Fora disso (volume externo, subpasta) o app não existe para a regra.
Não usa provenance, quarantine, prefixo de bundle id nem catálogo curado como prova. A redação
(`--redact names`, MCP) passa `bundle_id` e `name` pela mesma limpeza dos outros nomes.

**Ficha:**

| Campo | Tipo | O quê |
|---|---|---|
| `what`, `owner`, `loses`, `step` | texto traduzido | o que é, de quem é, o que se perde se remover, o passo |
| `self_cleans` | `yes`, `when`, `no`, `unknown` | se o macOS (ou o dono) cuida sozinho |
| `self_cleans_text` | texto traduzido ou `null` | o "quando" (só em `yes` e `when`) |
| `level` | nível do `advice` | `measurement`, `system_handles`, `safe`, `cost`, `check_first`, `anomaly`, `do_not_touch` |
| `actions` | `[{kind, target, command}]` | `kind`: `finder`, `settings`, `owner_app`, `command`, `restart`, `log_out`, `none`. `target` só em `settings` (`storage`, `time_machine`, `spotlight`); `command` só em `command`, sempre da lista fixa do `advice` |
| `refs` | `[{kind, ref}]` | `kind`: `internal` (arquivo do repositório), `apple`, `report` (link de relato), `measured` (medição registrada no repositório) |
| `verified_on` | string | versão do macOS em que a ficha foi conferida |

**`sdtally --catalog --json`** (sem `--json` sai com 2; não aceita caminho nem outro modo):

```
{"schema_version": 1, "kind": "catalog", "cards": {…todas…},
 "sources": {"<source>": {"checked": "byte|margin|no|n/a", "checked_on": ["15", "26", "27"],
                          "checked_at": "AAAA-MM-DD"|null, "apple_command": str|null}}}
```

`sources` tem uma entrada para cada `source` que aparece em qualquer saída. `checked`: `byte` (igual
ao byte a um número da Apple), `margin` (dentro de uma margem medida), `no` (sem comparação possível),
`n/a` (conta, não leitura). `checked_on`: versões do macOS em que foi conferida; `checked_at`: data
da conferência (`null` se não houve). `apple_command`: o comando da Apple que mostra o mesmo valor,
de uma lista fixa (`catalogo::COMANDOS_VERIFICAR`), ou `null`.

## Fase 6: dados

Tudo aditivo, `schema_version` continua 1. Fonte que falha dá `null`, nunca 0.

**Raiz** (completo e `--quick`): `boot_time`, "AAAA-MM-DD HH:MM" em hora local, do `sysctl
kern.boottime` (fonte `sysctl_kern_boottime`), ou `null`.

**`informational.snapshots[]`** ganha, além de `kind`, `date` e `private_bytes`:

| Campo | O quê |
|---|---|
| `purgeable` | bool do `diskutil apfs listSnapshots /System/Volumes/Data -plist` (fonte `diskutil_list_snapshots`), junção por `SnapshotXID`; nome e UUID não são guardados |
| `cumulative_bytes` | fsctl {0, xid}: privado acumulado do mais antigo até este |
| `creator` | até 3 componentes reverse-DNS do nome (`com.apple.TimeMachine`, `com.apple.os`, `com.bombich.ccc`), cada um `[A-Za-z0-9-]`; senão `null` |
| `tm_role` | `reference` ou `stable` quando a data do snapshot TM é igual (ao segundo) a `reference_at` ou `stable_at` de algum destino; senão `null` |
| `verdict` | por prioridade: `tm_reference`, `tm_stable`, `tm_recent` (TM com menos de 24 h), `tm_aged` (24 h ou mais), `apple_other` (`com.apple.*` fora do TM), `third_party`. Idade = `generated_at` − `date`; TM sem `date` dá `null` |
| `card` | id da ficha, preenchido pelo `catalogo::anexar` |

**`informational.time_machine`** (só no completo; `null` se a etapa de snapshots não rodou):

```
{state: "measured"|"not_measured", reason: str|null, source: "tm_preferences_plist",
 running: bool|null,
 destinations: [{reference_at, stable_at, last_completed_at, last_attempt_at,
                 attempts_since_completed, result_raw}] | null}
```

- Lido de `/Library/Preferences/com.apple.TimeMachine.plist` com `plutil -extract`, chave por chave,
  de uma lista fixa (`Destinations`, e por destino `ReferenceLocalSnapshotDate`,
  `StableLocalSnapshotDate`, `RESULT`, `SnapshotDates`, `AttemptDates`). `NetworkURL`,
  `LastKnownVolumeName`, `DestinationID` e UUIDs nunca são pedidos.
- Datas em hora local, "AAAA-MM-DD HH:MM:SS" (o plist guarda UTC).
- `last_completed_at`: o último `SnapshotDates` ("último backup registrado").
  `attempts_since_completed`: quantos `AttemptDates` vêm depois dele (`null` sem um dos dois).
- `result_raw`: o `RESULT` cru, sem significado atribuído.
- `running`: `Running` do `tmutil status` (1 → true, 0 → false, falha → `null`).
- Plist ilegível ou ausente: `state: "not_measured"`, `destinations: null`. Sem a chave
  `Destinations`: `measured` com lista vazia.

**`informational.deleted_open_processes[]`**: ganha `launchservices_dv_files`, os inodes únicos do
processo cujo nome contém `com.apple.LaunchServices.dv`, contados antes da deduplicação global por
(DEVICE, NODE); o nome do arquivo é descartado na leitura. A lista passa a vir por
`max(bytes, logical_bytes_without_allocated)` decrescente (a forma não muda).

**`top[].top_process`** do termo `deleted_open`: o processo de maior `bytes` medido, não mais o
primeiro da lista.

## Fase 6: regras e detectores

Tudo aditivo; `schema_version` continua 1. Só no completo.

**`advice[]`**, além de `kind` e `card`:

| Campo | Onde | O quê |
|---|---|---|
| `evidence` | só `kind: "signature"` | `[{key, value, limit, source}]`: o número medido, o limite e a fonte. Nas linhas de `REGRAS`, um item só: `key` `private_bytes` (ou `bytes` quando a regra compara Ocupa ou algum privado é `null`), `value` = Σ do grupo, `source` = a da 1ª linha |
| `bytes_logical` | só quando `true` (`chronod_datavault`) | os `bytes` são lógicos (`SIZE` do lsof) e ficam fora de qualquer soma |
| `bytes_floor` | só quando `true` (`tm_backup_stuck` com algum snapshot que não é do TM) | os `bytes` são um piso (Σ dos privados do TM), não o total; o texto diz "at least" / "pelo menos" |

Ids novos:

| id | kind | level | Condição |
|---|---|---|---|
| `asset_voices`, `asset_xcode`, `asset_sim_runtime` | rule | `cost` | linhas de `/System/Library/AssetsV2/com_apple_MobileAsset_*` da ficha, Σ `bytes` ≥ 1 GB (Ocupa: o privado vem 0) |
| `asset_os_update` | rule | `system_handles` | idem |
| `spotlight_index_large` | signature | `check_first` | `/.Spotlight-V100` e `~/Library/Metadata/CoreSpotlight`, Σ privado ≥ 20 GB |
| `failed_smb_mount` | signature | `anomaly` | `/Volumes` (`tree` ou `protected_folders`) ≥ 1 GB |
| `restart_worth` | rule | `safe` | `swap_large` e `deleted_open` valem juntas: sai no lugar das duas; `bytes` = swap total + apagados, `process` = o do `deleted_open` |
| `tm_backup_stuck` | rule | `check_first` | termo `snapshots` ≥ 1 GB; snapshot TM com `tm_role: reference` e mais de 24 h (privado pode ser 0); `attempts_since_completed` ≥ 2 no destino dele; `time_machine.running: false`. `bytes` = o termo (com algum snapshot que não é do TM, a soma dos privados do TM, um piso; vale também para o portão de 1 GB), `private_bytes` = o da referência. Tira `time_machine_snapshots` de `advice` (a única supressão) |
| `chronod_datavault` | signature | `safe` | `deleted_open_processes[name = chronod]` com `files_without_allocated` ≥ 50, `logical_bytes_without_allocated` ≥ 1 GB e `launchservices_dv_files` ≥ 50; `bytes` lógicos |

As linhas de `REGRAS` casam por ordem: as `asset_*` vêm antes de `system_folders` e as linhas delas
saem de lá. Os dois últimos são detectores: leem só o JSON montado, nunca o disco.

**`advice_unchecked`** (raiz): `[{id, card, reason}]`, os detectores que não deram para olhar, em vez
de dispararem. `reason` em inglês: `term not measured` (termo fora de `measured`/`partial`),
`plist unreadable` (`time_machine` `null` ou `not_measured`), `tm status unreadable` (`running`
`null`), `no lsof` (sem `deleted_open_processes`). Com o termo `snapshots` abaixo de 1 GB, ou sem
snapshot TM com mais de 24 h, `tm_backup_stuck` não precisa do plist e fica fora da lista.

## Fase 6: `sdtally --verify --json` (kind `verify`)

Confere, agora e lado a lado, os números que um comando da Apple mostra igual. Sem root (com euid 0
recusa, como o `--last`), só leitura, nada gravado; fora do MCP. Sem `--json`, o mesmo em texto.
Não aceita caminho, `--quick`, `--redact` nem outro modo.

```
{"schema_version": 1, "kind": "verify", "file_generated_at": str|null,
 "checks": [{"id", "sdtally": int|null, "apple": [a, b]|null, "unit": "B|KiB|MiB/100|count",
             "result": "equal|within|outside|not_checkable",
             "apple_command": str|null, "in_file": int|null, "reason": str|null}]}
```

Cada conferência é um sanduíche: o comando da Apple (`a`), o sdtally, o comando de novo (`b`). O valor
do sdtally sai convertido para a `unit` do comando, arredondado para baixo ou para cima (o
arredondamento do comando não é documentado): é a resolução da fonte, não tolerância. `equal`: `a` =
`b` e o valor bate; `within`: o valor cai em `[min(a,b), max(a,b)]` (o disco mudou entre as
leituras); `outside`: não cai.

| `id` | sdtally | Apple (`apple_command`) | `unit` |
|---|---|---|---|
| `data_used` | `ATTR_VOL_SPACEUSED` | `df -k /System/Volumes/Data` (Used) | `KiB` |
| `swap` | `sysctlbyname("vm.swapusage")` | `sysctl vm.swapusage` (total) | `MiB/100`; mesmo contador do kernel, confere a leitura e a conversão (dito no `reason`) |
| `snapshots_tm` | `fs_snapshot_list` | `tmutil listlocalsnapshots /` | `count`; `equal` exige a mesma quantidade e as mesmas datas (do nome) |
| `capacity`, `macos`, `screen` | — | — | `not_checkable`, `reason: "same source"` |
| `purgeable`, `tree`, `dir_stats`, `apple_intelligence` | — | — | `not_checkable`, `reason` com o `checked` do catálogo |

`in_file` (informativo): o mesmo número na medição mais nova que o app salvou (a do `--last`), na
`unit` da linha; `file_generated_at` é o `generated_at` dela. Sem arquivo, os dois são `null`.
Comando ou leitura que falha dá `not_checkable` com o motivo em `reason`.

Código de saída: 0 se nada deu `outside`, 1 se algo deu `outside`, 2 em erro de uso.

## Fase 2: categorias do Ajustes

Só no `--json` completo; ausente em medição salva por um sdtally antigo. Tudo só leitura. O
objeto `system_data` tem duas partes.

**`system_data.by_line`** [objeto], do maior para o menor: a Σ de `top[].bytes` por `settings_line`.
`line` (id da categoria, `system_data` ou `mixed`), `bytes`, `grade_b_bytes` (a parte que vem de
regra de grau B), `rows` (linhas do `top` somadas). A entrada `line: "system_data"` é o que a
travessia mediu e as regras mandam para Dados do Sistema; o app a mostra na tela do Ajustes > Dados
do Sistema ("Ao todo, as pastas medidas que caem aqui somam..."). Linha do `top` sem `settings_line`
fica fora. Não é o número do Ajustes: o painel subtrai o que o macOS libera sozinho.

**`system_data.reproduced`**: as categorias do Ajustes reproduzidas por regras sobre o que o
sdtally mede (`src/sources/categorias/`).

| Campo | Tipo | Descrição |
|---|---|---|
| `state` | texto | `measured` (todas as 15 medidas) ou `partial` |
| `categories[]` | [objeto] | uma por categoria, na ordem da Frente P: `id`, `bytes` (`null` se não medida), `state`/`reason` (como `Medida`), `grade` (`A` regra lida na Apple, `B` hipótese ou aproximação), `source`, `note`, e, com o log do painel do Ajustes na última hora, `screen_bytes`, `error_bytes` (nosso − tela) e `error_percent` |
| `total` | objeto | `bytes` (Σ das reproduzidas), `screen_bytes` (Σ do log), `error_bytes`, `error_percent`, `tolerance_percent` (1,0), `within_tolerance` (`null` sem log), `not_reproduced` (categorias sem número) e `not_reproduced_screen_bytes` (quanto a tela tem nelas) |
| `unplaced` | objeto | `bytes` das linhas que as regras não colocaram em categoria nenhuma e `reasons` (`app_containers`, `desktop_documents`, `unexpected_path`) |
| `inputs` | objeto | o que se leu para decidir: `apps_listed`, `desktop_in_icloud`, `documents_in_icloud`, `other_accounts`, `mail_purgeable_read`, `tree_rows` (`unfolded_buckets` ou `top_rows`) e `clone_copies_bytes_added` |

`source` de cada categoria: `tree_buckets` (Σ dos baldes da travessia, sem dobrar, com a cópia de cada
clone completo contada de novo, como o `STMSize` da Apple), `music_library_db` e `tv_library_db`
(leitura do `Library.musicdb`/`Library.tvdb`), `messages_db` (soma no `chat.db`), `library_presence`
(Livros e Podcasts: sem biblioteca é 0; com biblioteca, não medido), `not_reproduced` (iCloud Drive).

Sem o painel aberto na última hora não há `screen_bytes`: o `error_*` fica `null` e quem confere é o
dono, abrindo Ajustes > Armazenamento e rodando de novo.

### Erro medido (Mac M4, macOS 27, sem root, com Acesso Total ao Disco; 18/10/2026)

Oráculo: `frenteq`, o spike que chama as funções da Apple (StorageManagement, CacheDelete,
PhotoLibraryServicesCore e o `Library.musicdb`), rodado no mesmo minuto. Não havia log do painel.
O teste `mac27_bate_com_o_oraculo_a_um_por_cento` (`tests/categorias.rs`, fixture
`tests/fixtures/completo-mac27.json`, escrubada) repete a conta e exige Σ dentro de ±1 %, cada
categoria de grau A dentro de 1 % (mínimo 100 MB) e as de grau B dentro de 2 %.

| Categoria | Erro | Grau |
|---|---|---|
| Aplicativos | +0,05 % | B (containers, Spotlight) |
| Documentos | 0,00 % | A |
| Música | 0 B | A |
| Fotos | +1,86 % | B (alocado da biblioteca; a tela usa lógico − purgeable) |
| TV, Podcasts, Lixo, GarageBand, iOS, Developer, Mensagens, Outras contas | 0 B | A |
| Mail | 0 B | B (sem o purgeable do `MailCacheDelete` lido, é um teto) |
| Livros | não medido | com biblioteca, falta o tamanho por livro |
| iCloud Drive | não reproduzido | falta o banco do File Provider |
| Σ | +0,13 % | dentro de ±1 % |

Grau B: iCloud Drive (e a parte de Documentos e Mesa gerida por ele), Fotos, Aplicativos
(containers e `.app` fora de `/Applications`), Mail sem purgeable, Outras contas sem root.
A home falsa (`home_falsa_bate_com_o_oraculo_ao_byte`) fecha ao byte com um oráculo próprio
(Σ `st_blocks × 512`, hardlink uma vez, clone completo de novo).

## Lixo por idade e metadados do APFS (`informational.trash`, `informational.apfs_metadata`)

Ambos informativos (não somar): o Lixo já está em `top`/Σ `terms.tree`; o metadado já está em
`terms.apfs_metadata` (e em `top`, com `term: apfs_metadata`), e portanto **não** está mais dentro de
`unexplained`. Nenhum dos dois derruba a coleta (um motivo de não medido não vira `stage_errors`
nem INCOMPLETO). Sem root.

### `trash`

O `~/.Trash` da conta (com `sudo`, a do `SUDO_USER`), só do volume Data: o projeto não percorre
`/Volumes/*/.Trashes`, então o Lixo de volume externo **não é medido**.

| Chave | O quê |
|---|---|
| `scope` | `~/.Trash` |
| `threshold_days` | 30 |
| `total` | Medida + `items`: tudo o que está no Lixo (alocado, por item de primeiro nível; pasta conta com o que tem dentro) |
| `older` | Medida + `items`: o que entrou no Lixo há **mais** de 30 dias. `partial` se algum item não deu a data (`undated`) ou o tamanho |
| `undated` | `{items, bytes}` dos itens sem data de entrada (ficam fora de `older`); `null` se não mediu |
| `source` | `getattrlist_addedtime` |

A idade vem de `ATTR_CMN_ADDEDTIME` (getattrlist(2): "criado ou renomeado para dentro da pasta
onde está", e o Finder move para o Lixo por rename), nunca do mtime. Medido no Mac do dono: arquivos
com mtime de 2023 no Lixo tinham entrado 5 dias antes. Limites: item que veio de outro volume (cópia)
mostra a data da cópia; hardlink pode dar data inconsistente (o próprio man diz). O atributo é
`ATTR_CMN_ADDEDTIME` (comum), não `ATTR_CMNEXT_*`. `~/.Trash` sem Acesso Total ao Disco dá
`not_measured` com `reason` (nunca 0); pasta ausente é 0 medido. O sdtally só informa: não esvazia nem sugere esvaziar.

### `apfs_metadata`

Medida (`bytes`, `source: apfs_user_client`) mais:

| Chave | O quê |
|---|---|
| `nodes` | `{omap, fsroot, extentref}`: nós das árvores vivas do volume Data; `null` se não mediu |
| `block_size` | bytes por bloco (seletor 6 do `AppleAPFSUserClient`); `null` se não mediu |

`bytes` = (omap + fsroot + extentref) × `block_size`, pelos seletores de leitura 6, 7 e 0x1e do
`AppleAPFSUserClient` (IOKit; Frente G). Sem root, sem escrita. Identidade provada numa imagem APFS sem
snapshots (`fs_used = nós × 4096 + 8192 + blocos de dados`); no Data só as árvores vivas estão
cobertas: as árvores de cada snapshot e as auxiliares (snap_meta, fext) não entram, então é um piso
do metadado. É o termo `apfs_metadata` de `data_volume.terms` (a travessia só soma arquivos, então
ele não se sobrepõe à árvore) e sai de `unexplained`; este bloco guarda o detalhe (`nodes`, `block_size`).
No `top` ele é uma linha sem `path`, na linha `system_data` do Ajustes (grau B: o metadado do volume
cai em Dados do Sistema por diferença; a Apple não o lista). Os seletores são privados e
foram conferidos só no macOS 27: o `fs_used` do seletor 7 precisa bater (1 %) com o
`ATTR_VOL_SPACEUSED` do Data, senão é `not_measured` ("o volume ... não confere"); erro do seletor
também. Em 15 e 26 pode sair `not_measured`.

## Pastas protegidas sem tamanho (`informational.protected_without_size`)

As pastas que a travessia não abriu (EPERM/EACCES, `denied_folders`) entram em
`terms.protected_folders` pelo dir-stats do APFS. Esta lista traz as que ficaram **sem** número
(dir-stats devolveu erro, ou 0, que não se distingue de "não mantido"): elas não entram na soma e o
termo sai `partial`. Informativo (não somar). `null` se a etapa `dirstats_negadas` não rodou. Não
confundir com `protected_without_size` do `sdtally <pasta> --json` (um inteiro).

| Chave | Tipo | O quê |
|---|---|---|
| `count` | int | total de pastas sem tamanho (sempre o total, mesmo com a lista cortada) |
| `folders` | [objeto] | até 200, em ordem de `path`; cada uma `{path, errno}` |
| `folders[].path` | string | o balde da pasta, o mesmo critério e a mesma anonimização de `top[].path` (home como `~`; com `--redact`, `~/<pasta do usuário>` etc.); pastas do mesmo balde aparecem repetidas |
| `folders[].errno` | int ou `null` | errno do fsctl; `null` quando o dir-stats devolveu 0 |

Os processos de `deleted_open` sem tamanho alocado já saem em
`informational.deleted_open_processes[].files_without_allocated`.

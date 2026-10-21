# Frente K — Impacto da varredura no Mac e pesquisa da fase de limpeza — 2026-09-28

Duas perguntas:

1. **A varredura atrapalha o Mac?** Quanto custa em memória e CPU, e o que as políticas de
   prioridade (I/O e QoS) mudam.
2. **Como seria a limpeza?** Para cada alvo comum de System Data: comando oficial, o que ele
   informa antes, o que acontece depois, riscos e se exige root. **Nada foi limpo**: só `--help`,
   `man`, código-fonte e dry-run lido (não executado).

Máquina: Apple M4 (4 núcleos P + 6 E, `hw.perflevel0/1.logicalcpu`), macOS 27 (Darwin 27.0.0),
volume Data com 1,89 mi arquivos e 354 mil pastas. Processo filho do Claude Code (tem FDA), sem
sudo. `tmutil status` → `Running = 0` antes e durante todas as rodadas (checado pelo script a
cada execução; nenhuma espera registrada).

Confiança: **A** = medido aqui, código-fonte lido ou documentação primária; **M** = inferência
forte sobre fato medido; **B** = relato de terceiro, não verificado aqui.

---

## 0. Resumo

1. **O gargalo é CPU de kernel, não disco nem o nosso Mutex.** Com 10 threads a varredura gasta
   **~90–100 s de CPU de sistema em ~17 s** (≈5,5 núcleos ocupados); com 4 threads, **~33 s de CPU
   no mesmo tempo** (16,6–20,6 s). Acima de 4–5 threads o tempo não cai e a CPU triplica (A).
2. **Recomendação: padrão = número de núcleos P (4 aqui), QoS `utility`.** Mesmo tempo que 10
   threads, 1/3 da CPU, e a latência de agendamento de outro processo fica no nível do Mac ocioso
   (p99 0,64–1,06 ms contra 2,2–5,7 ms com 10 threads) (A).
3. **`IOPOL_THROTTLE`/`UTILITY` não mudam nada com cache quente** (17,0 vs 16,7 s a 10 threads), e
   deixaram 1 thread 52 % mais lenta (45 → 69 s). Com cache frio (após reboot) não foi medido:
   exige `purge`, que exige sudo (A para o medido; cache frio em aberto).
4. **Contenção do Mutex é desprezível:** 1,5–1,9 % dos `lock()` encontraram o lock ocupado, 0,03–0,8 s
   somados de espera em 16–17 s × N threads. No `sample`, `__psynch_mutexwait` = 41 de ~19,6 mil
   amostras. Não trocar por `crossbeam-deque` (A).
5. **O `open()` da pasta escala mal:** fatia do `open` nas amostras vai de 9 % (1 thread) a 16 % (4)
   e 44 % (10). Trocar por `openat(fd_pai, nome)` **não ajudou** (testado, 3 repetições): a
   contenção é no kernel, não no comprimento do caminho (A).
6. **Pico de memória: 31 MiB (1 thread) a 74 MiB (16 threads)** para 1,89 mi arquivos; `peak memory
   footprint` 60 MiB a 10 threads (A, `/usr/bin/time -l`). Dentro da meta de 100 MiB/1 mi.
7. **Achado de segurança: a varredura grava `atime` em toda pasta que lista.** 22.200 de 22.200
   pastas de teste tiveram o `atime` reescrito; com `setiopolicy_np(IOPOL_TYPE_VFS_ATIME_UPDATES,
   PROCESS, OFF)`, 0 de 22.200. Numa ferramenta "só medição" isso é escrita no disco: **ligar
   `ATIME_UPDATES_OFF` no início, junto com o `MATERIALIZE_DATALESS_FILES_OFF`** (A).
8. **`EINTR` aparece em 12 de 18 rodadas** (1–7 ocorrências por varredura de todo o Data); o
   produto precisa repetir a chamada em `EINTR` em vez de contar como pasta perdida (A).
9. **Limpeza:** quase nenhum comando oficial informa tamanho medido antes; os que têm dry-run são
   `brew cleanup -n`, `brew autoremove -n`, `go clean -n`, `cargo clean -n` / `cargo clean gc -n`,
   `simctl runtime delete --dry-run`. **Sem dry-run:** `tmutil deletelocalsnapshots`,
   `thinlocalsnapshots`, `docker system prune`, `colima prune/delete`, `ollama rm`, `pnpm store
   prune`, `npm cache clean --force`, `uv cache clean`, `pip cache purge`, `simctl delete` (A, help).
10. **`tmutil deletelocalsnapshots /` apaga TODOS os snapshots do volume** (man: "If a disk is
    specified, delete all local Time Machine snapshots on the specified disk"). O CLI só deve
    imprimir a forma por data (A).
11. **Crate `trash` 5.2.9: no macOS o padrão NÃO é `NSFileManager`**, é `osascript` mandando o
    Finder apagar (pede permissão de Automação, faz som). `trashItemAtURL` só com
    `DeleteMethod::NsFileManager` explícito. E caminho não-UTF-8 é convertido para `%XX` antes do
    `fileURLWithPath`, o que aponta para **outro nome de arquivo** (A, código lido).
12. **`log erase` apaga a evidência que o produto lê** (o bloco `StorageLogInvestigation` do painel).
    Nunca oferecer (A: man + dependência do produto).

---

## Parte 1 — não atrapalhar o Mac

### 1.1 Método

- Código: `research/experimentos/frenteK/src/bin/walk_k.rs`, cópia do `probe/src/bin/walk_par.rs`
  com: `SDC_IOPOL` (`setiopolicy_np(IOPOL_TYPE_DISK, IOPOL_SCOPE_PROCESS, x)`), `SDC_QOS`
  (`pthread_set_qos_class_self_np` na thread principal e em cada trabalhadora), `SDC_NOATIME`,
  `SDC_OPENAT`, contadores de `try_lock` falho, tempo esperando o lock e o `Condvar`, e `getrusage`
  (CPU user/sys, RSS, trocas de contexto). Constantes conferidas no SDK (`sys/resource.h`:
  `IOPOL_THROTTLE=3`, `IOPOL_UTILITY=4`; `sys/qos.h`: `UTILITY=0x11`, `BACKGROUND=0x09`) (A).
- Sonda de primeiro plano: `fgprobe.rs`. Durante 15 s, em QoS padrão: (a) dorme 2 ms e mede o
  atraso ao acordar (disputa de CPU); (b) `pread` de 4 KiB em offset aleatório de um arquivo de
  1 GiB com `F_NOCACHE`.
- Scripts: `bench.sh` (matriz) e `bench_fg.sh` (sonda). Compilado com
  `CARGO_TARGET_DIR` no scratchpad; saídas no scratchpad (`frenteK/bench1..3`).
- Raiz: `/System/Volumes/Data`, profundidade de balde 1, `SDC_QUIET=1`.
- **Cache quente**: todas as rodadas depois da primeira do dia encontram os metadados no cache.
  Não há como esvaziar o cache sem `sudo purge`.

### 1.2 Threads × throttle (bench1, uma rodada cada, 10:55–11:04)

| Threads | Política | Tempo (s) | CPU sys (s) | CPU/tempo | maxrss (MiB) | Mutex ocupado | Espera no lock (s) |
|---|---|---|---|---|---|---|---|
| 1 | padrão | 45,5 | 21,1 | 0,5 | 32,3 | 0 % | 0 |
| 1 | `IOPOL_THROTTLE` | **69,0** | 25,6 | 0,4 | 31,3 | 0 % | 0 |
| 4 | padrão | 17,7 | 33,5 | 1,9 | 52,2 | 0,4 % | 0,001 |
| 4 | `IOPOL_THROTTLE` | 21,2 | 35,5 | 1,7 | 47,5 | 0,4 % | 0,003 |
| 8 | padrão | 20,3 | 98,4 | 4,8 | 52,0 | 1,5 % | 0,028 |
| 8 | `IOPOL_THROTTLE` | 18,7 | 86,6 | 4,6 | 49,9 | 1,5 % | 0,033 |
| 10 | padrão | 16,7 | 98,6 | 5,9 | 65,3 | 1,7 % | 0,074 |
| 10 | `IOPOL_THROTTLE` | 17,0 | 87,3 | 5,1 | 62,9 | 1,9 % | 0,075 |
| 16 | padrão | 17,5 | 115,6 | 6,6 | 69,1 | 1,9 % | 0,765 |
| 16 | `IOPOL_THROTTLE` | 17,3 | 107,8 | 6,2 | 73,8 | 1,8 % | 0,819 |

CPU de usuário: 0,3–0,8 s em todas. Total de `lock()`: ~708 mil por varredura (2 por pasta).

Curva fina (curve.txt, duas rodadas):

| Threads | 2 | 3 | 4 | 5 | 6 |
|---|---|---|---|---|---|
| Tempo (s) | 28,3 / 28,7 | 20,3 / 19,9 | 16,6 / 17,8 | 16,6 / 14,4 | 15,4 / 15,0 |
| CPU sys (s) | 24,9 / 24,6 | 26,5 / 26,3 | 32,1 / 33,9 | 38,1 / 35,1 | 51,5 / 46,7 |

Leitura (A):

- Com 1 thread o processo passa metade do tempo esperando (21 s de CPU em 45 s): há I/O ou
  espera de verdade. A partir de 4 threads o tempo empata em ~15–20 s e o que cresce é a CPU de
  kernel: **de 4 para 10 threads, +65 s de CPU para 0–2 s de ganho**.
- Ruído entre rodadas iguais: 10 threads padrão deu 15,5–19,7 s em 9 execuções ao longo do dia.
  Diferenças < 3 s entre configurações não são conclusivas.
- A antiga recomendação "10 threads é o ótimo" (síntese §8) mediu só tempo; pela CPU gasta, o
  ótimo é **4–5** nesta máquina.

### 1.3 Outras políticas, 10 threads (bench1)

| Política | Tempo (s) | CPU sys (s) |
|---|---|---|
| padrão (3 rodadas) | 16,7 / 19,5 / 19,7 | 98,6 / 101,7 / 98,4 |
| `IOPOL_UTILITY` | 17,1 | 86,4 |
| QoS `utility` | 18,0 | 86,4 |
| QoS `background` | 24,6 | 74,9 |
| `taskpolicy -b` (`PRIO_DARWIN_BG`) | 19,6 | 86,7 |
| `taskpolicy -c utility` | 16,9 | 91,3 |
| `taskpolicy -c background` | 18,7 | 98,5 |

Nenhuma política reduz a CPU total de forma relevante; QoS `background` a reduz um pouco porque
confina aos núcleos E (mais lentos: 24,6 s). O que reduz CPU é **menos threads** (A).

`taskpolicy` (man, A): `-d` = `setiopolicy_np(IOPOL_TYPE_DISK, PROCESS)`, `-b` =
`setpriority(PRIO_DARWIN_BG)`, `-c` = teto de QoS. O filho herda. Para o produto, chamar as APIs
no próprio processo; `taskpolicy` só serve para experimento.

### 1.4 Efeito em outro processo (fgprobe)

bench3: 3 rodadas por caso, alternadas, 15 s de sonda com a varredura ao lado. p99 em ms.

| Caso | Atraso de agendamento p99 | `pread` 4 KiB p99 | Tempo da varredura (média) | CPU sys (média) |
|---|---|---|---|---|
| Mac ocioso | 0,55 / 0,57 / 0,98 | 0,54 / 0,76 / 0,88 | — | — |
| 10 threads, padrão | **2,23 / 2,28 / 5,72** | 1,29 / 1,38 / 2,91 | 17,6 s | 86,5 s |
| 10 threads, QoS utility | 1,20 / 1,31 / 2,11 | 1,04 / 1,07 / 1,31 | 21,5 s | 82,4 s |
| 4 threads, padrão | 0,67 / 0,81 / 1,06 | 0,87 / 0,92 / 1,01 | 20,6 s | 38,5 s |
| 4 threads, QoS utility | 0,64 / 0,70 / 3,35 | 0,87 / 0,99 / 1,23 | 22,5 s | 44,5 s |
| 4 threads, QoS background | 0,58 / 0,75 / 1,36 | 0,95 / 0,96 / 1,02 | 25,4 s | 52,8 s |

bench2 (uma rodada) acrescenta: 10 threads `IOPOL_THROTTLE` p99 1,53 / 1,19 ms; 10 threads
`taskpolicy -b` 1,42 / 1,04 ms.

Leitura:

- **10 threads padrão é a única configuração que piora visivelmente o p99 de agendamento** (4×
  o ocioso). 4 threads, com ou sem QoS reduzida, fica dentro da faixa do Mac ocioso (A).
- O `pread` com `F_NOCACHE` quase não muda (0,5–0,9 ms ocioso, até 1,3–2,9 ms com 10 threads):
  com cache quente a varredura não disputa o SSD (A). Ressalva: na primeira rodada ociosa do
  bench2 o arquivo recém-criado ainda estava na memória (p50 0,002 ms), o que mostra que
  `F_NOCACHE` não descarta páginas já em cache; essa rodada foi descartada.
- Máximos isolados de 35–195 ms aparecem também com o Mac ocioso (80 ms): são do sistema, não da
  varredura.

### 1.5 Onde o tempo vai (`sample`, 5 s no meio da varredura)

| Topo da pilha | 1 thread | 4 threads | 10 threads |
|---|---|---|---|
| `getattrlistbulk` | 3.538 | 8.604 | 9.805 |
| `__open` | 363 (9 %) | 1.626 (16 %) | **7.849 (44 %)** |
| `__psynch_mutexwait` | — | — | 41 |
| `pthread_mutex_trylock` | — | — | 5 |
| código do `walk_k` | — | 15 | 19 |

(`__ulock_wait` = thread principal no `join`, fora da conta.) Porcentagem = `open` / (`open` +
`getattrlistbulk`). O Mutex não aparece; o custo está nas duas syscalls, e o do `open` cresce
com a concorrência (A).

**Teste `openat`** (`SDC_OPENAT=1`: o pai abre a subpasta com `openat(fd, nome)` enquanto ainda
tem o fd; teto de 8.192 fds, cai para o caminho completo em `EMFILE`), 3 rodadas alternadas:

| | 4 threads | 10 threads |
|---|---|---|
| caminho completo | 17,3 s, 32,5 s sys | 16,8 s, 96,4 s sys |
| `openat` | 17,5 s, 32,9 s sys | 17,7 s, 89,8 s sys |

Sem ganho. Descartado (A). O teto de fds existe porque `kern.maxfiles` é global (122.880 aqui):
uma ferramenta que abre dezenas de milhares de fds atrapalharia o resto do sistema.

### 1.6 A varredura escreve `atime`

Pasta de teste com 22.200 subpastas, `atime` forçado para 2020 com `touch -a`, contagem feita só
com `stat` (que não altera `atime`) sobre a lista de caminhos:

| Rodada | Pastas com `atime` reescrito |
|---|---|
| sem política | 22.200 / 22.200 |
| `IOPOL_TYPE_VFS_ATIME_UPDATES = OFF` | 0 / 22.200 |
| sem política (repetição) | 22.200 / 22.200 |
| OFF (repetição) | 0 / 22.200 |

Uma segunda passada 2 s depois, sem a política, não reescreveu (o APFS não atualiza `atime`
recente; a regra exata não foi identificada, B). Ou seja: a **primeira** varredura depois de um
tempo reescreve o `atime` de ~354 mil pastas do Data. É escrita de metadado (e, havendo snapshot,
nó de metadado novo em copy-on-write). O `mount` do Data não tem `noatime` (A). O man de
`setiopolicy_np` recomenda exatamente isso "for applications which access a large number of
files to reduce the metadata I/O writes" (A).

Arquivos não são abertos pela varredura; o `atime` deles não mudou (A, mesmo teste).

### 1.7 Erros observados

- `EINTR` (4): 1–7 por varredura em 12 de 18 rodadas; não se repete na mesma pasta. O `walk_k`
  herda do `walk_par` o comportamento de contar e seguir, **perdendo a pasta**. O produto deve
  repetir `open`/`getattrlistbulk` em `EINTR` (A para a ocorrência; qual das duas chamadas, não
  separado).
- `ERANGE` (34): 1 por varredura, sempre. Não investigado (em aberto).
- `EPERM` 68 e `EACCES` 183: os mesmos da síntese §3 (A).

### 1.8 Recomendações para o produto

1. No início do `main`, antes de criar threads:
   `setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, PROCESS, OFF)` (já existe),
   **`setiopolicy_np(IOPOL_TYPE_VFS_ATIME_UPDATES, PROCESS, IOPOL_ATIME_UPDATES_OFF)`** (novo) e
   `setiopolicy_np(IOPOL_TYPE_DISK, PROCESS, IOPOL_UTILITY)`.
   O throttle de disco não custou nada com 4+ threads e é o seguro para cache frio; `UTILITY` em
   vez de `THROTTLE` porque o man reserva `THROTTLE` para trabalho longo e o tempo de 1 thread
   piorou 52 % com ele (M: escolha; o efeito com cache frio não foi medido).
2. **Threads padrão = `hw.perflevel0.logicalcpu`** (núcleos P; 4 no M4), `--threads N` para mudar.
   Medido: mesmo tempo que 10 threads, ~1/3 da CPU, p99 de agendamento de outro processo igual
   ao ocioso (A).
3. QoS `utility` nas threads: sem custo mensurável com 4 threads (22,5 vs 20,6 s, dentro do
   ruído) e deixa o escalonador priorizar o usuário (M). `background` custa +25 % de tempo; usar
   só num modo "sem pressa".
4. Manter `Mutex<Vec>` + `Condvar`. Contenção medida < 2 %.
5. Repetir em `EINTR`.
6. Métrica de aceitação nova: "CPU sys ≤ 3× tempo de parede" e "zero `atime` alterado" (teste da
   §1.6 numa pasta temporária).

Em aberto: cache frio (o dono roda `sudo purge` antes de uma rodada de cada configuração, só
leitura), bateria (`powermetrics` exige sudo), e a causa do `ERANGE`.

---

## Parte 2 — fase de limpeza (só pesquisa)

Legenda da coluna "informa antes": **medido** = tamanho vindo de medição da ferramenta; **aprox.**
= a ferramenta diz que é estimativa; **nada** = não informa tamanho. "Verif." = como o comando
foi conferido aqui.

### 2.1 Tabela por alvo

| Alvo | Comando oficial | Dry-run | Informa antes | Depois | Riscos | Root | Verif. |
|---|---|---|---|---|---|---|---|
| **Snapshots locais do TM** | `tmutil deletelocalsnapshots <AAAA-MM-DD-HHMMSS>`; `tmutil thinlocalsnapshots <mount> [bytes] [urgência 1–4]`; `diskutil apfs deleteSnapshot <dev> -uuid/-xid/-name` | **não** | nada (`listlocalsnapshotdates` só lista datas) | TM cria novo snapshot a cada backup; o sistema já apaga snapshots com > 24 h, exceto o último (Oakley 2026-02-02, B) | **a forma `deletelocalsnapshots <mount>` apaga todos**; próximo backup pode ficar maior sem o snapshot para diff (Oakley, B); espaço só volta para blocos que nenhum outro snapshot segura; `fsctl` durante backup travou o processo (síntese regra 4) | man: "Several, but not all, verbs require root and Full Disk Access"; não diz quais. Relatos divergentes (B). `listlocalsnapshotdates /` rodou sem sudo aqui (A) | `man tmutil`, `man diskutil`, listagem |
| **Xcode DerivedData** | nenhum CLI; Xcode apaga pela UI | — | nada | regenera no próximo build (minutos) | projeto aberto no Xcode | não | — (B) |
| **Simuladores: runtimes** | `xcrun simctl runtime delete <id> \| --notUsedSinceDays N \| --unusable \| --outdated` | **`--dry-run` / `-n`** | `simctl runtime list -j` traz `sizeBytes` (Frente C; não rodado: travou > 3 min antes) | desmonta e apaga a imagem; `--keep-asset` mantém o MobileAsset; rebaixar custa GB | simuladores daquele runtime deixam de bootar | não (M) | `simctl help runtime` com timeout 30 s |
| **Simuladores: devices** | `xcrun simctl delete <udid> \| unavailable \| all` | **não** | nada | estado do device perdido | `all` apaga tudo | não | `simctl help delete` |
| **DeviceSupport** (`~/Library/Developer/Xcode/* DeviceSupport`) | nenhum CLI; Xcode recria ao conectar o aparelho | — | nada | Xcode copia símbolos de novo (minutos, B) | nenhum além do tempo | não | — (B) |
| **Docker (Colima/Docker Desktop/OrbStack)** | `docker system prune [-a] [--volumes] [--filter]` | **não** (só `-f` para pular a pergunta) | `docker system df` mede **dentro da VM** | libera espaço na VM; no host só se o disco da VM encolher. OrbStack: `data.img` esparso "automatically shrinks" (docs OrbStack, A) | `--volumes` apaga dados de banco local | não | `docker system prune --help`, `df --help` (Docker 29.3.1) |
| **Colima** | `colima prune [-a]` (assets baixados), `colima delete [-d]` (VM inteira) | **não** | nada; `colima list` mostra o tamanho configurado, não o ocupado (Frente C) | `delete -d` apaga os dados dos containers | `delete` é destruição total | não | `--help` (colima 0.10.1) |
| **Podman** | `podman system prune`, `podman machine rm` | B | `podman system df` (B) | — | — | não | não instalado (B) |
| **Homebrew** | `brew cleanup [-s] [--prune=N\|all]`, `brew autoremove` | **`-n` / `--dry-run`** nos dois | aprox. ("would free approximately", Frente C) | downloads e versões antigas removidos; nada regenera sozinho | o `--help` sugere `rm -rf "$(brew --cache)"`: não seguir | não (prefixo do usuário em `/opt/homebrew`, M) | `brew cleanup --help`, `brew autoremove --help` |
| **npm** | `npm cache clean --force` | não | nada (sem comando de tamanho, diz a doc) | cache refeito sob demanda | `npm cache verify` **não é dry-run**: faz coleta de lixo | não | `npm help cache` (npm 11.11.0): "clean now requires --force" |
| **pnpm** | `pnpm store prune` | não | nada | pacotes órfãos saem; instalações futuras mais lentas | `prune --force` apaga "alien directories" | não | `pnpm store --help` (11.22.0) |
| **yarn / bun** | `yarn cache clean`; `bun pm cache rm` | B | B | B | — | não | não instalados (B) |
| **uv** | `uv cache prune [--ci]`, `uv cache clean [pkg]` | não | **medido**: `uv cache size` bateu com `du` (Frente C) | refaz sob demanda | `--force` ignora a checagem de uso: nunca usar | não | `uv cache --help` (0.12.5) |
| **pip** | `pip cache purge`, `pip cache remove <pad>` | não | `pip cache info` (tamanho por tipo, Frente C) | refaz sob demanda | nenhum | não | `pip cache --help` (26.1.2) |
| **cargo** | `cargo clean` (target do projeto), `cargo clean gc` (cache global) | **`-n` / `--dry-run`** | nada | recompila / baixa de novo | opções `--max-*-age/size` marcadas unstable | não | `--help` (cargo 1.97.1) |
| **Go** | `go clean -cache`, `go clean -modcache` | **`-n`** respeitado: `Shell.RemoveAll` retorna antes de apagar se `BuildN` (`work/shell.go:324`), e o bloco de `-modcache` idem (`clean.go:223–226`) | nada | recompila / baixa de novo | `-modcache` quebra builds offline | não | `go help clean` + fonte do Go 1.26.2 |
| **Ollama** | `ollama rm <modelo>` | não | **medido**: `ollama list` bateu com `du` (Frente C) | download de GB para voltar | modelo em uso | não | `ollama rm --help` (0.32.13) |
| **LM Studio** | `lms` (CLI) ou UI | B | B | B | — | não | não instalado (B) |
| **Hugging Face** | `hf cache` (CLI) | B | B | `blobs/` são compartilhados por `snapshots/` via symlink: apagar só a pasta de uma revisão não libera o blob (docs HF, A) | apagar blob à mão quebra snapshots | não | docs `manage-cache`; CLI não instalado |
| **Caches de app** (`~/Library/Caches/<id>`, `Containers/<id>/Data/Library/Caches`) | nenhum | — | nada | o app recria; primeira abertura mais lenta | app aberto; `CloudKit` com sync ativo (runbook) | não | catálogo `docs/04` |
| **Logs e DiagnosticReports** | nenhum para arquivos; `log erase [-A]` para o log unificado | não | nada | relatórios somem; `log erase` apaga o log | **`log erase` apaga o `StorageLogInvestigation` que o produto lê**; o macos-sysdata o marca "seguro" (síntese regra 6) | `~/Library/Logs/DiagnosticReports`: usuário (grupo `_analyticsusers`); `/Library/Logs/DiagnosticReports`: `root:_analyticsusers 770` → admin/root | `man log`, `ls -ld` |
| **`/private/var/folders`** | nenhum; `T/` é limpo no boot | — | nada | — | processos vivos usam os arquivos; relato de Mac que não religou após `sudo rm -rf` (pesquisa comunidade) | root para outros usuários | Frente C |
| **Backups iOS** (`MobileSync/Backup`) | Finder → aparelho → Gerenciar Backups; painel Armazenamento → iOS Files | não | o painel mostra tamanho (B) | backup perdido | único backup do aparelho | não (pasta do usuário, protegida por TCC) | `ls -ld` (vazia aqui) |
| **Mail** | nenhum seguro | — | nada | — | "Remover Anexos" do Mail pode apagar no servidor IMAP (B); apagar arquivo em `~/Library/Mail` corrompe o índice | não | catálogo proíbe |
| **Messages** | Ajustes do Messages → manter mensagens por 30 dias/1 ano; painel Armazenamento → Mensagens (extensão `messages.StorageManagementExtension`, Frente C) | não | o painel lista anexos (B) | apaga no iCloud se Mensagens no iCloud estiver ligado (B) | mídia real do usuário | não | catálogo proíbe |
| **Aerials** (`/Library/Application Support/com.apple.idleassetsd/Customer`) | nenhum; trocar papel de parede e protetor para estáticos e então apagar | — | nada | baixa de novo se um aerial for escolhido; relatos de rebaixar mesmo assim (Apple Community, B) | um relato (Reddit, B) diz que o sistema trata como despejável; conferir no itemizado do CacheDelete antes de oferecer | **pasta `root:admin 755`**: apagar exige admin (A, `ls -ld`) | vazia aqui (0 KiB) |
| **Spotlight** (`/.Spotlight-V100`, 1,79 GiB com root na síntese §3c) | `mdutil -E <vol>` (apaga e reconstrói), `mdutil -X <vol>` (remove sem desligar) | não | nada | reindexa: horas de CPU e I/O | busca incompleta até terminar | man não diz; relatos usam `sudo` (B) | `man mdutil` |
| **`.DocumentRevisions-V100`** (2,11 GiB de `ChunkStorage` na síntese §3c) | nenhum; app Revisionist (terceiro) remove versões por documento; limpeza interna após reiniciar | — | nada | versões apagadas não voltam | apagar arquivos dentro dessincroniza o banco e pode desativar o versionamento (Oakley 2024-04-04, 2025-09-08, B) | root para ler | fontes Oakley |
| **iCloud "Otimizar"** | Ajustes → Conta Apple → iCloud → Otimizar armazenamento do Mac; por arquivo: `NSFileManager evictUbiquitousItemAtURL:error:` (SDK, `NSFileManager.h:820`, macOS 10.7+) | não | nada | arquivo vira dataless, baixa ao abrir | só funciona para item já enviado; sem acesso offline; `brctl evict` não existe mais (Frente E) | não | header do SDK |

### 2.2 Padrões

- **Nenhum comando da Apple para snapshots, simuladores ou Spotlight informa bytes medidos antes
  de agir**, salvo `simctl runtime list -j` (`sizeBytes`, declarado pelo CoreSimulator). O
  produto mede por conta própria (`fsctl` de snapshot, travessia) e mostra o comando oficial ao
  lado, rotulado.
- **Ferramentas de desenvolvedor só têm dry-run de verdade em brew, go, cargo e simctl runtime.**
  Para as outras, o "dry-run" do produto é a própria medição: "este comando vai apagar a pasta X,
  que mede Y (privado Z)". O número é nosso, nunca o da ferramenta.
- **"Libera" ≠ "apaga".** Com snapshots presentes, apagar libera só o `PRIVATESIZE` fora de
  snapshot; clone total libera ~0; container de VM só libera se o disco esparso encolher. O
  relatório tem de mostrar o `PRIVATESIZE`, não o alocado.
- **O que exige root, o CLI imprime e não executa** (regra da Frente E): `deletelocalsnapshots`,
  `mdutil -E`, aerials, `/Library/Logs`.

### 2.3 Mover para o Lixo em Rust: crate `trash`

Fonte: crates.io API (5.2.9, 2026-09-13, MIT, MSRV 1.85, 3,1 mi downloads) e código na tag
`v5.2.9` (`54e0a2d`) de `ArturKovacs/trash`, `src/macos/mod.rs` e `src/lib.rs` (A).

| Ponto | O que o código faz |
|---|---|
| Método padrão | `DeleteMethod::new() -> DeleteMethod::Finder`: roda `osascript -e 'tell application "Finder" to delete { POSIX file "..." }'` (linhas 44–48, 112–153) |
| `NSFileManager` | Só com `ctx.set_delete_method(DeleteMethod::NsFileManager)` (trait `TrashContextExtMacos`): chama `NSFileManager::defaultManager().trashItemAtURL_resultingItemURL_error(&url, None)` via `objc2-foundation` 0.3.2 (linhas 85–110) |
| Diferenças documentadas no próprio código | Finder: "Pôr de volta" funciona, faz som, pode pedir permissão (Automação). `NsFileManager`: mais rápido, sem permissão extra, sem som, "Pôr de volta" às vezes ausente (bug do macOS citado: `sindresorhus/macos-trash#4`, `trash-rs#14`) |
| Caminho não-UTF-8 | `percent_encode` troca bytes inválidos por `%XX` e passa a string a `fileURLWithPath`, que a trata como nome literal: o alvo vira **outro arquivo** (erro ou, pior, um arquivo que exista com esse nome). Rejeitar caminhos não-UTF-8 antes de chamar (M: leitura do código, não testado) |
| Canonicalização | `canonicalize_paths` resolve só o **pai** (segue symlink do pai, não do alvo: bom). Caminho terminado em `..` retorna o pai canonicalizado, ou seja `a/b/..` manda **`a/b`** para o Lixo, não `a`. Validar os caminhos antes (A, `lib.rs` 228–258) |
| Listar/esvaziar o Lixo | `os_limited` (`list`, `purge_all`, `restore_all`) só existe para Windows e Linux (`lib.rs` 346–350). No macOS a crate não esvazia o Lixo: bom para a regra do projeto |

Recomendação: usar `NSFileManager` diretamente pelo `objc2-foundation` (`NSFileManager`,
`NSURL`), que a crate já usa por baixo, e dispensar a crate: são ~10 linhas, sem `osascript`,
sem codificação `%`, e o caminho vem de `OsStr` via `NSURL::fileURLWithFileSystemRepresentation`
(M: nome do método no `objc2-foundation` não conferido). Se usar a crate, sempre
`DeleteMethod::NsFileManager`.

Comportamento do `trashItemAtURL` a comunicar: move para o Lixo **do volume do item** (`.Trashes`
em volume externo); não libera espaço até esvaziar (`docs/03`); em pasta `root` sem permissão,
falha em vez de pedir senha (M).

### 2.4 Por que `rm` é proibido no projeto

Fontes: `README.md` ("Sempre mover para o Lixo via `FileManager.trashItem`. Nunca `rm -rf`"),
`docs/03-restricoes-de-plataforma.md` §"Mover para o Lixo, nunca apagar", síntese regra 6 (A).

1. **Irreversível.** O Lixo deixa o usuário desfazer; `rm` não. O projeto já achou casos em que o
   que parece cache é dado real (`workspaceStorage` com cópias de arquivos, `Mobile Documents`,
   backup do WhatsApp no `CloudKit`) (`docs/04`).
2. **É como limpadores destroem dados.** O `macos-sysdata` criticado pelo Oakley roda
   `rm -rf` como root via `osascript` e apaga direto os itens "Safe" (Frente E §6.1).
3. **`rm` com root atravessa proteções que existem por um motivo**; `trashItem` respeita
   permissões e falha onde o usuário não pode escrever (`docs/03`).
4. **Ganho zero sem esvaziar.** Os dois caminhos só liberam espaço quando os blocos deixam de ser
   referenciados; com snapshot presente, nenhum dos dois libera na hora. `rm` não "libera mais".
5. **Auditável.** A Fase 4 da Frente E exige "zero chamadas a `rm`, `sudo` ou `osascript` no
   código (`grep` no CI)". Com a crate `trash` no modo padrão, o `osascript` entraria pela
   dependência: mais um motivo para o `NsFileManager` explícito ou chamada direta.

---

## Artefatos

`research/experimentos/frenteK/`:

| Arquivo | O que é |
|---|---|
| `Cargo.toml` | só `libc` 0.2.189 |
| `src/bin/walk_k.rs` | `walk_par` + `SDC_IOPOL`, `SDC_QOS`, `SDC_NOATIME`, `SDC_OPENAT`, contadores de lock e `getrusage` |
| `src/bin/fgprobe.rs` | sonda de latência de primeiro plano |
| `bench.sh` | matriz threads × política (`bench.sh <bin> <saída> [raiz]`) |
| `bench_fg.sh` | sonda com a varredura ao lado |

Compilar: `CARGO_TARGET_DIR=<scratchpad>/frenteK/target cargo build --release --offline`.
Logs brutos, amostras do `sample` e o arquivo de teste de 1 GiB ficaram no scratchpad
(`frenteK/bench1`, `bench2`, `bench3`, `curve.txt`, `openat.txt`, `sample{1,4,10}.txt`), fora do
iCloud. Nada foi apagado fora do scratchpad; nenhum comando de limpeza foi executado.

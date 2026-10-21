# O que ocupa System Data na vida real — 2026-09-27

Levantamento em Apple Support, Eclectic Light (Howard Oakley), Apple Community, Stack Exchange,
Reddit, Hacker News, MacRumors e issues de ferramentas. Conselho sem número, comando ou
verificação foi descartado. Blogs de vendedores de limpadores (CleanMyDev, iBoysoft etc.)
entram só como confiança baixa. Confiança por item: **A** alta, **M** média, **B** baixa.

---

## 1. Como o painel calcula

- **Apple documenta só o conteúdo, não a conta** (A): arquivos que não cabem nas outras
  categorias — logs, caches, arquivos de VM, temporários, fontes, suporte de apps, plugins.
  [support.apple.com/102624](https://support.apple.com/en-ca/102624),
  [mchl3d437fbc](https://support.apple.com/en-au/guide/mac-help/mchl3d437fbc/mac)
- **System Data é o resto** (A como observação, sem confirmação da Apple): o painel soma as
  categorias que sabe classificar e joga a diferença do espaço usado em System Data. Inclui
  snapshots "em muitos casos" e tudo que ele classifica mal (ex.: de 100+ GB em Music, só ~50
  contados como Music). Conta alocado, não aparente.
  [eclecticlight 2025-02-11](https://eclecticlight.co/2025/02/11/what-is-system-data-in-storage-settings/),
  [explainer 2025-12-20](https://eclecticlight.co/2025/12/20/explainer-storage-settings/)
- **Purgeable e "available" vêm do CacheDelete** (daemon `deleted`), não do APFS (A). Serviços
  em `/System/Library/CacheDelete`; snapshots do Time Machine são purgados por último; snapshots
  de terceiros (CCC, ChronoSync) **não** são purgeable.
  [2023-04-27](https://eclecticlight.co/2023/04/27/where-does-macos-get-its-volume-free-space-figures-from/),
  [2026-09-02](https://eclecticlight.co/2026/09/02/caches-and-purging-in-macos-tahoe/)
- **O painel atrasa** (A, medido por Oakley): após apagar 94 GB, CacheDelete recalculou em ~9 s,
  Disk Utility em <50 s, painel ainda errado após 3 min, Get Info após 7 min. Referência de
  precisão: Disk Utility no **container**.
  [2023-04-24](https://eclecticlight.co/2023/04/24/how-ventura-space-management-tracks-changes/)
- **Categorias vizinhas tiram coisas de System Data** (A): Developer (parte do Xcode), iOS Files,
  Messages, Music Creation, Trash; Apple Intelligence aparece em **macOS**.

**Implicação para o projeto:** não dá para reproduzir o número do painel (algoritmo fechado,
recálculo atrasado). O relatório mede o volume inteiro e mostra quais fatias o painel costuma
pôr em System Data, sem prometer bater com ele. Investigar depois se o processo do painel
(StorageManagement) deixa rastro em `log stream`.

## 2. Causas reais, por frequência e tamanho

| # | Causa | Tamanho relatado | Conf. | Fonte |
|---|---|---|---|---|
| 1 | Snapshots APFS (TM, `com.apple.os.update-*`, CCC, ChronoSync) | dezenas a centenas de GB; CCC 416 GB | A | [SE 367879](https://apple.stackexchange.com/questions/367879), [AC 255806791](https://discussions.apple.com/thread/255806791), **medido aqui: 96 GiB** |
| 2 | iCloud Drive baixado (Optimize desligado, redownload pós-update) | 50–700 GB | A | [AC 256229682](https://discussions.apple.com/thread/256229682), [eclecticlight](https://eclecticlight.co/2025/07/24/beware-the-full-startup-disk/) |
| 3 | Xcode: CoreSimulator, runtimes, DeviceSupport, DerivedData | 20–300 GB | A | [mjtsai](https://mjtsai.com/blog/2025/08/19/tahoe-free-space-problems/) |
| 4 | `~/Library` Caches / App Support / Containers / Group Containers, restos de apps | 5–100+ GB; Podcasts 120–250 GB "sem downloads" | A | [AC 251323776](https://discussions.apple.com/thread/251323776), [HN](https://news.ycombinator.com/item?id=47237939) |
| 5 | Docker, Podman, Parallels, UTM, apple/container | 10–100+ GB alocados; arquivos esparsos de TB aparentes | A | [apple/container#119](https://github.com/apple/container/issues/119) |
| 6 | Modelos de IA (`~/.ollama`, LM Studio, `~/.cache/huggingface`) | 10–230 GB | M–A | AC 255806791 |
| 7 | Bugs de daemons da Apple (seção 3) | até centenas de GB | A existe / M versões | — |
| 8 | `/private/var/folders` (T e C por usuário) | 1–85 GB | A | [AC 256045921](https://discussions.apple.com/thread/256045921), [SE 404355](https://apple.stackexchange.com/questions/404355) |
| 9 | Toolchains fora de `~` (`/usr/local/share/dotnet` ~200 GB, Homebrew) | variável | M | [jcsawyer](https://jcsawyer.com/blog/2026-08-12-reducing-macos-system-data-size) |
| 10 | Volumes VM (swap, sleepimage), Update, Preboot (33 GB num Sonoma) | 1–33 GB | M | [AC 255666161](https://discussions.apple.com/thread/255666161) |
| 11 | Restos de update (`/macOS Install Data`, `/Library/Updates`) | 3–13 GB | M | [AC 253293163](https://discussions.apple.com/thread/253293163) |
| 12 | Assets de sistema (`AssetsV2`, Apple Intelligence) — categoria macOS, confundido com System Data | 8–30 GB | A | [MacRumors 2026-09-23](https://www.macrumors.com/2026/09/23/apple-intelligence-30gb-some-macs-macos-27/) |
| 13 | Mail (log do Connection Doctor de 300 GB), Messages, WhatsApp, jogos, Adobe | variável | M | [AC 255359308](https://discussions.apple.com/thread/255359308) |

### macOS 27 (reportagem de uma semana, causa desconhecida)

Modelo de Apple Intelligence baixa obrigatoriamente, sem o toggle que apagava. Apple documenta
até 14 GB; o painel mostra 20–30 GB. AppleInsider apagou os modelos: `du` = 23 GB, painel caiu
24,5 GB, **espaço livre real subiu ~14 GB** — prova de que `du` e painel não medem o que o disco
libera. É exatamente o problema que `PRIVATESIZE` resolve.
[AppleInsider](https://appleinsider.com/inside/macos-27/tips/how-to-remove-apple-intelligence-models-reclaim-storage-in-macos-27),
[support 121115](https://support.apple.com/en-us/121115)

## 3. Casos com números (bugs e armadilhas)

- **idleassetsd** (A): aerials 4K 240 fps baixam até sobrar ~4 GB livres; dezenas de GB em
  `/Library/Application Support/com.apple.idleassetsd/Customer/4KSDR240FPS`; ~3 TB de download
  num dia. Solução: wallpaper **e** screensaver estáticos. Pego com `fs_usage`.
  [r/MacOS](https://www.reddit.com/r/MacOS/comments/1f33muq/), [AC 255931263](https://discussions.apple.com/thread/255931263),
  [SE 461119](https://apple.stackexchange.com/questions/461119)
- **mediaanalysisd, Sequoia 15.1** (M): arquivos de 64 MB por hora, nunca apagados, em
  `~/Library/Containers/com.apple.mediaanalysisd/Data/Library/Caches`; corrigido no 15.2 segundo
  usuários.
- **Spotlight descontrolado** (M): `mds_stores` gravando TB, System Data 300–384 GB com `du`
  achando 90 GB. [mjtsai](https://mjtsai.com/blog/2025/07/21/spotlight-indexing-running-wild/)
- **Montagem SMB que falhou** (A): dados gravados em `/System/Volumes/Data/Volumes/sambas*`,
  286 GB. [SE 481112](https://apple.stackexchange.com/questions/481112)
- **DMG APFS meio montado** (A): 226 GB que voltaram ao ejetar. [SE 339928](https://apple.stackexchange.com/questions/339928)
- **`~/.Trash` com 86 GB** e Finder mostrando vazia (permissões) (M). [AC 255526095](https://discussions.apple.com/thread/255526095)
- **Arquivos apagados mas abertos** (`/var/folders/zz` preso por daemon até reboot): ocupam
  disco e nenhuma varredura de árvore os vê.
- **Queda sozinha** após reboot ou safe mode: 365 → 45 GB (AC 255806791).

## 4. Mitos e conselhos ruins

| Conselho | Veredito | Por quê |
|---|---|---|
| "System Data é do sistema, não mexa" | Meia-verdade | É o resto; inclui iCloud, VMs, caches de app do usuário |
| "Reinicie" | Meia-verdade | Libera swap, TMPDIR, arquivos abertos já apagados. Não resolve iCloud, simuladores, VMs, daemons que voltam |
| "Safe mode limpa" | Meia-verdade | Só purgeable e o cache dir de `/var/folders`; às vezes nada ([eclecticlight](https://eclecticlight.co/2023/09/22/will-flushing-caches-free-up-disk-space/)) |
| `sudo rm -rf /private/var/folders/*` | **Perigoso** | Quebra processos em uso; relato de Mac que não religou |
| Desativar SIP para apagar | **Perigoso** | Quase nunca necessário; há comando oficial (`simctl`, `tmutil`) ou Recovery |
| Apagar `~/Library/Caches` inteiro com apps abertos | Meia-verdade | Ganho temporário; apagar a pasta em vez do conteúdo causa bug |
| Apagar Containers / Group Containers "órfãos" | **Perigoso** | Dados reais (Messages, WhatsApp, licenças) |
| CleanMyMac e similares | Ruim | Liberam menos do que dizem (8 de 94 GB), apagam logs úteis, não mostram System Data |
| "Encha o disco para forçar o purge" | **Perigoso** | CacheDelete pode recusar; numa VM cheia o macOS parou de gravar; dispara o loop do idleassetsd |
| `tmutil thinlocalsnapshots / 999999999999 4` | Meia-verdade | Não apaga snapshots de terceiros nem `os.update`; às vezes precisa `deletelocalsnapshots` um a um |
| Apagar `.Spotlight-V100` à mão | Ruim | Usar `sudo mdutil -E /System/Volumes/Data` |
| "Apague e reinstale o macOS" | Exagero | Migration traz o problema de volta; um caso reencheu em 45 min pelo iCloud |
| Apagar volumes em "Other Volumes" | **Perigoso** | Preboot, Recovery, VM são necessários |
| "Desligar Apple Intelligence libera ~7 GB" | Desatualizado | macOS 27 não tem mais o toggle |

**Aviso direto ao projeto** (A): Oakley criticou em 06/09/2026 uma ferramenta open source de
System Data (`Jarvis322/macos-sysdata`) por abordagem perigosa, informação errada e script para
privilégio em vez de privileged helper.
[eclecticlight 2026-09-06](https://eclecticlight.co/2026/09/06/last-week-on-my-mac-storage-crisis-management/).
O projeto vai ser comparado a isso: só leitura primeiro, nada de root por script, cada número
com origem.

## 5. O que as pessoas pedem numa ferramenta

1. **Decompor o número em itens nomeados**, com caminho e tamanho. Queixa nº 1.
2. **Explicar a diferença** entre o que o analisador soma e o que o painel diz.
3. **Mostrar o que a árvore não vê**: purgeable, snapshots (inclusive de terceiros), outros
   volumes, arquivos apagados mas abertos. O DaisyDisk tem "hidden space"; o Mole confundiu
   usuário com "320 GB left" ([Mole#222](https://github.com/tw93/Mole/issues/222)).
4. **Números honestos**: clone uma vez ([dust#590](https://github.com/bootandy/dust/issues/590)),
   esparso pelo alocado, **quanto apagar libera de fato**.
5. **Não travar**: symlinks cíclicos (Mole#378); rodar mesmo com disco cheio (analisadores que
   criam cache falham na crise).
6. **Full Disk Access explicado**: OmniDiskSweeper sem FDA via 102 de 225 GB.
7. **Nada de "limpar tudo"**: risco por item, comando oficial para cada tipo.
8. **Recorrência**: "cresceu desde ontem" (daemon com bug) × acúmulo histórico.

## 6. Técnicas de medição de fonte séria

| O quê | Como medir |
|---|---|
| Uso por volume | `diskutil apfs list` (Capacity Consumed), `df -k /System/Volumes/Data` |
| Purgeable | `volumeAvailableCapacityForImportantUsage` − free do `statfs` (CacheDelete) |
| Purgeable por serviço | `log show --predicate 'subsystem == "com.apple.cache_delete"'` (bytes por serviço, ex. `com.apple.TMCacheDelete`) |
| Snapshots | `tmutil listlocalsnapshots /`, `diskutil apfs listSnapshots /System/Volumes/Data` (sem tamanho) |
| Árvore | Volume **Data**, não `/` (`-x /` para no SSV); não seguir symlinks; FDA |
| Clones, esparsos, hardlinks | `getattrlist` (`CLONEID` 0x100 confirmado no SDK e no teste daqui), `ALLOCSIZE`, (dev, ino) |
| Arquivos apagados e abertos | `lsof +L1` (inferência do agente; confirmar) |
| Quem está gravando | `sudo fs_usage -w -f filesys`, Activity Monitor > Disco |
| Simuladores | `xcrun simctl runtime list -j` (`sizeBytes`, `lastUsedAt`, `deletable`) |
| Outros | `sysctl vm.swapusage`, `getconf DARWIN_USER_TEMP_DIR` / `DARWIN_USER_CACHE_DIR`, `docker system df`, `ollama list`, `mdutil -s` |

---

## 7. Alvos da ferramenta, cada um com a medição correspondente

| # | Alvo | Medição (sem estimativa) |
|---|---|---|
| 1 | Snapshots APFS (TM, os.update, terceiros) | Lista: `diskutil apfs listSnapshots`. Bytes presos: soma de `ALLOCSIZE − PRIVATESIZE` por arquivo; o que só existe no snapshot fica no "não atribuído" |
| 2 | Purgeable | API de capacidade (CacheDelete); por serviço via log do `cache_delete` |
| 3 | iCloud Drive / File Providers baixados | Varredura de `Mobile Documents` e `CloudStorage`: alocado + contagem de `SF_DATALESS` |
| 4 | Xcode e simuladores | Varredura dos caminhos + `simctl runtime list -j` (tamanho informado pela ferramenta da Apple) |
| 5 | VMs e containers | `ALLOCSIZE` e `EF_IS_SPARSE` do arquivo de disco |
| 6 | `~/Library` por app, instalado ou órfão | Varredura; órfão = bundle id sem app correspondente (fato verificável, não palpite) |
| 7 | `/private/var/folders`, arquivos apagados e abertos | Varredura (precisa ver permissões) + `lsof +L1` |
| 8 | Assets de sistema, idleassetsd | Varredura de `AssetsV2` e `com.apple.idleassetsd` |
| 9 | Modelos de IA e toolchains fora de `~/Library` | Varredura de dotfiles e `/usr/local`, `/opt/homebrew`, com dedup de clone |
| 10 | Volumes fora do Data, DMGs, `/Volumes/*` sem mount | `diskutil apfs list`, `statfs` por mount, varredura de `/System/Volumes/Data/Volumes` |
| 11 | Spotlight, logs, restos de update, Trash de todos os usuários | Varredura (com FDA; algumas pastas pedem root) |
| 12 | Crescimento entre execuções | Guardar o resultado anterior e mostrar a diferença medida |

## 8. Incertezas

- Algoritmo do painel não publicado; "resto" é dedução do Oakley.
- Se o painel atual conta purgeable, snapshots e volume VM em System Data: relatos divergem.
- Versões exatas dos bugs de daemon vêm de usuários, sem nota da Apple.
- macOS 27: uma semana de reportagem.
- Tamanhos de blogs de vendedores: marketing, confiança baixa.
- `lsof +L1` para arquivos apagados e abertos: não há fonte que cite; testar.

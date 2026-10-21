# Frente F: iCloud Drive e File Providers — 2026-09-27

macOS 27 (build 26A5416b), Apple Silicon, usuário comum, **sem sudo, sem Full Disk Access, sem GUI**.
Backup do Time Machine rodando. Nada foi baixado nem despejado. Nomes pessoais omitidos.

Legenda de confiança: **A** = medido nesta máquina ou lido no código/header; **M** = inferência
apoiada em mais de uma evidência; **B** = hipótese.

Código: `research/experimentos/frenteF/` (ver §9).

---

## 0. Resumo

1. **`EF_IS_PURGEABLE` em iCloud Drive = "arquivo já enviado ao iCloud", não "o sistema vai
   liberar isso".** 29.539 de 30.744 arquivos enviados têm a flag (51,14 GiB); nenhum arquivo não
   enviado ou excluído do sync tem (fora 2 arquivos, 2 MB, em corrida). Com "Otimizar armazenamento"
   **desligado**, o CacheDelete oferece **0** desse espaço. Não somar ao purgeable do volume. (A)
2. **"Otimizar armazenamento" nesta máquina: desligado.** Três fontes batem: `defaults read
   com.apple.bird optimize-storage` = 0, `brctl dump` = `optimize storage: disabled`, e o domínio
   do File Provider marcado `repl` (replicado). Por isso **0 arquivos dataless** em `Mobile Documents`. (A)
3. **Os 51,80 GiB dataless do volume são todos OneDrive (43,45) e Google Drive (8,35)**, em
   `~/Library/CloudStorage`. Ocupam 0 local. Os apps dos dois **não estão rodando**. (A)
4. **Clones do CloudKit confirmados por `CLONEID`**, não por tamanho: 7 arquivos de
   `Caches/CloudKit/.../MMCS/ClonedFiles` têm o mesmo `CLONEID` dos backups do WhatsApp (19,20 GiB),
   outros 3 o de arquivos do CloudDocs (1,63 GiB). Todos com `EF_SHARES_ALL_BLOCKS` e refcount 2:
   apagar libera ~0. (A)
5. **A política de materialização padrão do processo é ON** (herdada, `getiopolicy_np` = 2). O
   kernel manda o default para OFF, mas a herança ganha. Por isso `du`/`find` pedem materialização.
   `stat`/`getattrlist` num arquivo dataless **não** disparam nada (medido). (A)
6. Sem entitlement, `NSFileProviderManager.getDomains` falha (`-2001 ProviderNotFound`). O que
   funciona sem root: `NSURLUbiquitousItem*`, `brctl status/dump/quota`, `fileproviderctl dump`,
   `getIdentifierForUserVisibleFile`. (A)
7. Fora de `CloudStorage` há **restos de legado** medidos: `OneDrive.noindex` antigo (1,36 GiB
   reais), `~/My Drive` em modo espelho do Google Drive (6,00 GiB reais), pasta de migração do
   iCloud de 27/03 (2,43 GiB, **fora de qualquer domínio**). (A para medida, M para "legado")

---

## 1. Flags por arquivo

Constantes de `<sys/stat.h>` do SDK (A):

| Flag | Valor | Comentário do header |
|---|---|---|
| `SF_DATALESS` (st_flags) | `0x40000000` | "file is dataless object"; read-only, sintética |
| `EF_MAY_SHARE_BLOCKS` | `0x01` | pode compartilhar blocos (clone) |
| `EF_NO_XATTRS` | `0x02` | nenhum xattr (útil para pular `listxattr`) |
| `EF_IS_SYNC_ROOT` | `0x04` | "sync root for iCloud" |
| `EF_IS_PURGEABLE` | `0x08` | "file is purgeable" |
| `EF_SHARES_ALL_BLOCKS` | `0x40` | clone completo |

As `EF_*` saem de `ATTR_CMNEXT_EXT_FLAGS` no `getattrlist[bulk]` com `FSOPT_ATTR_CMN_EXTENDED`.

### 1.1 `EF_IS_PURGEABLE` — o que é e quem marca

Medido em `~/Library/Mobile Documents` (73.572 arquivos), cruzando a flag com as chaves
`NSURLUbiquitousItem*` arquivo por arquivo (`cruzar_purgeable_sync.sh`):

| Flag | Estado de sync (status / enviado / excluído) | Arquivos | GiB alocados |
|---|---|---|---|
| purgeable | Current / enviado / não | 29.539 | 51,135 |
| **não** purgeable | Current / enviado / não | 1.205 | 0,043 |
| **não** purgeable | Current / **não enviado** / não | 162 | 0,003 |
| purgeable | Current / não enviado / não | 2 | 0,002 |
| **não** purgeable | sem status (dentro de pacote `.app`, `.pages` etc.) | 12.288 | 3,477 |
| **não** purgeable | excluído do sync (`node_modules`, `target`, `tmp`, `.nosync`) | 30.378 | 0,620 |

- Purgeable ⇒ enviado, em 99,99% dos bytes. Os 2 purgeable "não enviados" são arquivos
  modificados durante a varredura (corrida). (A)
- Enviados **sem** a flag: 1.205 arquivos pequenos (0,043 GiB); numa amostra de 300, 278 tinham
  mtime < 30 dias. Parece haver atraso ou critério extra antes de marcar. (M)
- Conteúdo de pacotes nunca recebe a flag, embora o pacote apareça como enviado (13 pacotes, todos
  `Current`/enviado). O File Provider declara `features: evict_packages` (despeja o pacote inteiro). (A medida, M interpretação)
- Quem marca: o `fileproviderd`. `fileproviderctl dump` mostra o campo `evictionUrgency`
  propagado do File Provider para o disco (`fp→fs: update-item(... diffs:evictionUrgency)`). (M:
  o campo é visível; o vínculo "evictionUrgency → EF_IS_PURGEABLE" não está documentado)

Mesmo padrão em `~/Documents` (Mesa e Documentos no iCloud está ligado): 133 purgeable = 133
enviados; 4 não enviados sem a flag. (A)

**Relação com "Otimizar armazenamento":** a flag existe com a otimização **desligada**. O
Eclectic Light descreve o modo desligado como File Provider replicado, em que nem o usuário nem o
macOS podem despejar ([2023-10-30](https://eclecticlight.co/2023/10/30/how-macos-sonoma-has-changed-icloud-even-more/),
[2024-03-11](https://eclecticlight.co/2024/03/11/icloud-drive-in-sonoma-optimise-mac-storage-or-not/)).
Nesta máquina o domínio está como `features: repl,syncTrash` (`fileproviderctl dump`) e o bird
reporta `purgeableSpace = 0; nonPurgeableSpace = 90214400` (`brctl dump`, `client_state`). Conclusão:
**a flag marca o que *seria* despejável; com a otimização desligada, ninguém usa isso para liberar
espaço.** (M: modo e números medidos; a regra interna do CacheDelete não foi lida em código)

Ao ligar a otimização, o File Provider percorre o iCloud Drive trocando o atributo `evictable` de
cada item (log `diffs:evictable`, Eclectic Light 2024-03-11). Não testado aqui (mudaria config).

### 1.2 Purgeable no volume inteiro

`fp_scan /System/Volumes/Data` (sem dedup de clone, sem atravessar montagens):
**73,49 GiB** purgeable em 30.913 arquivos, 68,75 GiB deles com `EF_MAY_SHARE_BLOCKS`.
O `walk_par` (com dedup) dá 52,03 GiB. A diferença vem dos clones: `Caches/CloudKit` tem 20,96 GiB
purgeable que são clones de arquivos de `Mobile Documents`. (A)

Onde está (sem dedup, GiB): `Mobile Documents/com~apple~CloudDocs` 31,89 · `Caches/CloudKit` 20,96 ·
`Mobile Documents/<WhatsApp>` 19,24 · `Daemon Containers/<uuid>` 0,59 · `Containers/com.apple.mediaanalysisd` 0,42 ·
`Documents` 0,06 · resto < 0,05 cada. (A)

**No CacheDelete**, o log de `deleted` (coletado pela Frente B às 14:45) mostra
`com.apple.FileProvider.cache-delete : 0`, `com.apple.cloudd.cache-delete : 0` e
`CACHE_DELETE_SHARED_PURGEABLE : 53742462383` (50,05 GiB). Esse último é da mesma ordem dos 52 GiB
de `EF_IS_PURGEABLE` deduplicados, mas as medições têm 1 h de diferença e a semântica da chave
não está documentada. **Não usar como igualdade.** (B)

### 1.3 `SF_DATALESS`

| Local | Arquivos dataless | Lógico | Pastas dataless |
|---|---|---|---|
| `~/Library/Mobile Documents` | 0 | 0 | 0 |
| `~/Desktop`, `~/Documents` | 0 | 0 | 0 |
| `CloudStorage/OneDrive-Personal` | 2.468 | 43,45 GiB | 139 |
| `CloudStorage/GoogleDrive-<conta>` | 334 | 8,35 GiB | 1 |

Todos os 2.802 dataless respondem `NotDownloaded` + enviado. Um arquivo dataless típico tem
`st_flags = 0x40000060` (`SF_DATALESS | UF_TRACKED | UF_COMPRESSED`), 0 blocos. (A)

### 1.4 `EF_IS_SYNC_ROOT`

20 pastas com a flag. As raízes de domínio: `~/Library/Mobile Documents`, `~/Desktop`,
`~/Documents` (as 3 raízes do domínio iCloud no `fileproviderctl dump`), a raiz do OneDrive e a do
Google Drive. As outras 15 estão **dentro** do iCloud Drive e são todas pastas excluídas do sync
(`target` ×7, `tmp` ×7, `node_modules.nosync` ×1). (A medida; M: "a flag marca raiz de sync e raiz de
subárvore ignorada")

### 1.5 xattrs de nuvem que existem de verdade

`listxattr` em todos os itens (pulando `EF_NO_XATTRS`), só nomes e tamanhos:

| xattr | Itens | Onde |
|---|---|---|
| `com.apple.fileprovider.dir#N` | 103 + 50 | pastas em Mobile Documents / CloudStorage |
| `com.apple.fileprovider.fpfs#P` | 72 + 7 | idem |
| `com.apple.fileprovider.before-bounce#PX` | 11 | iCloud (conflito de nome resolvido) |
| `com.apple.fileprovider.ignore#P` | 9 | iCloud (itens ignorados) |
| `com.apple.fileprovider.pinned#PX` | 6 + 3 | "Manter baixado" (bate com 3 `cp:keepDownloaded` no dump) |
| `com.apple.fileprovider.detached-link#P` | 2 | iCloud |
| `com.apple.fileprovider.trash`, `trash-put-back#PN` | 1 + 5 | lixeiras |
| `com.apple.file-provider-domain-id` | 2 | raízes em CloudStorage |
| `com.apple.clouddocs.private.trash-parent-bookmark#B` | 3 | pasta de migração do iCloud (27/03) |
| `com.google.drivefs.item-id#S`, `cse-target-item-id#S` | 392, 56 | Google Drive |
| `com.microsoft.OneDrive.Macbox.*` (`PhData`, `OriginalCtime`, `SyncRoot`…) | ~27.900 | `OneDrive.noindex` (cliente antigo) |

**Não existem** nesta máquina: `com.apple.icloud.*`, `com.apple.clouddocs.*` por arquivo (fora os 3
acima), nenhum xattr de status de upload ou de purgeable. **O estado de sync do FPFS fica no banco
do `fileproviderd`, não em xattr.** (A)

O header do FileProvider diz que o sistema põe e preserva xattrs em arquivos dataless
(`NSFileProviderItem.h`, bloco sobre extended attributes). (A)

---

## 2. Estado de sync sem baixar nada

| Método | Funciona sem root? | O que dá | Risco |
|---|---|---|---|
| `NSURLUbiquitousItemDownloadingStatusKey` | Sim | `Current` / `Downloaded` / `NotDownloaded` | Nenhum (metadado) |
| `NSURLUbiquitousItemIsUploadedKey` / `IsUploadingKey` | Sim | enviado / enviando | Nenhum |
| `NSURLUbiquitousItemHasUnresolvedConflictsKey` | Sim | 40 arquivos com conflito | Nenhum |
| `NSURLUbiquitousItemIsExcludedFromSyncKey` | Sim | 30.378 excluídos | Nenhum |
| Mesmas chaves em OneDrive/Google Drive | Sim | status e enviado; excluído sempre `0` | Nenhum |
| `NSFileProviderManager.getDomainsWithCompletionHandler` | **Não**: `-2001 ProviderNotFound` (subjacente `-2014 ApplicationExtensionNotFound`) | — | — |
| `NSFileProviderManager.getIdentifierForUserVisibleFile` | Sim | id do item + id do domínio | Nenhum |
| `brctl status` | Sim, 3,6 s | 112 containers, estado por container, itens "unclean" com tamanho (`sz:… (bytes)`) | Nenhum. Nomes de pasta aparecem em claro |
| `brctl dump -i` | Sim, 0,9 s | `optimize storage`, `purgeableSpace`, `availableQuota`, `fsType: FPFS` | Nenhum |
| `brctl dump` (com itens) | Não rodado | banco inteiro do CloudDocs | Leitura, mas enorme e com nomes |
| `brctl quota` | Sim | bytes livres na conta | Nenhum |
| `brctl accounts` | Sim, saída vazia | — | — |
| `fileproviderctl dump -l` | Sim, 2,4 s, 9 MB | 4 provedores (iCloud, OneDrive, Google Drive, Fotos), domínios, raízes, `features`, progresso de upload, política por item (`cp:`) | Nenhum. Nomes ofuscados parcialmente |
| `fileproviderctl domain list` | **Não existe** nesta versão (help só tem `dump`, `diagnose`, `evaluate`, `check/repair`, `obfuscate`) | — | — |
| `fileproviderctl check` | Não rodado | FPCK | Parente de `repair`; evitar |
| `fileproviderctl evaluate`, `brctl evict/download` | **Não rodados** (proibidos) | — | Materializa/despeja |

Chaves de NSURL também foram lidas com `setiopolicy_np(...OFF)` e sem descer em pasta dataless
(`ubiq_keys.swift`); 73.574 arquivos em 84 s, sem erro. (A)

**Conteúdo pendente de upload, medido agora:** `brctl status` listou 2 itens (1 `needs-upload`
de 1.140.476 bytes, 1 `needs-sync-up`). `fileproviderctl dump` mostrou upload a 78% (9.416.632 de
12.064.419 bytes). As chaves NSURL deram 4 arquivos `IsUploading`. São três fotos em instantes
diferentes; a fonte por arquivo mais estável é a das chaves NSURL. (A)

**Política de conteúdo por item** (`fileproviderctl dump`): `cp:inherited` 2.987, `cp:lazy` 3.025,
`cp:evict` 460 (pastas de apps desinstalados), `cp:keepDownloaded` 3. Mapeia para
`NSFileProviderContentPolicy` do SDK: `Inherited`, `DownloadLazily` ("allow eviction on low disk
pressure"), `DownloadLazilyAndEvictOnRemoteUpdate`, `DownloadEagerlyAndKeepDownloaded` ("prevent
eviction") — `NSFileProviderItem.h`. (A; o dump foi truncado com `-l`, então as contagens são parciais)

---

## 3. "Otimizar armazenamento" e CacheDelete

**Como detectar, sem GUI (A):**
- `defaults read com.apple.bird optimize-storage` → `0`.
- `brctl dump -i` → seção `system`: `+ optimize storage: disabled`.
- `fileproviderctl dump` → domínio iCloud com `features: repl,…` (replicado = otimização desligada,
  segundo o Eclectic Light).

**CacheDelete para iCloud (A, com dados da Frente B):**
- Serviços registrados: `com.apple.FileProvider.cache-delete` (`/System/Library/CacheDelete/com.apple.FileProvider.CacheDelete.plist`,
  `CACHE_DELETE_USER_AGENT = true`), `com.apple.cloudd.cache-delete` (serviços `PURGE`, `PERIODIC`)
  e `com.apple.cloudd.system.cache-delete`. **Não há** um serviço `com.apple.clouddocs.*` próprio.
- Às 14:45: FileProvider → `returned 0 an invalid CACHE_DELETE_AMOUNT, using zero`; cloudd → 0.
- bird: `purgeableSpace = 0`, `purgeableSpaceDate = 2026-09-27 00:35`.

Leitura: com a otimização desligada, o File Provider não oferece nada ao CacheDelete, e os 52 GiB
com `EF_IS_PURGEABLE` **não entram** no purgeable do volume (3,3 GiB medidos na pesquisa de 26/09). (M)

Não medido: o que o File Provider ofereceria com a otimização ligada. Teria de mudar a config.

---

## 4. Medição local agora

| Item | Alocado | Purgeable | Dataless | Fonte |
|---|---|---|---|---|
| `Mobile Documents` inteiro | 55,28 GiB (55,23 com dedup) | 51,14 GiB | 0 | `fp_scan`, `walk_par` |
| ↳ `com~apple~CloudDocs` | 36,03 | 31,89 | 0 | |
| ↳ WhatsApp | 19,24 | 19,24 | 0 | |
| `~/Desktop` + `~/Documents` (no iCloud) | 0,06 | 0,06 | 0 | |
| `CloudStorage/OneDrive-Personal` | 0,00 | 0 | 43,45 GiB lógicos | |
| `CloudStorage/GoogleDrive-<conta>` | 0,00 | 0 | 8,35 GiB lógicos | |
| `CloudStorage/iCloudDrive-iCloudDrive (27-03-26 21:21)` | 2,43 | 0 | 0 | sem status NSURL: fora de domínio |
| `Caches/CloudKit` | 21,21 (clones) | 20,96 | 0 | |

- Baixado vs dataless em `Mobile Documents`: **100% baixado** (0 dataless), coerente com a
  otimização desligada. (A)
- `PRIVATESIZE` = 0 em todo `Mobile Documents`: logo após o snapshot do backup, todo bloco está
  preso em snapshot. Não é propriedade do iCloud. (A medida, M causa)
- **Comparação com o painel "iCloud Drive":** não medida (sem GUI). A auditoria de 09/08 anotou
  "201,3 MB" no painel contra 35,6 GiB em `com~apple~CloudDocs`. Fica para a Frente A. (—)

---

## 5. OneDrive, Google Drive, Dropbox: cache fora de CloudStorage

| Caminho | Alocado | O que é | Conf. |
|---|---|---|---|
| `Group Containers/UBF8T346G9.OneDriveSyncClientSuite/OneDrive.noindex/OneDrive` | 1,36 GiB, 26.345 arquivos, **nenhum dataless**, sem status NSURL, xattrs `com.microsoft.OneDrive.Macbox.*` | Pasta do cliente antigo (pré-File Provider). Dados reais | A medida; M "legado" |
| `Group Containers/UBF8T346G9.OneDriveStandaloneSuite` | 0,01 GiB | 2ª instalação | A |
| `Containers/com.microsoft.OneDrive.FileProvider`, `.FinderSync` | 0,00 | extensões | A |
| `Application Support/Google/DriveFS/<id>` | 0,42 GiB | bancos SQLite (`mirror_metadata_sqlite.db` 146 MB + WAL 147 MB, `metadata_sqlite_db` 98 MB, `mirror_sqlite.db` 41 MB) | A |
| `Application Support/Google/DriveFS/<id>/content_cache` | 0,00 | cache de conteúdo do modo stream (vazio agora) | A |
| `Application Support/Google/DriveFS/Logs` | 0,39 GiB | logs | A |
| `~/My Drive` | 6,00 GiB, 377 arquivos, 0 dataless, sem status NSURL | pasta do **modo espelho** (existem bancos `mirror_*`), fora do File Provider | M |
| `Group Containers/EQHXZ8M8AV.group.com.google.drivefs`, `Containers/com.google.drivefs.*` | desprezível | extensões | A |
| `~/Dropbox`, `~/.dropbox` | vazios | Dropbox não instalado | A |
| `Application Support/FileProvider` | 0,21 GiB | bancos do `fileproviderd` | A |
| `Application Support/CloudDocs` | 0,10 GiB | banco do bird | A |

**Como separar "cache" de "arquivo baixado":** no File Provider moderno o arquivo baixado mora na
própria árvore de `CloudStorage/<domínio>` (flag dataless 0, blocos alocados); cache do provedor é o
que fica fora (`content_cache`, bancos, logs). O Google Drive em modo stream deveria guardar conteúdo
em `content_cache`; aqui está vazio. Processos: OneDrive e Google Drive **não estão rodando**; bird,
cloudd e fileproviderd estão. (A)

---

## 6. Caches do CloudKit (`MMCS/ClonedFiles`)

Medição exata por `ATTR_CMNEXT_CLONEID` (o mesmo valor em todos os membros de um grupo de clone
completo) e `ATTR_CMNEXT_CLONE_REFCNT`:

| Grupo | Arquivos no cache | GiB | Original |
|---|---|---|---|
| mesmo `CLONEID` de arquivo do WhatsApp | 7 | 19,20 | `Mobile Documents/<WhatsApp>/.../backup/*.tar` |
| mesmo `CLONEID` de arquivo do CloudDocs | 3 | 1,63 | achado por tamanho exato e confirmado por `CLONEID` |
| sem clone | 2 | 0,23 | `ChunkStoreDatabase`, asset |

Todos os clones: `MAY_SHARE, SHARES_ALL, PURGEABLE`, refcount 2. Mtimes de hoje (11:58–14:47); 549 de
732 arquivos com mtime < 24 h. **Confirma: são clones de uploads recentes.** Economia real ao apagar ≈
0, porque o original segura os blocos. (A)

Regra de medição para o projeto: agrupar por `CLONEID` quando `EF_SHARES_ALL_BLOCKS` estiver setado
(o `walk_par` já faz). Casamento por tamanho (runbook atual) só serve para achar o caminho do original.

---

## 7. `setiopolicy_np` e `SF_DATALESS` no código

Fonte: `apple-oss-distributions/xnu`, branch `main` commit `f6217f8` (tag mais nova `xnu-12377.121.6`).

1. **Quem decide materializar:** `vfs_context_dataless_materialization_is_prevented()`
   (`bsd/vfs/vfs_syscalls.c` ~12465). Ordem: contexto do kernel → `EDEADLK`; entitlement de
   "dataless manipulator" → `EJUSTRETURN`; flag da thread `UT_NSPACE_NODATALESSFAULTS` → `EDEADLK`;
   `UT_NSPACE_FORCEDATALESSFAULTS` → materializa; bit do processo
   `P_VFS_IOPOLICY_MATERIALIZE_DATALESS_FILES` → materializa; senão `EDEADLK`. (A)
   **É isso que dá `EDEADLK` (errno 11) na hora com política OFF**, sem falar com o provedor.
2. **Default:** `set_proc_materialize_policy()` (`bsd/kern/kern_resource.c` ~2537) troca DEFAULT
   por OFF, e o man page diz o mesmo. Mas o man page também diz "New processes inherit the policy of
   their parent process", e medido aqui: `getiopolicy_np(…, IOPOL_SCOPE_PROCESS)` = **2 (ON)** num
   processo comum. Na prática, todo processo do usuário materializa. (A)
3. **Por que `ETIMEDOUT`:** com a política ON, o kernel manda o pedido ao resolvedor
   (`filecoordinationd`, comentário "Namespace Resolver Up-call Mechanism", ~11814) e espera a
   resposta. `ETIMEDOUT` sai quando não há porta do resolvedor, quando o envio falha ou quando o
   próprio resolvedor responde com esse erro (~12057, ~12705, ~12828). Aqui OneDrive e Google Drive
   estão fechados, então o provedor não responde e volta `ETIMEDOUT`. (A código; M causa. Com o app
   aberto, listar a pasta **materializaria a listagem**)
4. **O que dispara:** o comentário do xnu lista "read, write, lookup-in, etc." em arquivo ou pasta
   dataless. Quem chama a up-call é o sistema de arquivos (APFS, código fechado), então a lista exata
   de operações não é verificável no xnu. (A para o comentário, B para a lista completa)
5. **`stat`/`getattrlist` em arquivo dataless:** medido com política **ON** num arquivo dataless do
   OneDrive: `getattrlist` voltou em 0,17 s, `SF_DATALESS` continuou setada, 0 blocos. Metadado é
   local. (A para este caso; o Eclectic Light diz o mesmo sobre xattrs e ícones de arquivos despejados)
6. Outros ganchos no xnu: `rename` de dataless devolve `EDATALESS` e chama `vfs_materialize_reparent`
   (~9930); `unlinkat(AT_REMOVEDIR_DATALESS)` remove pasta dataless sem materializar (~6750). Não usar. (A)
7. Existe também `sysctl vfs.nspace.prevent_materialization` e `thread_prevent_materialization`
   (~12359, ~12384), equivalentes por sysctl. `setiopolicy_np` basta. (A)

**Regra que se mantém:** `setiopolicy_np(3, 0, 1)` no início **e** não descer em pasta com
`SF_DATALESS`. A política garante `EDEADLK` em vez de download; o não descer evita até o pedido.
`NSURL resourceValues` e `listxattr` rodaram com a política OFF sem erro nenhum.

---

## 8. O que o CLI deve fazer com isso

1. Mostrar `EF_IS_PURGEABLE` de iCloud como **"já está no iCloud (enviado)"**, nunca como espaço
   liberável. O liberável vem do CacheDelete (Frente B), que aqui dá 0 para iCloud.
2. Mostrar se "Otimizar armazenamento" está ligado (`com.apple.bird optimize-storage` e
   `brctl dump -i`). Se desligado, dizer: "ligar permitiria ao macOS despejar até X GiB já enviados"
   (X = soma medida de `EF_IS_PURGEABLE` sem pacotes, deduplicada). É a única frase de "potencial"
   que continua sendo medição.
3. `CloudStorage`: somar dataless à parte (0 local) e contar pastas dataless não abertas.
4. Pastas fora de domínio (sem status NSURL) dentro de `CloudStorage`: sinalizar como resto de
   migração, com o tamanho medido.
5. `ClonedFiles`: agrupar por `CLONEID` e mostrar "clone de <arquivo do iCloud>, libera ~0".
6. Pendente de upload: por arquivo via `NSURLUbiquitousItemIsUploadedKey = false` (fora dos excluídos).

---

## 9. Programas (`research/experimentos/frenteF/`)

| Arquivo | O que faz |
|---|---|
| `src/bin/fp_scan.rs` | Varredura serial com `getattrlistbulk`, política OFF, não entra em pasta dataless nem em outro volume. Por balde: alocado, purgeable (e quanto com `MAY_SHARE`), dataless, sync roots. `--xattr` = histograma de nomes; `--list np,p,dl` = lista por estado (stderr, uso local). Sem dedup de clone (use `walk_par` para isso) |
| `ubiq_keys.swift` | TSV com as chaves `NSURLUbiquitousItem*` por arquivo, política OFF, pula pastas dataless |
| `cruzar_purgeable_sync.sh` | Junta os dois acima: tabela flag × estado de sync (a tabela do §1.1) |
| `fpmanager_teste.swift` | Mostra o que `NSFileProviderManager` responde a um CLI sem entitlement |
| `iopolicy_atual.c` | Imprime a política de materialização herdada pelo processo |

Compilar: `CARGO_TARGET_DIR=<fora do iCloud> cargo build --release --offline`; `swiftc -O ubiq_keys.swift -o ubiq_keys`.

---

## 10. Ocorrência durante os testes

A primeira versão do `fp_scan` não checava o dispositivo e, rodada em `/System/Volumes/Data`,
atravessou pontos de montagem por ~10 min (entre eles os snapshots locais montados e o volume SMB
do Time Machine) antes de eu interromper. Só leitura de metadados, nada escrito, mas concorreu com o
backup. Corrigido (`ATTR_CMN_DEVID`; 59 montagens puladas na rodada seguinte, 78 s). O `walk_par` já
tinha essa proteção.

## 11. Em aberto

- Comportamento com "Otimizar armazenamento" **ligado**: quanto o `FileProvider.cache-delete`
  oferece, e se ele usa exatamente o conjunto `EF_IS_PURGEABLE`. Exige mudar a config: só com o dono.
- Semântica de `CACHE_DELETE_SHARED_PURGEABLE` e `CACHE_DELETE_TOTAL_FSPURGEABLE` (Frente B).
- Valor do painel "iCloud Drive" contra estas medidas (Frente A).
- Se `OneDrive.noindex` e `~/My Drive` ainda são sincronizados: os apps estavam fechados; confirmar
  com eles abertos antes de rotular como resto.

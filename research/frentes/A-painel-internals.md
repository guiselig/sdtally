# Frente A — Como o painel de Armazenamento calcula as categorias — 2026-09-27

Máquina: macOS 27.0 (26A5416b), Apple Silicon, usuário admin, sem sudo. Método: engenharia
reversa estática (`dyld_info`, `strings`, `codesign`, `plutil`), introspecção em runtime
(programas ObjC em scratchpad com `dlopen` + `objc_msgSend`) e leitura passiva de `log show`.
Confiança por item: **A** alta (medido/visto aqui), **M** média (inferência forte), **B** baixa.

---

## Resumo

1. Quem calcula é o **`StorageManagementService`** (LaunchAgent do `StorageManagement.framework`),
   acionado pela extensão de Ajustes `com.apple.settings.Storage` (`Storage.appex`). Não é
   `spaceattributiond` (não roda no macOS), nem `diskspaced` (só notificação de disco cheio).
2. **Fórmula exata, verificada em 74 de 74 blocos de log:** `Dados do Sistema = Usado − macOS − Σ(categorias das extensões)`,
   com `Usado = Total − Disponível − Outros Volumes` e `Disponível = espaço livre + purgeable do CacheDelete`.
3. O serviço **registra no log unificado, em nível Default e sem redação**, cada categoria em bytes,
   inclusive "Other" (= `DISK_OTHER` = "Dados do Sistema"). `log show` sem sudo lê isso por dias.
4. Consultar o serviço direto por XPC **não dá**: exige o entitlement restrito `com.apple.storage-data`;
   o AMFI mata binário ad-hoc que o declare. Root não muda isso.
5. O purgeable **por serviço** do CacheDelete (inclusive `com.apple.TMCacheDelete` = snapshots do TM)
   é legível sem entitlement por `CacheDeleteCopyItemizedPurgeableSpaceWithInfo`, e a soma bate ao byte.

---

## 1. Achados

### 1.1 Peças e papéis

| Peça | Caminho | Papel | Conf. |
|---|---|---|---|
| `Storage.appex` (`com.apple.settings.Storage`) | `/System/Library/ExtensionKit/Extensions/` | UI SwiftUI do painel (ponto `com.apple.Settings.extension.ui`). Mede volumes por conta própria (DiskManagement, NSURL, CacheDelete) e recebe as categorias do serviço | A |
| **`StorageManagementService`** | `StorageManagement.framework/PlugIns/`, LaunchAgent `com.apple.StorageManagement.Service` | Calcula as categorias: carrega extensões, soma, deriva "System Data". Sai 10 s depois que o painel fecha | A |
| Extensões `com.apple.storagemanagement` | `StorageManagement.framework/PlugIns/*.appex` (Applications, Developer, Trash, OtherUsers, iOSFiles, GarageBand, AppleInternal) + extensões de apps (Music, Photos, Messages, Podcasts, TV, iCloud Drive, Livros, Mail) | Cada uma informa `totalSpaceUsed` da sua categoria | A |
| `SPInfoDocumentsStorageUsageReporter` (dentro do serviço) | — | Categoria Documentos: tamanho do home excluindo caminhos gerenciados pelas outras extensões. Último valor salvo em `SPLastDocumentsSize` | A |
| `deleted` (CacheDelete) | `CacheDelete.framework/deleted`, LaunchAgent `com.apple.cache_delete` | Espaço disponível e purgeable, total e por serviço | A |
| `diskspaced` | `StorageManagement.framework/Resources/diskspaced` | Só alerta de pouco espaço (`STORAGE_CRITICALLY_LOW`) e abre o painel. Não calcula categorias | A |
| `STMUIHelper` | `StorageManagement.framework/Resources/STMUIHelper.app` | Diálogos de confirmação | M |
| `SpaceAttribution.framework` + `spaceattributiond` | `PrivateFrameworks/SpaceAttribution.framework` | Atribuição estilo iOS (`com.apple.fakeapp.SystemData`, `/var/db/spaceattribution/*.db`). No macOS **não há** plist de launchd nem `/var/db/spaceattribution`. O `Storage.appex` importa `SAAppSizer`/`SAVolumeSizer`, mas o caminho "SpaceForce" parece atrás de feature flag desligada | A (ausência do daemon) / M (flag) |
| `StorageKit` / `storagekitd` | `/usr/libexec/storagekitd` | Não aparece no fluxo do painel (nenhuma linha no log das sessões) | M |
| `DiskSpaceDiagnostics.framework` | `XPCServices/FilesystemMetadataSnapshotService.xpc` | Diagnóstico (snapshot de metadados para sysdiagnose). Fora do fluxo | M |
| `StorageUI.framework` | — | Componentes do gauge (`GaugeView`, `CategoryIdentifier`) | A |

### 1.2 Como "Dados do Sistema" é derivado (medido)

As strings do serviço ligam as chaves aos rótulos (`Localizable.loctable` do framework):
`DISK_OTHER` = "System Data" / "Dados do Sistema", `DISK_SYSTEM` = "macOS",
`DISK_OTHER_VOLS` = "Outros Volumes", `DISK_CALC` = "Calculando…".

Cada recálculo grava no log (`subsystem com.apple.storage`, categoria `log`, prefixo
`StorageLogInvestigation`):

```
totalSpace, availSpace, purgeSpace, otherVols, Used, System, <id da extensão>: <bytes> ..., Other
```

Último bloco completo (2026-09-26 21:58:02):

| Chave | Bytes | Significado |
|---|---|---|
| totalSpace | 494 384 795 648 | capacidade do container |
| availSpace | 125 307 230 850 | livre + purgeable (CacheDelete, classe "Essential") |
| purgeSpace | 3 328 252 546 | purgeable total do CacheDelete |
| otherVols | 604 651 520 | "Outros Volumes" (origem exata incerta, ver §5) |
| Used | 368 472 913 278 | = total − avail − otherVols (exato) |
| System (macOS) | 23 023 312 896 | categoria macOS |
| Music | 36 429 056 548 | |
| Messages | 214 752 | |
| OtherUsers | 884 736 | |
| Mail | 849 178 624 | |
| Applications | 75 530 461 184 | |
| Photos | 10 169 778 544 | |
| iCloud Drive (CloudDocs) | 252 719 104 | |
| Developer | 2 050 179 072 | |
| Documents | 41 252 458 496 | |
| **Other (Dados do Sistema)** | **178 914 669 322** | = Used − System − Σ extensões (**diferença 0**) |

Verificação: script Python sobre os 74 blocos com `totalSpace > 0` dos últimos 7 dias:
`Used − System − Σext == Other` e `total − avail − otherVols == Used` em **74/74**, erro 0 byte. (A)

Consequências diretas da fórmula (A, por aritmética):

- Purgeable **não** entra em Dados do Sistema (vai em Disponível). Snapshots do Time Machine
  entram só na parte que o CacheDelete **não** declara purgeable.
- Tudo que ocupa o container e não é categoria vira Dados do Sistema: VM (swap), Recovery,
  Preboot além do que o "macOS" contar, snapshots de terceiros, `~/Library` não reivindicado,
  e **qualquer categoria cuja extensão ainda não respondeu**.
- Extensão que responde "desconhecido" (`totalSpaceUsedDidChange : -9223372036854775808`,
  isto é, `INT64_MIN` = `STMExtensionTotalSpaceUsedUnknown`) fica fora da soma. Aconteceu com
  Lixo e Developer em parte da sessão. O tamanho dela cai em Dados do Sistema.
- O número é **transitório** enquanto o painel carrega: na mesma sessão "Other" foi de 314 GB
  (21:35:12, nada respondido) a 225 GB, e de 253 GB a 178,9 GB (21:57–21:58). Só o bloco em que
  todas as extensões já responderam representa o painel estável.

### 1.3 O que o CacheDelete informa e como

- `availSpace` do serviço = `CacheDeleteCopyAvailableSpaceForVolume` na classe Essential:
  log do `Storage.appex` às 21:58:00 diz `125308603010 = freespace 121980350464 + purgeableSpace: 3328252546`. (A)
- A mesma conta é o que o Foundation expõe em `volumeAvailableCapacityForImportantUsage`:
  hoje, 102 700 892 160 (livre) + 28 154 570 158 (purgeable) = 130 855 462 318, contra
  130 855 421 358 do NSURL (diferença de 41 KB entre chamadas). (A)
- **Purgeable por serviço** sem entitlement: `CacheDeleteCopyItemizedPurgeableSpaceWithInfo({CACHE_DELETE_VOLUME: "/System/Volumes/Data"})`
  devolve cada serviço em bytes; a soma dos serviços = `CACHE_DELETE_TOTAL_AVAILABLE` =
  28 154 570 158, **ao byte**. Maior item hoje: `com.apple.TMCacheDelete` = 24 418 709 504
  (snapshots locais do TM). (A)
- A queda de 52,5 GB → 3,3 GB de purgeable entre 21:57:39 e 21:57:44 foi reclassificação do
  CacheDelete, não arquivo apagado; o `Used` subiu o mesmo tanto e "Other" pulou para 253 GB. (A para
  o fato; M para a causa exata, provavelmente `TMCacheDelete`.)

### 1.4 O `Storage.appex` (painel novo do macOS 27) também mede por conta própria

O gauge desenhado vem do `Storage.appex`, que loga (categoria `StorageUI`, nível Error):
`Sum of all categories and available space doe not match capacity ... Sum of categories: X, available: Y, capacity: Z`.

- Às 21:35 o appex e o serviço concordam (categorias 415,58 GB ≈ `Used` 414,92 GB). (A)
- Às 21:57–21:58 divergem em ~50 GB: appex `available` = 174,7–176,7 GB, serviço `availSpace` =
  125,3–126,4 GB. O appex continuou contando como disponível os ~49 GB que o CacheDelete deixou de
  declarar purgeable. (A para a divergência; B para a causa.)
- Logo, **o número que o usuário vê pode não ser o "Other" do serviço** quando os dois divergem. Se
  as categorias do appex forem "extensões + macOS + Dados do Sistema" (as "17 storage items"
  recebidas), Dados do Sistema exibido = `Sum of categories − System − Σext`; às 21:58:00 daria
  ~129 GB, e não 178,9 GB. Hipótese, não verificada (B).

### 1.5 Categorias possíveis

Do serviço: `com.apple.STMExtension.{Applications, Developer, Documents, Trash, OtherUsers, iOSFiles, GarageBand, AppleInternal, Mail, CloudFiles}`,
`com.apple.Music.MusicStorageExtension`, `com.apple.Photos.StorageManagementExtension`,
`com.apple.messages.StorageManagementExtension`, `com.apple.podcasts.MacPodcastsStorageExtension`,
`com.apple.TV.TVStorageExtension`, `com.apple.CloudDocsDaemon.StorageManagement`,
`com.apple.iBooksX.DiskSpaceEfficiency`, mais `DISK_SYSTEM` (macOS) e `DISK_OTHER` (Dados do Sistema).
Do appex/StorageUI: `SACategoryIdentifier{System, SystemData, Documents, Trash, iCloudDrive, iOSFiles, OtherUsers, MusicCreation, SoftwareUpdate, LocalStorage, AppleInternal, Ringtones}`
e um detalhamento de macOS com "Apple Intelligence" lido de
`/System/Library/AssetsV2/com_apple_MobileAsset_UAF_FM_{GenerativeModels,Visual}/`. (A para a
existência das strings; M para o uso.)

---

## 2. Evidências (comandos e saídas resumidas)

Scratchpad: `/private/tmp/claude-501/.../scratchpad/frenteA/` (dumps `*.exports`, `*.cstr`,
`*.strings`, `log_stm.txt`, programas `introspect.m`, `probe.m`, `telem.m`, `itemized.m`).

| # | Comando | Saída relevante |
|---|---|---|
| 1 | `ls /System/Library/PrivateFrameworks \| grep -iE 'storage\|space\|cachedelete\|delete\|attribut'` | CacheDelete, DiskSpaceDiagnostics, SpaceAttribution, StorageKit, StorageManagement, StorageManagementService, StorageUI, StorageContainersPrivate… |
| 2 | `ls /System/Library/ExtensionKit/Extensions \| grep -i storage` | Storage.appex, StorageDESDXC.appex, StorageDESSD.appex (os dois últimos são diagnóstico de SSD), StorageSettingsIntentsExtension.appex |
| 3 | `plutil` nos plists de launchd | `com.apple.StorageManagement.Service` → `.../PlugIns/StorageManagementService`; `com.apple.diskspaced`; `com.apple.cache_delete` → `deleted`; `com.apple.deleted_helper` (root); `com.apple.storagekitd`. Nenhum plist para `spaceattributiond` (o framework referencia `/System/Library/LaunchDaemons/com.apple.spaceattributiond.plist`, que não existe) |
| 4 | `ls /var/db/spaceattribution` | `No such file or directory` |
| 5 | `strings StorageManagementService` | `Disk: TOT: %@ / USE: %@ / AVL: %@ / PRG: %@ / OTH: %@ / SYS: %@`, `StorageLogInvestigation`, `totalSpace: %@`…`Other: %@`, `Client %d is missing entitlements required to use this service`, `com.apple.storage-data`, `SPLastDocumentsSize` |
| 6 | `/usr/bin/log show --last 7d --predicate 'process == "StorageManagementService" OR subsystem == "com.apple.storage" OR process == "Storage" ...'` | 25 874 linhas; sessões do painel em 2026-09-20 15–16h e 2026-09-26 21h; 1 488 linhas `StorageLogInvestigation`; linhas `Disk: TOT:` (ver §1.2). Atenção: no zsh, `log` é builtin; usar `/usr/bin/log` |
| 7 | `plutil -p .../StorageManagement.framework/Resources/Localizable.loctable` | `DISK_OTHER` → "System Data" / pt_BR "Dados do Sistema"; `DISK_SYSTEM` → "macOS" |
| 8 | `plutil -p ~/Library/Preferences/com.apple.StorageManagement.Service.plist` | `SPLastDocumentsSize = 41252458496` (igual ao Documents do log) |
| 9 | `codesign -d --entitlements - StorageManagementService` / `Storage.appex` | Serviço: TCC FDA embutido (`kTCCServiceSystemPolicyAllFiles`) etc. Appex: `com.apple.storage-data = true`, sem sandbox |
| 10 | `./telem` (chama `-[STMTelemetryController getTelemetryWithCompletionHandler:]`) | Log do serviço: `Client 78427 is missing entitlements required to use this service`; conexão cancelada |
| 11 | `codesign -s - --entitlements ent.plist telem2` (com `com.apple.storage-data`) e executar | Exit 137; amfid: `The file is adhoc signed but contains restricted entitlements` |
| 12 | `./probe` | statfs Data livre = 102 701 944 832; NSURL ImportantUsage = 130 855 421 358; `CacheDeleteCopyPurgeableSpaceWithInfo` → `CACHE_DELETE_AMOUNT = 28154570158` (funciona sem entitlement, `CACHE_DELETE_CACHED_RESULT = 1`); `+[STMFreeSpaceRequest freeSpaceOnVolume:]` = 102 700 892 160, `recoverableSpaceOnVolume:` = 28 154 570 158; `+[SAVolumeSizer computeSizeOfVolumeAtURL:completionHandler:]` → info nil, erro nil; `+[SASupport getDiskUsed]` = 391 683 928 064 (≈ "Capacity In Use By Volumes" do `diskutil apfs list`) |
| 13 | `./itemized` | Lista de ~40 serviços com bytes; soma = `CACHE_DELETE_TOTAL_AVAILABLE` = 28 154 570 158; `com.apple.TMCacheDelete = 24418709504` |
| 14 | `ls /System/Library/FeatureFlags/Domain/StorageManagement.plist` | Dicionário vazio (flags como "SpaceForce" no padrão) |
| 15 | Web | Apple não documenta a conta; Oakley descreve System Data como resto e diz que não inclui o volume de sistema ([explainer 2025-12-20](https://eclecticlight.co/2025/12/20/explainer-storage-settings/), [2025-02-11](https://eclecticlight.co/2025/02/11/what-is-system-data-in-storage-settings/)). Man page `StorageManagementService(8)`: "daemon backing the Storage section" ([manp.gs](https://manp.gs/mac/8/StorageManagementService)). Nenhuma fonte pública encontrada sobre `StorageLogInvestigation`, `STMTelemetryController` ou a fórmula exata |

---

## 3. O que é mensurável e como

| Número | Como obter | Precisa de | Conf. |
|---|---|---|---|
| **Dados do Sistema do painel, e cada categoria, em bytes** | `/usr/bin/log show --last Nd --style ndjson --predicate 'subsystem == "com.apple.storage" AND eventMessage BEGINSWITH "StorageLogInvestigation"'` e pegar o último bloco completo. Conferir `Used − System − Σext == Other` antes de exibir | Usuário **admin** (o `log` recusa leitura a não-admin); o painel precisa ter sido aberto dentro da retenção do log (7 dias aqui cobriram as duas sessões) | A |
| Totais do painel numa linha | Mesma consulta com `eventMessage BEGINSWITH "Disk: TOT:"` | idem | A |
| Gauge efetivamente desenhado pelo appex | `subsystem == "com.apple.settings.Storage" AND eventMessage CONTAINS "Sum of all categories"` (só aparece quando a soma não fecha) | idem | A (existência) |
| Documentos (último cálculo) | `defaults read com.apple.StorageManagement.Service SPLastDocumentsSize` | nada | A |
| Disponível do jeito do painel | `volumeAvailableCapacityForImportantUsage` (API pública) ou `statfs` + `CacheDeleteCopyPurgeableSpaceWithInfo` | nada | A |
| Purgeable por serviço (snapshots TM, Spotlight, caches de apps, fspurgeable) | `dlopen` CacheDelete + `CacheDeleteCopyItemizedPurgeableSpaceWithInfo` | nada (sem sandbox; sem entitlement) | A |
| Uso por volume do container | `diskutil apfs list` / `+[SASupport getDiskUsed]` | nada | A |
| Recalcular o painel ao vivo | Não sem o painel: a única entrada é o XPC com entitlement. Dá para **observar** com `/usr/bin/log stream --predicate 'subsystem == "com.apple.storage"'` enquanto o usuário abre o painel | admin | A |

## 4. O que não é mensurável (por um processo de terceiro)

| Tentativa | Resultado | Conf. |
|---|---|---|
| Pedir as categorias ao `StorageManagementService` (XPC, `STMTelemetryController`) | Rejeitado: falta `com.apple.storage-data` | A |
| Assinar ad-hoc com esse entitlement | AMFI mata (restricted entitlement). Com SIP ligado não há saída; root não ajuda, porque a checagem é de assinatura | A |
| `spaceattributiond` / `AppSizerCache.db` | Não existem no macOS | A |
| `SAVolumeSizer` in-process | Devolve nil sem erro | A |
| Ler um cache/plist com as categorias | Só `SPLastDocumentsSize`. Nada em `~/Library/Caches`, `$DARWIN_USER_CACHE_DIR/com.apple.settings.Storage` (só cache Metal), container `com.apple.settings.Storage` (vazio) | A |
| Dados do Sistema do painel sem o painel ter rodado | Impossível pela fórmula: depende das extensões, que só rodam dentro do serviço | A |

## 5. Incertezas

- **Origem de `System` (macOS) = 23 023 312 896.** Hoje System (12 648 747 008) + Preboot
  (10 818 850 816) = 23 467 597 824, 444 MB acima. Pode ser outro recorte ou Preboot que mudou desde
  ontem. Strings sugerem `setSystemBytes:` e `Associating preboot size %lld with disk %@`. (B)
- **Origem de `otherVols` ≈ 604 MB** (oscila na casa das centenas de KB durante a sessão). Não bate
  com nenhum volume do `diskutil` atual (Update ≈ 80 MB, Recovery 2,95 GB). (B)
- **Qual "System Data" o usuário viu** quando appex e serviço divergem (§1.4). (B)
- Se o caminho "SpaceForce" (`SAVolumeSizer`) liga em alguma condição no macOS 27. (M para desligado)
- Retenção: as linhas são nível Default no arquivo persistente; a janela real depende do volume
  de log da máquina. Aqui 7 dias funcionaram. (M)
- `log show` exige admin (comportamento conhecido do `log`; usuário atual é admin, não testei com
  não-admin). (M)
- Tudo isto é macOS 27.0 beta/primeira build (26A5416b); strings de log podem mudar. (A como risco)

## 6. Próximos experimentos

1. **Fechar a fórmula ao vivo:** `/usr/bin/log stream --predicate 'subsystem == "com.apple.storage" OR subsystem == "com.apple.settings.Storage"' --level debug`
   enquanto o dono abre o painel manualmente; comparar com screenshot do gauge para decidir qual
   número (serviço ou appex) é exibido. Resolve §1.4 e §5 (System/otherVols), porque o appex tem
   linhas debug (`Caching volume sizes`, `System size set to %lld`, `Volume size at mount point`).
2. Implementar no CLI um leitor `log show --style ndjson` que devolve o último bloco completo,
   valida a identidade aritmética e mostra data/hora do cálculo ("painel calculado em …").
   Se não houver bloco na retenção, dizer isso, sem inventar.
3. Adicionar ao relatório o purgeable itemizado do CacheDelete (via `dlopen`; se a regra for evitar
   framework privado, cair para `volumeAvailableCapacityForImportantUsage − statfs livre`, que dá só o total).
4. Com o bloco do log em mãos, a ferramenta passa a **explicar o "Other" do painel** com números
   próprios: Dados do Sistema do painel − (soma medida das fatias que a varredura atribui) = resíduo
   não atribuído, exibido como tal.
5. Disassemblar `StorageManagementService` (`dyld_info -disassemble`) em torno de
   `updateSystemSidebarItemSubtitle` e `setSystemBytes:` para achar de onde vem `System`.
6. Testar se `STMSizeDirectoryWithExcludedChildURLs` (exportado, sem entitlement aparente) reproduz o
   número de Documentos; a assinatura precisa ser recuperada por disassembly antes de chamar.

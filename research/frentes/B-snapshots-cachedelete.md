# Frente B — Snapshots APFS e purgeable por serviço (medição, sem estimativa)

Data: 2026-09-27. Máquina: macOS 27.0 (26A5416b), Apple Silicon, container `disk3` 494,4 GB.
Execução sem `sudo`, processo pai = Claude Code (herda Full Disk Access; ver Incertezas).
Backup do Time Machine em andamento durante todas as medições (não foi tocado).

## 1. Resumo

- **O tamanho de snapshot é mensurável sem root.** O Utilitário de Disco lista com
  `fs_snapshot_list()` e mede com `fsctl(mount, 0xC0184A13, {from_xid, to_xid, out})`.
  Confirmado no binário do app (chamadas diretas a `_fs_snapshot_list` e `_fsctl` com essa
  constante). O mesmo ioctl está em `+[TMSnapshot calculatePrivateSizeOfSnapshotsFromXID:toXID:…]`
  (TimeMachine.framework). Um programa C de 80 linhas reproduz os números, sem root, em ~1–7 s.
  - **Private Size** = `fsctl(xid, xid)`. **Cumulative Size** = `fsctl(0, xid)` (faixa a partir de 0,
    como o Utilitário de Disco faz). **Total liberável ao apagar todos** = `fsctl(0, xid_mais_novo)`.
- **Purgeable por serviço é mensurável sem root** via `CacheDeleteCopyItemizedPurgeableSpaceWithInfo`
  (CacheDelete.framework, privado, `dlsym`). A soma dos serviços bate **exatamente** com o total.
- **O "purgeable" da API pública é a urgência 3 do CacheDelete.** O log do cliente CacheDelete mostra a
  fórmula: `important = freespace + purgeable(urgência 3)`; `opportunistic = freespace +
  purgeable(urgência 1) − diskPadding`. Isso explica a anomalia "oportunista > importante" da
  pesquisa de 26/09.
- **`com.apple.TMCacheDelete` (snapshots do TM) = o `fsctl` de faixa total**, com diferença de 16 KiB
  quando o cache do CacheDelete é forçado a recalcular. Ou seja, o purgeable de snapshots do TM é o
  mesmo número do ioctl.
- Estado medido agora: 7 snapshots TM no Data, **24,5 GB** presos em snapshots (só 1,58 GB são
  "privados" de um único snapshot; o resto é compartilhado entre vários). Purgeable (urgência 3) =
  **28,15 GB**, dos quais 24,4 GB são snapshots do TM.

## 2. Achados

### 2.1 API do Utilitário de Disco para tamanho de snapshot (confiança: alta)

| Item | Valor |
|---|---|
| Listagem | `fs_snapshot_list(dirfd, attrlist, buf, len, 0)` (`<sys/snapshot.h>`) |
| XID do snapshot | `ATTR_CMN_OBJID` do item listado (`fid_objno`, `fid_generation<<32`); conferido contra `diskutil apfs listSnapshots` (XIDs idênticos) |
| Tamanho | `fsctl(path_do_mount, 0xC0184A13, &args, 0)` com `struct { uint64 from_xid, to_xid, out_bytes; }` |
| Decodificação | `0xC0184A13` = `_IOWR('J', 0x13, 24 bytes)`. `'J'` é o grupo de ioctl do APFS. O nome oficial não é público; chamamos de `APFSIOC_SNAP_PRIVATE_SIZE` no código |
| Header | `fsctl` não tem header no SDK; declarar `int fsctl(const char*, unsigned long, void*, unsigned int);` (símbolo em libSystem) |

Evidências:

- `otool -tV "/System/Applications/Utilities/Disk Utility.app/Contents/MacOS/Disk Utility"`:
  duas chamadas `bl … _fs_snapshot_list` e duas sequências
  `mov w1,#0x4a13; movk w1,#0xc018,lsl #16; mov w3,#0; bl _fsctl`. Na primeira, o struct é montado com
  `str xzr,[sp,#0x78]` (from = 0) e `stp x0,xzr,[sp,#0x80]` (to = xid, out = 0) → é o **Cumulative**.
  A string `"Could not size snapshots 0 - %llu on '%@'"` no mesmo binário confirma a faixa 0..xid.
  Strings `privateSize`, `cumulativeSize`, `tidemark`, `xidHighTidemark` e a classe `SUAPFSSnapshotInfo`.
- `dyld_info -disassemble …/TimeMachine.framework/…/TimeMachine`: em
  `+[TMSnapshot calculatePrivateSizeOfSnapshotsFromXID:toXID:forLiveVolumeMountPoint:error:]`,
  mesma constante `0xC0184A13`, struct `{from,to,out}` em `sp+0x28`, resultado lido de `sp+0x38`.
- StorageKit, DiskManagement e APFS.framework **não** contêm essa constante (grep na desmontagem).
  O Utilitário de Disco faz a chamada no próprio executável.
- Definições (Eclectic Light): *Private Size* = dados referenciados só por aquele snapshot;
  *Cumulative/Size* = aquele snapshot mais todos os anteriores.
  [eclecticlight 2021-11-13](https://eclecticlight.co/2021/11/13/understanding-snapshot-data-in-disk-utility/),
  [Apple, Disk Utility: ver snapshots APFS](https://support.apple.com/guide/disk-utility/view-apfs-snapshots-dskuf82354dc/mac)

**Correção da pesquisa de 26/09 (§4):** "`fs_snapshot_list` exige root e entitlement" está **errado para
listagem** nesta máquina. `man fs_snapshot_create` diz que *todas* as funções exigem superusuário e
entitlement, mas `fs_snapshot_list` funcionou como uid 501. Criar, apagar, renomear, montar e reverter
não foram testados (fora do escopo e alteram estado).

### 2.2 Método alternativo: TimeMachine.framework (confiança: alta)

`+[TMSnapshot …]` (classe privada) expõe, **sem root**:

- `snapshotNamesForLiveVolumeMountPoint:error:` / `snapshotsForLiveVolumeMountPoint:error:` (NSString)
  → nome, `xid`, `flags` (o snapshot "dataless" tem `flags = 0x20`).
- `calculatePrivateSizeOfSnapshotsForLiveVolumeMountPoint:error:` (NSString) → total.
- `calculatePrivateSizeOfSnapshotsWithNames:forLiveVolumeMountPoint:error:` (**NSURL**, não NSString:
  com string lança `-[__NSCFConstantString path]: unrecognized selector`).
- `calculatePrivateSizeOfSnapshotsFromXID:toXID:forLiveVolumeMountPoint:error:` (NSURL).
- `-[TMDisk calculateBytesUsedByAllSnapshots:]`, `-[TMDisk calculateVolumeBytesUsedIncludingSnapshots]`
  (não chamados).

É o mesmo ioctl por baixo; para o projeto em Rust, o `fsctl` direto é mais simples e não depende de
classe privada Objective-C.

### 2.3 `tmutil`: verbos ocultos (confiança: alta para a existência; semântica: média)

`strings /usr/bin/tmutil` mostra verbos ausentes do `tmutil help`:

| Verbo | Uso (string do binário) | Executado? |
|---|---|---|
| `sizelocalsnapshotsof` | `Usage: %s %s <pathtosize> [<anotherpathtosize> ...]` | **Não** (semântica não confirmada; `tmutil help sizelocalsnapshotsof` imprime nada) |
| `mountlocalsnapshots` / `unmountlocalsnapshots` | `<mount_point> <snapshot_date>` | Não (altera estado) |
| `testswiftfailmodule`, `logging --on …` | — | Não |

Seletores relacionados no binário: `calculatePrivateSizeOfSnapshotsFromXID:toXID:forLiveVolumeMountPoint:error:`,
`purgeLocalSnapshotsForVolumeGroupContaining:purgeAmount:urgency:` (usado por `thinlocalsnapshots`).
`tmutil uniquesize` já é documentado. Não é preciso nenhum verbo oculto: o `fsctl` cobre.

### 2.4 CacheDelete (confiança: alta)

Exportações relevantes de `/System/Library/PrivateFrameworks/CacheDelete.framework` (via
`dyld_info -exports`): `CacheDeleteCopyPurgeableSpaceWithInfo`, `CacheDeleteCopyItemizedPurgeableSpaceWithInfo`,
`CacheDeleteCopyAvailableSpaceForVolume`, `CacheDeletePurgeableSpace`, `CacheDeleteCopyPurgeHistory`,
`CacheDeleteCreatePurgeablePathEnumerator`, e as de purge (`CacheDeletePurgeSpace*`, `CacheDeletePurgeAsync`,
`CacheDeletePerformOperation`) — **estas nunca foram chamadas**.

Chaves de dicionário (strings do framework): `CACHE_DELETE_VOLUME`, `CACHE_DELETE_URGENCY_LIMIT`,
`CACHE_DELETE_NO_CACHE`, `CACHE_DELETE_AMOUNT`, `CACHE_DELETE_FREESPACE`, `CACHE_DELETE_TOTAL_AVAILABLE`,
`CACHE_DELETE_SHARED_PURGEABLE`, `CACHE_DELETE_TOTAL_FSPURGEABLE`, `CACHE_DELETE_NONPURGEABLE_AMOUNT`,
`CACHE_DELETE_NAME_MAP`, entre outras.

Assinaturas que funcionaram (inferidas e testadas, não documentadas):

```c
CFDictionaryRef CacheDeleteCopyPurgeableSpaceWithInfo(CFDictionaryRef info);          // total
CFDictionaryRef CacheDeleteCopyItemizedPurgeableSpaceWithInfo(CFDictionaryRef info);  // por serviço
// info = { "CACHE_DELETE_VOLUME": "/System/Volumes/Data", "CACHE_DELETE_URGENCY_LIMIT": 0..4,
//          opcional "CACHE_DELETE_NO_CACHE": true  (força recálculo; não purga) }
```

`CacheDeleteCopyAvailableSpaceForVolume(CFStringRef)` com essa assinatura derrubou o processo (SIGBUS,
exit 138): assinatura errada, não usar sem desmontar.

Resultado itemizado: uma chave por serviço (bytes), mais `CACHE_DELETE_NAME_MAP` (serviço →
`SIGNING_ID`/`PROCNAME`). Soma dos serviços = `CACHE_DELETE_AMOUNT` = `TOTAL_AVAILABLE`, exata.
`TOTAL_FSPURGEABLE` = `fspurgeable_document + fspurgeable_data + photolibraryd.cacheDelete` (exato:
15 350 284 288 + 3 137 613 824 + 1 572 864 = 18 489 470 976).

**Urgência** (nível até o qual os serviços aceitam liberar):

| Urgência | Total (bytes) | Observação |
|---|---|---|
| 0 | 28 154 570 158 | idêntico à 3 (0 parece "padrão") |
| 1 | 25 239 338 818 | entra na fórmula do "opportunistic" |
| 2 | 171 032 386 | **inconsistente**: itemizado somou 41,8 GB (photolibraryd 39,7 GB) ≠ total. Não confiar |
| 3 | 28 154 570 158 | = `NSURLVolumeAvailableCapacityForImportantUsageKey − statfs livre` |
| 4 | 53 742 458 287 | inclui `fspurgeable_document` 15,4 GB, `mds` 4,8 GB, `mobileassetd` 4,6 GB |

Fórmulas, lidas do log do cliente (`log show … subsystem == "com.apple.cache_delete"`, processo
`Storage`/`cdprobe`):

```
publicClientAvailableSpaceForVolume kCacheDeleteAvailableSpaceClassEssential 131770221998
  = freespace 103615483904 + purgeableSpace: 28154738094
publicClientAvailableSpaceForVolume 116506791746
  = (freespace 103615483904 + purgeableSpace: 25239338818) - diskPadding: 12348030976
```

Logo: **important = livre + purgeable(urg. 3)**; **opportunistic = livre + purgeable(urg. 1) − diskPadding**.
A origem de `diskPadding` (12 348 030 976) não foi identificada.

O painel Ajustes > Armazenamento (processo `Storage`) consulta o CacheDelete com
`CACHE_DELETE_URGENCY : 3` e loga cada serviço (ex.: 26/09 21:58, `TOTAL_AVAILABLE 3 328 252 546`).

**Cache do daemon:** sem `CACHE_DELETE_NO_CACHE`, `TMCacheDelete` repetiu `24 418 709 504` por vários
minutos com backup rodando. Com `NO_CACHE`, deu `24 532 549 632`, contra `fsctl` total `24 532 566 016`
medido logo depois (diferença de 16 384 B, com backup escrevendo). Conclusão: TMCacheDelete ≈ ioctl de faixa.

**Log como fonte:** o daemon `deleted` loga quase tudo como `<private>`; valores por serviço aparecem
só em parte das linhas. Serve para auditoria, não para medir. A API itemizada é a fonte.

Serviços registrados em `/System/Library/CacheDelete/` (28 plists), mais os dinâmicos do `NAME_MAP`
(extensões do Safari, Music, TV, Mail, cloudd, triald etc.).

### 2.5 O que significa cada coisa

- **`LimitingContainerShrink`** (plist de `diskutil apfs listSnapshots -plist`; nota em texto *"This
  snapshot limits the minimum size of APFS Container disk3"*): o snapshot com o bloco mais alto
  (*tidemark*) define até onde o container pode encolher sem destruí-lo. Não tem relação com espaço
  ocupado. Aqui: `com.apple.TimeMachine.2026-09-26-220859.local`. Confiança média (fontes:
  [eclecticlight 2021-11-13](https://eclecticlight.co/2021/11/13/understanding-snapshot-data-in-disk-utility/),
  [Bresink, painel APFS](https://www.bresink.com/osx/300863620-2/Docs-en/pgs/0166-APFS.html) — 404 ao buscar, citado pelo resumo da busca).
- **`Purgeable`** (mesmo plist): o snapshot pode ser apagado pelo sistema sob pressão de espaço
  (via CacheDelete → `TMCacheDelete`). Snapshots de terceiros (CCC, ChronoSync) não são purgeable
  ([eclecticlight 2023-04-27](https://eclecticlight.co/2023/04/27/where-does-macos-get-its-volume-free-space-figures-from/), já citado na pesquisa de 27/09).
- **Snapshot `(dataless)`** (em `tmutil listlocalsnapshots`, `flags 0x20` no TMSnapshot): o TM já
  converteu o snapshot para não reter dados. Medido: privado 0 e cumulativo 0.
- **`com.apple.os.update-<hash>`**: snapshot selado do volume **System** (`disk3s1s1`), que é o que o
  Mac monta como `/`. `mount`: `/dev/disk3s1s1 on / (apfs, sealed, local, read-only)`. É o sistema em
  uso, não sobra de atualização. `fs_snapshot_list("/")` o lista (XID 9 909 264), mas `fsctl` de
  tamanho em `/` devolve `EINVAL`: o volume vivo (`disk3s1`) não está montado. Não mensurável sem montar.
- **Snapshots de terceiros**: aparecem no mesmo `fs_snapshot_list` do volume, com outro prefixo de nome.
  Nenhum nesta máquina; o `fsctl` por XID deve funcionar igual (não testado, sem amostra).

### 2.6 Listar todos os snapshots de todos os volumes sem root

1. Enumerar volumes APFS montados: `getmntinfo()`/`statfs` (tipo `apfs`), ou `diskutil apfs list -plist`
   para o mapa container → volume → papel.
2. Para cada ponto de montagem: `open(mount, O_RDONLY)` + `fs_snapshot_list`.
3. Limite: volume **não montado** não é listável (`diskutil apfs listSnapshots disk2s1` →
   *"A disk with a mount point is required (-69854)"*); o System vivo idem. `diskutil apfs list -plist`
   não traz snapshots em macOS 27 (nenhuma chave com "nap" nos volumes).
4. Medido aqui: snapshots só no Data (7) e o `os.update` do System. Preboot, VM, Update, xART,
   iSCPreboot, Hardware, Recovery (`disk3s3`): zero.
5. **Não apontar para o volume de destino do TM durante backup**: `snapsize "/Volumes/Backups of …"`
   ficou em estado `U` (espera no kernel, imune a `kill -9`) por ~5 min e terminou sozinho. O backup
   seguiu (`tmutil status`: `Copying`, 17% → 21%).

## 3. Estado medido desta máquina (2026-09-27 ~14:45 -03, backup em curso)

`tmutil listlocalsnapshots /`: 7 snapshots (1 dataless). `diskutil apfs listSnapshots`: todos
`Purgeable: Yes`, `220859` com `LimitingContainerShrink`.

`snapsize /System/Volumes/Data` (bytes):

| Snapshot | XID | Privado | Cumulativo (0..xid) |
|---|---|---|---|
| 2026-09-20-163315 (dataless) | 13778332 | 0 | 0 |
| 2026-09-26-220859 | 14860700 | 581 595 136 | 581 595 136 |
| 2026-09-26-232000 | 14875915 | 154 050 560 | 1 066 156 032 |
| 2026-09-27-005232 | 14883463 | 175 497 216 | 1 542 799 360 |
| 2026-09-27-043642 | 14889473 | 171 892 736 | 1 862 324 224 |
| 2026-09-27-081652 | 14896291 | 196 706 304 | 23 682 535 424 |
| 2026-09-27-140444 | 14959151 | 266 567 680 | 24 463 130 624 |

- Soma dos privados: 1 546 309 632. Total preso (faixa 0..último): 24 463 130 624. A diferença
  (~22,9 GB) é dado compartilhado por dois ou mais snapshots: apagar um sozinho não libera; apagar
  todos libera. O salto no `081652` (+21,8 GB) mostra onde está esse bloco.
- O total cresce durante o backup (24,42 → 24,54 GB em minutos): o live continua mudando e os
  blocos antigos ficam presos no snapshot mais novo.

Volume Data (`cdprobe`, urgência 3, 14:47):

| Métrica | Bytes |
|---|---|
| `statfs` total | 494 384 795 648 |
| `statfs` livre (`f_bavail`) | 103 554 363 392 |
| NSURL important − livre | 28 153 628 078 |
| CacheDelete total (urg. 3) | 28 154 738 094 |

Por serviço, urgência 3 (itemizado, 14:44): TMCacheDelete 24 418 709 504; fspurgeable_data
3 137 290 240; app_container_caches 229 142 528; mobileassetd 177 890 024; mds 170 864 450;
ThumbnailsAgent 10 695 556; fspurgeable_document 8 278 016; photolibraryd 1 572 864; AssetCache 126 976.
Soma = 28 154 570 158.

Urgência 4 (máximo): 53 742 458 287 (tabela completa no `cdprobe`).

## 4. Mensurável e como (chamada exata)

| O quê | Chamada | Root? | Custo medido |
|---|---|---|---|
| Lista de snapshots de um volume montado | `fs_snapshot_list(open(mnt,O_RDONLY), {ATTR_CMN_NAME\|ATTR_CMN_OBJID}, buf, len, 0)` | Não | < 0,1 s |
| XID | `ATTR_CMN_OBJID` do item | Não | — |
| Private Size | `fsctl(mnt, 0xC0184A13, {xid, xid, 0}, 0)` → `out` | Não | ~1 s por chamada com backup ativo |
| Cumulative Size | `fsctl(mnt, 0xC0184A13, {0, xid, 0}, 0)` | Não | idem |
| Liberado ao apagar todos | `fsctl(mnt, 0xC0184A13, {0, xid_max, 0}, 0)` | Não | idem |
| Purgeable total (painel/API pública) | `CacheDeleteCopyPurgeableSpaceWithInfo({VOLUME, URGENCY_LIMIT:3})` ou NSURL important − `statfs` | Não | ~2 s frio, 0,05 s em cache |
| Purgeable por serviço | `CacheDeleteCopyItemizedPurgeableSpaceWithInfo({VOLUME, URGENCY_LIMIT:3[, NO_CACHE:true]})` | Não | 0,05 s (cache) a ~4 s (NO_CACHE) |
| Flags de snapshot (Purgeable, LimitingContainerShrink) | `diskutil apfs listSnapshots -plist <mnt>` | Não | — |

Em Rust: `libc::open` + `extern "C" { fn fs_snapshot_list(...); fn fsctl(...); }` (não conferido se o
crate `libc` já declara esses dois; declarar à mão é trivial). CacheDelete: `libc::dlopen`/`dlsym` +
`objc2-core-foundation` para montar/ler o `CFDictionary`.

## 5. Não mensurável (nesta frente)

- Tamanho do snapshot `com.apple.os.update-*`: `fsctl` em `/` → `EINVAL`; o System vivo não está
  montado. Montar exige root e altera estado.
- Snapshots de volumes não montados (Recovery `disk2s1`, Update `disk2s2`): nem listáveis sem montar.
- Semântica de `sizelocalsnapshotsof` do `tmutil`: não executado.
- Origem de `diskPadding` na fórmula do "opportunistic".
- Tamanho de snapshots no volume de destino do TM durante backup (bloqueou no kernel).

## 6. Incertezas

- **Full Disk Access**: todos os testes rodaram com FDA herdado (`ls ~/Library/Mail` funcionou). Não
  sei se `fs_snapshot_list`/`fsctl` funcionam num terminal **sem** FDA. Crítico para o CLI.
- **Semântica exata do ioctl** vem do uso pelo Utilitário de Disco e dos nomes do TimeMachine.framework,
  não de documentação. O Apple File System Reference não descreve esse ioctl (não conferido a fundo).
- **Estabilidade**: `0xC0184A13` e as funções do CacheDelete são privadas; podem mudar entre versões.
  O CLI deve tratar `ENOTTY`/`EINVAL`/símbolo ausente e cair para "não atribuído".
- **Urgência 2** deu resultado inconsistente (itemizado ≠ total). Não usar.
- **Assinaturas do CacheDelete** foram inferidas; as duas `Copy…WithInfo` funcionaram. `CopyAvailableSpaceForVolume` não.
- Números mudam a cada minuto com o backup rodando; comparações foram feitas em sequência curta.
- `LimitingContainerShrink`: fonte secundária; semântica de tidemark vem de Eclectic Light.

## 7. Próximos experimentos

1. Rodar `snapsize` e `cdprobe` num Terminal **sem** Full Disk Access (e como app assinado ad-hoc)
   para saber se o CLI precisa pedir FDA.
2. Repetir as medições **sem backup rodando** e comparar com os números visíveis no Utilitário de Disco
   (validar Private/Cumulative 1:1 na GUI; o usuário faz a comparação visual).
3. Com um snapshot de terceiro (ex.: CCC) numa máquina de teste: confirmar que aparece no
   `fs_snapshot_list` e que o `fsctl` mede, e que o CacheDelete **não** o conta.
4. Desmontar `CacheDeleteCopyAvailableSpaceForVolume` e `diskPadding` para fechar a fórmula do
   "opportunistic".
5. Desmontar o verbo `sizelocalsnapshotsof` do `tmutil` (sem executar) para confirmar que é só leitura.
6. Portar `snapsize` para Rust no `probe` e medir custo com e sem backup.
7. Fechar a conta do volume: `usado (statfs) = arquivos medidos + fsctl(0..xid_max) + … + não atribuído`,
   agora com o termo de snapshots medido direto (substitui a soma `ALLOCSIZE − PRIVATESIZE`, que só vê
   arquivos vivos).

## 8. Programas

Em [`research/experimentos/frenteB/`](../experimentos/frenteB/) (compilar com `./build.sh`):

| Arquivo | Faz |
|---|---|
| `snapsize.c` | Lista snapshots (`fs_snapshot_list`) e mede privado/cumulativo/total (`fsctl`). C puro |
| `cdprobe.m` | Purgeable total e por serviço (CacheDelete, somente leitura), comparado com `statfs` e NSURL. `CD_NO_CACHE=1` força recálculo |
| `tmsnap.m` | Mesmo resultado do `snapsize` via `TMSnapshot` (TimeMachine.framework) |
| `introspect.m` | Lista classes/métodos ObjC de um framework privado que casam com um regex |

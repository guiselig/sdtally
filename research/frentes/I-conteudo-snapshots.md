# Frente I — O que cada snapshot local está segurando

Data: 2026-09-28, 10:40–11:30 -03. macOS 27.0, Apple Silicon. Sem `sudo`; processo filho do
Claude Code (herda Full Disk Access). `tmutil status` = `Running = 0` antes de cada medição. Nada foi
montado, criado ou apagado. Programas em [`experimentos/frenteI/`](../experimentos/frenteI/).

## 1. Resumo

- **Achado central (confiança alta):** o `backupd` tem um **diff de snapshots no kernel**:
  `fsctl(mount, 0xC0304A10, args)` em `-[TMSnapshotDiffer enumerateChangesBetweenSnapshots:error:]`.
  Recebe dois XIDs e devolve lotes de `(oid, tipo)`. **Funciona sem root** (uid 501, FDA herdado) e
  **sem montar nada**. Reproduzido em `snapdiff.c`.
- Tipos: 1 = criado, 2 = modificado, 3 = apagado entre os dois snapshots (inferido: todos os tipo 3 e
  90 % dos tipo 1 não existem mais no volume vivo; os tipo 2 existem. Confiança média-alta).
- **Só dá nomes e contagens, não bytes.** Bytes exatos por arquivo exigem ler os blocos da cópia no
  snapshot, e para isso é preciso montar (root). Sem root, dá para nomear os arquivos (FSEvents dá o
  caminho de inodes já apagados) e medir o tamanho **atual** dos que ainda existem.
- **FSEvents sem root funciona** e o histórico cobre o volume desde a instalação (event id 5.733). Mas
  `FSEventsGetLastEventIdForDeviceBeforeTime` devolve **0** para qualquer data. O event id de cada
  snapshot foi achado cruzando snapdiff e FSEvents pelo inode (`bracket.py`).
- Com root, `snapheld.c` compara os blocos físicos (`F_LOG2PHYS_EXT`) da cópia no snapshot com os do
  mesmo oid no snapshot vizinho ou no vivo. É a medida exata. O método foi validado com clones num
  volume vivo; **no snapshot montado ainda não foi rodado**. Plano pronto: `plano_sudo.sh`.
- A Apple faz o mesmo em `tmutil sizelocalsnapshotsof <caminhos>` (verbo oculto): exige root, pede ao
  `backupd` que monte todos os snapshots, soma os blocos físicos do caminho em cada snapshot e tira os
  do vivo ("total blocks, unique blocks").

## 2. Estado medido (28/09 ~10:45, sem backup)

`snapsize /System/Volumes/Data` (Frente B), 9 snapshots:

| Snapshot | XID | Privado (B) | Cumulativo 0..xid (B) |
|---|---|---|---|
| 2026-09-20-163315 (dataless) | 13778332 | 0 | 0 |
| 2026-09-27-081652 | 14896291 | 21 820 211 200 | 21 820 211 200 |
| 2026-09-27-140444 | 14959151 | 386 011 136 | 23 914 266 624 |
| 2026-09-27-211000 | 15108969 | 562 327 552 | 32 121 516 032 |
| 2026-09-27-230621 | 15148369 | 170 352 640 | 32 734 973 952 |
| 2026-09-28-014333 | 15153334 | 108 236 800 | 33 185 009 664 |
| 2026-09-28-035230 | 15157013 | 59 879 424 | 33 256 689 664 |
| 2026-09-28-050331 | 15158100 | 51 060 736 | 33 519 009 792 |
| 2026-09-28-074016 | 15162830 | 146 587 648 | **55 464 857 600** |

O último snapshot sobe o cumulativo em 21,9 GB com só 0,15 GB privados: são blocos que estão em
vários snapshots e que o volume vivo reescreveu **depois das 07:40 de hoje**.

`/Library/Preferences/com.apple.TimeMachine.plist`: `SnapshotDates` do destino termina em
2026-09-20 20:41 UTC, `ReferenceLocalSnapshotDate` = 2026-09-20 19:33 UTC (o snapshot dataless),
`RESULT = 18`, 18 tentativas desde então. **Hipótese** (confiança baixa, não investigada): os backups
não estão fechando desde 20/09, e os snapshots locais se acumulam por isso.

## 3. Montar snapshot (pergunta 1)

| Via | Root? | Estado | Fonte |
|---|---|---|---|
| `fs_snapshot_mount(dirfd, dir, nome, flags)` | Sim + entitlement | Não testado (proibido) | `man fs_snapshot_create`: *"All snapshot functions require superuser privileges and also require an additional entitlement"*. `fs_snapshot_list` funciona sem root (Frente B), então o man não é confiável. Flags em `<sys/snapshot.h>`: `SNAPSHOT_MNT_DONTBROWSE`, `NOEXEC`, `NOSUID`, `NODEV`, `IGNORE_OWNERSHIP`, `NOFOLLOW` |
| `mount_apfs -s <snap> <volume_montado> <dir>` | Sim | Não testado | `man mount_apfs` (sinopse `-s snapshot pathname directory`); prática comum com `sudo` (Eclectic Light 2021-03-22; script MacGrab) |
| `tmutil mountlocalsnapshots <mount_point> <snapshot_date>` | O `tmutil` não confere o euid (desmontagem: sem `geteuid` no caminho do verbo); quem monta é o `backupd` (root) por XPC | Não executado (altera estado) | `+[LocalSnapshotVerbs _mountOrUnmountLocalSnapshotsWithAction:…]` → `+[TMLocalSnapshotMessenger mountLocalSnapshotForDate:forDisk:completionHandler:]`. Data no formato `2026-09-27-081652`. Par: `unmountlocalsnapshots` |
| `tmutil sizelocalsnapshotsof <caminho>…` | **Sim**: string `"Must be run as root."` após `geteuid` | Não executado (monta) | Chama `mountLocalSnapshotsForDisk:` e `mountedLocalSnapshotsForDisk:`, depois `-[NSURL tm_physicalBlocksError:]` por caminho e `removeIndexes:` (tira os blocos do vivo). Não vi desmontagem no fim: **presumir que deixa montado** e rodar `unmountlocalsnapshots` depois |
| Montagem do próprio TM durante backup | — | Não usar | `/Volumes/com.apple.TimeMachine.localsnapshots/…`; legível com FDA e sem root (Frente D §2). Só existe com backup rodando, e medir nessa hora concorre com o backup |
| Ler o dispositivo cru (`/dev/rdisk3s5`) | Sim | — | `crw-r----- root operator` |
| `tmutil mountsnapshot` (citado em blog de 2026) | — | **Não existe** no `tmutil` do macOS 27 | `strings /usr/bin/tmutil`: só `mountlocalsnapshots` |

Conclusão: **sem root não há montagem legítima.** O caminho sem root é o diff no kernel (§4).

## 4. Diff sem montar: `fsctl 0xC0304A10` (confiança alta no layout, média na semântica dos tipos)

Desmontagem de `/System/Library/CoreServices/TimeMachine/backupd` (`dyld_info -disassemble`):

```
-[TMSnapshotDiffer enumerateChangesBetweenSnapshots:error:]
  log "Diffing from %llu to %llu for %@"
  struct (sp+0x20, 48 B): earlierXID | laterXID | u32 cap=1000 | u32 count(out)
                          | u64 *oids (8000 B) | u8 *types (1000 B) | u64 cursor
  fsctl([mountPoint fileSystemRepresentation], 0xC0304A10, &s, 0)   ; _IOWR('J', 0x10, 48)
  repete enquanto count == 1000; só usa tipos 1..3
```

O bloco que consome converte o tipo em flags de evento do FSEvents (tipo 1 → `0x880`, 2 → `0x80`,
3 → `0x4080`), ou seja, o TM usa o diff como fonte de eventos (`eventSourceOptions`,
`readEventsWithError:readBlock:`) no lugar do FSEvents.

Comportamento medido com `snapdiff`:

| Teste | Resultado |
|---|---|
| Sem root, volume Data | Funciona |
| Ordem dos XIDs invertida | Mesma saída, byte a byte (o kernel ordena) |
| `later = 0` | `EINVAL` |
| `later` = XID que não é snapshot (`UINT64_MAX`, `xid+1`) | `ENOENT`: **não dá para diferenciar snapshot × vivo** |
| Custo | ~40–75 s por par, mesmo com 625 objetos (sys 13,6 s; resto é espera de E/S). Não depende do tamanho do diff |

Todos os pares consecutivos (sem `-p`, contagens):

| De → para | Objetos | Criado | Modificado | Apagado |
|---|---|---|---|---|
| dataless 09-20 → 09-27-081652 | 779 137 | 425 853 | 15 366 | 337 918 |
| 081652 → 140444 | 10 882 | 6 367 | 2 007 | 2 508 |
| 140444 → 211000 | 19 409 | 8 473 | 3 662 | 7 274 |
| 211000 → 230621 | 12 250 | 3 722 | 4 961 | 3 567 |
| 230621 → 09-28-014333 | 5 149 | 381 | 1 096 | 3 672 |
| 014333 → 035230 | 3 757 | 276 | 576 | 2 905 |
| 035230 → 050331 | 625 | 154 | 318 | 153 |
| 050331 → 074016 | 2 382 | 788 | 1 031 | 563 |

Semântica dos tipos, conferida com `fsgetpath` no volume vivo (par 035230 → 050331): tipo 3 → 153 de
153 `ENOENT`; tipo 1 → 139 `ENOENT`, 14 resolvidos (criados e depois apagados, ou vivos); tipo 2 →
246 resolvidos, 55 `EACCES`, 17 `ENOENT` (modificados e apagados depois). Pastas aparecem como
tipo 2 quando um filho muda.

O que isso responde: "o snapshot A segura blocos **destes objetos**" = tipos 2 e 3 do diff
A → seguinte. Para o privado de A, os objetos também precisam ter mudado no diff anterior → A
(ou o anterior ser dataless, como aqui).

## 5. FSEvents (pergunta 3)

| Pergunta | Resposta | Confiança |
|---|---|---|
| Histórico sem root? | Sim. `FSEventStreamCreateRelativeToDevice(dev, sinceWhen, FileEvents\|UseExtendedData)`. Teste no scratchpad: criar, clonar, anexar, renomear e apagar apareceram com inode (`kFSEventStreamEventExtendedFileIDKey`) e flags corretas. O arquivo apagado veio com `0x00419300` (criado+apagado+modificado+clonado) e o inode dele | Alta |
| Eventos em áreas root (`private/var/db`…) aparecem? | Sim, com FDA herdado. Sem FDA: não testado | Média |
| Quanto histórico? | O primeiro evento tem id 5 733 (o volume é de 13/09). Reproduzir tudo passa de 10 min (15,9 mi eventos em 590 s, interrompido). Desde o id 412 000 000: 2 024 373 eventos em 58 s | Alta |
| `FSEventsGetLastEventIdForDeviceBeforeTime` | Devolve **0** para 20/09, 27/09, 28/09 e 28/09 10:00 | Alta (medido) |
| O TM grava o event id do snapshot? | Sim, mas não em lugar legível sem montar: `eventMarkerWithLastDate:lastEventID:lastFSEventsUUID:snapshotXID:` e as chaves `com.apple.backupd.SnapshotVolumeLastFSEventID` e `SnapshotVolumeFSEventStoreUUID` (strings do `tmutil`/`backupd`, provavelmente xattrs no volume de backup no destino). Não está em `com.apple.TimeMachine.plist` | Média |
| UUID do banco | `FSEventsCopyUUIDForDevice` = `89466FE7-…`. Se mudar, o histórico foi zerado e os ids não valem mais | Alta |

**Event id de cada snapshot, sem root (`bracket.py`).** Para o snapshot X: primeira criação (flag
`Created`) de cada inode criado em X-1 → X, e de cada inode que só aparece em X → X+1. As duas
distribuições se separam; ficam caudas de 0 a 6 inodes (eventos coalescidos repetem `Created`).

| Snapshot | p99 de "já em X" | mínimo de "só em X+1" | `since` conservador |
|---|---|---|---|
| 081652 | 436 384 955 | 437 291 704 | 436 384 955 |
| 140444 | 439 395 751 | 439 409 534 | 439 395 751 |
| 211000 | 445 968 515 | 445 977 012 | 445 968 515 |
| 050331 | 448 416 463 | 448 416 118 | 448 416 118 |
| 074016 (mais novo) | 448 697 898 | — | 448 697 898 |

Depois do 074016: 31 901 inodes removidos e 22 768 modificados ou renomeados. Isso cobre o trecho
**do snapshot mais novo até o vivo**, que o diff do kernel não cobre.

**Nomes sem root (`nomes.py`)**, snapshot 081652 (o dos 21,8 GB), diff 081652 → 140444:

- Apagados: 2 508, com nome para 786 (1 722 foram criados antes da janela do FSEvents lida; com
  histórico desde id 0 teriam nome). Pastas mais comuns: `searchpartyuseragent`, WhatsApp
  ServiceExtension, `Biome`, `Mail/V10`, `Caches/CloudKit`.
- Modificados e ainda vivos: 2 007. Os maiores **hoje**: `Claude/vm_bundles/claudevm.bundle/rootfs.img`
  8,10 GiB, `.colima/_lima/_disks/colima/datadisk` 7,49 GiB, um `.jsonl` do Claude no iCloud
  1,52 GiB, `.colima/.../disk` 1,03 GiB, `Photos.sqlite` 0,52 GiB. Somam 21,76 GiB vivos.
- **Isso é tamanho atual, não bytes presos.** Imagens de disco de VM reescritas no lugar são as
  candidatas óbvias para os 21,8 GB e para o salto de 21,9 GB no 074016. É **hipótese** até o
  `snapheld` rodar.

## 6. Bytes exatos (pergunta 2)

| Método | Exato? | Root? | Estado |
|---|---|---|---|
| `F_LOG2PHYS_EXT` na cópia do snapshot menos o mesmo oid no vivo ou no snapshot vizinho (`snapheld`) | Sim, por bloco físico, com união global (clones dentro do snapshot contam uma vez) | Sim (montar) | **Validado no vivo**: clone 8 MiB → 8 MiB compartilhados; 2 MiB reescritos no clone → 2 097 152 só no original; `cp` sem clone → 0 compartilhado. `snapheld -t` testa a subtração. Não rodado em snapshot |
| `ATTR_CMNEXT_PRIVATESIZE` do vivo | Não: diz o que o vivo tem só dele (escrito depois do snapshot mais novo), não o que o snapshot segura | Não | — |
| Comparar por fileid + mtime + tamanho (getattrlistbulk nos dois lados) | Não: acha os objetos, não os blocos. `snapdiff` já dá os objetos sem montar | Sim (montar) | Substituído pelo `snapdiff` |
| `tmutil sizelocalsnapshotsof` | Sim, por caminho, contra o vivo | Sim | Não executado |
| `tmutil uniquesize` | Só para backups no destino (`man tmutil`). Não usar: seria no volume de destino | — | — |
| `tmutil compare` | Compara o computador com backups no destino. Não roda contra snapshot local sem montar | — | — |
| Árvore `extentref` do APFS (Apple File System Reference) | Seria a fonte exata, mas só lendo o dispositivo cru (root) com parser próprio (libfsapfs, apfs-fuse). Não conferido a fundo nesta frente | Sim | Não recomendado |

Limites do `snapheld` (ceiling):

- **Arquivo comprimido (decmpfs)**: `F_LOG2PHYS_EXT` → `ENOTSUP` (testado num `.nib`). O programa soma
  `st_blocks × 512` e marca `estimado_comprimido`.
- Bloco apagado aqui mas clonado para **outro** oid vivo conta como preso (superestima). Raro.
  Tratar com `ATTR_CMNEXT_CLONEID` se aparecer.
- `fsgetpath` com o fsid do **snapshot montado**: não testado. Se `sem_caminho_no_snapshot` vier alto,
  a saída é percorrer a montagem com `getattrlistbulk` e indexar por fileid.
- Metadados (nós de árvore) não entram. O `fsctl` de tamanho pode incluí-los: esperar diferença
  pequena na validação.
- Esparsos: pula buracos de 4 KiB em 4 KiB (lento em esparso enorme).

## 7. Custo e riscos (pergunta 4)

- Snapshot montado **não é apagável**: o `deleted`/`thinlocalsnapshots` falha enquanto estiver
  montado (Eclectic Light 2021-07-31 descreve o uso; o bloqueio é o comportamento padrão de volume
  ocupado, não medido aqui). Desmontar logo depois; o plano usa `trap`.
- Não montar nem medir com backup rodando: o `backupd` monta os mesmos snapshots
  (`createAndMountLocalSnapshots`) e a Frente F mostrou varredura concorrendo 10 min com ele.
- `snapdiff` e o `fsctl` de tamanho só no volume de boot. **Nunca no destino do TM** (Frente B: 5 min
  preso no kernel, imune a `kill -9`).
- `snapdiff` gasta ~45 s de kernel e E/S por par. 8 pares ≈ 6 min. O CLI deve rodar sob demanda,
  não por padrão.
- Montar com `nobrowse` para o Spotlight não indexar (confiança média).
- `0xC0304A10` é privado, como o `0xC0184A13`: tratar `ENOTTY`/`EINVAL`/`ENOENT` e cair para "não
  atribuído".

## 8. Plano para o dono (com `sudo`)

Pronto em [`experimentos/frenteI/plano_sudo.sh`](../experimentos/frenteI/plano_sudo.sh). Para o
snapshot dos 21,8 GB, cujo anterior é dataless, o "presos vs B" deve bater com o privado do `fsctl`:

```sh
cd "research/experimentos/frenteI"
./plano_sudo.sh 2026-09-27-081652 14896291 2026-09-27-140444 14959151
```

O que faz, em ordem:

1. Para se `tmutil status` não disser `Running = 0`.
2. Compila `snapdiff` e `snapheld` em `/private/tmp/frenteI` e roda `snapheld -t`.
3. `snapdiff … 14896291 14959151 > diff.txt` (sem sudo).
4. `sudo mount_apfs -o rdonly,nobrowse -s com.apple.TimeMachine.<A>.local /System/Volumes/Data /private/tmp/snapI_A` e o mesmo para B.
5. `sudo snapheld /private/tmp/snapI_A /private/tmp/snapI_B < diff.txt` → `presos_vs_B.tsv` (bytes,
   estado, oid, caminho) e o total deduplicado. Depois, o mesmo contra `/System/Volumes/Data` →
   `presos_vs_vivo.tsv`.
6. `sudo umount` das duas montagens e `rmdir` (também em caso de erro, via `trap`).

Critério de sucesso: `presos_exatos_dedup` (vs B) ≈ 21 820 211 200 B, a menos de metadados e
comprimidos estimados. Se bater, o método fica provado e o produto pode oferecer "este snapshot segura
X por causa destes arquivos" como passo opcional com sudo.

Alternativa só com ferramenta Apple (menos controle; monta **todos** os snapshots):
`sudo tmutil sizelocalsnapshotsof "$HOME/Library/Application Support/Claude/vm_bundles" "$HOME/.colima"`
e depois `sudo tmutil unmountlocalsnapshots / <data>` para cada data listada em `tmutil listlocalsnapshotdates /`.

## 9. Para o produto

| Sem root (padrão) | Com root (opcional, explícito) |
|---|---|
| `fsctl 0xC0184A13`: bytes por snapshot (Frente B) | `snapheld`: bytes exatos por arquivo |
| `fsctl 0xC0304A10`: quais objetos mudaram entre snapshots (contagem por tipo) | — |
| `fsgetpath` + FSEvents com inode: nomes, inclusive de apagados | — |
| Tamanho atual dos modificados, com rótulo "tamanho atual, não bytes presos" | — |

## 10. Programas

| Arquivo | Faz | Root |
|---|---|---|
| `snapdiff.c` | Diff de dois snapshots (`fsctl 0xC0304A10`); `-p` resolve caminho no vivo | Não |
| `fsevhist.c` | Histórico do FSEvents desde `epoch` ou `id:N`, com inode e flags; `-q` só conta | Não |
| `bracket.py` | Event id de um snapshot a partir de dois snapdiffs + fsevhist | Não |
| `nomes.py` | Nomeia objetos de um snapdiff pelo FSEvents e soma o tamanho vivo | Não |
| `extents.c` | Mapa físico de um arquivo; bytes de A que não estão em B | Não |
| `snapheld.c` | Bytes presos por arquivo de um snapshot montado (`-t` = autoteste) | Sim, para montar |
| `plano_sudo.sh` | Plano completo: checar backup, montar, medir, desmontar | Sim (dono) |

Compilar: `clang -O2 snapdiff.c -o snapdiff`, `clang -O2 -framework CoreServices fsevhist.c -o fsevhist`,
`clang -O2 extents.c -o extents`, `clang -O2 snapheld.c -o snapheld` (inclui `extents.c`).

## 11. Incertezas

- Nomes dos ioctls (`APFSIOC_SNAP_DIFF`, `…_PRIVATE_SIZE`) são nossos; a semântica vem do uso no
  `backupd`/Utilitário de Disco e dos testes, não de documentação.
- Tipos 1/2/3 do diff: inferidos (§4). O `backupd` não diz os nomes.
- Tudo rodou com FDA herdado. Sem FDA não foi testado.
- `sizelocalsnapshotsof`: comportamento lido da desmontagem, não executado.
- Fontes externas: Eclectic Light
  [2021-03-22](https://eclecticlight.co/2021/03/22/time-machine-to-apfs-backup-structure-and-access/)
  (snapshot local contém tudo, inclusive exclusões; acesso exige montar),
  [2021-07-31](https://eclecticlight.co/2021/07/31/explainer-snapshots/) (snapshot só leitura,
  montável); [Apple SE 451408](https://apple.stackexchange.com/questions/451408) (`mount_apfs` por
  nome). A Apple File System Reference (snap_meta, extentref) e libfsapfs/apfs-fuse não foram
  lidos a fundo: não são necessários com o diff do kernel.

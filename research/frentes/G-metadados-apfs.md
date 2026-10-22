# Frente G — metadados do APFS e o "não atribuído" do volume Data — 2026-09-27

Pergunta: dos ~30,8 GiB "não atribuídos" do Data (síntese §3), quanto é metadado do APFS e
quanto é outra coisa que a varredura não vê — medindo, sem root.

Máquina: macOS 27 (26A5416b), Apple Silicon, container `disk3`, Data = `disk3s5` (índice 4).
Sem sudo; processo herda FDA do Claude Code (Frente D §0). Backup do Time Machine rodando
(`BackupPhase = Copying`) durante tudo. Nenhum `fsctl` de snapshot no volume de destino do TM.

Confiança: **A** medido aqui ou código lido; **M** fonte secundária ou medido com ressalva;
**B** inferência.

Programas em `research/experimentos/frenteG/` (compilar com `./build.sh`):

| Arquivo | Faz |
|---|---|
| `apfsuc.c` | Abre o `AppleAPFSUserClient` do container e chama só seletores de leitura: 6, 7 e **0x1e** (contadores do volume) |
| `apfsspace.c` | Chama `APFSExtendedSpaceInfo` e `APFSVolumeGetSpaceInfo` (APFS.framework, privado, `dlsym`) |
| `meta_regress.py` | Imagem APFS esparsa descartável; cria arquivos/pastas/xattrs/clones em escalas e mede |
| `regress_2026-09-27.csv` | Resultado bruto da imagem |
| `denied_children.py` | Filhos diretos de pastas negadas via `lstat` (sem abrir) |
| `mobileasset_sizes.py` | Tamanhos gravados pelo MobileAsset para assets automáticos (Apple Intelligence etc.) |
| `snapc.c` | Teste de `fs_snapshot_create` sem root (falhou, esperado; removido do repo, no histórico do git) |

---

## 1. Resultado em uma tabela

Tudo medido entre 15:35 e 15:40, em sequência curta (o backup muda o volume em ~0,1 GiB).

| Parcela | GiB | Medição | Conf. |
|---|---|---|---|
| Uso do volume Data | **330,46** | `ATTR_VOL_SPACEUSED` = `fs_used` do seletor 7 (354 831 032 320 B) | A |
| Árvore visível (hardlink e clone 1×) | 273,08 | `walk_par` 10 threads | A |
| Xattrs em fluxo próprio (> 3 804 B) | 0,45 | `walk_par` `SDC_XATTR=1` | A |
| **Metadados das árvores vivas do volume** | **1,226** | (nós omap 6 415 + fsroot 314 474 + extentref 529) × 4 096, seletor 0x1e | A (identidade provada na imagem, §3) |
| Blocos só em snapshot (7 snapshots) | 23,02 | `fsctl 0xC0184A13` faixa total (24 711 954 432 B), `snapsize` da Frente B | A |
| Apagados e abertos (usuário) | 3,04 | `lsof +L1`, 295 arquivos, tamanho lógico | A (lógico, não alocado) |
| **Não atribuído** | **29,65** | resto exato | A |

Dentro dos 29,65 GiB, o que dá para dizer medindo:

| Item | Medida | Conf. |
|---|---|---|
| **Objetos que existem no volume e a varredura não vê** | **46 208 arquivos** e **~11 157 pastas** (contadores do superbloco − varredura) | A |
| Desses, filhos diretos das 253 pastas negadas | 21 180 (16 211 só em `/.fseventsd`) | A |
| Metadado de snapshot (versões antigas de nós do fsroot) | **≤ 1,54 GiB** (limite superior; §4.2) | M |
| Modelos da Apple Intelligence em pastas `restricted` | 19,16 GiB **declarados** pelo catálogo do MobileAsset (`_UnarchivedSize`), não alocação medida | M |

**Conclusão:** metadado do APFS não explica o não atribuído. O metadado das árvores vivas é
1,23 GiB e **já estava** dentro dos 30,8 (agora sai como linha própria). O de snapshot é no
máximo 1,54 GiB. Sobram ≥ 28 GiB que só podem ser conteúdo dos **46 mil arquivos invisíveis**
(pastas de root/SIP), arquivos apagados-e-abertos de processos de outros usuários e árvores
auxiliares pequenas. O candidato de maior peso, com número gravado pelo próprio sistema, são os
modelos da Apple Intelligence (19,16 GiB declarados). Bytes dos invisíveis continuam sem medição
sem root.

---

## 2. Onde estão os contadores do APFS (sem root)

### 2.1 IORegistry: não tem tamanho (A)

`ioreg -r -c AppleAPFSVolume -l`: 38 chaves por volume (`Role`, `Sealed`, `Size` = tamanho do
container, `UUID`, `VolGroupUUID`, `Encrypted`...) e `Statistics`, que só traz contadores de I/O
(`Metadata: Number of fsroot bytes written`, `omap bytes read`...). Nenhum contador de espaço,
arquivos ou nós. `AppleAPFSContainer`: mesma coisa; `IOUserClientClass = AppleAPFSUserClient`.
Confirma a Frente D §4.2.

### 2.2 APFS.framework → `AppleAPFSUserClient` (A)

`/System/Library/PrivateFrameworks/APFS.framework` exporta 130 funções `APFS*`. As de leitura
de espaço, desmontadas com `dyld_info -disassemble` (todas passam por `_apfs_container_iouc` →
`IOServiceOpen(container, task, 0)` → `IOConnectCallStructMethod`):

| Função | Seletor | Entrada → saída | Devolve (medido aqui) |
|---|---|---|---|
| `APFSContainerGetSpaceInfo` | 6 | — → 0x4c8 B | struct de spaceman; decodificado em parte: `[0x08]` bloco 4096, `[0x10]` blocos/chunk 32768, `[0x20]` blocos 120 699 413, `[0x28]` livres, `[0x40]` chunks 3 684. Sem totais de metadado |
| `APFSVolumeGetSpaceInfo` | 7 | índice (4 B) → 16 B | `{tamanho do container, fs_used}`; `fs_used` = `ATTR_VOL_SPACEUSED` |
| `APFSExtendedSpaceInfo` | 8 (container) / 0x41 (volume) | → CFDictionary serializado | container: `size, free, used, reserve, blksize, Volume count, Max volume count`; por volume: `fs_size, fs_free, fs_used, fs_quota, fs_reserve, Role` |
| `APFSStatisticsProcessContainer` | **0x1e** | índice (4 B) → **0x110 B** | contadores do volume, abaixo |

`DiskManagement.framework` importa `APFSExtendedSpaceInfo`: é daí que vem o "Capacity Consumed" /
`CapacityInUse` do `diskutil` (import visto com `dyld_info -imports`; a chamada exata dentro do
DiskManagement não foi desmontada, M).

**Seletor 0x1e** (offsets conferidos contra a saída do `apfs_stats` nos volumes do `disk1`):

| Offset | Campo | Data (disk3s5) |
|---|---|---|
| 0x08 | arquivos (inodes regulares; apagado-e-aberto **não** conta, §3.3) | 2 021 038 |
| 0x10 | pastas | 365 802 |
| 0x18 | snapshots | 7 |
| 0x20 | nós da árvore omap | 6 415 |
| 0x30 | nós da árvore fsroot | 314 474 |
| 0x28 | nós da árvore extentref | 529 (oscila 450–880 com o backup) |
| 0x38 | "extents" | 35 501 (oscila; semântica não confirmada) |
| 0x40 | arquivos purgeable | 31 113 |
| 0xa8 | bytes purgeable | 18 617 475 072 |
| 0xb0 / 0xc8 / 0xd0 | purgeable por classe | 1 572 864 / 15 372 722 176 / 3 242 524 672 |

Os três últimos batem com `photolibraryd.cacheDelete`, `fspurgeable_document` e
`fspurgeable_data` do CacheDelete (Frente B §2.4), e somam exatamente o 0xa8. (A)

Custo: 0,2 s para os seis volumes do `disk3`. Sem root, sem entitlement. Todos os testes com FDA
herdado; sem FDA não testado (IOKit não passa pelo TCC de arquivos, mas não confirmado). (A/B)

Contêiner, conferido de novo: `used − Σ fs_used` = 391 058 067 456 − 390 897 672 192 =
**160 395 264 B**, idêntico ao byte à Frente D §4.1. Reserve e quota: 0 em todos os volumes e no
container. (A)

### 2.3 `apfs_stats`: não usar (A)

`/System/Library/Filesystems/apfs.fs/Contents/Resources/apfs_stats -e` imprime esses contadores
("Number of fsroot tree nodes" etc.) sem root. Mas o histograma de tamanhos que ele mostra vem de
um **`fts_open(FTS_PHYSICAL|FTS_XDEV)` em cada volume montado** (desmontagem de
`APFSStatisticsProcessContainer`, `0x192B1FE70`): percorre tudo, inclusive o destino do TM e pastas
dataless. Rodado com `alarm 60`, não terminou. O CLI deve chamar o seletor 0x1e direto.
Ele também usa dois `fsctl` do grupo `'J'` (`0xC0584A1E`, `0xC2004A78`) ligados a graft/cryptex;
não chamados.

`sm_stats` e `slurpAPFSMeta` (mesma pasta) leem o dispositivo bruto; não executados.

### 2.4 Kext e fontes

`/System/Library/Extensions/apfs.kext` não tem binário (só `Info.plist`); o código está no
kernelcache (IMG4) em Preboot, não extraído. Não achei `APFSIOC_*` em texto em nenhum binário de
`apfs.fs`. O Apple File System Reference descreve `apfs_num_files`, `apfs_num_directories`,
`apfs_num_snapshots`, `apfs_fs_alloc_count` e `btree_info_t.bt_node_count`, que é o que o
seletor 0x1e devolve (nomes por correspondência, M).
[Apple File System Reference](https://developer.apple.com/support/downloads/Apple-File-System-Reference.pdf)

---

## 3. Prova do método numa imagem APFS (reproduzível)

`meta_regress.py`: `hdiutil create -type SPARSE -fs APFS -size 8g`, montada com `-nobrowse`,
passos cumulativos, `sync` + 7 s antes de cada leitura, contadores do seletor 0x1e.

### 3.1 Identidade exata

Em **todos os 15 pontos** (vazio, 1k/10k/100k arquivos, 1k/10k/50k pastas, 10k/100k xattrs,
10k/50k arquivos de 1 byte, original de 1 MiB, 1k/10k clones, tudo apagado):

```
fs_used = (nós omap + nós fsroot + nós extentref) × 4096 + 8192 + blocos de dados
```

O resto depois dos nós foi 0 nos passos sem dados, 40 960 000 e 204 800 000 com 10k e 50k
arquivos de 1 byte (= n × 4096, conferido com `st_blocks`), +1 048 576 com o original, e
**não mudou** com 10 mil clones. Depois de apagar tudo: 28 672 = (3+1+1) × 4096 + 8192. Os
8 192 fixos são superbloco do volume + raiz da árvore snap_meta (B). (A)

Consequência: nesta imagem o metadado do volume é **só** o que os contadores de nós mostram.
No Data isso vale para as árvores vivas; árvores auxiliares que não aparecem no seletor
(snap_meta, extentref de cada snapshot, fext, doc-id, pfkur) ficam no não atribuído (§4).

### 3.2 Bytes de metadado por objeto (regressão linear, nomes de 24–30 caracteres)

| Objeto | Pontos (n → bytes) | Bytes/objeto |
|---|---|---|
| Arquivo vazio | 1k→303 104; 10k→3 141 632; 100k→32 059 392 | **321** |
| Pasta vazia | 1k→299 008; 10k→3 063 808; 50k→15 728 640 | **315** |
| Xattr embutido de 100 B | 10k→2 154 496; 100k→21 082 112 | **210** |
| Arquivo de 1 B (além do bloco de dados) | 10k→4 759 552; 50k→23 785 472 | **476** |
| Clone completo de 1 MiB (`cp -c`) | 1k→339 968; 10k→3 588 096 | **361** |

No Data real: 1 316 528 128 B / (2 021 038 + 365 802) = **552 B por objeto** (mais extents,
743 mil objetos com xattr, nomes longos, nós menos cheios). (A)

### 3.3 Apagado mas aberto

Arquivo de 64 MiB, aberto e apagado: `arquivos` cai de 1 para 0 na hora; `fs_used` só cai no
`close`. Logo, os 46 208 invisíveis **não** incluem apagados-e-abertos. (A)

### 3.4 Snapshots na imagem: impossível sem root

`fs_snapshot_create` numa imagem do próprio usuário: `EPERM`. Não dá para medir na imagem se o
`fsctl 0xC0184A13` inclui metadado de snapshot. (A)

---

## 4. Metadado de snapshot (item 2 da tarefa)

### 4.1 O `fsctl` de tamanho inclui metadado? Não medido

Sem snapshot na imagem, não há como separar. A Frente B mostrou que ele bate com o
`TMCacheDelete` (16 KiB de diferença), o que não diz nada sobre nós de árvore. (B)

### 4.2 Limite superior medido

Nós do fsroot modificados depois de um snapshot ficam retidos como versões antigas; cada versão
precisa de uma entrada na árvore omap (chave `oid+xid`). Entrada de folha omap = 4 B de índice +
16 + 16 = 36 B; folha de 4 096 B com 56 B de cabeçalho cabe **112** (APFS Reference,
`btree_node_phys_t`, `omap_key_t`, `omap_val_t`, M).

- Entradas omap ≤ 6 415 × 112 = 718 480 (contando nós internos como folhas, o que só folga o limite)
- Versões vivas do fsroot = 314 474
- Versões antigas ≤ 404 006 → **≤ 1,54 GiB** de nós do fsroot presos em snapshot

Não é estimativa: é limite a partir de contadores medidos e de constantes da especificação. O
valor real pode ser 0. Árvores extentref de cada snapshot (físicas, fora do omap): sem contador;
a viva tem 450–880 nós (2–3,5 MiB), então 7 cópias ficam na casa de dezenas de MiB (B).

---

## 5. Outras fontes invisíveis à varredura (item 3)

| Fonte | Achado | Conf. |
|---|---|---|
| **Pastas de root/SIP** | 249 negadas no `open` + 4 no `getattrlistbulk`. `lstat` da pasta negada funciona e, no APFS, `st_nlink − 2` = filhos diretos (conferido em 22 085 pastas legíveis, 0 divergências). Soma: 21 180 filhos diretos. Maiores: `/.fseventsd` 16 211, `/Library/Caches/com.apple.iconservices.store` 2 855, `/private/var/db/fseventsd` 825, `/private/var/folders/q8/…/C` 133. Profundidade maior e bytes: só com root | A |
| **Apple Intelligence** (`AssetsV2/*UAF_FM*`, `UAF_IF_Planner*`, flag `restricted`) | Cada pasta tem 1 filho. Mas `/System/Library/AssetsV2/persisted/AutoAssetDescriptors/*.state` é legível: 439 descritores `MADAutoAssetDescriptor`, todos `isOnFilesystem = True`. `_UnarchivedSize` dos tipos em pastas restritas: GenerativeModels 16,20, CodeLM 2,53, IF.Planner 0,33, Visual 0,09, Overrides 0,01 GiB = **19,16 GiB declarados**. `downloadedFilesystemBytes` soma 60,4 GiB só no GenerativeModels, maior que o não atribuído inteiro, e dá 4,1 GiB para assets com `_UnarchivedSize` 0: **não confiável**, não usar | M (valor do catálogo, não alocação) |
| Swap | Volume VM próprio (`disk3s6`, `st_dev` 16777234 ≠ Data 16777231): 8 `swapfile` + `kernelcore`, 9,66 GB de `fs_used`. Não entra no Data. `/private/var/vm/sleepimage` (2 GiB) está no Data e **é visível** na árvore | A |
| `.fseventsd` | `EACCES`; 16 211 entradas diretas; bytes só com root | A |
| Journal | APFS não tem journal (copy-on-write + checkpoints); o "journaled" do `mount` é rótulo. Checkpoints ficam no container, dentro dos 160 395 264 B de metadado do container, não no Data | M (APFS Reference) |
| Reservas do space manager / reserve / quota | `fs_reserve = 0`, `fs_quota = 0` em todos os volumes; `reserve = 0` no container | A |
| Clones parciais | Contam duas vezes na árvore: **reduzem** o não atribuído. O não atribuído é limite inferior do invisível | A (Frente D §4.2) |
| Purgeable do APFS em pasta negada | 31 113 arquivos, 17,34 GiB pelo 0x1e; se estão ou não em pasta visível não foi cruzado (próximo passo: `CacheDeleteCreatePurgeablePathEnumerator`, só leitura, não testado) | B |
| Apagados-e-abertos de processos de root/outros uid | `lsof` sem sudo não vê; `libproc` nega (232 processos na Frente D). Não medível | A (negação) |

Não tentado de propósito: enumerar inodes por `/.vol/<dev>/<id>` para ler tamanhos dentro de
pastas negadas. Seria contornar a permissão do sistema; decisão do dono.

---

## 6. O que muda no produto

1. **Linha nova, medida:** "Metadados do APFS (árvores do volume)" = nós × 4096, seletor 0x1e.
   1,23 GiB aqui.
2. **Linha nova, medida:** "Objetos invisíveis: N arquivos, M pastas" = contadores do volume −
   varredura (arquivos visíveis = únicos + clones repetidos; hardlink não soma). Dá número ao
   que hoje é só "pastas não lidas", sem precisar de root.
3. Por pasta negada, mostrar filhos diretos (`st_nlink − 2`).
4. Opcional, rotulado como "declarado pelo sistema, não medido": modelos da Apple Intelligence
   por `_UnarchivedSize`.
5. Não usar `apfs_stats` (varre todos os volumes).

Rust: IOKit com `extern "C"` + `#[link(name = "IOKit", kind = "framework")]`, sem crate novo.
`IOServiceMatching("AppleAPFSContainer")`, escolher o container cujo filho tem `BSD Name`
`disk3s*`, `IOServiceOpen(tipo 0)`, `IOConnectCallStructMethod(0x1e, &idx, 4, buf, 0x110)`.
O índice do volume é o sufixo `sN` − 1. Tratar `kIOReturnBadArgument`/erro como "indisponível"
(seletor privado, pode mudar entre versões).

## 7. Incertezas

- Seletores 6/7/0x1e e o layout de 0x110 B são privados; conferidos só no macOS 27.
- Semântica de "extents" (0x38) e de parte do struct do seletor 6 não confirmada.
- A identidade da §3.1 foi provada numa imagem sem snapshots, sem criptografia e sem selo; no Data
  (FileVault, 7 snapshots) só as árvores vivas estão cobertas.
- Os 46 208 / 11 157 invisíveis têm ruído de algumas centenas (varredura de 36 s com backup).
  Contas de pastas: vistas = abertas (354 197) + negadas no `open` (249) + dataless (140) +
  pontos de montagem (59).
- Sem FDA não testado.
- O binário `scratchpad/target/release/walk_par` foi recompilado a partir de cópia do `probe` com
  o limite da lista `SDC_DENIED` de 200 para 100 000; o resto do comportamento é o mesmo.

## 8. Próximos experimentos

1. Varredura com `sudo` (já na síntese §9): agora dá para conferir se ela encontra exatamente os
   46 208 arquivos que faltam; o que sobrar em bytes depois disso é SIP + metadado auxiliar.
2. `CacheDeleteCreatePurgeablePathEnumerator` para saber onde estão os 31 mil purgeable.
3. Repetir a conta sem backup e depois que os snapshots saírem: o limite de 1,54 GiB deve cair.

## 9. Aplicado no produto (18/10/2026)

O item 1 de §6 (metadados das árvores vivas) virou `informational.apfs_metadata` (`src/sys/iokit.rs`,
`src/sources/apfs_meta.rs`): seletores 6, 7 e 0x1e do `AppleAPFSUserClient`, sem root e só leitura. O
índice do volume é o sufixo `sN` menos 1 (o IOKit mostra "Preboot@2" para disk3s2) e o `fs_used` do
seletor 7 precisa bater (1 %) com o `ATTR_VOL_SPACEUSED` do Data, senão sai "não medido". Mac do dono,
27, 18/10: 1,66 GB (omap 6.862, fsroot 393.338, extentref 6.278 nós) dentro de 17,0 GB de Sem
explicação sem sudo. Fora do produto, de propósito: "objetos invisíveis" (item 2 de §6) e o limite de
1,54 GiB de metadado de snapshot (§4.2), que é um teto e não uma medida.

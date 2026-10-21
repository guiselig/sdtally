# Frente H — quanto os modelos do Apple Intelligence ocupam no volume Data, medido — 2026-09-28

Pergunta: trocar os 19,16 GiB **declarados** pelo catálogo do MobileAsset (`_UnarchivedSize`,
Frente G) por um número **medido**, sem sudo, sem contornar SIP/TCC.

Máquina: macOS 27.0 (26A5416b), Apple Silicon, Data = `disk3s5`. Sem sudo; FDA herdado do
Claude Code. Backup do Time Machine parado (`Running = 0`), 9 snapshots locais. Medições entre
10:57 e 11:13.

Confiança: **A** medido aqui ou código lido; **M** fonte secundária ou medido com ressalva;
**B** inferência.

Programas em `research/experimentos/frenteH/` (compilar com `./build.sh`; nenhum pede root):

| Arquivo | Faz |
|---|---|
| `dirstats.c` | Lê as estatísticas de pasta do APFS (dir-stats) com `fsctl(path, 0xC1104A71)`, **sem abrir a pasta** |
| `assetgc_dirsize.py` | Soma, por tipo de asset, o `dirSize` que o próprio `mobileassetd` mede e grava no log |
| `cmpwalk.py` | Confere dir-stats contra varredura própria (`st_blocks`, hardlink 1×, separa comprimidos) |
| `negada_por_diferenca.py` | Pasta `EACCES` = dir-stats(pai) − dir-stats(irmãs) − arquivos diretos (prova de conceito) |
| `stmsize.m` | Chama `STMSizeDirectoryWithExcludedChildURLs`, a função que o painel usa para "Apple Intelligence" |
| `dirattr.c` | `getattrlist` de pasta sem abrir (resultado negativo, §4) |

---

## 1. Resultado

**Os modelos ocupam 20 586 885 120 B = 19,173 GiB no Data, medido pelo APFS.** (A)

A fonte é a **estatística de pasta do APFS** (`dir-stats`: bytes físicos e descendentes que o
sistema de arquivos mantém por pasta). Ela é lida com `fsctl` direto no caminho, sem abrir nem
listar a pasta. Funciona nas pastas `restricted` que dão `EPERM` até para root. É o mesmo que
`/System/Library/Filesystems/apfs.fs/Contents/Resources/apfs.util -S <pasta>` imprime.

| Pasta em `/System/Library/AssetsV2/` | Descendentes | Bytes físicos (dir-stats) | GiB | Declarado (`_UnarchivedSize`) GiB | `dirSize` do mobileassetd GiB |
|---|---|---|---|---|---|
| `com_apple_MobileAsset_UAF_FM_GenerativeModels` | 2 709 | 17 397 940 224 | 16,203 | 16,195 | 16,198 |
| `com_apple_MobileAsset_UAF_FM_CodeLM` | 73 | 2 711 678 976 | 2,525 | 2,525 | 2,525 |
| `com_apple_MobileAsset_UAF_IF_Planner` | 85 | 354 394 112 | 0,330 | 0,330 | 0,330 |
| `com_apple_MobileAsset_UAF_FM_Visual` | 157 | 100 790 272 | 0,094 | 0,094 | 0,094 |
| `com_apple_MobileAsset_UAF_IF_PlannerOverrides` | 2 409 | 14 864 384 | 0,014 | 0,014 | — |
| `com_apple_MobileAsset_UAF_FM_Overrides` | 2 668 | 7 217 152 | 0,007 | 0,006 | — |
| **Total** | | **20 586 885 120** | **19,173** | 19,16 | |

Três fontes independentes concordam dentro de 0,05 %: a do sistema de arquivos (dir-stats), a
do daemon que gerencia os assets (`dirSize`, §3) e a do catálogo (declarada). O produto deve
usar **dir-stats** como valor medido e pode mostrar as outras duas como conferência.

### 1.1 A conta do volume com o valor medido

Com todas as 20 pastas `EPERM` do Data (lista refeita hoje por varredura sem root), dir-stats dá
**19,268 GiB**: os 19,173 dos modelos, mais `private/var/db/searchparty` 0,058,
`private/var/containers/Data/ProtectedSystem` 0,020, `private/var/protected/trustd/private` 0,012,
`private/var/networkd/db` 0,004 e outras menores. Seis pastas `EPERM` respondem ao `fsctl` com
`EACCES` (`private/var/db/{SystemKeys, Spotlight-V100, Spotlight, appinstalld}`,
`sysdiagnose/com.apple.sysdiagnose`, uma de `private/var/folders/zz/…/C`). (A)

Conta da síntese §3c (28/09, com root) com o valor medido no lugar do declarado:

| Parcela | GiB | Natureza |
|---|---|---|
| Resto (uso − árvore com root − snapshots − apagados abertos) | 20,46 | medido (§3c) |
| Pastas `EPERM` (dir-stats) | 19,27 | **medido** (hoje, 11:00) |
| Metadados das árvores vivas | 1,23 | medido (Frente G) |
| **Resto − EPERM − metadados** | **−0,04** | |

Ressalvas: parcelas medidas em horários diferentes (manhã e 11:00 do mesmo dia; os modelos
não mudaram, ver §2.3). A árvore com root provavelmente conta **2,2 GiB a mais** por causa dos
grafts (§2.4). Se contar, o resto real é ~22,7 GiB e sobram ~2,2 GiB sem explicação. Candidatos:
xattrs em fluxo próprio (0,45 GiB em 27/09), metadado de snapshot (≤ 1,54 GiB) e pastas
`EPERM` que não responderam ao `fsctl`. (M)

---

## 2. dir-stats do APFS: como funciona e o que conta

### 2.1 A chamada (A)

`apfs.util -S` foi desmontado (`otool -arch arm64e -tvV`, `__text` em `0x100000d80`). A chamada
é `fsctl(path, 0xC1104A71, buf, 0)` com `buf` de 0x110 B **zerado**. Na saída,
`buf+0x38` = descendentes, `buf+0x40` = bytes físicos e `buf+0x30` = gen-count.
`dirstats.c` reproduz o `apfs.util` ao byte.

**Cuidado:** o mesmo código com `buf+4 = 1` é o `apfs.util -M` ("flag the named directory as
maintain-dir-stats"), que **escreve** no volume. O produto tem de fixar o buffer zerado e nunca
expor esse campo. `-P`, `-K` e `-C` do `apfs.util` purgam; não foram usados.

### 2.2 Permissão (A)

- Funciona sem root em pasta `restricted`/SIP (`EPERM` no `open`) quando o modo POSIX dá leitura
  (as pastas `UAF_*` são `drwxr-xr-x root`). O SIP não bloqueia o `fsctl`.
- Falha com `EACCES` quando o modo POSIX nega (`.fseventsd`, `.Spotlight-V100`, `drwx------ root`).
  Nesses casos o tamanho sai por diferença a partir do pai (§5).
- Pasta dataless (File Provider) devolve 0 e **não é materializada**: flags, tamanho e ctime não
  mudaram depois da chamada.

### 2.3 Semântica, testada numa pasta do scratchpad e em pastas legíveis (A)

| Caso | dir-stats |
|---|---|
| Arquivo comum | alocado (bate com `st_blocks × 512`) |
| Clone (`cp -c`) | **conta de novo, inteiro** (10 MiB + clone = 20 MiB) |
| Hardlink | conta uma vez (descendentes e bytes não mudam) |
| Esparso (`truncate -s 1g`) | só o alocado (0) |
| Xattr grande (fluxo próprio) | **não conta** |
| Arquivo comprimido (decmpfs, `UF_COMPRESSED`) | **não conta** (`/Library/Apple`: dir-stats 20 094 976 = varredura sem comprimidos 20 094 976; com eles, 76 419 072) |
| Subpasta | entra em descendentes e bytes |
| Conteúdo em `.AssetData` dos assets com cryptex | **não conta** (§2.4) |

Conferência em pastas legíveis de `AssetsV2`: 57 das 60 primeiras batem ao byte e em
descendentes com a varredura própria. As 3 que não batem são explicadas por comprimidos ou por
`.AssetData`.

Consequências:
- Para os modelos, dir-stats é **limite inferior**, porque exclui comprimidos e xattrs, e também
  **limite superior**, porque conta clone em dobro. Como bate com o `dirSize` do mobileassetd e com o
  catálogo, nenhum dos dois efeitos pesa aqui. (M para "nenhum dos dois pesa")
- dir-stats da **raiz** do volume **não** substitui a varredura: às 10:58, dir-stats(raiz)
  340 161 277 952 + snapshots 55 586 414 592 + metadados 1 315 512 320 = 397,06 GB, 13,56 GB acima
  do `fs_used` 383 498 702 848. Clones contados em dobro são a explicação coerente. (A para a
  diferença; B para a causa)
- `ATTR_CMNEXT_RECURSIVE_GENCOUNT` = 2 em quase todas as pastas de sistema. Não é tamanho e não
  indica dir-stats: o `fsctl` responde também onde o gencount é 0 (`~/Library`, pastas novas).

### 2.4 Grafts de cryptex: a varredura conta em dobro (M)

Assets com `SecureAssetData/SecureMobileAsset.dmg → ../AssetData/Restore/*_Cryptex.dmg` têm uma
pasta `.AssetData` (root, sem `restricted`, datada do build) com o conteúdo do DMG. Mesmo `st_dev`
e mesmo `f_mntonname` do Data; não aparece em `mount` nem em `hdiutil info`. Nem dir-stats nem o
`dirSize` do mobileassetd contam esse conteúdo. Uma varredura comum conta o DMG **e** o conteúdo:

| Asset legível | Varredura − dir-stats |
|---|---|
| `UAF_Siri_Understanding` | 1,652 GiB |
| `UAF_Photos_SpatialPhotosRelive` | 0,383 GiB |
| `UAF_Speech_AutomaticSpeechRecognition` | 0,179 GiB |
| **Total em `AssetsV2`** | **2,215 GiB** |

Hipótese: graft de cryptex. Não confirmei a montagem. O efeito na varredura foi medido.
O `walk_par` do projeto não trata isso (`grep graft|AssetData|cryptex` sem resultado no código).
**Correção para o produto:** não descer em `.AssetData` que seja irmã de `SecureAssetData`. Outra
opção é usar dir-stats na pasta do asset, se a regra for medir o que ocupa o Data.

---

## 3. `dirSize` do mobileassetd (item 1): segunda medição, independente (A)

O `mobileassetd` tem acesso às pastas restritas. Ele grava no log (`com.apple.mobileassetd:CacheDelete`,
nível Default, legível sem sudo por admin) uma linha por asset a cada avaliação de coleta:

```
AssetGC determine | /System/Library/AssetsV2/com_apple_MobileAsset_UAF_FM_GenerativeModels/purpose_auto/<id>.asset
  | dirSize: 65270150(62MB 252KB 390 bytes) | Urgency:1 | GCTypeUnlockedReferenced | ... | Result: will not delete - reason: never remove
```

- Uma consulta de purgeable ao CacheDelete, feita com o `cdprobe` da Frente B nas urgências 1–4
  (só leitura), **dispara** essa avaliação na hora: 1 557 linhas às 11:08, todas `will not delete`.
- Soma do último `dirSize` por pasta: GenerativeModels 17 392 908 130 (155 assets), CodeLM
  2 711 544 747 (4), Planner 354 118 592 (5), Visual 100 507 060 (6).
- Semântica: ≈ tamanho **lógico** dos arquivos do asset. Em `AppleDeveloperDocumentation` dá
  1 384 235 265 contra lógico 1 384 260 210 e alocado 1 384 275 968. Não inclui `.AssetData`, o
  que confirma §2.4.
- Útil como conferência. Depende do log e do formato da mensagem (privado), então o valor principal
  continua sendo dir-stats.

### 3.1 APIs do MobileAsset/UAF: só tamanho declarado (A para a lista, M para a semântica)

Introspecção (`objc_copyClassNamesForImage` + `class_copyMethodList`, `introspect` da Frente B;
nada chamado):
- `UnifiedAssetFramework`: `-[UAFAssetSetManager downloadSizeForSubscriptions:…]`,
  `diskSpaceNeededForSubscribers:…`, `_sizeForSubscriptions:policies:sumByKey:…`,
  `+[UAFAutoAssetManager sizeInBytesForConfig:key:error:]`, `-[UAFAssetSetStatus totalBytes/completedBytes]`.
  Todos são tamanho de download ou necessidade de espaço, somados por chave do catálogo. Nenhum
  mede ocupação em disco.
- `MobileAsset`: `-[MAAsset spaceCheck:]`, `purge*` (não chamados). Não há atributo "installed/on-disk".
- Conclusão: pela API só se obtém o declarado. O `downloadedFilesystemBytes` dos descritores já
  era não confiável (Frente G).

---

## 4. `getattrlist` na pasta negada (item 3) (A)

Sem abrir, com `FSOPT_ATTR_CMN_EXTENDED`: `ATTR_DIR_ENTRYCOUNT` = 1 (`purpose_auto`),
`ATTR_DIR_LINKCOUNT` = 1, `ATTR_DIR_DATALENGTH` = 96, `ATTR_DIR_ALLOCSIZE` = 0,
`ATTR_CMNEXT_PRIVATESIZE` = 0, `RECURSIVE_GENCOUNT` = 2. Nenhum atributo dá tamanho recursivo.
`stat` também não (`st_size` 96, `st_blocks` 0). `/.vol` não foi usado.

---

## 5. Pastas `EACCES` por diferença (fora do escopo, prova de conceito) (M)

`negada_por_diferenca.py`: dir-stats(pai) − dir-stats(irmãs) − alocado dos arquivos diretos não
comprimidos. Na raiz do Data, `.fseventsd` + `.Spotlight-V100` juntas = 2,37 GiB. Com root, em
§3c, deram 0,65 + 1,79 = 2,44 GiB: mesma ordem, horários diferentes e comprimidos fora. Só separa
quando há uma única irmã negada. Vale como linha própria no produto: "pastas negadas: X GiB
(dir-stats)", sem root.

---

## 6. Quem calcula "Apple Intelligence" no painel (item 2)

- O `Storage.appex` tem as strings "Apple Intelligence" e os caminhos
  `…/com_apple_MobileAsset_UAF_FM_GenerativeModels/` e `…/UAF_FM_Visual/`. Em `0x100067794` e
  `0x100067804` ele chama `STMSizeDirectoryWithExcludedChildURLs(url, nil)`
  (StorageManagement.framework) nessas **duas** pastas. CodeLM, Planner e Overrides ficam fora. (A)
- `STMSizeDirectoryWithExcludedChildURLs` importa `_NodeSizingV2` do `DesktopServicesPriv`, o
  dimensionamento do Finder. Ele enumera: inclui comprimidos (`SafariBackgroundImage` 233 472 =
  varredura). (A para o import e o valor; M para "enumera")
- O appex tem `com.apple.private.security.storage.MobileAssetGenerativeModels`, o entitlement que
  abre a pasta. Chamada do nosso processo (`stmsize`), a função devolve **0** para as três pastas
  restritas. (A)
- Pela mesma regra, o painel deveria mostrar ≈ GenerativeModels + Visual = 17 498 730 496 B
  (17,5 GB), se os modelos não tiverem comprimidos nem grafts. Não verificado: sem GUI, e o valor
  **não vai para o log**. Nenhuma linha em 7 dias com `intelligence`/`GenerativeModels`/`UAF` nos
  subsystems `com.apple.storage*` e `com.apple.settings.Storage`. (B)
- Imprensa: o painel mostrou de 20,67 a 30,16 GB em outras máquinas
  ([MacRumors](https://www.macrumors.com/2026/09/23/apple-intelligence-30gb-some-macs-macos-27/)).
  No teste do AppleInsider, ele caiu de 27,59 para 3,07 GB ao apagar a pasta principal pelo
  Recovery, e o Mac ganhou ~14 GB
  ([AppleInsider](https://appleinsider.com/inside/macos-27/tips/how-to-remove-apple-intelligence-models-reclaim-storage-in-macos-27),
  via resumo de busca; a página deu 403). O painel conta mais do que o espaço liberado, o que é
  coerente com enumeração contando o que dir-stats não conta (grafts/clones). (M)

---

## 7. CacheDelete (item 4) (A)

`CacheDeleteCopyItemizedPurgeableSpaceWithInfo`, só consulta, 11:07:

| Urgência | `com.apple.mobileassetd.cache-delete` |
|---|---|
| 1 | 0 |
| 2 | 146 195 545 |
| 3 | 162 607 557 |
| 4 | 4 647 588 775 |

Só por serviço; não há detalhe por asset nas chaves devolvidas. O detalhe está no log do
`AssetGC` (§3): os modelos de GenerativeModels aparecem como `never remove` ou
`recent client interest`. Nenhum `modelmanager` aparece como serviço. Não se chamou purge.

---

## 8. Web (item 5)

- Eclectic Light (2019) descreve `total_size` por pasta e o flag `INODE_MAINTAIN_DIR_STATS`, e diz
  que o macOS não expõe isso a apps
  ([Oakley](https://eclecticlight.co/2019/02/06/how-big-is-that-folder-what-happened-to-apfs-fast-directory-sizing/)).
  No macOS 27 o `fsctl` responde em todas as pastas testadas do Data; não achei fonte sobre essa
  mudança. (M)
- Nenhuma fonte encontrada usa `apfs.util -S` ou dir-stats para medir o Apple Intelligence. Os
  artigos citam o número do painel ou o Finder.

---

## 9. O que muda no produto

1. **Linha "Apple Intelligence (medido pelo APFS)"** = Σ dir-stats das pastas
   `AssetsV2/com_apple_MobileAsset_UAF_FM_*` e `UAF_IF_Planner*`. Sai de "declarado" para "medido".
   O declarado do catálogo pode ficar como conferência, com o rótulo.
2. **Linha "Pastas protegidas pelo sistema (SIP), medido pelo APFS"** = Σ dir-stats das pastas
   `EPERM`. As que respondem `EACCES` ficam como "não medidas", com o motivo.
3. A varredura não deve descer em `.AssetData` irmã de `SecureAssetData` (−2,2 GiB de contagem
   dupla aqui). Opção mais simples: nas pastas de asset, usar dir-stats.
4. Rust: `extern "C" { fn fsctl(path: *const c_char, request: c_ulong, data: *mut c_void, options: c_uint) -> c_int; }`,
   buffer `[0u8; 0x110]` e leitura de `u64` em 0x38/0x40. Erro vira "indisponível". O código é
   privado e só foi conferido no macOS 27.
5. Não usar `STMSizeDirectoryWithExcludedChildURLs` (devolve 0 sem entitlement) nem as APIs do UAF
   (declarado).

## 10. Comando para o dono

Root não ajuda: o SIP nega o `open` também a root, e dir-stats já é a medição do sistema de
arquivos. Não há comando de root que meça melhor. Para fechar a dúvida da §1.1 (grafts na
árvore), sem root:

```sh
cd research/experimentos/frenteH && ./build.sh
./dirstats /System/Library/AssetsV2/com_apple_MobileAsset_UAF_FM_* /System/Library/AssetsV2/com_apple_MobileAsset_UAF_IF_Planner*
ls -d /System/Library/AssetsV2/*/ | tr '\n' '\0' | xargs -0 python3 cmpwalk.py 2>/dev/null   # contagem dupla por pasta
```

Depois, repetir a conta §3c com a árvore **excluindo** `.AssetData`. Se o resto cair para
≈ 19,27 + 1,23 + xattrs, a conta fecha só com medições.

## 11. Incertezas

- `0xC1104A71` e o layout de 0x110 B são privados (desmontagem do `apfs.util` do macOS 27).
- Por que todas as pastas do Data têm dir-stats (o flag seria por pasta) não foi explicado. Pode
  ser herança a partir da raiz do volume (`dir-stats origin` nas strings do `fsck_apfs`). (B)
- Se os arquivos dos modelos têm comprimidos, clones ou xattrs grandes: não dá para ver sem abrir.
  A concordância com `dirSize` e com o catálogo indica que não pesam. (M)
- A natureza de graft de `.AssetData`. (M)
- O número que o painel mostra nesta máquina. (B)
- A varredura `denied.py` (scandir) recebeu `ETIMEDOUT` em 140 pastas dataless do OneDrive.
  Pode ter pedido materialização ao File Provider; o `fsctl` comprovadamente não pede.

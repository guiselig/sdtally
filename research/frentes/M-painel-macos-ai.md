# Frente M: de onde vêm "macOS 39,82 GB" e "Apple Intelligence 26,59 GB" no painel (28/09)

Máquina: macOS 27.0 (26A5416b), Apple Silicon, sem sudo. Medições de 14:15 a 14:35. Log do painel
gravado das 13:51 às 14:11 (`painel-stream.ndjson` no scratchpad).
Marcação: **(A)** medido ou lido no código; **(B)** hipótese.

## 1. Resultado

Os dois números foram reproduzidos ao byte, sem root, por `research/experimentos/frenteM/painel.swift`:

| Número da tela | Reproduzido | Fórmula |
|---|---|---|
| macOS "System size set to" | **39.823.978.496** (14:11 e agora); 39.823.986.688 às 13:52 | Σ `CapacityInUse` de **todos os volumes APFS do disco interno** (3 containers), menos o volume Data (A) |
| Apple Intelligence (i) | **26.585.645.056 = 26,59 GB** | dir-stats(GenerativeModels) + dir-stats(Visual) + Σ dir-stats(`<asset>/.AssetData`) dos cryptex enxertados (A) |

**A hipótese (B) do enunciado é falsa.** (A)
- O macOS da tela não contém nada do Apple Intelligence nem do volume Data. É a soma dos volumes
  de sistema dos 3 containers: disk3 (System, Preboot, Recovery, Update, VM), disk1 (iSC:
  iSCPreboot, xART, Hardware, Recovery) e disk2 (Apple_APFS_Recovery: Recovery, Update).
- A diferença de 16,80 GB entre a tela e o `SYS` do serviço vem de duas fórmulas diferentes para
  "sistema". Não é Apple Intelligence. O `SYS` do serviço (23.023.333.376) segue sem decomposição (§5).
- Os 26,59 GB do Apple Intelligence estão **todos no volume Data**. Deles, 9,09 GB são conteúdo de
  cryptex enxertado que o painel soma **além** das imagens `.dmg` que já contou. Ocupação física no
  Data: ~17,50 GB (dir-stats). Nada vem de Preboot nem de cryptex do sistema.

## 2. macOS = 39.823.978.496

### 2.1 O código e o log (A)

- `Storage.appex` (`/System/Library/ExtensionKit/Extensions/Storage.appex/Contents/MacOS/Storage`,
  fora do shared cache). As strings `System size set to %lld for %s` (`0x10005e5e4`),
  `Volume %s is system volume` (`0x10005e410`) e `Unable to retreive sizes on volume %s, using
  diskArbitration sizing instead` (`0x10005dcdc`) estão na mesma função, que percorre os
  containers e os volumes.
- O log `StorageShared` (nível Debug) mostra a ordem: para cada container, o appex mede cada volume
  montado com NSURL (`Volume size at mount point (NSURL) ... used: N`). Os volumes sem ponto de
  montagem são medidos pelo DiskArbitration/DiskManagement: o System selado (montado só como
  snapshot em `/`), os dois volumes de disk2 e o Recovery de disk1. No fim de cada container ele
  loga `Total used in container`.
- A "Unable to retreive sizes ... diskArbitration" que precede "is system volume" é o volume
  System (disk3s1). Não é Apple Intelligence.

### 2.2 A conta, conferida em todas as 8 passagens do log (A)

Volumes, `diskutil apfs list -plist` (`CapacityInUse`), 14:30:

| Container | Volumes (fora o Data) | Soma |
|---|---|---|
| disk1 (Apple_APFS_ISC, 524 MB) | iSCPreboot 6.295.552, xART 6.311.936, Hardware 3.948.544, Recovery 20.480 | 16.576.512 |
| disk2 (Apple_APFS_Recovery, 5,4 GB) | Recovery 2.563.006.464, Update 2.928.640 | 2.565.935.104 |
| disk3 (principal) | System 12.648.747.008, Preboot 10.818.871.296, Recovery 2.952.699.904, Update 83.341.312, VM 10.737.807.360 | 37.241.466.880 |
| **Total** | | **39.823.978.496** |

- As somas por container batem com o `Total used in container` do log: disk1 16.576.512 (ou
  16.584.704, porque Hardware oscila 8 KiB) e disk2 2.565.935.104.
- Em disk3, `Total used in container` = Data (usado − purgeable) + os 5 volumes. Às 13:52:01:
  363.793.501.262 − 326.552.034.382 (Data com purgeable) − 10.818.871.296 − 83.341.312 −
  10.737.807.360 − 2.952.699.904 = **12.648.747.008**, igual ao `CapacityInUse` do System.
- O valor logado usa os totais **da passagem anterior**:
  - 13:52:01 loga 39.823.986.688 = disk1 16.576.512 + (disk3 425.263.235.072 − Data 388.021.760.000
    da passagem de 13:51:56) + disk2 2.565.935.104.
  - 14:11:32 loga 39.823.978.496 = 16.576.512 + (425.390.882.816 − 388.149.415.936) + 2.565.935.104.
  - Na primeira passagem de cada abertura do painel (13:51:56 e 14:11:27), ele loga só o disk1
    (16.576.512 e 16.584.704), porque os outros containers ainda não tinham sido medidos.
  - Os 8 valores do log batem ao byte. O mecanismo (cache dos totais por container menos o Data) é
    (B); os números são (A).
- A diferença para o `diskutil info` da síntese §1b (37.158.125.568, faltando 2.665.861.120) é
  **Update de disk3 (83.341.312) + disk1 (16.576.512) + disk2 (2.565.935.104) = 2.665.852.928**.
  Os 8.192 B restantes são a oscilação do Hardware de disk1. O `diskutil list` não mostra o
  disk3s4 (Update), mas `diskutil apfs list -plist` e `mount` mostram.

### 2.3 System Data na tela (A para as igualdades, B para o que falta)

- `Sum of categories` de cada passagem é igual ao `Total used in container` da passagem anterior
  (13:52:06: 363.793.501.262; 13:52:11: 363.790.154.830; 14:11:53: 364.353.891.189). Logo, na
  tela, macOS + System Data + categorias = usado do appex. **O System Data da tela é o resto:**
  `SD = usado_appex − macOS_appex − Σ categorias`.
- Consequência: tudo o que está no Data e não pertence a uma categoria, **incluindo os modelos do
  Apple Intelligence**, fica em System Data. O (i) de macOS só **exibe** o Apple Intelligence
  como detalhe. Esse valor não entra nos 39,82 GB.
- Às 13:52, com as categorias da síntese §1b, o resto dá 150,34–150,48 GB, contra 150,36 na tela.
  Às 14:11, com as categorias do último bloco do serviço (14:11:57, Σ = 174.687.864.619), dá
  149,84 GB, contra 155,86 relatado. Os 6,02 GB de diferença não foram explicados (§5).

## 3. Apple Intelligence = 26.585.645.056

### 3.1 O código (A)

- `0x100067664`: calcula uma vez e guarda em `self+0xb0` a lista
  `[STMSizeDirectoryWithExcludedChildURLs(".../UAF_FM_GenerativeModels/", nil),
    STMSizeDirectoryWithExcludedChildURLs(".../UAF_FM_Visual/", nil)]` e a soma dela. A soma é
  `Array<Int64>.total` do StorageUI (`_$sSa9StorageUIs5Int64VRszlE5totalACvg`, GOT `0x1000d0d38`).
- `0x10005a078` → `0x10003b56c(x0 = self+0xa8, x1 = self+0xb0)`: se o total é ≥ 1.000.001 B
  (`0xF4241`), cria o único item "Apple Intelligence" do detalhe de macOS. CodeLM, Planner e
  Overrides ficam de fora (confirma a Frente H §6).
- O valor não vai para o log.

### 3.2 O que `STMSizeDirectoryWithExcludedChildURLs` conta, medido em pastas legíveis (A)

| Caso | STMSize | dir-stats |
|---|---|---|
| Arquivo comum, hardlink ×3, esparso, clone (`cp -c`) | alocado; hardlink 1×; esparso só o alocado; clone 2× | igual |
| Comprimido (SafariBackgroundImage) | alocado (233.472) | não conta (24.576) |
| Pasta sem dir-stats (assets de `UAF_Siri_TextToSpeech`, criados por `_nsurlsessiond`) | enumera (390.488.064) | **0** |
| **Cryptex enxertado em `.AssetData`** | **conta o conteúdo** | não conta no pai |

- Asset do Siri: STMSize(asset) 1.343.889.408 = dir-stats(asset) 688.836.608 +
  dir-stats(`.AssetData`) 655.052.800, ao byte.
- Nas 84 pastas de `AssetsV2` com STMSize > 0: **81 batem ao byte** com dir-stats(pasta) + Σ
  dir-stats(`.AssetData`). As 3 exceções são pelos motivos da tabela: comprimidos, pasta sem
  dir-stats e 1,18 MB no Siri_Understanding.
- **dir-stats lido no caminho do enxerto devolve o conteúdo do enxerto.** O caminho pode ser
  consultado mesmo dentro de pasta `EPERM`, porque o `fsctl` só faz lookup.

### 3.3 A conta nas pastas restritas (A)

- Os IDs dos 161 assets (155 GenerativeModels + 6 Visual) vêm do log `AssetGC determine` do
  mobileassetd. A lista cobre a pasta: Σ dir-stats dos assets + 2 entradas pequenas (o `.xml` do
  catálogo e o `.xml.purged`) = dir-stats de `purpose_auto`.
- `stat` e `fsctl` no caminho `<asset>/.AssetData` funcionam sem root. `ls` e `open` dão `EPERM`.

| Parcela | Bytes |
|---|---|
| dir-stats `UAF_FM_GenerativeModels` | 17.398.173.696 |
| dir-stats `UAF_FM_Visual` | 100.806.656 |
| Σ dir-stats `.AssetData` (25 enxertos montados; outros 17 `.AssetData` são pasta vazia) | 9.086.664.704 |
| **Total = painel** | **26.585.645.056 (26,59 GB)** |

- Maior enxerto: `8d6a1a11….asset/.AssetData` com 6.224.691.200 B (`model.odixpackage`). O
  `AssetData` desse asset, que contém a imagem, tem 6.445.109.248 B.
- As 17 `.AssetData` com dir-stats 0 são diretórios comuns (inode próprio, data de 24/06), ou
  seja, pontos de montagem sem enxerto. Se tivessem conteúdo que o STMSize contasse, a tela
  passaria de 26,59 GB: a folga de arredondamento é < 9,4 MB. (B, coerente com a tela)
- O enxerto é o conteúdo do `*_Cryptex.dmg` do próprio asset. No Siri, as Frentes H e L mediram
  os mesmos blocos físicos (`F_LOG2PHYS_EXT`). Aqui não dá para confirmar, porque a imagem está em
  pasta `EPERM`. Então **~9,09 GB dos 26,59 são contagem dupla**, e a ocupação física no Data é
  ~17,50 GB. (B para o GenerativeModels; A para o Siri)
- A tela era de 14:11 e a medição de 14:27–14:35. Os enxertos são montados e desmontados pelo
  sistema. Uma primeira contagem minha achou só 17, mas foi erro de script: o `head` cortou o
  laço. O número pode variar se um modelo for desmontado. (B)

## 4. Programas (`research/experimentos/frenteM/`)

| Arquivo | Faz |
|---|---|
| `painel.swift` | Reproduz os dois números. macOS: `diskutil apfs list -plist`. AI: dir-stats das 2 pastas + `.AssetData` dos assets tirados do log do mobileassetd. Não abre nem lista pasta. `swiftc -O painel.swift -o painel && ./painel 72` |
| `stmsize_ai.m` | Chama `STMSizeDirectoryWithExcludedChildURLs` e dir-stats lado a lado. Aplica `setiopolicy_np` com materialização de dataless e atime desligados |

Saída de `./painel 72` às 14:3x: `macOS (painel) = 39823978496`,
`Apple Intelligence (painel) = 26585645056`.

## 5. O que ficou sem explicação

1. **Tela "39,81" contra 39,82 calculado.** 39.823.978.496 arredonda para 39,82. Candidatos (B):
   a tela exclui disk1 (39.807.401.984 → 39,81), ou mostra o valor ajustado do gauge da passagem
   com `Difference is -136175616` (39,813). Para separar os dois é preciso ver a tela de novo.
2. **`SYS` do serviço = 23.023.333.376.** Não bate com nenhuma soma simples dos volumes
   (System + Preboot = 23.467.618.304). A Frente M não o decompôs; ele não é o que a tela mostra.
3. **System Data 155,86 às 14:11** contra o resto de 149,84 com as categorias do serviço (§2.3).
4. Onde fica o conteúdo dos enxertos de GenerativeModels: é o mesmo físico das imagens? (§3.3).
5. Os 1,18 MB do Siri_Understanding fora da fórmula.

Correção para a Frente H: **dir-stats não existe em toda pasta.** Pastas criadas fora e movidas
para dentro (`_nsurlsessiond`) têm dir-stats 0, e o pai não conta o conteúdo delas. Em
`UAF_Siri_TextToSpeech`, dir-stats dá 24.576 contra 390 MB reais. Em GenerativeModels e Visual
isso não acontece: todos os 161 assets têm dir-stats > 0 e a soma deles fecha com a pasta.

## 6. Como o produto reproduz a tela

1. **macOS:** somar `CapacityInUse` de todos os volumes dos containers APFS cujo
   `DesignatedPhysicalStore` está no mesmo disco físico do container do Data, menos o volume de
   papel `Data`. Usar `diskutil apfs list -plist` (sem root). Em Rust dá para ler o mesmo plist.
   O volume System não montado só aparece aí ou no DiskArbitration. Rótulo: "macOS (como o
   Ajustes calcula): volumes de sistema, recuperação, VM e atualização de todo o disco".
2. **Apple Intelligence (como o Ajustes mostra):** dir-stats(GenerativeModels) +
   dir-stats(Visual) + Σ dir-stats(`<asset>/.AssetData`). Os IDs dos assets vêm do log
   `AssetGC determine`; uma consulta de purgeable ao CacheDelete dispara esse log (Frente H §3).
   Mostrar ao lado a **ocupação física** = dir-stats das pastas (17,50 GB) e explicar: "o Ajustes
   conta de novo 9,09 GB de modelos montados a partir de imagens que já estão no disco".
3. **System Data como a tela:** `usado_appex − macOS_appex − Σ categorias`, com
   `usado_appex = Σ (usado − purgeable do Data, mais os outros volumes do disk3)`. Os modelos do
   Apple Intelligence estão **dentro** de System Data, apesar de aparecerem no (i) de macOS. O
   produto deve dizer isso.
4. Nunca escrever no buffer do dir-stats (`buf+4 != 0` grava no volume). IDs que não existem mais
   dão `ENOENT` e são ignorados.

## 7. Observações de método

- Uma varredura de conferência (`find -maxdepth 3` em `/System/Library/AssetsV2`, `-xdev`) rodou
  **sem** `setiopolicy_np`. `AssetsV2` não tem conteúdo de File Provider, então o risco de
  materializar era nulo, mas a regra não foi seguida nesse comando. As travessias de `stmsize_ai`
  aplicam a política.
- Arquivos de teste da semântica (`semt/`) ficaram só no scratchpad.
- Estado do disco: às 14:30, o `CapacityInUse` do Data era 364.597.014.528, contra 388,05 GB às
  13:52 (−23,5 GB). A causa não foi investigada. O macOS não mudou.

# Mapa de fontes: de onde a Apple tira cada número (01/10/2026)

Para o app, o valor do dia não importa: ele muda a cada minuto e o app vai ler de novo. O que
precisa estar certo é **a fonte**: a mesma API, log ou regra que a Apple usa, se isso foi
conferido, em qual macOS, com qual privilégio e quanto custa ler de novo.

Compilado da síntese (`2026-09-27-sintese.md`), das frentes M, O, P e Q, do código (`src/`) e
do caderno do projeto. Cada linha cita a evidência.

**Status**
- **byte**: conferido ao byte contra a tela ou o log da Apple.
- **margem**: bateu com diferença pequena e explicada (instante diferente, arredondamento).
- **implementado**: o código lê, mas ninguém comparou com a Apple.
- **hipótese**: suposição com base parcial.

**Versões**: "27" = Mac do dono (M4, 27.0). "CI" = rodou com sudo nos runners 15.7.9, 26.6.2
e 27.0. Desde 01/10 o job `fontes` abre o Ajustes > Armazenamento na VM, guarda o log e um
print da tela e compara (run 36869476887).

**Custo** medido nas rodadas do sdtally/sdc-coleta (30/09), por etapa.

## 1. Tela do Ajustes > Geral > Armazenamento

| Item | Fonte da Apple | Como lemos | Status | Versões | Privilégio | Custo |
|---|---|---|---|---|---|---|
| Usado (barra) | `StorageManagementService`: Usado = Total − Disponível − Outros volumes; linha `Disk: TOT/USE/AVL/PRG/OTH/SYS` no log `com.apple.storage` | log (`/usr/bin/log show`), só se o painel foi aberto há < ~21 h | byte (74 de 74 cálculos, síntese §1) | 27; a linha `Disk:` existe nos 3 (CI) | admin, sem sudo | < 1 s |
| Usado do cabeçalho da tela ("X GB de Y GB usados") | **Total − `NSURLVolumeAvailableCapacityForImportantUsageKey`** do volume `/` no instante em que o painel calcula; o livre da barra é Total − usado | `URLResourceValues` (sem privilégio) | margem nos 3 (CI run 36877609309): 296,54 / 240,17 / 112,45 GB caem dentro da faixa de Total − ImportantUsage medida a cada 15 s durante a carga; Total − livre (sem purgeable) nunca chega ao valor da tela. Byte não dá no CI: o disco do runner muda ~1 GB por minuto | 15, 26, 27 | nenhum | ~0 s |
| Disponível e "o macOS libera sozinho" | Disponível = livre + purgeable; purgeable = `NSURLVolumeAvailableCapacityForImportantUsageKey` − livre (urgência 3) | `CacheDeleteCopyItemizedPurgeableSpaceWithInfo` por serviço (soma = total) | byte no 27 (soma dos serviços = total, síntese §2.2) | 27; CI roda nos 3 | nenhum | 0,1 s |
| macOS | extensão `com.apple.settings.Storage` (`StorageShared`): Σ `CapacityInUse` dos volumes APFS do disco interno, menos o Data **e menos o container ISC** (xART, Hardware, iSCPreboot; "Volume should not be shown in the UI"). O log `System size set to` inclui o ISC; a linha da tela não (02/10: 38,73 GB na tela = fórmula − 16,58 MB) | `diskutil apfs list -plist` | byte no 27 (igual ao log `System size set to`, Frente M); **tela nos 3** (CI: 19,65 / 21,13 / 25,87 GB no print = reproduzido; o `SYS` do serviço dá 18,06 / 19,24 / 23,79) | 15, 26, 27 | nenhum | 0,8–1,7 s |
| Apple Intelligence (no (i) de macOS) | Σ `STMSizeDirectoryWithExcludedChildURLs` em `AssetsV2/…UAF_FM_GenerativeModels` e `…Visual`; soma também o `.AssetData` enxertado (contagem dupla da Apple) | dir-stats (`fsctl 0xC1104A71`, buffer zerado) nas mesmas pastas + `.AssetData` | byte no 27 em 28/09 (26,59 GB nos dois); físico ~17,50 GB | só 27 (no CI os modelos não estão baixados) | nenhum | < 1 s |
| Dados do Sistema | **a tela**: (Total − ImportantUsage) − Σ categorias − macOS da tela (CI, 01/10). A fórmula antiga, (usado do Data − purgeable) − Σ categorias, dá ~0,75 GB a menos no runner 27: hipótese, os metadados do container entram no Total − ImportantUsage e não em nenhum volume. **O log do serviço** dá outro número: o macOS do serviço (`SYS`) é menor que o da tela, e a diferença fica no Dados do Sistema dele | hoje: a linha `STMVolumeCategory … System Data` do log, **que é o número do serviço, não o da tela** (ver §6) | fórmula da tela: margem (150,34–150,48 contra 150,36, Frente M §2.3); 6,02 GB sem explicação às 14:11. A linha `STMVolumeCategory` existe nos 3 (CI). **Print rolado do CI (runs 36873816017 e 36877609309):** soma das linhas = cabeçalho nos 3, então **tela = (Total − ImportantUsage) − Σ categorias − macOS da tela**. O cabeçalho não é o `USE` do serviço; a diferença de +1,40 GB no 27 do primeiro run era instante, não regra | 27 | admin | < 1 s (log) |
| As 15 categorias | uma extensão `com.apple.storagemanagement` por categoria (§2) | Frente Q em Rust, ainda fora da lib | ver §2 | 27 | FDA para 6 delas | 48–54 s sozinhas; ~5 s se vierem da travessia |

**Linha da própria tela (02/10, Mac do dono):** com o painel aberto, o processo do Ajustes grava a
cada ~5 s `Sum of categories: X, available: Y, capacity: Z` (subsystem `com.apple.settings.Storage`,
category `StorageUI`, fica gravado). X é o "usado" do cabeçalho ao byte; Dados do Sistema da tela =
X − Σ outras linhas. É a fonte mais direta do número da tela. Y não é ImportantUsage nem
OpportunisticUsage no Mac do dono (livre + 22,77 GB, com 42,93 GB de purgeable); a origem exata
de Y segue em aberto. Achado de 02/10 (log, Mac do dono): uma instância do Ajustes mantém
Y = livre + purgeable **capturado na abertura do painel** (43,09 GB fixos de 08:27 a 12:20); reaberto às
12:20:55, mostrou Y = livre + purgeable e, 5 s depois, Y = só o livre, com X subindo 43 GB. Ou seja,
o Dados do Sistema da tela muda dezenas de GB conforme a instância do painel conta ou não o
purgeable (66 GB) como disponível, sem o disco mudar. Y ≈ Z − X vale por construção (a linha só
sai quando não fecha por uns MB). `System size set to` (macOS da tela) é nível debug: só aparece em
`log stream --level debug` no momento em que o painel carrega; igual ao byte à fórmula em 02/10.
`SpaceAttribution.framework` (27) tem a atribuição da própria Apple (`rawSystemDataSize`,
`systemDataDetails`), mas recusa processo sem entitlement (erro 4099).

Regras de leitura do log: usar o último bloco completo (o valor muda enquanto o painel carrega);
retenção < ~21 h no 27 (caderno 29/09); no zsh `log` é builtin, usar `/usr/bin/log`. Pedir o
cálculo direto ao serviço não dá: o XPC exige o entitlement restrito `com.apple.storage-data`.

## 2. Categorias (Frentes P e Q)

| Categoria | Fonte da Apple | Status no 27 | No produto | FDA |
|---|---|---|---|---|
| Aplicativos | `NSMetadataQuery` de bundles + `STMSize` de bundle, container e App Support, com filtros | margem (3,6 MB entre rodadas) | travessia + Spotlight | não testado sem FDA |
| Documentos | `STMSize(~)` sem `~/Library`, Mesa/Documentos (se iCloud ligado), mídia e apps | byte P×Q; sem tela no mesmo minuto | travessia | não |
| Música, TV | Σ `FileSize` gravado em `Library.musicdb` / `.tvdb` (não o tamanho da pasta) | byte contra o log de 28/09 | leitura do banco | não |
| Fotos | `PLFileUtilities calculateTotalSizeOfFilesAtPath:` (lógico − purgeable) | A (reproduzido) | **framework privado** | sim (sem FDA trava > 2 min) |
| Livros, Podcasts, Mensagens | `BKLibrary`, `MTLibrary`, `chat.db` | byte (Mensagens contra o log); Livros/Podcasts = 0 aqui | leitura do banco | sim |
| Lixo, GarageBand, iOS, Desenvolvedor | caminhos fixos + `STMSize` | byte (Developer contra o log); CI: igual ao byte nos 3 depois da regra por versão (§6.6) | travessia | não |
| Mail | `STMSize(~/Library/Mail)` − purgeable do `com.apple.MailCacheDelete` | A | travessia + CacheDelete | sim |
| Outros usuários | `/Users/Shared`, `Deleted Users`, outras homes | byte (fixos) | travessia; outra home exige root | não |
| iCloud Drive | parte A: banco FPFS (`metadata_physical_size` não despejável); parte B: `BRSyncProxy computePurgeableSpaceWithReply:` | A: reproduzida; **B: hipótese** (oscila 1,66 GB → 0,22 GB em 19 min) | banco FPFS; B fica fora | sim |

No CI (01/10) a tela mostra exatamente o valor do log para Aplicativos, Desenvolvedor e
Documentos nos 3 macOS; a extensão é a fonte da tela. frenteq contra o log: Documentos dentro
de ±0,06 %; Aplicativos 15 MB abaixo no 15, **368 MB abaixo no 26 (estável em 2 runs)** e no 27
342 MB num run e 16 MB no outro, com o frenteq igual ao KB nos dois: o lado da Apple conta algo
transitório. A lista por app (`FRENTEQ_POR_ITEM=1`) não mostra nenhum app pulado desse tamanho.

`STMSize` = Σ `st_blocks × 512` com hardlink uma vez e clone contado de novo: conferido ao byte
em 5 pastas e num teste sintético (Frente Q §4). Por isso 7 categorias saem da travessia sem
framework privado. Não testado: dataless, xattr grande, comprimido.

## 3. Dentro de Dados do Sistema (a conta do volume)

| Parcela | Fonte | Status | Versões | Privilégio | Custo |
|---|---|---|---|---|---|
| Usado do volume Data | `getattrlist` `ATTR_VOL_SPACEUSED` (não `statfs`, que dá o container) | byte (container fecha com +153 MiB, Frente D) | 12+; CI nos 3 | nenhum | ~0 s |
| Árvore de arquivos | `getattrlistbulk`: `ALLOCSIZE`, `PRIVATESIZE`, `CLONEID` + `EF_SHARES_ALL_BLOCKS`, `LINKCOUNT`/`FILEID`, `SF_DATALESS`, `DEVID` | conta fecha: resto 0,32 % com sudo (aceite 30/09) | CI nos 3 | root para ~8 GiB de pastas do sistema | **26–38 s** |
| Xattrs grandes | `listxattr` + `getxattr(NULL,0)`; > 3.804 B ficam fora do `ALLOCSIZE` | byte numa imagem APFS (fixture) | CI nos 3 | nenhum | dentro da travessia (+40 %) |
| Snapshots locais | `fs_snapshot_list` + `fsctl(0xC0184A13)` por faixa de XID (o mesmo do TimeMachine.framework) | margem/byte no 27 (Frente B) | **só 27**: o CI não consegue criar snapshot | nenhum | 1,9–15 s (mais lento com backup) |
| Apagados ainda abertos | `lsof -b -w +L1` acha; `proc_pidfdinfo` dá o alocado real | implementado; parcial por natureza (mapeados ilegíveis; em datavault, como os `.csstore` do chronod em `com.apple.LaunchServices.dv`, o kernel zera o stat da região sem `com.apple.rootless.datavault.metadata`, que só o lsof da Apple tem: o SIZE lógico do lsof vai no motivo, fora da soma) | CI nos 3 | root para todos os processos | 0,5 s |
| Pastas protegidas (SIP, EPERM) | dir-stats `fsctl 0xC1104A71`, buffer zerado (o do `apfs.util -S`) | byte no 27: 3 fontes em 0,05 % (Frente H); em 01/10, 85 de 99 pastas de AssetsV2 iguais ao byte à travessia e 8 iguais depois de tirar o que o dir-stats **não conta: arquivo comprimido (`UF_COMPRESSED`) e conteúdo enxertado (`.AssetData`)** (6 só com sudo). **CI com sudo: 49/50/51 pastas iguais ao byte e 3 explicadas por comprimido no 15/26/27, nenhuma sem explicação** | 15, 26, 27 | nenhum | 0,4–0,8 s |
| Enxertos de cryptex | pasta com `cloneid ≠ fileid` = raiz de enxerto; blocos são os do `.dmg` vizinho (`F_LOG2PHYS_EXT`) | byte (7/7, zero falso positivo em 377 mil pastas, Frente L) | só 27; enxerto MobileAsset existe a partir do 15 | nenhum | dentro da travessia |
| Metadados do APFS | seletor 0x1e do `AppleAPFSUserClient` (árvores vivas) | medido em 27/09 (1,23 GiB); não está no produto | 27 | nenhum | — |
| iCloud baixado, `com.apple.bird`, dataless | travessia + `SF_DATALESS`; estado de sync: `NSURLUbiquitousItem*`, `brctl`, `fileproviderctl` | implementado (dataless contado, não descido) | 27 | nenhum | dentro da travessia |

## 4. O que o app precisa para atualizar sem refazer tudo

| Fonte | Custo | Pode ler a cada… | Gatilho de mudança |
|---|---|---|---|
| Usado, disponível, purgeable, macOS, snapshots (lista) | < 2 s | minuto | nenhum necessário (barato) |
| Tamanho dos snapshots | 2–15 s | após backup | `tmutil status` / fim de backup |
| Log do painel (Dados do Sistema do serviço) | < 1 s | quando o Ajustes abrir | aparece linha nova no log |
| Apple Intelligence, pastas protegidas (dir-stats) | < 1 s | minuto | — |
| Apagados abertos | 0,5 s | minuto | — |
| Árvore inteira e categorias | 30–50 s | **não**: só na primeira vez | **FSEvents**: `FSEventStreamCreateRelativeToDevice` funciona sem root e cobre o volume (Frente I §5); refazer só as pastas que mudaram |

**FSEvents testado em 01/10 (Frente R, `research/experimentos/frenteR/`)** no `~/Library` do
dono (447 mil arquivos, 1 h de uso normal com iCloud ativo): o id do evento é pego antes da
travessia; no fim, `FSEventStreamCreate` desde esse id devolveu 2.178 eventos de pasta (737
pastas, nenhum `MustScanSubDirs`, nenhuma perda) e só essas pastas foram relidas.
- Incremental em **1,9 s** contra **16,0 s** da travessia nova; diferença **−37 MB (−0,026 %)**, 1 arquivo.
- As pastas com evento que divergiram (iCloud `session/i`, `wharf/ingest`, banco do
  FileProvider, telemetria do RustRover) mudaram depois da releitura: corrida, não falha.
- **Limite real:** 3 pastas sem nenhum evento cresceram 4,2 MB (`Knowledge`, `Keychains/<uuid>`,
  `usernoted/db2`). São bancos SQLite que o processo mantém abertos; o kernel só gera "conteúdo
  modificado" ao fechar o arquivo. O app precisa de uma travessia completa de tempos em tempos
  (ou reler o tamanho dos bancos abertos) para pegar esse crescimento.

## 5. Versões (Frente O + CI)

- **Provado nos 3 (CI com sudo):** travessia, `ATTR_VOL_SPACEUSED`, CacheDelete itemizado,
  `diskutil apfs list -plist`, `lsof`, conta fechando (resto < 1 GB).
- **Provado nos 3 pelo job `fontes` (01/10):** dir-stats (mesma semântica, §3), linhas de log
  `Disk:` e `STMVolumeCategory`, fórmula do macOS contra a tela, categorias da tela = log,
  cabeçalho da tela = Total − ImportantUsage e Dados do Sistema da tela = cabeçalho − Σ linhas,
  símbolos `STMSizeDirectoryWithExcludedChildURLs`, `PLFileUtilities` (mesma assinatura
  `v40@0:8@16B24B28@?32`), `CacheDeleteCopyItemizedPurgeableSpaceWithInfo`, `fileproviderctl`.
- **Muda por versão:** Desenvolvedor só conta `/Library/Developer/CoreSimulator` no 27 (§6.6).
- **Só no 27:** fsctl de snapshot (`0xC0184A13`), regra de enxerto, linha `System size set to`
  (ausente nos 3 runners, inclusive no 27: depende de algo da sessão do dono), fórmula do Apple
  Intelligence, as categorias que o runner não tem (Fotos, Mail, Mensagens, Música, iCloud…).
- Apple Intelligence só existe a partir do 15.1; `…Visual` entra na tela a partir do 15.4 (B).

## 6. Erros que este mapa achou

1. **"Dados do Sistema (tela de HH:MM)" no sdtally é o número do serviço, não o da tela.** A
   linha `STMVolumeCategory … System Data` sai do `StorageManagementService`; a tela tira dele a
   diferença entre o macOS dela e o `SYS` do serviço (síntese §1b: 166,69 no log, 150,36 na
   tela). Correção imediata: rotular como "log do serviço". Número da tela: fase 2, por
   (usado do Data − purgeable) − Σ categorias, ou log − (macOS da tela − `SYS`) no mesmo bloco.
2. **Escopo do macOS:** o código soma volumes sem papel (criados pelo usuário) e o System/Preboot
   de outra instalação no mesmo disco; a tela mostra esses à parte (`DISK_OTHER_VOLS`, Frente O
   §2.3). Só muda em Mac com volumes extras.
3. **dir-stats subestima pasta protegida com arquivos comprimidos:** ele não conta
   `UF_COMPRESSED` (Ferramentas de Linha de Comando: 0,73 contra 2,04 GB). Nos modelos do Apple
   Intelligence quase não há comprimido (dir-stats 26,59 GB = `STMSize` da Apple), mas em outra pasta
   protegida a diferença fica em "sem explicação". Nenhum número sai errado para cima.
4. **`PLFileUtilities` é `void`** (`v40@0:8@16B24B28@?32` no 27), não `BOOL`: o frenteq lia lixo
   como retorno; corrigido. O resultado sempre veio pelo bloco.
5. **Purgeable total por `ImportantUsage`** não está implementado: hoje só a soma itemizada do
   CacheDelete. Serve de conferência barata (PLANO, Fase 1 "ficou de fora").
6. **Desenvolvedor mudou no 27 e o frenteq usava a regra do 27 em todas as versões.** O
   binário da `DeveloperStorageExtension` do 15.7.9 e do 26.6.2 não tem `/Library/Developer/CoreSimulator`
   nem o item "Simulator Support"; o do 27.0 tem (strings, run 36877609309). Por isso, nos runners
   15 e 26, o log é igual ao byte às Ferramentas de Linha de Comando e o frenteq contava 55 e 38 GB
   a mais (`CoreSimulator/Caches`; as montagens dos runtimes não entram no `STMSize`). No 27 o log
   é igual ao byte a CLT + CoreSimulator. Corrigido: o frenteq só soma `/Library/Developer/CoreSimulator`
   no 27 ou mais.
7. **iCloud sem conta trava 120 s e derruba a fórmula da tela no frenteq:** nos 3 runners
   `fileproviderctl` estoura o tempo e Dados do Sistema sai "não medida". Sem conta do iCloud a
   categoria tem de ser 0 sem chamar o `fileproviderctl`.

## 7. O que falta confirmar, por ordem de impacto no app

| # | O quê | Como | Precisa do dono? |
|---|---|---|---|
| 1 | Fontes "só 27" no 15 e no 26 | **feito em 01/10** (§5, §6.6); fica em aberto a diferença de Aplicativos no 26 (368 MB) | não |
| 2 | Dados do Sistema da tela pela fórmula, no mesmo minuto | **feito em 02/10:** a tela grava X (usado) no log; macOS ao byte. Falta a origem do "available" Y | não |
| 3 | fsctl de snapshot fora do 27 | não sai no CI (tmutil exit 73); só Mac real com 15/26 | amigos |
| 4 | FSEvents mantém a árvore igual à travessia | **feito em 01/10** (§4): −0,026 % em 1 h; arquivo aberto que cresce não gera evento | não |
| 5 | iCloud Drive parte B | ler o argumento de `computePurgeableSpaceWithReply:` e o fsctl `0xC0284A20` do bird | não |
| 6 | `lsof -b` durante backup | repetir o sdtally com sudo durante um backup do TM | sim |

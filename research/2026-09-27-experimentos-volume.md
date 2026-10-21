# Experimentos: varredura paralela e conta do volume Data — 2026-09-27

Código: `research/experimentos/probe/src/bin/walk_par.rs`. macOS 27, Apple Silicon, 10 núcleos
(4 de performance), usuário comum, **sem sudo**. Correção posterior: o processo herda Full Disk
Access do app que o lançou (`~/Library/Mail` e `~/Library/Safari` legíveis); o `TCC.db` continua
ilegível mesmo assim, então ele **não** serve para detectar FDA (ao contrário do `docs/03`). Backup do Time Machine
rodando (fase Copying) durante todas as medições.

---

## 1. A varredura paralela

Pool simples: pilha de pastas num `Mutex`, `Condvar`, contador de pendentes, N threads, cada
uma com buffer próprio de 256 KiB para o `getattrlistbulk`. Totais por thread, juntados no fim.
Hardlinks (por `fileid`) e clones completos (por `cloneid`) são deduplicados globalmente depois
da varredura.

### Validação

| Teste | Serial (`walk`) | Paralelo (`walk_par`) |
|---|---|---|
| Pasta com original, clone, hardlink, clone parcial, esparso | 0,60 GiB + 0,20 clone | 0,60 GiB, +1 hardlink, +1 clone |
| `~/Library` alocado único | 106,22 GiB | 106,21 GiB |

**Bug encontrado e corrigido:** um inode com hardlinks que também é clone completo era contado
nas duas deduplicações. Correção: primeiro por `fileid`, e o inode resultante entra na
deduplicação por `cloneid` como uma ocorrência. Teste mínimo que pega o bug: `orig` + `ln orig
hard` + `cp -c orig clone` tem de dar 1× o tamanho.

### Velocidade

| Alvo | Arquivos | Threads | Tempo |
|---|---|---|---|
| `~/Library` | 456 mil | 1 (serial) | 23 s |
| `~/Library` | 456 mil | 10 | 6,7 s |
| `/System/Volumes/Data` inteiro | 1,89 milhão + 354 mil pastas | 4 | 23,4 s |
| idem | idem | 10 | 15,5–16,1 s |
| idem | idem | 16 | 16,6 s |

Ponto ótimo nesta máquina: **~10 threads**. Acima disso não melhora (I/O e backup concorrente).

---

## 2. Achado de segurança: pastas dataless de File Provider

Na primeira varredura, 140 pastas deram `ETIMEDOUT` (errno 60) no `getattrlistbulk`. Todas em
`~/Library/CloudStorage/` (OneDrive e Google Drive), todas com a flag `dataless` na **pasta**
(`ls -lOd` mostra `compressed,dataless`).

**Listar uma pasta dataless pede ao File Provider para materializar a listagem.** O processo
fica esperando o provedor até dar timeout. `du`, `dua`, `find` e as primeiras varreduras deste
projeto fizeram esse pedido.

Medido em `~/Library/CloudStorage` (7.449 arquivos, 1.404 pastas):

| Modo | Resultado |
|---|---|
| Padrão do sistema, entrando nas pastas dataless | 140 × `ETIMEDOUT`, 1,6–3,2 s |
| `setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS, IOPOL_MATERIALIZE_DATALESS_FILES_OFF)`, entrando | 140 × `EDEADLK` (errno 11) na hora, 0,0 s |
| Política OFF **e** sem descer em pasta com `SF_DATALESS` | 0 erros, 140 pastas contadas como "dataless não abertas" |

**Regra para o projeto:** chamar `setiopolicy_np(...OFF)` no início do processo (rede de
segurança) **e** não descer em pasta com `SF_DATALESS` (evita até a tentativa). O relatório
mostra quantas pastas dataless não foram abertas; o conteúdo delas está na nuvem e ocupa 0 local.

Constantes (de `<sys/resource.h>`, ausentes no crate `libc`): `IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES = 3`,
`IOPOL_SCOPE_PROCESS = 0`, `IOPOL_MATERIALIZE_DATALESS_FILES_OFF = 1`.

---

## 3. A conta do volume Data (medida)

| Linha | GiB | Origem |
|---|---|---|
| Volume usado | **364,67** | `statfs`: `(f_blocks − f_bfree) × f_bsize` |
| Alocado único medido na árvore | **272,79** | soma de `ATTR_FILE_ALLOCSIZE`, hardlink e clone uma vez |
| **Não atribuído** | **91,88** | diferença exata das duas linhas acima |
| Hardlinks repetidos (fora da soma) | 10,98 | 27.931 entradas extras |
| Clones completos repetidos (fora da soma) | 24,02 | 81.157 entradas extras |
| Privado (libera na hora ao apagar tudo) | 2,64 | soma de `PRIVATESIZE` |
| Preso em snapshot (arquivos sem clone) | 217,08 | soma de `ALLOCSIZE − PRIVATESIZE` quando sem `EF_MAY_SHARE_BLOCKS` |
| Marcado purgeable por arquivo | 52,03 | `EF_IS_PURGEABLE` |
| Dataless | 2.802 arquivos, 51,80 GiB aparentes, 0 local | `SF_DATALESS` |
| Esparsos | 360,01 aparentes, 17,77 alocados | `EF_IS_SPARSE` |

Erros: 68 × `EPERM` (TCC), 184 × `EACCES` (permissão Unix; quase todos `/private/var/folders`
de outros usuários/daemons e `/private/var/db/*`), 1 × `ERANGE`. 65 pontos de montagem de outros
volumes não atravessados.

### O que está dentro dos 91,88 GiB não atribuídos

Nenhum destes itens tem medição ainda; ficam como hipóteses a medir, **não** como rótulo:

1. **Blocos que só existem em snapshot** (arquivos apagados ou reescritos depois do snapshot).
   Com um backup em andamento e snapshots de 20/09 e de 26/09 na máquina, é o candidato natural.
   Medir: Frente B (API de tamanho de snapshot) ou repetir após o Time Machine remover os
   snapshots, sem forçar nada.
2. **Pastas negadas** (TCC e permissão Unix): 252 pastas não lidas. Medir: repetir com Full Disk
   Access e, se o dono autorizar, com root.
3. **Metadados do APFS** (árvores, bitmaps, journal). Medir: Frente D.
4. **Arquivos apagados mas abertos.** Medir: `lsof +L1` / `proc_pidinfo` (Frente D).

### Observações

- **"Preso em snapshot" = 217 GiB** logo após um snapshot do backup: quase todo arquivo compartilha
  blocos com o snapshot recém-criado. Não é espaço extra; é o que **não** seria liberado se o
  arquivo fosse apagado agora. O relatório precisa dizer isso com todas as letras.
- **`EF_IS_PURGEABLE` por arquivo (52 GiB) ≠ purgeable do volume (3,3 GiB da API).** Frente F
  cruzou arquivo a arquivo: no iCloud a flag significa **"já enviado ao iCloud"** (29.539 de 30.744
  enviados têm a flag; nenhum não enviado tem). Com "Otimizar armazenamento" desligado, o bird
  informa `purgeableSpace=0` e nada disso entra no purgeable do volume. Não somar um com o outro.
- **Clones valem 24 GiB no volume inteiro**, 21,4 só em `~/Library` (backup do WhatsApp no
  CloudKit). Qualquer ferramenta sem deduplicação de clone erra por essa margem aqui.

---

## 3b. Correção da conta: `statfs` mede o container, não o volume

**A tabela da seção 3 está errada na primeira linha.** No APFS, `statfs` devolve tamanho e livre
do **container** inteiro; `(f_blocks − f_bfree) × f_bsize` = 364 GiB inclui System, Preboot,
Recovery e VM. O uso só do volume Data é `getattrlist` com `ATTR_VOL_SPACEUSED` (igual ao
"Volume Used Space" do `diskutil info`, conferido ao byte). A Frente D chegou à mesma conclusão
independentemente.

Conta refeita (27/09, tarde, backup ainda rodando):

| Linha | GiB | Origem |
|---|---|---|
| Container usado | 364,01 | `statfs` |
| **Volume Data usado** | **330,17** | `ATTR_VOL_SPACEUSED` |
| Alocado único na árvore | 272,99 | varredura |
| **Não atribuído** | **57,18** | diferença exata |

Decomposição do não atribuído com as ferramentas das frentes B e D:

| Parcela | GiB | Medição |
|---|---|---|
| Blocos só em snapshot (7 snapshots do Time Machine) | 22,89 | `fsctl(0xC0184A13)` faixa de xid completa, `snapsize` (Frente B); bate com `com.apple.TMCacheDelete` do CacheDelete |
| Arquivos apagados e ainda abertos | ~3,05 | `lsof +L1` sem sudo (Frente D); o programa com `libproc` só vê 0,16 porque o kernel zera metadados em data vault |
| Pastas que nem FDA abre (SIP / data vault) | não medível | 15 × `EPERM`: modelos do Apple Intelligence (`AssetsV2/com_apple_MobileAsset_UAF_FM_*`, `UAF_IF_Planner*`), `/private/var/db/AppleIntelligencePlatform`, `CoreDuet/Knowledge`, `DifferentialPrivacy`, `ConfigurationProfiles/Store`, `/private/var/OOPJit`, `/private/var/containers/Data/ProtectedSystem`, caches `aned`/`aneuserd`/`amsengagementd` |
| Pastas com permissão Unix de outros donos | não medido | 184 × `EACCES` (`/private/var/folders` de daemons, `/private/var/db/*`); root resolveria |
| Metadados do volume APFS | não medível sem root | Frente D |
| **Resto após as parcelas medidas** | **~31,2** | 57,18 − 22,89 − 3,05 |

Tentativa que falhou: `ATTR_DIR_ALLOCSIZE` / `ATTR_DIR_DATALENGTH` numa pasta negada, para ler o
tamanho sem abrir. O APFS devolve **0** em todas as pastas testadas (dir-stats não mantido:
`ATTR_CMNEXT_RECURSIVE_GENCOUNT` = 0). Programa: `experimentos/dirstat.c`.

### Atributos estendidos: espaço real que `du` não vê

Teste controlado: um xattr de 500 MB num arquivo de 3 bytes aumentou o `ATTR_VOL_SPACEUSED` do
volume em **503 MiB**, enquanto `st_blocks`, `du` e `ATTR_FILE_ALLOCSIZE` continuaram em 4 KiB.
Xattrs acima de 3.804 bytes (limite de valor embutido no inode, APFS Reference) ficam num fluxo
próprio que nenhum desses atributos conta. Exceção: `com.apple.ResourceFork` de arquivo comprimido
(decmpfs) **entra** no `ALLOCSIZE` (conferido: 6.948.215 bytes nos dois).

Medição: `listxattr` + `getxattr(NULL, 0)` (só o tamanho, sem ler o valor), apenas em objetos sem
`EF_NO_XATTRS`. Volume inteiro: **0,49 GiB** de valores em 743.773 objetos; 0,45 GiB acima do
limite embutido, 0,43 GiB deles em `~/Library/Containers`. Custo: varredura foi de 16 para 36 s.
Não explica o resto aqui, mas é espaço real que a ferramenta passa a medir (opcional, por custo).

### Pastas legíveis só por root (184 × `EACCES`)

Candidatas conhecidas a ocupar GB, todas na lista de negadas: `/.DocumentRevisions-V100`
(versões de documentos), `/.Spotlight-V100` (índice), `/.fseventsd`, `/private/var/db/biome`,
`analyticsd`, `coreml`, `knowledgegraphd`, `modelmanagerd`, `/private/var/root`,
`/Library/Application Support/Apple/AssetCache/Data`, `/macOS Install Data/Locked Files`, e 80
pastas de `/private/var/folders` de daemons. **Experimento decisivo pendente:** o dono roda a
varredura uma vez com `sudo` (só leitura) para medir essa parcela; o que sobrar depois disso é
SIP/data vault (Apple Intelligence) + metadados APFS.

### O número do próprio painel (log do `StorageManagementService`)

Último cálculo gravado: 26/09 às 21:58, **antes** de o backup ser parado (os 96 GiB de snapshots
ainda existiam). Lido com `/usr/bin/log show` sem sudo (Frente A):

| Campo | Bytes |
|---|---|
| Total | 494.384.795.648 |
| Usado | 368.472.913.278 |
| Disponível (livre + purgeable) | 125.307.230.850 |
| Purgeable | 3.328.252.546 |
| Outros volumes | 604.651.520 |
| macOS | 23.023.312.896 |
| Aplicativos | 75.530.461.184 |
| Documentos | 41.252.458.496 |
| Música | 36.429.056.548 |
| Fotos | 10.169.778.544 |
| Developer | 2.050.179.072 |
| Mail | 849.178.624 |
| iCloud Drive | 252.719.104 |
| **Dados do Sistema ("Other")** | **178.914.669.322** |

Identidade conferida: Usado − macOS − soma das extensões = Other, ao byte (Frente A, 74 de 74
blocos). Próximo passo: o dono abre o painel uma vez, sem backup rodando, e a ferramenta compara
o "Other" gravado com a própria conta na mesma hora.

## 4. Maiores fatias medidas (prefixo de 3 níveis)

| Alocado (GiB) | Caminho |
|---|---|
| 105,53 | `Users/<usuário>/Library` |
| 35,68 | `Users/<usuário>/Music` |
| 9,65 | `Users/<usuário>/Pictures` |
| 9,12 | `System/Library/AssetsV2` (2,22 privados: assets sem snapshot) |
| 8,54 | `Users/<usuário>/.colima` |
| 6,66 | `opt/homebrew/Cellar` |
| 6,00 | `Users/<usuário>/My Drive` |
| 4,28 | `Users/<usuário>/.npm` |
| 3,95 | `private/var/db` |
| 3,72 | `Applications/Xcode.app` |
| 3,16 | `Users/<usuário>/.ollama` |
| 2,88 | `Users/<usuário>/.whisper-models` |
| 2,00 | `private/var/vm` |
| 1,18 | `private/var/folders` (o que foi legível) |
| 0,50 | `Library/Updates/<id>` |

---

## 5. Próximos experimentos

1. Repetir a conta **sem backup rodando** e depois que o Time Machine descartar os snapshots
   locais sozinho (sem `tmutil`), para isolar o peso dos snapshots nos 91,88 GiB.
2. As 252 pastas negadas foram negadas **com** FDA: são permissão Unix de outros usuários/daemons
   e SIP. Medir quanto pesam exige root (só com autorização do dono) ou fica como "não lido".
3. Integrar os resultados das Frentes B (tamanho de snapshot, CacheDelete por serviço) e D
   (metadados APFS, arquivos apagados e abertos).
4. Medir contenção do `Mutex` (perfil com `samply` ou contagem de espera) e só então trocar por
   deque com roubo de trabalho (técnica do `dua-core`). Contenção ainda não medida; o ganho parar
   em 10 threads pode ser I/O, backup concorrente ou o `Mutex`.

# Frente Q: as 15 categorias do Ajustes em Rust (29/09)

Máquina: macOS 27.0, M4, sem sudo, terminal com FDA. Spike em
`research/experimentos/frenteQ/` (crate isolado, `[workspace]` vazio; deps só `libc` e
`serde_json`; FFI à mão para libobjc, Foundation/CoreServices, libsqlite3, CommonCrypto, zlib e,
por `dlopen`/`dlsym`, funções privadas **só de leitura**). Regras de cada categoria: Frente P.

## 1. Resultado

- **As 15 categorias saem em Rust com o mesmo número da Frente P.** 13 iguais ao byte na mesma
  rodada; Aplicativos difere 3,6 MB (0,004 %) e iCloud Drive 0,18 MB, os dois por mudança real do
  disco entre as rodadas (repetidas em seguida, iCloud Drive deu igual ao byte).
- **Dados do Sistema = (usado do Data − purgeable) − Σ categorias:** Rust
  367.448.952.832 − 38.051.837.426 − 176.603.180.261 = **152.793.935.145** (estado "parcial
  (teto)"), Frente P no minuto seguinte 152.699.026.729. A diferença (95 MB) é quase toda a parte
  B do iCloud Drive, que a Frente P soma como estimativa e o Rust deixa fora (regra "só medição").
- **Tempo total: 48–54 s em Rust, 45–51 s na Frente P.** Não há ganho: 85 % do tempo é o
  `STMSizeDirectoryWithExcludedChildURLs` da Apple (Aplicativos 23–26 s, Documentos 13–16 s,
  Desenvolvedor 5–6 s), igual nos dois.
- **Achado que muda a integração:** o `STMSize` é, ao byte, Σ `st_blocks × 512` com hardlink
  contado uma vez e clone contado de novo (5 pastas reais e um teste sintético, §4). O walker da
  lib já mede isso. As 8 categorias de "caminhos + STMSize" viram baldes da travessia, sem
  framework privado e sem os ~45 s de segunda travessia.
- **Framework privado indispensável:** só Fotos (`PLFileUtilities`, conta tamanho lógico menos
  purgeable) e o desconto do Mail (`CacheDelete`, que a lib já usa).
- **Root:** só "Outros usuários", e só quando existe outra conta (aqui não existe: medida).
- **Correção da Frente P:** Livros e Música/TV davam 0 se o banco mudasse de nome em outra versão.
  Agora "pacote/pasta da biblioteca existe, banco não" sai "não medida"; Livros acha o
  `BKLibrary-*.sqlite` pelo prefixo.

## 2. Tabela (mesma categoria, Frente P e Rust rodados em seguida)

Bytes. Tempo em segundos (Frente P / Rust); Python inclui a partida do interpretador.
Frente P rodada com os bancos lidos **no lugar** (`mode=ro`): `bancos.py`, `icloud.py` e
`livros.m` copiam os bancos para um temporário, o que esta rodada não podia fazer (§6).

| Categoria | Frente P | Rust | Diferença | Tempo P / Rust |
|---|---:|---:|---:|---:|
| Aplicativos | 81.773.555.712 | 81.777.123.328 | +3.567.616 (drift: containers +1,8 MB, App Support +1,7 MB) | 22,95 / 22,65 |
| Documentos | 45.167.603.712 | 45.167.603.712 | 0 | 37,11 / 38,74 (inclui a passagem de Aplicativos; só Documentos ≈ 15) |
| Música | 36.429.056.548 | 36.429.056.548 | 0 | 0,11 / 0,08 |
| Fotos | 10.207.620.065 | 10.207.620.065 | 0 | 0,86 / 2,50 |
| TV | 0 | 0 | 0 | 0,04 / 0,00 |
| Livros | 0 (4 descartados, no iCloud) | 0 (4 descartados) | 0 | 0,27 / 0,06 |
| Podcasts | 0 | 0 | 0 | 0,04 / 0,02 |
| Lixo | 0 | 0 | 0 | 0,21 / 0,00 |
| GarageBand | 0 | 0 | 0 | 0,02 / 0,00 |
| Arquivos do iOS | 0 | 0 | 0 | 0,02 / 0,00 |
| Desenvolvedor | 2.050.097.152 | 2.050.097.152 | 0 | 2,54 / 5,24 |
| iCloud Drive | A 27.602.944 (+ B 93.626.368 = 121.229.312, estimativa) | 27.426.816 parcial (só A; B no detalhe) | A: −176.128 (drift; repetido: 26.836.992 × 26.832.896 e 26.902.528 × 26.902.528) | 1,89 / 1,11 |
| Mail | 912.019.456 | 912.019.456 | 0 | 1,27 / 0,86 |
| Mensagens | 214.752 | 214.752 | 0 | 0,07 / 0,00 |
| Outros usuários | 884.736 | 884.736 | 0 | 0,07 / 0,05 |

Conta inteira, duas rodadas (ordem P→Rust, depois Rust→P):

| | Frente P (`todas.py`) | Rust (`frenteq`) |
|---|---:|---:|
| Rodada 1: Σ / Dados do Sistema | 176.665.025.765 / 152.693.108.009 | 179.888.413.925 / 149.486.841.129 |
| Rodada 1: tempo | 44,92 s | 48,36 s |
| Rodada 2: Σ / Dados do Sistema | 176.706.854.117 / 152.699.026.729 | 176.603.180.261 / 152.793.935.145 (teto) |
| Rodada 2: tempo | 51,10 s | 53,53 s |

Na rodada 1 do Rust, `~/Library/Application Support` mediu 26.814.709.760 contra 23,52 GB antes
e depois (conferido com `frenteq tam:` logo em seguida: 23.525.621.760). Foram 3,29 GB
transitórios de algum app; comparar categorias exige o mesmo instante (como a Frente P já dizia).

### 2b. Rodada do cético (verificado, 29/09 19:34:58, P e Rust em paralelo)

`todas.py` (via wrapper `mode=ro`, binários recompilados das fontes da Frente P) e `frenteq`
lançados no **mesmo segundo**; terminaram 19:35:52 e 19:35:55 (54,1 s e 57,4 s, os dois
disputando disco). Onde houve diferença, a categoria foi repetida em paralelo, isolada.

| Categoria | Frente P | Rust | Diferença | Conferência |
|---|---:|---:|---:|---|
| Aplicativos | 83.559.456.768 | 83.552.608.256 | −6.848.512 | só `apps` em paralelo: **81.914.929.152 nos dois**, componente a componente (bundles 51.915.771.904, containers 6.456.897.536, restritos 13.078.528, App Support 23.529.181.184; 133 itens, 71 pulados). App Support oscila ±1,7 GB em minutos: drift |
| Documentos | 45.169.836.032 | 45.169.836.032 | 0 | Mesa e Documentos no iCloud: ligado (medido) |
| Música | 36.429.056.548 | 36.429.056.548 | 0 | 1.578 faixas, 53 sem arquivo local |
| Fotos | 10.207.614.995 | 10.207.614.995 | 0 | r2 10.213.127.555 − r4 5.512.560 nos dois |
| TV, Livros, Podcasts, Lixo, GarageBand, iOS | 0 | 0 | 0 | Livros: 4 descartados (iCloud) nos dois |
| Desenvolvedor | 2.050.097.152 | 2.050.097.152 | 0 | |
| iCloud Drive (parte A) | 24.985.600 | 25.612.288 | +626.688 | só `icloud` em paralelo, 3×: 27.439.104 = 27.439.104; 26.664.960 = 26.664.960; 26.734.592 × 26.664.960 (A muda a cada segundo): drift |
| iCloud Drive (P soma A + B) | 117.186.560 (estimativa) | 25.612.288 parcial | B fora da soma no Rust | B oscila 81–93 MB entre chamadas |
| Mail | 913.092.608 | 913.092.608 | 0 | purgeable do MailCacheDelete = 0 nos dois |
| Mensagens | 214.752 | 214.752 | 0 | |
| Outros usuários | 884.736 | 884.736 | 0 | |
| **Dados do Sistema** | 152.242.422.007 | 152.345.288.951 (teto) | +102.866.944 | = 6.848.512 (Apps) + 91.574.272 (iCloud: B + drift de A) + 4.448.256 (usado lido em outro instante) − 4.096 (purgeable) |

**Nenhuma diferença sem explicação por drift.** Leitura do código contra a Frente P, função a
função (regras, filtros, ordem de exclusões, CacheDelete com `NO_CACHE=NO` como a 1ª linha que
`todas.py` usa, callback do Fotos com 4 u64 + NSError, varredura do iCloud): mesma lógica.

**Chamadas privadas só leem** (desmontadas com `lldb` no processo, callees resolvidos):
`STMSizeDirectoryWithExcludedChildURLs` → `NodeContextOpen` + `_NodeSizingV2` →
`TOperationSizer::ComputeSize` (nada de unlink/rename/write/xattr/fsctl);
`CacheDeleteCopyItemizedPurgeableSpaceWithInfo` → XPC `clientUnifiedPurgeableSpace:queryType:replyBlock:`
(consulta; o ramo "critical relinquish" só com chave que o spike não passa);
`+[PLFileUtilities calculateTotalSizeOfFilesAtPath:…]` → `initWithFilePath:` +
`visitURLsLoadingPropertiesForKeys:withBlock:`, bloco só com `resourceValuesForKeys:error:`.
O resto é público de leitura (MDQuery, MDItem, NSBundle, getResourceValue, sqlite `mode=ro`,
`fileproviderctl dump`, `diskutil apfs list`, dir-stats com buffer zerado).

**Defeito corrigido:** na parte B do iCloud (só detalhe), `client.db`/`server.db` que não abrisse
virava freelist 0 (0 falso). Agora B sai `null`. Build e os 2 testes passam.

**Nota:** o SQLite `mode=ro` no lugar ainda pode tocar o `-shm` (índice do WAL, memória
compartilhada) se precisar recuperar o índice; o conteúdo do banco não muda.

## 3. O que cada categoria exige

| Categoria | Fonte no Rust | Framework privado | FDA | Root |
|---|---|---|---|---|
| Aplicativos | Spotlight `MDQuery` (público) + `STMSize` de bundles, containers e App Support | StorageManagement (substituível, §4) | containers de outros apps (não testado sem FDA) | não |
| Documentos | `STMSize(~)` com excluídos; "Mesa e Documentos" por `NSURLIsUbiquitousItemKey` | StorageManagement (substituível) | não | não |
| Lixo, GarageBand, Arquivos do iOS, Desenvolvedor | caminhos fixos + `STMSize` | StorageManagement (substituível) | não | não |
| Outros usuários | `/Users/Shared` + `/Users/Deleted Users` | StorageManagement (substituível) | não | **sim, só com outra home** (sai "parcial") |
| Mail | `STMSize(~/Library/Mail)` − `CacheDelete` (só consulta) | StorageManagement (substituível) + **CacheDelete** | sim | não |
| Fotos | `+[PLFileUtilities calculateTotalSizeOfFilesAtPath:…]`, r2 − r4 | **PhotoLibraryServicesCore** | sim (sem FDA pode travar > 2 min) | não |
| Música, TV | `Library.musicdb`/`.tvdb` (AES-128-ECB com chave fixa + zlib) e `stat` das faixas | não (formato privado de arquivo) | não | não |
| Livros | `BKLibrary-*.sqlite` + `isUbiquitousItemAtURL` + `kMDItemFSSize` | não | sim | não |
| Podcasts | `MTLibrary.sqlite` + alocado dos episódios | não | sim | não |
| Mensagens | `chat.db`, uma consulta `SUM` | não | sim | não |
| iCloud Drive | banco FPFS (UUID por `fileproviderctl dump`) | não (dir-stats `fsctl` só para a parte B, fora da soma) | sim | não |

Resumo: **privado indispensável em 2** (Fotos, Mail); **privado substituível pelo walker em 7**
(Aplicativos, Documentos, Lixo, GarageBand, iOS, Desenvolvedor, Outros usuários); **só banco ou
pastas em 6** (Música, TV, Livros, Podcasts, Mensagens, iCloud Drive). Todos os bancos são lidos
no lugar com `SQLITE_OPEN_READONLY | SQLITE_OPEN_URI` e `?mode=ro`, sem cópia; o JSON só leva
números e caminhos fixos.

## 4. `STMSize` = soma alocada da travessia

`frenteq tam:<caminho>` (o `STMSizeDirectoryWithExcludedChildURLs` sem excluídos) contra uma
travessia simples (`st_blocks × 512`, hardlink uma vez, sem cruzar volume, dataless e atime OFF):

| Pasta | STMSize | Travessia (arquivos) | Travessia (arquivos + pastas) |
|---|---:|---:|---:|
| `~/Library/Mail` | 913.092.608 | 913.092.608 | 913.092.608 |
| `/Library/Developer/CommandLineTools` | 2.044.735.488 | 2.044.735.488 | 2.044.735.488 |
| `~/Library/Developer` | 403.963.904 | 403.963.904 | 403.963.904 |
| `~/Library/Containers/com.apple.Safari/Data` | 2.908.819.456 | 2.908.819.456 | 2.908.819.456 |
| `~/.npm` | 4.600.913.920 | 4.600.913.920 | 4.600.913.920 |

Teste sintético no scratchpad: 1 arquivo de 1 MiB = 1.048.576; + hardlink = 1.048.576; + clone
(`cp -c`) = 2.097.152. Ou seja: **hardlink deduplicado, clone não** (é `ALLOCSIZE`, não
`PRIVATESIZE`), pastas valem 0 no APFS. Não testado: pasta dataless, xattr grande em fluxo
próprio, arquivo comprimido e travessia que cruza montagem dentro da pasta.

## 5. Riscos

**APIs e formatos privados por versão (15/26/27).** Só o 27.0 foi rodado. Pontos que podem
mudar sem aviso:
- `STMSizeDirectoryWithExcludedChildURLs` (StorageManagement): nome e assinatura `(NSURL*,
  NSArray*) -> u64`. Some da lista de riscos se a integração usar o walker (§4).
- `+[PLFileUtilities calculateTotalSizeOfFilesAtPath:calculatePurgeable:allocatedSize:result:]`:
  seletor e **assinatura do bloco** (4 × u64 + `NSError*`). Assinatura errada não falha no
  `dlsym`: corrompe a pilha. Antes de chamar, conferir `respondsToSelector:` e, no CI, a
  assinatura com `method_getTypeEncoding`.
- `CacheDeleteCopyItemizedPurgeableSpaceWithInfo`: a lib já usa (`sys/cachedelete.rs`); CI verde não prova que a chamada rodou no 15 e no 26.
- Formatos: `Library.musicdb` ("hfma", chave AES fixa, offsets de `boma`), nome e colunas do
  `BKLibrary`, esquema do `MTLibrary` (Core Data), colunas de `attachment` no `chat.db`,
  `FS_snapshot`/`engine_state` do FPFS e a linha `+ database: … UUID=` do `fileproviderctl`.
  Falha de formato sai "não medida" (Err ou parser que recusa), exceto os casos de "banco
  ausente = 0", agora protegidos pela existência da pasta da biblioteca (§1).
- `frenteq simbolos` imprime o que existe na versão (hoje no 27: tudo `true`). Serve para rodar
  no CI de 15 e 26 antes da fase 2.

**Sandbox e TCC.** As extensões rodam em sandbox e o Rust não: exclusões que vêm do sandbox
(ex.: `~/Library/Caches/com.apple.dt.Xcode` vale 0 no Desenvolvedor) têm de ser copiadas à mão,
e já estão. Sem FDA: Fotos, Livros, Podcasts, iCloud Drive, Mail e Mensagens saem "não medida
(terminal sem Acesso Total ao Disco)" sem tentar abrir (abrir a biblioteca do Fotos sem FDA
travou > 2 min, síntese §4.9). Containers de outros apps em Aplicativos: comportamento sem FDA
não testado (proteção "dados de apps" do TCC). Nenhuma categoria gravou nada: bancos em
`mode=ro`, CacheDelete só consulta, dir-stats com buffer zerado.

**Tempo.** 48–54 s só para as categorias, quase tudo em `STMSize` de pastas grandes (App
Support 23,5 GB, a home para Documentos). Somado à travessia da fase 1 (≤ 60 s pelo critério), o
total passaria de 100 s se as categorias rodassem à parte. Pelo walker, o custo extra fica em
Fotos (2–3 s), Spotlight (< 1 s), `fileproviderctl` (≈ 1 s) e bancos (< 0,3 s).

**Outros.** O timeout por categoria deixa a thread para trás (o spike usa `mpsc`); na lib usar o
`sys::with_timeout` que já existe. "Mesa e Documentos" é inferido de `NSURLIsUbiquitousItemKey`
nas pastas, não do estado do serviço (pergunta aberta 2 da Frente P). A parte B do iCloud Drive
continua hipótese; com ela fora, Dados do Sistema sai "parcial (teto)", erro de ~0,1 GB hoje e
até ~1,7 GB no pior caso visto em 28/09.

## 6. Integração na lib (fase 2)

**Módulo.** `src/sources/categorias/`:
- `mod.rs`: `pub struct Categoria { id: &'static str, medida: ledger::Medida, fonte: &'static str }`
  e `pub fn coletar(ctx: &Ctx, baldes: &Baldes, caps: &Caps, err: &mut Vec<Value>) -> Vec<Categoria>`.
  Usa o `Medida` do `ledger.rs` (já tem `NaoDisponivel`, `NaoMedida`, `Parcial`). Dados do
  Sistema entra no `ledger` como termo "diferença", com o estado herdado (parcial = teto).
- `regras.rs` (puro, sem I/O): `home` como parâmetro; caminho → categoria. Contém
  `pasta_em_applications`, os excluídos de Documentos, os caminhos fixos de Desenvolvedor,
  GarageBand, iOS e Outros usuários, o filtro de Lixo e os bundle IDs ignorados. A travessia
  pergunta a `regras` em que balde cada pasta cai (prefixo mais longo) e soma `ALLOCSIZE` com
  hardlink uma vez e clone de novo (§4), o que também dá o "está em Dados do Sistema" por linha
  que a fase 2 pede.
- `midia.rs` (parser puro do `musicdb`, já testado com buffer sintético), `bancos.rs` (Mensagens,
  Podcasts, Livros, FPFS), `fotos.rs`.
- `sys/`: `objc.rs` (macro `msg!`, `Pool`, chaves de `NSURL`, `MDQuery`), `sqlite.rs` (só
  `READONLY|URI`, `busy_timeout`), `cripto.rs` (CCCrypt e zlib), `fotos.rs` (bloco global e
  `PLFileUtilities`). Todo `unsafe` fica em `sys/`, como hoje. Sem dependência nova: sqlite3,
  CommonCrypto e zlib são do sistema.
- `caps.rs` ganha `pl_file_utilities` e o resultado do `simbolos`; fonte ausente sai
  `NaoDisponivel`.
- Aplicativos precisa de dados antes da travessia: lista do Spotlight e mapa de containers
  (< 1 s). Documentos depende da lista de apps sob a home.

**Interface.** No texto, cada linha do ranking ganha a coluna "categoria do Ajustes"; a tela
reproduzida lista as 15 com fonte e estado. No `--json`: `categories: [{id, bytes|null, state,
source}]` e `system_data: {bytes, state, formula}`, chaves em inglês, bytes inteiros.

**Testes.**
- Unitários de `regras.rs` com home falsa (caminhos, exclusões, `pasta_em_applications`).
- Parser do `musicdb`: casos sintéticos + os 100 mil buffers determinísticos, como o `parse.rs`.
- `tests/fixture_apfs.rs`: montar na imagem uma home falsa (Lixo, `Library/Developer`, `Library/Mail`,
  hardlink e clone) e exigir balde do walker = `STMSize` da mesma pasta ao byte, em cada versão do
  CI (o `STMSize` vira oráculo de teste, não fonte). Pula com aviso se o símbolo não existir.
- Bancos: SQLite criado no teste com o esquema mínimo (chat.db, MTLibrary, BKLibrary, FPFS)
  para conferir consulta e o "não medida" quando falta coluna.
- Na máquina de referência: `frenteq` e a lib no mesmo minuto, e as duas contra o log do painel
  (critério da fase 2: Σ ±1 %).

## 7. Como rodar

```sh
cd research/experimentos/frenteQ
CARGO_TARGET_DIR=<fora do iCloud> cargo build --release && cargo test --release
perl -e 'alarm 300; exec @ARGV' <target>/release/frenteq            # todas + a conta (JSON)
<target>/release/frenteq music photos     # algumas (ids curtos da tabela CATS em main.rs)
<target>/release/frenteq simbolos         # o que existe de privado nesta versão
<target>/release/frenteq tam:<caminho>    # STMSize de um caminho
```

## 8. O que ficou de fora

- Rodar no macOS 15 e 26 (só `simbolos` preparado para o CI).
- Conferência com o log do painel no mesmo minuto (depende do dono abrir Ajustes; Frente P §5).
- Paralelizar as categorias no spike: sem sentido se a integração for pelo walker.
- Cópias de bancos pessoais que a Frente P deixou no scratchpad antigo (pergunta 10 dela)
  continuam lá; esta frente não criou nenhuma.

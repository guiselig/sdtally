# Frente C — Atribuição de donos e categorias — 2026-09-27

Pergunta: dá para dizer, **com fato verificável**, de quem é cada pasta (app, daemon, ferramenta),
em qual categoria do painel ela cai e se está órfã?

Máquina: macOS 27 (Darwin 27.0.0), Apple Silicon, **sem Full Disk Access**, sem root. Tudo somente
leitura; programas em `research/experimentos/frenteC/`. Nomes de arquivos pessoais foram omitidos;
ids de apps aparecem porque são o objeto da pesquisa.

Confiança: **A** = medido aqui ou documentado pela Apple/SDK; **M** = inferência forte sobre fato
medido (ex.: string no binário da Apple); **B** = relato de terceiro ou suposição.

---

## 0. Resumo em 12 linhas

1. **Containers e Group Containers têm dono gravado pelo sistema** (A): `MCMMetadataIdentifier` no
   `.com.apple.containermanagerd.metadata.plist` (e no xattr `com.apple.containermanager.identifier`).
   Legível sem FDA. 100% dos 840 + 183 + 27 containers desta máquina.
2. **"Instalado?" se responde em ~20 µs por id** com `LSCopyApplicationURLsForBundleIdentifier`
   (A, medido: 840 ids em 17 ms). Mas LaunchServices só conhece **apps**; extensões, helpers e
   daemons precisam de outras provas (pluginkit, Info.plist no volume selado, assinatura).
3. Com 6 regras de prova em cadeia (`atribuir.py`), **sobram 19 containers de terceiros sem dono
   localizado** (≈0,06 GiB) e 99 da Apple sem prova (≈1,45 GiB). Nenhum é "órfão provado":
   ausência de prova não é prova de ausência.
4. **Pastas soltas em `~/Library` (Caches, Application Support, Preferences, HTTPStorages) só têm o
   nome.** Quando o nome é um bundle id, a checagem acima vale; quando é nome livre ("Google",
   "JetBrains", "Code"), **não há fato no disco que diga o dono** (A).
5. **Attribution tag (`ATTR_CMNEXT_ATTRIBUTION_TAG`) não serve** (A): o volume Data informa
   `VOL_CAP_INT_ATTRIBUTION_TAG` válido mas **não suportado**; hash sempre 0; `F_QUERY_TAG` = `EPERM`.
6. **`com.apple.provenance` não identifica o app sem root** (A): o id aponta para a tabela
   `provenance_tracking` de `/var/db/SystemPolicyConfiguration/ExecPolicy` (só root). E o id no
   xattr de um `.app` é o de **quem gravou o bundle** (instalador), não o do app: 12 apps aqui
   compartilham o mesmo id.
7. **`com.apple.quarantine` guarda o nome do processo** (A), não bundle id; em `~/Library` quase
   nunca tem o UUID de evento. Em `Preferences`, o agente é sempre `cfprefsd`. Sinal fraco.
8. **`lsof -u $UID` (0,36 s) prova uso ativo**: qual processo tem arquivo aberto na pasta agora
   (A). Serve para "em uso", não para "dono" de pasta parada.
9. **O painel é um conjunto de extensões `com.apple.storagemanagement`** (A: `pluginkit`), 15 nesta
   máquina. As strings dos binários da Apple listam os caminhos de **Developer** e **iOS Files**
   (M). "System Data" não tem extensão: é o resto (A, Apple + Oakley).
10. **Apps usa Spotlight** (`kMDItemContentType='com.apple.application-bundle'`) e soma o container
    de dados de cada app (M: strings `STMContainerSizer`, `dataContainerSizeForApp:`).
11. **Spotlight não indexa `~/Library/Caches`, `~/Library/Containers` nem dotfolders** (A: `mdfind
    -onlyin` = 0 resultados), mas indexa `Application Support` (96.958). `kMDItemLastUsedDate` é
    nulo em 28 de 92 apps: nulo = "não registrado", não "nunca usado".
12. **Comandos oficiais de limpeza raramente informam tamanho medido**: `uv cache size` e `ollama
    list` bateram com `du`; `brew cleanup -n` diz "approximately"; `colima list` mostra o tamanho
    **configurado** (100 GiB) de um disco esparso de 8,5 GiB; `docker system df` mede dentro da VM.

---

## 1. Containers (`~/Library/Containers`, `Group Containers`, `Daemon Containers`)

### 1.1 O que o sistema grava (A, medido)

Script de agregação sobre todos os metadados (ver `atribuir.py`):

| | Containers | Group Containers | Daemon Containers |
|---|---|---|---|
| Entradas | 840 (6 com nome UUID: apps iOS) | 183 | 27 (todas UUID) |
| Metadata legível sem FDA | 840 | 183 | 27 |
| `MCMMetadataIdentifier` == nome da pasta | 834/834 não-UUID | 183/183 | nome é UUID |
| `MCMMetadataContentClass` | 2 (todas) | 7 (todas) | — |
| Tem `MCMMetadataCreator` | 838 | 178 | — |
| Tem `MCMMetadataInfo.Owners` | 414 | 0 | — |
| Tem entitlements (`SandboxProfileDataValidationInfo`) | 438 | 0 | — |

Chaves:

- **`MCMMetadataIdentifier`**: bundle id (Containers) ou id do grupo (Group Containers). É o dono
  declarado. Também no xattr `com.apple.containermanager.identifier` (840/840 e 183/183), o que
  permite ler sem abrir o plist.
- **`MCMMetadataCreator`**: identificador de assinatura do **processo que criou** o container.
  Não é o dono: 546 containers foram criados por `com.apple.pluginkit.pkd` (extensões), 33 por
  `syncdefaultsd`, 23 por `com.apple.STMExtension.Applications` (o próprio painel de
  Armazenamento). Em Group Containers o criador é um app membro do grupo (útil como pista).
- **`MCMMetadataInfo.Owners`**: JSON `[{signingIdentifier, teamIdentifier, validationCategory}]`.
  `validationCategory` ∈ {`platform` 349, `app_store` 48, `developer_id` 17}. `platform` = binário
  da Apple → dono é o SO.
- **`SandboxProfileDataValidationInfo.Entitlements`**: entitlements do app no momento da criação,
  inclusive `com.apple.security.application-groups` (liga Container → Group Containers) e
  `com.apple.developer.team-identifier`.
- **`MCMMetadataUserIdentity`**: uid/gid/persona. `MCMMetadataOwnership` (372): `{uid, gid, mode}`.

Group Containers: 34 começam com team id (`XXXXXXXXXX.<grupo>`), o resto com `group.` ou
`com.apple.`. **O team id no prefixo é convenção, não prova** (há `group.*` sem team id).

### 1.2 Cadeia de prova de dono (A, medido nesta máquina)

`atribuir.py` aplica, em ordem, e para na primeira prova:

| Regra | Prova | Containers | Group | Daemon |
|---|---|---|---|---|
| LS | `LSCopyApplicationURLsForBundleIdentifier(id)` devolve URL | 99 | 0 | 1 |
| pluginkit | `pluginkit -mAD -v` lista extensão com o id | 503 | 2 | 0 |
| sistema | `Info.plist` com o id dentro do volume selado (`/System/...`, cryptex do Safari) | 148 | 2 | 3 |
| bundle | `Info.plist` ou `codesign -dv` `Identifier=id` dentro de um app instalado | 6 | — | — |
| grupo | um container **com dono provado** declara o grupo nos entitlements | — | 111 | — |
| entitlement | um app instalado declara o grupo na própria assinatura (`codesign -d --entitlements`) | — | 5 | — |
| platform | `validationCategory=platform` ou Daemon Container | 30 | — | 23 |
| **sem dono** | nenhuma prova | **54** | **64** | 0 |

Achados:

- **`pluginkit` não lista todas as extensões** (A): 110 containers de extensões da Apple (ex.:
  `com.apple.settings.Storage`, `com.apple.Accessibility-Settings.extension`) não aparecem no
  `pluginkit -mAD`, mas o `.appex` existe em `/System/Library/ExtensionKit/Extensions`. Daí a regra
  "sistema": indexar os `CFBundleIdentifier` do volume selado custa ~7 s e dá 7.411 ids.
- **Helpers dentro de apps não estão no LaunchServices** (A): Teams (`com.microsoft.teams2.agent`,
  achado por `codesign` em `Contents/Library/LaunchAgents`), WhatsApp (XPC service aninhado 5
  níveis), Kaspersky (XPC). Só a regra "bundle" os encontra. O prefixo do id escolhe onde procurar;
  **a prova é o arquivo achado**, não o prefixo.
- **Grupos declarados só por containers sem dono ficam sem dono** (ex.: um grupo cujo único app
  declarante foi desinstalado). Sem a exigência "declarante com dono provado", 2 grupos seriam
  atribuídos a containers órfãos.
- **Sem dono, terceiros**: 10 Containers + 9 Group Containers (widgets e extensões de apps
  desinstalados, jogos iOS removidos). Somam **~0,06 GiB** nesta máquina.
- **Sem dono, Apple** (99, ~1,45 GiB): extensões que não existem mais no sistema atual (resto de
  versões anteriores do macOS, suposição B) e grupos criados por daemons cujo binário não foi
  localizado. **Nunca oferecer**: a regra do catálogo proíbe Group Containers da Apple e o custo de
  errar é alto.

Custo total medido: ~16 s por execução, dominado por `codesign` em ~100 apps. Em Rust dá para
trocar `codesign` por `SecStaticCodeCreateWithPath` + `SecCodeCopySigningInformation`
(Security.framework, via `objc2-security`) — não testado.

### 1.3 Definição operacional de "órfão"

"Órfão" só pode ser **"nenhuma das provas localizou o dono"**. O relatório deve dizer qual regra
foi tentada. Motivos para não afirmar mais que isso (A, observados aqui):

- app instalado fora de `/Applications` e não registrado no LaunchServices (ex.: volume externo
  desmontado);
- helper sem `Info.plist` com nome diferente do app pai;
- app iOS em Mac Apple Silicon (6 containers UUID): o dono vem do metadata, a instalação do LS.

---

## 2. Pastas soltas em `~/Library`

Não têm metadata de dono. O nome é o único dado, e só vale quando é um bundle id (A, medido):

| Pasta | Entradas | Nome é bundle id | LS | pluginkit | sistema | Apple sem prova (GiB) | 3º sem dono (GiB) |
|---|---|---|---|---|---|---|---|
| Caches | 196 | 145 | 45 | 0 | 23 | 70 (0,206) | 7 (0,001) |
| Application Support | 147 | 40 | 13 | 0 | 7 | 16 (0,389) | 4 (0,010) |
| Preferences | 619 | 591 | 143 | 3 | 69 | 330 (0,002) | 46 (0,004) |
| HTTPStorages | 127 | 119 | 58 | 0 | 18 | 32 (0,050) | 11 (0,003) |
| WebKit | 23 | 20 | 16 | 0 | 2 | 0 | 2 (0,001) |
| Logs | 34 | 5 | 2 | 0 | 1 | 2 (0,000) | 0 |

(Sufixos `.plist`, `.binarycookies`, `.ShipIt`, `.savedState` removidos antes da consulta. "sistema" = id achado em `Info.plist` do volume selado.)

**O que isso significa para o projeto:** órfãos por bundle id de terceiros são mínimos (≈0,02 GiB aqui; ≈0,06 GiB em containers). O
volume está em pastas de nome livre (`Caches/JetBrains` 4,6 GiB, `Application Support/Google`
2,2 GiB, `Application Support/Code` 0,95 GiB) e dotfolders. Para essas, o dono só pode vir de:

1. **A ferramenta declarar o caminho** (A quando o comando existe): `npm config get cache`,
   `brew --cache`, `go env GOCACHE GOMODCACHE`, `pip cache dir`, `uv cache dir`,
   `pnpm store path`. Todos testados aqui (seção 5).
2. **Catálogo curado com fonte** (documentação do fornecedor), declarado como tal no relatório:
   "dono segundo documentação X", não "dono medido".
3. **Uso ativo** via `lsof` (seção 3.4): prova que um processo usa a pasta agora.

`Saved Application State` **não existe** no topo de `~/Library` no macOS 27 (A); há uma pasta
com esse nome em 838 containers. Não investigado a fundo.

---

## 3. Atributos de arquivo como prova de origem

### 3.1 `ATTR_CMNEXT_ATTRIBUTION_TAG` — inútil aqui (A)

- SDK (`sys/attr.h`): `VOL_CAP_INT_ATTRIBUTION_TAG` = "o volume suporta estabelecer relação de
  dono entre um arquivo (exceto arquivos pequenos) e um processo na primeira
  leitura/escrita/truncate/clone". `ATTR_CMNEXT_ATTRIBUTION_TAG = 0x800` devolve um hash de 64
  bits. `fcntl(F_ATTRIBUTION_TAG)` com `F_QUERY_TAG` devolveria o nome (`sys/fcntl.h`).
- **Teste (`attrtag.c`)**: volume `/System/Volumes/Data` → `valid=1 supported=0`. Mesmo em um
  arquivo novo de 8 MiB criado pelo teste, hash = 0 e `F_QUERY_TAG` = `EPERM`. Em 300 arquivos
  grandes de `Caches`/`Application Support`: hash 0 em todos.
- Conclusão: recurso existe no kernel, não está ligado no volume de dados do macOS 27; e mesmo o
  nome só identificaria o processo, não o app. **Descartar.** Observação de segurança: pela
  descrição do header, uma *leitura* pode criar o tag — onde o recurso estiver ligado, não abrir
  arquivos para consultar.

### 3.2 `com.apple.provenance` — agrupa, não identifica (A)

- Formato: 11 bytes, `01 02 00` + inteiro de 8 bytes; chave da tabela `provenance_tracking` em
  `/var/db/SystemPolicyConfiguration/ExecPolicy` (`-rw------- root`: ilegível sem root, testado).
  Fonte: [Eclectic Light 2023-05-10](https://eclecticlight.co/2023/05/10/how-macos-now-tracks-the-provenance-of-apps/)
  — o app recebe um id ao passar pelo Gatekeeper e o grava nos arquivos que cria/escreve; apps da
  Apple e da App Store não recebem id.
- Teste: 97 apps com o xattr, 169 pastas de `~/Library` com o xattr. Casamento por valor:
  - acertos plausíveis: `Caches/com.microsoft.VSCode` ↔ Visual Studio Code, `Caches/JetBrains` ↔
    RustRover, `Caches/org.gimp.gimp` ↔ GIMP;
  - **erro sistemático**: 12 apps compartilham o mesmo id; `Caches/com.hnc.Discord` casa com
    MySQLWorkbench; o bundle de um jogo da Steam carrega o id da **Steam** (quem gravou o bundle).
    O xattr num `.app` é o id do **processo que escreveu o bundle** (instalador, loja, updater),
    não o id atribuído ao app. O id "do app" só está na `ExecPolicy`.
- Uso possível sem root: "estes N itens foram escritos pela mesma linhagem de processo". Não
  serve como dono. **Confiança B para qualquer atribuição por provenance.**

### 3.3 `com.apple.quarantine`, `com.apple.macl`, `kMDItemWhereFroms`

| Xattr | Onde aparece (topo de `~/Library/*`) | O que diz | Serve como dono? |
|---|---|---|---|
| `com.apple.quarantine` | Preferences 606, Caches 21, App Support 8 | `flags;hex_time;nome_do_agente;UUID`. Agente = **nome do processo** (`Chrome`, `FindMy`, `com.apple.cfprefsd`). UUID vazio para arquivos criados (não baixados); com UUID, liga a `~/Library/Preferences/com.apple.LaunchServices.QuarantineEventsV2` (SQLite legível, coluna `LSQuarantineAgentBundleIdentifier`, 265 de 332 eventos com bundle id) | Fraco: nome de processo; em Preferences é sempre `cfprefsd` |
| `com.apple.macl` | Preferences 604, Group Containers 40 | Lista de apps com acesso concedido pelo TCC ao arquivo (via painel Abrir) | Não: é acesso, não autoria ([Oakley 2025-12-05](https://eclecticlight.co/2025/12/05/quarantine-macl-and-provenance-what-are-they-up-to/)) |
| `com.apple.metadata:kMDItemWhereFroms` | nenhuma ocorrência no topo de `~/Library` | URL de origem de download | Só para downloads |
| `com.apple.containermanager.identifier` | 100% dos containers | Dono do container | **Sim** (= metadata) |
| `com.apple.metadata:com_apple_backup_excludeItem` | HTTPStorages 110 | Excluído do Time Machine | Não; útil para "é cache" |

### 3.4 `lsof` — uso ativo (A)

`lsof -n -P -u $UID -Fpcn`: 0,36 s, 39 mil linhas, 448 pastas de `~` com arquivo aberto. Ex.:
`.colima` ← `limactl`, `com.apple.Virtualization`; `.ollama` ← `Ollama`; `.npm` ← `node`.
O nome de comando vem truncado (15 caracteres); para chegar ao bundle id, usar o pid →
`proc_pidpath` (libc) → `.app` que contém o executável → `CFBundleIdentifier`. Prova "em uso
agora", nunca "dono" de pasta parada. Útil para a regra de segurança "não oferecer se em uso".

### 3.5 Spotlight: "última vez usado" (A)

- `mdls` em apps: 64 de 92 apps em `/Applications` têm `kMDItemLastUsedDate`; 28 têm `(null)`.
  `kMDItemUseCount` e `kMDItemUsedDates` (lista de dias) vêm juntos. Nulo significa "Spotlight não
  registrou" (app aberto por outro caminho, instalado sem abrir pelo Finder, índice refeito) —
  **não** "nunca usado".
- `kMDItemDateAdded` existe sempre que indexado.
- Cobertura do índice: `mdfind -onlyin` retorna **0** em `~/Library/Caches`, `~/Library/Containers`
  e `~/.npm`; 96.958 em `Application Support`. **Não dá para usar Spotlight para medir ou classificar
  caches/containers.**

---

## 4. Como o painel decide as categorias

### 4.1 Fatos (A)

- **Cada categoria é uma extensão do ponto `com.apple.storagemanagement`**: `pluginkit -mAD -p
  com.apple.storagemanagement` lista 15: `STMExtension.{Applications, Developer, iOSFiles, Trash,
  OtherUsers, GarageBand, AppleInternal, Mail}`, `Photos.StorageManagementExtension`,
  `Music.MusicStorageExtension`, `TV.TVStorageExtension`, `podcasts.MacPodcastsStorageExtension`,
  `iBooksX.DiskSpaceEfficiency`, `messages.StorageManagementExtension`,
  `CloudDocsDaemon.StorageManagement` (iCloud Drive). O framework é
  `/System/Library/PrivateFrameworks/StorageManagement.framework` com o daemon `diskspaced`.
- **Não há extensão para Documents, macOS nem System Data.** Documents e macOS devem estar no
  próprio framework (no dyld cache, não inspecionado).
- Apple ([mchl3d437fbc](https://support.apple.com/guide/mac-help/mchl3d437fbc/mac)): Documents =
  "arquivos na pasta de início que não estão em outras categorias"; iOS Files = "backups e
  firmware"; macOS = "apps e arquivos de sistema"; System Data = "arquivos que não cabem nas
  categorias ... logs, caches, arquivos de VM". [102624](https://support.apple.com/en-us/102624):
  System Data mede "todos os arquivos Apple e de terceiros que não pertencem a uma categoria mais
  específica".
- Oakley ([explainer 2025-12-20](https://eclecticlight.co/2025/12/20/explainer-storage-settings/)):
  o painel soma as outras categorias e System Data é o resto; Photos conta só a biblioteca do
  sistema; macOS mostra componentes de IA.

### 4.2 Caminhos nos binários das extensões (M: string presente ≠ prova de contagem, mas forte)

`strings` nos executáveis desta máquina:

| Extensão | Caminhos referenciados |
|---|---|
| Developer | `~/Library/Developer`, `Library/Developer/CoreSimulator/Caches`, `Xcode/DerivedData`, `Xcode/Archives`, `Xcode/Installs`, `Xcode/{iOS,macOS,tvOS,visionOS,watchOS} DeviceSupport`, `~/Library/Caches/com.apple.dt.Xcode`, `/Library/Developer/CommandLineTools`, `/Library/Developer/CoreSimulator`; rótulos "Xcode Caches", "Project Archives", "Simulator Support", "Xcode Project Build Files" |
| iOS Files | `Library/Application Support/MobileSync/Backup`, `Library/iTunes/{iPhone,iPad,iPod,Apple TV} Software Updates`, `Library/Group Containers/{4WXS7A4F54,K36BKF7T3D}.group.com.apple.configurator/Library/Caches/Firmware` |
| Applications | `/Applications`, `/System/Applications`, `Library/Containers/`, `Library/Application Support/Steam` (lê manifestos da Steam), query Spotlight `kMDItemContentType='com.apple.application-bundle'`, classes `STMApplicationSizer`, `STMContainerSizer`, `dataContainerSizeForApp:`, `stm_appSize`/`stm_dataSize`; mensagem "Spotlight is still gathering so returning an unknown total size" |
| Trash | `.Trash`, `.Trashes` |
| OtherUsers | `/Users/Deleted Users/`, `/Users/Shared/`, `/private/var/` |
| GarageBand (Music Creation) | `/Library/Application Support/{GarageBand,Logic}/...`, `/Library/Audio/Apple Loops/Apple`, `/Library/Audio/Impulse Responses/Apple` |
| Messages | `/Library/Messages` |

### 4.3 Regras verificáveis por caminho (para o CLI)

| Categoria | Regra implementável | Confiança |
|---|---|---|
| Developer | prefixos da tabela 4.2 | M |
| iOS Files | prefixos da tabela 4.2 | M |
| Trash | `~/.Trash`, `/.Trashes`, `<vol>/.Trashes` | M |
| Apps | bundles `com.apple.application-bundle` fora de `/System` + **container de dados do app** + Steam | M (a soma do container é inferência do nome do método) |
| Photos | pacote com UTI `com.apple.photos.library` que é a biblioteca do sistema | A (UTI medido com `mdls`), M (qual biblioteca) |
| Mail / Messages | `~/Library/Mail`, `~/Library/Messages` | M |
| iCloud Drive | `~/Library/Mobile Documents` | A (Apple descreve) |
| Documents | "arquivos da home fora das outras categorias" — critério exato desconhecido; hipótese: query Spotlight por tipo | **B** |
| macOS | SSV + assets de sistema (Apple Intelligence) | B para caminhos concretos |
| System Data | resto | A (definição), não reproduzível por caminho |

**UTType por arquivo:** `mdls -name kMDItemContentType` funciona, inclusive em arquivo fora do
índice (testado em `.plist` dentro de `~/Library/Developer`). Em Rust, sem processo externo:
`UTType(filenameExtension:)` / `conformsTo` via `objc2-uniform-type-identifiers` (não testado).
**Não reproduzir o número do painel** (algoritmo fechado e com atraso; ver pesquisa comunidade).
Mostrar "categoria provável segundo string do binário da Apple" com a confiança.

---

## 5. Sistema e ferramentas de desenvolvimento

### 5.1 `/private/var/folders` (A)

- `getconf DARWIN_USER_DIR` → `.../<xx>/<hash>/0/`; `DARWIN_USER_TEMP_DIR` → `.../T/`;
  `DARWIN_USER_CACHE_DIR` → `.../C/`. Em Rust: `libc::confstr(_CS_DARWIN_USER_{DIR,TEMP_DIR,CACHE_DIR})`.
- Aqui: `C` 717 entradas (~0,86 GiB), `T` 572 (~0,24 GiB), `0` 51 (~0,09 GiB), `X` 3 (clones de
  assinatura: `com.google.Chrome.code_sign_clone`, `com.microsoft.teams2.helper.code_sign_clone`),
  `Cleanup At Startup`.
- **Dono das subpastas de `C`**: o nome é escolhido pelo processo (sufixo de `confstr`); 596 de 717
  são bundle ids → LS 228, pluginkit 49, Apple sem prova 268, **terceiros sem dono 51**. Sem
  metadata. 9 subpastas negam listagem (sandbox de WebKit/Shortcuts, `LaunchServices.dv`).
- `T`: nomes livres (`tmp.*`, `com.apple.WebKit.*+<app>`, `playwright_*`); o dono não é
  provável fora de `lsof`. `0/sysdiagnose.*`: restos de sysdiagnose.
- Limpeza oficial: nenhuma; `T` é limpo no boot e por `dirhelper`
  ([Oakley 2023-09-22](https://eclecticlight.co/2023/09/22/will-flushing-caches-free-up-disk-space/), B para detalhes).
  Não apagar com processos vivos.

### 5.2 Volume de sistema e assets (A)

- `/usr/share/firmlinks` confirma que `/Library`, `/System/Library/AssetsV2`,
  `/System/Library/Caches`, `/opt`, `/usr/local`, `/private`, `/Users`, `/Volumes` moram no
  volume Data. O scanner deve contar uma vez (via Data, com `SF_FIRMLINK`).
- `/System/Library/AssetsV2/com_apple_MobileAsset_<Tipo>`: 107 entradas; **o nome da pasta é o
  tipo MobileAsset** e esse é o dono (serviço `mobileassetd`). Tipos relevantes aqui: `UAF_FM_*`
  (modelos de Apple Intelligence), `iOSSimulatorRuntime`, `watchOSSimulatorRuntime`,
  `xrOSSimulatorRuntime`, `MacSoftwareUpdate`, `MetalToolchain`, fontes, dicionários, voz.
  Conteúdo protegido por TCC sem FDA (`ls` = `Operation not permitted`); `du` parcial = 9,1 GiB.
- `/Library/Application Support/com.apple.idleassetsd`: aerials (ilegível sem FDA).

### 5.3 Ferramentas: caminho, prova e comando (testado aqui quando existe)

| Ferramenta | Caminho que ela declara | Comando que prova | Limpeza oficial | Informa tamanho? |
|---|---|---|---|---|
| npm | `~/.npm` | `npm config get cache` (A) | `npm cache clean --force` | Não |
| pnpm | `~/Library/pnpm/store/v11` | `pnpm store path` (A) | `pnpm store prune` | Não verificado |
| uv | `~/.cache/uv` | `uv cache dir` (A) | `uv cache clean` / `uv cache prune` | `uv cache size` (experimental): **580,2 MiB = `du`** (A) |
| pip | `~/Library/Caches/pip` | `pip cache dir` (A) | `pip cache purge` | `pip cache info` (tamanho por tipo) (A) |
| Go | `~/Library/Caches/go-build`, `~/go/pkg/mod` | `go env GOCACHE GOMODCACHE` (A) | `go clean -cache`, `go clean -modcache` | Não |
| rustup | `~/.rustup/toolchains` | `rustup toolchain list` (A) | `rustup toolchain uninstall <tc>` | Não |
| cargo | `~/.cargo/registry`, `git` | `$CARGO_HOME` (padrão `~/.cargo`) | sem comando estável verificado | Não |
| Homebrew | `/opt/homebrew/Cellar/<f>/<v>`, `~/Library/Caches/Homebrew` | `brew --prefix`, `brew --cache`; `INSTALL_RECEIPT.json` em cada versão (A) | `brew cleanup`, `brew autoremove` | `brew cleanup -n`: "would free **approximately** 41.2MB" — estimativa da própria ferramenta |
| Ollama | `~/.ollama/models` | `ollama list` (A) | `ollama rm <modelo>` | Sim: 3,4 GB = `du` 3,16 GiB (A) |
| Hugging Face | `~/.cache/huggingface/hub` | `$HF_HOME` | `hf cache delete` | `hf cache scan` (CLI não instalada aqui; B) |
| Colima | `~/.colima/_lima/_disks/<perfil>/datadisk` | `colima list` (A) | `colima delete`, `colima prune` | **Não**: coluna DISK = 100 GiB configurado; disco esparso ocupa ~8,5 GiB |
| Docker (via Colima) | disco da VM | `docker system df` (A) | `docker system prune` | Sim, **dentro da VM** (5,6 GB imagens, 92% recuperável); não é espaço do host até compactar o disco |
| Xcode/simuladores | `~/Library/Developer/CoreSimulator/Devices`, `/Library/Developer/CoreSimulator`, AssetsV2 | `xcrun simctl list` | `xcrun simctl delete unavailable`, `xcrun simctl runtime delete` | `simctl runtime list -j` tem `sizeBytes` (fonte: pesquisa comunidade). **Aqui travou >3 min** (CoreSimulatorService com 13 dias): rodar com timeout |
| nvm | `~/.nvm/versions/node` | `nvm ls` | `nvm uninstall <v>` | Não |
| pyenv | `~/.pyenv/versions` | `pyenv versions` | `pyenv uninstall <v>` | Não |

Não presentes nesta máquina (caminhos segundo documentação dos projetos, B): yarn
(`yarn cache dir`), bun (`~/.bun/install/cache`, `bun pm cache rm`), Gradle (`~/.gradle/caches`),
Maven (`~/.m2/repository`), LM Studio (`~/.lmstudio/models`), OrbStack
(`~/.orbstack`, `Group Containers/HUAQ24HBR6.dev.orbstack`), Docker Desktop
(`~/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw`), Podman
(`~/.local/share/containers`), CocoaPods, conda.

**Regra para o projeto:** usar o comando da ferramenta **só para descobrir o caminho** (fato) e
medir o tamanho com a nossa travessia (`ALLOCSIZE`/`PRIVATESIZE`). O número da ferramenta, quando
existir, aparece ao lado com a origem ("segundo `ollama list`"). Nunca somar número de ferramenta.

---

## 6. Tabela de caminhos

Tamanho = `du -skx` nesta máquina (alocado; 0 pode ser TCC sem FDA). "Cat." = categoria provável
do painel (Dev = Developer, iOS = iOS Files, SD = System Data). Risco: S seguro, C custo,
Cd condicional, P proibido.

| # | Caminho | Dono | Como provar | Cat. | Limpeza oficial | Risco |
|---|---|---|---|---|---|---|
| 1 | `~/Library/Containers/<id>` | app/extensão `<id>` | `MCMMetadataIdentifier` + cadeia 1.2 | Apps (M) | desinstalar o app | P |
| 2 | `~/Library/Containers/<id>/Data/Library/Caches` | app `<id>` | idem 1 | Apps (M) | nenhuma; app fechado | Cd |
| 3 | `~/Library/Containers/<UUID>` | app iOS | `MCMMetadataIdentifier` | Apps (M) | desinstalar | P |
| 4 | `~/Library/Group Containers/<TEAM>.<grupo>` | apps do grupo | entitlements (1.2) | SD (B) | desinstalar | P |
| 5 | `~/Library/Group Containers/group.com.apple.*` | daemon Apple | metadata + criador | SD | nenhuma | P |
| 6 | `~/Library/Daemon Containers/<UUID>` | daemon Apple | `MCMMetadataIdentifier` | SD | nenhuma | P |
| 7 | `~/Library/Application Support/<bundle id>` | app `<id>` | LS/pluginkit/sistema | SD | nenhuma | P |
| 8 | `~/Library/Application Support/<nome livre>` | desconhecido | só catálogo/lsof | SD | — | P |
| 9 | `~/Library/Caches/<bundle id>` | app `<id>` | LS/pluginkit/sistema | SD | nenhuma | Cd |
| 10 | `~/Library/Caches/<id>.ShipIt` | updater Squirrel do app `<id>` | LS do `<id>` | SD | — | S |
| 11 | `~/Library/Preferences/<id>.plist` | app `<id>` (via cfprefsd) | LS | SD | `defaults delete <id>` | Cd |
| 12 | `~/Library/HTTPStorages/<id>` | app `<id>` (URLSession) | LS | SD | — | S |
| 13 | `~/Library/WebKit/<id>` | app `<id>` (WKWebView) | LS | SD | — | Cd |
| 14 | `~/Library/Logs/<x>` | app/ferramenta | nome | SD | — | S |
| 15 | `~/Library/Logs/DiagnosticReports` | ReportCrash | caminho fixo do SO | SD | — | S |
| 16 | `~/Library/Caches/CloudKit/<serviço>` | daemon de sync (`com.apple.bird` etc.) | subpasta = serviço | SD | nenhuma | Cd (runbook) |
| 17 | `~/Library/Mobile Documents` | iCloud Drive (`bird`) | caminho documentado | iCloud Drive (A) | Otimizar Armazenamento | P |
| 18 | `~/Library/CloudStorage/<provedor>` | File Provider do app | extensão `fileprovider-nonui` no pluginkit | SD/Docs (B) | app do provedor | P |
| 19 | `~/Library/Mail` | Mail | caminho | Mail (M) | pelo Mail | P |
| 20 | `~/Library/Messages` | Messages | caminho | Messages (M) | pelo Messages | P |
| 21 | `~/Pictures/<x>.photoslibrary` | Photos | UTI `com.apple.photos.library` | Photos (A se biblioteca do sistema) | pelo Photos | P |
| 22 | `~/Library/Application Support/MobileSync/Backup` | Finder/AMPDevices | string iOS Files | iOS (M) | Finder > Gerenciar Backups | C |
| 23 | `~/Library/iTunes/iPhone Software Updates` | Finder | string iOS Files | iOS (M) | — | S |
| 24 | `Group Containers/*.group.com.apple.configurator/Library/Caches/Firmware` | Apple Configurator | string iOS Files | iOS (M) | — | S |
| 25 | `~/Library/Developer/Xcode/DerivedData` | Xcode | string Developer | Dev (M) | painel Developer | C |
| 26 | `~/Library/Developer/Xcode/Archives` | Xcode | string Developer | Dev (M) | Organizer | Cd (simbolização) |
| 27 | `~/Library/Developer/Xcode/iOS DeviceSupport` | Xcode | string Developer | Dev (M) | painel | C |
| 28 | `~/Library/Developer/CoreSimulator/Devices` | CoreSimulator | `simctl list` | Dev (M) | `xcrun simctl delete unavailable` | C |
| 29 | `~/Library/Developer/CoreSimulator/Caches` | CoreSimulator | string Developer | Dev (M) | — | S |
| 30 | `~/Library/Caches/com.apple.dt.Xcode` | Xcode | string Developer | Dev (M) | — | S |
| 31 | `/Library/Developer/CoreSimulator` (Images/Volumes/Cryptex) | simdiskimaged | string Developer | Dev (M) | `xcrun simctl runtime delete` | C |
| 32 | `/Library/Developer/CommandLineTools` | CLT | string Developer | Dev (M) | nenhuma sem root | P |
| 33 | `/System/Library/AssetsV2/com_apple_MobileAsset_iOSSimulatorRuntime` | mobileassetd/Xcode | nome = tipo | Dev? (B) | `xcrun simctl runtime delete` | C |
| 34 | `/System/Library/AssetsV2/com_apple_MobileAsset_UAF_FM_*` | Apple Intelligence | nome = tipo | macOS (A Oakley) | nenhuma no macOS 27 | P |
| 35 | `/System/Library/AssetsV2/<outros tipos>` | mobileassetd | nome = tipo | macOS/SD (B) | nenhuma | P |
| 36 | `/Library/Application Support/com.apple.idleassetsd` | idleassetsd | caminho | SD (B) | wallpaper e protetor estáticos | Cd |
| 37 | `/Library/Updates` (0,5 GiB) | softwareupdated | caminho | SD (B) | nenhuma verificada | P |
| 38 | `/private/var/vm` (2 GiB) | kernel (swap, sleepimage) | caminho | SD/fora (B) | reboot | P |
| 39 | `/private/var/folders/.../C/<id>` | processo `<id>` | nome + LS | SD | nenhuma | Cd |
| 40 | `/private/var/folders/.../T/` | processos do usuário | `lsof` | SD | reboot | Cd |
| 41 | `/private/var/folders/.../0/sysdiagnose.*` | sysdiagnose | nome | SD | — | S |
| 42 | `/private/var/folders/.../X/*.code_sign_clone` | app do nome | nome | SD | — | Cd |
| 43 | `~/.Trash` | Finder | caminho | Trash (M) | Esvaziar Lixo | S |
| 44 | `/Users/Shared`, `/Users/Deleted Users` | outros usuários | string OtherUsers | Outros usuários (M) | — | P |
| 45 | `/Library/Application Support/{GarageBand,Logic}`, `/Library/Audio/Apple Loops` | GarageBand/Logic | string GarageBand | Music Creation (M) | pelo app | C |
| 46 | `~/Library/Application Support/Steam` (2,2 GiB) | Steam | string Applications | Apps (M) | pela Steam | C |
| 47 | `~/.npm` (4,3 GiB) | npm | `npm config get cache` | SD | `npm cache clean --force` | S |
| 48 | `~/Library/pnpm/store` (0,5 GiB) | pnpm | `pnpm store path` | SD | `pnpm store prune` | S |
| 49 | `~/.cache/uv` (0,57 GiB) | uv | `uv cache dir` | SD | `uv cache prune` | S |
| 50 | `~/Library/Caches/pip` | pip | `pip cache dir` | SD | `pip cache purge` | S |
| 51 | `~/Library/Caches/go-build` | go | `go env GOCACHE` | SD | `go clean -cache` | S |
| 52 | `~/go/pkg/mod` (1 GiB) | go | `go env GOMODCACHE` | SD | `go clean -modcache` | C |
| 53 | `~/.rustup/toolchains` (1,5 GiB) | rustup | `rustup toolchain list` | SD | `rustup toolchain uninstall` | C |
| 54 | `~/.cargo/registry` | cargo | `$CARGO_HOME` | SD | — | C |
| 55 | `/opt/homebrew/Cellar/<f>` (6,7 GiB total) | Homebrew | `INSTALL_RECEIPT.json` | SD | `brew uninstall`, `brew autoremove` | C |
| 56 | `/opt/homebrew/Caskroom` | Homebrew cask | `brew --prefix` | SD | `brew uninstall --cask` | C |
| 57 | `~/Library/Caches/Homebrew` (0,6 GiB) | Homebrew | `brew --cache` | SD | `brew cleanup` | S |
| 58 | `~/.ollama/models` (3,2 GiB) | Ollama | `ollama list` | SD | `ollama rm` | C |
| 59 | `~/.cache/huggingface` (1,5 GiB) | huggingface_hub | `$HF_HOME` | SD | `hf cache delete` | C |
| 60 | `~/.colima` (8,5 GiB, disco esparso) | Colima/Lima | `colima list`, `lsof` | SD | `colima prune`/`delete` | Cd |
| 61 | `~/.nvm/versions` (0,5 GiB) | nvm | `nvm ls` | SD | `nvm uninstall` | C |
| 62 | `~/.pyenv/versions` | pyenv | `pyenv versions` | SD | `pyenv uninstall` | C |
| 63 | `~/Library/Caches/JetBrains/<IDE><ver>` (4,6 GiB) | IDE JetBrains | nome + provenance casou com a IDE (B) | SD | "Invalidate Caches" na IDE | C |
| 64 | `~/Library/Application Support/Code/User/workspaceStorage` | VS Code | caminho documentado pelo VS Code (B) | SD | — | Cd (runbook) |
| 65 | `~/Library/Caches/ms-playwright` (1,1 GiB) | Playwright | documentação (B) | SD | `npx playwright uninstall` (B) | C |
| 66 | `~/Library/Metadata/CoreSpotlight` (2,5 GiB em `Metadata`) | corespotlightd | caminho (B) | SD | nenhuma | P |
| 67 | `~/Library/Biome` | biomed | caminho (B) | SD | nenhuma | P |

---

## 7. Rust: como fazer cada prova

| Prova | API | Crate/versão (docs.rs) | Estado |
|---|---|---|---|
| App instalado | `LSCopyApplicationURLsForBundleIdentifier(&CFString, *mut *mut CFError) -> Option<CFRetained<CFArray>>` | `objc2-core-services` 0.3.2, features `LaunchServices` + `LSInfo` (padrão). Deprecada em favor de `NSWorkspace` | **Testado** com FFI crua sem crate (`ls_installed.rs`): ~20 µs/id |
| App instalado (moderno) | `NSWorkspace::URLsForApplicationsWithBundleIdentifier(&NSString) -> Retained<NSArray<NSURL>>` | `objc2-app-kit` 0.3.2, feature `NSWorkspace` (padrão) | Não testado; AppKit num CLI puxa o framework inteiro |
| Alternativa sem API | `mdfind "kMDItemCFBundleIdentifier == '<id>'"` | processo externo | 40–70 ms por chamada; depende do índice. **Não usar** |
| Dump completo | `lsregister -dump` | processo externo | 4,2 s, 345 mil linhas, formato não documentado. Não usar |
| Metadata do container | `plist` crate ou `CFPropertyListCreateWithData` | `plist` / `objc2-core-foundation` | Lido com Python aqui |
| Extensões | `pluginkit -mAD -v` (0,6 s, uma vez) | processo externo | Incompleto (seção 1.2); complementar com índice do volume selado |
| Entitlements/assinatura | `SecStaticCodeCreateWithPath` + `SecCodeCopySigningInformation` | `objc2-security` | Não testado; hoje via `codesign` |
| Uso ativo | `proc_listpids` + `proc_pidfdinfo`/`proc_pidpath` ou `lsof` | `libc` | `lsof` testado |
| Dirs por usuário | `confstr(_CS_DARWIN_USER_*)` | `libc` | `getconf` testado |
| Xattrs | `getxattr` | `libc` | testado via `xattr` |
| UTType | `UTType(filenameExtension:)`, `conformsTo` | `objc2-uniform-type-identifiers` | Não testado |

---

## 8. O que é fato e o que é suposição

**Fato (medido aqui):** metadados de container e suas chaves; cobertura LS/pluginkit/sistema;
pluginkit incompleto; attribution tag desligado; provenance compartilhado entre apps; formato de
quarantine; cobertura do Spotlight; extensões `com.apple.storagemanagement` e suas strings;
caminhos e saídas de npm/pnpm/uv/pip/go/rustup/brew/ollama/colima/docker; `simctl` travando.

**Suposição ou fonte secundária:** que cada string de caminho nas extensões é de fato somada
naquela categoria; como Documents e macOS são calculados; que os 99 containers Apple sem prova são
restos de versões antigas; caminhos de ferramentas não instaladas aqui; comportamento de limpeza
de `/private/var/folders`.

## 9. Recomendações para o projeto

1. Atribuição em camadas, cada item com a **regra que provou** (LS, pluginkit, sistema, bundle,
   grupo, entitlement, platform, ferramenta declara caminho) ou "dono não localizado".
2. Nunca usar provenance, quarantine, prefixo de id ou nome de pasta como dono. Podem aparecer como
   "pista" rotulada, fora da conta.
3. "Órfão" = "dono não localizado pelas regras X, Y, Z". Oferecer só terceiros fora do catálogo
   proibido, e só depois de `lsof` negativo.
4. Categoria do painel = "provável", com a fonte (string do binário da Apple, doc Apple). Não
   tentar bater o número do painel.
5. Comandos de ferramenta: para descobrir caminho e para executar a limpeza oficial. Tamanho sempre
   medido pela travessia; número da ferramenta só como referência rotulada.
6. Rodar comandos externos com timeout (`simctl` travou >3 min).

## 10. Experimentos (`research/experimentos/frenteC/`)

| Arquivo | O que faz | Compilar/rodar |
|---|---|---|
| `ls_installed.rs` | bundle ids no stdin → nº de URLs, primeira URL, µs. FFI crua para CoreServices, sem crates | `rustc -O ls_installed.rs -o ls_installed` |
| `atribuir.py` | cadeia de prova da seção 1.2 para todos os containers; TSV | `python3 atribuir.py ./ls_installed` |
| `attrtag.c` | capacidade de attribution tag do volume, hash via `getattrlist`; `-q` tenta `F_QUERY_TAG` (abre o arquivo); `-mk` cria arquivo de teste | `cc -O2 -o attrtag attrtag.c` |

Binários e saídas ficaram no scratchpad, fora do iCloud.

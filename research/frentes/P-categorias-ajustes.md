# Frente P: o que cada categoria do Ajustes > Armazenamento conta (29/09)

Máquina: macOS 27.0, M4, sem sudo, terminal com FDA. Método igual ao da Frente M: achar a
extensão de cada categoria, ler no binário a regra de tamanho e refazer o número com as mesmas
funções (privadas só de leitura, via `dlopen`/`dlsym`) ou lendo os mesmos bancos.
Marcação: **(A)** lido no binário/código e reproduzido; **(B)** hipótese ou estimativa.
Valores "hoje" medidos em 29/09 entre 15:05 e 15:07 com `experimentos/frenteP/todas.py`.
Quando a pesquisa e a verificação divergiram, vale a verificação (as correções estão marcadas).

## 1. Resultado

- **15 categorias, 14 reproduzidas com grau A e 1 estimada (iCloud Drive, B).** Programas em
  `research/experimentos/frenteP/` (como rodar: `LEIAME.md` de lá).
- **Dados do Sistema da tela = (usado do Data − purgeable) − Σ categorias.** Com as categorias
  reproduzidas, hoje: **361.291.763.712 − 35.789.805.042 − 172.484.389.885 = 153.017.568.785
  (153,02 GB)**. O painel de 28/09 mostrou 150,36 (13:52) e 155,86 (14:11).
- **Conferido ao byte contra o log de 28/09:** Developer 2.050.097.152, Outros usuários 884.736,
  Música 36.429.056.548, Mensagens 214.752, e os zeros de TV, Livros, Podcasts, Lixo, GarageBand
  e Arquivos do iOS. O agregado "Developer and Others" (`DISK_MISC`) do último bloco (14:11:56,
  3.161.958.112) é, ao byte, Developer 2.050.097.152 + iCloud Drive 246.538.240 + Mail
  864.223.232 + Mensagens 214.752 + Outros usuários 884.736. O das 13:52 (4.819.314.400) fecha
  igual com iCloud 1.904.959.488 e Mail 863.158.272.
- **Sem conferência no mesmo minuto:** Aplicativos, Documentos, Fotos, Mail e iCloud Drive mudam
  entre medições (containers +165 MB em 20 min). O log de 28/09 já sumiu e o painel não foi
  aberto hoje. §4 diz como o dono fecha isso.
- **O que isto muda no produto:** Dados do Sistema é tudo o que está no volume Data e não cai
  numa das regras abaixo. As mais importantes para o walker: `~/Library` inteira fica fora de
  Documentos; só algumas partes dela voltam para Aplicativos, Mail, Developer, Mensagens e iOS
  (§3). Pastas ocultas da home (`~/.npm`, `~/.colima`...) são **Documentos**, não Dados do
  Sistema. Música e Fotos contam o que está no banco da biblioteca, não o tamanho da pasta.

## 2. Tabela

Valores em bytes. "Baldes" = pastas do walker que caem na categoria; o que sobra do balde vai
para Dados do Sistema (§3).

| Categoria (id) | Fonte | Grau | Reprodutor | Valor hoje | Baldes |
|---|---|---|---|---|---|
| **Aplicativos** (`com.apple.STMExtension.Applications`) | `ApplicationsStorageExtension.appex`: apps de um `NSMetadataQuery` (`com.apple.application-bundle` em `/Applications/`, `/System/Applications`, `~`) com o filtro `shouldIgnoreItemAtPath`; item = bundle (`_NodeSizingV2`) + container `~/Library/Containers/<x>/Data` do bundle ID; + `~/Library/Application Support` − `MobileSync` − overrides; só itens no volume da home; + restritos sem container | A | `stm_categorias apps` | 81.692.733.440 | `/Applications/*`, `~/Applications`, `.app` indexados na home fora de `~/Library`, `~/Library/Application Support/*` (menos `MobileSync`), `~/Library/Containers/<id>/Data` só dos apps listados, web apps do Safari em `Containers/com.apple.Safari.WebApp/Data/Library/Containers/<bid>`; `/System/Applications/*.app` (volume System, o walker não vê) |
| **Documentos** (`com.apple.STMExtension.Documents`) | `StorageManagementService`, `SPInfoDocumentsStorageUsageReporter`: `_NodeSizingV2(~)` pulando os excluídos e todo `.app` que é aplicação. Excluídos: `~/Applications`, `~/Desktop`, `~/Documents`, `~/Library`, a biblioteca do Fotos do sistema, 5 pastas de mídia fixas e os `_APPPATHS_` das extensões. Desktop e Documents só saem se "Mesa e Documentos" do iCloud estiver ligado (estado ≥ 2) | A | `stm_categorias documents` | 41.100.300.288 | `~` (arquivos soltos), `~/.<oculta>` (menos `~/.Trash`), `~/<pasta do usuário>` sem `.app`, `~/Downloads`, `~/Public`, `~/Movies` e `~/Music` menos as bibliotecas, `~/Pictures` menos a biblioteca do sistema; `~/Desktop` e `~/Documents` só com "Mesa e Documentos" desligado |
| **Música** (`com.apple.Music.MusicStorageExtension`) | `AMPLFrameworkClient` (domínio 1, kinds 0x60) → `AMPLibraryAgent` → `totalBytes` = Σ `FileSize` gravado em `Library.musicdb`, só faixas com arquivo local | A | `musica.py music` | 36.429.056.548 | `~/Music/Music/Media.localized` (parcial: só os arquivos de faixa da biblioteca) |
| **Fotos** (`com.apple.Photos.StorageManagementExtension`) | `CPSConfigurationClient estimateLibrarySize` → `PLFileUtilities calculateTotalSizeOfFilesAtPath:calculatePurgeable:YES allocatedSize:NO` na biblioteca do sistema; mostra total − purgeable (lógico, não alocado); 0 se a biblioteca está em outro volume | A | `fotos_pl` (r2 − r4) | 10.213.366.521 | `~/Pictures/Photos Library.photoslibrary` (menos os arquivos purgeable) |
| **TV** (`com.apple.TV.TVStorageExtension`) | Igual à Música, domínio 2, kinds 0x142, `Library.tvdb` | A | `musica.py tv` | 0 | `~/Movies/TV/Media.localized` (parcial: só itens da biblioteca) |
| **Livros** (`com.apple.iBooksX.DiskSpaceEfficiency`) | `BKAgent fetchBooks:`; descarta amostra, sem nome, sem ID e livro ubíquo (iCloud); tamanho = max(`BKAllocatedSize`, `kMDItemFSSize`) | A | `livros` | 0 (4 livros, todos no iCloud) | nenhum hoje; livro local fica em `~/Library/Containers/com.apple.iBooksX/Data` |
| **Podcasts** (`com.apple.podcasts.MacPodcastsStorageExtension`) | `MTEpisode` com `currentMediaEnclosure.localURL` não vazio e arquivo existente; soma alocado (`NSURLTotalFileAllocatedSizeKey`). Banco que não abre: tamanho de `<grupo 243LU875E5.groups.com.apple.podcasts>/Library/Cache` | A | `bancos.py podcasts` | 0 | nenhum hoje (sem download) |
| **Lixo** (`com.apple.STMExtension.Trash`) | `~/.Trash` + `<vol>/.Trashes/<uid>`; filhos sem "."; só os do volume da home; lixo do iCloud vazio | A | `stm_categorias trash` | 0 | `~/.Trash/*`, `/.Trashes/<uid>` |
| **GarageBand** (`com.apple.STMExtension.GarageBand`) | 10 caminhos fixos de `/Library/Application Support/{GarageBand,Logic}` e `/Library/Audio`, só se no FS raiz | A | `stm_categorias garageband` | 0 (nenhum existe) | `…/GarageBand/Instrument Library`, `…/Logic/{Sampler Instruments, EXS Factory Samples, Alchemy Samples, Ultrabeat Samples, Channel Strip Settings, Plug-In Settings}`, `/Library/Audio/Impulse Responses/Apple`, `/Library/Audio/Apple Loops/Apple`, `…/GarageBand/Learn to Play/Basic Lessons` |
| **Arquivos do iOS** (`com.apple.STMExtension.iOSFiles`) | Filhos de `MobileSync/Backup` (sem "." e sem `Logs`) + `*.ipsw` do Configurator e de `~/Library/iTunes/* Software Updates` | A | `stm_categorias ios` | 0 | `~/Library/Application Support/MobileSync/Backup`, `*.ipsw` em `~/Library/Group Containers/{K36BKF7T3D,4WXS7A4F54}.group.com.apple.configurator/Library/Caches/Firmware` e em `~/Library/iTunes/{iPhone,iPad,iPod,Apple TV} Software Updates` |
| **Desenvolvedor** (`com.apple.STMExtension.Developer`) | Caminhos fixos do Xcode e das CLT; `~/Library/Caches/com.apple.dt.Xcode` vale 0 porque o sandbox da extensão só lê `~/Library/Developer` e `/AppleInternal/Developer` | A | `stm_categorias developer` | 2.050.097.152 | `~/Library/Developer/CoreSimulator/Caches`, `~/Library/Developer/Xcode/{DerivedData, Installs, Archives, *OS DeviceSupport}`, `/Library/Developer/CommandLineTools`, `/Library/Developer/CoreSimulator`, `/AppleInternal/Developer/KDKs/*` |
| **Mail** (`com.apple.STMExtension.Mail`) | `~/Library/Mail` (`calculateSizeOfFolderAtPath`) − purgeable do `com.apple.MailCacheDelete`; negativo = desconhecido | A | `stm_categorias mail` | 890.437.632 (purgeable 0) | `~/Library/Mail/*` |
| **Mensagens** (`com.apple.messages.StorageManagementExtension`) | `IMDAttachmentSpaceTakenByAttachmentClass(0)`: Σ `total_bytes` de `attachment` com `hide_attachment=0 AND transfer_state=5` em `chat.db` (sem condição de `ck_sync_state`) | A | `bancos.py messages` | 214.752 | `~/Library/Messages/Attachments/<xx>` (parcial: anexos baixados e visíveis) |
| **Outros usuários** (`com.apple.STMExtension.OtherUsers`) | `/Users/Shared` + `/Users/Deleted Users` + `SMUtilities_Client obtainTotalSizeOfOtherUsersOnSystem…` (homes no mesmo volume, fora de `/private/var`) | A (fixo) / B (outros usuários) | `stm_categorias otherusers`; outro usuário: `sudo …/frenteM/stmsize_ai /Users/<outro>` | 884.736 | `/Users/Shared`, `/Users/Deleted Users`, `/Users/<outro>` |
| **iCloud Drive** (`com.apple.CloudDocsDaemon.StorageManagement`) | A + B. A = `calculateNonPurgeableSpaceUsageOfDomain` (fileproviderd): Σ `metadata_physical_size` não despejável do banco FPFS. B = `BRSyncProxy computePurgeableSpaceWithReply:` (bird): sessão do bird menos stages, graveyard e freelist. Itens fixados não somam | **B** | `icloud.py` | ≈ 107.298.816 (estimativa; A 25.763.840 + B ≈ 81,5 MB) | `~/Library/Application Support/CloudDocs/session` (menos stages x e u); em `~/Library/Mobile Documents/*`, `~/Desktop` e `~/Documents` só os itens **não despejáveis** |

`Σ categorias` hoje = 172.484.389.885. "Developer and Others" (Desenvolvedor + iCloud Drive +
Mail + Mensagens + Outros usuários) = 3.048.933.088.

### 2.1 Detalhes que mudam o número

- **Aplicativos (corrigido pela verificação):** um app restrito (SF_RESTRICTED) **sem container**
  sai da lista, mas o `stm_validatedSize` dele entra no total (acumulador `[sp+0x60]`,
  `0x100008b88`). Hoje são 8 apps de `/System/Applications` com 13.078.528 B. Outras regras:
  bundle IDs `com.apple.{Music,iBooksX,mail,TV,podcasts,Photos,MobileSMS}` ignorados; apps sob
  `~/Library`, no lixo, aninhados, e restritos em subpasta (`/System/Applications/Utilities`)
  ignorados; `/Applications/<X>/X.app` (ou override `USE_PARENT_FOLDER_SIZE`: Cinema 4D, Houdini)
  mede a pasta inteira. Só atualiza se |Δ| ≥ 1.000.000. Composição hoje: bundles 51.915.771.904
  (133 itens) + containers 6.251.442.176 + App Support 23.505.981.440 + restritos 13.078.528.
- **Documentos:** o log de 28/09 (`Updating Documents size due to new excluded paths`) mostra a
  lista de excluídos com `Music Library.musiclibrary`, `Previous Libraries.localized` e
  `Media.localized` da Música e os caminhos da TV. Isso fecha a pergunta da verificação da
  Música: os arquivos dessas pastas que não estão na biblioteca vão para **Dados do Sistema**
  (A). O reprodutor pula só os `.app` que o Spotlight indexou; o serviço pula qualquer bundle
  que seja aplicação (B nesse detalhe). O `SPLastDocumentsSize` (43.420.205.056) é de 27/09
  22:34 e não serve de comparação.
- **Música/TV:** é o `FileSize` gravado no banco, não o tamanho da pasta. Hoje
  `Media.localized` tem 37.992.480.768 alocados; 2023 arquivos fora da biblioteca (≈ 1,56 GB),
  `Music Library.musiclibrary` (≈ 83 MB) e `Previous Libraries.localized` (≈ 217 MB) ficam em
  Dados do Sistema. A regra "só arquivo no volume da home" não foi achada no agente (B).
- **Fotos:** conta tamanho **lógico**, não alocado (alocado seria 10.379.378.688 − 5.550.080).
- **Mensagens (corrigido):** a extensão passa 0, então o WHERE não tem `ck_sync_state`. Hoje as
  variantes "sem" e "!= 1" dão o mesmo; mudam com anexos sincronizados pelo iCloud.
- **iCloud Drive:** o conteúdo **baixado e despejável** do iCloud Drive (≈ 58,8 GB físicos no
  FPFS, medido pela pesquisa: `com~apple~CloudDocs` ≈ 38,1 GB, WhatsApp ≈ 20,7 GB) **não** entra na categoria e, como
  Documentos exclui `~/Library` (e Desktop/Documents com "Mesa e Documentos" ligado), cai em
  **Dados do Sistema**. A máscara de `metadata_is_evictable` não foi lida, mas "= 0" é o único
  valor compatível com o log (as outras máscaras dariam ≥ 22,5 GB). B vem da parte do bird, que
  oscilou de 1,66 GB para 220 MB em 19 min em 28/09.

## 3. O que fica em Dados do Sistema (regras para o walker)

Tudo o que está no volume Data e não entra numa linha da §2. Em especial:

| Balde do walker | Destino |
|---|---|
| `~/Library/Caches/*` (inclui `CloudKit/com.apple.bird` ≈ 23,1 GB e `com.apple.dt.Xcode`) | Dados do Sistema |
| `~/Library/Containers/<id>` de apps não listados (não instalados, não indexados, ou dos bundle IDs ignorados: Music, Books, Mail, TV, Podcasts, Photos, Messages) | Dados do Sistema |
| `~/Library/Group Containers/*` (exceto `*.ipsw` do Configurator) | Dados do Sistema |
| `~/Library/Mobile Documents/*` e `~/Desktop`, `~/Documents` com iCloud: parte despejável | Dados do Sistema |
| `~/Library/Messages` menos anexos baixados e visíveis | Dados do Sistema |
| `~/Library/Developer` fora dos caminhos do Developer (`CoreSimulator/Devices`, `Xcode/UserData`...) | Dados do Sistema |
| `~/Library/Application Support/MobileSync` fora de `Backup` | Dados do Sistema (sai de Aplicativos e não entra em iOS) |
| Resto de `~/Library` (Preferences, Logs, Metadata, Biome, etc.) | Dados do Sistema |
| `~/Music/Music/Media.localized` fora da biblioteca, `Music Library.musiclibrary`, `Previous Libraries.localized`; `~/Movies/TV/*` fora da biblioteca | Dados do Sistema |
| `/Library/*` (exceto GarageBand e Developer acima), `/private/var/*`, `/usr/local`, `/opt` e tudo fora de `/Applications`, `/Users` | Dados do Sistema |
| Modelos do Apple Intelligence (Frente M) | Dados do Sistema (a tela só os mostra no (i) de macOS) |

Consequências de granularidade para `anon::Ctx::bucket`:
- `~/Music` e `~/Movies` no nível 1 misturam Documentos, Música/TV e Dados do Sistema. Descer
  até `~/Music/Music/*` e `~/Movies/TV/*`; para Música exata, ler o `Library.musicdb`
  (`musica.py`), senão o erro hoje é ≈ 1,8 GB.
- `~/Library/Containers` precisa saber quais apps o Spotlight lista (mesma query da extensão).
- `/Library/Application Support/GarageBand/Learn to Play` e os Group Containers do Configurator
  precisam de mais um nível ou de filtro por extensão (`.ipsw`).
- Mail e Fotos descontam purgeable; esse pedaço vira espaço purgeable, não Dados do Sistema.

## 4. Dados do Sistema = usado − Σ categorias

```
Dados do Sistema = CapacityInUse(Data) − purgeable − Σ categorias
  CapacityInUse(Data): diskutil apfs list -plist, volume de papel Data (hoje disk3s5)
  purgeable: NSURLVolumeAvailableCapacityForImportantUsageKey − NSURLVolumeAvailableCapacityKey
  Σ categorias: as 15 da §2
```

- Vem da síntese §1 (Usado = Total − livre − purgeable − outros) com o macOS da tela (Frente M):
  a tela põe o macOS em "usado" e tira de novo, então sobra o Data menos o purgeable.
- Com as categorias de 28/09 13:52, essa conta deu 150,34–150,48 contra 150,36 na tela
  (Frente M §2.3). Às 14:11 sobram 6,02 GB sem explicação (Frente M §5.3); não reabri.
- Hoje: `python3 todas.py <bin>` → 153.017.568.785. O iCloud Drive entra como estimativa (B);
  em 28/09 a tela deu 0,25 e 1,90 GB para ele em 19 min, então o erro possível chega a ~1 % do resultado.
- Critério da fase 2 (Σ reproduzida ±1 % do log): **ainda não conferido no mesmo instante.**
  Contra o último bloco de 28/09 (Σ 174.687.864.619), a Σ de hoje é 1,3 % menor, e quase tudo é
  Documentos (43,42 do cache velho contra 41,10 medido agora).

## 5. O que falta e como o dono confere

1. **Rodar junto com a tela** (fecha Aplicativos, Documentos, Fotos, Mail, iCloud Drive e a
   conta inteira). Terminal 1:
   `log stream --style ndjson --predicate 'process == "StorageManagementService" OR process BEGINSWITH "com.apple.STMExtension"' > /tmp/painel.ndjson`.
   Abrir Ajustes > Geral > Armazenamento, esperar ~2 min. Terminal 2:
   `python3 todas.py <bin>; defaults read com.apple.StorageManagement.Service SPLastDocumentsSize`.
   Comparar cada categoria com `StorageLogInvestigation - <id>: <bytes>` e Dados do Sistema com
   a tela.
2. **Outros usuários** (se houver outra conta): `sudo research/experimentos/frenteM/stmsize_ai /Users/<outro>`.
3. **Casos não exercitados nesta máquina** (hoje valem 0): Lixo cheio, backup do iPhone, `.ipsw`,
   livro local, episódio de podcast baixado, faixa em disco externo. Não criei nenhum para não
   mexer em dados do dono.

## 6. Perguntas abertas

1. iCloud Drive (parte B): qual argumento de `computePurgeableSpaceWithReply:` entra, o que o
   fsctl `0xC0284A20` do bird devolve (o reprodutor usa o dir-stats `0xC1104A71`), onde ficam
   `ciconia` e `cacheDirPath`, e por que B oscila tanto.
2. Estado "Mesa e Documentos": qual API de leitura alimenta `serviceStatesByServiceIdentifier`
   (`SPInfoFetchCloudServiceStatesOperation`)? O reprodutor assume ligado (log de 28/09).
3. Aplicativos: `stm_name` (de `NSURLLocalizedNameKey`) já existe quando a regra de pasta-pai
   roda na primeira passagem? Com Steam, os `steamapps` fora de `common/` (872.448) somem ou
   duplicam? A correção dos restritos (+13 MB) fica abaixo do drift e só se confirma com a tela.
4. Documentos: o reprodutor só pula `.app` indexados; um `.app` não indexado em `~/.<oculta>` ou
   `~/dev` entra a mais.
5. Fotos: para onde vão o purgeable da biblioteca (5,5 MB) e bibliotecas que não são a do sistema?
6. Música/TV: onde o agente filtra "arquivo no volume da home"?
7. Outros usuários: semântica completa do `userFilterBlock` e saída do `SMUtilities_Client`.
8. Livros: correspondência entre as chaves do `BKAgent` e as colunas do `BKLibrary` quando houver
   livro local.
9. Mail: quando `com.apple.MailCacheDelete` reporta purgeable > 0.
10. Dados pessoais no scratchpad: cópias de `chat.db`, `BKLibrary`, `MTLibrary` e dos bancos do
    bird/FPFS ficaram em `/private/tmp/claude-501/.../scratchpad` (inclusive `cet/fpdb`). A regra
    proíbe apagar fora do repo; o dono decide se remove.

# Catálogo de ideias do SDTally (versão revisada em 03/10/2026)

Fonte: 89 ideias de 6 frentes de pesquisa (visualizadores, assistentes e limpadores, monitores, CLI e IA, conhecimento, design). Depois de juntar as parecidas, ficaram 58, mais uma (G9) que antes só aparecia no Top 12. O que já está planejado (boas-vindas, áreas, cores, histórico, barra de menus, exportar, "o que fazer", acesso para IA) só entra aqui quando a ideia é um jeito melhor de fazer.

Legenda: esforço **P/M/G**, impacto **alto/médio/baixo**.

**Verificado:**
- **Projeto lido:** PLANO.md, README.md, docs/json-schema.md, research/mapa-de-fontes.md, `app/Package.swift`, `app/montar.sh` e `app/Sources/SDTallyHelper/main.swift`. Também o uso de `UserDefaults` em `app/Sources/SDTally/Painel.swift`.
- **SDK do macOS 27.0 (Xcode):**
  - `LongRunningIntent` existe só a partir do 27 (`@available(anyAppleOS 27.0)`). `IndexedEntity` existe a partir do 15.
  - `ControlWidget` existe no macOS a partir do 26.0. WidgetKit, TipKit, Charts (`SectorMark` no 14+), `MeshGradient`, QuickLookUI, FinderSync, UserNotifications e ServiceManagement estão presentes.
  - FoundationModels existe a partir do 26.0.
  - `searchfs` está em `unistd.h`. `ri_diskio_byteswritten` e `ri_logical_writes` estão em `sys/resource.h`. `ATTR_CMN_ADDEDTIME` e `ATTR_CMNEXT_PRIVATESIZE` estão em `sys/attr.h`. `NSURLVolumeAvailableCapacityForOpportunisticUsageKey` existe (10.13+).
  - O processador de metadados de App Intents (`appintentsmetadataprocessor`) só existe no toolchain do Xcode, e o `swift build` não o executa.
- **Neste Mac:**
  - `diskutil apfs listSnapshots / -plist` traz as chaves `Purgeable`, `LimitingContainerShrink`, `SnapshotXID`, `SnapshotUUID` e `SnapshotName`.
  - `man tmutil` **não tem** `sizelocalsnapshotsof`. Por isso a aposta 3 foi corrigida.

**O que esta revisão mudou:**
- **Helper.** Ele só aceita conexões do app: `setConnectionCodeSigningRequirement(reqApp)`. O `reqCLI` serve só para conferir o binário que o helper vai executar. Ou seja, a frase "o `reqCLI` já aceita o CLI" estava errada. F3 foi dividida em duas (F3a e F3b), e a F3b passou para esforço G.
- **App Intents** exige `xcodebuild` (F5 passa a M/G). **Widgets** exigem um alvo `.appex` que o SwiftPM não gera (E3 passa a G).
- **A1 e G2** misturavam a conta do volume Data com "Dados do Sistema". Os termos somam o usado do Data, não Dados do Sistema, e essa separação depende da Fase 2. As duas foram corrigidas.
- **B4** mostrava a regra das 24 h como se fosse um horário medido. Corrigido.
- **D7** (projeção) foi para "Não fazer": viola a regra de só medição. Pode voltar se o dono decidir.
- **Impactos rebaixados:** B6 (alto → médio), B10 (médio → baixo) e F8 (médio → baixo, por risco de o conteúdo chegar ao Private Cloud Compute).
- **Riscos de privacidade acrescentados:** IMEI e telefone no Info.plist dos backups (A10); Universal Clipboard (F10 e G6, resolvido com `.currentHostOnly`); esquema de URL chamável por qualquer página (F5); `ollama` e `docker` falam com servidores locais por socket (B9); o plano B do widget dispara o aviso de dados de outro app (E3).
- **Top 12 reordenado.**

**Duas decisões do dono que destravam metade do catálogo:**

1. **Escrita no container do próprio app.** O princípio 2 do PLANO diz "nenhuma escrita no volume, nem de metadado". **Na prática o app já grava:** `Painel.swift` salva `hashHelper` em `UserDefaults`, ou seja, em `~/Library/Preferences`. O histórico, o "já sei, mantenha", o TipKit e as sentinelas precisam gravar em `~/Library/Application Support/SDTally`. Proposta:
   - liberar a escrita só nesse diretório e nas preferências do app, de forma declarada;
   - mostrar isso na conta como a linha "dados do SDTally", com um botão "esquecer histórico";
   - no modo crise, zero escrita, o que inclui deixar de gravar o `hashHelper`.
2. **Fase 3 (limpeza guiada) contra a restrição "nunca apaga".** O PLANO ainda prevê `trashItemAtURL` e a execução de comandos com simulação. O README também ainda diz "Sempre mover para o Lixo" e "Deduplicar por tamanho + hash" (requisito 4). Já a restrição atual diz que o app só lê e abre o Finder, o Ajustes ou o Terminal. O catálogo segue a restrição atual. Se ela for a definitiva, a Fase 3 vira "recomendar e conferir" (C1, C2 e C3) e o README precisa ser corrigido.

---

## A. Ver o disco

**A1. A barra que se desdobra** · M · alto
Um clique num segmento da barra faz esse segmento ocupar a largura toda (`matchedGeometryEffect`) e se dividir em partes medidas, com breadcrumb e Esc para voltar. O mesmo gesto serve para descer no ranking. Segmento cheio significa algo somável; contorno ou hachura significa informativo (purgeable, preso em snapshot).
- **Correção:** os termos de `data_volume.terms` somam o **usado do volume Data**, não "Dados do Sistema". Até a Fase 2 marcar cada linha como "está em Dados do Sistema" ou "está em outra categoria", o que se desdobra é o volume Data. O primeiro nível são as categorias do log (`settings_screen.screen.categories`) mais Dados do Sistema; o segundo, os termos e o ranking, com o rótulo "volume Data inteiro". Desdobrar só o segmento "Dados do Sistema" mostraria parcelas que pertencem a outras categorias.
- Inspiração: animação "blossom" do DaisyDisk, transições de período do Stripe, barra do Armazenamento do iOS.
- Dado: `data_volume.terms`, `top`, `settings_screen.screen`, `sdtally <caminho> --json`.
- Risco: misturar somável com informativo quebraria o princípio 5, então a regra visual tem de ser fixa. Itens com menos de ~0,5 % vão para um segmento "menores". Com Reduzir Movimento, a animação vira crossfade.

**A2. Mapa da conta (treemap do volume inteiro) e três vistas sincronizadas** · M (lista + treemap) / G (com anéis) · alto
Treemap squarified em `Canvas`. O primeiro nível são os termos da conta: árvore de arquivos, um bloco por snapshot (privado), pastas protegidas, apagados-abertos por processo, xattrs e "sem explicação" hachurado. Purgeable e "preso" entram como listras, nunca como área extra. Pacotes viram um bloco só, com "Abrir no app dono". ⌘1, ⌘2 e ⌘3 alternam entre lista com barras (padrão), treemap e anéis de área proporcional. A seleção é compartilhada entre as vistas, e o VoiceOver lê "nome, GB, % do pai, fonte".
- Inspiração: "Show entire volume" do GrandPerspective, export do WizTree, hidden space do DaisyDisk, Baobab, Disk Inventory X, QDirStat; discussão no HN sobre o sunburst distorcer a área.
- Dado: JSON atual mais um **nó de árvore podado** no schema (pastas ≥ X MB até a profundidade N, podado no Rust).
- Risco: muda o schema (`schema_version` 2). O `SectorMark` (macOS 14+) não varia o raio, então os anéis pedem um `Canvas` próprio. Lançar lista e treemap primeiro; anéis depois.

**A3. Modos de cor e painel de idade** · M · alto
Seletor de cor no mapa: idade (mtime), criação, app dono e "privado × compartilhado". O modo "conta em Dados do Sistema × outra categoria" só entra depois da Fase 2. Painel "Idade" com bytes e número de arquivos por ano (por mês nos últimos dois anos), esmaecendo os anos parados.
- Inspiração: modos de cor do GrandPerspective, File Age Statistics do QDirStat, filtro >2years do SpaceSniffer.
- Dado: novo. `ATTR_CMN_MODTIME` e `ATTR_CMN_CRTIME` no mesmo `getattrlistbulk`, agregados em baldes no Rust; `kMDItemLastUsedDate` onde existir (com a fonte "Spotlight").
- Risco: não oferecer "último acesso" (ver "Não fazer"). Medir antes quanto o buffer do `getattrlistbulk` cresce. Dono por pasta depende de `owners.rs`, que ainda não existe.

**A4. Filtro digitável com máscara, e `sdtally find` com a mesma sintaxe** · M · alto
Campo com sintaxe curta (`>1g`, `privado<10m`, `idade>2a`, `dono:com.apple.bird`, `clone`, `em:sistema`). Por padrão funciona como máscara: o total não muda e o que não casa fica esmaecido. "Aplicar como filtro" põe "filtrado: X de Y" no cabeçalho.
O CLI ganha `sdtally find [raiz] --min 500MB --kind dataless|clone|sparse --unused-since 365d --json` (NDJSON). Ele recusa rodar sem `--min` ou sem caminho e substitui o `find / -size` que as IAs improvisam. Filtros podem ser salvos com nome, o que depende da decisão 1.
- Inspiração: SpaceSniffer, WizTree, máscara × filtro do GrandPerspective, tabela file do osquery, gdu/dust.
- Dado: atributos já lidos, mais idade e dono.
- Risco: parser novo, com teste puro em Rust. Filtrar durante a varredura deixa a conta "parcial (filtro)". `em:sistema` depende da Fase 2.

**A5. Aba Descobrir** · P · médio
Perguntas prontas, cada uma clicável até o arquivo, com "por que importa":
- maiores arquivos do volume;
- esparsos (aparente × alocado);
- grupos de clone;
- hardlinks múltiplos;
- pastas com mais de 100 mil arquivos pequenos;
- Preferences acima de 100 MB;
- lixeiras órfãs (`CloudStorage/*/.Trash`);
- xattrs grandes.
- Inspiração: menu Discover do QDirStat, abas do WinDirStat 2, Sparsity e Precize (Oakley).
- Dado: `TOTALSIZE`, `ALLOCSIZE`, `CLONEID`, `LINKCOUNT`, `EXT_FLAGS`.
- Risco: duplicados por hash ficam de fora (ver "Não fazer"). "SQLite aberto crescendo sem FSEvents" saiu daqui porque exige histórico; está em E1.

**A6. Primeira resposta em segundos com searchfs** · M · médio
Enquanto a travessia roda (30–50 s), `searchfs` com faixa de tamanho lista os arquivos gigantes em poucos segundos, como "Maiores arquivos (prévia)".
- Inspiração: OpenDisk (getattrlistbulk + searchfs), MFT do WizTree.
- Dado: novo. `searchfs` existe em `unistd.h` (SDK 27). `VOL_CAP_INT_SEARCHFS` foi conferido em `/` e no Data (macOS 27).
- Risco: não testado com critério de tamanho nem como root. Falta confirmar como lida com enxertos, dataless e travessia de volume, e conferir no 15 e no 26 pelo CI. É só prévia, com rótulo.

**A7. Medição viva: navegar durante a varredura e frescor por linha** · M · médio
`sdtally --json --stream` emite nós parciais. O app preenche o ranking e o mapa com o selo "parcial: X % das pastas" e só mostra a conta quando ela fecha. Depois, o FSEvents relê as pastas alteradas e cada linha ganha "medido há 3 min" ou "mudou depois da medição".
- Inspiração: SpaceSniffer, OpenDisk (replay de FSEvents), diskonaut.
- Dado: travessia e FSEvents (Frente R: incremental de 1,9 s, −0,026 % em 1 h).
- Risco: número parcial sem rótulo fere o "só medição". Mostrar Δ só acima de um limiar, porque há corridas. Banco SQLite aberto não gera evento (Frente R), então uma travessia completa periódica continua necessária.

**A8. Tipos de arquivo e histograma de tamanhos** · P · médio
Bytes e contagem por família (UTType resolvido uma vez por extensão) e histograma log2 com frases como "312 mil arquivos < 4 KB ocupam 1,3 GB".
- Inspiração: File Kind Statistics do Disk Inventory X, extensões do WinDirStat, histograma do QDirStat.
- Dado: nome, alocado e contagem, já lidos.
- Risco: o rótulo vem da extensão, mas o número é medido. Deixar isso explícito.

**A9. AssetsV2 em linguagem humana** · M · médio
Cada `com_apple_MobileAsset_*` vira um item com nome humano (vozes, dicionários, modelos de IA, runtimes de simulador), tamanho, a linha do Ajustes em que cai e o comportamento de coleta lido do Info.plist. Botões levam às vias oficiais (Conteúdo Falado, Xcode > Components).
- Inspiração: The Apple Wiki (Asset Types), Oakley sobre cryptexes, xcode-phantom-runtime-fix.
- Dado: dir-stats por pasta (conferido ao byte em 85 de 99 pastas; comprimido e enxerto explicam o resto) e Info.plist lido pelo helper.
- Risco: `__AssetDefaultGarbageCollectionBehavior` não é documentado, então mostrar o valor bruto. A tabela de nomes é mantida à mão. Nunca sugerir apagar dentro do AssetsV2.

**A10. Backups de iPhone e iPad legíveis** · P · médio
Cada UDID de `MobileSync/Backup` vira uma linha: aparelho, iOS, data, criptografado ou não, alocado e "libera se apagar". Destaca aparelhos sem backup há mais de 1 ano e duplicados. A ação é Gerenciar Backups no Finder.
- Inspiração: Gerenciar Backups do Finder, iMazing, nota Apple 108809.
- Dado: só as chaves `Device Name`, `Product Version` e `Last Backup Date` do Info.plist e `IsEncrypted` do Manifest.plist (FDA). Nunca o Manifest.db.
- Risco: **o Info.plist também tem IMEI, número de telefone, número de série e ICCID.** Ler só as chaves acima, nunca guardar o resto no JSON e anonimizar o nome do aparelho nas exportações.

**A11. Vale a pena reiniciar? (e a linha "macOS" desdobrada)** · P · médio
Três colunas medidas:
1. **Libera ao reiniciar:** apagados-abertos e swap.
2. **Teto do Modo Seguro:** purgeable por serviço, com o aviso de que costuma liberar bem menos.
3. **Não muda:** snapshots, iCloud, simuladores.

Abaixo, a linha macOS dividida por volume do container (System, Preboot, VM, Update, Recovery).
- Inspiração: Oakley sobre Modo Seguro e volumes; Apple StackExchange sobre VM.
- Dado: lsof, CacheDelete, `diskutil apfs list -plist`; novos e sem root: `sysctl vm.swapusage`, `pmset -g`.
- Risco: o swap volta, então rotular "preso agora". Nunca recomendar mudar o hibernatemode. Rótulo "teto", nunca "vai liberar".

---

## B. Entender

**B1. Ficha única: o que é, de quem é, como agir** · M · alto
Junta a "ficha de recomendação" e as fichas "o que é isto?". É a camada de dados do "o que fazer" planejado. Campos fixos, sempre na mesma ordem:
1. o que é;
2. dono e evidência;
3. em que linha do Ajustes cai (depois da Fase 2);
4. quanto libera de verdade (privado, com o alocado ao lado e "preso em snapshot");
5. natureza, com 4 rótulos: "o macOS libera sozinho", "volta sozinho", "é seu, decisão sua", "só pelo app dono";
6. o que se perde;
7. como agir: abrir o Finder, o Ajustes ou o app dono, ou copiar um comando oficial que tenha simulação;
8. fonte;
9. "conferido em macOS 27.0".

A base é um JSON versionado embutido no Rust, então `why`, `--json`, o MCP e o app usam o mesmo texto. A fila é ordenada por "libera de verdade" e por risco, sem checkbox e sem total geral.
- Inspiração: recomendações do Armazenamento do iPhone, Windows 11, StorageRadar, DevCleaner (Seguro/Atenção/Perigo), Files by Google (perda de vídeo de família), série Explainer do Oakley.
- Dado: `PRIVATESIZE`/`ALLOCSIZE`, fsctl de snapshot, CacheDelete por serviço, docs/04-catalogo-de-alvos.md, Frente C; `Purgeable` por snapshot via `diskutil apfs listSnapshots -plist` (chave conferida).
- Risco: o texto envelhece, então o selo de versão some quando a versão do macOS muda. Deep links do Ajustes mudam entre 15, 26 e 27 (testar no CI, com fallback). A ficha diz explicitamente "não faça" para SIP, `chflags` e `/dev/null`.

**B2. Pedra de Roseta dos números de espaço (inclui os três "livres")** · M · alto
Uma tela alinha, no mesmo instante: o cabeçalho do Ajustes (log), o livre do container, ImportantUsage, OpportunisticUsage, statfs/df e o usado do Data. O Finder aparece como "não legível", com um campo para o usuário digitar o que vê. Entre cada par, a diferença é dividida em termos medidos (purgeable por serviço, snapshots, outros volumes, metadados).

Também mostra:
- o congelamento do purgeable por instância do painel (achado de 02/10: Y fixo de 08:27 a 12:20);
- a linha "o macOS avisa abaixo de N GB (com.apple.diskspaced); faltam X GB", só se a chave for legível.

Os alertas usam o livre físico.
- Inspiração: Oakley (fontes do livre, o Finder confunde, aritmética do livre), DaisyDisk hidden space, issue #3249 do Stats, DiskPulse, iStat Menus.
- Dado: já medidos, mais `NSURLVolumeAvailableCapacityForOpportunisticUsageKey` (conferida no SDK), preferências do diskspaced (CFPreferences, só leitura) e linhas "Purge: goal/Result" do log.
- Risco: chaves do diskspaced e linhas do log não são documentadas; sem elas, `not_available`. A origem do "available" Y da tela segue em aberto (mapa §1). Nunca sugerir mudar o limiar.

**B3. Raio-X dos snapshots** · M · alto
Para cada snapshot: criador pelo nome (TM, os.update, CCC, ChronoSync), Purgeable, `LimitingContainerShrink`, privado e cumulativo, idade e um veredito com causa medida:
- "normal (<24 h)";
- "mais recente, necessário ao próximo backup";
- "órfão: o TM fez backup depois";
- "backups não fecham desde DD/MM" (do plist do TM);
- "de terceiro: só o app dono apaga".

Faixa temporal em Swift Charts. Botões para o Utilitário de Disco e para Ajustes > Time Machine. `tmutil deletelocalsnapshots <data>` é só copiado.
- Inspiração: colunas de snapshot do Utilitário de Disco, DaisyDisk, Oakley (24 h, órfãos), itpro-tips (112 GB presos em os.update).
- Dado: `fs_snapshot_list` e fsctl de privado; novos: `diskutil apfs listSnapshots -plist` (chaves `Purgeable` e `LimitingContainerShrink` conferidas neste Mac) e `/Library/Preferences/com.apple.TimeMachine.plist`.
- Risco: as 24 h são regra, não medição; nunca mostrar "expira em X" como número. O fsctl só foi validado no 27 (o CI não cria snapshot). O plist pode exigir FDA ou o helper. A chave de resultado do último backup precisa ser confirmada.

**B4. Painel "O macOS cuida disso sozinho"** · P · alto
Faixa no Resumo: "Estes X GB não pedem nada de você". Lista:
- o purgeable por serviço;
- os snapshots do TM com a idade medida e a frase "o TM costuma removê-los após 24 h (regra da Apple, não medição)";
- o Lixo automático ligado e quanto do Lixo já passou de 30 dias;
- as atualizações baixadas.

Quando a idade passa de 26 h, vira sentinela (E1).
- **Correção:** a versão anterior mostrava "deveria sair às HH:MM", o que apresentava a regra como horário medido e contradizia o B3.
- Inspiração: suporte Apple (o que o macOS apaga sozinho), Storage Sense, Plumby, Oakley.
- Dado: CacheDelete e snapshots; novos: `FXRemoveOldTrashItems` do `com.apple.finder` (a confirmar), `ATTR_CMN_ADDEDTIME` no `~/.Trash` (conferido no SDK), dir-stats de MacSoftwareUpdate.
- Risco: a chave do Finder não é documentada; sem ela, "não medido". Não prometer quanto será purgado.

**B5. Assinaturas de bugs conhecidos** · M · alto
Motor de regras local: cada regra tem uma condição medida, uma faixa de versões e o link do relato. Regras iniciais:
- idleassetsd > 5 GB;
- dezenas de `.bundle` de 64–68 MB do mediaanalysisd;
- Biome > 1 GB;
- `.Spotlight-V100` desproporcional;
- runtime de simulador fantasma;
- pasta comum com GBs em `/Volumes` (montagem SMB que falhou);
- os.update não purgável e antigo;
- `.Trash` do CloudStorage;
- várias `Data_Rebuild*` no Mail.

A saída mostra "padrão reconhecido, por que disparou (números), via oficial, relato".
- Inspiração: Major/Minor issues do EtreCheck; relatos na Apple Community, MacRumors e MPU; xcode-phantom-runtime-fix.
- Dado: travessia, histograma por pasta (novo e barato), getmntinfo, `diskutil apfs listSnapshots`, `xcrun simctl runtime list -j` (como usuário, com timeout, só se `xcode-select -p` existir, para não abrir o instalador das CLT).
- Risco: falso positivo, então limiares conservadores e sempre visíveis. Relato não é nota da Apple; citar como relato.

**B6. Dono com nível de evidência e "sem dono identificado"** · M · médio
Junta a evidência de dono com a vista de recibos. Toda atribuição mostra a cadeia de prova e um nível:
- **provado:** bundle id + LaunchServices, recibo do Instalador, `INSTALL_RECEIPT` do Homebrew, `MCMMetadataIdentifier`;
- **provável:** nome parecido;
- **desconhecido.**

"Órfão" só com prova dupla: nenhum app com aquele id em nenhum volume montado, e data de último uso. Lista os "pacotes cujo app já não existe". Itens só "prováveis" nunca recebem ação. Botão "Ver a prova".
- Inspiração: vistas Packages e Unpackaged do QDirStat, sensibilidade do Pearcleaner; falsos positivos do CleanMyMac (Inbox do DEVONthink, música tratada como cache do Spotify).
- Dado: `/var/db/receipts` via lsbom, plist do containermanagerd, `LSCopyApplicationURLsForBundleIdentifier`, Spotlight.
- Risco: Spotlight desligado dá falso "não encontrado", então tratar como "não medido". App em volume desmontado parece órfão. **O ganho medido aqui é pequeno (0,02–0,06 GiB), por isso o impacto caiu para médio.** O valor maior está em servir de base para B7. `owners.rs` ainda não existe.

**B7. Custo total por app com último uso** · M · alto
Uma linha por app: bundle + container + Group Containers + Application Support + Caches + LaunchAgents, cada parte com o privado, e o último uso. Texto honesto: "arrastar para o Lixo deixa 4,2 GB para trás, nestes lugares". A ação é abrir no Finder.
- Inspiração: "Último uso" do Armazenamento do iPhone, Apps não usados do Windows, Files by Google, Assistant do CleanMyMac.
- Dado: travessia, B6, `kMDItemLastUsedDate`, categoria Aplicativos (Frente Q).
- Risco: ferramentas de CLI não têm data, então mostrar "sem registro de uso", nunca "nunca usado". Depende de B6.

**B8. iCloud Drive: fixado, despejável, parado e só na nuvem** · P · alto
"iCloud Drive (baixado)" vira linhas medidas:
- **fixado:** xattr de pin do File Provider; não sai nem com o disco cheio;
- **despejável:** já enviado, com a lista dos maiores sem uso há N dias;
- **dataless.**

O que ainda está enviando nunca é listado. Avisa quando "Otimizar" está desligado. A ação é o Finder (Remover Download).
- Inspiração: Oakley (pinning no Sequoia, disco cheio após upgrade), Storage Sense (desidratação), recomendação "Armazenar no iCloud".
- Dado: o nome do xattr vem do mesmo `listxattr` que a travessia **já faz** para os xattrs grandes (os +40 % já estão pagos). Para o despejável, `NSURLUbiquitousItemIsUploadedKey` (só metadado), lido apenas nos candidatos grandes.
- Risco: o nome exato do xattr precisa ser confirmado com `xattr -l` num arquivo fixado (`com.apple.fileprovider.pinned`, possivelmente com sufixo `#PX`). O estado do "Otimizar" pode depender de fonte privada; sem ela, "não medido".

**B9. Conferência com a ferramenta do dono** · M · médio
Ao lado da medição, mostra o relatório só leitura do próprio ecossistema:
- Docker.raw: aparente, alocado e o que o `docker system df` diz;
- runtimes do simctl com `lastUsedAt`;
- `brew cleanup -n`;
- `ollama list`.

O comando oficial é só copiado.
- Inspiração: DevCleaner, Dev Environment Manager do Pearcleaner, FAQ do Docker Desktop.
- Dado: binários locais, rodados como usuário (nunca pelo helper), sem shell, com timeout, `HOMEBREW_NO_AUTO_UPDATE=1` e `HOMEBREW_NO_ANALYTICS=1`.
- Risco: executar binário de terceiro. `docker` e `ollama` falam com o daemon por socket local (o `ollama` usa HTTP em localhost). Nada sai do Mac, mas se o daemon estiver parado vira "não medido", sem iniciá-lo. Opção barata para garantir só leitura e sem rede: rodar sob `sandbox-exec` com perfil que nega `network-outbound` remoto e `file-write*` (deprecado, mas presente no 27; conferir no 15 e no 26). Fontes diferentes: comparar, nunca somar.

**B10. Saúde do SSD sem nota** · M · baixo
Card em Informações com os campos crus do log NVMe (% usado "segundo o fabricante", TB escritos, spare, erros, horas) e a tendência. Só avisa se o próprio disco sinalizar.
- Inspiração: Sensei, OSXDaily/smartctl, CoreGuard.
- Dado: `IONVMeSMARTInterface` (plug-in IOKit), `vm.swapusage`.
- Risco: API mal documentada e campos ausentes. **Fora do foco do produto, por isso o impacto caiu para baixo.** Nunca calcular "vida restante".

---

## C. Agir com segurança (o app recomenda, o usuário age)

**C1. Simulador "e se eu apagar?" com três números** · M · alto
Bandeja para onde se arrastam itens. Mostra três números:
1. quanto sai da barra do Ajustes (alocado, contado como o `STMSize` conta);
2. quanto o livre sobe agora (privado; hardlink e clone só entram com o grupo inteiro);
3. quanto fica preso em snapshot (alocado − privado, exato sem clone; "não separável" com clone parcial).

Ações: Finder, copiar caminhos, exportar lista. O rótulo fixo é "simulação".
- Inspiração: Collector do DaisyDisk, marcas do SpaceSniffer, AppleInsider (o du dava 23 GB, o painel caiu 24,5 e o livre subiu ~14).
- Dado: `PRIVATESIZE`, `CLONEID`, `EF_SHARES_ALL_BLOCKS`, `LINKCOUNT`, `FILEID`.
- Risco: falta validar "alocado − privado = preso" no Mac do dono (o CI não cria snapshot). Clone parcial vira "não medido", sem estimativa (conta dupla vista na fixture da Fase 0).

**C2. Recibo medido / gravar uma ação** · M · alto
Junta o recibo e a sessão antes/depois. Há dois botões:
- "Vou fazer isto agora", por item;
- "Gravar … Parar", para uma sessão livre (atualizar o macOS, instalar o Xcode, apagar no Finder).

O app mede antes e depois e emite um recibo: "esperado 12,4 GB; o livre subiu 9,1; 3,3 seguem presos no snapshot das 14:00". Avisa "está no Lixo, ainda ocupa". O que mudou por outros processos aparece separado. CLI: `sdtally record` / `--stop`.
- Inspiração: Plumby (livre antes/depois), críticas aos "X GB liberados", airlock honesto do tymrtn/diskspace, treino do Fitness.
- Dado: `ATTR_VOL_SPACEUSED`, livre do container, `PRIVATESIZE`, fsctl, `~/.Trash`, FSEvents incremental.
- Risco: a conta completa leva 30–80 s, então o "antes" tem de vir antes da ação, ou usar a última amostra dizendo de quando é. Purgeable e Finder atualizam devagar. Guardar a sessão depende da decisão 1; sem ela, a sessão vive só na memória.

**C3. Diagnóstico guiado "meu disco encheu", com modo crise** · M · alto
Roda as medições na ordem em que os relatos costumam achar a causa: snapshots, iCloud, Xcode/VM/Docker, bugs conhecidos, apagados-abertos, pastas protegidas, resto. Marca cada passo como "descartado" ou "achado".

Com o livre do container abaixo de ~5 GB, entra em modo crise:
- só fontes baratas;
- **zero escrita**, o que inclui não gravar o `hashHelper` em `UserDefaults` como `Painel.swift` faz hoje;
- tela simples.

A primeira sugestão libera espaço sem apagar dado.
- Inspiração: Oakley (storage crisis, ENOSPC), r/MacOS (11 passos sem achar 193 GB), narrativa do EtreCheck.
- Dado: `--quick`, snapshots, dir-stats, travessia por ramo, B5.
- Risco: testar num volume APFS cheio (fixture com hdiutil). A ordem dos passos não é medição, e o app deve dizer isso.

**C4. Taxa de regeneração medida** · M · médio
Com histórico, cada cache ganha "voltou a 80 % em 3 dias", e a ficha diz "apagar rende quase nada (medido neste Mac)". Caches que nunca voltam sobem na fila; caches que crescem sem teto são marcados.
- Inspiração: fórum do CCleaner (o lixo volta em segundos), Plumby, Oakley.
- Dado: série histórica (D1) e FSEvents.
- Risco: sem histórico, "ainda sem dado". Não confundir troca de versão do app com regeneração.

**C5. "Já sei, mantenha"** · P · médio
Tira o item da fila; o número continua na conta, com o selo "reconhecido em 03/10". O item volta à fila se crescer X %. O CLI lê a mesma lista.
- Inspiração: Ignore List do CleanMyMac, whitelist do Mole, exceções do AppCleaner e do Pearcleaner.
- Dado: preferência do app (Application Support).
- Risco: depende da decisão 1.

---

## D. Acompanhar no tempo (melhorias do histórico planejado)

**D1. Gravador barato com lacunas honestas** · M · alto
Duas cadências:
- **Amostra leve** a cada 10–15 min, em menos de 2 s: usado, livre, lista de snapshots, apagados-abertos e dir-stats de pastas-chave. **Sem CacheDelete**, porque o `--quick` atual inclui o purgeable e acordaria o deleted; o ImportantUsage também passa pelo CacheDelete.
- **Medição completa** diária via `NSBackgroundActivityScheduler` (ocioso, na tomada) e travessia cheia semanal.

Cada amostra guarda a fonte, a versão do macOS, o privilégio e o estado. Mac dormindo ou app fechado vira lacuna, nunca zero ou interpolação. Tetos: amostras leves por 90 dias; diárias resumidas a pastas ≥ 100 MB, guardadas por `(device, inode)`. Opção de não guardar nomes (anon.rs).
- Inspiração: iStat Menus (amostras médias, lacunas), Stats, coconutBattery, DiskSwell (exclui a própria pasta), duc (índice), TreeSize (guarda só pastas).
- Dado: tudo já existe, menos um modo leve no helper. Hoje o helper só roda `sdtally --json` (conta completa), uma medição por vez.
- Risco: é a primeira escrita declarada (decisão 1). O `NSBackgroundActivityScheduler` só funciona com o processo vivo. Portanto o gravador precisa do app como item de início (`SMAppService.mainApp`) com `MenuBarExtra`; o daemon root não deve gravar na home. Medições com e sem root não se comparam.

**D2. "O que mudou": diff com motor único (CLI + app)** · M · alto
`sdtally diff antes.json depois.json` ou `--desde 7d` mostra:
- a conta lado a lado (antes, agora, Δ, fonte);
- o ranking por Δ (cresceu, encolheu, nova, sumiu);
- "sem explicação" também como Δ, de modo que a soma dos Δ fecha.

`sdtally show conta.json` renderiza sem medir, e `--tree[=1GB]` exporta a árvore podada. Na tela, "Comparar com…" mostra o treemap com tamanho = hoje e cor = Δ, num layout ordenado (strip/pivot) para os blocos não mudarem de lugar.
- Inspiração: comparação do TreeSize, reimport do WizTree, ncdu -o/-f, gdu, pdu; treemaps temporais (Shneiderman, SizePairs); Tempo de Uso.
- Dado: JSONs salvos e o event id do FSEvents de cada medição.
- Risco: tamanho do JSON (poda obrigatória). Respeitar `schema_version`. No CLI, quem grava o arquivo é o shell, não o sdtally, então **a parte de CLI não depende da decisão 1** e pode vir antes do D1.

**D3. Destaques e tendências em degrau** · M · alto
No máximo 3 cards por regras fixas (sem IA), cada um com frase, chip de Δ, sparkline e fonte, por exemplo "Snapshots: ~9 GB até 20/09; desde 21/09, ~26 GB". Vocabulário fixo ("subiu de patamar", "voltou ao normal", "estável") e limiar de ruído (> 1 GB e > 10 %, em 3 ou mais amostras). Antes de 14 dias, "tendências ficam prontas em N dias". Histórico em barras diárias empilhadas por categoria.
- Inspiração: Destaques e Tendências do Saúde, comparação 90/365 do Fitness, leancrew (tendência em degrau), sparklines do Stripe, Tempo de Uso.
- Dado: médias das amostras de D1 (o "~" indica média de amostras medidas, com as amostras visíveis).
- Risco: documentar o algoritmo do degrau e mostrar as amostras por trás. Sem notificação de tendência por padrão.

**D4. Diário do disco: eventos alinhados aos deltas** · G · alto
Junta "o que o macOS limpou" com os marcos de atualização. Marcadores no gráfico:
- purge do deleted ("liberou 7,7 GB a pedido do Finder; livre 4,1 → 11,8");
- purge que falhou;
- snapshot do TM criado ou apagado;
- instalações (`InstallHistory.plist`);
- aviso de disco cheio;
- abertura do painel.

O texto diz "no mesmo intervalo", nunca "causou". O helper copia só as linhas estruturadas a cada amostra, porque o log retém pouco.
- Inspiração: Oakley (fases de purga), DeltaFree, DiskSwell.
- Dado: `log show` com predicados estreitos (`com.apple.cache_delete`, TimeMachine, diskspaced) e `InstallHistory.plist`.
- Risco: formato de log privado, então o parser é por versão e conferido no CI. Retenção curta (< ~21 h no 27). Custo de CPU. Depende da decisão 1.

**D5. Linha do tempo arrastável** · M · médio
Um ponto por medição embaixo da barra. Ao arrastar, a barra e a conta se transformam (`contentTransition`), com háptico `.alignment` ao encaixar. As marcas de evento vêm do D4. Shift-arrastar mostra o Δ entre as pontas. Área empilhada no Swift Charts (`chartXSelection`, macOS 14+).
- Inspiração: abas do Saúde, transição do Stripe, NSHapticFeedbackManager, chartXSelection.
- Dado: D1.
- Risco: háptico só em trackpad Force Touch.

**D6. "Desde a última vez" e Novidades da versão (mesmo componente)** · P · médio
Faixa não modal ao abrir depois de 1 dia ou mais: "Desde 28/09: Dados do Sistema +6,2 GB; 3 snapshots novos; o macOS liberou 4 GB". O mesmo componente mostra as novidades depois de um update do app, com o texto embutido (sem rede).
- Inspiração: What's New da Apple, changelog do Linear, lista Hoje do Things.
- Dado: D2 e `CFBundleShortVersionString`.
- Risco: depende de D1. "Dados do Sistema" aqui é o da tela, pelo log, só quando houver leitura nos dois momentos; sem isso, usar o usado do Data com esse rótulo.

**D8. Resumos periódicos (semanal e mês)** · P · médio
Notificação opt-in na segunda: "usado +4,1 GB; maior alta: cache do iCloud +6 GB; o macOS liberou 3,2 GB". No mês, uma página exportável com 5 números e um gráfico. Uma semana com lacunas diz "medido em 4 de 7 dias".
- Inspiração: Relatório Semanal do Tempo de Uso.
- Dado: D1, D2, D4.
- Risco: incômodo se não for opt-in. Notificação pelo app (ver E1).

(D7, a projeção de ritmo, foi para "Não fazer".)

---

## E. Vigiar e avisar

**E1. Sentinelas com anti-ruído (inclui o vigia de salto e rampa)** · M · alto
Poucas regras prontas, cada uma explicável e com a medição que a disparou:
- snapshot do TM com mais de 26 h, ou de terceiro;
- pasta com salto (+N GB/h) ou rampa (cresce todo dia por 7 dias), via FSEvents + releitura;
- maiores arquivos abertos (lsof) relidos a cada amostra, para pegar WAL e o chronod (bancos abertos não geram FSEvents);
- apagados-abertos acima de N GB, com o processo ("X segura 3,8 GB de arquivos apagados; fechar X os devolve");
- livre físico abaixo de N;
- Lixo acima de N;
- Dados do Sistema da tela mudou X.

Histerese, tempo de espera, no máximo 1 aviso por regra por dia, tudo desligável. As ações abrem a ficha, o Finder ou o Ajustes.
- Inspiração: regras do iStat Menus, Mister Plimsoll, histerese do MHN Vitals, DiskSwell (WAL do Safari de 51 GB), limites do Tempo de Uso, alertas do CleanMyMac, Sentinel do Pearcleaner.
- Dado: D1, dir-stats, lsof/`proc_pidfdinfo`, FSEvents.
- Risco: quem notifica é o **app**, rodando como item de início (`SMAppService.mainApp`) com `MenuBarExtra`. O daemon root não tem sessão gráfica e não deve postar notificações. `UNUserNotificationCenter` precisa de bundle assinado: com Apple Development deve funcionar; com ad hoc, não (testar). Nada de time-sensitive (é entitlement). Excluir a pasta do próprio app.

**E2. Barra de menus que só fala quando há novidade** · P · médio
Ícone monocromático e parado; ponto ou cor só quando uma sentinela dispara. O menu tem 4 coisas:
1. o livre escolhido (B2) com uma barra de 3 camadas (livre, purgeable, ocupado) e mini gráfico de 7 dias;
2. "desde a última vez que você olhou +3,2 GB", com as 3 linhas;
3. o alerta atual;
4. "Medir agora".

No 26+, instrui sobre o bloqueio em Ajustes > Barra de Menus.
- Inspiração: gatilhos do Bartender 5, Ice, FAQ do Stats, menu do DiskSwell, iStat Menus.
- Dado: D1. `MenuBarExtra` (macOS 13+) no mesmo processo do app.
- Risco: não há API pública conhecida para saber se o item foi bloqueado; mostrar só a instrução. Atualizar só a cada amostra.

**E3. Widgets e controle da Central de Controle** · G · médio
Widget pequeno, médio e grande, atualizados só quando sai amostra nova (`reloadTimelines`). Controle "Medir agora" via `ControlWidget` (no SDK, macOS 26.0+).
- Inspiração: Stats, eul, MacStories (Central de Controle).
- Dado: JSON pequeno compartilhado com a extensão.
- Risco: **build e assinatura.**
  - O widget é um alvo `.appex` com sandbox obrigatório, e o `swift build`/`montar.sh` não gera `.appex`. Precisa de projeto Xcode ou `xcodebuild`, por isso o esforço subiu para G.
  - Compartilhar dados pede um App Group com prefixo de Team ID (`<TEAMID>.group…`). No macOS isso costuma dispensar perfil de provisionamento, mas não foi testado com Personal Team. Se exigir perfil, o perfil gratuito expira em 7 dias.
  - O plano B (o app grava no container da extensão) dispara no 14+ o aviso de proteção "acessar dados de outros apps".
  - O controle precisa de App Intents (F5).
  - Experimento, não compromisso.

---

## F. Integrar com IA e com o sistema

**F1. Contrato de agente no CLI** · P · alto
- Códigos de saída com sentido: 0 completo, **3 medido mas incompleto**, 1 erro, 2 uso, 130 interrompido (hoje o incompleto sai com 0).
- `sdtally schema [kind]` com JSON Schema escrito à mão e embutido via `include_str!`.
- `--help --json`.
- `next_actions` montado só a partir de enums.
- `--progress=ndjson` no stderr, porque a conta leva 38–83 s e o timeout padrão do Bash dos agentes é 120 s.
- A doc passa a dizer que a primeira chamada é `--quick`.
- Inspiração: clispec.dev, CLI Agent Spec, Terry Li ("Seven patterns"), gdu, duf.
- Dado: JSON atual (`incomplete_reasons`, `stage_errors`, `settings_screen.screen.state`).
- Risco: o código 3 quebra `sudo sdtally --json > x && …` (documentar). O schema escrito à mão precisa ser validado em `tests/relatorio.rs`.

**F2. Fronteira de confiança e redação para IA** · P · alto
O JSON e o MCP separam:
- `guidance`: só enums, números e textos do catálogo;
- `evidence`: strings vindas do disco, marcadas como não confiáveis, com caracteres de controle e bidi escapados e tamanho limitado.

`--redact=off|names|full` mantém locais conhecidos e bundle IDs e troca nomes pessoais por `<pasta 3f9a>` (anon.rs). O padrão do MCP é `names`. A fixture APFS ganha uma pasta com nome hostil, e um teste garante que esse nome nunca aparece em `guidance`.
- Inspiração: Arcjet, OWASP (LLM Prompt Injection, MCP Tool Poisoning), Microsoft.
- Dado: classificação da origem de cada string do JSON atual.
- Risco: redação demais tira contexto. Sal estável exigiria gravar uma chave (decisão 1); sem ela, o sal vale por execução. A fronteira reduz o risco, não o elimina.

**F3a. `sdtally mcp` (stdio), medindo como o usuário** · M · alto
Servidor MCP stdio no próprio binário: JSON-RPC sobre serde_json, sem tokio e sem rmcp.
- Ferramentas `storage_quick`, `storage_account`, `folder_breakdown`, `explain_path` e `settings_screen`, todas com `readOnlyHint`/`idempotentHint` e `openWorldHint:false`.
- `outputSchema` vindo de F1, `notifications/progress` e cancelamento ligado ao mesmo `AtomicBool` do Ctrl-C.
- Sem root a conta sai marcada "incompleto (sem root)", o que já é melhor que o `du`/`df` dos MCPs de disco existentes.
- Inspiração: spec MCP 2025-06-18, sysops-mcp, disk-space-monitor; os MCPs de disco existentes usam `df`/`du` e um deles tem telemetria.
- Dado: tudo o que o sdtally mede; caminho validado com `realpath` e restrito ao Data.
- Risco: a spec muda rápido, então fixar uma versão e testar com o MCP Inspector. Depende de F1 e F2. O host precisa de FDA para a conta ficar boa.

**F3b. MCP medindo pelo helper root** · G · alto
A mesma F3a, mas com a medição feita pelo helper via XPC: a IA recebe dados de root sem sudo e o host não precisa de FDA.
- **Correção:** hoje o helper só aceita conexões do app (`setConnectionCodeSigningRequirement(reqApp)`). O `reqCLI` serve para conferir a cópia do binário que o helper executa, não para aceitar o CLI como cliente. É preciso:
  - aceitar `reqApp or reqCLI`;
  - criar um modo leve;
  - limitar a frequência no helper (cada conta custa 40–80 s de I/O);
  - garantir que o helper devolva ao cliente só o que o usuário da conexão poderia ver, ou agregados (nomes de pastas de outros usuários ou do root são informação nova para um cliente sem privilégio).
- Risco: o próprio helper registra o limite "confiança = assinado pelo time". Com o certificado Apple Development no chaveiro do usuário, abrir o helper a mais um cliente aumenta a superfície. Fazer só depois do endurecimento previsto para a distribuição.

**F4. Plugin do Claude Code, Skill, AGENTS.md, llms.txt e .mcpb** · P · médio
- `integrations/claude-code/` com plugin.json, `.mcp.json`, `bin/sdtally` e a skill `diagnostico-disco-mac`: `--quick` primeiro, somável × informativo, fórmula da tela, log com mais de 21 h, `reclaim-check` antes de sugerir apagar.
- AGENTS.md na raiz para quem contribui.
- llms.txt e llms-full.txt gerados do schema.
- Pacote `.mcpb` para o Claude Desktop, com `user_config` de privacidade e uso do helper.
- Inspiração: plugins do Claude Code, Agent Skills, agents.md, llms.txt, MCPB/Desktop Extensions.
- Dado: conhecimento de research/ e docs/.
- Risco: binário sem notarização cai na quarentena (testar num Mac limpo; no 15+ não existe mais o atalho Control-clique, só Ajustes > Privacidade > "Abrir Mesmo Assim" ou `xattr -d`). Skill é conselho; a garantia fica no código.

**F5. App Intents, Serviços do Finder e esquema de URL** · M/G · alto
- **Serviços (NSServices):** "SDTally: por que isto ocupa tanto?" no clique direito do Finder.
- **Esquema de URL:** `sdtally://why?path=`.
- **Intents:** MedirAgora (`--quick`, cabe em 30 s), MedirCompleto (`LongRunningIntent`, **só no 27** pelo SDK; nas outras versões abre o app), TamanhoDaPasta, AbrirArmazenamento (que de quebra renova o log) e MostrarNoFinder. Devolvem uma StorageReportEntity com `@Property`.
- Ganha Atalhos, ações no Spotlight e Automações sem código. Nada disso é extensão: dispensa App Groups.
- Inspiração: extensão de Finder e deep links do Pearcleaner, SmartDelete do AppCleaner, WWDC25 260, WWDC26 343/345.
- Dado: medições via helper.
- Risco:
  - **Build:** os metadados de App Intents são gerados pelo `appintentsmetadataprocessor`, que só roda no build do Xcode. O `swift build`/`montar.sh` não os gera, então é preciso `xcodebuild`, por isso M/G. Serviços e esquema de URL funcionam sem isso e podem vir antes.
  - **Serviços:** o item às vezes precisa ser ativado pelo usuário, e o app tem de estar registrado no LaunchServices.
  - **URL:** qualquer página ou app pode chamar o esquema. Ele só abre a vista "why" de um caminho validado (`realpath`, dentro do Data) e **nunca** dispara medição como root nem ação sozinho.
  - Finder Sync de verdade fica de fora (ver "Não fazer").

**F6. Prova em cada número: selo de verificação e `sdtally verify`** · P · alto
Junta o selo e a fonte executável.
- **Selo:** glifo de 4 níveis ao lado de cada número (conferido ao byte com a tela; bate com margem explicada; medido sem conferência; não medido), com "conferido no 27; neste Mac (15.7): não conferido", privilégio e hora. Status "hipótese" nunca vira número.
- **`verify`:** cada Medida ganha o comando da Apple que a reproduz (`diskutil apfs list -plist`, `df -k`, `tmutil listlocalsnapshots`, `log show`, `lsof +L1`). `sdtally verify conta.json` mostra os dois lado a lado, com tolerância por fonte.
- Inspiração: monocromia com cor só para estado (Stripe, Vercel Geist), seções "Sobre" do Saúde, roxo de restrito do DaisyDisk, osquery.
- Dado: o mapa-de-fontes.md vira campo no schema (a lib continua a única fonte de verdade).
- Risco: o selo não pode sugerir "certificado pela Apple". O disco muda entre a medição e o verify, então a tolerância tem de ser documentada.

**F7. Raycast e Alfred** · M · médio
- **Raycast:** "Resumo do disco", "O que mais pesa" (List com Finder, Why e Detalhar), item de barra e ferramentas de IA só leitura, com as instructions da skill e evals como teste de regressão.
- **Alfred:** palavra-chave `sd` com Script Filter `file:skipcheck`.
- Inspiração: Raycast AI Extensions, Script Filter JSON do Alfred, workflow disk-usage.
- Dado: CLI JSON.
- Risco: a loja da Raycast não aceita binário embutido (o usuário instala o sdtally antes, pelo tap da Fase 5). A IA da Raycast roda na nuvem: os dados saem do Mac por escolha do usuário, então vale F2 com `--redact=names` como padrão. Alfred exige Powerpack.

**F8. Indexar as maiores pastas no Spotlight** · M · baixo
Opt-in. Depois de cada medição, `IndexedEntity` (macOS 15+) com o resumo e os N maiores itens, sempre com `medido_em`, apagando as entradas antigas. Spotlight e Siri passam a responder "o que mais ocupa meu Mac?".
- Inspiração: WWDC25 260, WWDC26 240/246.
- Dado: `top` e `settings_screen`.
- Risco:
  - grava no índice do sistema (decisão 1);
  - número velho engana;
  - **o que está indexado fica disponível ao Apple Intelligence**, que pode usar o Private Cloud Compute; isso tensiona o "nada sai do Mac";
  - impacto rebaixado para baixo. Se entrar, só com aviso explícito e indexando caminhos anonimizados.

**F9. Comparar com outros Macs por arquivo importado** · M · médio
Importa relatórios do sdc-coleta (AirDrop, e-mail) e mostra "seu Mac × os outros" por balde, com mediana, faixa, n e versões.
- Inspiração: reimport do WizTree e do TreeSize, duc, sdc-coleta.
- Dado: JSON do sdc-coleta (já anonimizado).
- Risco: amostra pequena e enviesada; nunca "média da população". Comparar só baldes com a mesma regra por versão (ex.: Desenvolvedor mudou no 27). Não embutir relatórios de amigos sem o consentimento explícito deles.

**F10. Compartilhar para pedir ajuda** · P · médio
Junta mapa exportável, relatório para fórum e "copiar como imagem".
- `sdtally map --svg|--html --anon` gera um arquivo único sem JS de terceiros.
- O botão "Copiar para pedir ajuda" gera Markdown com a conta, os padrões de B5, os snapshots, o modo e o que já foi descartado, e vai só para a área de transferência. Anonimiza por lista de permitidos, com prévia antes de copiar.
- Inspiração: export do WizTree, `duc graph`, botão Compartilhar do EtreCheck.
- Dado: JSON e anon.rs.
- Risco: vazamento por caminho, por isso `--anon` é o padrão quando o objetivo é compartilhar. **Universal Clipboard** levaria o texto a outros aparelhos da conta: usar `NSPasteboard.prepareForNewContents(with: .currentHostOnly)`.

---

## G. Encantar

**G1. Boas-vindas que termina no número do próprio Mac** · M · alto
Três passos enquanto o `--quick` roda:
1. a barra do Ajustes do usuário se desenha;
2. o usuário escolhe o nível de medição, vendo a diferença em bytes neste Mac;
3. o maior termo é explicado.

Sem log recente, o botão abre o Armazenamento e o card se completa sozinho quando a linha `Sum of categories` aparece. As dicas viram TipKit, cada uma uma vez só.
- Inspiração: onboarding do Arc (pequenas vitórias, carteirinha), Checklist do Saúde, TipKit.
- Dado: leitura rápida existente, repetida durante o passo.
- Risco: a URL do painel varia por versão. O TipKit grava um datastore local (decisão 1); configurar sem CloudKit.

**G2. Cobertura da explicação como única pontuação, e conta fechada** · P · médio
Anel "Volume Data explicado: 99,7 % (1,16 GB sem explicação)", com o que aumentaria a cobertura ("sem o helper ficariam 8,61 GB"). Com resto ≤ 1,5 %, o anel fecha com `checkmark.seal` e `.bounce`, som opcional. Estados vazios que comemoram ("nenhum snapshot prendendo espaço").
- **Correção:** `data_volume.unexplained` é uma fração do **usado do volume Data** (1,16 de 360,77 GB = 0,32 %), não de "Dados do Sistema". O rótulo "Dados do Sistema explicado" só pode existir depois da Fase 2.
- Inspiração: anel de progresso do Things, confete merecido do Raycast, NN/g sobre estados vazios; inverte o placar do CleanMyMac.
- Dado: `data_volume.unexplained` e `caps.rs`.
- Risco: 100 % não é meta (PLANO). Nunca celebrar apagar.

**G3. Estados vazios e "não medido" que ensinam** · P · alto
Catálogo único de estados, cada um com explicação, número e ação:
- sem log de ~21 h: abre o Armazenamento;
- INCOMPLETO sem root: diferença medida entre rodadas + botão do helper;
- "não disponível nesta versão";
- pasta protegida: tamanho por dir-stats.
- Inspiração: NN/g, tela de FDA do DaisyDisk, voz do Raycast, `ContentUnavailableView` (macOS 14+).
- Dado: os três estados por linha e `meta.capacidades`.
- Risco: tabela de URLs do Ajustes por versão. Nunca 0 no lugar de "não medido".

**G4. Cor que significa algo** · P · médio
- Cerca de 7 matizes base; os filhos herdam o matiz do pai com passos de luminosidade; a cor vem de um id estável.
- "Sem explicação" é cinza hachurado; informativos só em contorno; laranja e vermelho reservados a "não medido" e INCOMPLETO.
- Com "Diferenciar sem cor", glifo + ImagePaint. Contraste ≥ 4,5:1.
- Inspiração: círculo cromático do DaisyDisk, Stripe/Vercel, HIG de gráficos, WWDC22 110340.
- Dado: ids do JSON.
- Risco: matizes demais.

**G5. Paleta ⌘K, Type Travel e barra de ações** · M · médio
Busca aproximada sobre os caminhos medidos e sobre ações: why, Finder, copiar caminho, copiar o comando `sdtally` equivalente, medir como root. Digitar no ranking já filtra. Barra fixa com os atalhos do item.
- Inspiração: ⌘K do Raycast, Type Travel do Things 3, Linear.
- Dado: árvore em memória.
- Risco: só alcança o que está carregado (limite de ≤ 100 MiB por 1 mi de entradas). Copiar só comandos de leitura.

**G6. Painel flutuante ao terminar e Quick Look no ranking** · M · médio
`NSPanel` que não rouba o foco, com a manchete e os 3 maiores termos, reabrível pelo menu. Espaço abre o Quick Look (`QLPreviewPanel`, sem extensão); arrastar uma linha solta o caminho.
- Inspiração: overlay do CleanShot X, Quick Look do DaisyDisk.
- Dado: fim do processo e JSON final.
- Risco: "Copiar como imagem" anonimizado por padrão e com `.currentHostOnly` (ver F10). O Quick Look de um arquivo dataless pode materializá-lo: desativar nesses itens.

**G7. Identidade "tally"** · M · médio
Quatro traços e uma diagonal, usados em:
- ícone;
- progresso por etapa (travessia, snapshots, CacheDelete, apagados-abertos, dir-stats), com a diagonal riscada só quando a conta fecha;
- template na barra de menus;
- `NSDockTile`.

SF Mono só em caminhos; `.monospacedDigit()` em todo número.
- Inspiração: keycap do Raycast, ícone-progresso do DaisyDisk 4, Linear/Vercel.
- Dado: eventos de etapa de F1.
- Risco: ícone para 15 e 26+ (`.icns` + `.icon` via actool no `montar.sh`), frágil entre versões do Xcode.

**G8. Retrospectiva ("Wrapped") e impressão digital decorativa** · G / P · médio / baixo
- **Retrospectiva:** cartões mensais e anuais a partir do histórico ("o maior salto: +19 GB em 12/08 em workspaceStorage"; "o macOS liberou sozinho 230 GB em N medições"), cada um com "Copiar como imagem" anonimizado.
- **Impressão digital:** `MeshGradient` (macOS 15+) determinístico a partir das proporções medidas, como "carteirinha" no fim das boas-vindas, com o rótulo "decorativa".
- Inspiração: Raycast Wrapped, Spotify Wrapped, carteirinha do Arc, MeshGradient.
- Dado: D1 e D4.
- Risco: dizer "em N medições", nunca "no ano todo". A carteirinha não leva número de série.

**G9. Carta anti-scareware no CI** · P · alto
Teste que procura termos proibidos e regras de UI: sem nota de saúde, sem contagem de "problemas", sem "lixo"/"junk", nada pré-selecionado, vermelho só para INCOMPLETO e "não medido".
- **Correção:** ainda não há String Catalog (PLANO, Fase 4, "fica para depois"). O teste começa com grep em `app/Sources/**/*.swift` e `src/report/` (i18n do Rust) e passa a cobrir o `.xcstrings` quando ele existir. Junto, o grep da Fase 3 ("zero `rm`, `osascript`"), adaptado: o `osascript` ainda aparece no protótipo para pedir a senha.
- Inspiração: processo contra o MacKeeper (acordo de US$ 2 mi), Health Check do CCleaner, Mac Health do CleanMyMac.
- Dado: código-fonte.
- Risco: falso positivo em textos que explicam o termo ("o que outros apps chamam de lixo"); ter uma lista de exceções.

---

## Top 12 para fazer a seguir

Ordem: impacto ÷ esforço, respeitando dependências e o encaixe no que já está planejado.

| # | Ideia | Esf. | Por que agora |
|---|---|---|---|
| 1 | **F1 Contrato de agente no CLI** | P | Base do que vem depois: progresso para o app (G7, A7), MCP (F3a) e código 3 para incompleto. Mudança pequena no `sdtally.rs`. |
| 2 | **F2 Fronteira de confiança e redação** | P | Tem de existir antes de qualquer IA ler o JSON. Reaproveita o anon.rs. |
| 3 | **G9 Carta anti-scareware no CI** | P | Barata e protege a identidade antes de as telas de recomendação existirem. |
| 4 | **G3 Estados vazios que ensinam** | P | O app já tem os três estados por linha; falta voz e ação. Reduz o atrito com permissões. |
| 5 | **F6 Selo de verificação + `verify`** | P | Torna visível o diferencial (medição com fonte). O mapa-de-fontes já tem os dados; o schema vem de F1. |
| 6 | **B8 iCloud fixado × despejável** | P | Ganho alto: o `listxattr` já roda na travessia, falta só confirmar o nome do xattr. |
| 7 | **B4 + B2 O macOS cuida sozinho / três livres** | P→M | Responde à dúvida nº 1 ("por que não bate com o Finder") com fontes já medidas. |
| 8 | **B3 Raio-X dos snapshots** | M | Maior causa medida de "apaguei e não voltou". fsctl existente + `listSnapshots -plist` (chaves conferidas). |
| 9 | **B1 Ficha única** | M | É o "o que fazer" planejado, com uma anatomia que impede scareware. Um catálogo só para CLI, app e IA. |
| 10 | **A1 Barra que se desdobra** | M | Interação central da direção vencedora. Primeira versão desdobra o volume Data; o segmento Dados do Sistema espera a Fase 2. |
| 11 | **D2 (CLI) e depois D1** | M | O diff no CLI não precisa da decisão 1 (o shell grava). O gravador exige a decisão 1 e o app como item de início. Juntos destravam D3–D8, C2, C4 e E1. |
| 12 | **F3a `sdtally mcp` como usuário** | M | É o "acesso para IAs" planejado, sem esperar o endurecimento do helper. A F3b (root pelo helper, G) vem depois. |

Próximos da fila: B5 (assinaturas de bugs), C1 (simulador de três números), G1 (boas-vindas), E1 + E2 (sentinelas no app da barra de menus), C2 (recibo), F5 parcial (Serviços + esquema de URL, que não precisam de `xcodebuild`), F3b.

---

## Apostas grandes

1. **Autópsia por snapshot e história na primeira abertura** (G).
   - O snapdiff do kernel (`fsctl 0xC0304A10`, sem root e sem montar) mais o FSEvents responde "o que está nos 21 GB que o snapshot das 08:16 segura": criados, modificados e apagados entre snapshots, agrupados por pasta e por app.
   - Na primeira abertura, o journal do FSEvents (7–30 dias) e, de forma opcional e só com o helper, um snapshot montado só leitura por segundos dão "o que mudou nas últimas 24 h" sem esperar 14 dias de histórico.
   - Nenhum visualizador de Mac abre o conteúdo de um snapshot APFS.
   - Risco: fsctl privado validado só no 27. Montar snapshot fica fora do modo padrão, nunca durante backup, com desmontagem garantida.
2. **Quem está enchendo o disco agora** (M–G).
   - `proc_pid_rusage` (`ri_diskio_byteswritten`, `ri_logical_writes`, conferidos em `sys/resource.h`) de todos os PIDs pelo helper, cruzado com FSEvents e releitura das pastas tocadas.
   - Vira a vista "Atividade" no app e `sdtally watch` (TUI/NDJSON): "últimos 10 min: −2,1 GB livres; +1,8 GB em com.apple.bird; mediaanalysisd escreveu 640 MB". Pega os casos do chronod e do mds_stores.
   - Escrita nunca entra na conta (rótulo "escrita", informativo).
   - Risco: os contadores zeram quando o processo reinicia; bateria; atribuição de escrita bufferizada; o helper precisa de um modo novo além do `sdtally --json`.
3. **Oráculo `reclaim-check` para agentes, com hook opcional antes do rm** (M).
   - Um veredito por caminho, num enum (`frees_now`, `frees_after_snapshots_expire`, `frees_nothing_placeholder`, `shared_clone`, `held_open`, `owned_by_app_use_app`, `protected_or_graft`), com bytes e fonte.
   - **Correção:** `tmutil sizelocalsnapshotsof` não existe (`man tmutil` conferido). O "preso em snapshot" vem de `ALLOCSIZE − PRIVATESIZE` e do fsctl de privado que o sdtally já usa.
   - Um hook PreToolUse no plugin injeta o veredito quando o Claude Code vai apagar algo grande. O SDTally continua sem apagar, mas passa a corrigir as IAs que apagam.
   - Risco: `owners.rs` e a checagem de sync ainda não existem (até lá, `not_measured`). O hook mexe na configuração do usuário, então é só opt-in.
4. **"Pergunte ao SDTally" com o modelo local da Apple** (G).
   - FoundationModels (26+ no SDK, sem rede) com 5 ferramentas (resumo, ranking, pasta, porquê, catálogo) e resposta `@Generable` sem dígitos: o app insere bytes e fonte pelos `itemIds`, então o modelo nunca escreve um número.
   - Mostra também que o próprio Apple Intelligence ocupa de 17 a 26 GB.
   - Risco: só funciona no 26+ com o AI ligado (o app mira 15+, então `if #available`); qualidade em pt-BR; janela de 4096 tokens; avaliar com o Evaluations framework.
5. **Diário do disco** (D4, G). Uma linha do tempo única com os purges do macOS (livre antes e depois, inclusive os que falharam), o afinamento do TM e as instalações. Explica "disco cheio com 60 GB disponíveis" melhor que qualquer ferramenta. Risco: formato de log privado e retenção curta.
6. **MCP App: a tela "a sobra, desdobrada" dentro do Claude e do ChatGPT** (M).
   - Recurso `ui://sdtally/conta` em HTML inline (a CSP já impede rede), com a barra desdobrada e o ranking clicável dentro da conversa.
   - A IA explica e o usuário vê os números medidos com a fonte ao lado.
   - Risco: dois visuais para manter; o suporte varia por host; a extensão ainda pode mudar; depende de F3a.

---

## Não fazer

| Ideia | Por quê |
|---|---|
| Botão de apagar, desinstalar ou "limpar tudo"; executar `tmutil`, `brew`, `simctl` ou `docker prune` | Viola o só leitura. Comando oficial só é copiado (e só se tiver modo simulação); quando não houver, só abrir o Finder. Ver a decisão 2 sobre a Fase 3 e o README. |
| Nota de "saúde" do Mac, contagem de "problemas", vermelho para o que o sistema gerencia, palavra "lixo"/"junk" | É a mecânica do scareware (MacKeeper, Health Check do CCleaner, Mac Health do CleanMyMac). Coberto por G9. |
| Duplicados por hash como varredura | Lê o conteúdo, materializa dataless e é lento. No máximo "candidato por tamanho + nome", sem afirmar. Conflita com o requisito 4 do README e com o runbook `achar-duplicatas.md`: o dono decide. Se voltar, só sob demanda, para um grupo escolhido, só arquivos locais (nunca dataless). |
| "Último acesso" por atime | O APFS atualiza o atime de forma preguiçosa e a travessia o desliga para si: seria um dado falso. Usar `kMDItemLastUsedDate` com a fonte "Spotlight". |
| D7. Projeção de ritmo ("chega a 20 GB livres em ~N dias") | Não é medição nem aritmética exata sobre o presente; viola "dados sempre medidos". Volta só se o dono aceitar explicitamente, como bloco "projeção" fora da conta, com a conta exposta, 7+ dias de amostras e desligado por padrão. |
| Atribuir escrita por `fs_usage` contínuo | Pesado e exige root permanente. Usar `proc_pid_rusage` (aposta 2). |
| Montar snapshot no modo padrão | Cria ponto de montagem e segura o snapshot. Só opt-in, pelo helper, por segundos. |
| Finder Sync Extension de verdade | É `.appex` com sandbox, App Group e build pelo Xcode. Com o certificado gratuito e o `montar.sh`, usar Serviços (F5). |
| Notificação time-sensitive ou push; Sparkle; base de fichas atualizada pela rede; comparação com outros Macs via servidor; TipKit com CloudKit | Exigem rede ou entitlement. Tudo chega com o .app novo ou por arquivo importado. |
| IA na nuvem embutida (Use Model com PCC/ChatGPT como padrão) | Os dados sairiam do Mac. No máximo o modelo on-device; o MCP deixa a escolha do cliente para o usuário, com F2. |
| Recomendar desligar SIP, `chflags schg`, `ln -s /dev/null`, mudar o hibernatemode ou o limiar do diskspaced | Altera o sistema e é perigoso. A ficha diz "não faça". |
| "Vida restante" do SSD, ou qualquer projeção dentro da conta | São estimativas. Mostrar os campos crus. |
| Gamificar limpeza (confete ao apagar, placar de GB liberados) | O app não apaga; a única pontuação é a cobertura da explicação (G2). |
| Atualizar a barra de menus ou o widget ao vivo | Custo de CPU e bateria; com widgets, piora o bug do chronod. Só a cada amostra. |
| Amostra frequente que chama CacheDelete ou ImportantUsage | Acorda o deleted a cada leitura (o iStat Menus já foi flagrado em loop). Purgeable só na amostra diária (D1). |
| Contar com a ponte MCP → App Intents vista no beta do 26.1 | Não aparece como API pública no guia do WWDC26. |
| Distribuir o .app ou o helper para outros Macs como está | Assinatura Apple Development sem notarização: o Gatekeeper bloqueia (no 15+, só "Abrir Mesmo Assim" em Ajustes ou `xattr -d`), e o helper confia em "assinado pelo time", o que o próprio código marca como insuficiente para distribuição. Fica para a Fase 5. |

---

## Fontes

### Visualizadores
- https://grandperspectiv.sourceforge.net/index.html
- https://grandperspectiv.sourceforge.net/HelpDocumentation/MasksAndFilters.html
- https://grandperspectiv.sourceforge.net/HelpDocumentation/ViewDisplayPanel.html
- https://sourceforge.net/projects/grandperspectiv/files/grandperspective/3.6/
- https://sourceforge.net/p/grandperspectiv/news/
- https://github.com/shundhammer/qdirstat
- https://github.com/shundhammer/qdirstat/blob/master/doc/File-Age-Stats.md
- https://github.com/shundhammer/qdirstat/blob/master/doc/stats/File-Size-Histogram.md
- https://github.com/shundhammer/qdirstat/blob/master/doc/Unpkg-View.md
- https://github.com/shundhammer/qdirstat/issues/172
- https://github.com/windirstat/windirstat/blob/master/CHANGELOG.md
- https://github.com/windirstat/windirstat/blob/master/README.md
- https://blog.windirstat.net/20241016/windirstat-2-0-1-released/
- https://diskanalyzer.com/about
- https://diskanalyzer.com/guide
- https://diskanalyzer.com/whats-new
- https://diskanalyzer.com/wiztreemac-vs-wiztree
- https://wiztree.world/guide/
- https://wiztree.world/download/
- https://www.uderzo.it/main_products/space_sniffer/index.html
- https://www.uderzo.it/main_products/space_sniffer/features.html
- https://www.uderzo.it/main_products/space_sniffer/tips_and_tricks.html
- https://apps.kde.org/filelight/
- https://docs.kde.org/stable_kf6/en/filelight/filelight/filelight.pdf
- https://github.com/kde/filelight
- https://help.gnome.org/baobab/
- https://apps.gnome.org/en/Baobab/
- https://github.com/GNOME/baobab/blob/main/NEWS
- https://www.derlien.com/
- http://www.musingsfrommars.org/2006/09/disk-inventory-x-visually-map-your-files.html
- https://tech-me-out.blogspot.com/2024/05/how-to-manage-macos-disk-space-using.html
- https://www.itechguides.com/products/omnidisksweeper/
- https://dissectmac.com/blog/daisydisk-alternatives
- https://manuals.jam-software.de/treesize/EN/disk_usage_comparison.html
- https://manuals.jam-software.com/treesize/EN/history.html
- https://manuals.jam-software.com/treesize/EN/file_menu.html
- https://duc.zevv.nl/
- https://github.com/zevv/duc/blob/master/doc/duc.md
- https://github.com/137137137/OpenDisk
- https://opendisk.app/
- https://github.com/jgalea/disk-bloom
- https://vocallabs.ai/pocodisk
- https://eclecticlight.co/2021/04/07/precize-now-informs-you-of-sparse-files-and-clones/
- https://eclecticlight.co/2024/01/03/what-should-we-know-about-apfs-special-files/
- https://news.ycombinator.com/item?id=46401052
- https://news.ycombinator.com/item?id=40449767
- https://news.ycombinator.com/item?id=13574825
- https://news.ycombinator.com/item?id=36868940
- https://news.ycombinator.com/item?id=40935283
- https://dl.acm.org/doi/10.1145/571647.571649
- https://onlinelibrary.wiley.com/doi/10.1111/cgf.13989
- https://graphics.uni-konstanz.de/publikationen/Han2022SizePairs/index.html
- https://pubmed.ncbi.nlm.nih.gov/17968076/

### Limpadores e assistentes
- https://macpaw.com/support/cleanmymac-x/knowledgebase/safety-database
- https://macpaw.com/support/cleanmymac/knowledgebase/smart-care
- https://macpaw.com/support/cleanmymac-x/knowledgebase/cleanmymac-safety
- https://macpaw.com/support/cleanmymac-x/knowledgebase/system-junk
- https://macpaw.com/cleanmymac-x/cleanup-assistant
- https://macpaw.com/support/cleanmymac/knowledgebase/mac-health
- https://macpaw.com/support/cleanmymac/knowledgebase/missing-features
- https://www.prnewswire.co.uk/news-releases/macpaw-launches-new-cleanmymac-the-smart-effortless-app-for-mac-care-302275057.html
- https://cleanmymac.com/blog/cleanmymac-vs-cleanmymac-x
- https://pearcleaner.com/
- https://github.com/alienator88/pearcleaner
- https://github.com/alienator88/Pearcleaner/releases
- https://freemacsoft.net/appcleaner/
- https://yama-mac.com/en/appcleaner/
- https://titanium-software.fr/en/onyx
- https://en.wikipedia.org/wiki/OnyX
- https://cindori.com/sensei
- https://www.macstories.net/stories/sensei-a-beautifully-designed-dashboard-and-set-of-utilities-for-your-mac/
- https://www.macworld.com/article/233898/sensei-review-gorgeous-mac-performance-utility-hamstrung-by-thunderbolt.html
- https://www.drbuho.com/wiki/everything-about-buhocleaner
- https://mole.fit/docs
- https://github.com/tw93/mole
- https://support.ccleaner.com/s/article/what-is-health-check
- https://community.ccleaner.com/t/i-really-dont-like-health-check/71716
- https://community.ccleaner.com/t/any-way-to-delete-all-trackers-without-heath-check/74106
- https://support.apple.com/guide/mac-help/optimize-storage-space-sysp4ee93ca4/mac
- https://support.apple.com/en-us/102624
- https://support.apple.com/en-mk/guide/mac-help/mchl3d437fbc/mac
- https://discussions.apple.com/thread/254005330
- https://ios.gadgethacks.com/how-to/free-up-storage-space-iphone/
- https://au.pcmag.com/mobile-phones/42835/how-to-free-up-space-on-your-iphone-or-ipad
- https://support.microsoft.com/en-us/windows/experience/storage-filemanagement/storage-settings-in-windows
- https://support.microsoft.com/en-us/windows/experience/storage-filemanagement/manage-drive-space-with-storage-sense
- https://support.microsoft.com/en-us/windows/experience/storage-filemanagement/free-up-drive-space-in-windows
- https://learn.microsoft.com/en-us/windows/configuration/storage/storage-sense
- https://www.makeuseof.com/hidden-windows-storage-page-free-up-disk-space/
- https://cleanor.app/blog/how-to-clean-android-storage-the-safe-way-checklist
- https://www.appcrazy.com/app/files-by-google.html
- https://eathealthy365.com/an-overview-of-the-files-by-google-app/
- https://plumby.app/guides/what-mac-cleaners-actually-free
- https://plumby.app/guides/what-is-system-data-on-mac
- https://storageradar.app/blog/why-one-click-mac-cleaners-are-risky/
- https://www.theodorehq.com/shiny/blog/is-cleanmymac-safe
- https://corelock.net/blog/is-cleanmymac-safe
- https://jenisystems.com/avoid-high-risk-pc-mac-cleaners/
- https://forums.macrumors.com/threads/is-cleanmymac-actually-legit-or-snake-oil.2476091/
- https://discourse.devontechnologies.com/t/anyone-have-problems-with-cleanmymac-new-version/82317
- https://discussions.apple.com/thread/255526751
- https://www.reddit.com/r/MacOS/comments/oousnr/never_had_this_problem_before_cleanmymac_x_is_now/
- https://truthinadvertising.org/wp-content/uploads/2014/06/Yencha-v.-ZeoBIT-complaint-.pdf
- https://yenchasoftwaresettlement.com/mainpage/CommonlyAskedQuestions.aspx
- https://eclecticlight.co/2023/09/22/will-flushing-caches-free-up-disk-space/
- https://eclecticlight.co/2023/04/15/what-is-purgeable-disk-space/
- https://eclecticlight.co/2023/04/19/ventura-space-management-what-gets-purged-and-how/
- https://eclecticlight.co/2023/04/27/where-does-macos-get-its-volume-free-space-figures-from/
- https://eclecticlight.co/2026/09/02/caches-and-purging-in-macos-tahoe/
- https://eclecticlight.co/2025/02/06/start-planning-your-macs-spring-clean/
- https://eclecticlight.co/2025/02/11/what-is-system-data-in-storage-settings/
- https://eclecticlight.co/2025/12/20/explainer-storage-settings/
- https://mjtsai.com/blog/2025/08/19/tahoe-free-space-problems/
- https://tidbits.com/2024/03/18/follow-these-steps-to-clear-space-on-your-mac/
- https://osxdaily.com/2024/04/10/how-to-check-disk-health-on-mac-with-smartctl/
- https://github.com/smartmontools/smartmontools/issues/176
- https://coreguard.app/insights/mac-ssd-endurance-disk-writes/

### Monitores
- https://bjango.com/help/istatmenus7/historygraphs/
- https://bjango.com/help/istatmenus7/rules/
- https://bjango.com/mac/istatmenus/versionhistory/
- https://bjango.com/mac/istatmenus/
- https://www.macrumors.com/2024/07/31/istat-menus-7-0-brings-new-features/
- https://github.com/exelban/stats
- https://github.com/exelban/stats/releases
- https://github.com/exelban/stats/issues/3249
- https://github.com/exelban/stats/issues/2733
- https://github.com/exelban/stats/issues/2717
- https://github.com/exelban/stats/issues/2881
- https://github.com/exelban/stats/issues/2901
- https://github.com/exelban/stats/issues/1626
- https://mac-stats.com/faq
- https://github.com/gao-sun/eul
- https://support.apple.com/en-gb/105003
- https://support.apple.com/guide/iphone/view-your-health-data-iphe3d379c32/ios
- https://leancrew.com/all-this/2024/11/apple-health-trends/
- https://www.apple.com/om/newsroom/2021/06/apple-advances-personal-health-by-introducing-secure-sharing-and-new-insights/
- https://support.apple.com/en-au/guide/mac-help/mchlf2c0c770/mac
- https://support.apple.com/en-am/guide/iphone/iphbfa595995/ios
- https://apple.stackexchange.com/questions/452146/ios-16-can-i-choose-when-the-screen-time-weekly-report-is-delivered
- https://www.robustperception.io/reduce-noise-from-disk-space-alerts
- https://github.com/prometheus/node_exporter/blob/master/docs/node-mixin/alerts/alerts.libsonnet
- https://groups.google.com/g/prometheus-users/c/hTcZYTNMLjg
- https://misterplimsoll.app/
- https://github.com/0k-lab/DiskSwell
- https://github.com/tymrtn/diskspace
- https://github.com/tymrtn/diskspace/releases
- https://saltygnome.com/diskpulse.html
- https://github.com/scasella/DiskPulse
- https://metahuman.network/mhn-vitals/
- https://github.com/jordanbaird/Ice
- https://www.macbartender.com/Bartender5/
- https://www.zdnet.com/article/what-are-triggers-in-bartender-5-and-how-do-they-improve-macos-sonoma/
- https://appleinsider.com/inside/macos-tahoe/tips/inside-control-center-on-macos-tahoe
- https://sixcolors.com/post/2025/09/macos-26-tahoe-review-power-under-glass/
- https://www.macstories.net/stories/mystery-solved-why-third-party-apps-vanished-from-the-macs-control-center/
- https://www.apple.com/os/pdf/All_New_Features_macOS_Tahoe_Sept_2025.pdf
- https://developer.apple.com/documentation/widgetkit/keeping-a-widget-up-to-date
- https://developer.apple.com/documentation/widgetkit/creating-a-widget-extension
- https://s00d.github.io/tauri-plugin-widgets/guide/setup/macos
- https://github.com/Vatsal057/PhotoWidget
- https://developer.apple.com/forums/thread/737732
- https://support.apple.com/guide/activity-monitor/view-disk-activity-actmntr1005/mac
- https://stackoverflow.com/questions/15786618/per-process-disk-read-write-statistics-in-mac-os-x
- https://docs.rs/libproc/latest/x86_64-unknown-linux-gnu/libproc/pid_rusage/struct.RUsageInfoV4.html
- https://www.coconut-flavour.com/coconutbattery/
- https://apple.stackexchange.com/questions/254810/what-is-the-deleted-daemon-in-macos
- https://apple.stackexchange.com/questions/254485/silencing-your-disk-is-almost-full-notification/282813
- https://forums.macrumors.com/threads/tahoe-beta-lots-of-disk-space-gone-massive-group-containers-folder.2464861/
- https://eclecticlight.co/2021/03/22/time-machine-to-apfs-backup-structure-and-access/
- https://danielcosenza.com/posts/mac-howto-apfs-snapshots/
- https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/FSEvents_ProgGuide/UsingtheFSEventsFramework/UsingtheFSEventsFramework.html
- https://github.com/fsnotify/fsevents/

### CLI e IA
- https://github.com/bootandy/dust
- https://github.com/dundee/gdu
- https://pkg.go.dev/github.com/dundee/gdu/cli
- https://github.com/Byron/dua-cli
- https://github.com/KSXGitHub/parallel-disk-usage
- https://github.com/muesli/duf
- https://github.com/aristocratos/btop
- https://ncdu.blicky.net/
- https://g.blicky.net/ncdu.git/log/src/bin_export.zig
- https://clispec.dev/
- https://cli-agent-spec.github.io/
- https://terryli.ai/posts/seven-patterns-for-agent-facing-clis/
- https://docs.hiroleague.com/ai-coding-bible/building-cli-for-agents
- https://github.com/Johnixr/agent-cli-guide/blob/main/GUIDE.md
- https://modelcontextprotocol.io/specification/2025-06-18/server/tools
- https://docs.rs/rust-mcp-schema/latest/rust_mcp_schema/struct.ToolAnnotations.html
- https://blog.modelcontextprotocol.io/posts/2026-01-26-mcp-apps/
- https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/2026-01-26/apps.mdx
- https://github.com/modelcontextprotocol/mcpb
- https://www.anthropic.com/engineering/desktop-extensions
- https://claude.com/docs/connectors/building/mcpb
- https://github.com/Enrique-S-J/sysops-mcp
- https://github.com/sandhyayadavalli/disk-space-monitor
- https://github.com/u1i/mcp-server-disk-usage
- https://github.com/surendranb/macos-mcp
- https://code.claude.com/docs/en/plugins-reference
- https://code.claude.com/docs/en/skills
- https://github.com/openai/agents.md/
- https://github.com/agentsmd/agents.md/issues/135
- https://github.com/openai/agents.md/pull/48
- https://developer.apple.com/videos/play/wwdc2025/260/
- https://developer.apple.com/videos/play/wwdc2025/244/
- https://support.apple.com/en-us/125148
- https://support.apple.com/guide/mac-help/use-apple-intelligence-in-shortcuts-mchl91750563/mac
- https://developer.apple.com/wwdc26/guides/macos/
- https://developer.apple.com/videos/play/wwdc2026/345/
- https://developer.apple.com/videos/play/wwdc2026/343/
- https://developer.apple.com/videos/play/wwdc2026/240/
- https://developer.apple.com/videos/play/wwdc2026/246/
- https://developer.apple.com/documentation/foundationmodels
- https://developer.apple.com/videos/play/wwdc2025/286/
- https://developer.apple.com/videos/play/wwdc2025/301/
- https://9to5mac.com/2025/09/22/macos-tahoe-26-1-beta-1-mcp-integration/
- https://developers.raycast.com/api-reference/tool
- https://developers.raycast.com/ai/learn-core-concepts-of-ai-extensions
- https://manual.raycast.com/ai/ai-extensions
- https://www.alfredapp.com/help/workflows/inputs/script-filter/json/
- https://github.com/alfredapp/disk-usage-workflow
- https://osquery.readthedocs.io/en/5.18.1/introduction/sql/
- https://fleetdm.com/tables/apfs_volumes
- https://arcjet.com/blog/how-we-defend-mcp-tool-outputs-from-prompt-injection
- https://cheatsheetseries.owasp.org/cheatsheets/LLM_Prompt_Injection_Prevention_Cheat_Sheet.html
- https://developer.microsoft.com/blog/protecting-against-indirect-injection-attacks-mcp/
- https://github.com/OWASP/www-community/blob/master/pages/attacks/MCP_Tool_Poisoning.md
- https://mjtsai.com/blog/2024/09/11/group-container-names-in-sequoia/
- https://github.com/XueshiQiao/CCSwitcher/issues/14

### Conhecimento (Apple, Eclectic Light, comunidade)
- https://eclecticlight.co/2025/07/24/beware-the-full-startup-disk/
- https://eclecticlight.co/2025/10/02/why-did-that-macos-upgrade-take-so-much-space/
- https://eclecticlight.co/2026/09/06/last-week-on-my-mac-storage-crisis-management/
- https://eclecticlight.co/2026/09/07/what-happens-when-a-macs-boot-volumes-run-out-of-space/
- https://eclecticlight.co/2026/09/18/how-did-golden-gate-affect-free-disk-space/
- https://eclecticlight.co/2026/09/20/last-week-on-my-mac-is-ai-worth-30-gb/
- https://eclecticlight.co/2026/09/10/prepare-to-update-or-upgrade-macos/
- https://eclecticlight.co/2023/04/16/last-week-on-my-mac-finder-or-loser/
- https://eclecticlight.co/2023/04/17/the-finder-confuses-with-wildly-inaccurate-figures-for-available-space/
- https://eclecticlight.co/2023/04/23/last-week-on-my-mac-when-the-numbers-dont-add-up/
- https://eclecticlight.co/2026/08/21/the-arithmetic-of-free-space/
- https://eclecticlight.co/2026/09/08/what-does-safe-mode-do-and-when-to-use-it/
- https://eclecticlight.co/2023/05/03/safe-mode-caches-and-cachedelete/
- https://eclecticlight.co/2026/09/01/deltafree-version-1-1-greatly-improves-its-charting/
- https://eclecticlight.co/2026/08/17/how-long-until-my-backup-storage-is-full-deltafree-can-inform-you/
- https://eclecticlight.co/2026/08/22/can-deltafree-help-size-your-backup-storage/
- https://eclecticlight.co/2025/07/30/a-deeper-dive-into-spotlight-indexes/
- https://eclecticlight.co/2025/09/29/inside-the-unified-log-3-log-storage-and-attrition/
- https://eclecticlight.co/2026/03/12/how-long-does-the-log-keep-entries/
- https://eclecticlight.co/2024/09/16/sequoia-introduces-pinning-to-icloud-drive/
- https://eclecticlight.co/2025/07/13/last-week-on-my-mac-ghosts-in-the-machine/
- https://eclecticlight.co/2023/04/05/how-cryptexes-are-changing-macos-ventura/
- https://mjtsai.com/blog/2025/03/10/purgeable-disk-space/
- https://mjtsai.com/blog/2025/07/21/spotlight-indexing-running-wild/
- https://mjtsai.com/blog/2020/07/14/apfs-snapshots-of-the-big-sur-system-volume/
- https://support.apple.com/en-us/121115
- https://support.apple.com/en-us/108809
- https://support.apple.com/guide/disk-utility/view-apfs-snapshots-dskuf82354dc/mac
- https://support.apple.com/guide/mac-help/check-icloud-drive-file-folder-status-mac-mchlc994344b/mac
- https://support.apple.com/en-au/guide/messages/icht1035/mac
- https://daisydiskapp.com/guide/4/en/HiddenSpace/
- https://daisydiskapp.com/guide/4/en/PurgeableSpace/
- https://daisydiskapp.com/guide/4/en/Snapshots/
- https://discussions.apple.com/docs/DOC-6172
- https://etrecheck.com/
- https://devcleaner.app/
- https://github.com/vashpan/xcode-dev-cleaner
- https://github.com/eloybasado/xcode-phantom-runtime-fix
- https://theapplewiki.com/wiki/List_of_Asset_Types
- https://theapplewiki.com/wiki/MobileAsset
- https://theapplewiki.com/wiki/Simulator_Runtime_MobileAssets
- https://docs.docker.com/desktop/troubleshoot-and-support/faqs/macfaqs/
- https://github.com/docker/for-mac/issues/6475
- https://github.com/abiosoft/colima/issues/817
- https://bysiber.github.io/cleardisk/blog/xcode-simulators-device-support-cleanup-mac.html
- https://deepclean.app/blog/clean-xcode-simulators-mac
- https://apple.stackexchange.com/questions/306566/can-i-delete-unnecessary-device-simulators-of-xcode
- https://discussions.apple.com/thread/256058005
- https://discussions.apple.com/thread/256037221
- https://discussions.apple.com/thread/255931263
- https://discussions.apple.com/thread/255835917
- https://discussions.apple.com/thread/255806791
- https://forums.macrumors.com/threads/com-apple-mediaanalysisd-taking-space.2446697/
- https://discussions.apple.com/thread/253811686
- https://talk.macpowerusers.com/t/really-66gb-of-biome-something-must-be-wrong/32746
- https://apple.stackexchange.com/questions/485695/what-is-library-biome
- https://forums.macrumors.com/threads/how-to-fix-the-spotlight-disk-write-issues.2477410/
- https://forums.macrumors.com/threads/spotlight-indexing-enabled-for-all-volumes-after-15-1-update.2441371/
- https://itpro-tips.com/macos-disk-full-apfs-snapshot/
- https://github.com/sauravvarma/stethoscope
- https://superuser.com/questions/1195657/which-timeframe-is-covered-by-the-disk-tab-in-activity-monitor-from-macos
- https://apple.stackexchange.com/questions/50235/what-i-can-delete-from-private-var-vm
- https://apple.stackexchange.com/questions/409231/why-does-var-vm-on-macbook-pro-tend-to-show-sleepimage-and-nothing-else
- https://www.reddit.com/r/MacOS/comments/1w1nq5s/system_data_i_have_done_everything/
- https://www.reddit.com/r/MacOS/comments/1u22urc/system_data_taking_up_over_300gb_of_storage/
- https://www.reddit.com/r/mac/comments/1w2wp8x/system_data_381_gb_what_is_it_can_i_delete_it_is/
- https://www.reddit.com/r/MacOSBeta/comments/1vvv6vh/anyone_know_why_system_data_is_insanely_huge/
- https://www.tomsguide.com/computing/if-your-mac-is-suddenly-running-out-of-space-after-macos-27-check-this-one-setting
- https://www.macworld.com/article/676493/how-to-delete-system-data-mac.html
- https://imazing.com/guides/how-to-find-your-itunes-backup-location

### Design
- https://culturedcode.com/things/features/
- https://www.macstories.net/reviews/things-3-beauty-and-delight-in-a-task-manager/
- https://www.theverge.com/2017/5/19/15664348/things-3-review-ios-mac-productivity
- https://sspai.com/post/39724
- https://www.inverse.com/input/design/the-browser-company-arc-design-interview
- https://www.howtheygrow.co/p/how-arc-grows
- https://uxdesign.cc/why-the-arc-browser-is-the-chrome-replacement-ive-been-waiting-for-1e0a05da9fc9
- https://www.raycast.com/blog/a-fresh-look-and-feel
- https://www.raycast.com/blog/a-technical-deep-dive-into-the-new-raycast
- https://www.raycast.com/changelog/1-45-0
- https://www.raycast.com/changelog/macos-v1/1-104-0
- https://www.raycast.com/changelog/macos/1-31-0
- https://blakecrosley.com/guides/design/raycast
- https://cleanshot.com/features
- https://cleanshot.com/changelog
- https://www.macworld.com/article/617854/cleanshot-x-review.html
- https://linear.app/now/behind-the-latest-design-refresh
- https://www.linear.app/now/how-we-redesigned-the-linear-ui
- https://linear.app/changelog
- https://support.apple.com/en-us/108429
- https://support.apple.com/guide/iphone/view-your-health-data-iphe3d379c32/26/ios/26
- https://www.apple.com/newsroom/2021/06/apple-advances-personal-health-by-introducing-secure-sharing-and-new-insights/
- https://screensdesign.com/showcase/apple-health
- https://support.apple.com/guide/activity-monitor/check-if-your-mac-needs-more-ram-actmntr34865/mac
- https://developer.apple.com/design/human-interface-guidelines/charts
- https://developer.apple.com/design/human-interface-guidelines/charting-data
- https://developer.apple.com/videos/play/wwdc2022/110340/
- https://developer.apple.com/documentation/charts/sectormark
- https://developer.apple.com/videos/play/wwdc2023/10037/
- https://developer.apple.com/documentation/tipkit
- https://developer.apple.com/videos/play/wwdc2024/10070/
- https://developer.apple.com/documentation/swiftui/contenttransition
- https://developer.apple.com/videos/play/wwdc2024/10151/
- https://developer.apple.com/documentation/appkit/nshapticfeedbackmanager/feedbackpattern
- https://developer.apple.com/documentation/appkit/nshapticfeedbackperformer
- https://developer.apple.com/documentation/appkit/nsdocktile
- https://developer.apple.com/icon-composer/
- https://mjtsai.com/blog/2025/08/08/separate-icons-for-macos-tahoe-vs-earlier/
- https://github.com/psulak/Tahoe-Sequoia-Hybrid-Icon
- https://developer.apple.com/forums/thread/756701
- https://sosumi.ai/documentation/xcode/accessing-app-group-containers
- https://daisydiskapp.com/
- https://daisydiskapp.com/guide/4/en/UnderstandingSunburst/
- https://daisydiskapp.com/guide/4/en/DeletingFiles/
- https://tbrizitsky.medium.com/the-making-of-daisydisk-retrospective-706714b4a5e9
- https://www.925studios.co/blog/stripe-dashboard-design-breakdown
- https://docs.stripe.com/stripe-apps/patterns/chart-layout
- https://www.mo4tech.com/discover-stripe-dashboard-product-design.html
- https://vercel.com/geist/colors
- https://vercel.com/design
- https://www.nngroup.com/articles/empty-state-interface-design/
- https://www.pencilandpaper.io/articles/empty-states
- https://hegeman.co/project/spotify-your-2020-wrapped/
- https://developer.apple.com/videos/play/wwdc2023/10271/
- https://medium.com/@krupaconference/interface-sound-design-44e934db2169
- https://features.vote/swiftui-whats-new-screen

Arquivos consultados: `PLANO.md`, `.../README.md`, `.../docs/json-schema.md`, `.../research/mapa-de-fontes.md`, `.../app/Package.swift`, `.../app/montar.sh`, `.../app/Sources/SDTallyHelper/main.swift`, `.../app/Sources/SDTally/Painel.swift`.
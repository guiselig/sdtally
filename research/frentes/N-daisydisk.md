# Frente N: DaisyDisk como referência de produto (28/09)

Pesquisa só em fontes públicas (daisydiskapp.com: guia, notas de versão, blog; fóruns; reviews).
O app não foi baixado nem rodado. Complementa a linha sobre o DaisyDisk em
`E-produto-arquitetura.md` §1.1 e o §5 de `2026-09-27-comunidade-system-data.md`.
Marcação: **(A)** fonte oficial ou medido aqui, com link; **(B)** inferência ou relato de usuário.

## 1. Resultado

- **O DaisyDisk é um mapa de arquivos com uma fatia de resto.** Varre a árvore e mostra
  "hidden space" = usado − soma varrida. Abre esse resto em três itens: purgeable (número do
  macOS), other volumes e "still hidden". Snapshots aparecem como lista com tamanho
  **estimado**, "só para referência", fora da soma (A,
  [Hidden space](https://daisydiskapp.com/guide/4/en/HiddenSpace/),
  [Snapshots](https://daisydiskapp.com/guide/4/en/Snapshots/)).
- **Ele não fala em "Dados do Sistema".** A página que trata do assunto ainda é a do High Sierra
  (categoria "System"). A resposta dela é "varra e procure em `/System`, `/private`, `~/Library`".
  Não há mapeamento para as categorias dos Ajustes (A,
  [SystemTooBig](https://daisydiskapp.com/guide/4/en/SystemTooBig/)).
- **"Still hidden" é onde o usuário empaca.** Ele junta pastas protegidas, índice do Spotlight,
  versões de documentos, metadado do FS, snapshots de terceiros e erro de disco, sem
  separar os itens. Há casos reais de 31 a 159 GB (§3).
- **Snapshots: estimado, não medido.** O guia usa "estimated sizes". Na App Store não mostra nem
  a estimativa (A).
- **Clones: só desde a 4.34 (01/07/2026), e por arquivo inteiro.** A primeira ocorrência conta e
  as demais valem 0 B. Clone parcial (blocos compartilhados em parte) continua errado. A página
  "Hard links" ainda diz que o macOS não permite identificar clones, o que contradiz a página
  "APFS" (A, [4.34](https://daisydiskapp.com/blog/daisydisk-4-34-released),
  [Hard links](https://daisydiskapp.com/guide/4/en/HardLinks/),
  [APFS](https://daisydiskapp.com/guide/4/en/APFS/)).
- **Nosso produto não é "um DaisyDisk".** Posicionamento: *"a conta do volume, linha a linha,
  com a origem de cada número, e o que dá para liberar de verdade"*. Da lista do DaisyDisk vale
  copiar: o fluxo do Collector, o "hidden space" com filhos e o texto honesto sobre por que o
  espaço não volta (§5).

## 2. O que ele faz e como

### 2.1 Visualização, varredura, velocidade

- Sunburst: pasta é pétala colorida e arquivo é pétala cinza; a largura é proporcional ao
  tamanho, tudo já ordenado. Objetos finos demais viram uma pétala semitransparente, "smaller
  objects". Pasta restrita aparece em roxo, e item apagado fora do app aparece riscado (A,
  [Understanding the disk map](https://daisydiskapp.com/guide/4/en/UnderstandingSunburst/)).
- A tela inicial lista os volumes, cada um com um medidor: verde se pelo menos metade está livre,
  vermelho quando falta espaço. O medidor soma o purgeable ao livre, como o Finder; com ⌥ ele
  passa para o lado ocupado (A,
  [Disks overview](https://daisydiskapp.com/guide/4/en/DisksOverview/)).
- Velocidade: o site diz "~15 s" contra 6 min do Finder. A 4.0 (2015) prometia "até 20× mais
  rápido" e menos memória. Em SSD, varre vários discos em paralelo; em HDD, põe em fila
  (A, [site](https://daisydiskapp.com/), [releases](https://daisydiskapp.com/releases),
  [ParallelScan](https://daisydiskapp.com/guide/4/en/ParallelScan/)). Não há número
  independente sério. Review de concorrente dá ~20 s para 500 GB (B,
  [anhphong.dev](https://anhphong.dev/blog/best-disk-space-analyzer-mac/), que vende o Storage Peek).
- Sonoma deixou a varredura lenta em `~/Library/Containers` e `Group Containers` por um recurso
  novo de privacidade. A 4.26.1 trouxe um contorno (A, [blog](https://daisydiskapp.com/blog/)).
  Os dir-stats da Frente H evitam isso (medem sem listar).
- Timeout de 1 min por arquivo vira o erro "File system not responding" (A,
  [TimeoutError](https://daisydiskapp.com/guide/4/en/TimeoutError/)).
- Extra: varre Dropbox, Google Drive, OneDrive e Box direto na nuvem, por API. iCloud Drive não,
  por causa do sandbox da Apple; só a cópia local (A,
  [CloudScan](https://daisydiskapp.com/guide/4/en/CloudScan/)).

### 2.2 Hidden, purgeable, snapshots, other volumes

| Item | Como aparece | Origem do número |
|---|---|---|
| hidden space | "resto de espaço não contabilizado": usado − varrido (A, [HiddenSpace](https://daisydiskapp.com/guide/4/en/HiddenSpace/)) | Diferença. Como inclui "other volumes", o "usado" é o do **container** (B, dedução) |
| purgeable | Filho de hidden. Descrito como "principalmente snapshots do TM, swap, sleep image" (A, [PurgeableSpace](https://daisydiskapp.com/guide/4/en/PurgeableSpace/)) | Número do macOS ("calculated by macOS itself", A, [blog 4.10](https://daisydiskapp.com/blog/important-update-daisydisk-4-10-adds-support-of-apfs-snapshots)). Sem quebra por serviço |
| snapshots | Filho de hidden, lista com "estimated sizes"; **não soma** no hidden porque "o purgeable já inclui snapshots, em regra" (A, [Snapshots](https://daisydiskapp.com/guide/4/en/Snapshots/)) | Método não documentado (B). A doc explica que blocos compartilhados fazem a soma diferir do grupo |
| other volumes | Filho de hidden, "tipicamente alguns GB", opaco (A, [ReservedSpace](https://daisydiskapp.com/guide/4/en/ReservedSpace/)) | Não diz se é por volume nem por qual API (B) |
| still hidden | O resto: outros usuários, Spotlight, versões, metadado ("2–3 GB é normal"), erro de FS, snapshots de terceiros (A, HiddenSpace) | Diferença, sem decomposição |

- Snapshots de terceiros (CCC, Synology) caem em "still hidden", não em purgeable. O item de
  snapshots serve para mostrar que eles existem; o app não consegue apagá-los e manda usar o app
  que os criou (A, Snapshots, HiddenSpace).
- A 4.9 e a 4.11 incluíram na varredura o conteúdo sem firmlink de `/System/Volumes/Data`,
  `/private/var/VM` e Recovery/Preboot montados. A 4.25 "incluiu mais áreas do sistema". A 4.33
  fala em "maior precisão da **estimativa** do hidden space" (A,
  [releases](https://daisydiskapp.com/releases)). Não explica o que mudou.
- Avisa que o purgeable demora de segundos a minutos para atualizar e que, nesse intervalo, o
  hidden "pega emprestado" o valor. Quando o macOS publica o número novo, o app atualiza sozinho,
  sem varrer de novo (A, HiddenSpace).

### 2.3 Modo administrador

- "Scan as administrator": pede a senha e varre como root. Tem "Quick Rescan as Administrator"
  (⌥ faz a varredura completa). Só em volumes APFS e HFS+ locais. **Só na versão fora da App
  Store** (A, [AdminScan](https://daisydiskapp.com/guide/4/en/AdminScan/)). O mecanismo (helper
  privilegiado ou AuthorizationExecute) não é documentado (B).
- Na hora de apagar, "tenta elevar a permissão sozinho quando possível" (A,
  [Restricted](https://daisydiskapp.com/guide/4/en/Restricted/)).
- **Limite que ele não admite:** root não abre as pastas `EPERM` de SIP e data vault. Aqui eram
  86 com root, e os modelos do Apple Intelligence somavam 19,27 GiB (Frentes H e §3c da síntese).
  Neste Mac, portanto, o DaisyDisk em modo administrador deixaria ~19 GiB em "still hidden".
  O guia diz que "alguns GB" é normal (B, inferência a partir de medição A nossa).

### 2.4 iCloud/dataless, hardlinks, clones

- **Dataless:** "suporte a arquivos dataless do APFS" desde a 4.31 (jan/2025). O post diz que ele
  passa a "tratar corretamente" arquivos que não ocupam espaço local (A,
  [blog](https://daisydiskapp.com/blog/)). Não diz se deixa de descer em **pasta** dataless nem
  se desliga a materialização (`IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES`) (B). Na nossa
  Frente F, listar pasta dataless com a política herdada disparou download (140 × `ETIMEDOUT`).
- **Hardlinks:** a primeira ocorrência conta; as demais ganham "tamanho virtual 0 B", com um ícone
  que distingue primário de secundário. Ao apagar um hardlink pelo app, ele recalcula os outros
  (A, [HardLinks](https://daisydiskapp.com/guide/4/en/HardLinks/)). Qual ocorrência vira a
  primária depende da ordem da varredura: o mesmo arquivo pode "mudar de pasta" entre varreduras (B).
- **Clones:** ver §1. A regra "primeira ocorrência, o resto 0 B" é a mesma dos hardlinks.
  - Com clone parcial (arquivo editado depois de clonado), o clone com 0 B tem blocos próprios
    que ficam sem contar. O guia antigo admite isso ("o agregado pode ser menor que a soma").
  - Não depende de macOS anterior ao 14: a 4.34.1 existiu porque a detecção quebrava a
    varredura nessas versões (A, [blog 4.34.1](https://daisydiskapp.com/blog/)).
  - Enxertos de cryptex (Frente L, 2,21 GiB em dobro aqui) não são clone por `CLONEID` e não
    aparecem em nenhuma doc (B).
- **Compressão HFS+/APFS:** conta o tamanho físico (A,
  [FinderMismatch](https://daisydiskapp.com/guide/4/en/FinderMismatch/)).

### 2.5 Fluxo de apagar

1. Arrastar pétala para o **Collector** (ou ⌘⌫). Nada é apagado nesse passo (A,
   [DeletingFiles](https://daisydiskapp.com/guide/4/en/DeletingFiles/)).
2. O Collector expande e funciona como a barra lateral: pré-visualizar (Quick Look com Espaço),
   arrastar para fora, × para tirar.
3. Recusa `/System`, `/Library`, a home do usuário e pétalas consolidadas. Usa uma lista de
   "arquivos de sistema que não devem ser apagados", atualizada na 3.0 e na 4.9, que liberou
   parte de `/private` (A, DeletingFiles, [releases](https://daisydiskapp.com/releases)).
4. Botão **Delete** e contagem regressiva de **5 s** com Cancel.
5. **Apaga direto, sem Lixeira**, "para o espaço voltar de fato". Não há desfazer; o guia manda
   desligar o Mac e procurar recuperação profissional (A,
   [Undelete](https://daisydiskapp.com/guide/4/en/Undelete/)).
6. Explica por que o espaço não volta: com o TM ligado, o arquivo apagado continua no snapshot e
   o purgeable cresce (A, DeletingFiles).
7. Snapshots e purgeable também vão para o Collector. Purge forçado leva de 2 a 10 min e deixa 7–10
   GB. Na App Store, o app só mostra um script para colar no Terminal (A,
   [PurgeableSpace](https://daisydiskapp.com/guide/4/en/PurgeableSpace/), Snapshots). O
   mecanismo do purge não é documentado (B).
8. A recomendação oficial é "só apague o que você reconhece como seu" (A,
   [WhatToDelete](https://daisydiskapp.com/guide/4/en/WhatToDelete/)).

### 2.6 FDA, preço, edições, plataformas

- FDA: aba nas Preferências que mostra o estado e deixa arrastar o ícone para a lista do Ajustes
  (4.7, 2019). O app afirma que só lê metadados. O vídeo da WWDC 2018 cita o DaisyDisk como
  exemplo (A, [FullDiskAccess](https://daisydiskapp.com/guide/4/en/FullDiskAccess/)).
- Preço: **US$ 9,99, pagamento único**, até 5 Macs pessoais, 30 dias de reembolso, atualizações
  4.x grátis (A, [site](https://daisydiskapp.com/)). Venda pela FastSpring e pela App Store.
- Trial: varre e mostra, mas não apaga, não varre como admin, não abre o hidden e não faz purge (A,
  [ReasonsToBuy](https://daisydiskapp.com/guide/4/en/ReasonsToBuy/)).
- Edições: a da App Store (sandbox) não tem varredura como admin, não abre o hidden, não faz purge,
  não apaga arquivos "presos" e não estima snapshots. A migração para a avulsa é grátis (A,
  [Editions](https://daisydiskapp.com/guide/4/en/Editions/)). No Tahoe, o sandbox "esquecia" o
  acesso ao disco (bug r.157722315 da Apple, contornado na 4.33) (A, [blog](https://daisydiskapp.com/blog/)).
- Versão atual **4.34.2 (10/07/2026)**. Exige macOS 10.13+, Apple Silicon e Intel. **Sem nota de
  versão para o macOS 27** (lançado em 14/09/2026) até 28/09 (A,
  [releases](https://daisydiskapp.com/releases)).
- Só macOS; o app é fechado (Software Ambience Corp.).

## 3. O que ele não explica em "Dados do Sistema" (casos)

| Caso | Números | O que faltou | Fonte |
|---|---|---|---|
| MacRumors 2020 | hidden 212,8 GB = purgeable 180,9 + still hidden 31,1 | O usuário desligou o TM e o espaço continuou lá; resolveu com `tmutil thinLocalSnapshots` sugerido no fórum. O app não diz que snapshot é e como tirar | (B) [MacRumors 2273011](https://forums.macrumors.com/threads/200gb-hidden-space-in-daisydisk.2273011/) |
| Apple Community 2019 | hidden 296 GB (purgeable 137 + still hidden 159); purgou 142 GB e recuperou 5 | Snapshots do CCC em "still hidden", sem nome. O suporte do DaisyDisk resolveu por e-mail; apagar ~50 GB de snapshots CCC levou o livre de 84 para 222 GB | (B) [AC 250490791](https://discussions.apple.com/thread/250490791) |
| Apple Community 2018 | 135 GB hidden, 127 "Still Hidden", "Unable to locate" | Corrupção na árvore de snapshots (erros `delta_tree` no log do kernel). Só resolveu com reinstalação | (B) [AC 8619010](https://discussions.apple.com/thread/8619010) |
| MacRumors 2023 | >140 GB hidden num M1 512 GB | Snapshots do Synology Active Backup, achados via `fsck_apfs`; resolveu desinstalando o agente | (B) [MacRumors 2379696](https://forums.macrumors.com/threads/hidden-space-stories-solved.2379696/) |
| MacRumors 2024 | DaisyDisk: 425 GB em snapshots e 381 GB hidden; Utilitário de Disco: 23 snapshots, 184 GB, picos de >300 GB | Estimativa de snapshot do DaisyDisk diferente do Utilitário de Disco. Quem explicou foi outro usuário, pelas colunas Private/Cumulative Size do Utilitário | (B) [MacRumors 2419555](https://forums.macrumors.com/threads/why-does-system-data-keep-increasing.2419555/) |
| Apple Community | iMac Fusion: hidden 718 GB, sem snapshots | Nada; o fio termina sem causa | (B) [AC 251267313](https://discussions.apple.com/thread/251267313) |
| MacRumors 2025 | System Data 142 GB em Mac de 256 GB; caiu para 51,66 GB após update | Recomendação: "DaisyDisk é o melhor, e mesmo assim você terá de entender snapshots" | (B) [MacRumors 2456056](https://forums.macrumors.com/threads/system-data-consumes-142-gb-of-space-unable-to-figure-out-why.2456056/) |

O padrão:
- "Still hidden" grande não tem nome nem causa. A orientação oficial é FDA → admin →
  `tmutil listlocalsnapshots` → First Aid → `fsck_apfs` → suporte (A, HiddenSpace). É um roteiro
  para o usuário, não uma medição.
- Ele não compara com o número do painel do macOS. O usuário recebe duas contas e nenhuma ponte (B).
- Pastas `EPERM` (Apple Intelligence, `CoreDuet/Knowledge`...) ficam em "still hidden" mesmo como
  root; ver §2.3 (B).
- Arquivos apagados e ainda abertos: nenhuma doc cita. Caem em "still hidden" (B). Aqui eram 3,47 GiB.

## 4. Erros conhecidos

| Tipo | O que se sabe | Conf. |
|---|---|---|
| Clones contados mais de uma vez | Até a 4.33 não detectava clone: cada cópia contava inteira. Desde a 4.34, clone completo conta uma vez; clone parcial fica abaixo do real | A (notas e guia); efeito do parcial B |
| Snapshot estimado | "estimated sizes", método não publicado; relato de 425 GB no app contra 184 GB no Utilitário de Disco (horários diferentes, backup rodando) | A (doc) / B (caso) |
| Hidden "emprestado" | Depois de apagar ou purgar, o hidden sobe temporariamente porque o purgeable atrasa | A (doc) |
| Purge que não libera | 142 GB "purgados", 5 GB recuperados (snapshots de terceiros fora do purgeable) | B (AC 250490791) |
| Tamanho errado em FAT32/exFAT | Corrigido na 4.33 | A ([releases](https://daisydiskapp.com/releases)) |
| Varredura quebrada antes do Sonoma | Corrigido na 4.34.1, efeito da detecção de clone | A |
| Sonoma lento em Containers | Contorno na 4.26.1 | A |
| Apagar coisa errada | Apaga sem Lixeira e sem desfazer. A proteção é a lista de pastas de sistema e os 5 s. Nenhum relato público grave achado; a mitigação oficial é "tenha Time Machine" | A (doc) / B (ausência de relato) |
| Docs contraditórias | "Hard links" diz que clone não é detectável; "APFS" e o blog 4.34 dizem que é | A |

Não achei relato confiável de contagem dupla por firmlink (`/Users` × `/System/Volumes/Data/Users`)
no DaisyDisk; a 4.9 trata o "conteúdo sem firmlink" do Data à parte (A).

## 5. Concorrentes e alternativas

| Ferramenta | Diferencial | Falha para "Dados do Sistema" | Fonte |
|---|---|---|---|
| **Painel do macOS** (Ajustes > Armazenamento) | Categorias e recomendações, o número que o usuário vê | System Data é resto sem lista; atrasa minutos; soma Apple Intelligence fora do lugar (Frentes A e M) | A (nossas frentes) |
| **Utilitário de Disco** | Snapshots com **Private Size** e **Cumulative Size**; uso por volume | Não desce em pastas; ninguém sabe que as colunas existem | B ([MacRumors 2419555](https://forums.macrumors.com/threads/why-does-system-data-keep-increasing.2419555/)) |
| **GrandPerspective** 3.8.1 (13/09/2026) | Treemap aberto, grátis, ativo; checa FDA desde a 3.6.1 | Sem hidden space, sem snapshots | A ([site](https://grandperspectiv.sourceforge.net/)) |
| **OmniDiskSweeper** | Lista por tamanho, estilo Finder, grátis | Sem FDA via 102 de 225 GB; move para a Lixeira; sem hidden | A ([Omni](https://www.omnigroup.com/more)); caso em [AC 255526095](https://discussions.apple.com/thread/255526095) |
| **Disk Inventory X** | Treemap e tipos de arquivo, GPL | Parado há anos | A ([derlien.com](https://www.derlien.com/)); data B |
| **CleanMyMac** (Space Lens) | Mapa de bolhas, limpeza automática; assinatura US$ 39,95/ano ou US$ 119,95 único | Limpador automático; relatos de liberar menos do que diz | A ([preços](https://macpaw.com/store/cleanmymac), [Space Lens](https://macpaw.com/support/cleanmymac/knowledgebase/space-lens)); crítica B (comunidade §4) |
| **Mole** (`mo analyze`, GPL-3.0, 68,7k ★) | TUI, `--dry-run`, JSON com `scan_status` complete/partial/unavailable; app pago à parte | Move para a Lixeira; não mostra snapshot nem resto do volume ([#222](https://github.com/tw93/Mole/issues/222)) | A (README via `gh`) |
| **dust / dua / gdu / ncdu** | CLI rápido; dua com marcar-e-apagar | Clone contado várias vezes (dust#590), memória, nada de APFS | A (Frente E §1.1) |
| **Storage Peek**, **Sensei** | Menu bar, treemap barato | Sem nada de APFS além da varredura | B (reviews de vendedor) |
| `tmutil`, `diskutil apfs`, `du` | Oficiais, scriptáveis | Sem tamanho de snapshot (`tmutil`), `du` sem clone | A (comunidade §6) |

## 6. Ideias para copiar e onde somos diferentes

### 6.1 Para copiar

1. **Uma fatia explícita para o resto, com filhos.** "hidden space → purgeable / other volumes /
   still hidden" é o modelo mental que o usuário já conhece. Nosso "não atribuído" pode abrir do
   mesmo jeito, mas cada filho medido (§6.2).
2. **Colocar numa área, rever, confirmar.** O Collector separa a escolha da execução, deixa
   pré-visualizar e tirar item. Serve à fase 4: o `sdtally` pode imprimir um plano que só
   executa com `--apply`. O "5 s para cancelar" também cabe no CLI.
3. **Lista de pastas que nunca aceita apagar** (`/System`, `/Library`, a home). Nossa versão:
   bloquear por padrão e explicar o motivo.
4. **Texto "por que o espaço não voltou".** O DaisyDisk explica snapshot e atraso do purgeable
   no fluxo de apagar. Copiar a ideia, com o nosso número: "estes X GB continuam presos no
   snapshot de 27/09 08:16".
5. **Atualizar purgeable e hidden sem varrer de novo** quando o macOS publica o número novo.
   No CLI: `--watch` só nas linhas por volume.
6. **Marcas visuais por tipo de item**: restrito, hardlink primário/secundário, apagado desde a
   varredura, grupo "smaller objects". No texto: marcadores fixos por linha.
7. **Tela de FDA que mostra o estado e leva ao lugar certo.** No CLI: mostrar o estado, dizer
   qual app de terminal precisa de FDA e dar o link `x-apple.systempreferences:`.
8. **Documentação em FAQ curto por sintoma** ("Mismatches with Finder", "Free space not
   appearing after deletion"). Serve para o README.
9. **Avisar de snapshot de terceiro** (CCC, Synology) e mandar ao app dono, sem tentar apagar.

### 6.2 Onde somos diferentes

| Medição | DaisyDisk | Nosso plano |
|---|---|---|
| Tamanho de snapshot | Estimado, fora da soma, ausente na App Store | **Privado exato por `fsctl 0xC0184A13`**, faixa completa, dentro da conta (síntese §3c: 51,49 GiB) |
| Clones | Clone completo por arquivo (desde jul/2026) | `CLONEID` + `EF_SHARES_ALL_BLOCKS`, **dedup por bloco físico** (`F_LOG2PHYS_EXT`) e enxertos de cryptex (Frente L) |
| "Quanto apagar libera" | Mostra alocado; não distingue o que o snapshot segura | `ATTR_CMNEXT_PRIVATESIZE` por arquivo |
| Pastas `EPERM` (SIP, data vault) | Viram "still hidden", mesmo como root | **Dir-stats sem abrir** (`fsctl 0xC1104A71`, buffer zerado): 19,27 GiB nomeados aqui (Frente H) |
| Arquivos apagados e abertos | Não cita | `lsof +L1` deduplicado por (dev, inode): 3,47 GiB aqui |
| Purgeable | Um número | **Por serviço** (`CacheDeleteCopyItemizedPurgeableSpaceWithInfo`), soma igual ao total |
| Número do painel | Não relaciona | **Reproduz a tela** (Frente M: macOS e Apple Intelligence ao byte) e mostra a ponte |
| Atribuição | Caminho e nome | **Dono por app** (`MCMMetadataIdentifier`, instalado ou não; Frente C) |
| Metadado APFS | "2–3 GB é normal" | Medido (1,23 GiB, Frente G) |
| iCloud/dataless | "Suporte" sem detalhe | Política de não materializar, não descer em pasta dataless, contar e reportar (Frente F) |
| Origem de cada número | Não mostra | Coluna de origem em toda linha |
| Segurança de leitura | Não documentada | `atime` desligado, sem atravessar montagens, sem `fsctl` durante backup (síntese §4) |

### 6.3 Onde ele é melhor que o nosso plano

- **UX visual e descoberta:** o sunburst acha o maior ramo em segundos. Nosso CLI de texto não
  faz isso; a TUI só chega na fase 3.
- **Fluxo completo de limpeza** hoje, com proteção e pré-visualização. Nós só lemos até a fase 4.
- **Purgar e apagar snapshot com um gesto.** Não vamos oferecer purge geral (síntese §4b); o
  DaisyDisk resolve o caso "preciso de espaço agora" que nós só explicamos.
- **Nuvem por API** (Dropbox, Drive, OneDrive, Box), fora do nosso escopo.
- **Anos de casos de suporte** (notas desde a 2.0.7, 2011), as páginas de FAQ e o suporte por e-mail, que resolveu
  o caso CCC (§3).
- **Distribuição:** notarizado, App Store e avulso. Nós: tap próprio, ad-hoc, sem notarização
  (Frente E). Para leigo, o DaisyDisk é mais fácil de instalar.
- **Não precisa de Terminal.** Nosso público inicial é quem abre o Terminal.

## 7. Recomendação

- **Não somos "um DaisyDisk".** Ele responde "onde estão os arquivos grandes?" e deixa o resto
  como "hidden". Nós respondemos "por que o volume tem X usados e o Ajustes diz Y em Dados do
  Sistema?", medindo justamente as partes que ele estima ou esconde: snapshot, `EPERM`, abertos
  apagados, purgeable por serviço, clone por bloco.
- **Posicionamento em uma frase:** *"Explica o 'Dados do Sistema' do macOS com números medidos:
  o que mais pesa, de quem é e quanto apagar libera de verdade; o resto vira uma linha com o
  motivo."*
- **Relação com ele:** complementar. O README pode dizer "use o DaisyDisk para navegar nos seus
  arquivos; use este para entender o que ele chama de hidden space". Isso evita competir onde
  ele é melhor (§6.3).
- **Métrica de sucesso** que o DaisyDisk não atinge: "still hidden" equivalente abaixo de 5 % do
  uso, ou cada GB acima disso com nome e causa medida. É o critério central da síntese §8.

## 8. Incertezas

- O método de estimativa dos snapshots, o mecanismo do purge e o helper do modo admin não são
  públicos. Confirmar só rodando o app, fora do escopo desta frente.
- Não há teste independente de precisão do DaisyDisk em macOS 26/27. Os números de velocidade vêm
  de blogs de concorrentes.
- Os casos do §3 são de 2018–2025; nenhum do macOS 27 foi achado.
- A frase "DaisyDisk calcula tudo duas vezes" apareceu num resumo de busca atribuído à Apple
  Community e não foi confirmada numa página. Ficou fora.

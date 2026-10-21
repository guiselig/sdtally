# Frente D — permissões, volumes e a conta do container — 2026-09-27

O que o CLI consegue **ler** em cada nível de privilégio e em cada volume, e como fechar a
conta do container APFS inteiro só com medições.

Máquina: macOS 27.0 (26A5416b), Apple Silicon, SSD de 500 GB. Sem sudo, sem instalar nada,
sem mudar configuração. **Backup do Time Machine rodando durante as medições** (7 snapshots
locais montados), então os números de uso variam alguns centésimos de GiB entre medições.

Confiança: **A** alta (medido aqui ou fonte primária), **M** média (fonte secundária confiável
ou medido com ressalva), **B** baixa (inferência).

Programas (só leitura, só `libc`) em `research/experimentos/frenteD/`:

| Binário | O que faz |
|---|---|
| `tcc_probe` | `lstat` + `open` + `readdir` em caminhos protegidos, errno por camada; `--disclaim` re-executa o binário como o próprio "responsible process" |
| `vols` | `statfs` + `getattrlist(ATTR_VOL_*)` por volume montado; soma por container |
| `deleted_open` | Arquivos apagados e ainda abertos/mapeados (`nlink == 0`) via libproc |
| `errwalk` | Varre um volume sem cruzar para outros e lista toda pasta negada com errno |

Compilar: `CARGO_TARGET_DIR=/tmp/sdc-target cargo build --release --offline` (target fora do iCloud).

---

## 0. Correção importante a pesquisas anteriores

**Os experimentos de 26/09 muito provavelmente não rodaram "sem Full Disk Access"**, e sim com FDA herdado. (M: o mecanismo e a concessão são medidos hoje; que a sessão de 26/09 tinha a mesma cadeia de processos é inferência)

- A cadeia de processos é `Claude.app → disclaimer → claude (Claude Code) → zsh`. O helper
  `disclaimer` renuncia à responsabilidade, então o **responsible process** do shell é o binário
  `claude` (`com.anthropic.claude-code`), medido com `responsibility_get_pid_responsible_for_pid`.
- O TCC.db do sistema tem `com.anthropic.claude-code | kTCCServiceSystemPolicyAllFiles | auth_value=2`
  (concedido em 2026-03-30), e o log do `tccd` mostra
  `AUTHREQ_ATTRIBUTION … responsible=com.anthropic.claude-code … authValue=2` para o nosso shell.
- Consequência: os "141 erros por TCC" de `research/2026-09-26-pesquisa-rust.md` são o que sobra
  **com** FDA (pastas de root e SIP), não o custo de não ter FDA. O custo real de não ter FDA foi
  medido agora (§1.3).

---

## 1. TCC e Full Disk Access

### 1.1 Camadas e errno

| Camada | errno | Quem passa | Fonte |
|---|---|---|---|
| Permissão BSD / ACL | `EACCES` (13) | root | Quinn, [On File System Permissions](https://developer.apple.com/forums/thread/678819) (A) |
| Sandbox, MAC/TCC, SIP (`SF_RESTRICTED`), data vault | `EPERM` (1) | ninguém sem o direito certo; **root não passa** | idem (A) |

Medido aqui: o errno depende de qual camada nega **primeiro**. `.Spotlight-V100` na raiz do Data
dá `EACCES` com FDA (só falta ser root) e `EPERM` sem FDA (o TCC nega antes). (A)

`lstat` do próprio diretório protegido **funciona sem FDA** (tamanho do inode, flags, datas); o
que falha é `open`/`readdir`. Então o CLI sempre sabe que a pasta existe e pode listá-la como
"não lida". (A, medido em 27 caminhos)

### 1.2 O que exige FDA (lista medida em macOS 27)

Apple documenta só a categoria: "Mail, Messages, Safari, Home, backups do Time Machine e certos
ajustes administrativos de todos os usuários" ([Apple, mchl211c911f](https://support.apple.com/en-us/guide/mac-help/mchl211c911f/mac)). (A)
A lista abaixo é o que **deu EPERM sem FDA e OK com FDA** nesta máquina (`tcc_probe --disclaim` × `tcc_probe`):

| Caminho | Sem FDA | Com FDA |
|---|---|---|
| `~/Library/Mail`, `Messages`, `Safari`, `Cookies`, `HomeKit`, `Suggestions` | EPERM | OK |
| `~/Library/Metadata/CoreSpotlight`, `Biome`, `IdentityServices`, `Accounts`, `Trial`, `DuetExpertCenter`, `Sharing`, `PersonalizationPortrait` | EPERM | OK |
| `~/Library/Application Support/{AddressBook, CallHistoryDB, Knowledge, FaceTime, com.apple.sharedfilelist}` | EPERM | OK |
| `~/Library/Containers/com.apple.{Safari, mail, stocks}`, `~/Library/Group Containers/group.com.apple.notes` | EPERM | OK |
| `~/.Trash` | EPERM | OK |
| `/Library/Application Support/com.apple.TCC` e `TCC.db` do sistema | EPERM | OK |
| `/System/Volumes/Data/.DocumentRevisions-V100`, `.TemporaryItems` | EPERM | EACCES / OK |
| `/Volumes/com.apple.TimeMachine.localsnapshots` (snapshots montados) | EPERM | OK |

Não testados sem FDA, de propósito, porque **podem abrir diálogo na tela** (§1.5): Desktop,
Documents, Downloads, iCloud Drive, volumes de rede/removíveis, Containers e Group Containers
de apps de terceiros, `Pictures/Photos Library.photoslibrary`. Por fonte:

- **Files & Folders** (Desktop, Documents, Downloads, iCloud Drive, rede, removível, "dados de
  outros apps"): prompt por app; FDA tem precedência e dispensa todos.
  [Oakley 2026-04-08](https://eclecticlight.co/2026/04/08/privacy-files-folders-or-full-disk-access/) (A)
- **App Data** (`kTCCServiceSystemPolicyAppData`): Containers de outros apps desde macOS 14,
  Group Containers desde macOS 15. Quinn lista "App container protection (14+)" e "App group
  container protection (15+)". [CVE-2023-42929](https://jhftss.github.io/CVE-2023-42929-Why-Do-We-Need-The-App-Container-Protection/) (A)
- Com FDA herdado, **todos** os 24 Containers e 55 Group Containers de terceiros abriram. (A, medido)

### 1.3 Custo medido de não ter FDA

Sem FDA, 26 dos 27 caminhos da tabela deram EPERM (o 27º não existe, ver §1.6). Isso inclui
`~/.Trash` e os Containers da Apple, que são alvos do projeto. **Não medi quantos GiB ficam de
fora sem FDA**: varrer `~` sem FDA num processo com identidade própria dispararia prompts. Fica
para rodar num terminal sem FDA, com o usuário olhando. (Referência externa: OmniDiskSweeper sem
FDA viu 102 de 225 GB, em `research/2026-09-27-comunidade-system-data.md`, M.)

### 1.4 Como FDA é herdado por um CLI

- **Quem o TCC avalia é o "responsible process"**, não o binário que faz o `open`. Um CLI lançado
  pelo Terminal herda o FDA do Terminal; pelo iTerm, do iTerm; pelo VS Code, do VS Code; pelo
  Claude Code, do `claude`. (A, medido: mesmo binário, OK herdando, EPERM com responsabilidade
  renunciada; Quinn, "responsible code")
- Um processo pode **renunciar** (`responsibility_spawnattrs_setdisclaim`, API privada da
  libSystem, a mesma que o helper `disclaimer` do Claude.app usa). Aí ele vira responsável por si
  mesmo e o TCC avalia a assinatura dele. (A, medido)
- **Binário assinado ad hoc** (o padrão do linker no arm64; `codesign -dv` mostra
  `flags=0x20002(adhoc,linker-signed)`, `TeamIdentifier=not set`): o TCC guarda a concessão pela
  assinatura; ad hoc muda a cada build, então dar FDA ao próprio binário se perde a cada
  `brew upgrade`. O `macos-sysdata` documenta o mesmo problema no README. (M)
- **ssh / launchd**: sem sessão GUI não há prompt, só negação; job de launchd tem o binário como
  responsável (use `AssociatedBundleIdentifiers` se for de um app). (A, Quinn)
- **Não existe API para pedir FDA**, nem por CLI. Quinn: "No. The purpose of such privileges is
  to allow the user to control their Mac." Só MDM (`com.apple.TCC.configuration-profile-policy`).
  [forum 771162](https://developer.apple.com/forums/thread/771162) (A)

**Recomendação:** o CLI não pede FDA para si. Detecta, e se faltar diz: "dê Acesso Total ao Disco
ao seu terminal (Terminal, iTerm, …)", nomeando o app responsável (dá para obter o caminho com
`responsibility_get_pid_responsible_for_pid` + `proc_pidpath`; API privada, usar só para a
mensagem, com fallback).

### 1.5 Detectar FDA sem prompt

Checar **abrindo** (`open(O_RDONLY)` / `opendir`), nunca com `stat` (que funciona sem FDA):

1. `/Library/Application Support/com.apple.TCC` (existe em 27; EPERM sem FDA, OK com). (A)
2. `~/Library/Safari`, `~/Library/Mail`, `~/Library/Containers/com.apple.stocks` como reforço
   (sugestão também do [cleardisk#40](https://github.com/bysiber/cleardisk/issues/40)). (A, medido)
3. `ENOENT` = inconclusivo, não "sem FDA". EPERM em todos = sem FDA.

Esses caminhos são da classe FDA, que **não abre prompt**: negação silenciosa. Nenhum prompt
apareceu nos logs do `tccd` para eles. (A)

**Não usar como sonda** nada de Files & Folders ou App Data: isso abre diálogo em nome do
terminal ("Terminal quer acessar a pasta Mesa") e o usuário pode clicar "Não Permitir", o que
**nega de vez** (FDA desligado só "não concede"; Files & Folders negado nega). (A, Oakley 2026-04-08)

Incidente do teste: sem FDA, o `tcc_probe` travou **mais de 2 minutos** em `open` de
`~/Pictures/Photos Library.photoslibrary` e foi morto. O log do `tccd` (nível padrão) não mostrou
pedido de autorização, então **não sei se apareceu diálogo** nem qual a causa. O caminho saiu da
lista segura e o filho agora se mata após 30 s. O CLI deve tratar bibliotecas de fotos como
"delegar ao app dono" e não abrir. (B quanto à causa)

### 1.6 O que mudou em macOS 26/27

- **TCC.db do usuário sumiu de `~/Library/Application Support/com.apple.TCC/`** em macOS 27: foi
  para `/private/var/containers/Data/ProtectedSystem/<UUID>/…`, ilegível até com FDA. A sonda
  clássica (tentar ler esse arquivo) agora dá `ENOENT` e **faz toda ferramenta achar que não tem
  FDA**. Medido: `ENOENT` com e sem FDA; `ProtectedSystem` dá EPERM com FDA.
  [Csaba Fitzl](https://x.com/theevilbit/status/2066443202479976461), [fleet#53273](https://github.com/fleetdm/fleet/issues/53273),
  [cleardisk#40](https://github.com/bysiber/cleardisk/issues/40), `docs/03` precisa corrigir. (A)
- **"Golden Gate"**: macOS 27 protege com `com.apple.macl` o `~/Library/Application Support/<app>`
  de uma lista fechada de apps não sandboxed (Chrome, Firefox, Brave, Edge, Discord, carteiras
  cripto), atualizável por XProtect; nega com "Operation not permitted".
  [Regula](https://wojciechregula.blog/post/golden-gate-appdata-protection/), [mjtsai](https://mjtsai.com/blog/2026/07/24/golden-gate-application-support-protection/) (A).
  **Medido aqui:** com FDA herdado, `discord`, `Google/Chrome`, `Firefox`, `BraveSoftware`,
  `Microsoft Edge` abriram normalmente. Sem FDA não testado (risco de prompt). Pode ser que FDA
  passe por cima ou que o bloqueio só afete Terminal sem FDA; **não confirmado** (M).
- `tccutil list` existe em 27 mas só em Mac gerenciado por MDM.
  [feedback-assistant#844](https://github.com/feedback-assistant/reports/issues/844) (M)
- Novo firmlink `/pkg` (dir criado em 26.4, firmlink em 27). Já aparece em `/usr/share/firmlinks`
  aqui. [Oakley 2026-09-19](https://eclecticlight.co/2026/09/19/macos-tahoe-26-4-changed-boot-volume-structure/) (A)

---

## 2. Root

### 2.1 O que fica de fora mesmo com FDA (medido)

`errwalk /System/Volumes/Data` com FDA, sem root: 2,13 mi arquivos, 354 mil pastas, 63 s;
**206 pastas negadas** (180 EACCES, 26 EPERM).

**EACCES (só root resolve):** 157 em `/private/var`: `folders/zz` (78, temporários de daemons
de root), `spool/postfix`, `protected/sfanalytics`, `db/CoreDuet`, `db/assetsubscriptiond`,
`db/timed`, `root`, `db/sudo`, `audit`, `run/mds`, `networkd/Library`, `ma`, `install`,
`dirs_cleaner`; mais `.Spotlight-V100`, `.fseventsd`, `.DocumentRevisions-V100` na raiz do Data;
`/Library/Application Support/{Apple/Remote Desktop, Google/GoogleUpdater}`, `/Library/Caches`
(1 pasta), `/usr/local/mysql-*/data|keyring`, `XAMPP/xamppfiles/var`. (A)

**EPERM (nem root resolve: SIP/`restricted`, data vault, sandbox):** 26, entre eles
`/private/var/db/{Spotlight-V100, Spotlight, AppleIntelligencePlatform, SystemKeys, searchparty,
lockdown, oah, DumpPanic, SoC, appinstalld, installcoordinationd, sysdiagnose/…}`,
`/private/var/containers/Data/ProtectedSystem`, `/private/var/folders/zz/…/{C,0}` (flag `sunlnk`),
`/private/var/protected/trustd/private`, e **seis pastas de modelos da Apple Intelligence**:
`/System/Library/AssetsV2/com_apple_MobileAsset_UAF_FM_{GenerativeModels, CodeLM, Visual,
Overrides}` e `UAF_IF_Planner*` (flag `restricted`). (A)

Implicação direta: **os modelos da Apple Intelligence (20–30 GB relatados em macOS 27) não são
medíveis por varredura, nem como root.** Caem no "não atribuído". O CLI pode listar essas pastas
pelo nome (o `lstat` funciona) e dizer que não dá para medir. (A para a negação; tamanho B)

Outros usuários: `/Users` aqui só tem `Shared` e o dono. Em Mac com vários usuários, `~outro`
é `drwxr-x---`/`drwx------` → EACCES; exige root **e** o TCC do outro usuário continua valendo
para Containers/Mail dele. (M)

`/private/var/log`, `/private/var/log/asl`, `/private/var/db/diagnostics`, `uuidtext`, `vm`,
`/Library/Logs/DiagnosticReports` e os volumes `/System/Volumes/*` abriram sem root. (A)

### 2.2 Riscos de rodar como root

- **Não ganha TCC.** MAC vale para root; `sudo` num terminal sem FDA continua com EPERM. (A, Quinn)
- **Não ganha SIP/data vault.** As 26 pastas EPERM seguem fechadas. (A)
- Ganha só ~180 pastas de sistema (EACCES). Nesta máquina o que elas somam não foi medido (não
  temos root); o volume inteiro fecha com erro de 33 GiB antes da dedup de clones (§4.4), então
  o ganho de root é menor que isso.
- Custo: qualquer arquivo que a ferramenta crie (cache, relatório, histórico) fica de root no
  home do usuário; leitura de dados de outros usuários (privacidade); ataque por symlink/TOCTOU
  se um dia houver remoção. Um CLI de só leitura não precisa correr esse risco. (M)

### 2.3 Privileged helper (SMAppService) para um CLI open source

- `SMAppService.daemon(plistName:)` exige **app bundle** com o plist em
  `Contents/Library/LaunchDaemons/`, `BundleProgram` relativo ao bundle, mesma Team ID no app e no
  daemon, aprovação do usuário em Ajustes > Geral > Itens de Início, macOS 13+. `SMJobBless` está
  obsoleto desde o 13. [SMAppService](https://developer.apple.com/documentation/servicemanagement/smappservice) (A)
- DTS: "SMAppService is really intended to be used by apps". Fora de app, o caminho é `.pkg` que
  instala em `/Library/LaunchDaemons`. [forum 771162](https://developer.apple.com/forums/thread/771162) (A)
- Custo real: conta Developer ID (USD 99/ano, para Team ID estável), notarização, app bundle,
  XPC com autenticação do cliente, e superfície de ataque de um daemon root. Para um CLI de só
  leitura cujo ganho com root é marginal (§2.1), **não se paga**. Mantém a decisão de `docs/03`.
- Se um dia houver ação de root (ex.: `tmutil deletelocalsnapshots`), o CLI **imprime o comando**
  para o usuário rodar com `sudo`, em vez de escalar privilégio sozinho.

### 2.4 O que o `macos-sysdata` (Jarvis322) fez de errado

Lido: commit `9343008` (2026-09-25). Crítica do Oakley: abordagem "inherently dangerous",
informação "misleading or incorrect", "old scripted workaround … rather than … a privileged
helper". [eclecticlight 2026-09-06](https://eclecticlight.co/2026/09/06/last-week-on-my-mac-storage-crisis-management/) (A)

No código:

1. **Root por AppleScript:** `Shell.runPrivileged` monta
   `do shell script "<script>" with administrator privileges` e roda via `/usr/bin/osascript`
   (`Sources/SysDataMenu/Services/Shell.swift:116-122`). A string é escapada à mão (`\` e `"`).
2. **`rm -rf` como root sobre caminhos montados em string:** `Reclaimer.remove` tenta
   `trashItem`, cai para `removeItem` e, se falhar, junta tudo num `rm -rf '<a>' '<b>'` e roda como
   root (`Reclaimer.swift:117-147`). Qualquer falha de permissão vira apagamento irreversível como
   root, fora do Lixo.
3. **Globs como root:** `rm -rf /Library/Logs/DiagnosticReports/*`,
   `rm -f /private/var/log/asl/*.asl`, `rm -rf /Library/Developer/CommandLineTools`
   (`SystemProbes.swift:28, 36, 286`).
4. **"Safe" que não é:** `log erase --all` (apaga todo o log unificado, útil para diagnóstico)
   classificado `.safe` (`SystemProbes.swift:15-19`); `tmutil deletelocalsnapshots / ; tmutil
   thinlocalsnapshots / 9999999999999 4` apaga **todos** os snapshots locais, `.safe`
   (`DeveloperProbes.swift:25`); sugere `sudo pmset -a hibernatemode 0` + `rm sleepimage`
   (muda ajuste de energia do sistema).
5. **CLI `sysdata` em bash com `sudo rm -rf`** e `sudo find … -exec rm -rf {} +` (`sysdata:82-136`).
6. **Medição sem dedup:** `DiskSize.measureSync` soma `totalFileAllocatedSize` de cada arquivo
   (`DiskSize.swift`), sem hardlink nem clone; com `du -sk` no CLI. Aqui isso superestima
   `~/Library` em ~21 GiB (clones) e o Data em 11 GiB (hardlinks, §4.4).
7. **Erro de leitura vira zero, calado:** o enumerator usa `errorHandler: { _, _ in true }`; pasta
   negada (TCC, root) some da soma sem aviso. Oposto do princípio "mostrar o não lido".
8. **Comentário que afirma sem fonte:** "following the same rules Finder uses for System Data".
9. **Purgeable = `importantUsage − free`** (`DiskSize.purgeableSpace`), que o próprio projeto chama
   de estimativa; ok desde que rotulado.
10. Licença **proprietária** (badge e `LICENSE`), apesar de parecer projeto aberto.

O que o nosso projeto deve fazer ao contrário: só leitura na v1; nenhuma escalada automática;
remoção futura só via `trashItem` e com o comando oficial impresso para o usuário rodar; toda
pasta não lida aparece; toda soma com dedup de hardlink e clone.

---

## 3. Volumes e container

### 3.1 O que existe nesta máquina (medido)

`diskutil apfs list -plist` (sem root, 1,6 s):

| Container | Tamanho | Volume | Papel | Em uso | Montado em |
|---|---|---|---|---|---|
| **disk3** (o do painel) | 494,38 GB (460,43 GiB) | disk3s1 Macintosh HD | System (SSV, selado) | 11,78 GiB | `/` via snapshot `disk3s1s1` |
| | | disk3s5 Data | Data | 330,02 GiB | `/System/Volumes/Data` |
| | | disk3s2 Preboot | Preboot | 10,08 GiB | `/System/Volumes/Preboot` |
| | | disk3s6 VM | VM | 10,00 GiB | `/System/Volumes/VM` |
| | | disk3s3 Recovery | Recovery | 2,75 GiB | `/Volumes/Recovery` |
| | | disk3s4 Update | Update | 0,08 GiB | `/System/Volumes/Update` |
| disk1 (iSCPreboot) | 0,52 GB | disk1s1 iSCPreboot, s2 xART, s3 Hardware | Preboot/xART/Hardware | 6,0 + 6,0 + 3,8 MiB | `/System/Volumes/{iSCPreboot,xarts,Hardware}` |
| | | disk1s4 Recovery | Recovery | 20 KiB | **não montado** |
| disk2 (recoveryOS de reserva) | 5,37 GB | disk2s1 Recovery, s2 Update | | 2,39 GiB + 2,8 MiB | **não montado** |

- O painel de Armazenamento usa só o **disk3** (494,38 GB = "494 GB" do painel). disk1 e disk2
  são containers separados no mesmo SSD. (A)
- **VM** = 9 `swapfile` de 1 GiB + `kernelcore` 1 GiB; `sysctl vm.swapusage` = 9 GiB total,
  7,3 GiB usados. Legível sem root por `ls` do diretório. (A)
- `Sealed` não vem no `-plist` do `diskutil apfs list` (vem no texto e em IOKit:
  `ioreg -c AppleAPFSVolume` → `"Sealed" = "Yes"`). (A)

### 3.2 Como medir cada volume sem root

| Método | Dá | Não dá |
|---|---|---|
| `getattrlist(mountpoint, ATTR_VOL_SPACEUSED)` | **Uso do volume**, igual ao `CapacityInUse` do `diskutil` (Data: 330,025 GiB × 330,02). Também `ATTR_VOL_SIZE/SPACEFREE/SPACEAVAIL/FILECOUNT/DIRCOUNT/QUOTA_SIZE/RESERVED_SIZE` | Volume não montado |
| `statfs(2)` | `f_blocks` = **tamanho do container**; `f_bfree` = `f_bavail` = **livre do container** | Uso por volume. `(f_blocks − f_bfree)` dá o **usado do container inteiro** (364,86 GiB em todo volume do disk3) |
| `diskutil apfs list -plist` | Todos os containers e volumes, montados ou não, papel, `CapacityInUse`, quota, reserva, FileVault | Snapshots com tamanho |
| IOKit (`AppleAPFSVolume`) | Papel, `Sealed`, BSD name, `Size` (= container) | Uso por volume |
| `diskutil apfs listSnapshots -plist <vol>` | Nome, XID, `Purgeable`, `LimitingContainerShrink` | Tamanho |

Medido, e **corrige** `research/2026-09-26-pesquisa-rust.md` §4 ("usado (`statfs`)"): no APFS o
`statfs` não dá o uso do volume. O `df` mostra o uso por volume (não verifiquei de onde ele tira o número).
Fonte do "usado" do Data: **`ATTR_VOL_SPACEUSED`**. (A)

Armadilha de parser (medida por hexdump): o kernel empacota `ATTR_VOL_SPACEUSED` **logo depois de
`ATTR_VOL_SPACEAVAIL`**, fora da ordem dos bits. Parser que segue a ordem de bits lê lixo. (A)

Rust: `libc` já tem `getattrlist`, `getmntinfo`, `ATTR_VOL_SPACEUSED/QUOTA_SIZE/RESERVED_SIZE`.
Para o `diskutil`: `std::process::Command` + crate `plist` (não instalado aqui; decidir). Não
achei crate que exponha o `CapacityInUse` sem o `diskutil` (a API é o `APFSUserClient`, privado). (M)

### 3.3 Volume group, firmlinks e por que `du /` duplica

- `/` é o snapshot selado do System; `/usr/share/firmlinks` liga 19 diretórios do System ao
  Data (`/Users`, `/Applications`, `/Library`, `/private`, `/opt`, `/usr/local`, `/pkg`,
  `/System/Library/{Caches, Assets, AssetsV2, PreinstalledAssets*, Speech}`, …). (A, lido aqui)
- **`st_dev` é o mesmo para o volume group inteiro**: `/`, `/System`, `/Users` e
  `/System/Volumes/Data` dão todos `st_dev = 0x100000f` (o do Data). Objetos do System têm inode
  com prefixo `0x0FFFFFFF…` (ex.: `/System` = `0x0fffffff00000017`). (A, medido;
  [Oakley, inodes](https://eclecticlight.co/2025/10/04/explainer-inodes-and-inode-numbers/))
- Por isso **`du -x /` e `find -x /` não param em `/System/Volumes/Data`**: entram no Data duas
  vezes, pelos firmlinks e pelo ponto de montagem. Medido: `find -x / -maxdepth 4` lista
  `/System/Volumes/Data/Users`. Os outros volumes (Preboot `0x100000d`, VM, Update, Recovery,
  snapshots do TM, backup) têm `st_dev` próprio e são pulados. (A)
- Dedup por `(st_dev, st_ino)` continua correta (inodes são únicos no grupo). (A)

### 3.4 Como varrer o Data sem cruzar volumes

1. Raiz = `/System/Volumes/Data`, nunca `/`.
2. Não seguir symlink (`/Volumes/Macintosh HD -> /` está dentro do Data).
3. Pular entrada com `st_dev` (ou `ATTR_CMN_DEVID`) ≠ o da raiz: pega `/System/Volumes/Data/home`
   (autofs), `Volumes/Recovery`, `Volumes/Backups of …`, snapshots do TM em
   `Volumes/com.apple.TimeMachine.localsnapshots/…`, o nullfs do app `Canvas` em
   `/private/var/folders/…/X/…`. (A, medido: 6 entradas puladas)
4. Reforço barato no `getattrlistbulk`: `ATTR_DIR_MOUNTSTATUS & DIR_MNTSTATUS_MNTPOINT`
   (`sys/attr.h`); e a lista de `getmntinfo` como no `errwalk`. (A para a existência; não medido
   no bulk)

---

## 4. Fechar a conta

### 4.1 Container = soma dos volumes + metadado do container (medido, exato)

| | disk3 | disk1 |
|---|---|---|
| Tamanho − livre (`CapacityCeiling − CapacityFree`) | 364,857 GiB | 0,020 GiB |
| Σ `ATTR_VOL_SPACEUSED` (todos montados no disk3) | 364,707 GiB | 0,015 GiB (falta disk1s4, 20 KiB) |
| **Diferença** | **160.395.264 B (153 MiB)** | 4.956.160 B |

A diferença do disk3 foi **idêntica, ao byte, em duas medições** com 20 min e o backup mudando
o Data entre elas; bate com a mesma conta sobre o `diskutil`. É espaço que o container usa e não
cobra de volume nenhum: checkpoint, object map e space manager do container
([Apple File System Reference](https://developer.apple.com/support/downloads/Apple-File-System-Reference.pdf),
`nx_superblock_t`). A diferença é **medida**; a atribuição a essas estruturas é inferência (M).

Armadilha: o texto do `diskutil` chama `CapacityCeiling − CapacityFree` de "Capacity In Use By
Volumes", mas o número **inclui** esses 153 MiB. A soma real dos volumes é menor. (A)

Conta do container, só com medições:

```
tamanho do container (diskutil / ATTR_VOL_SIZE)
  = livre (ATTR_VOL_SPACEFREE)
  + Σ ATTR_VOL_SPACEUSED dos volumes montados
  + Σ CapacityInUse dos não montados (diskutil)
  + metadado do container (diferença exata)
```

### 4.2 Dentro do Data

```
ATTR_VOL_SPACEUSED(Data)
  = Σ alocado da árvore (hardlink 1×, clone total 1×)        ← varredura
  + clones parciais contados 2× (sinal negativo; não separável) ← limitação conhecida
  + blocos só em snapshot                                     ← não medível direto
  + arquivos apagados-e-abertos                               ← §4.3
  + pastas não lidas (TCC/root/SIP)                            ← contadas, sem bytes
  + metadado do volume (B-trees, omap do volume)              ← não medível sem root
  = "não atribuído" é o resto exato
```

- **Metadado APFS do volume:** não achei API pública que o devolva. `ATTR_VOL_SPACEUSED` já
  inclui; nenhum arquivo o carrega. O superbloco do volume tem `apfs_fs_alloc_count`, mas lê-lo
  exige acesso bruto ao disco (root). IOKit só expõe contadores de I/O de metadado
  (`Statistics` → `Metadata: …`), não tamanho. **Fica no "não atribuído"**, sem rótulo. (M)
- **Snapshots:** `listSnapshots` e `tmutil` listam sem root e sem tamanho; a parte medível é por
  arquivo (`ALLOCSIZE − PRIVATESIZE`, ver pesquisa de 26/09). `tmutil listlocalsnapshots` marca
  um snapshot como `(dataless)` aqui; não investigado. (A para a listagem)
- **Purgeable:** frente de outra pesquisa (`vol.swift`); não entra na soma do usado.

### 4.3 Arquivos apagados mas abertos

`lsof +L1` sem sudo: 0,2 s, 1.012 linhas, **315 arquivos únicos, 3,05 GiB** (tamanho lógico),
todos de processos do próprio usuário. **2,89 GiB são 195 cópias antigas do banco do
LaunchServices** (`/private/var/folders/…/0/com.apple.LaunchServices.dv/*.csstore`, ~16 MB cada)
mapeadas por daemons como `chronod`, `linkd`, `WindowManager`, widgets. O resto: PDFs abertos no
Preview (108 MB), índices do CoreSpotlight, Biome. (A, medido)

`deleted_open` (Rust, `libproc`, sem root), 1,4 s:

- `proc_listallpids` → `proc_pidinfo(PROC_PIDLISTFDS)` → `proc_pidfdinfo(PROC_PIDFDVNODEPATHINFO)`
  para fds; **e** `proc_pidinfo(PROC_PIDREGIONPATHINFO)` percorrendo o espaço de endereço para
  arquivos **mapeados** (o `lsof` mostra como FD `txt`; não aparecem na lista de fds). Filtro
  `vst_nlink == 0`, arquivo regular; dedup por `(vst_dev, vst_ino)`; tamanho `vst_blocks × 512`.
- Resultado: 547 processos lidos, **232 negados (outro uid)**; 120 arquivos, **0,163 GiB alocados**.
- **Falta 2,9 GiB em relação ao `lsof`:** para os `.csstore` dentro do data vault
  `com.apple.LaunchServices.dv`, o kernel devolve o caminho mas `vi_stat` **zerado** (ino 0,
  nlink 0, tamanho 0). O `/usr/sbin/lsof` tem o entitlement privado
  `com.apple.rootless.datavault.metadata` (visto com `codesign -d --entitlements`) e consegue o
  stat; binário comum não. O programa conta esses caminhos à parte (211 únicos) em vez de somar. (A)

O que dá sem root: processos do próprio usuário (fds e mmap); dentro de data vault, só o caminho.
Processos de root/outros usuários (ex.: temporários em `/private/var/folders/zz`): nada, nem com
`lsof`. Para o número completo do usuário, a única fonte medida é **rodar `/usr/sbin/lsof +L1 -F`**
e usar o SIZE dele (lógico, não alocado; rotular assim). (A)

### 4.4 Fechamento medido agora (Data, com FDA, sem root)

| Item | GiB | Origem |
|---|---|---|
| Usado do Data | 330,02 | `ATTR_VOL_SPACEUSED` |
| Árvore, hardlink 1×, **clone não deduplicado** | 297,02 | `errwalk` (st_blocks) |
| Hardlinks repetidos (fora da soma acima) | 10,99 | `errwalk` |
| Apagados-e-abertos do usuário | 0,16 medido + 2,89 lógico só via `lsof` | §4.3 |
| Pastas não lidas | 206 pastas, bytes desconhecidos | `errwalk` |
| Snapshots locais | 7 | `tmutil` |
| Diferença usado − árvore | **33,0** (antes de deduplicar clones) | aritmética |

Por árvore: `Users` 217,3 · `Applications` 44,7 · `private` 10,8 · `System` (AssetsV2 etc. no
Data) 9,1 · `opt` 8,0 · `Library` 5,2 · `usr` 1,9 GiB.

Como os clones repetidos em `~/Library` somavam 21,4 GiB em 26/09, o "não atribuído" real depois
da dedup de clones fica perto de 33 + 21 ≈ 54 GiB, com 7 snapshots do backup em curso. É ordem de
grandeza, não número: as duas varreduras são de dias diferentes. O protótipo precisa medir tudo
na mesma passada (walk do `probe` com clone dedup sobre `/System/Volumes/Data`). (M)

---

## 5. Recomendações para o CLI

1. Detectar FDA abrindo `/Library/Application Support/com.apple.TCC` + `~/Library/Safari`; nunca
   pela TCC.db do usuário (sumiu em 27). ENOENT = inconclusivo.
2. Sem FDA: avisar **antes** de varrer e não entrar em Desktop/Documents/Downloads/iCloud/
   Containers de terceiros/bibliotecas de Fotos, para não disparar prompts em nome do terminal.
3. Não pedir root. Mostrar as pastas EACCES/EPERM por nome e contagem; os modelos da Apple
   Intelligence e os `db` do sistema são não medíveis até para root.
4. Uso de volume por `ATTR_VOL_SPACEUSED`; nunca `statfs` para uso de volume APFS.
5. Container por `diskutil apfs list -plist` (inclui não montados) + `getattrlist`; mostrar o
   metadado do container como linha própria (diferença exata).
6. Varrer `/System/Volumes/Data` com poda por `st_dev`/mountpoint; dedup `(dev, ino)` e clone.
7. Apagados-e-abertos: `libproc` (fds + mmap) para o próprio usuário e `lsof +L1` para o que está
   em data vault, rotulado como tamanho lógico.

## 6. Incertezas

- Travamento do acesso a `Photos Library.photoslibrary` sem FDA: causa e se houve diálogo.
- Golden Gate com FDA × sem FDA: só testado com FDA (abriu).
- Quanto um usuário sem FDA perde, em GiB, na varredura de `~` (precisa de terminal sem FDA).
- Quanto as 180 pastas EACCES somam (precisa de root, fora do escopo).
- Atribuição dos 153 MiB a estruturas específicas do container (inferência pela especificação).
- `(dataless)` num snapshot do Time Machine: significado não investigado.

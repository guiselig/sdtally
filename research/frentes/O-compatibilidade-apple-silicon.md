# Frente O — Compatibilidade com todo Mac Apple Silicon (macOS 11 a 27)

29/09/2026. Decisão do dono: o `sdtally` roda **só em Apple Silicon (arm64)**, portanto do
macOS 11 Big Sur ao 27. Esta frente confere, fonte por fonte do `coleta/src/*.rs`, a partir de
qual versão cada API existe e o que muda por versão. A máquina de referência é um M4 com
macOS 27.0 (26A5416b).

**Graus.** (A) = lido em fonte primária (código da xnu por tag em
github.com/apple-oss-distributions/xnu, header do SDK com macro de disponibilidade, man/doc/nota
oficial da Apple) ou medido aqui. (B) = inferência, relato de terceiro ou testado só no 27.
Quando pesquisa e verificação divergiram, vale a verificação.

**Mapa macOS → xnu (A, submódulos de `apple-oss-distributions/distribution-macOS`):**
11.0.1 = 7195.50.7.100.1; 11.1 = 7195.60.75; 11.2 = 7195.81.3; 11.5 = 7195.141.2;
12.0.1 = 8019.41.5; 13.0 = 8792.41.9; 14.0 = 10002.1.13; 15.0 = 11215.1.10; 26.0 = 12377.1.9.
O 11.0 (20A2411, o de fábrica dos primeiros M1) e o 11.6–11.7.x não têm tag publicada (B para
eles).

---

## 1. Resultado

**O binário carrega em todo Apple Silicon.** `otool -l coleta/dist/sdc-coleta` dá
`minos 11.0`, arm64 (A, medido). Todos os símbolos importados sem weak (`_getattrlistbulk`,
`_fgetattrlist`, `_fs_snapshot_list`, `_fsctl`, `_setiopolicy_np`) existem na libSystem desde
≤ 10.12 (A, macros do SDK). Uma falha, se houver, é por chamada e não do dyld.

### 1.1 Funciona do 11 ao 27 (API presente, grau A)

- Travessia: `getattrlistbulk` (10.10), `fgetattrlist` (10.6), `FSOPT_ATTR_CMN_EXTENDED` (10.10),
  `ATTR_CMN_ERROR` (10.10), `ATTR_CMN_RETURNED_ATTRS` (10.6), `ATTR_CMNEXT_PRIVATESIZE` (10.13),
  `ATTR_CMNEXT_CLONEID` e `ATTR_CMNEXT_EXT_FLAGS` (10.15.4), `SF_DATALESS` (10.15.0),
  `listxattr`/`getxattr` com `XATTR_SHOWCOMPRESSION` (10.6), atributos básicos (10.6).
- Regras de segurança: `setiopolicy_np` com `IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES` = OFF
  (10.15.0) e `IOPOL_TYPE_VFS_ATIME_UPDATES` = OFF (10.14), ambos aceitos em escopo de processo
  sem privilégio no 11 (xnu-7195 `kern_resource.c`).
- `fs_snapshot_list` como API (10.12); a syscall `fsctl` (10.6);
  `pthread_set_qos_class_self_np` (10.10).
- `sysctl` `hw.model`, `hw.memsize`, `hw.physicalcpu`, `machdep.cpu.brand_string`, `kern.hostname`,
  `kern.osproductversion`.
- `lsof` 4.91 (tag lsof-68 no 11.0.1 até lsof-76 no 26), `scutil --get`, `id -F`
  (shell_cmds-216.40.4 no 11.0.1), `statfs`.

A existência da API não garante a semântica. Tudo o que o APFS (código fechado) calcula por
trás desses atributos só foi medido no 27 (ver §1.3).

### 1.2 Só a partir de uma versão (grau A, salvo indicação)

| Fonte | Desde | Efeito abaixo disso |
|---|---|---|
| `ATTR_VOL_SPACEUSED` (topo da conta) | **12.0** | EINVAL no 11. A conta fica sem o lado esquerdo |
| `hw.perflevel0.physicalcpu` | 12.0 | O sysctl falha e as threads caem para 4 (correto para M1) |
| `hw.perflevel0.name` | 13.0 | Não usado |
| Nome real do chip em `machdep.cpu.brand_string` | 11.1 | 11.0.x devolve "Apple processor" |
| Enxerto de cryptex (`graftdmg`) | 13.0 | Não há enxerto no 11 e no 12 |
| Enxerto do tipo MobileAsset (`.AssetData` em AssetsV2) | **15.0** | A regra de enxerto não tem o que achar no 11–14 |
| `EF_SHARES_ALL_BLOCKS` (dedup de clone completo) | **14.0** no header | No 11–13 é provável que a dedup não dispare (B) |
| `ATTR_CMNEXT_CLONE_REFCNT` | 14.0 | Não usado; pedir no 11–13 dá EINVAL |
| `VOL_CAP_FMT_CLONE_MAPPING` | 14.4 | Capacidade que a doc liga a "extended directory statistics" |
| Ajustes do Sistema > Geral > Armazenamento | 13.0 | No 11/12 é "Sobre Este Mac > Armazenamento" |
| Nome "Dados do Sistema"/"System Data" | 12 (B, visto no 12.6) | A doc da Apple diz que o nome antigo era "Outros"/"Other" |
| Apple Intelligence (pastas `UAF_FM_*`) | **15.1** (requisito oficial) | Não se aplica |
| `STMSizeDirectoryWithExcludedChildURLs`, classe `STMVolumeCategory` | SDK 13.0 (B) | O produto não chama `STMSize` |

### 1.3 Sem confirmação fora do 27 (grau B)

- **`fsctl 0xC0184A13`** (tamanho de snapshot) e **`fsctl 0xC1104A71`** (dir-stats; com
  `buf+4 != 0` grava). São do apfs.kext e não existem na xnu. O ioctl 0xC0184A13 foi achado só no
  TimeMachine.framework do 27.
- `fs_snapshot_list` **sem root** e **OBJID = XID** do snapshot.
- Semântica APFS de `PRIVATESIZE`, dos bits `EF_*`, do `CLONEID` de pasta (base da regra de
  enxerto) e se o APFS respeita `ATIME_UPDATES_OFF`.
- CacheDelete: presença em runtime, assinatura de `CacheDeleteCopyItemizedPurgeableSpaceWithInfo`
  e chaves `CACHE_DELETE_*`.
- Linhas de log do painel (`Disk: TOT`, `STMVolumeCategory`, `System size set to`) e do
  mobileassetd (`AssetGC … dirSize`).
- Chaves de `diskutil apfs list -plist` e `diskutil info -plist`; `tmutil status [-X]`, que não é
  documentado; `plutil -convert json` com stdin/stdout.
- Fórmula da tela (`macOS = Σ CapacityInUse − Data`) e detecção de FDA por `~/Library/Safari`.

### 1.4 Bug que vale em todas as versões

**`walk.rs` lê `ATTR_CMN_ERROR` no lugar errado** (A: `man 2 getattrlistbulk` e
`vfs_attrlist.c` em 2782, 7195 e 12377). Numa entrada com erro o kernel devolve só
`RETURNED_ATTRS|NAME|ERROR`, no layout `len | attrset(20) | errno(4) | nameref{off,len}(8) | nome`.
O parser (`coleta/src/walk.rs:229-247`, hoje [src/walk.rs](../../src/walk.rs)) lê o nameref logo depois do attrset e o
erro só depois de FILEID. Por isso usa o errno como offset do nome e o `attr_length` do nameref
(tamanho do nome + 1) como "errno". O histograma `errors` sai com chaves falsas, iguais ao
tamanho do nome. Se o offset falso cair fora do buffer, a entrada some sem ser contada. Não
trava. O `sys.rs snapshots()` não pede ERROR e está certo.

Correção: logo depois dos 5 u32 de `returned_attrs`, fazer
`if ret_cmn & ATTR_CMN_ERROR != 0 { let e = rd32(..) as i32; if e != 0 { contar; continue } }`
e só então ler o nameref. Apagar a leitura atual depois de FILEID.

---

## 2. Tabela por fonte

"Desde" é a primeira tag ou SDK em que a coisa aparece. Os caminhos de código são relativos a
`coleta/src/`. Fallback "nunca 0" quer dizer: `null` no JSON e "não disponível/não medido
(motivo)" no texto.

### 2.1 Syscalls e atributos (xnu)

| API/fonte | Usada em | Disponível desde | Grau | Fallback |
|---|---|---|---|---|
| `getattrlistbulk` (syscall 461) | walk.rs `scan_dir`; por baixo de `fs_snapshot_list` | 10.10: SDK `sys/unistd.h:188` `__OSX_AVAILABLE_STARTING(__MAC_10_10)`; `syscalls.master` de xnu-2782.1.97 l. 729, ausente em 2422.115.4 | A | Nenhum. No 11 o kernel força `FSOPT_ATTR_CMN_EXTENDED`, não valida os bits de options e exige `NAME|RETURNED_ATTRS`, que o código pede |
| `fgetattrlist` + `FSOPT_ATTR_CMN_EXTENDED` + forkattr como `ATTR_CMNEXT_*` | walk.rs `dir_ids`, `attrlist()` | `fgetattrlist` 10.6 (SDK `unistd.h:761`); `ATTR_CMN_RETURNED_ATTRS` 10.6 (xnu-1456.1.26); `FSOPT_ATTR_CMN_EXTENDED` 10.10 (2782.1.97); forkattr lido como CMNEXT 10.13 (4570.1.46) | A | Não pedir CMNEXT além de `PRIVATESIZE|CLONEID|EXT_FLAGS` (já é assim). Bit fora da tabela dá EINVAL no 11 (7195 `vfs_attrlist.c` 557-567 e 681-683). Sem `RETURNED_ATTRS`, um CMNEXT não suportado volta 0, então `dir_ids` dá cloneid 0 e não marca enxerto |
| `ATTR_CMNEXT` VALIDMASK | — | 0x1c (4570, 4903), 0xfc (6153.11.26), 0x3fc (6153.101.6), 0x7fc (7195, 8019, 8796), 0x1ffc (10002 em diante) | A | — |
| `ATTR_CMN_ERROR` | walk.rs | 10.10 (2782.1.97 `attr.h:366`) | A | Corrigir o parser (§1.4) |
| `ATTR_CMNEXT_PRIVATESIZE` | walk.rs:134, 276 | 10.13 (4570.1.46 `attr.h:507`) | A (API) / B (semântica APFS no 11) | Se não voltar em `returned_attrs`, hoje o código usa `alloc` sem avisar. Deve sair "privado: não medido" |
| `ATTR_CMNEXT_CLONEID` | walk.rs (dedup de clone, raiz de enxerto) | 10.15.4 (6153.101.6 `attr.h:534`; ausente em 6153.81.5) | A | Se não voltar, cloneid = 0: não é enxerto, e a proteção por DEVID continua |
| `ATTR_CMNEXT_EXT_FLAGS` | walk.rs | 10.15.4 (6153.101.6 `attr.h:535`) | A (API) / B (bits que o APFS liga) | Sem ele, ext = 0: lê xattr de tudo (mais lento) e não deduplica clone |
| `EF_MAY_SHARE_BLOCKS` 0x1, `EF_NO_XATTRS` 0x2, `EF_IS_PURGEABLE` 0x8, `EF_IS_SPARSE` 0x10 | walk.rs (só `EF_NO_XATTRS`); síntese §2.1 | Header 10.15.4 (6153.101.6 `stat.h:546-550`); se o APFS 11–13 liga: não confirmado | B | Sem `EF_NO_XATTRS`: chamar `listxattr` (atual). Sem `EF_IS_*`: "não disponível", não 0 |
| `EF_SHARES_ALL_BLOCKS` 0x40 | walk.rs:18, 287 | Header **14.0** (10002.1.13 `stat.h:553`; ausente em 8796.141.3); o que o APFS 11–13 devolve: não confirmado | A (header) / B (runtime) | No 11–13, rotular "clones completos: não deduplicados nesta versão". Só a soma **alocada** superestima; o **privado** já desconta blocos compartilhados (B no 11). Nunca "clones: 0" |
| `ATTR_CMNEXT_CLONE_REFCNT` | não usado | 14.0 (10002.1.13 `attr.h:558`) | A | Não pedir no 11–13 (EINVAL) |
| `SF_DATALESS` 0x40000000 | walk.rs:15 | 10.15.0 (6153.11.26 `stat.h:520`) | A (constante) / B (pasta dataless medida só no 27) | Manter a política OFF: um dataless não detectado dá EDEADLK, não download |
| **`ATTR_VOL_SPACEUSED`** | sys.rs:101-110; main.rs:246, 678 | **12.0** (8019.41.5 `attr.h:490`, VALIDMASK 0xF087FFFF; ausente em 7195.50.7.100.1 e 7195.141.2, VALIDMASK 0xF007FFFF) | A | No 11 dá EINVAL (`getvolattrlist_parsetab`, 7195 l. 407-410; lido, não executado). Usar o `CapacityInUse` do volume Data que main.rs já lê do `diskutil apfs list -plist`, rotulando a fonte. A chave existir no 11 é B. Do 12 ao 26 o valor é `f_bsize*f_bused` (mesmo código); no 27 bate com `CapacityInUse` dentro de ~40 KB (A, medido) |
| `setiopolicy_np(MATERIALIZE_DATALESS=3, PROCESS, OFF)` | main.rs:107 (aborta se falhar) | Função 10.5 (SDK `resource.h:593`); iotype 3 e OFF 10.15.0 (6153.11.26) | A | Nenhum; abortar está certo. Detalhe (A, 7195 `vfs_syscalls.c` 10912-10921): a decoração por thread vale mais que a do processo. As threads do sdtally não a ligam. Código de terceiro carregado no processo (CacheDelete) poderia ligar na própria thread (B); não afeta a travessia |
| `setiopolicy_np(ATIME_UPDATES=2, PROCESS, OFF)` | main.rs:108 | 10.14 (4903.221.2; ausente em 4570.71.2) | A (chamada) / B (APFS respeitar, medido só no 27) | Documentar a ressalva no 11–13 até testar |
| `fs_snapshot_list` | sys.rs `snapshots()`; main.rs:281 | 10.12 (SDK `sys/snapshot.h:44`; syscall 518 e `FSOPT_LIST_SNAPSHOT` em 3789.1.32) | A (API) / B (sem root fora do 27) | É um wrapper de `getattrlistbulk` com `FSOPT_LIST_SNAPSHOT`. A xnu só exige a raiz do volume, sem checagem de privilégio no 11 nem no 26 (A). Se exigir root, isso fica no apfs.kext ou no MACF (B). EPERM/EACCES → `diskutil apfs listSnapshots -plist`; se ambos falharem, "não listados (errno)" |
| XID via `ATTR_CMN_OBJID` | sys.rs `snapshots()` | Não confirmado. No caminho genérico, OBJID de 64 bits = `va_linkid` ou `va_fileid` (7195 l. 1252-1275 = 12377 l. 1367-1390, A). O man diz que OBJID é obsoleto e indefinido em volume de 64 bits | B | Pedir também `ATTR_CMN_FILEID` e exigir igualdade. Se divergir, vier 0 ou repetido, usar o `SnapshotXID` do diskutil. Sem XID confiável, não chamar o fsctl daquele snapshot. XID é u64 sem sinal (há XID com bit 63 ligado no Big Sur, B) |
| `fcntl F_LOG2PHYS_EXT` | só experimentos | 10.7 (1699.22.73 `fcntl.h:269`) | A | Fora do produto |
| `listxattr`/`getxattr` + limiar 3804 B | walk.rs `xattr_sizes` | Chamadas 10.6 (1504.15.3) | A (chamadas) / B (3804 é do formato APFS) | Nenhum |
| `pthread_set_qos_class_self_np` | sys.rs `qos_utility` | 10.10 (SDK `pthread/qos.h:153`) | A | O retorno é ignorado; se falhar, a thread fica na QoS padrão |
| `fsctl`/`ffsctl` | sys.rs | `fsctl` sem macro; `ffsctl` 10.6 (SDK `unistd.h:785-786`); syscall 242 em 1504.15.3 | A | — |

### 2.2 fsctl privados do APFS e enxertos

| API/fonte | Usada em | Disponível desde | Grau | Fallback |
|---|---|---|---|---|
| `fsctl 0xC0184A13` (`_IOWR('J',0x13,24)`), tamanho de snapshot por faixa de XID | sys.rs `snap_private`; main.rs:295-307 | Não confirmado. Constante vista só no TimeMachine do 27 (`dyld_info -disassemble`: `mov w1,#0x4a13` + `movk w1,#0xc018`, A, medido). O Utilitário de Disco mostra Private Size desde o 12 (A só UI; B que seja o mesmo ioctl) | B | Erro → "blocos só em snapshot: indisponível nesta versão (errno N)"; o valor fica dentro de "não atribuído" com o motivo e a lista continua. Manter a regra §4.4 (só o volume de boot, `tmutil status` antes) em todas as versões |
| `fsctl 0xC1104A71` (`_IOWR('J',0x71,0x110)`), dir-stats com buffer zerado | sys.rs `dirstats`; main.rs:506-628 | Não confirmado. `apfs.util -S/-M` numa man page com data de ago/2020 (B, cópia de terceiro); no 27 `apfs.util` usa 0xC1104A71 (A, `otool -tv`) | B | **Lista fixa de versões testadas** (hoje só o 27). Fora dela, pastas negadas contadas por número, bytes `null`, "dir-stats não verificado nesta versão". `Ok(0)` = "sem dir-stats mantido", nunca "vazia" |
| Formato dir-stats (`APFS_TYPE_DIR_STATS=10`, `j_dir_stats_val_t`, `INODE_MAINTAIN_DIR_STATS`) | base do 0xC1104A71 | Publicado na Apple File System Reference rev. 2020-06-22, p. 86 (A, lido); desde 2018: B | A | Quais pastas mantêm dir-stats em cada versão: B. É esperado ver mais zeros em versões antigas |
| `graftdmg`/`ungraftdmg` (549/555) | contexto de `graft_roots` | 13.0 (8792.41.9 `syscalls.master:865,871`; ausente em 8020.140.41) | A | No 11/12 o contador de raízes de enxerto tem de dar 0; se não der, anomalia |
| Tipo `GRAFTDMG_CRYPTEX_MOBILE_ASSET` (8) / `_WITH_CODE` (9) | causa da contagem dupla que walk.rs evita | 8: 15.0 (11215.1.10 `mount.h:1507`; ausente em 10063.141.1); 9: 26.1 (12377.41.6) | A | Em 11–14 esperar 0 raízes |
| `FSIOC_GET_GRAFT_INFO` (`'J',102`) | só experimento (root) | **Anterior ao 14.4, versão não confirmada**: entra no header da xnu no 10063.101.15 com o comentário "This used to be defined in APFS" | B | Não depender. Só auditoria manual com sudo |
| Regra "pasta aberta com CLONEID ≠ FILEID = raiz de enxerto" | walk.rs `dir_ids` + `scan_dir` | Atributo 10.15.4 (A); a regra foi validada só no 27 (7/7, 0 falso positivo em 377 mil pastas) | B | **Ligar só com versão ≥ 15.0 e `VOL_CAP_FMT_CLONE_MAPPING` anunciado.** Trava: raiz fora de `*/AssetsV2/*/.AssetData` ou mais de N raízes → desligar a regra e marcar "resultado suspeito" |
| `diskutil apfs listSnapshots -plist` (`SnapshotXID`) | fallback proposto | Chave no plist: não confirmada antes do 27. A linha "XID:" do texto aparece no Big Sur (B, gist de terceiro) | B | plist → texto "XID:" → sem XID e sem tamanho, com o motivo |

### 2.3 Frameworks privados, logs e painel

| API/fonte | Usada em | Disponível desde | Grau | Fallback |
|---|---|---|---|---|
| `CacheDeleteCopyItemizedPurgeableSpaceWithInfo` (dlopen/dlsym) | sys.rs:249-300; main.rs:337 | Símbolo no stub do SDK 10.10 e em todos os SDKs 11.0–27 (27 local = A; os outros vêm de espelhos de terceiros = B). Runtime e assinatura só no 27 | B | Hoje o dlopen usa o symlink sem versão `.../CacheDelete.framework/CacheDelete`, que está **pendente em disco** no 27, porque o binário só existe no shared cache (A, `ls`; nota do Big Sur 11.0.1). Trocar pelo install-name `.../Versions/A/CacheDelete`. Falha → "purgeable: indisponível (motivo)" |
| Chaves `CACHE_DELETE_*` | sys.rs:261-262, 317; main.rs:350-359 | 85 strings no binário do 27 (A); 11/12 não confirmados | B | Cada chave é opcional. sys.rs:317 descarta valores que não são CFNumber, então "o volume pedido foi respeitado?" não é detectável hoje. Guardar os nomes das chaves recebidas. Dicionário sem serviços → hoje sai "0,00 GB" (main.rs:769); deve sair "não informado" |
| Purgeable total por `kCFURLVolumeAvailableCapacityForImportantUsageKey` | main.rs:369 diz "pulado: exige Foundation" | 10.13 (`CFURL.h` do SDK, `API_AVAILABLE(macos(10.13))`) | A | É CoreFoundation, que já está linkado: o comentário do código está errado. Serve de total e de verificação da soma do CacheDelete |
| `StorageManagementService` + linha `Disk: TOT/USE/AVL/PRG/OTH/SYS` | main.rs:145-152, 556-565 | Strings no 27 (A). Binário visto no 15.3.2 (B). 11–14 não confirmados | B | Sem linha → "sem registro do painel no log retido". **Retenção medida < ~21 h** (0 linhas em 3 dias em 29/09, A): os textos "últimos 7 dias" em main.rs:640 e :754 estão errados |
| Linha `<STMVolumeCategory: … System Data N>` | main.rs:150, 566-580 | Classe exportada a partir do SDK 13.0 (B, espelhos); linha vista só no 27 | B | O parser compara com `"System Data"`; no 11 o nome seria "Other" (B). Usar o id da categoria (`toks[1]`) |
| `Storage.appex` + linha Debug `System size set to` | main.rs:154-158, 583-587 | Ajustes do Sistema 13.0 (A, Apple Support 102624); appex visto no 13.5.2 (B); linha vista só no 27 | B | A linha é Debug e quase nunca fica retida. O caminho principal deve ser `macos_reproduzido` |
| Fórmula `macOS = Σ CapacityInUse − Data` | main.rs:205-217 | Validada ao byte só no 27 | B | Bug de escopo (A pelo código): main.rs:215 soma volumes sem papel (criados pelo usuário) e o System/Preboot de outra instalação. O painel mostra esses à parte (`DISK_OTHER_VOLS`). Excluir volumes sem papel e os de outro `APFSVolumeGroupID`. Fora das versões validadas: "reprodução não validada nesta versão" |
| Log do mobileassetd `AssetGC … dirSize` | main.rs:377-385, 596-609 | Só no 27 (strings em MobileAssetDaemon, A) | B | Irrelevante abaixo do 15.1. Sem linhas → "enxertos .AssetData não identificados" |
| `/System/Library/AssetsV2` (firmlink) | main.rs:17-18, 610-631 | Firmlink no 27 (A); desde o 10.15 segundo Oakley (B) | B | main.rs:624 ignora erro de `read_dir` em silêncio: registrar o erro |
| Apple Intelligence (`UAF_FM_GenerativeModels`, `UAF_FM_Visual`) | main.rs:610-631 | 15.1 (Apple Support 121115, A). `Visual` só entra nas strings do Storage.appex no 15.4 (B, ipsw-diffs) | B | Abaixo do 15.1 → "não se aplica nesta versão". Hoje main.rs:620 faz `unwrap_or(0)` e mostra 0 (também no resumo, main.rs:756). No 15.1–15.3, avisar que a tela possivelmente soma só GenerativeModels |
| Caminho do painel (texto de ajuda) | main.rs:640, 754 | 13+: Ajustes do Sistema > Geral > Armazenamento; ≤ 12: Sobre Este Mac > Armazenamento (A, Apple Support 102624 en/pt-br) | A | Texto por versão |
| `log show --style ndjson` | main.rs:147-158, 379-384 | Man do 27 (A); em uso no 10.15 em 05/2020 (B). Logo, anterior ao 11 | B | Estilo inválido → tentar `json`. Saída vazia não significa "painel não aberto". O registro final `{"count":0,"finished":1}` é tratado (A, medido) |

### 2.4 Ferramentas de linha de comando e sysctl

| API/fonte | Usada em | Disponível desde | Grau | Fallback |
|---|---|---|---|---|
| `diskutil apfs list -plist` (`ContainerReference`, `DesignatedPhysicalStore`, `Volumes`, `Roles`, `CapacityInUse`, …) | main.rs:186-240 | Chaves medidas só no 27 (A); lidas pelo asahi-installer, que exige 12.3+ (B) | B | **main.rs:213 `unwrap_or(0)` → chave ausente vira 0 na soma "macOS".** Trocar por Option. Sem `DesignatedPhysicalStore` → `PhysicalStores[0].DeviceIdentifier` |
| Layout de 3 containers (ISC, Recovery, principal), papéis Update/xART/Hardware | main.rs:205-218 | 11.0 em Apple Silicon (B, Oakley 2021); papéis na APFS Reference 2020 (A, citado) | B | O código só depende do papel `Data`. Listar o detalhe por papel |
| `diskutil info -plist` (`DeviceIdentifier`, `Internal`, `TotalSize`) | main.rs:175-201 | Só no 27 (A); `Internal` em 12.3+ (B, asahi) | B | Já trata ausência (desconhecido/null/erro) |
| `plutil -convert json -o - -` | sys.rs:190-206 `plist_json` | plutil 10.2 (A, man). JSON + stdin/stdout no 11: B. `-type/-expect/raw` só 12+ (A, man 27; não usar) | B | sys.rs:202-205 ignora o status e manda stderr para /dev/null: um plist com `<date>`/`<data>` vira "EOF". Checar o status e dizer a causa |
| `tmutil status` / `status -X` | main.rs:264-270 (regra §4.4) | Não documentado (ausente no man e no `tmutil help` do 27, A) | B | Preferir `-X` + plist; sem `Running` → "estado do backup desconhecido" (nunca "parado") |
| `tmutil destinationinfo -X` | main.rs:271-279 | Man do 27, página datada de 2015 (A no 27; B forte no 11) | B | Já trata erro |
| `lsof -nP +L1 -F ctsDi` | main.rs:393-455 | 4.91 do 11.0.1 ao 26 (A, tags lsof-68…76) | A | Já trata (Null + erro) |
| `scutil --get …` | main.rs:133-138 | Mac OS X Public Beta (A, man) | A | `.ok()` |
| `id -F` | main.rs:132 | ≤ 11.0.1 (A, shell_cmds-216.40.4 `id.c:116`) | A | String vazia |
| `hw.perflevel0.physicalcpu` | main.rs:171 | 12.0 (8019.41.5 `kern_mib.c:857-861`; ausente em 7195.x) | A | `unwrap_or(4)` basta: no 11 só roda M1 (4P+4E); M1 Pro/Max saíram com o 12 (B). Registrar no JSON que o valor veio do padrão |
| `hw.model`, `hw.memsize` | main.rs:696, 698 | ≤ 11.0.1 (7195 `kern_mib.c:773`, `:723`) | A | null |
| `machdep.cpu.brand_string` | main.rs:697 | 11.0.1 dá "Apple processor"; nome real a partir de 11.1 (7195.60.75, lido do device tree) | A | Mostrar como veio, junto com `hw.model`; não deduzir o chip |
| `sw_vers` / `kern.osproductversion` | main.rs:688-689 | Com `SYSTEM_VERSION_COMPAT=1` devolvem "10.16" (A, 7195 `kern_sysctl.c`) | A | Tratar "10.16" como 11. Preferir `sysctl kern.osproductversion`, que dispensa subprocesso |
| `SUDO_USER`/`SUDO_UID`/`SUDO_GID` | main.rs:102, 122-127, 728-735 | man sudo do 27 (sudo 1.9.17p2, A); 11: B | B | — |
| Detecção de FDA por `read_dir(~/Library/Safari)` | main.rs:163-169 | Medido só no 27 (Frente D) | B | Testar em ordem `/Library/Application Support/com.apple.TCC`, `~/Library/Safari`, `~/Library/Mail`. EPERM → sem FDA; OK → com FDA; só ENOENT → indeterminado. Nunca `stat`. O `TCC.db` do usuário mudou de lugar no 27 (A) |
| sudo sem FDA | main.rs:78-83 | Regra do TCC descrita pelo DTS da Apple (fórum oficial, B por versão) | B | Avisar "sudo não substitui Acesso Total ao Disco" |

---

## 3. Mudanças de comportamento por versão que afetam a conta

| Versão | Mudança | Efeito no sdtally | Grau |
|---|---|---|---|
| 11.0.x | `machdep.cpu.brand_string` = "Apple processor" | Relatório sem nome do chip | A |
| 11 | Sem `ATTR_VOL_SPACEUSED` | Topo da conta vem do `CapacityInUse` do diskutil (outra fonte, rotular) | A |
| 11 | Categoria chamada "Outros"/"Other" e "Sistema"/"System", na janela Sobre Este Mac | Rótulo: "Outros (chamado Dados do Sistema a partir do macOS 12)". O parser do log não pode exigir "System Data" | A (nome antigo pela doc) / B (versão da troca) |
| 11, 12 | Sem `hw.perflevel0` | Threads = 4 | A |
| 11, 12 | Painel em "Sobre Este Mac > Armazenamento"; `StorageManagementService` e linhas de log não confirmados | O número da tela pode não existir no log | A (caminho) / B (log) |
| 11–12 | Sem enxertos (sem `graftdmg`) | Raízes de enxerto = 0; se aparecer alguma, é anomalia | A |
| 11–13 | Sem `EF_SHARES_ALL_BLOCKS` no header | Clone completo contado em dobro na soma alocada; o privado segue válido | A (header) / B (efeito) |
| 12 | Nome "Dados do Sistema"/"System Data" e "macOS" (visto no 12.6) | — | B |
| 12 | `ATTR_VOL_SPACEUSED` = `f_bsize*f_bused`, mesmo código até o 26 | Topo da conta comparável do 12 ao 27 | A |
| 13.0 | Ajustes do Sistema > Geral > Armazenamento; `graftdmg` (BOOT/PREBOOT/DOWNLEVEL) | Texto de ajuda por versão | A |
| 13.x (xnu-8796.141.3) | Tipos de enxerto `PDI_NONCE` e `EFFECTIVE_AP` (não no 14.0, como a pesquisa dizia) | Os cryptex do sistema ficam no Preboot no 27 (medido); no 13/14 não foi verificado | A |
| 14.0 | `EF_SHARES_ALL_BLOCKS`, `ATTR_CMNEXT_CLONE_REFCNT`; CMNEXT VALIDMASK 0x1ffc | A dedup de clone passa a funcionar | A |
| 14.4 | `VOL_CAP_FMT_CLONE_MAPPING` e `FSIOC_GET_GRAFT_INFO` entram na xnu | Condição proposta para ligar a regra de enxerto | A |
| 15.0 | Enxerto `GRAFTDMG_CRYPTEX_MOBILE_ASSET` (`.AssetData` em AssetsV2) | A contagem dupla que walk.rs evita só existe a partir daqui | A |
| 15.1 | Apple Intelligence; o (i) de macOS mostra o espaço | Linha "Apple Intelligence" só a partir daqui | A (requisito) / B (UI) |
| 15.4 | Assets UAF sob SIP; `Visual` entra na soma da tela | Pastas negadas, medidas por dir-stats (sem confirmação) | B |
| 26.0 | `FSCTL_UNGRAFT_NOFORCE`, tipo `AUTH_ENV_GENERIC`. Os flags de graft já eram u64 desde o 13; no 26 só mudam os literais | Nenhum | A |
| 26.1 | `GRAFTDMG_CRYPTEX_MOBILE_ASSET_WITH_CODE` | Nenhum (a regra de pasta cobre) | A |
| 27 | `TCC.db` do usuário saiu de `~/Library/Application Support/com.apple.TCC` | A detecção de FDA não pode depender dele | A |
| 27 (medido) | Linhas Default de `com.apple.storage` retidas por < ~21 h | Textos "últimos 7 dias" errados | A |
| Todas | Pasta dataless: constante desde 10.15; pasta dataless (não só arquivo) medida só no 27 | A política OFF protege em qualquer caso | A / B |
| Todas | Layout de erro do `getattrlistbulk` sem mudança de 10.10 a 26 | O bug do §1.4 vale em todas | A |

---

## 4. Plano de teste

### 4.1 CI (GitHub Actions, runners reais em 29/09/2026, A: README e readme da imagem via `gh api`)

| Label | macOS | Situação |
|---|---|---|
| `macos-14`, `macos-14-xlarge` | 14 arm64 | Descontinuado; **sem suporte a partir de 2/11/2026** |
| `macos-15`, `macos-15-xlarge` | 15 arm64 | GA |
| `macos-26`, `macos-latest`, `macos-26-xlarge` | 26 arm64 | GA |
| `xcode-27`, `xcode-27-xlarge` | **27.0 (26A428)** arm64 | Preview |

Não há runner 11, 12 ou 13 em arm64. O PLANO §7 diz "arm64 a partir do 14", o que só vale até
2/11/2026.

O que o CI cobre (15, 26, 27, e o 14 até 2/11): build, testes do parser puro (buffers
sintéticos, inclusive entrada com `ATTR_CMN_ERROR`), a fixture APFS feita com `hdiutil` (se o
runner permitir anexar imagem: B, testar) e um passo de sondagem que registra
`sw_vers`, `otool -l | grep minos`, o `diskutil apfs list -plist` convertido, `tmutil status -X`,
`ls ~/Library/Safari; echo $?`, `sdc-coleta --json` e os errnos de `vol_space_used`,
`snap_private`, `dirstats` e CacheDelete. Os runners são VMs: o layout de containers e o FDA
podem diferir de um Mac real (B). O CI não cobre o log do painel, porque ninguém abre a tela.

### 4.2 VM no M4 (Virtualization.framework / Tart)

- Host: as APIs `VZMac*` existem desde 12.0 (A, SDK `VZMacOSRestoreImage.h:28`,
  `VZMacPlatformConfiguration.h:36`, `VZMacOSInstaller.h:85`).
- Convidado: macOS 12 ou mais novo; **Big Sur não roda como convidado** (B, Oakley 2026-04-29).
  Limite de 2 VMs macOS simultâneas (B).
- Versões a rodar: **12.x, 13.x, 14.0–14.3, 14.4+, 15.0, 15.1–15.3, 15.4+** (as fronteiras da §3).
  O 26 e o 27 já estão cobertos pelo CI e pela máquina local.
- Exemplo (B, doc do Tart): `tart clone ghcr.io/cirruslabs/macos-monterey-base:latest m12 && tart run m12`.
  Registrar o layout de containers da VM, que pode não ser o de um Mac real.
- Instalar o Tart é decisão do dono (não foi feito aqui).

### 4.3 Só hardware real ou relatórios dos amigos

- **macOS 11**: não há VM. Opções: M1 emprestado e restaurado para 11 (IPSW assinado: não
  confirmado), EC2 `mac2.metal` se ainda houver AMI arm64 do 11 (não confirmado; conferir com
  `aws ec2 describe-images --owners amazon --filters 'Name=name,Values=amzn-ec2-macos-11*' 'Name=architecture,Values=arm64_mac'`),
  ou um amigo com M1 no 11. Sem isso, o 11 fica "sem teste". "Deve funcionar sem teste" não
  vale: `ATTR_VOL_SPACEUSED` falha com certeza no 11.
- Painel e log (`Disk: TOT`, `STMVolumeCategory`, `System size set to`), fórmula da tela e
  Apple Intelligence exigem alguém abrir a tela. Isso só acontece em Mac real ou numa VM usada à
  mão.
- Volumes extras no container e dual boot: só em Mac de amigo.

### 4.4 Teste concreto por item não confirmado

Todos rodam sem sudo, salvo indicação, e só leem.

| Item | Versões | Teste | Esperado |
|---|---|---|---|
| `ATTR_VOL_SPACEUSED` no 11 | 11 | `sdc-coleta --json`; ver o erro de `vol_space_used`; `diskutil apfs list -plist \| plutil -convert json -o - - \| grep -c CapacityInUse` | errno 22 (EINVAL); chave presente |
| Chaves do `diskutil apfs list -plist` | 11, 12 | `diskutil apfs list -plist \| plutil -convert json -o - - \| python3 -c 'import json,sys;c=json.load(sys.stdin)["Containers"];print(sorted(c[0]));print(sorted(c[0]["Volumes"][0]))'` | `ContainerReference`, `DesignatedPhysicalStore`, `Volumes`, `Roles`, `CapacityInUse` |
| `plutil` JSON | 11 | `diskutil apfs list -plist \| plutil -convert json -o /dev/null -; echo $?` (idem `diskutil info -plist disk0` e `tmutil destinationinfo -X`) | 0 |
| `fsctl 0xC0184A13` | 11–15 | Com `tmutil status` → `Running = 0`: `cc -O2 -o /tmp/snapsize research/experimentos/frenteB/snapsize.c && /tmp/snapsize /System/Volumes/Data`; no 12+ comparar `{x,x}` com Private Size do Utilitário de Disco. Sem executar: `dyld_info -disassemble /System/Library/PrivateFrameworks/TimeMachine.framework/Versions/A/TimeMachine \| grep -B1 '#0xc018' \| grep 0x4a13` | Valor igual à UI, ou ENOTTY/EINVAL registrado |
| `fsctl 0xC1104A71` | 11–15 | (1) `strings -a /System/Library/Filesystems/apfs.fs/Contents/Resources/apfs.util \| grep -i dir-stats` e `otool -tv` do mesmo `\| grep -B1 '#0xc110'`; (2) `apfs.util -S <pasta>` e `research/experimentos/frenteH/dirstats.c` na mesma pasta; (3) gen-count igual antes e depois. **Nunca `-M`** | Bytes e descendentes iguais; gen-count inalterado |
| Pastas com dir-stats e capacidade | 11–15 | `dirstats.c` nas pastas negadas (contar 0 vs >0); ler `ATTR_VOL_CAPABILITIES` de `/System/Volumes/Data` (bit 0x04000000) | Mais zeros antes do 14.4 |
| `fs_snapshot_list` sem root + XID | 11–15 | Programa que pede `NAME|OBJID|FILEID` em `/System/Volumes/Data`; comparar com `diskutil apfs listSnapshots -plist` (ou o texto "XID:") | Lista sem root; OBJID = FILEID = SnapshotXID |
| `SnapshotXID` no plist | 11–15 | `diskutil apfs listSnapshots -plist /System/Volumes/Data \| grep -c SnapshotXID` (timeout 30 s) | = nº de snapshots |
| CLONEID de pasta comum | 11–15 | `research/experimentos/frenteL/ids.c` + `open(O_DIRECTORY)`+`fgetattrlist(FILEID, CLONEID, FSOPT_ATTR_CMN_EXTENDED)` em até 10 mil pastas de `~/Library` | cid == id ou 0 em todas (exceto `.AssetData` no 15+) |
| `FSIOC_GET_GRAFT_INFO` antes do 14.4 | 13.x, 14.0–14.3 | Pelo dono com sudo: `graftinfo.c` em `/System/Volumes/Data` | ENOTTY = não existe; outra resposta = anterior ao 14.4 |
| `EF_SHARES_ALL_BLOCKS` e `EF_*` | 11, 13 | Numa imagem APFS descartável (`hdiutil`): `cp -c grande.bin clone.bin`; ler `EXT_FLAGS`/`CLONEID` dos dois; arquivo sem xattr (0x2), depois de `xattr -w k v f` (sem 0x2), `mkfile -n 1g` (0x10) | 0x1 sem 0x40 no 11–13; cloneid igual |
| `PRIVATESIZE` | 11 | Na mesma imagem: `cp -c` de 10 MB; ler `PRIVATESIZE` dos dois | ~0 nos dois |
| `ATIME_UPDATES_OFF` | 11, 13 | `stat -f %a` numa pasta de teste antes e depois do `sdc-coleta` | Inalterado |
| Dataless | 11 | iCloud Drive com "Otimizar armazenamento": `ls -lO` num item evitado | Flag `dataless` |
| CacheDelete | 11, 12 | `/usr/bin/python3 -c "import ctypes;[print(p,hasattr(ctypes.CDLL(p),'CacheDeleteCopyItemizedPurgeableSpaceWithInfo')) for p in ['/System/Library/PrivateFrameworks/CacheDelete.framework/CacheDelete','/System/Library/PrivateFrameworks/CacheDelete.framework/Versions/A/CacheDelete']]"`; `sudo sdc-coleta` e comparar `soma_servicos` com importantUsage − livre | True nos dois; soma igual ao byte |
| Log do painel | 11–15 | Abrir o painel, esperar e rodar `/usr/bin/log show --info --last 10m --predicate 'subsystem == "com.apple.storage"' \| grep -E 'Disk: TOT\|STMVolumeCategory'` e `log show --info --debug --last 10m --predicate 'subsystem == "com.apple.settings.Storage"' \| grep 'System size'`; repetir 24 h e 48 h depois | Existência e retenção por versão |
| `ndjson` | 11 | `/usr/bin/log show --last 1m --style ndjson \| head -2`; com usuário não-admin, `log show --last 1m \| head -2` | Aceito; ver se não-admin é negado |
| Nome da categoria | 11 | `plutil -p '/System/Applications/Utilities/System Information.app/Contents/Resources/pt_BR.lproj/SPInfo.strings' \| grep -E 'DISK_OTHER\|DISK_SYSTEM'` (ou o `.loctable`), e en.lproj | "Outros"/"Other" |
| Fórmula da tela | 11–15 | Anotar Usado, macOS/Sistema e Dados do Sistema/Outros na tela; logo em seguida `diskutil apfs list -plist` e `sudo sdc-coleta`; comparar | macOS = Σ CapacityInUse − Data |
| Apple Intelligence | 15.1–15.3 | `ls /System/Library/AssetsV2 \| grep UAF_FM`; `strings -a .../Storage.appex/Contents/MacOS/Storage \| grep UAF_FM`; comparar o (i) com dir-stats(GM) e GM+Visual | Descobrir a fórmula por versão |
| `tmutil status -X` | 11, 12 | Com e sem backup: `tmutil status; tmutil status -X \| plutil -convert json -o - -; echo $?` | Chave `Running` |
| FDA | 11, 14 | Terminal sem e com FDA: `for p in '/Library/Application Support/com.apple.TCC' ~/Library/Safari ~/Library/Mail; do ls "$p" >/dev/null 2>&1; echo "$p $?"; done` | 1 (EPERM) sem FDA; 0 com |
| `SYSTEM_VERSION_COMPAT` | 11 | `SYSTEM_VERSION_COMPAT=1 sw_vers -productVersion` | 10.16 |

---

## 5. Impacto no PLANO.md

### Fase 0

1. **Corrigir o bug de `ATTR_CMN_ERROR`** no parser e cobri-lo com um buffer sintético de
   entrada com erro no teste do parser puro.
2. **Uma checagem de versão e capacidade, feita uma vez** (`kern.osproductversion`, com "10.16" → 11,
   e `ATTR_VOL_CAPABILITIES`), consultada por cada fonte. Cada linha tem três estados: medida,
   "não disponível nesta versão" e "não medida (motivo)". Nunca 0.
3. **Trocar os `unwrap_or(0)` que viram número falso** por Option: main.rs:213 (`CapacityInUse`),
   :620 (Apple Intelligence), :769 (purgeable sem serviços), e registrar o erro do `read_dir` em :624.
   `plist_json` passa a checar o status do plutil.
4. **dlopen do CacheDelete pelo install-name** `.../Versions/A/CacheDelete`.
5. **CI**: `macos-15`, `macos-26` e `xcode-27` (macOS 27), com `macos-14` até 2/11/2026, mais o
   passo de sondagem da §4.1 e a checagem `minos 11.0` (fixar `MACOSX_DEPLOYMENT_TARGET=11.0`).
6. Corrigir o PLANO §7: "CI a partir do 14" vale até 2/11/2026; depois fica 15, 26 e 27.

### Fase 1

1. **Topo da conta no 11**: `CapacityInUse` do Data como fallback de `ATTR_VOL_SPACEUSED`, com a
   fonte na linha.
2. **fsctl privados (0xC0184A13, 0xC1104A71) só nas versões da lista testada** (hoje só o 27).
   Fora dela, "não verificado nesta versão".
3. **Regra de enxerto só no 15+ com `VOL_CAP_FMT_CLONE_MAPPING`**, com a trava de sanidade.
4. **XID**: pedir `ATTR_CMN_FILEID` junto e cruzar com o `SnapshotXID` do diskutil.
5. **Clones no 11–13**: rotular a soma alocada como limite superior; o privado continua valendo.
6. **Painel**: texto e caminho por versão ("Sobre Este Mac" ≤ 12, "Outros" no 11); corrigir
   "últimos 7 dias" (main.rs:640, :754); parser pelo id da categoria; `macos_reproduzido` como
   caminho principal, excluindo volumes sem papel e de outro grupo de volumes.
7. **Purgeable total por `kCFURLVolumeAvailableCapacityForImportantUsageKey`** (CoreFoundation),
   que verifica a soma do CacheDelete.
8. **FDA** pela ordem TCC do sistema → Safari → Mail.
9. **Critério de pronto 5** passa a ser: matriz publicada de versão × fonte (medida, não
   disponível, não testada), com 12–27 rodados em VM ou CI sem erro fatal. O 11 fica "sem teste"
   até haver hardware ou um amigo com M1 nessa versão.

---

## 6. Perguntas abertas

1. O apfs.kext do 11 e do 12 aceita 0xC0184A13? Mesmo layout de 24 B?
2. O layout de 0x110 B e o "zerado = get" do 0xC1104A71 valem do 11 ao 26? Em que versão o
   dir-stats passou a ser mantido em quase toda pasta do Data?
3. `fs_snapshot_list` sem root e OBJID = XID fora do 27?
4. Que CLONEID o APFS devolve para pasta comum no 11–14.3? Se não for 0 nem igual ao fileid, a
   regra de enxerto sem a trava pularia árvores inteiras.
5. O APFS do 11–13 liga `EF_SHARES_ALL_BLOCKS` (0x40) sem o header declarar? Respeita
   `ATIME_UPDATES_OFF`?
6. `StorageManagementService` e as linhas `Disk: TOT`/`STMVolumeCategory` existem no 11–14? No
   11/12, o System Information.app calcula a barra por conta própria? Existe algum log
   equivalente?
7. Qual é a fórmula da tela no 11 (categoria "Sistema" inclui Preboot?) e no 15.1–15.3 (Apple
   Intelligence só com GenerativeModels?)?
8. Existe enxerto no volume Data no 13/14 (tipos `PDI_NONCE`/`EFFECTIVE_AP`)?
9. Chaves de `diskutil`/`tmutil status -X` e JSON do `plutil` no 11 e no 12.
10. O 11 é testável? Há AMI arm64 do 11 no EC2 ou IPSW do 11 ainda assinado? Se não houver,
    aceitar o 11 como "sem teste" ou subir o mínimo testado para o 12?
11. `log show` por usuário não-admin (`--sem-sudo`) é negado em alguma versão?
12. O volume Update do container principal existe em Macs novos no 12–26? Oakley (2024) diz que
    não; no 27 ele existe.

Nota de processo: para ler `vfs_attrlist.c` e `kern_resource.c`, a verificação salvou cópias
temporárias no scratchpad da sessão (`/private/tmp/claude-501/...`), fora de `research/frentes/`.
Não foram apagadas porque as regras da tarefa proíbem apagar.

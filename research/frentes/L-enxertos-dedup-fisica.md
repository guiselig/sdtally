# Frente L — enxertos (graft) e deduplicação física exata — 2026-09-28

Pergunta: detectar com exatidão o conteúdo enxertado que a varredura conta em dobro (Frente H,
§2.4) e trocar a deduplicação por `cloneid` por uma deduplicação por bloco físico, que resolve
também o clone parcial.

Máquina: macOS 27.0, `xnu-13432.1.9`, Apple Silicon, Data = `disk3s5`. Sem sudo; FDA herdado do
Claude Code. Time Machine parado (`Running = 0`) em todas as varreduras. Medições entre 11:20 e 11:50.

Confiança: **A** medido aqui ou código lido; **M** fonte secundária ou medido com ressalva;
**B** inferência.

Código em `research/experimentos/frenteL/` (tudo só leitura, nada pede root):

| Arquivo | Faz |
|---|---|
| `src/main.rs` (`frentel`) | Travessia do `walk_par` + detecção de graft + união de faixas físicas (`F_LOG2PHYS_EXT`) |
| `graft_check.py` | Prova que os blocos do conteúdo enxertado estão dentro do `*_Cryptex.dmg` do asset |
| `graftinfo.c` | `fsctl(FSIOC_GET_GRAFT_INFO)`: lista os grafts do volume (exige root; comando para o dono) |
| `ids.c` | `fileid`, `parentid`, `cloneid`, `clone_refcnt`, `ext_flags`, privado e alocado de um caminho |

Compilar: `CARGO_TARGET_DIR=<scratch>/target cargo build --release` e
`cc -O2 -o graftinfo graftinfo.c`, `cc -O2 -o ids ids.c`. O `walk_par.rs` do probe não foi alterado.

---

## 1. Resultado

1. **Detecção exata de graft sem root:** a pasta enxertada, **aberta**, devolve
   `ATTR_CMNEXT_CLONEID = 2` (a raiz do fs do cryptex) e `fileid` do Data. Pasta comum tem
   `cloneid == fileid`. Uma `fgetattrlist` por pasta aberta. Achou as 7 raízes de `AssetsV2`,
   1.120 arquivos, **2,215 GiB**. Todos os blocos físicos delas estão dentro do `*_Cryptex.dmg`
   vizinho (2.373.746.688 B dentro, 0 fora). (A)
2. **Dedup física exata:** união das faixas de `F_LOG2PHYS_EXT` dos candidatos. Na imagem de
   teste, a soma bate **ao byte** com o esperado. No Data: 620 mil candidatos, **+8,9 s** sobre
   a travessia de 10,5 s. (A)
3. **Os 431 mil "falsos positivos" não compartilham nada:** 430.181 arquivos, 32,21 GiB, com
   físico = alocado ao byte. A regra "cloneid ≠ fileid sem MAY_SHARE" do `walk_par` tira
   **~32 GiB** de arquivos reais da árvore. **Tem de sair do `walk_par`.** (A)
4. **O que muda na árvore do Data (sem root):** −2,215 GiB de enxertos, −0,525 GiB de clones
   parciais, −0,014 GiB de clones completos que o `cloneid` não pegou, 0 de "salvamentos
   atômicos". Total: **−2,75 GiB** em relação a uma varredura que conta grafts e deduplica só
   clone completo. (A)

## 2. Como o sistema sabe onde há graft

### 2.1 Mecanismo (A, código e cabeçalho)

- Syscalls `graftdmg` (549) e `ungraftdmg` (555) em `<sys/syscall.h>` do SDK. Em `<sys/mount.h>`
  o enum `graftdmg_type` tem `GRAFTDMG_CRYPTEX_MOBILE_ASSET = 8` e `..._WITH_CODE = 9`: o
  MobileAsset enxerta cryptex. O kernel tem o entitlement `com.apple.private.vfs.graftdmg`
  (`strings /System/Library/Kernels/kernel.release.t6000`).
- Na xnu (`bsd/sys/fsctl.h`, `apple-oss-distributions/xnu`, branch main):
  `FSIOC_GRAFT_FS _IOW('A', 22)` e `FSIOC_UNGRAFT_FS` (só kernel), e
  **`FSIOC_GET_GRAFT_INFO _IOWR('J', 102, fsioc_get_graft_info_t)`** (visível ao usuário).
  Cada registro traz `gi_graft_file` (inode do DMG), `gi_graft_dir` (inode da pasta),
  `gi_inum_base` e `gi_inum_len` (faixa de inodes do Data dada ao graft) e `gi_graft_flags`.
  Com um caminho, o campo `ggi_is_in_graft` diz se aquele inode está num graft.
  É a resposta oficial e exata.
- **Sem root dá `EPERM`**, em qualquer caminho, inclusive `~` (`graftinfo`, medido). Por isso
  não serve para o produto sem root. Fica como prova para o dono (§6).

### 2.2 Ferramentas do `apfs.fs` (A)

- `apfs.util`: nenhuma opção lista grafts (ajuda completa lida; só `-S`/`-G`/`-Z` e afins).
- `apfs_prepare_cryptex`: sela um volume (`-p`, `-M`), não lista. Não foi rodado.
- `apfs_diskimage_map -o json <dmg>`: rodado numa **cópia** do DMG do PKITrustStore (hash igual
  antes e depois): devolveu só o nome do volume (`Creedence11M6270.SECUREPKITRUSTSTORE..._Cryptex`).
  Os campos `files`/`extents` existem nas strings, mas não vieram para esse DMG.
- `cryptexctl` não existe no macOS 27; há `/usr/libexec/cryptexd`, `CryptexKit.framework` e
  `CryptexServer.framework`. O log do `mobileassetd` tem `no grafting required` nos pedidos,
  sem a lista de pastas.

### 2.3 O que o `getattrlist` mostra (A, medido com `ids`)

PKITrustStore (`.../6dd55b0d....asset`):

| Objeto | fileid | cloneid | ext_flags | privado |
|---|---|---|---|---|
| pasta do asset | 1871148 | 1871148 | 0 | — |
| `.AssetData` (aberta pelo caminho) | 1871472 | **2** | 0x2 | — |
| `.AssetData/AnalyticsSamplingRates.plist` | 1871489 | 16 | 0x2 | = alocado |
| `.AssetData/manifest.data` | 1871506 | 33 | 0x2 | = alocado |
| `.AssetData/System` (pasta) | 1871495 | 22 | 0x2 | — |
| `AssetData/Restore/*_Cryptex.dmg` | 1871166 | 1871166 | 0 | 0 (preso em snapshot) |

- Dentro do graft, `fileid − cloneid` é constante (1871473): o `cloneid` é o id interno do fs
  enxertado (começa em 16) e o `fileid` é esse id somado à base da faixa (`gi_inum_base`). (A
  para a constância, M para a ligação com `gi_inum_base`, que não li sem root.)
- `st_dev`, `f_mntonname`, `ATTR_CMNEXT_REALDEVID/REALFSID` e `ATTR_DIR_MOUNTSTATUS` são os do
  Data: nada disso revela o graft. (A, `realdev`)
- **Armadilha:** o `getattrlistbulk` da pasta do asset devolve a `.AssetData` **coberta** (a do
  host, `cloneid == fileid`). Só a pasta aberta (ou `getattrlist` pelo caminho) mostra
  `cloneid = 2`. A primeira versão do protótipo só pegava as subpastas. (A)
- **Armadilha 2:** a raiz do volume Data também tem `cloneid = 2`, com `fileid` sintético
  `0x0FFFFFFF00000002`. A regra não vale para a raiz da travessia. (A)

### 2.4 Regra para o produto (A nesta máquina; M como regra geral)

```
ao abrir uma pasta (profundidade > 0): fgetattrlist(fd, FILEID | CMNEXT_CLONEID)
  cloneid != 0 && cloneid != fileid  ->  raiz de graft: não descer, reportar à parte
na listagem: pasta com cloneid != fileid  ->  também é graft (defesa extra)
```

Pasta não tem clone no APFS: em 377 mil pastas do Data, só as 7 `.AssetData` dispararam. Custo:
uma chamada por pasta, dentro do mesmo `open` que a travessia já faz. O conteúdo enxertado não
ocupa espaço próprio: os bytes são os do `*_Cryptex.dmg`, que já está na soma.

`graft_check.py` (conteúdo × DMG, por asset):

| Asset | Enxertado dentro do DMG | Fora | DMG |
|---|---|---|---|
| PKITrustStore | 593.920 | 0 | 4.194.304 |
| SpatialPhotosRelive (2) | 387.043.328 + 24.330.240 | 0 | 406.847.488 + 29.360.128 |
| Siri_Understanding (3) | 653.692.928 + 470.949.888 + 645.496.832 | 0 | 687.865.856 + 494.927.872 + 679.477.248 |
| Speech_ASR | 191.639.552 | 0 | 205.520.896 |
| **Total** | **2.373.746.688 B (2,211 GiB)** | **0** | |

O `ALLOCSIZE` somado do conteúdo é 2.378.076.160 B (2,215 GiB); a diferença de 4,3 MB é o
arredondamento por arquivo e metadado do fs enxertado. (A)

## 3. Deduplicação física

### 3.1 A chamada (A)

`fcntl(fd, F_LOG2PHYS_EXT = 65, struct log2phys)`: entra o deslocamento no arquivo
(`l2p_devoffset`) e o tamanho pedido (`l2p_contigbytes`); sai o deslocamento no disco e quantos
bytes seguem contíguos. `struct log2phys` usa `#pragma pack(4)` (20 bytes): em Rust é
`#[repr(C, packed(4))]`; com `repr(C)` os campos saem deslocados.

Comportamento medido na imagem:

| Caso | Resultado |
|---|---|
| Buraco de esparso | `devoffset = −1`; pular |
| Fim do arquivo | `contigbytes` em bytes, não em blocos: arredondar para 4 KiB |
| Comprimido (`UF_COMPRESSED`, decmpfs) | Fork de dados dá `ENOTSUP`. **Abrir `<arq>/..namedfork/rsrc`** mapeia os blocos reais (1.684.091 B → 1.687.552 alocados, igual ao `ALLOCSIZE`) |
| Clone completo | Mesmas faixas nos dois arquivos |
| Clone parcial (10 MiB reescritos no meio) | Faixas comuns + 5 faixas novas de 2 MiB |
| Pasta de outro usuário sem leitura | `open` falha (`EACCES`): cai no método antigo |

`ditto --hfsCompression` não comprime mais no macOS 27 (arquivo saiu sem `compressed`, na imagem
e no Data). Para testar comprimido copiei com `ditto` um arquivo já comprimido de
`/Applications/Copilot.app`. (A)

### 3.2 Candidatos

Candidato = arquivo com `EF_MAY_SHARE_BLOCKS` **ou** `cloneid != fileid` (fora de graft). Os
demais somam `ALLOCSIZE` como antes (hardlink 1×). Os candidatos são deduplicados por `fileid`,
mapeados em paralelo (10 threads), e a soma é a união das faixas: ordenar por início e somar só
o que passa do fim já coberto. O trecho novo vai para o primeiro dono na ordem física (define o
balde). Quem não mapeia (errno) volta ao método antigo: alocado, clone completo 1× por `cloneid`.
Alocado além do último byte mapeado (pré-alocação após o EOF) conta como privado.

Também promove a candidato o arquivo comum cujo `fileid` é o `cloneid` de um candidato (origem do
clone que perdeu o sinalizador), achado por `fsgetpath`. No Data foram 79 arquivos, ~0 bytes.
**Não compensa no produto** (custa um mapa de todos os arquivos; ver §3.5).

### 3.3 Prova na imagem APFS descartável (A)

`hdiutil create -fs APFS -size 2g` no scratchpad. Arquivos: `a` 100 MiB aleatório; `b = cp -c a`;
`c = cp -c a` com 10 MiB reescritos no meio; `s` esparso (2 MiB de dados em 50 MiB);
`cz`/`czc` comprimido e seu clone; `w` (clone cuja origem foi apagada); `p2`, `q2` (clones com
origem apagada, um reescrito); `x1`, `x2`, `small`, `empty`.

| Medida | Bytes |
|---|---|
| Candidatos, físico medido | 140.578.816 |
| Esperado: 100 MiB + 10 MiB + 20.000.768 (z) + 1.687.552 (cz) + 2.494.464 (w) + 4.096 + 1 MiB | **140.578.816** |
| Não candidatos | 12.984.320 |
| `ATTR_VOL_SPACEUSED` − (físico + não candidatos) | 28.672 (7 blocos, igual em dois estados diferentes do volume) |
| Método `walk_par` (clone completo 1×) | 234.950.656 (conta `c`, `w`, `z` e `cz` a mais) |

Imagem ejetada e apagada ao fim. `exchangedata(2)` dá `ENOTSUP` no APFS, então o salvamento
atômico antigo não foi reproduzido. (A)

### 3.4 Volume Data inteiro, sem root (A)

`frentel /System/Volumes/Data 10 3`, 11:40:

| Categoria | Arquivos | Alocado | Método `walk_par` | Físico |
|---|---|---|---|---|
| Clone completo (`SHARES_ALL`) | 116.999 | 47,988 | 23,698 | 23,684 |
| Clone parcial (`MAY_SHARE`) | 72.698 | 33,115 | 33,115 | **32,590** |
| `cloneid ≠ fileid` sem `MAY_SHARE` | 430.181 | 32,210 | 32,210 | **32,210** |
| Origem de clone promovida | 79 | 0,000 | 0,000 | 0,000 |
| Não candidatos | ~1,38 mi | 185,871 | 185,871 | 185,871 |
| **Árvore** | 2.003.982 | 299,185 | 274,895 | **274,356** |

(GiB.) Enxertos fora da árvore: 7 raízes, 2,215 GiB. Não mapeados: 166 arquivos, 0,002 GiB
(`ENOENT` 162, apagados durante a varredura; `EACCES` 4). Travessia 10,5 s, mapeamento 8,9 s,
971 mil `fcntl` (1,57 por candidato), 14,4 µs por candidato no relógio com 10 threads, pico de
memória 753 MB.

**Prova contra a verdade completa** (`SDC_MAP_ALL=1`: mapeia **todos** os 1,98 mi arquivos,
44,2 s, 1,23 GB): físico − `walk_par` = −0,531 GiB (contra −0,539 no modo candidatos, minutos
depois). Os 1,36 mi arquivos comuns deram físico 186,164 contra 186,156 alocado: nenhum bloco
compartilhado com candidato. Os `cloneid ≠ fileid` deram físico = alocado de novo (32,208). Os
97 arquivos que `open` não abre (`EACCES`, 2,12 GiB) são arquivos comuns: ficam no método antigo
sem perda, porque não são candidatos. (A)

### 3.5 Custo e o que o produto deve fazer (A para os números, B para o desenho)

- Mapear só candidatos custa ~85 % a mais de tempo que a travessia sozinha (8,9 s sobre 10,5 s).
  Mapear tudo custa 4×. A economia real está nos 72,7 mil clones parciais (0,525 GiB) e nos
  clones completos que o `cloneid` não une (0,014 GiB).
- **Restringir ainda mais:** os 430 mil `cloneid ≠ fileid` sem `MAY_SHARE` somaram 0 de
  compartilhamento nas duas rodadas. Tirá-los do mapeamento corta 69 % dos candidatos. Candidato
  no produto = `EF_MAY_SHARE_BLOCKS` (189,7 mil arquivos). (B: vale nesta máquina; o
  sinalizador pode faltar em algum caso que não vi.)
- Memória: o protótipo guarda caminho de todo candidato e um mapa de todos os arquivos (para
  origens). No produto: mapear dentro da travessia com `openat(dirfd, nome)` e guardar só
  `(início, fim, balde)` (24 B por faixa) e sem promover
  origens.
- Clone completo continua podendo usar o atalho do `cloneid`; o mapa físico é necessário só para
  `MAY_SHARE` sem `SHARES_ALL`. Os 0,014 GiB extras são clones completos que compartilham com um
  terceiro (cadeia de clones).

## 4. Quanto muda a árvore do Data (sem root) (A)

| Efeito | Arquivos | GiB |
|---|---|---|
| Enxertos (cryptex do MobileAsset) contados em dobro | 1.120 | **−2,215** |
| Clones parciais | 72.698 | **−0,525** |
| Clones completos que o `cloneid` não une | — | −0,014 |
| `cloneid ≠ fileid` sem `MAY_SHARE` ("salvamento atômico") | 430.181 | **0** |
| **Total** (contra varredura que conta graft e une só clone completo) | | **−2,75** |

- Onde muda (clone parcial): `Users/<usuário>/Library` concentra quase tudo (−22,4 GiB físico −
  alocado, a maior parte já unida por `cloneid`); depois `private/tmp/claude-501` −0,61,
  `Users/<usuário>/dev` −0,47, `iCloud Drive (Archive)` −0,35, `.cache` −0,32, `Xcode.app` −0,18.
- **Correção para o `walk_par` do probe:** hoje, sem `SDC_COUNT_GRAFTS`, ele tira da árvore todo
  arquivo com `cloneid ≠ fileid` sem `MAY_SHARE`: 32,2 GiB de arquivos reais além dos 2,2 GiB de
  graft (os "34,66 GiB" da síntese §3d). A conta que usar o `walk_par` atual subestima a árvore
  nesse valor. Trocar pela regra de pasta da §2.4.
- **Na conta da síntese §3d:** a linha "+ enxertos contados em dobro na árvore 2,21" continua
  certa para a árvore de 3c (feita antes da regra de arquivo). O novo item é −0,54 GiB de
  clones: o "sem explicação" sobe de ~2,17 para ~2,7 GiB. (A para os valores; a soma mistura
  horários diferentes.)
- Com root a árvore cresce (~8 GiB em 3c); pode haver mais clones parciais nas pastas `EACCES`.
  Comando para o dono na §6.

## 5. O que são os 430 mil `cloneid ≠ fileid` sem `MAY_SHARE` (B)

Exemplo medido: `Discord.app/Contents/Info.plist`, `fileid` 70382826, `cloneid` 70381133,
`ext_flags` 0, privado 0 (preso em snapshot). `fsgetpath(70381133)` = `ENOENT`: a origem foi
apagada. Na imagem, um clone cuja origem foi apagada **mantém** `MAY_SHARE` (`p2`, `w`); no Data o
sinalizador sumiu. Causa provável: o arquivo nasceu como clone de um temporário (instalador,
atualizador, `ditto`: `z` na imagem ganhou `cloneid` 21 e `fileid` 22) e o APFS limpou o
sinalizador depois. Não reproduzi a limpeza. O que importa está medido: não compartilham blocos.

## 6. Comandos para o dono (opcionais, só leitura)

```sh
cd "research/experimentos/frenteL"
cc -O2 -o graftinfo graftinfo.c
sudo ./graftinfo /System/Volumes/Data \
  /System/Volumes/Data/System/Library/AssetsV2/com_apple_MobileAsset_PKITrustStore/purpose_auto/6dd55b0d06633a00de6f57ccb910a66a5ba2409a.asset/.AssetData/EVRoots.plist
```

Esperado: `grafts=7`, registros com `dir` = fileids das 7 `.AssetData` (1871472, 46024632, ...)
e `in_graft=1` no segundo caminho. Isso confirma a regra da §2.4 pela fonte oficial.
`FSIOC_GET_GRAFT_INFO` é consulta (`_IOWR`, "get"); não enxerta nem desenxerta.

Árvore com root e dedup física (só leitura):
`sudo <target>/release/frentel /System/Volumes/Data 10 3`.

## 7. Incertezas

- A regra "pasta aberta com `cloneid ≠ fileid`" foi validada em um volume (7 de 7 grafts, 0 falso
  positivo em 377 mil pastas). Não testei graft de outro tipo (Preboot, cryptex de sistema, que
  ficam fora do Data). (M)
- `F_LOG2PHYS_EXT` pode mudar entre a leitura e a soma se o arquivo for reescrito; 162 arquivos
  sumiram durante a rodada. Efeito medido: ~0,002 GiB. (A)
- Arquivo comprimido pequeno guarda os dados no xattr `decmpfs` (sem fork de recurso): não tem
  faixa e não compartilha; fica com o alocado. (B)
- Blocos compartilhados com snapshot não entram aqui: a união só olha a árvore viva. A parcela de
  snapshot continua vindo do `fsctl` de snapshot (síntese §2.2). (A)

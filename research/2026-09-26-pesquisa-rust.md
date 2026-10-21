# Pesquisa Rust — 2026-09-26

Pesquisa para reescrever o plano em Rust (o plano anterior era Swift). Duas fontes:

1. **Experimentos locais** em `research/experimentos/probe/` (macOS 27, Apple Silicon, sem root; com Full Disk Access herdado do app que lançou o processo, descoberto em 27/09).
2. **Pesquisa web** em fonte primária (repositórios, headers do SDK/xnu, crates.io, man pages).

Resumo em uma linha: **o macOS já entrega, sem root, quase tudo que o projeto precisa por
arquivo** (espaço liberável real, clones, dataless, sparse). O que não existe é uma API para o
tamanho de cada snapshot.

## Princípio: só medição, nada de estimativa

Decisão do dono (2026-09-27). Todo número exibido vem de uma syscall ou API do sistema, ou é
aritmética exata sobre números medidos, com a origem declarada. Proibido: heurística por nome
de caminho, "economia provável", fator de correção, extrapolação por amostra.

Consequências:

- O que o sistema não informa aparece como **"não atribuído: X GiB"**, a diferença exata entre
  o uso do volume (`statfs`) e a soma medida. Nunca com um rótulo adivinhado ("provavelmente
  snapshots").
- Bytes presos em snapshot são **medidos por arquivo** (`ALLOCSIZE − PRIVATESIZE`), não inferidos.
- Clone parcial: a parte compartilhada não é medível por arquivo. Mostrar o `PRIVATESIZE`
  (medido) e dizer que o resto é compartilhado, sem dividir.
- Quando uma medição falha (TCC, sem FDA), contar e mostrar as pastas não lidas; não completar
  com palpite.

---

## 1. Correções à pesquisa de 2026-08-09

| Doc antigo dizia | Realidade (testado) |
|---|---|
| "Não há syscall que responda se um arquivo é clone" (`docs/02`) | `getattrlist` devolve `ATTR_CMNEXT_CLONEID` (mesmo id = clone puro), `EF_SHARES_ALL_BLOCKS`, `EF_MAY_SHARE_BLOCKS` e `ATTR_CMNEXT_CLONE_REFCNT` |
| Clone: "marcar economia incerta, heurística por caminho" | `ATTR_CMNEXT_PRIVATESIZE` dá o número exato de bytes liberados se o arquivo for apagado |
| Snapshots são item secundário ("no caso real não havia") | Em 26/09 eram o maior item: parar o backup liberou **96 GiB** |
| Dataless: detectar com `ls -lO` / `ubiquitousItemDownloadingStatusKey` | `ATTR_CMN_FLAGS & SF_DATALESS` no mesmo `getattrlist`, sem abrir o arquivo |
| Stack: Swift + `URLResourceKey` | Rust + `libc::getattrlist[bulk]` cobre tudo; só o purgeable do volume pede CoreFoundation |

---

## 2. APIs confirmadas no experimento

Um único `getattrlist`/`getattrlistbulk` com `FSOPT_ATTR_CMN_EXTENDED` pede:

| Atributo | O que deu no teste |
|---|---|
| `ATTR_FILE_ALLOCSIZE` | Blocos alocados reais. Sparse de 10 GiB lógico → 0,01 GiB |
| `ATTR_CMNEXT_PRIVATESIZE` | Bytes liberados na hora ao apagar. Clone puro → 0. Clone com 50 MB reescritos → 50 MB. Arquivo preso em snapshot → ~0 |
| `ATTR_CMNEXT_CLONEID` | Original e `cp -c` têm o mesmo id |
| `ATTR_CMNEXT_EXT_FLAGS` | `SHARES_ALL` no clone puro, `MAY_SHARE` no parcial, `SPARSE` no disco do Colima |
| `ATTR_CMNEXT_CLONE_REFCNT` | 2 para original + clone |
| `ATTR_CMN_FLAGS` | `SF_DATALESS` detectado num placeholder do OneDrive, sem download |
| `ATTR_FILE_LINKCOUNT` + `ATTR_CMN_FILEID` | Hardlink: mesmo fileid, links = 2 |

Constantes ausentes no crate `libc` 0.2.189 (declarar à mão, valores do SDK):
`SF_DATALESS = 0x40000000`, `SF_FIRMLINK = 0x00800000`, `ATTR_CMNEXT_CLONE_REFCNT = 0x1000`,
`EF_MAY_SHARE_BLOCKS = 0x1`, `EF_IS_PURGEABLE = 0x8`, `EF_IS_SPARSE = 0x10`, `EF_SHARES_ALL_BLOCKS = 0x40`.

Disponibilidade por versão (pela primeira tag do xnu com o símbolo, não testado em runtime):
`PRIVATESIZE` macOS 10.13; `CLONEID`/`EXT_FLAGS` macOS 11; `SHARES_ALL_BLOCKS`/`CLONE_REFCNT` macOS 14.
**Alvo mínimo sugerido: macOS 14.**

### Armadilhas encontradas no experimento

- **`getattrlistbulk` não aceita `FSOPT_PACK_INVAL_ATTRS`.** Cada entrada só traz os atributos
  presentes no `attribute_set` retornado; o parser tem de checar bit a bit. Atributos de arquivo
  não vêm para diretórios.
- **Avançar o cursor antes de qualquer `continue`.** O primeiro parser pulava entradas ao
  deduplicar hardlink/clone (bug real, corrigido em `walk.rs`).
- **`PRIVATESIZE` depende do momento.** Logo após um snapshot do Time Machine, quase tudo fica
  "preso": em `~/Library`, 106 GiB alocados e 0,05 GiB privados. O número responde "quanto libero
  agora", não "quanto é lixo". A UI precisa explicar isso.
- **Clone parcial conta duas vezes no alocado.** Só `SHARES_ALL` pode ser deduplicado por
  `CLONEID`; no parcial, a parte compartilhada é invisível (mesma limitação do `dua-cli`).
- **Target do Cargo fora do iCloud.** O projeto mora no iCloud Drive; compilar com
  `CARGO_TARGET_DIR` apontando para fora, senão o iCloud sincroniza GBs de build.

---

## 3. Números reais desta máquina (26/09)

Varredura de `~/Library` com `walk` (single-thread, `getattrlistbulk`):

| Métrica | Valor |
|---|---|
| Entradas | 455 mil arquivos, 97 mil pastas, 141 erros (140 eram pastas dataless de File Provider, ver 2026-09-27-experimentos-volume.md) |
| Tempo | 22 s (17 s sem atributos estendidos; `PRIVATESIZE` custa ~25%) |
| `du -sxk` na mesma pasta | 15,6 s, **127,4 GiB** |
| Alocado real (clones e hardlinks uma vez) | **106,0 GiB** |
| Clones repetidos | **21,4 GiB** que o `du` conta duas vezes |
| Dataless | 2.802 arquivos, 51,8 GiB aparentes, 0 no disco |
| Esparsos | 229,7 GiB aparentes, 8,9 GiB alocados |

O `du` e a varredura batem exatamente: 106,0 + 21,4 = 127,4. **O `du` superestima `~/Library`
em 21 GiB nesta máquina.** Os clones são o backup do WhatsApp no iCloud (`Video.tar` 11,1 GiB,
`Media.tar` 5,6 GiB, `Document.tar` 2,3 GiB) clonados pelo cache do CloudKit, e um `.jsonl`
de 1,5 GiB do Claude Code. É o mesmo achado da auditoria antiga, agora medido com exatidão.

Volume de dados (via Foundation, script em `experimentos/`):

| Métrica | Valor |
|---|---|
| Total | 460,4 GiB |
| Livre (`statfs`) | 119,3 GiB |
| Livre p/ uso importante | 122,6 GiB → **purgeable = 3,3 GiB** (diferença entre os dois valores da API) |
| Livre p/ uso oportunista | 143,1 GiB (maior que "importante": não entendido, ver incertezas) |

---

## 4. Snapshots e purgeable (volume)

- **Não existe API pública para o tamanho de um snapshot.** `fs_snapshot_list` exige root e
  entitlement. `tmutil listlocalsnapshots /` e `diskutil apfs listSnapshots -plist` listam sem
  root, mas sem bytes. Só o Utilitário de Disco mostra ("Private Size"), via API privada.
- **O que dá para medir sem essa API:**
  1. Por arquivo: soma de `ALLOCSIZE − PRIVATESIZE` nos arquivos sem `MAY_SHARE` = bytes que
     só um snapshot segura. Em `~/Library` deu 55,7 GiB logo após um snapshot novo.
  2. Por volume: "não atribuído" = usado (`statfs`) − soma medida − purgeable. É uma diferença
     exata; o relatório não diz o que ela é, só quanto é.
- Blocos que existem **só** no snapshot (arquivo já apagado) não aparecem em nenhum arquivo vivo
  e caem no "não atribuído".
- **Purgeable** = `volumeAvailableCapacityForImportantUsage` − livre do `statfs`. Em Rust:
  `CFURLCopyResourcePropertyForKey` com `kCFURLVolumeAvailableCapacityForImportantUsageKey`
  (crate `objc2-core-foundation` 0.3.2; o `core-foundation` 0.10.1 não tem a constante).

---

## 5. Ecossistema Rust

### Ferramentas existentes

| Ferramenta | Licença | Último release | Relevância |
|---|---|---|---|
| **dua-cli** / `dua-core` | MIT | 2.45.0 (2026-09-12) | **Base mais próxima.** Já usa `getattrlistbulk` (desde 2.42.0), `CLONEID`/`EXT_FLAGS`, `SF_FIRMLINK`, dedup de clones opt-in. Não trata `SF_DATALESS` nem `PRIVATESIZE` |
| dust | Apache-2.0 | 1.2.6 (2026-09-16) | `read_dir` + rayon, dedup de hardlink. Nada de APFS |
| pdu | Apache-2.0 | 0.24.0 (2026-05-28) | rayon, hardlink opt-in. Nada de APFS |
| diskus | Apache-2.0 | 0.9.0 (2025-12-06) | Só total |

### Crates de travessia

| Crate | Situação |
|---|---|
| jwalk 0.9.0 | **Arquivado em 2026-08-05**, README manda usar `dua-core` |
| `dua-core` 4.1.0 | Work-stealing (crossbeam) + `getattrlistbulk` no macOS, predicado `descend`. Candidato principal |
| walkdir 2.5.0 | Sequencial, estável. Insuficiente sozinho |
| ignore 0.4.33 | `WalkParallel`, mas um `stat` por entrada no macOS e foco em gitignore |
| getattrlistbulk 0.1.0 | Wrapper novo, 1 release, 2 estrelas. Imaturo |

**Questão aberta:** o `dua-core` expõe os atributos que precisamos (`PRIVATESIZE`, `SF_DATALESS`)
ou só os que o `dua` usa? Se não expõe, escrever a travessia própria custa ~200 linhas (o
`walk.rs` single-thread tem ~150) mais paralelismo. Ler `crates/dua-lib/src/macos/attributes.rs`
antes de decidir.

### Outros crates

- `libc` 0.2.189: `getattrlist`, `getattrlistbulk`, `statfs`, structs. Falta `setiopolicy_np`.
- `setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS,
  IOPOL_MATERIALIZE_DATALESS_FILES_OFF)`: impede que o processo baixe arquivo dataless por
  acidente. Declarar via FFI. Rede de segurança obrigatória.
- `objc2-core-foundation` 0.3.2: purgeable sem Objective-C.

---

## 6. Distribuição

- **cargo-dist** 0.33.0: gera fórmula de Homebrew num tap próprio. Binário universal ainda não
  (issue #77 aberta): `lipo -create` à mão no CI.
- **Fórmula de Homebrew não recebe quarentena** (só casks), então o CLI dispensa notarização e os
  USD 99/ano. Ad-hoc signing do linker basta no arm64.
- Binário baixado por navegador/`curl` recebe quarentena e aí precisaria de notarização.

---

## 7. Concorrentes

| Projeto | O que faz | O que não faz |
|---|---|---|
| **Mole** (tw93), Go, GPL-3.0, ~68k estrelas | Limpador + analisador. Lista snapshots (sem tamanho), purgeable por Finder − `statfs`, mede com `du -skP` | Não usa `getattrlist`, não detecta clones nem dataless, não fecha a conta do System Data |
| Pearcleaner | Desinstalador de apps, "On Hold" | Não analisa volume |
| GrandPerspective | Treemap | Sem snapshots/purgeable (não verificado no código) |

**Diferencial confirmado:** ninguém usa `PRIVATESIZE` + `SF_DATALESS` + clones + bytes presos
em snapshot para fechar a conta do volume só com medições e explicar cada fatia.

---

## 8. Incertezas

- **Versões mínimas vêm dos headers do xnu**, não de teste em cada macOS.
- **Purgeable "oportunista" > "importante"** nesta máquina: contraria a documentação. Pode ser
  o backup em andamento. Medir de novo sem backup rodando.
- **A chamada de capacidade "importante" demorou minutos** no script Swift (parte é compilação do
  `swift`). Medir o custo real em Rust antes de pôr no caminho crítico.
- **Dataless em outros provedores:** testado só com OneDrive. Falta iCloud (nenhum placeholder
  encontrado nos primeiros 3.000 arquivos) e Google Drive.
- **Paralelismo não medido.** O `walk` é single-thread; o `du` foi 30% mais rápido com cache quente.
  Medir com `dua-core` ou pool próprio.
- ~~Sem Full Disk Access houve 141 pastas negadas~~ Correção 27/09: havia FDA herdado; 140 dos 141 erros eram pastas dataless de File Provider.
- **Benchmarks independentes** de `dua-core` × `ignore` × `getattrlistbulk` não encontrados.

---

## 9. Próximos passos sugeridos

1. Ler o `dua-core` e decidir: depender dele ou travessia própria.
2. Medir o `walk` em paralelo e com FDA no volume inteiro (`/System/Volumes/Data`).
3. Protótipo da **conta que fecha**: usado (`statfs`) = soma por categoria + preso em snapshot
   + purgeable + volumes do sistema + não atribuído, tudo medido.
4. Só depois: categorias com explicação em português, e a whitelist de limpeza.

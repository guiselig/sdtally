# Frente E — produto, arquitetura, testes, distribuição, comunidade — 2026-09-27

Escopo: como o CLI deve se apresentar, como organizar o código Rust, como testar de forma
reproduzível, como distribuir, que licença e nome usar, como ganhar confiança, e um roadmap com
critérios medíveis. Pesquisa em fonte primária (crates.io API, GitHub API/código, docs.brew.sh,
docs Apple, blogs dos autores) e experimentos locais no scratchpad (macOS 27 build 26A5416b,
Apple Silicon, sem sudo, sem FDA, com backup do Time Machine em fase Copying).

Confiança: **A** alta (fonte primária lida ou medido aqui), **M** média (fonte secundária ou
inferência de fonte primária), **B** baixa (não verificado).

Resumo:

1. **Produto:** um comando sem subcomando imprime "a conta do volume" (fatias medidas + não
   atribuído, cada linha com a origem). Detalhamento não interativo primeiro; TUI (ratatui 0.30.2)
   só na fase 3. JSON com `schema_version` e bytes inteiros.
2. **Arquitetura:** um pacote com `lib.rs` + `main.rs`, não workspace de 4 crates. `unsafe` só no
   módulo de FFI; o parser do `getattrlistbulk` é função pura sobre `&[u8]` (fuzzável, roda no Miri).
   Modo padrão agrega durante a travessia e não guarda árvore: **30,5 MiB de pico para 521 mil
   entradas** (medido).
3. **Testes:** clones, esparsos e hardlinks são reproduzíveis sem root numa imagem APFS criada
   com `hdiutil` em 1 s (testado). **Dataless não dá para criar sem File Provider**: testado,
   `chflags` do userspace não grava `SF_DATALESS`. Dataless se testa no parser com buffers
   sintéticos, mais um teste manual.
4. **Distribuição:** cargo-dist 0.33.0 com tap próprio, binário só arm64 (macOS 27 não roda em
   Intel), ad-hoc do linker, sem notarização. homebrew-core só depois de 225 estrelas (regra
   para quem submete o próprio projeto).
5. **Nome:** `sdc` e `system-data` estão ocupados no crates.io; `sysdata` é o nome do CLI e do
   cask da ferramenta criticada pelo Oakley. Cinco nomes livres verificados na seção 5.

---

## 1. UX de CLI

### 1.1 O que as ferramentas existentes ensinam

| Ferramenta | O que funciona | Do que usuários reclamam (issue) | Conf. |
|---|---|---|---|
| **dua-cli** 2.45.0 (MIT, 6,3k ★) | `dua` sem args = resumo; `dua i` = TUI; marcar e depois apagar; i18n sem dependência | Atalho perigoso: `Ctrl+r` apagava sem confirmação ([#248](https://github.com/Byron/dua-cli/issues/248)); refresh ([#96](https://github.com/Byron/dua-cli/issues/96)); navegação lenta com 13M arquivos, 1,5 GB de RAM ([#227](https://github.com/Byron/dua-cli/issues/227)); `aggregate --stack` guardava a árvore inteira, 4,2 GiB para 20,5M entradas e OOM em 2 GiB, corrigido com streaming no 2.43.1 ([#382](https://github.com/Byron/dua-cli/issues/382)) | A |
| **dust** 1.2.6 (Apache-2.0, 12,4k ★) | Uma tela, árvore com barras, sem interação | Clones APFS contados uma vez por arquivo ([#590](https://github.com/bootandy/dust/issues/590), aberta); OOM com 32 GB num FS de 600 TB, e o autor diz que é limitação conhecida: monta a árvore inteira antes de mostrar ([#581](https://github.com/bootandy/dust/issues/581)); unidade decimal ([#182](https://github.com/bootandy/dust/issues/182)) | A |
| **gdu** 5.37.0 (Go, MIT, 6k ★) | Rápido, exporta JSON | 80 GB de RAM em 300M itens; pedem parar a varredura e manter o parcial ([#150](https://github.com/dundee/gdu/issues/150)); conta arquivos do OneDrive que não estão no disco ([#265](https://github.com/dundee/gdu/issues/265)); pedem opção para desligar a exclusão ([#224](https://github.com/dundee/gdu/issues/224)) | A |
| **ncdu** 2.x (Zig) | Memória pequena: 3,8M arquivos em 162 MB, 38,9M em 1.686 MB; 25 B por arquivo e 56 B por pasta, contra 78 B no 1.x; exporta a árvore para navegar depois | — | A ([dev.yorhel.nl/doc/ncdu2](https://dev.yorhel.nl/doc/ncdu2)) |
| **diskonaut** 0.11 (parado desde 2020) | Treemap no terminal | 22 GB de RAM em diretório grande ([#65](https://github.com/imsnif/diskonaut/issues/65)); excluir pasta ([#80](https://github.com/imsnif/diskonaut/issues/80)); ficar no mesmo FS ([#23](https://github.com/imsnif/diskonaut/issues/23)) | A |
| **DaisyDisk** | Mostra "hidden space" = usado − soma varrida, com snapshots e purgeable dentro | Mostra tamanho **estimado** de snapshot (a regra do projeto proíbe) | A ([guia](https://daisydiskapp.com/guide/4/en/HiddenSpace/), [snapshots](https://daisydiskapp.com/manual/4/en/Topics/Snapshots.html)) |
| **Mole** V1.56.0 (GPL-3.0, 68k ★) | `--dry-run` em tudo que apaga, whitelist | "320 GB left": o usuário não entende a diferença entre `df` e o que o Utilitário de Disco mostra ([#222](https://github.com/tw93/Mole/issues/222)) | A |
| GrandPerspective | Treemap | Não verificado | B |

Padrões que se repetem:

- **A árvore inteira na memória é a causa nº 1 de falha** (dust, gdu, diskonaut, dua aggregate).
  Numa ferramenta para disco cheio isso é fatal.
- **Número sem explicação gera issue** (Mole#222, gdu#265). O DaisyDisk resolve com uma fatia
  explícita, "hidden space". É o nosso "não atribuído".
- **Exclusão é onde a confiança se perde** (dua#248, gdu#224, gdu#140). Coerente com a regra de
  começar só com leitura.

### 1.2 Proposta de interface

```
$ sdtally                    # nome provisório; ver seção 5
Volume Data (/System/Volumes/Data)                        GB (10^9)   origem
  Usado                                                    391,58      statfs
  ├─ Arquivos lidos, alocado único                         ...         getattrlistbulk ALLOCSIZE
  │    ~/Library                                           114,06
  │    /Applications                                       ...
  ├─ Purgeable (o macOS libera sozinho)                    3,54        CFURL ImportantUsage − statfs
  ├─ Pastas não lidas (sem permissão): 141                 —           erro EACCES/EPERM, contado
  └─ Não atribuído                                         X (Y%)      diferença: usado − soma acima
Informativo (já contido nas linhas acima, não somar):
  preso em snapshot (Σ ALLOCSIZE − PRIVATESIZE)            59,87
  clones repetidos (não contados de novo)                  23,17
  dataless: 2.802 arquivos, 55,6 GB aparentes, 0 no disco
```

Regras de interface:

1. **Um comando, sem subcomando, faz a conta** (padrão `dua`/`dust`). Nada é apagado em
   nenhuma fase antes da 4.
2. **Toda linha tem coluna de origem** (syscall, API ou "diferença"). Aplicação direta da regra
   de só medir.
3. **Linhas somáveis ficam separadas das informativas.** "Preso em snapshot" já está dentro do
   alocado; somar os dois é dupla contagem. Na fixture da seção 3 o erro aparece de verdade.
4. **Detalhamento sem TUI primeiro:** `sdtally <caminho> [--depth N] [--top N]` imprime a árvore
   de um ramo. O subcomando `why <caminho>` imprime os atributos brutos de um arquivo (alocado,
   privado, clone id, flags). A TUI vem na fase 3, com ratatui **0.30.2** (MIT, MSRV 1.88; o
   `dua` usa 0.30.2 com crossterm 0.29) (A, crates.io e `Cargo.toml` do dua).
5. **Varredura interrompida vira relatório parcial marcado INCOMPLETO**, não saída sem nada
   (lição de gdu#150 e dua#382).
6. **Unidade:** padrão GB decimal, a mesma do painel de Ajustes, com o rótulo sempre visível;
   `--binary` para GiB. As pesquisas anteriores usam GiB; a decisão é do dono. O pedido existe
   (dust#182) (M).
7. **JSON estável:** `--json` imprime só em stdout; progresso e avisos vão para stderr. Formato:
   - `schema_version` inteiro na raiz; muda quando um campo existente muda de sentido ou é removido.
   - Bytes como **inteiro u64**, nunca float nem string formatada; origem como enum
     (`"statfs"`, `"getattrlistbulk"`, `"cf_important_usage"`, `"difference"`).
   - Chaves em inglês, independentes do idioma.
   - Schema publicado gerado por `schemars` 1.2.2 (A, crates.io) e testado em CI (snapshot do
     schema com `insta`: mudar o schema sem subir a versão quebra o teste).
   - O `gdu` recebeu pedidos de exportar JSON a partir do modo interativo (#141) e de escolher os
     atributos (#605). Começar com um formato só.
8. **Cores acessíveis:**
   - Cor nunca é a única pista: toda linha tem texto ("não atribuído", "INCOMPLETO").
   - Nada de par vermelho/verde com o mesmo significado.
   - Respeitar `NO_COLOR` e `--color auto|always|never`; detectar TTY com
     `std::io::IsTerminal` (stdlib, sem crate).
   - `anstream` 1.0.0 remove ANSI quando a saída não é TTY (A, crates.io; comportamento M).
9. **Disco cheio:**
   - Não gravar nada por padrão: sem cache e sem índice.
   - Histórico ("cresceu desde ontem") só com `--save`, e grava só o resumo por fatia (KB),
     nunca a árvore.
   - Se `write` falhar com `ENOSPC`, avisar e seguir.
   - Memória limitada: ver 2.5.
10. **Full Disk Access:** num CLI, o TCC atribui o acesso ao app de terminal (Terminal, iTerm),
    não ao binário. A mensagem deve dizer "conceda FDA ao seu terminal" e contar as pastas
    negadas (M: comportamento conhecido do TCC, sem doc da Apple lida nesta sessão).

### 1.3 i18n pt-BR / en

| Opção | Versão | Prós | Contras | Conf. |
|---|---|---|---|---|
| **Tabela estática própria** (como o `dua`) | — | Zero dependência. O `dua` faz assim: `enum Language` + detecção por `LC_ALL` > `LC_MESSAGES` > `LANG` ([`src/interactive/widgets/i18n.rs`](https://github.com/Byron/dua-cli/blob/main/src/interactive/widgets/i18n.rs)); erro de chave aparece na compilação | Plural feito à mão | A |
| `rust-i18n` | 4.2.3 (2026-09-27), MIT, 669 ★ | Macro `t!`, YAML/JSON/TOML embutidos na compilação, fallback de locale, `cargo i18n` acha textos sem tradução | Sem suporte a plural no README; chave errada só aparece em runtime | A (README) |
| `fluent` / `fluent-bundle` | 0.17.0 / 0.16.0 (mai/2025), Apache-2.0 | Plurais CLDR, gênero, formato para tradutores | API mais pesada; último release há 16 meses | A |

**Recomendação:** tabela estática com 2 idiomas. pt-BR e en têm só as formas "one" e "other";
basta uma função `plural(n)` de 3 linhas. Migrar para fluent se aparecer um terceiro idioma com
tradutores da comunidade (M).

No macOS, `LANG` pode vir vazio quando o terminal não o define. Como fallback, ler o idioma
preferido do sistema pelo CoreFoundation (`CFLocaleCopyPreferredLanguages`, disponível no
`objc2-core-foundation`, que já é dependência para o purgeable) (M: símbolo não conferido no crate).

---

## 2. Arquitetura Rust

### 2.1 Organização

Pedido original: workspace com crates core/scan/report/cli. **Recomendação: um pacote só, com
`src/lib.rs` + `src/main.rs`**, e módulos:

```
src/
  main.rs      # clap, cores, idioma; anyhow
  lib.rs       # #![deny(unsafe_code)]
  sys.rs       # #[allow(unsafe_code)]: getattrlistbulk, statfs, setiopolicy_np, CF
  parse.rs     # parser puro do buffer (&[u8] -> entradas). Sem unsafe
  walk.rs      # pool de threads, cancelamento, contadores de progresso
  ledger.rs    # a conta: fatias, somas, não atribuído (aritmética pura, testável)
  report.rs    # texto e JSON
  i18n.rs
```

Por quê:

- O próprio `dua-cli` é 1 binário + 1 crate de biblioteca (`dua-lib`), e só separou a lib quando
  outros projetos passaram a usá-la (A, `Cargo.toml` do dua).
- `lib.rs` + `main.rs` no mesmo pacote já dá a fronteira de biblioteca (testes de integração
  usam a lib) sem gerenciar versões entre crates.
- Workspace só quando existir um segundo consumidor real (app gráfico, publicar a lib).

### 2.2 Erros

- **Biblioteca:** `thiserror` 2.0.21 (MSRV 1.77), com enum pequeno: `Scan { path, errno }`,
  `Volume(..)`, `Parse { offset, reason }`.
- **Binário:** `anyhow` 1.0.104 para `context()` e saída amigável (A, crates.io).
- **Erro por entrada não é `Err`, é dado.** Conta por errno e segue (o `walk_par.rs` já faz isso:
  `erros por errno [(13, 1)]`). Pasta ilegível vira linha do relatório, não aborta a varredura.

### 2.3 FFI e `unsafe`

- `libc` 0.2.189 (MSRV 1.65) para `getattrlistbulk`, `statfs`, `open`/`close`.
- Constantes ausentes no crate ficam declaradas à mão num único lugar, com o valor e o header do
  SDK de origem (já listadas em `2026-09-26-pesquisa-rust.md`).
- `objc2-core-foundation` 0.3.2 (MSRV 1.71) só para a capacidade "important usage".
- **`unsafe` confinado a `sys.rs`.** `#![deny(unsafe_code)]` na raiz e `#[allow(unsafe_code)]`
  só nesse módulo; `unsafe_op_in_unsafe_fn` já é erro na edition 2024.
- **O parser não usa `unsafe`:** lê o buffer com `u32::from_ne_bytes(buf[o..o+4].try_into()?)`,
  checa limites e devolve `Err` em vez de pânico.

  Hoje o `walk_par.rs` faz leitura por ponteiro. Separar parser e syscall é o que permite fuzz
  e Miri. O Miri já está instalado aqui: `~/.cargo/bin/cargo-miri` (A, `ls`).
- **Rede de segurança:** chamar `setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, …OFF)`
  logo no início do `main`, antes de qualquer thread (já no `walk_par.rs`).

### 2.4 Paralelismo, cancelamento e progresso

| Opção | Versão | Situação | Conf. |
|---|---|---|---|
| Threads da stdlib + `Mutex<Vec>` + `Condvar` (o `walk_par.rs` atual) | stdlib | **Medido aqui: `~/Library`, 8 threads, 3,9–4,1 s, 521 mil entradas, pico de 30,5 MiB**, com backup do TM rodando | A |
| `crossbeam-deque` (work stealing, modelo do `dua-core`) | 0.8.8 (2026-09-05) | Medido em 27/09: `dua-core` com 4 threads fez 4,8 s e com 12 fez 3,4 s | A |
| `rayon` | 1.12.0 | `dust` e `pdu` usam. Recursão com `scope` funciona, mas cancelar e limitar a fila é menos direto | M |

**Recomendação:** manter a stdlib, que já está medida e no mesmo patamar do `dua-core`. Trocar
por `crossbeam-deque` só se um perfil mostrar contenção no `Mutex` com mais de 8 threads.

- **Cancelamento:** `static CANCEL: AtomicBool`, checado a cada pasta. SIGINT via `libc::sigaction`
  (10 linhas, a dependência já existe) ou `ctrlc` 3.5.2. No cancelamento, o relatório sai com o
  que foi medido e a marca INCOMPLETO.
- **Progresso:** contadores `AtomicU64` (pastas, arquivos, bytes). Uma thread imprime em stderr a
  cada 250 ms, só se stderr for TTY e sem `--json`. `indicatif` 0.18.6 é opcional; `\r` + texto basta.

### 2.5 Memória para milhões de arquivos

Números reais de ferramentas equivalentes:

| Ferramenta | Carga | Memória | B/entrada (aritmética) | Fonte |
|---|---|---|---|---|
| ncdu 2.0 | 3,8M arquivos | 162 MB | ~43 | A, autor |
| ncdu 2.0 | 38,9M arquivos | 1.686 MB | ~43 | A, autor |
| ncdu 1.16 | 3,8M arquivos | 429 MB | ~113 | A, autor |
| dua (TUI) | 13M arquivos | ~1,5 GB | ~115 | A, dua#227 |
| dua `aggregate --stack` 2.43.0 | 20,5M entradas | 4,2 GiB | ~220 | A, dua#382 |
| gdu | 300M itens | 80 GB | ~285 | A, gdu#150 |
| dust | FS de 600 TB | OOM com 32 GB | — | A, dust#581 |
| diskonaut | "diretório grande" | 22 GB | — | A, diskonaut#65 |
| **walk_par (nosso, só agregando)** | 521 mil entradas | **30,5 MiB** | ~61 com o runtime | **A, medido aqui** |

Estrutura do `dua` hoje: árvore em arena própria com índices `u32` (`TreeIndex(NonZeroU32)`),
nó de **64 bytes** fixado por teste (`size_of::<TreeNode>() == 64`) e nomes em arena separada
([`src/traverse.rs`](https://github.com/Byron/dua-cli/blob/main/src/traverse.rs)) (A).

Proposta, em dois modos:

1. **Padrão, a conta:** sem árvore. Agrega por "balde" (prefixo de caminho, como o `walk_par`)
   durante a travessia.
   - Mapas de dedup só para quem precisa: `(dev, fileid)` quando `nlink > 1`, `clone_id` quando
     `SHARES_ALL`.
   - A memória cresce com o número de hardlinks e clones, não com o de arquivos. Medido: 30,5 MiB.
2. **Detalhamento/TUI:** arena `Vec<Node>` + `Vec<u8>` de nomes.
   - Nó proposto com `parent`, `first_child`, `n_children`, `name_off` (u32), `name_len`, `flags`
     (u16), `alloc`, `private` (u64): **40 bytes (medido com `size_of`)**; 56 bytes com
     `clone_id` e `file_id`.
   - Nome médio em `~/Library`: **25,9 caracteres (medido, 585.919 entradas)**.
   - Aritmética: ~66 B/entrada, ou seja ~66 MB por milhão, ~1,3 GB para 20M. Fica entre o
     ncdu 2 e o dua.
   - Para volume enorme: limitar a profundidade guardada e agregar abaixo dela (ideia de
     diskonaut#65), ou fazer streaming como o dua 2.43.1.

   `compact_str` e `bumpalo` são desnecessários: uma `Vec<u8>` única já é a arena.

---

## 3. Testes

### 3.1 Experimento: fixture APFS sem root (A, executado aqui)

```sh
hdiutil create -size 2g -fs APFS -type SPARSE -volname sdcfx fx.sparseimage   # 1,0 s
hdiutil attach -nobrowse -mountpoint "$S/mnt" fx.sparseimage                 # sem sudo
dd if=/dev/urandom of=t/orig.bin bs=1m count=64
cp -c t/orig.bin t/clone.bin                                      # clone total
cp -c t/orig.bin t/partial.bin; dd ... count=8 conv=notrunc       # clone parcial (8 MiB próprios)
ln t/orig.bin t/hard.bin                                          # hardlink
truncate -s 10g t/sparse.img; dd ... count=1 conv=notrunc         # esparso, 1 MiB escrito
```

Resultado do `walk_par` sobre a fixture:

| Medida | Valor |
|---|---|
| arquivos | 3 (+1 hardlink, +1 clone repetido) |
| esparso | 10,00 GiB aparentes, 0,00 alocados |
| `statfs` usado | 73 MiB (74.776 KiB) |
| alocado único somado | 0,13 GiB |
| diferença | **−0,05 GiB** |

A diferença negativa é o clone parcial: os 56 MiB compartilhados entram duas vezes no alocado.
É exatamente a limitação descrita na pesquisa de 26/09, agora reproduzível em 1 s.

**A fixture vira golden test:** a conta tem de reconhecer a sobreposição (usar `PRIVATESIZE` nos
arquivos `MAY_SHARE` ou mostrar a linha "compartilhado por clone parcial") e nunca produzir
"não atribuído" negativo. Decidir com as frentes de medição.

### 3.2 Dataless: não dá para criar sem File Provider (A, testado)

- `chflags 0x40000000 arquivo` → `chflags: invalid flags`.
- `os.chflags(p, 0x40000000)` pelo Python retorna sem erro, mas `ls -lO` mostra que a flag não
  foi gravada: o kernel descarta a flag vinda do userspace.
- `fileproviderctl` no macOS 27 só tem `dump`, `diagnose`, `evaluate`, `check|repair`,
  `obfuscate`; nenhum comando de evict. `brctl` só tem `diagnose`, `log`, `dump`, `status`,
  `accounts`, `quota`, `monitor`; o antigo `brctl evict` não existe mais.
- Criar dataless exige uma extensão File Provider assinada (M, inferência). Inviável em CI.

Plano para dataless:

1. Teste de unidade do parser com buffer sintético (`ATTR_CMN_FLAGS` com `SF_DATALESS`).
2. Teste de integração `#[ignore]` que lê o caminho de `SDC_DATALESS_SAMPLE`: roda na máquina
   do dono, que tem placeholders do OneDrive, e confere que `ALLOCSIZE == 0` e que o arquivo não
   foi materializado.
3. Teste manual documentado: "Remover download" no Finder num arquivo do iCloud.

### 3.3 Camadas

| Camada | O quê | Ferramenta (versão) | Onde |
|---|---|---|---|
| Unidade | `parse.rs` com buffers montados à mão; `ledger.rs` com números sintéticos (conta fecha, nunca negativa) | `cargo test` | qualquer macOS |
| Golden | Saída texto e JSON de um `Ledger` sintético; schema JSON | `insta` 1.48.0 (MSRV 1.66) | qualquer |
| CLI | `--help`, códigos de saída, `--json` válido | `trycmd` 1.2.1 ou `assert_cmd` 2.2.2 | qualquer |
| Integração APFS | Clones, esparsos e hardlinks em `$TMPDIR`, que é APFS no runner. `statfs`/conta na imagem `hdiutil` | stdlib + `std::process::Command` | macOS |
| Fuzz | `parse_entries(&[u8])` nunca entra em pânico nem lê fora do buffer | `cargo-fuzz` 0.13.2 + `libfuzzer-sys` 0.4.13 (nightly) | local/CI nightly |
| UB | Parser e ledger no Miri (sem syscalls) | `cargo miri test` (instalado) | local |
| Cobertura | Linhas e ramos | `cargo-llvm-cov` 0.9.1 | CI |

Todas as versões conferidas na API do crates.io em 2026-09-27 (A).

Ressalvas:

- O `hdiutil create` falha às vezes nos runners com "Resource busy"
  ([runner-images#7522](https://github.com/actions/runner-images/issues/7522), fechada, relatos
  até 2024): envolver em 3 tentativas. Clones, esparsos e hardlinks dispensam a imagem (A para o
  bug, M para a frequência).
- O `dua-cli` tem testes de "journeys" da TUI com fixtures em disco
  (`src/interactive/app/tests/`), um modelo para a fase 3 (A).

### 3.4 CI (GitHub Actions)

- Imagens em GA: `macos-26` (= `macos-latest`, arm64), `macos-15` (arm64), `-intel`/`-large`
  para x64. `macos-14` está **deprecated**. Preview: `xcode-27` (A,
  [runner-images](https://github.com/actions/runner-images)).
- Matriz: `macos-15` + `macos-26`, arm64. Um job `macos-26` com a MSRV. Um job nightly semanal
  com fuzz de 5 min.
- Runner não tem FDA: o teste de "pastas negadas" deve esperar erros em `~/Library` protegidas (M).
- Não existe runner com macOS 27 ainda (só preview do Xcode 27). Os testes no 27 rodam na máquina
  do dono (A).

---

## 4. Distribuição

| Tema | Fato | Conf. / fonte |
|---|---|---|
| cargo-dist | Última versão **v0.33.0** (GitHub, 2026-09-10); no crates.io a mais nova é 0.32.0. Gera fórmula num **tap próprio** (`installers = ["homebrew"]`, `tap = "user/homebrew-tap"`, secret `HOMEBREW_TAP_TOKEN`). Opções `cargo-auditable`, `cargo-cyclonedx` (SBOM) e `github-attestations` | A ([release](https://github.com/axodotdev/cargo-dist/releases/tag/v0.33.0), `book/src/installers/homebrew.md`, `reference/config.md`) |
| Binário universal | Issue [#77](https://github.com/axodotdev/cargo-dist/issues/77) aberta desde 2023 | A |
| Precisa de Intel? | **macOS 27 só roda em Apple Silicon**; o 26 é o último para Intel. Com alvo mínimo macOS 14, ainda existem Intels no 14–26. Recomendação: só arm64 na fase 1; `x86_64-apple-darwin` como arquivo separado, cruzado no runner arm64, se alguém pedir. Sem `lipo` | A ([AppleInsider](https://appleinsider.com/articles/26/09/02/developers-can-stop-supporting-intel-macs-says-apple)) |
| Assinatura | No arm64 o linker aplica assinatura ad-hoc automaticamente; basta para executar | M (comportamento conhecido do ld64) |
| Notarização | Obrigatória para software com Developer ID; Gatekeeper usa o ticket na primeira execução de item em quarentena. Tipos notarizáveis: apps, bundles, DMG, pkg | A ([doc Apple](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)) |
| Quarentena | **Testado aqui: `curl` não grava `com.apple.quarantine`** (só `com.apple.provenance`). Corrige a pesquisa de 26/09, que dizia "navegador/curl". Navegador grava. Fórmula de Homebrew não recebe quarentena; só cask | A (teste) / M (Homebrew) |
| Casks | Homebrew 5.0 aposentou `--no-quarantine`; cask sem assinatura e notarização sai do tap oficial em set/2026. Importa só para um app gráfico futuro | A ([brew.sh 5.0.0](https://brew.sh/2025/11/12/homebrew-5.0.0/)) |
| homebrew-core | Tem de compilar do código-fonte, licença compatível com DFSG, release estável com tag. Notabilidade: **≥30 forks, 30 watchers ou 75 ★; se o dono submete, ≥90/90/225**; repositório com menos de 30 dias normalmente não entra | A ([Package-Acceptance-Policy](https://docs.brew.sh/Package-Acceptance-Policy), [Acceptable-Formulae](https://docs.brew.sh/Acceptable-Formulae)) |
| crates.io | Publicar também (`cargo install`). Exige nome livre (seção 5) | A |
| MSRV | Edition 2024 exige ≥1.85; `clap` 4.6.7 pede 1.85; `ratatui` 0.30.2 pede 1.88. Declarar `rust-version = "1.85"` agora e subir para 1.88 com a TUI; um job de CI com a MSRV | A (crates.io) |

Recomendação: tap próprio via cargo-dist, só arm64, ad-hoc, sem os USD 99. Pedir entrada no
homebrew-core quando outra pessoa quiser submeter ou quando passar de 225 ★.

---

## 5. Licença e nome

### 5.1 Licença

- **Hoje:** MIT (`LICENSE`, 2026, Guilherme Selig).
- **MIT puro:** compatível com o `dua-cli` (MIT, "Copyright (c) 2019 Sebastian Thiel"), simples.
- **MIT OR Apache-2.0:** convenção do ecossistema Rust; a Apache-2.0 acrescenta concessão
  explícita de patente. O trecho copiado do `dua` continua MIT dentro de um projeto dual, sem
  conflito (M, leitura das licenças).
- **Recomendação:** manter MIT. A troca para dual só compensa se alguém pedir (empresas às vezes
  pedem por causa da patente).
- **Ao copiar trechos do `dua-core`:** a MIT exige manter o aviso de copyright e de permissão em
  "cópias ou partes substanciais". Na prática:
  1. Cabeçalho no arquivo:
     `// Partes adaptadas de dua-cli (https://github.com/Byron/dua-cli, commit d190513),`
     `// Copyright (c) 2019 Sebastian Thiel, licença MIT. Ver THIRD_PARTY_LICENSES.`
  2. Arquivo `THIRD_PARTY_LICENSES` com o texto MIT integral do dua.
  3. O `walk_par.rs` já cita a inspiração. Técnica sem código copiado não exige atribuição, mas
     citar é bom sinal.
- **Não copiar** código do **Mole** (GPL-3.0: obrigaria o projeto a virar GPL) nem do
  **macos-sysdata** (licença "proprietary" com código aberto à leitura) (A, API do GitHub e README).

### 5.2 Nome: verificação feita em 2026-09-27

| Nome | crates.io | Homebrew core/cask | GitHub | Veredito |
|---|---|---|---|---|
| `sdc` | **ocupado** (formato Riegl, 2015) | livre | 20 mil repos | Evitar |
| `system-data` | **ocupado** (`system_data`, 2024) | livre | 47 mil repos | Evitar |
| `sysdata` | livre | livre no core; **é o cask `Jarvis322/tap/sysdata` e o nome do CLI da ferramenta criticada pelo Oakley** | 39 repos | **Evitar**: confusão com o caso negativo |
| `systemdata`, `sysdata-cli` | livres | livres | — | Mesmo problema de associação |

Cinco nomes livres nos três lugares (crates.io API 404; `formulae.brew.sh` 404 para fórmula e
cask; busca de repositórios no GitHub):

| Nome | GitHub (repos com o nome) | Ideia |
|---|---|---|
| **`sdtally`** | 0 | "System Data tally": a conta |
| **`volledger`** | 0 | Livro-razão do volume; "fecha a conta" |
| **`apfs-ledger`** | 0 | Explícito sobre APFS (binário `apfs-ledger`) |
| **`contadisco`** | 0 | pt-BR, "conta do disco"; bom para público BR, opaco em inglês |
| **`apfsaudit`** | 0 | Auditoria só leitura |

Evitados: `diskledger` (2 repos com esse nome) e `disktally` (1 repo). Evitar também binário
`ledger`, que conflita com a fórmula `ledger` (M). Antes de decidir: registrar o nome no crates.io
com um 0.0.1 real e criar o repo, porque a verificação vale só para esta data.

---

## 6. Segurança e confiança

### 6.1 O caso criticado pelo Oakley (A)

[eclecticlight 2026-09-06](https://eclecticlight.co/2026/09/06/last-week-on-my-mac-storage-crisis-management/),
comentários. A ferramenta é
[Jarvis322/macos-sysdata](https://github.com/Jarvis322/macos-sysdata): Swift, 801 ★, criada em
2026-09-05. Oakley apontou abordagem perigosa, informação errada e o uso de um "old scripted
workaround" para privilégio em vez de um privileged helper. O que o código e o README mostram:

- `Sources/SysDataMenu/Services/Shell.swift` monta
  `do shell script "…" with administrator privileges` via `osascript`, e a confirmação mostra
  comandos como `As administrator: rm -rf /Library/Logs/DiagnosticReports/*`.
- Itens "Safe" são **apagados direto**, sem Lixo; há limpeza automática semanal opt-in.
- Mistura medição com estimativa (usa a estimativa do `docker system prune` como tamanho de item).
- Licença "proprietary" com código publicado.

Lições para o projeto:

1. **Sem root na fase de leitura.** Toda a conta é possível como usuário (seções 1–3 da
   pesquisa de 26/09).
2. **Nunca `osascript … with administrator privileges` nem `sudo` em script.** Se um dia for
   preciso root, usar o comando oficial (`tmutil`, `simctl`) digitado pelo usuário: o CLI imprime
   o comando, não executa.
3. **Nunca `rm -rf`.** Mover para o Lixo (`FileManager.trashItem`) ou delegar ao dono (já nas
   regras do README).
4. **Nenhuma ação automática ou agendada.**
5. **Nenhum número estimado**, nem vindo de outra ferramenta. Tamanho informado por uma
   ferramenta da Apple (`simctl … sizeBytes`) entra rotulado com a origem.

### 6.2 Práticas que dão confiança

| Prática | Como | Conf. |
|---|---|---|
| Só leitura por padrão | Fases 1–3 não têm código que apague. Na fase 4, `--dry-run` é o padrão (lição do Mole) e a ação exige `--apply` + confirmação por item | A (Mole README) |
| Sem telemetria e sem rede | Nenhuma dependência de rede; afirmar no README. Verificável com `cargo tree` e `cargo deny check` banindo `reqwest`/`hyper` | M |
| Auditoria de dependências | `cargo-deny` 0.20.2 (licenças, advisories, crates banidos, fontes) e `cargo-audit` 0.22.2 no CI | A (crates.io) |
| SBOM e lista embutida | `cargo-auditable` 0.7.6 embute a lista de dependências no binário; `cargo-cyclonedx` 0.5.9 gera o SBOM; os dois são opções do cargo-dist | A |
| Proveniência | `github-attestations = true` no cargo-dist; o usuário confere com `gh attestation verify` | A (config do cargo-dist) |
| Reprodutibilidade | `Cargo.lock` versionado, `--locked`, toolchain fixada em `rust-toolchain.toml`, `--remap-path-prefix` para tirar caminhos locais. Reprodutibilidade bit a bit no macOS não verificada | M/B |
| Transparência de número | `why <caminho>` mostra os atributos brutos; JSON com origem por linha | A (desenho) |
| Poucas dependências | Hoje: `libc`, `objc2-core-foundation`, `clap`, `thiserror`, `anyhow`, `serde`/`serde_json`. TUI na fase 3 | — |

---

## 7. Roadmap com critérios de aceitação

Métrica central, definida sem estimativa:

- **U = não atribuído / usado (`statfs`)**, no volume Data.
- **Máquina de referência:** a do dono, com FDA concedido ao terminal e **sem backup do TM
  rodando** (`tmutil status` → `Running = 0`).

### Fase 0: fundação (1–2 semanas)

- Pacote lib+bin, `sys.rs` / `parse.rs` separados, CI `macos-15` + `macos-26`, `cargo-deny`.
- **Aceite:**
  - O parser passa 10 min de `cargo fuzz` sem crash.
  - `cargo miri test` passa no parser.
  - A fixture `hdiutil` roda no CI com os valores exatos: esparso aloca ≤ 1 MiB; clone total
    reconhecido por `clone_id`; hardlink contado uma vez.

### Fase 1: a conta (MVP só leitura)

- Comando único, texto + `--json` (schema v1), cancelamento com parcial, pastas negadas contadas.
- **Aceite:**
  1. Na fixture, a conta fecha: usado = soma das linhas somáveis + não atribuído, **com não
     atribuído ≥ 0 e ≤ 1 MiB** (metadados do APFS), incluindo o clone parcial.
  2. Na máquina de referência, `~/Library` bate com o `walk_par` (106,2 GiB ± 0,1 %) e o tempo é
     ≤ 6 s com 8 threads (hoje 3,9 s).
  3. Pico de memória ≤ 100 MiB para 1M de entradas (`/usr/bin/time -l`); hoje 30,5 MiB para 521 mil.
  4. Nenhum arquivo dataless materializado: contagem de dataless igual antes e depois.
  5. Zero bytes gravados em disco sem `--save`, conferido com `fs_usage -w -f filesys` filtrando
     o PID (precisa de sudo; o dono roda uma vez).

### Fase 2: fechar a conta do volume Data

- Varrer o volume inteiro (`/System/Volumes/Data`, firmlinks uma vez só), purgeable pela API,
  lista de snapshots (`diskutil apfs listSnapshots`, sem tamanho).
- **Aceite:** **U < 5 % na máquina de referência, ou o relatório lista a causa medida de cada
  GB acima disso** (pastas negadas com contagem, snapshot presente, clone parcial). O relatório
  nunca rotula o não atribuído com palpite. Registrar U de cada release no changelog para ver a
  tendência.

### Fase 3: detalhamento

- `sdtally <caminho> --depth`, `why`, i18n pt-BR/en, depois TUI (ratatui 0.30.2) com arena de
  40 B/nó.
- **Aceite:**
  - TUI abre `~/Library` (521 mil entradas) com ≤ 100 MiB.
  - Navegar uma pasta com 10 mil filhos leva < 100 ms.
  - Testes de journey no estilo do dua.
  - Todo texto nos dois idiomas; um teste falha se faltar uma chave.

### Fase 4: limpeza com confirmação

- Só alvos da whitelist (`docs/04`), `--dry-run` por padrão, Lixo via `trashItem`, comando
  oficial impresso para o que pede root, `PRIVATESIZE` como "libera agora".
- **Aceite:**
  - Para cada item removido na máquina de referência, o espaço livre (`statfs`) após esvaziar o
    Lixo sobe **≥ 99 % do `PRIVATESIZE` somado**, ou o relatório explica a diferença (snapshot
    novo).
  - Zero chamadas a `rm`, `sudo` ou `osascript` no código (`grep` no CI).

### Fase 5: distribuição e comunidade

- cargo-dist 0.33 → tap próprio, crates.io, attestations, SBOM. README com FDA explicado,
  "sem telemetria", comparação honesta com dua/dust/Mole.
- **Aceite:**
  - `brew install <user>/tap/<nome>` funciona num Mac limpo sem aviso do Gatekeeper.
  - `gh attestation verify` passa.
  - `cargo deny check` limpo.

---

## 8. Incertezas e o que ficou de fora

- **FDA atribuído ao terminal:** não conferido em doc da Apple nesta sessão (M).
- **Ad-hoc automático do linker no arm64:** comportamento conhecido, sem doc lida (M).
- **Dataless só com File Provider:** inferência a partir do teste negativo (`chflags` descartado);
  pode existir SPI não pública (M).
- **Memória da TUI (~66 B/entrada):** aritmética sobre `size_of` e nome médio medidos, não
  medição de uma TUI rodando.
- **Testes:** `hdiutil attach` sem sudo testado aqui; nos runners do GitHub, só por relatos de
  terceiros (M). Snapshot APFS dentro da imagem de teste (para testar `PRIVATESIZE` preso) não foi
  testado; provavelmente exige root.
- **GrandPerspective:** não pesquisado a fundo.
- **Sem pesquisa própria desta frente:** reprodutibilidade bit a bit de binário Rust no macOS.
- **Medições com ruído:** todas feitas com backup do Time Machine em fase Copying.

## Artefatos do experimento

Scratchpad `…/scratchpad/frenteE/`:

- `target/` (build do `probe` com `--offline`)
- `sz.rs` (`size_of` do nó)
- `q.txt` (teste de quarentena)

A imagem `fx.sparseimage` foi desmontada e apagada.

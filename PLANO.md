# Plano — System Data Cleaner (29/09/2026)

Histórico: registro do plano da época; o código atual manda (estado em README.md).

Base: `research/2026-09-27-sintese.md` (medições), `research/frentes/E-produto-arquitetura.md`
(interface e roadmap originais), `research/frentes/N-daisydisk.md` (referência de produto) e o
coletor `coleta/`. Este plano atualiza a Frente E com o que foi medido depois dela.

## 1. O produto

**Posicionamento:** explica o "Dados do Sistema" do macOS com números medidos: o que mais pesa,
de quem é e quanto apagar libera de verdade. O resto sem explicação vira uma linha com o motivo.

- Não é "outro DaisyDisk". O mapa de pastas é um recurso; o produto é explicar o número da Apple.
- **Meta:** ranking medido do que mais pesa. Fechar a conta ao byte não é requisito.
- **Público:** quem abriu Ajustes > Armazenamento, viu "Dados do Sistema: 150 GB" e não sabe o
  que é.

## 2. Princípios (requisitos, não preferências)

1. **Só medição.** Todo número tem a fonte ao lado (syscall, API, log ou "diferença"). Nada de
   estimativa; o que não foi medido aparece como "não medido" com o motivo.
2. **Só leitura, sempre.** Nenhuma escrita no volume, nem de metadado. O sdtally recomenda; nunca
   apaga nem move para o Lixo.
3. **Regras de segurança da síntese §4 são obrigatórias:** `setiopolicy_np` (dataless e atime)
   desligados, não descer em dataless, não cruzar montagem, repetir em EINTR, pular enxertos,
   dir-stats só com buffer zerado, nunca tocar o destino do Time Machine.
4. **sudo + Acesso Total ao Disco** são o modo normal (medido: resto 8,61 GB sem sudo contra
   1,25 GB com). Sem sudo roda como "incompleto", marcado na saída.
5. **Linhas somáveis separadas das informativas** (ex.: "preso em snapshot" já está dentro do
   alocado).
6. **Unidade padrão GB decimal**, igual à tela do Ajustes; `--binary` para GiB.

## 3. O que a v0.1 mostra

```
$ sudo sdtally
Tela do Ajustes (reproduzida)                              GB     fonte
  macOS                                                  39,82   Σ CapacityInUse − Data
  Apple Intelligence (dentro de Dados do Sistema)         26,59   STMSize; físico 17,50
  Dados do Sistema (último valor da tela, 28/09 14:11)   166,14   log do StorageManagementService

Volume Data                                              357,91   ATTR_VOL_SPACEUSED
  O que mais pesa
    ~/Music                                               38,28   travessia
    iCloud Drive (baixado)                                36,35   travessia
    Cache do iCloud (com.apple.bird)                      22,96   travessia
    Modelos do Apple Intelligence                         17,40   dir-stats (pasta protegida)
    Snapshots locais do Time Machine (9)                  26,46   fsctl, faixa total
    ...
  Arquivos apagados ainda abertos                          3,81   lsof +L1 (chronod 2,19)
  Sem explicação                                           1,25   diferença (0,35 %)

Informativo (não somar)
  O macOS libera sozinho                                  11,07   CacheDelete, por serviço
  Clones repetidos, hardlinks, enxertos: ...
```

- `sdtally <caminho> [--top N] [--depth N]`: detalha um ramo.
- `sdtally why <caminho>`: atributos brutos de um arquivo (alocado, privado, clone, flags).
- `--json` (schema versionado, bytes inteiros, chaves em inglês); progresso em stderr.
- Idiomas pt-BR e en.

## 4. Arquitetura

Um pacote Rust `sdtally` com `lib.rs` + dois binários (`sdtally` e `sdc-coleta`). O código do
`coleta/` vira a semente da lib: ele já tem travessia, anonimização, snapshots, CacheDelete,
dir-stats, painel e lsof, testados nesta máquina.

```
src/
  lib.rs        #![deny(unsafe_code)]
  sys/          #[allow(unsafe_code)]: getattrlistbulk, fsctl, setiopolicy, CacheDelete, getpwnam
  parse.rs      parser puro do buffer do getattrlistbulk (fuzz + Miri)
  walk.rs       travessia paralela (P-cores, QoS utility), dedup hardlink/clone, enxertos
  sources/      snapshots, purgeable, painel (log + fórmulas), apagados-abertos, dir-stats
  ledger.rs     a conta: linhas somáveis, informativas, resto (aritmética pura, testável)
  owners.rs     pasta → app/dono (Frente C)
  report/       texto, JSON, i18n
  anon.rs       anonimização (usada pelo sdc-coleta)
bin/sdtally.rs, bin/sdc-coleta.rs
```

Workspace só quando o app gráfico existir (fase 4).

## 5. Fases e critérios de pronto

**Máquina de referência:** a do dono, com sudo, FDA no terminal e `tmutil status` → `Running = 0`.

### Fase 0: fundação (feita em 29/09/2026)
- Migrar `coleta/` para o pacote `sdtally` (lib + 2 bins), sem mudar a saída do `sdc-coleta`.
- Parser puro separado; fixture APFS com `hdiutil` (clone total, clone parcial, esparso,
  hardlink, comprimido); CI em GitHub Actions `macos-15`, `macos-26` e `xcode-27` (macOS 27),
  com `MACOSX_DEPLOYMENT_TARGET=15.0` conferido no binário.
- Da Frente O (`research/frentes/O-compatibilidade-apple-silicon.md` §5):
  - checagem de versão e capacidades feita uma vez e consultada por cada fonte; cada linha tem
    três estados: medida, "não disponível nesta versão", "não medida (motivo)". Nunca 0;
  - trocar os `unwrap_or(0)` que viram número falso por `Option` (CapacityInUse, Apple
    Intelligence, purgeable sem serviços) e checar o status do `plutil`;
  - `dlopen` do CacheDelete pelo caminho `Versions/A/CacheDelete`.
- **Pronto quando:** o `sdc-coleta` novo gera JSON igual ao atual (exceto tempos) na máquina de
  referência; a fixture fecha a conta com resto ≤ 1 MiB; parser passa 10 min de fuzz e Miri.

**Feita em 29/09/2026** (macOS 27.0, M4), com estas ressalvas:
- Pacote `sdtally` na raiz: `sys/` (único `unsafe`), `parse.rs`, `walk.rs`, `caps.rs`, `ledger.rs`,
  `sources/`, `anon.rs`, bins `sdc-coleta` e `sdtally` (só `--version`/`--ajuda`). Sem `report/` e
  `owners.rs`: ainda não há código deles.
- Equivalência: rodadas seguidas do binário antigo e do novo (`--sem-sudo`) com as mesmas chaves,
  exceto as novas `estado` (conta, macOS e Apple Intelligence) e `meta.capacidades`; árvore,
  snapshots e volumes com diferença < 0,04 %; mesmo histograma de errno; autoteste ok. Versão
  passou a `0.1.0-dev`.
- Fixture (`tests/fixture_apfs.rs`): resto 32.288 B (11.808 B acima do volume vazio), mas só
  depois de descontar a sobreposição do clone parcial (7.340.032 B). Sem esse desconto a conta
  dá −7,3 MB: o alocado conta duas vezes os blocos que o clone parcial divide com o original.
  Decidir na fase 1 como a conta trata isso (linha própria ou `PRIVATESIZE`).
- Fuzz: 100 mil buffers determinísticos no `cargo test`, não 10 min de `cargo-fuzz`. Miri não
  rodou: não há toolchain nightly instalada (não instalada de propósito).
- `VOL_CAP_FMT_CLONE_MAPPING` vem **desligado** no Data do 27 e mesmo assim dir-stats e a regra
  de enxerto funcionam: não serve de trava (corrige a Frente O §5 fase 1 item 3). Fica só
  registrado em `meta.capacidades`.
- CI escrito (`.github/workflows/ci.yml`), ainda não rodou: depende do push.

### Fase 1: v0.1, a conta (CLI só leitura) — aceite com sudo em 30/09/2026; macOS igual à tela ao byte em 02/10
- Tela do Ajustes reproduzida, conta do volume, ranking "o que mais pesa", snapshots, purgeable,
  apagados-abertos, pastas protegidas, `--json`, `<caminho>`, `why`, pt-BR/en.
- Por versão (Frente O §5):
  - fsctl privados do APFS (snapshot, dir-stats) só nas versões já testadas; fora delas,
    "não verificado nesta versão";
  - o log do painel fica retido por menos de ~21 h: a saída diz "abra Armazenamento antes de rodar";
  - purgeable total conferido por `kCFURLVolumeAvailableCapacityForImportantUsageKey`.
- **Pronto quando, na máquina de referência:**
  1. macOS e Apple Intelligence iguais aos da tela ao byte.
  2. Resto ≤ 1,5 % do usado, ou cada GB acima com causa medida.
  3. Zero escrita (`fs_usage` filtrando o PID, rodado pelo dono uma vez), atime e contagem de
     dataless iguais antes e depois.
  4. ≤ 60 s de ponta a ponta e ≤ 100 MiB de memória por 1 mi de entradas.
  5. Matriz publicada versão × fonte (medida, não disponível, não testada), com 15, 26 e 27 rodados no
     CI sem erro fatal.

**Código feito em 29/09/2026** (macOS 27.0, M4, sem sudo, FDA no terminal):
- Coleta na lib (`src/coleta.rs`), usada pelo `sdc-coleta` e pelo `sdtally`. JSON do `sdc-coleta`
  igual ao da Fase 0: mesmas chaves; rodadas seguidas do binário antigo e do novo com diferença
  só de tempo e do que o volume mudou entre elas.
- `sdtally` (conta, `--top`, `--binary`, `--lang`, `--json`), `sdtally <caminho> [--top] [--depth]`
  (recusa outro volume), `sdtally why <caminho>`; relatório em `src/report/` (modelo, texto, i18n)
  e `src/detalhe.rs`. Schema em `docs/json-schema.md`. Ctrl-C (sigaction → AtomicBool) imprime o
  parcial marcado INCOMPLETO (interrompido). Nenhuma escrita em disco; `unsafe` só em `sys/`.
- Medido sem sudo: 38 s de ponta a ponta (1,9 mi arquivos), pico de memória 59 MB (footprint;
  RSS máximo 398 MB), números iguais aos do `sdc-coleta` rodado em seguida (< 0,01 %, exceto
  apagados-abertos, que mudam entre rodadas). Resto 2,9–3,3 % do usado sem sudo.
- CI: roda `sdtally --json` com sudo e confere os invariantes da conta no JSON real (Σ termos +
  sem explicação = usado, Σ top = Σ termos medidos, sem erro de etapa). O snapshot com ~200 MB presos
  não sai nos runners (`tmutil localsnapshot` dá exit 73, "RPC version wrong", no 15, 26 e 27): o fsctl
  de snapshot com privado > 0 segue validado só no Mac do dono (27).
- Revisão de 35 achados aplicada em 30/09: texto em 80 colunas, snapshots/apagados/xattr sempre
  visíveis, `<0,01` no lugar de `0,00` falso, `why` de pasta sem tamanho enganoso, mensagens de
  argumento em pt/en, `--color`, resumo do `sdc-coleta` igual ao do `sdtally`, teste de `montar`
  com coleta sintética e render de drill/why no fixture.

**Ficou de fora:**
- Trava por versão dos fsctl privados ("não verificado nesta versão"): depende do CI com sudo
  confirmar 15 e 26; se confirmar, as três versões suportadas estão testadas e a trava é inútil.
- Purgeable conferido por `kCFURLVolumeAvailableCapacityForImportantUsageKey`.
- Clone parcial contado duas vezes no alocado (Fase 0): segue sem linha própria.
- Critérios 1–4 com sudo e matriz versão × fonte: dependem do aceite abaixo e do CI.
- Dados do Sistema com hora: o log não tinha registro nesta rodada; o caminho com registro só
  foi exercitado no teste de render (`tests/relatorio.rs`), não na máquina.

**Aceite com sudo (30/09/2026, 21:17, M4, macOS 27.0, FDA):** resto 0,32 % do usado (1,16 GB de
360,77 GB), invariantes fechando ao byte, nenhuma etapa com erro, `complete: true`. `fs_usage` do
começo ao fim (721 mil `getattrlistbulk`): a única escrita em disco é o `/tmp/conta.json` pelo
stdout redirecionado; fora isso só stderr, o pipe para o `plutil` e `/dev/null`. Apagados-abertos
medido com root (0,32 GB, parcial). 83 s com o `fs_usage` rastreando 5,5 mi eventos. Na 1ª rodada
(09:49, backup do Time Machine copiando para o destino SMB) o `lsof` estourou 60 s; corrigido com
`lsof -b -w`, ainda não reconfirmado durante um backup.

**Aceite final (o dono roda, com sudo):** abra Ajustes > Geral > Armazenamento e espere carregar;
num terminal `sudo fs_usage -w -f filesys sdtally > /tmp/fsu.txt`, noutro
`sudo sdtally > /tmp/conta.txt && sudo sdtally --json > /tmp/conta.json`, pare o fs_usage
(Ctrl-C) e confira:
- zero escrita: `grep -E 'WrData|write|pwrite|O_CREAT|mkdir|unlink|rename|setattrlist|chmod|chown|utimes|truncate' /tmp/fsu.txt`
  vazio (o `> /tmp/conta.*` é do shell, não do sdtally);
- resto ≤ 1,5 %: `python3 -c "import json;print(json.load(open('/tmp/conta.json'))['data_volume']['unexplained'])"`;
- macOS e Apple Intelligence de `/tmp/conta.txt` iguais aos da tela do Ajustes.

### Fase 2: o que é "Dados do Sistema" especificamente
- **Pesquisa nova:** quais pastas cada categoria do Ajustes conta (Apps, Documentos, Música,
  Fotos, Developer, iCloud Drive, Mail...). Engenharia reversa das extensões `STMExtension.*`,
  como a Frente M fez com macOS e Apple Intelligence.
- Com isso, a saída marca cada linha como "está em Dados do Sistema" ou "está em <categoria>".
- Atribuição por app (Frente C): cada balde com o app dono. **Feita (18/10/2026):** `src/owners.rs`,
  `top[].owner` no JSON (`docs/json-schema.md`) e o nome do app no app (Pastas maiores, Início).
- Comparação com os relatórios dos amigos: o que se repete entre Macs vira regra de produto.
- **Pronto quando:** Σ das categorias reproduzidas bate com o log do serviço (±1 %) e
  "Dados do Sistema" é decomposto com resto como uma linha.
- **Estado (18/10/2026):** `src/sources/categorias/` reproduz as 15 categorias; `system_data.by_line`
  e `system_data.reproduced` no JSON (`docs/json-schema.md`, "Fase 2"); o app mostra a Σ das pastas
  em Dados do Sistema. Contra o frenteq no mesmo minuto: Σ +0,13 %, Fotos +1,86 % (B). Falta o dono
  casar com o log do painel, o iCloud Drive (banco do File Provider) e Livros por livro.

- **"O que ainda não é medido" (18/10/2026):** Lixo com mais de 30 dias (`informational.trash`, pela
  `ATTR_CMN_ADDEDTIME`, só `~/.Trash` do volume Data) e metadados do APFS (`informational.apfs_metadata`,
  nós das árvores vivas × bloco, seletores 6/7/0x1e do `AppleAPFSUserClient`, sem root, só conferido
  no 27) saíram de "não medido"; Preboot/Recovery/VM e o container de recuperação aparecem com o total
  do `diskutil` e a marca "entra no macOS, não em Dados do Sistema". Seguem não medidos: o conteúdo de
  Preboot e Recovery, os metadados dos snapshots, o Lixo de volume externo.

### Fase 3: recomendações
- Recomenda, não apaga: para cada item, o que fazer e quanto libera; quem executa é o usuário.
- Só itens da lista segura; recomenda comandos oficiais com simulação (`brew cleanup -n`,
  `simctl ... --dry-run`); snapshots só pela forma por data.
- Lista de pastas que nunca recomenda apagar; texto "por que o espaço não voltou".
- **Pronto quando:** depois que o usuário remove um item recomendado, o espaço livre sobe ≥ 99 %
  do privado somado ou a saída explica a diferença; zero `rm` e `trashItem` no código (grep
  no CI).
- **Feito em 18/10/2026:** `src/voltou.rs` ("por que o espaço pode não voltar": só causas medidas,
  na CLI em texto, em `not_freed` do JSON e em O que fazer no app); `tests/nunca_apaga.rs` (Rust e
  Swift, no `cargo test`, no lugar do grep do CI); `conselho::SIMULACOES` e `tests/dryrun.rs`
  (dry-run conferido por comando). Ainda aberto: conferir no Mac o "pronto quando" do espaço livre.

### Fase 4: app gráfico — protótipo em 02/10/2026
- Decidido em 02/10: **SwiftUI chamando o binário `sdtally` embutido** e lendo o JSON dele (a lib
  continua a única fonte de número); senha de admin pelo `osascript` para medir como root (helper
  SMAppService depois); janela com barra lateral.
- Protótipo em `app/` (spec do painel de design de 3 direções; venceu "a sobra, desdobrada"):
  Resumo (barra do Ajustes pelo log da própria tela, conta usado − macOS − outras linhas = Dados do
  Sistema, conta do volume), O que mais pesa (ranking + detalhe de pasta) e Sobre esta medição.
  `app/montar.sh` monta `~/Library/Caches/sdtally-build/SDTally.app` (ad hoc); `swift test`.
- Falta conferir com o dono: senha pelo osascript (aparência do diálogo, `SUDO_USER`, se o root
  herda o Acesso Total ao Disco do app) e a tela no 15/26 (job `app` do CI).
- Fica para depois: dividir Dados do Sistema por linha (Fase 2), `why`/inspector, FSEvents,
  cache da última medição, helper privilegiado, String Catalog/inglês.

### Fase 5: distribuição
- cargo-dist, tap próprio, arm64, crates.io, attestations. README com FDA e sudo explicados,
  "sem telemetria", comparação honesta com DaisyDisk, dua, dust e Mole.

## 6. Em paralelo

- **Coleta com amigos:** `coleta/dist/sdc-coleta` + `coleta/LEIAME.md`. Cada relatório entra em
  `research/coletas/` (gitignored se o dono preferir) e alimenta a fase 2.
- **Perguntas abertas** (síntese §10): SYS do serviço, arredondamento 39,81/39,82, semântica de
  `CACHE_DELETE_SHARED_PURGEABLE`, "Otimizar armazenamento".

## 7. Decisões do dono (29/09)

| Decisão | Escolha |
|---|---|
| Nome | **`sdtally`** (livre no crates.io e no Homebrew em 27/09; reconferir antes de publicar) |
| Notarização | Não por agora; os amigos usam `xattr -d com.apple.quarantine`. Rever na fase 5 |
| Plataforma | **Só Macs com Apple Silicon** (arm64), **macOS 15 Sequoia, 26 e 27**. Versões anteriores não são suportadas |
| Licença | MIT (proposta) |
| Unidade padrão | GB decimal, igual ao Ajustes (proposta) |
| Apagar (03/10) | **Nunca.** App e CLI só leem e recomendam; não apagam nem movem para o Lixo |

**Consequência do macOS 15+:** todas as fontes da Frente O existem nas três versões, exceto o
Apple Intelligence (15.1+), que sai "não disponível nesta versão" no 15.0. Testes: CI com
`macos-15`, `macos-26` e `xcode-27`, sem VM. As fontes privadas do APFS (snapshot, dir-stats)
ainda precisam ser conferidas no 15 e no 26.

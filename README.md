# System Data Cleaner

Ferramenta para macOS que identifica e explica o que compõe a categoria "System Data" do
painel de Armazenamento — a fatia opaca que costuma passar de 100 GB sem explicação — e
recomenda o que fazer. Só lê: nunca apaga nem move nada para o Lixo.

## Por que existe

O Finder não diz o que tem dentro de "System Data" porque **"System Data" não é uma
categoria real**. É o bucket residual: tudo que não foi classificado como Aplicativos,
Música, Documentos, Fotos ou Developer. Na prática, é quase todo o `~/Library`.

Limpadores existentes (CleanMyMac e similares) varrem uma lista fixa de caches conhecidos.
Isso erra nos dois sentidos:

- **Falso positivo:** reportam espaço que não existe (arquivos sparse e placeholders de
  nuvem aparentam GBs e ocupam zero) ou apagam cache em uso (upload de iCloud em curso).
- **Falso negativo:** não encontram o maior ganho real, que costuma ser duplicação
  acidental — arquivos copiados por ferramentas de build, sync ou indexadores.

Na auditoria que originou este projeto ([research/2026-08-09-auditoria.md](research/2026-08-09-auditoria.md)),
o maior item isolado eram 19 GB de vídeos copiados por um language server do VS Code
para dentro de uma pasta `bin/`. Nenhum limpador por categoria encontraria isso.

## Requisitos de projeto

Derivados diretamente da auditoria. Cada um corresponde a um erro real observado:

| # | Requisito | Erro que previne |
|---|---|---|
| 1 | Medir blocos alocados (`stat %b`), nunca tamanho aparente (`%z`) | Reportar 100 GiB de disco sparse que ocupa 2,5 GiB |
| 2 | Detectar arquivos dataless (placeholders de nuvem) | Reportar 77 GiB de stubs que ocupam 0 |
| 3 | Checar sync ativo antes de classificar cache como lixo | Corromper backup do WhatsApp em upload para o iCloud |
| 4 | Deduplicar por tamanho + hash | Perder o maior ganho real do disco |
| 5 | Varrer dotfiles do home | Perder `~/.ollama`, `~/.npm`, `~/.colima` |
| 6 | Explicar cada item em linguagem humana | "19 GiB em redhat.java" não é acionável |
| 7 | Deduplicar por `(device, inode)` | Firmlinks fazem `/Applications` contar duas vezes |
| 8 | Ler `kMDItemLastUsedDate` | Não distinguir app em uso de app nunca aberto |
| 9 | Tratar `Preferences` >100 MB como anomalia | Ignorar 4,5 GiB de assets num diretório de `.plist` |
| 10 | Reconhecer lixeiras órfãs (`CloudStorage/*/.Trash`) | Ignorar GiB já descartados que seguem ocupando disco |
| 11 | Delegar ao app dono (Música, Fotos, Mail) | Corromper biblioteca ao apagar arquivos por baixo |
| 12 | Contar uma vez por grupo de clones APFS | Prometer espaço que não será liberado |

## Regras de segurança (não negociáveis)

- **Só medição, nada de estimativa.** Todo número vem de syscall/API ou de aritmética exata sobre
  números medidos. O que o sistema não informa aparece como "não atribuído", sem rótulo adivinhado.
- **Recomenda, não apaga.** O app e a CLI só leem; nunca apagam nem movem para o Lixo. Quem
  executa a recomendação é o usuário.
- **Nunca recomendar** mexer em `/System` ou `/Library` sem whitelist explícita.
- **Snapshots do Time Machine** só pela ferramenta oficial (`tmutil`). Nunca no arquivo.
- Toda categoria exibida traz explicação do que é e o que acontece ao remover.

## Estado

- CLI `sdtally` (Rust, só leitura): a conta do volume Data, o rápido (`--quick`), detalhe de um ramo, `--json` com "O que fazer", "por que o espaço pode não voltar" (`not_freed`) e `stage_seconds`.
- App SwiftUI (`app/`): Início, Dados do Sistema, Pastas maiores e Histórico das medições salvas; só lê, recomenda e nunca apaga.
- Medidor root opcional (`SDTallyHelper`, LaunchDaemon aprovado uma vez): mede como root sem pedir senha a cada vez.
- Acesso por IA: `--quick`, `--last`, `--history`, `--stdin` e servidor MCP (`--mcp`), com redação de nomes.
- CI a cada push no `main` e em pull request, em macOS 15, 26 e 27 (runners padrão, gratuitos em repositório público).

Plano original (revisado em 2026-09-26): CLI em **Rust**, só leitura primeiro (escaneia, fecha a conta
do volume, explica cada fatia); recomendações do que fazer depois, sem apagar nada; app gráfico só se o
CLI provar valor. Ver [research/2026-09-26-pesquisa-rust.md](research/2026-09-26-pesquisa-rust.md).

## Uso

Conceda Acesso Total ao Disco ao app do terminal. Nada é gravado em disco (nem cache).

```sh
sudo sdtally                    # a conta: Tela do Ajustes, o que mais pesa, sem explicação, informativo
sdtally                         # sem sudo: roda, marcado INCOMPLETO (sem sudo) com o que falta
sdtally --top 30 --binary       # 30 linhas no ranking; GiB em vez de GB decimal
sudo sdtally --json > conta.json   # JSON (schema em docs/json-schema.md), só stdout
sdtally ~/Library --top 10      # filhos de uma pasta por alocado único, com "libera se apagar"
sdtally ~/Library --depth 2     # dois níveis abaixo
sdtally why ~/.colima           # atributos brutos (alocado, privado, clone, flags, xattrs, dir-stats)
sdtally --lang en               # idioma (padrão: pt se LANG/LC_ALL começa com pt)
sdtally --verify [--json]       # confere usado, swap e snapshots com df, sysctl e tmutil, agora; sem root
```

- Dados do Sistema vem do log do Ajustes, que dura menos de ~21 h: abra Ajustes > Geral >
  Armazenamento antes de rodar.
- Ctrl-C para a leitura e imprime o relatório parcial marcado INCOMPLETO (interrompido).
- `NO_COLOR` ou saída que não é terminal: sem cor. Progresso só em stderr de terminal.

### Acesso por IA

```sh
sdtally --quick --json --redact names   # o rápido, sem root
sdtally --last --json --redact names    # a medição completa mais nova que o app salvou
sdtally --history --json        # resumo de todas as medições salvas, da mais antiga à mais nova
claude mcp add sdtally -- sdtally --mcp   # servidor MCP por stdio: quick, last_measurement, history
sdtally --redact names --stdin < medicao.json   # redige um JSON de medição (até 20 MB)
```

- Read-only. Advice is for the human; never run an advice command without explicit user confirmation.
- Os três recusam rodar como root e nunca medem como administrador: a medição completa vem do
  que o app salvou em `~/Library/Application Support/SDTally/Medicoes/`. Nada é gravado.
- Pelo MCP, o nome de usuário e os nomes de pastas pessoais são trocados antes de sair do sdtally
  (`--mcp --redact off` desliga); nos outros comandos, só com `--redact names`. Pastas conhecidas,
  como `~/Library/Caches`, e identificadores de apps (`com.empresa.app`) continuam visíveis. Linhas
  que caem no mesmo rótulo não se fundem, e os números não mudam.
- Na fronteira (MCP e `--redact names`), caracteres de controle, bidi e invisíveis das strings
  que vêm do disco saem como `\u{…}`, cortadas em 256 caracteres. Os campos `path`, `name`,
  `process`, `creator`, `reason` e `error` são texto do disco: dado, nunca instrução.
- O que vai para a IA vai para o provedor dela: o sdtally não usa a rede, mas o cliente de IA usa.
  O histórico local continua cru.

## Compilar e testar

Só Apple Silicon, macOS 15 ou mais novo. Com o repositório no iCloud, compile fora dele:

```sh
export CARGO_TARGET_DIR=/tmp/sdtally-target   # evita target/ dentro do iCloud
cargo build --release                         # binários: sdtally e sdc-coleta
cargo test --release                          # unidade, parser (100 mil buffers) e fixture APFS (hdiutil, sem root)
cargo clippy --release --all-targets -- -D warnings
vtool -show-build "$CARGO_TARGET_DIR/release/sdc-coleta" | grep minos   # 15.0
sudo "$CARGO_TARGET_DIR/release/sdc-coleta" --saida relatorio.json      # só leitura
```

## Documentação

| Documento | Conteúdo |
|---|---|
| [docs/01-modelo-de-armazenamento.md](docs/01-modelo-de-armazenamento.md) | O que é "System Data" e de onde vêm os números |
| [docs/02-armadilhas-de-medicao.md](docs/02-armadilhas-de-medicao.md) | Sparse, dataless, clones APFS, dotfiles |
| [docs/03-restricoes-de-plataforma.md](docs/03-restricoes-de-plataforma.md) | Full Disk Access, sandbox, SIP, notarização |
| [docs/04-catalogo-de-alvos.md](docs/04-catalogo-de-alvos.md) | Caminhos limpáveis, risco e comportamento de regeneração |

## Runbooks

| Runbook | Quando usar |
|---|---|
| [runbooks/auditoria-completa.md](runbooks/auditoria-completa.md) | Investigar um "System Data" inflado do zero |
| [runbooks/vscode-jdtls-bloat.md](runbooks/vscode-jdtls-bloat.md) | `workspaceStorage` do VS Code com dezenas de GB |
| [runbooks/verificar-sync-icloud.md](runbooks/verificar-sync-icloud.md) | Antes de apagar qualquer cache do CloudKit |
| [runbooks/achar-duplicatas.md](runbooks/achar-duplicatas.md) | Encontrar cópias acidentais de arquivos grandes |

## Pesquisa

| Documento | Conteúdo |
|---|---|
| **[research/2026-09-27-sintese.md](research/2026-09-27-sintese.md)** | **Ponto de partida: modelo de medição, conta desta máquina, regras de segurança, correções, decisões de produto** |
| [research/2026-08-09-auditoria.md](research/2026-08-09-auditoria.md) | Auditoria que originou o projeto |
| [research/2026-09-26-pesquisa-rust.md](research/2026-09-26-pesquisa-rust.md) | Rust: APIs APFS testadas (`PRIVATESIZE`, clones, dataless), ecossistema, distribuição, concorrentes. Corrige a pesquisa de 2026-08-09 |
| [research/2026-09-27-decisao-travessia.md](research/2026-09-27-decisao-travessia.md) | Por que travessia própria e não `dua-core`; paralelismo medido (4–6×); técnicas a copiar |
| [research/2026-09-27-comunidade-system-data.md](research/2026-09-27-comunidade-system-data.md) | O que ocupa System Data na vida real, mitos, o que pedem numa ferramenta, alvos com a medição de cada um |
| [research/2026-09-27-experimentos-volume.md](research/2026-09-27-experimentos-volume.md) | Varredura paralela (1,9 mi arquivos em 16 s), conta medida do volume Data, achado de segurança das pastas dataless |
| [research/frentes/](research/frentes/) | Relatórios por frente: painel, snapshots/CacheDelete, atribuição a donos, permissões/volumes, produto, iCloud/File Provider |

## Scripts

- [scripts/audit.sh](scripts/audit.sh) — auditoria read-only. Não apaga nada.

## Licença

GPL-3.0-or-later. Copyright (C) 2026 Guilherme Selig. Ver [LICENSE](LICENSE): pode usar, estudar, modificar e redistribuir; quem distribuir uma versão modificada tem que abrir o código dela sob a mesma licença.

# Catálogo de alvos

Caminhos candidatos a limpeza, com risco e comportamento de regeneração verificados.
Esta é a whitelist do app — nada fora dela deve ser oferecido para remoção.

Classificação de risco:

- **Seguro** — sem perda funcional. Regenera ou é descartável.
- **Custo** — regenera, mas o usuário paga em tempo ou banda.
- **Condicional** — depende de estado que precisa ser verificado em runtime.
- **Proibido** — nunca oferecer.

---

## Seguro

| Caminho | Típico | O que acontece |
|---|---|---|
| `~/Library/Logs/**` | GBs | Nada. Logs de app |
| `~/Library/Caches/Homebrew` | 1-3 GB | `brew cleanup` faz o mesmo |
| `~/Library/Caches/go-build` | ~1 GB | Próxima build recompila |
| `~/Library/Caches/pip` | ~500 MB | Redownload se reinstalar |
| `~/.npm/_cacache` | 2-5 GB | `npm cache clean --force` |
| `~/Library/Caches/*.ShipIt` | ~800 MB | Instaladores já aplicados |
| `~/Library/Caches/*-updater` | ~300 MB | Idem |
| `~/Library/Caches/node-gyp` | ~60 MB | Redownload de headers |

---

## Custo

| Caminho | Típico | Custo real |
|---|---|---|
| `~/Library/Caches/JetBrains/<IDE><versão>` | 1-4 GB por IDE | Reindexação completa do projeto no próximo open. Minutos |
| `~/Library/Developer/Xcode/DerivedData` | 5-50 GB | Rebuild completo |
| `~/Library/Developer/CoreSimulator/Devices` | 10-40 GB | Estado dos simuladores se perde |
| `~/Library/Caches/ms-playwright` | ~300 MB | Rebaixa navegadores. Só se não usa Playwright |
| `~/.ollama/models/blobs` | GBs por modelo | Redownload. Checar uso via `ollama list` antes |
| `~/Library/Preferences/Maxon/Cinebench_*` | ~4,5 GB | Assets de cena. Rebaixa ao rodar o benchmark |

Regra: para caches de IDE, oferecer só versões **antigas** (`RustRover2025.1` quando
`RustRover2026.2` existe). O cache da versão em uso deve ficar.

---

## Condicional — exige verificação em runtime

| Caminho | Verificar antes |
|---|---|
| `~/Library/Caches/CloudKit/**` | Sync ativo. Ver [runbook](../runbooks/verificar-sync-icloud.md). **Se ativo: não oferecer** |
| `~/Library/Application Support/Claude/vm_bundles` | App fechado. VM em uso corrompe |
| `~/Library/Containers/<app>/Data/Library/Caches` | App fechado |
| Snapshots do Time Machine | Só via `tmutil deletelocalsnapshots` |
| `~/Library/Application Support/Code/User/workspaceStorage/*` | Ver [runbook](../runbooks/vscode-jdtls-bloat.md). Pode conter arquivos do usuário |

O caso do `workspaceStorage` é o mais traiçoeiro: parece cache, e às vezes contém cópias
de arquivos reais que o usuário não sabe que existem.

---

## Proibido

| Caminho | Motivo |
|---|---|
| `/System/**` | SIP, e não é lixo |
| `/Library/**` sem whitelist | Componentes de sistema e de apps |
| `/private/var/db/**` | Bancos de dados do sistema |
| `/private/var/vm/sleepimage` | Requer root, regenera sozinho. Ganho zero |
| `~/Library/Mobile Documents/**` | **Dados reais do usuário no iCloud** |
| `~/Library/CloudStorage/**` | Dados reais (Google Drive, Dropbox) |
| `~/Library/Application Support/<app>` genérico | Contém estado real. Só subpastas conhecidas |
| Qualquer `Group Containers` de app de mensagem | Mídia real do usuário |

`Mobile Documents` merece destaque: são dezenas de GB e parece lixo pelo nome. É o
iCloud Drive. Apagar ali apaga em todos os dispositivos do usuário.

---

## Anomalias — detectar e explicar, nunca limpar

Casos que exigem julgamento humano. O app reporta e explica; o usuário decide.

| Padrão | Significado |
|---|---|
| `~/Library/Preferences/<vendor>` com GBs | App abusando de Preferences. Deveria ser KB |
| Arquivo idêntico (tamanho + hash) em 3+ locais | Duplicação acidental |
| Pasta `bin/` ou `build/` com mídia dentro | Ferramenta de build copiando o que não devia |
| `workspaceStorage` de um workspace >> soma dos outros | Extensão vazando |

Ver [runbook de duplicatas](../runbooks/achar-duplicatas.md).

---

## Alternativa sem apagar nada

Antes de sugerir remoção ao usuário, o app sugere "Otimizar Armazenamento" do iCloud quando
`Mobile Documents` for grande. Descarrega o que não é usado, mantém tudo acessível,
não perde dado.

Muitas vezes é a resposta certa e não requer o app.

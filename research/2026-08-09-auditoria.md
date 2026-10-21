# Auditoria de referência — 2026-08-09

Auditoria real que originou o projeto. Serve como caso de teste: qualquer versão do
scanner deve reproduzir estes achados e evitar estes erros.

Identificadores pessoais (telefone, e-mail, nomes de arquivo) foram substituídos por
placeholders. Os tamanhos e caminhos estruturais são os reais.

## Máquina

- macOS 27, Apple Silicon
- 494 GB total, 437 GB usados
- **System Data reportado pelo Finder: 166,68 GB**

Distribuição do painel de Armazenamento:

| Categoria | Tamanho |
|---|---|
| Applications | 146,85 GB |
| System Data | 166,68 GB |
| Music | 45,34 GB |
| Documents | 35,79 GB |
| macOS | 31,09 GB |
| Photos | 9,13 GB |
| Developer | 2,05 GB |
| iCloud Drive | 201,3 MB |
| Mail | 499,3 MB |

Nota: "iCloud Drive 201,3 MB" contra 36 GB reais em `Mobile Documents/com~apple~CloudDocs`.
O painel conta apenas o que **não** está baixado localmente. O conteúdo baixado cai em
System Data.

---

## Reconciliação dos 166,68 GB

| Bloco | Tamanho |
|---|---|
| `~/Library` | 153 GB |
| `/private/var` | 6,0 GB |
| `/opt/homebrew` | 5,8 GB |
| `/Library` | 5,0 GB |
| `/usr/local` | 2,0 GB |
| **Soma** | **~172 GB** |

Fecha dentro da margem GB/GiB. **System Data ≈ `~/Library`.**

---

## Achado principal: 19 GB de vídeo dentro do VS Code

```
~/Library/Application Support/Code/User/workspaceStorage/<hash>/redhat.java/jdt_ws/Downloads_227f6993/bin/
├── <show>.mov   10.295.731.258 bytes
└── <show>.m4v   10.295.731.220 bytes
```

A pasta `Downloads` foi aberta como projeto no VS Code. O JDT Language Server (`redhat.java`)
a interpretou como projeto Java e copiou o conteúdo para a pasta de build `bin/`.

Evidências de abandono:

- `client.log.2026-03-25` — último log
- mtime do `jdt_ws`: 25/mar/2026
- arquivos tocados nos últimos 60 dias: **0**
- workspaces irmãos: 21 pastas somando 150 MB; esta sozinha tem 19 GB

Runbook: [vscode-jdtls-bloat.md](../runbooks/vscode-jdtls-bloat.md)

---

## O mesmo arquivo em 5 lugares — 51 GB

| Caminho | Tamanho |
|---|---|
| `~/Library/Mobile Documents/com~apple~CloudDocs/Movies/<show>.mp4` | 10,29 GB |
| `~/Movies/TV/Media.localized/Home Videos/<show>.mp4` | 10,29 GB |
| `~/Music/Music/Media.localized/Music/<artista>/<album>/<show>.m4v` | 10,29 GB |
| `.../workspaceStorage/<hash>/.../bin/<show>.mov` | 10,29 GB |
| `.../workspaceStorage/<hash>/.../bin/<show>.m4v` | 10,29 GB |

Explica boa parte dos 45 GB da categoria Music. Três cópias estão em pastas legítimas —
nenhum limpador por categoria as encontraria.

Runbook: [achar-duplicatas.md](../runbooks/achar-duplicatas.md)

---

## Falsos positivos evitados

Cada um teria virado uma recomendação errada.

### Disco sparse do colima — 100 GB que não existem

```
~/.colima/_lima/_disks/colima/datadisk
  %z (aparente): 107.374.182.400   → 100 GB
  %b (blocos):   5.191.424 × 512   → 2,66 GB
~/.colima total (du): 3,5 GB
```

### Placeholders do OneDrive — 72 GB que não existem

Dois `.mkv` de 35,9 GB (mesmo arquivo em `OneDriveSyncClientSuite` e
`OneDriveStandaloneSuite`). `du` das pastas-pai: 1,4 GB e 370 MB. São stubs dataless.

### Cache do CloudKit — risco alto, ganho incerto

```
~/Library/Caches/CloudKit/com.apple.bird/<hash>/MMCS/ClonedFiles/   18 GB
```

- 62 arquivos, todos com mtime do dia anterior
- `bird` e `cloudd` em execução
- Tamanhos casam byte a byte com os backups do WhatsApp em
  `Mobile Documents/<container-whatsapp>/Accounts/<id>/backup/{Video,Media}.tar`

Eram uploads em curso. E, sendo clones APFS, apagar provavelmente liberaria zero espaço.

Runbook: [verificar-sync-icloud.md](../runbooks/verificar-sync-icloud.md)

### `sleepimage` — regenera

2 GB reais, mas volta na próxima suspensão. Requer root. Ganho líquido: zero.

---

## Erro de método corrigido durante a auditoria

O primeiro scan usou `du -sh ~/*`, que não expande dotfiles. Ficaram de fora:

| Caminho | Tamanho |
|---|---|
| `~/.ollama` | 8,9 GB |
| `~/.colima` | 3,5 GB |
| `~/.npm` | 2,8 GB |
| `~/.local` | 2,4 GB |
| `~/.rustup` | 1,5 GB |
| `~/.vscode` | 1,3 GB |

~21 GB invisíveis. Correção: `du -sh ~/.[a-zA-Z]* ~/*`.

---

## Resultado final

**Seguro, sem perda funcional — 35,7 GB**

| Item | GB |
|---|---|
| `redhat.java/jdt_ws` (cópias de vídeo) | 19,0 |
| Caches JetBrains (5 IDEs) | 7,4 |
| `~/.npm` | 2,8 |
| Logs `diagnosticextensionsd` (6.904 arquivos) | 2,7 |
| Cache Homebrew | 1,4 |
| `go-build` | 0,8 |
| `ShipIt` (instaladores aplicados) | 0,8 |
| `pip` | 0,5 |
| `antigravity-updater` | 0,3 |

**Decisão do usuário — +13,4 GB**

| Item | GB | Nota |
|---|---|---|
| Modelos ollama (2) | 8,9 | Sem uso há 3 meses |
| Assets do Cinebench (em `Preferences/Maxon`) | 4,5 | Rebaixa ao rodar |

**Duplicatas — +20,6 GB** (manter 1 das 3 cópias legítimas)

**Total: 35,7 GB seguro / 69,7 GB máximo.**

---

## Pendências

- `Applications` = 146,85 GB. Sete IDEs JetBrains somam ~26 GB. Não investigado.
- `~/Library/Application Support/Claude/vm_bundles` = 6 GB (`rootfs.img` não é sparse).
  Classificado como condicional, não auditado a fundo.
- Nível de sistema medido sem `sudo` — números possivelmente subestimados.

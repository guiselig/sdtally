# Armadilhas de medição

Quatro formas de o disco mentir sobre seu próprio tamanho. Todas foram observadas na
auditoria de 2026-08-09. Um scanner que ignora qualquer uma delas produz relatório errado.

---

## 1. Arquivos sparse

Um arquivo sparse declara tamanho grande mas só aloca os blocos efetivamente escritos.
Discos de VM são o caso clássico.

**Observado:**

```
~/.colima/_lima/_disks/colima/datadisk
  aparente (%z): 107.374.182.400 bytes  (100 GB)
  real     (%b): 5.191.424 blocos × 512 = 2,66 GB
```

Um scanner ingênuo reportaria "100 GB recuperáveis em colima". Existem 2,66 GB.

**Detecção:**

```bash
stat -f 'aparente=%z real_blocos=%b %N' <arquivo>
```

`%b` é o número de blocos de 512 bytes efetivamente alocados. **Esta é a fonte da verdade.**

Em Swift: `URLResourceKey.totalFileAllocatedSizeKey` (não `fileSizeKey`).

Nota: `du` sem flags já usa blocos alocados e acerta. `du --apparent-size` e qualquer
código que some `stat().st_size` erram.

---

## 2. Arquivos dataless (placeholders de nuvem)

OneDrive, iCloud Drive e Google Drive usam Files On-Demand: o arquivo aparece no Finder
com tamanho completo mas o conteúdo não está no disco. O macOS marca esses inodes com a
flag `SF_DATALESS`.

**Observado:**

```
~/Library/Group Containers/UBF8T346G9.OneDriveSyncClientSuite/.../<show-2025>.mkv
~/Library/Group Containers/UBF8T346G9.OneDriveStandaloneSuite/.../<show-2025>.mkv
  aparente: 35,9 GB cada  →  72 GB em dois "arquivos"
  du das pastas-pai: 1,4 GB e 370 MB
```

Os 72 GB não existem. São stubs.

**Armadilha secundária, mais grave:** ler um arquivo dataless (inclusive só para calcular
hash) **dispara o download**. Um deduplicador que hasheia tudo pode baixar centenas de GB
e encher o disco que deveria estar limpando.

**Detecção:**

```bash
ls -lO <arquivo>   # a flag aparece na coluna de flags
```

Em Swift: `URLResourceKey.ubiquitousItemDownloadingStatusKey`, ou checar `st_flags & SF_DATALESS`
via `stat(2)`.

**Regra: pular arquivos dataless na varredura. Nunca abrir para leitura.**

---

## 3. Clones APFS

`clonefile(2)` cria um arquivo novo que compartilha blocos com o original (copy-on-write).
Ambos reportam tamanho completo em `stat`, mas ocupam o espaço uma vez só.

**Observado:**

```
~/Library/Caches/CloudKit/com.apple.bird/<hash>/MMCS/ClonedFiles/
  documentContent__<hash-1>..._fileContent   10,6 GB
  documentContent__<hash-2>..._fileContent    5,4 GB
```

Casam byte a byte com `Mobile Documents/57T9237FN3~net~whatsapp~WhatsApp/.../backup/Video.tar`
e `Media.tar`. São clones dos uploads pendentes para o iCloud.

**O problema:** não há syscall que responda "este arquivo é um clone e quanto ele
economiza". `stat` reporta `st_nlink = 1` e blocos completos para ambos. `du` conta os dois.

**Implicação prática:** apagar o clone pode liberar zero espaço, porque os blocos seguem
referenciados pelo original.

**Mitigação viável:** heurística por nome e localização (`ClonedFiles`, caches de daemons
de sync) + comparação de tamanho exato com candidatos a original. Marcar como
"economia incerta" no relatório em vez de somar ao total prometido.

Não prometa espaço que talvez não exista. O usuário confia no número.

---

## 4. Dotfiles

`du -sh ~/*` **não expande arquivos ocultos**. Globbing de shell ignora entradas que
começam com ponto, salvo `dotglob`/`GLOB_DOTS`.

**Observado (o que ficou de fora do primeiro scan):**

| Caminho | Tamanho |
|---|---|
| `~/.ollama` | 8,9 GB |
| `~/.colima` | 3,5 GB |
| `~/.npm` | 2,8 GB |
| `~/.local` | 2,4 GB |
| `~/.rustup` | 1,5 GB |
| `~/.vscode` | 1,3 GB |

Quase 21 GB invisíveis. Em máquina de desenvolvedor, cache de ferramenta mora em dotfile
por convenção — é exatamente onde o lixo se acumula.

**Correção:**

```bash
du -sh ~/.[a-zA-Z]* ~/*     # o primeiro padrão pega dotfiles
```

`~/.[a-zA-Z]*` em vez de `~/.*` evita casar `.` e `..`.

Em Swift: `FileManager.contentsOfDirectory` já retorna ocultos por padrão. Só não passe
`.skipsHiddenFiles`.

---

## Resumo para implementação

| Armadilha | Detecção | Ação |
|---|---|---|
| Sparse | `%b` vs `%z` | Usar sempre `%b` / `totalFileAllocatedSizeKey` |
| Dataless | `SF_DATALESS` | Pular. Não ler, sob risco de baixar |
| Clone APFS | Heurística de caminho + tamanho | Marcar "economia incerta" |
| Dotfile | — | Não usar `skipsHiddenFiles` |

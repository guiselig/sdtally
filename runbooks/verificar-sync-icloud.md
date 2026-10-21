# Runbook: verificar sync do iCloud antes de limpar cache

**Rodar sempre antes de tocar em `~/Library/Caches/CloudKit/**`.**

Apagar cache de upload em andamento pode corromper a transferência e forçar reenvio —
ou perder dados que ainda não chegaram ao servidor.

---

## Sintoma que motiva a checagem

```
~/Library/Caches/CloudKit/com.apple.bird/<hash>/MMCS/   18 GB
```

Está em `Caches`. Parece lixo. Frequentemente não é.

`com.apple.bird` é o daemon do iCloud Drive. `MMCS` é o Mobile Media Content Service —
a camada de transferência de arquivos grandes.

---

## Verificação

### 1. Daemons rodando

```bash
pgrep -l bird cloudd
```

### 2. Estado do sync

```bash
brctl status | head -40
```

Procure por:

- `sync:idle` + `caught-up` → ocioso
- `sync:in-progress` ou fila de operações → **ativo, não toque**
- `SYNC DISABLED (app not installed)` → container morto, cache é resíduo

### 3. Recência dos arquivos (o sinal mais confiável)

```bash
B=~/Library/Caches/CloudKit/com.apple.bird/<hash>/MMCS
find "$B" -type f -mtime -1 | wc -l
find "$B" -type f -size +1G -exec stat -f '%z %Sm %N' {} \;
```

Arquivos modificados nas últimas horas = transferência em curso. Não apague.

### 4. Identificar o que está sendo transferido

Os arquivos em `MMCS/ClonedFiles` têm nome opaco (`documentContent__<UUID>_fileContent`),
mas o **tamanho em bytes casa exatamente** com o original:

```bash
# pegue o tamanho de um arquivo do cache
stat -f '%z' "$B/ClonedFiles/documentContent__<UUID>_fileContent"

# ache o que tem esse tamanho exato
find ~/Library/Mobile\ Documents -type f -size <N>c 2>/dev/null
```

Caso real (2026-08-09):

| Cache | Tamanho | Original |
|---|---|---|
| `documentContent__<hash-1>..._fileContent` | 10.602.249.728 | `.../WhatsApp/Accounts/<id>/backup/Video.tar` |
| `documentContent__<hash-2>..._fileContent` | 5.431.758.336 | `.../WhatsApp/Accounts/<id>/backup/Media.tar` |

Eram os backups do WhatsApp subindo para o iCloud. 62 arquivos, todos com mtime do dia.

---

## Decisão

| Estado | Ação |
|---|---|
| Daemons ativos + arquivos recentes | **Não tocar** |
| `SYNC DISABLED (app not installed)` + mtime antigo | Resíduo. Pode ir |
| `sync:idle` + `caught-up` + mtime > 30 dias | Provavelmente seguro |
| Em dúvida | Não tocar. iCloud gerencia esse cache sozinho |

---

## Nota sobre economia real

Os arquivos em `ClonedFiles` são clones APFS do original. Apagar pode **liberar zero
espaço**, porque os blocos continuam referenciados pelo arquivo original em
`Mobile Documents`.

Ou seja: risco real, ganho possivelmente nulo. É o pior trade do catálogo.

Ver [armadilha 3](../docs/02-armadilhas-de-medicao.md#3-clones-apfs).

---

## Regra para o app

`~/Library/Caches/CloudKit/**` entra como **condicional**, nunca como "seguro".

Gate em runtime:

1. `pgrep bird cloudd` → se rodando, exigir confirmação extra
2. Qualquer arquivo com mtime < 24 h → remover a categoria da lista
3. Exibir a economia como "incerta (clone APFS)", não somada ao total prometido

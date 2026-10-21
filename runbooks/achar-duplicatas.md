# Runbook: achar duplicatas acidentais

Duplicação costuma ser o maior ganho isolado de espaço e é invisível para limpadores que
varrem por categoria — as cópias estão em pastas legítimas.

---

## Por que acontece sem o usuário perceber

| Origem | Exemplo |
|---|---|
| Language server / ferramenta de build | JDT copia mídia para `bin/` |
| Import de biblioteca de mídia | Música e TV copiam em vez de referenciar |
| Cliente de sync | Cópia local + cache do daemon |
| Backup manual esquecido | `pasta 2`, `pasta-old`, `pasta bu` |

---

## Detecção

### 1. Arquivos grandes, ordenados por tamanho

```bash
find ~ -type f -size +1G 2>/dev/null -exec stat -f '%z %N' {} \; | sort -rn | head -30
```

Tamanhos idênticos ou quase idênticos em linhas adjacentes = candidatos.

### 2. Agrupar por tamanho exato

Arquivos de tamanho diferente não podem ser idênticos. Filtre por tamanho antes de hashear
— hash é caro, `stat` é barato.

```bash
find ~ -type f -size +500M 2>/dev/null -exec stat -f '%z %N' {} \; \
  | sort -rn | awk '{c[$1]++; l[$1]=l[$1]"\n"$0} END {for (s in c) if (c[s]>1) print l[s]}'
```

### 3. Confirmar por hash (só nos grupos suspeitos)

```bash
shasum -a 256 <arquivo1> <arquivo2>
```

**Antes de hashear, cheque se o arquivo é dataless** — ler um placeholder de nuvem dispara
o download:

```bash
ls -lO <arquivo>   # se aparecer "dataless", pule
```

Otimização: hashear só os primeiros 4 MB elimina quase todos os falsos positivos a custo
baixo. Hash completo só no empate.

---

## Caso de referência (2026-08-09)

O mesmo show ocupava **51 GB em 5 cópias**:

| Caminho | Tamanho | Natureza |
|---|---|---|
| `~/Library/Mobile Documents/.../Movies/<show>.mp4` | 10,29 GB | iCloud Drive |
| `~/Movies/TV/Media.localized/Home Videos/<show>.mp4` | 10,29 GB | Import do app TV |
| `~/Music/.../<artista>/<album>/<show>.m4v` | 10,29 GB | Import do app Música |
| `.../workspaceStorage/<hash>/redhat.java/jdt_ws/.../bin/<show>.mov` | 10,29 GB | Cópia do JDT |
| `.../workspaceStorage/<hash>/redhat.java/jdt_ws/.../bin/<show>.m4v` | 10,29 GB | Cópia do JDT |

Três estavam em pastas legítimas — nenhum limpador por categoria as tocaria. Duas estavam
enterradas em cache de IDE.

---

## Decisão: qual cópia manter

Ordem de preferência:

1. A que está em pasta de dados do usuário (`~/Movies`, `~/Documents`)
2. A que está no serviço de nuvem, se quer acesso em outros dispositivos
3. Descartar as que estão em `Caches`, `workspaceStorage`, `bin/`, `build/`

Duas cópias em serviços de nuvem diferentes normalmente é desperdício, não redundância.

**Nunca escolha automaticamente.** Apresente as opções e deixe o usuário decidir. Cópias
podem ter propósito que o scanner não enxerga.

---

## Regra para o app

Pipeline em três estágios, do barato para o caro:

1. Agrupar por tamanho exato (`stat`, barato)
2. Descartar dataless do grupo
3. Hash dos primeiros 4 MB nos grupos com 2+ membros
4. Hash completo só onde o parcial bateu

Na apresentação:

- Ordenar por espaço recuperável (`tamanho × (cópias − 1)`)
- Marcar qual cópia é a "canônica" sugerida, sem pré-selecionar a remoção
- Sinalizar cópias em caminhos de build/cache — essas são quase sempre acidentais

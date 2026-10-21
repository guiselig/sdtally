# Runbook: auditoria completa de System Data

Investigar um "System Data" inflado, do zero, sem apagar nada.

Todos os comandos são read-only. Tempo: 5-15 min conforme o disco.

---

## 0. Baseline

```bash
df -h / /System/Volumes/Data
```

Anote o usado. Todo o resto é reconciliação contra este número.

## 1. Descartar o suspeito mais comum

```bash
tmutil listlocalsnapshots /
```

Saída vazia = sem snapshots, siga. Se listar, cada snapshot pode segurar dezenas de GB.
Remover só com `tmutil deletelocalsnapshots <data>`.

## 2. Home, incluindo ocultos

```bash
du -sh ~/.[a-zA-Z]* ~/* 2>/dev/null | sort -rh | head -20
```

O `~/.[a-zA-Z]*` é obrigatório — sem ele os dotfiles somem do relatório
(ver [armadilha 4](../docs/02-armadilhas-de-medicao.md#4-dotfiles)).

## 3. `~/Library` por subpasta

```bash
du -sh ~/Library/* 2>/dev/null | sort -rh | head -15
```

Esperado: `Mobile Documents`, `Application Support` e `Caches` no topo.

Sinal de anomalia: `Preferences` com mais de ~100 MB.

## 4. Descer nos três maiores

```bash
du -sh ~/Library/Caches/* 2>/dev/null | sort -rh | head -15
du -sh ~/Library/Application\ Support/* 2>/dev/null | sort -rh | head -15
du -sh ~/Library/Containers/* ~/Library/Group\ Containers/* 2>/dev/null | sort -rh | head -10
```

Continue descendo enquanto um único filho dominar o pai. Pare quando chegar em arquivos.

## 5. Nível de sistema

```bash
du -sh /Library/* 2>/dev/null | sort -rh | head -10
du -sh /private/var/* 2>/dev/null | sort -rh | head -10
du -sh /opt/* /usr/local/* 2>/dev/null | sort -rh | head -10
```

Sem `sudo` várias pastas ficam inacessíveis e o `du` as reporta como menores. Aceitável
para triagem — o alvo real é o `~/Library`.

## 6. Arquivos grandes soltos

```bash
find ~ -type f -size +1G 2>/dev/null -exec stat -f '%z %N' {} \; | sort -rn | head -20
```

Este passo revela o que a varredura por pasta esconde. Foi aqui que apareceram os vídeos
duplicados na auditoria original.

## 7. Validar cada suspeito antes de concluir

**Nunca reporte um número sem passar por aqui.** Para cada item grande:

```bash
# sparse? compare aparente vs blocos alocados
stat -f 'aparente=%z blocos=%b %N' <arquivo>

# dataless? procure a flag
ls -lO <arquivo>

# em uso? processo com o arquivo aberto
lsof <caminho> 2>/dev/null | head

# modificado recentemente? cache ativo não se apaga
stat -f '%Sm %N' <caminho>
```

Regras:

- `blocos × 512` muito menor que `%z` → sparse. Use o valor real.
- Flag `dataless` → placeholder. Ocupa zero. **Não abra o arquivo.**
- mtime de horas atrás → em uso. Investigue antes de classificar.

## 8. Reconciliar

Some o que encontrou e compare com o passo 0. Diferença grande significa que algo escapou
— tipicamente dotfiles, um volume montado, ou espaço que só o `sudo` enxerga.

---

## Erros que este runbook previne

| Erro | Passo que evita |
|---|---|
| Perder 21 GB em dotfiles | 2 |
| Reportar 100 GB de disco sparse que não existem | 7 |
| Reportar 72 GB de placeholders de nuvem | 7 |
| Sugerir apagar cache de upload em andamento | 7 |
| Não achar 51 GB do mesmo vídeo em 5 cópias | 6 |

## Automação

[`scripts/audit.sh`](../scripts/audit.sh) roda os passos 0 a 6.
O passo 7 continua manual — é julgamento, não varredura.

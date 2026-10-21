#!/usr/bin/env bash
# Auditoria read-only de "System Data" no macOS.
# NAO apaga, NAO move, NAO modifica nada. Apenas mede e reporta.
#
# Uso:  ./audit.sh [--json]
#
# Implementa os passos 0-6 do runbook runbooks/auditoria-completa.md.
# O passo 7 (validacao de cada suspeito) permanece manual — ver `verificar` abaixo.

set -uo pipefail

TOP=${TOP:-15}
BIG_FILE_MIN=${BIG_FILE_MIN:-1G}

hr() { printf '\n\033[1m== %s ==\033[0m\n' "$1"; }

# du de uma lista de caminhos, maiores primeiro.
# du sem flags ja usa blocos alocados: arquivos sparse sao contados corretamente.
scan() { du -sh "$@" 2>/dev/null | sort -rh | head -"$TOP"; }

hr "0. Baseline"
df -h / /System/Volumes/Data

hr "1. Snapshots locais do Time Machine"
snaps=$(tmutil listlocalsnapshots / 2>/dev/null | grep -c 'com.apple.TimeMachine' || true)
if [ "$snaps" -gt 0 ]; then
  echo "$snaps snapshot(s) encontrado(s) — podem segurar dezenas de GB:"
  tmutil listlocalsnapshots /
  echo "Remover SOMENTE com: tmutil deletelocalsnapshots <data>"
else
  echo "Nenhum. Descartado como causa."
fi

hr "2. Home (incluindo ocultos)"
# ~/.[a-zA-Z]* e obrigatorio: globbing padrao ignora dotfiles.
# Sem isso, ~/.ollama, ~/.npm e ~/.colima somem do relatorio.
scan ~/.[a-zA-Z]* ~/*

hr "3. ~/Library por subpasta"
scan ~/Library/*
echo
prefs=$(du -sk ~/Library/Preferences 2>/dev/null | cut -f1)
if [ -n "${prefs:-}" ] && [ "$prefs" -gt 102400 ]; then
  echo "ANOMALIA: ~/Library/Preferences com $((prefs / 1024)) MB (esperado: poucos MB)."
  du -sh ~/Library/Preferences/* 2>/dev/null | sort -rh | head -5
fi

hr "4a. ~/Library/Caches"
scan ~/Library/Caches/*

hr "4b. ~/Library/Application Support"
scan ~/Library/Application\ Support/*

hr "4c. Containers"
scan ~/Library/Containers/* ~/Library/Group\ Containers/*

hr "5. Nivel de sistema (sem sudo: alguns caminhos ficam inacessiveis)"
scan /Library/* /private/var/* /opt/* /usr/local/*

hr "6. Arquivos maiores que $BIG_FILE_MIN"
echo "Tamanhos identicos em linhas vizinhas = candidatos a duplicata."
find "$HOME" -type f -size +"$BIG_FILE_MIN" 2>/dev/null \
  -exec stat -f '%z %N' {} \; | sort -rn | head -"$TOP"

hr "7. Validacao — MANUAL, nao pule"
cat <<'EOF'
Para CADA item grande acima, antes de concluir qualquer coisa:

  stat -f 'aparente=%z blocos=%b %N' <arquivo>   # blocos*512 << aparente = sparse
  ls -lO <arquivo>                                # flag "dataless" = placeholder, ocupa 0
  lsof <caminho> 2>/dev/null | head               # em uso por algum processo?
  stat -f '%Sm %N' <caminho>                      # mtime recente = cache ativo

Regras:
  - sparse   -> use blocos*512, nao o tamanho aparente
  - dataless -> ocupa zero. NAO abra o arquivo (dispara download)
  - mtime de horas atras -> em uso, investigue antes de classificar

Detalhes: docs/02-armadilhas-de-medicao.md
EOF

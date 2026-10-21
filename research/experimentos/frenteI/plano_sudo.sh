#!/bin/bash
# Plano da Frente I para o DONO rodar (tem 1 passo com sudo para montar, 1 para medir, 1 para desmontar).
# Mede quais arquivos o snapshot A segura, comparando com o snapshot seguinte B (privado de A,
# comparável ao fsctl) e opcionalmente com o volume vivo. Nada é apagado; montagem é só leitura.
# Uso: ./plano_sudo.sh <nome_A> <xid_A> <nome_B> <xid_B>
#  ex: ./plano_sudo.sh 2026-09-27-081652 14896291 2026-09-27-140444 14959151
set -euo pipefail
A=$1; XA=$2; B=$3; XB=$4
DIR="$(cd "$(dirname "$0")" && pwd)"; OUT=/private/tmp/frenteI; mkdir -p "$OUT"
MA=/private/tmp/snapI_A; MB=/private/tmp/snapI_B

# 0. Nunca durante backup.
tmutil status | grep -q 'Running = 0' || { echo "backup rodando: pare aqui e tente depois"; exit 1; }

# 1. Compilar (sem sudo).
clang -O2 -o "$OUT/snapdiff" "$DIR/snapdiff.c"
clang -O2 -o "$OUT/snapheld" "$DIR/snapheld.c"
"$OUT/snapheld" -t

# 2. Lista de objetos que mudaram de A para B (sem sudo; ~45 s).
"$OUT/snapdiff" /System/Volumes/Data "$XA" "$XB" > "$OUT/diff.txt"

# 3. Montar os dois snapshots só leitura (sudo). Montagem de snapshot é sempre read-only.
sudo mkdir -p "$MA" "$MB"
trap 'sudo umount "$MA" "$MB" 2>/dev/null; sudo rmdir "$MA" "$MB" 2>/dev/null' EXIT   # 5. desmonta sempre, mesmo se a 2ª montagem falhar
sudo mount_apfs -o rdonly,nobrowse -s "com.apple.TimeMachine.$A.local" /System/Volumes/Data "$MA"
sudo mount_apfs -o rdonly,nobrowse -s "com.apple.TimeMachine.$B.local" /System/Volumes/Data "$MB"

# 4. Medir (sudo só para ler tudo; nada escreve no volume).
sudo "$OUT/snapheld" "$MA" "$MB" < "$OUT/diff.txt" 2> "$OUT/total_vs_B.txt" | sort -rn > "$OUT/presos_vs_B.tsv"
sudo "$OUT/snapheld" "$MA" /System/Volumes/Data < "$OUT/diff.txt" 2> "$OUT/total_vs_vivo.txt" | sort -rn > "$OUT/presos_vs_vivo.tsv"
cat "$OUT/total_vs_B.txt" "$OUT/total_vs_vivo.txt"
head -30 "$OUT/presos_vs_B.tsv"
echo "compare presos_exatos_dedup (vs B) com o privado do fsctl: research/experimentos/frenteB/snapsize /System/Volumes/Data"

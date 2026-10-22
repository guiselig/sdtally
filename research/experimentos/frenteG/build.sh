#!/bin/sh
# Frente G: compila os probes (todos só leitura; nenhum pede root).
set -e
cd "$(dirname "$0")"
clang -O1 apfsspace.c -o apfsspace -framework CoreFoundation
clang -O1 apfsuc.c -o apfsuc -framework IOKit -framework CoreFoundation
echo "ok: ./apfsuc disk3 4   ./apfsspace disk3s5 disk3   python3 meta_regress.py <scratch> ./apfsuc"

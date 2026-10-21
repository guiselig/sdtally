#!/bin/sh
# Compila os experimentos da frente H. Todos são somente leitura; nenhum pede root.
set -e
cd "$(dirname "$0")"
clang -Wall -O2 dirstats.c -o dirstats
clang -Wall -O2 dirattr.c -o dirattr
clang -fobjc-arc -framework Foundation stmsize.m -o stmsize
echo "ok: ./dirstats <pasta>...  ./dirattr <pasta>...  ./stmsize <pasta>...  ./assetgc_dirsize.py [2h]"

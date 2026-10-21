#!/bin/sh
# Compila os experimentos da frente B. Todos são somente leitura; nenhum pede root.
set -e
cd "$(dirname "$0")"
clang -Wall snapsize.c -o snapsize
clang -fobjc-arc -framework Foundation cdprobe.m -o cdprobe
clang -fobjc-arc -framework Foundation tmsnap.m -o tmsnap
clang -fobjc-arc -framework Foundation introspect.m -o introspect
echo "ok: ./snapsize [mount]  ./cdprobe [volume] [urgência]  ./tmsnap [mount]  ./introspect <binário> <regex>"

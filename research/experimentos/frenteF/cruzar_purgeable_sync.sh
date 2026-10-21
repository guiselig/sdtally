#!/bin/sh
# Cruza EF_IS_PURGEABLE (fp_scan --list) com o estado de sync (ubiq_keys) por arquivo.
# Uso: cruzar_purgeable_sync.sh <raiz> <dir_binarios>   (fp_scan e ubiq_keys compilados em <dir_binarios>)
# Saída: tag(p=purgeable, np=não, dl=dataless)  status|uploaded|excluded  nº arquivos  GiB alocados
set -e
R="$1"; B="$2"; T=$(mktemp -d)
"$B/fp_scan" "$R" 0 --list np,p,dl 2> "$T/list.tsv" > /dev/null
"$B/ubiq_keys" "$R" > "$T/ubiq.tsv"
awk -F'\t' 'NR==FNR{t[$3]=$1; a[$3]=$2; next} {k=(($8 in t)?t[$8]:"??")" "$1"|"$2"|"$6; n[k]++; s[k]+=a[$8]}
  END{for(k in n) printf "%-70s %7d %9.3f\n", k, n[k], s[k]/2^30}' "$T/list.tsv" "$T/ubiq.tsv" \
  | sed 's/NSURLUbiquitousItemDownloadingStatus//' | sort -k3 -rn
rm -r "$T"

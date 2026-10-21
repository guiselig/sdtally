#!/bin/zsh
# Frente K: latência de primeiro plano (fgprobe) com a varredura rodando ao lado.
# Uso: bench_fg.sh <dir_do_binario> <dir_de_saida> [raiz]
B=$1; O=$2; ROOT=${3:-/System/Volumes/Data}
mkdir -p $O
$B/fgprobe $O/fgprobe.bin 1 > /dev/null   # cria o arquivo de teste de 1 GiB
case_() { # nome threads prefixo...
  local name=$1 t=$2; shift 2
  while /usr/bin/tmutil status | grep -q "Running = 1"; do sleep 60; done
  echo "== $name t=$t $(date +%T)" >> $O/fg.log
  if [ "$t" = 0 ]; then
    $B/fgprobe $O/fgprobe.bin 15 >> $O/fg.log
  else
    $B/fgprobe $O/fgprobe.bin 15 >> $O/fg.log &
    env SDC_QUIET=1 "$@" $B/walk_k $ROOT $t 1 2>/dev/null | grep "^K: tempo" >> $O/fg.log
    wait
  fi
  sleep 5
}
case_ ocioso 0
case_ padrao 10
case_ padrao 4
case_ iopol_throttle 10
case_ qos_background 10
case_ qos_background 4
case_ qos_utility 4
case_ taskpolicy_b 10 /usr/sbin/taskpolicy -b
case_ ocioso 0

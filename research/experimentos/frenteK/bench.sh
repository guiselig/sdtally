#!/bin/zsh
# Frente K: matriz de threads x prioridade para walk_k, e sonda de primeiro plano.
# Uso: bench.sh <dir_do_binario> <dir_de_saida> [raiz]
# Só leitura fora do <dir_de_saida> (onde grava logs e o arquivo de teste da sonda).
B=$1; O=$2; ROOT=${3:-/System/Volumes/Data}
mkdir -p $O
waittm() {
  while /usr/bin/tmutil status | grep -q "Running = 1"; do
    echo "$(date +%T) backup do TM rodando, esperando 60 s" | tee -a $O/tm.log; sleep 60
  done
}
run() { # nome threads prefixo...
  local name=$1 t=$2; shift 2
  waittm
  echo "== $name t=$t $(date +%T)" >> $O/runs.log
  env SDC_QUIET=1 "$@" /usr/bin/time -l $B/walk_k $ROOT $t 1 >> $O/runs.log 2>&1
  sleep 5
}
# 1) Matriz pedida: 1/4/8/10/16 threads, sem e com IOPOL_THROTTLE.
for t in 1 4 8 10 16; do
  run padrao $t
  run iopol_throttle $t SDC_IOPOL=throttle
done
# 2) Outras políticas, 10 threads.
run iopol_utility 10 SDC_IOPOL=utility
run qos_utility 10 SDC_QOS=utility
run qos_background 10 SDC_QOS=background
run taskpolicy_b 10 /usr/sbin/taskpolicy -b
run taskpolicy_c_utility 10 /usr/sbin/taskpolicy -c utility
run taskpolicy_c_background 10 /usr/sbin/taskpolicy -c background
# 3) Repetição do padrão para ver ruído.
run padrao_rep 10
run padrao_rep 10

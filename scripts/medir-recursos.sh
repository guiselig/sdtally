#!/bin/bash
# Mede tempo e recursos do cálculo completo (sdtally --json): sem senha e como root, na prioridade
# normal e na de fundo.
# Por rodada: /usr/bin/time -l (tempo, CPU, memória, instruções, leituras), stage_seconds do JSON
# e energia: SoC pelo powermetrics e, na bateria, o que sai dela (corrente × tensão do
# AppleSmartBattery a cada 1 s), ambos menos a média ociosa medida antes.
# Rodar uma vez em cada condição a comparar (bateria, bateria com Pouca Energia, tomada): a fonte
# e o Pouca Energia ficam no nome da pasta e no resumo.
# Cenários: semsenha, root (padrão), semsenha-fundo e root-fundo (prioridade de fundo, `taskpolicy -b`:
# núcleos de eficiência). Sem cenário "frio": o `sudo purge` não esvazia o cache de metadados do APFS
# (19/10: frio = quente, 0 leituras); frio de verdade só depois de reiniciar.
# Uso: [CENARIOS="semsenha-fundo root-fundo"] scripts/medir-recursos.sh [rodadas por cenário, padrão 3]
# Saída: ~/Library/Caches/sdtally-wt/bench/<data>-<fonte>/ (arquivos crus + resumo.txt)
set -euo pipefail

N=${1:-3}
BIN=${SDTALLY_BIN:-$HOME/Library/Caches/sdtally-wt/build-ui/cargo/release/sdtally}
FONTE=$(pmset -g batt | grep -q "AC Power" && echo tomada || echo bateria)
[ "$(pmset -g | awk '/lowpowermode/ {print $2}')" = 1 ] && FONTE=$FONTE-poucaenergia
OUT=$HOME/Library/Caches/sdtally-wt/bench/$(date +%Y%m%d-%H%M%S)-$FONTE
[ -x "$BIN" ] || { echo "sem binário em $BIN (cargo build --release)"; exit 1; }
mkdir -p "$OUT"

# Backup do Time Machine dobra o tempo (caderno 30/09): não mede com ele rodando.
tmutil status | grep -q 'Running = 0' || { echo "Time Machine rodando: espere terminar"; exit 1; }
echo "Condição: $FONTE"

echo "Pede a senha uma vez (root e powermetrics)."
sudo -v
while true; do sudo -n true; sleep 50; done 2>/dev/null &
MANTER=$!
trap 'kill $MANTER 2>/dev/null || true' EXIT

{
  sysctl -n hw.model machdep.cpu.brand_string hw.ncpu hw.memsize
  sw_vers
  "$BIN" --version 2>/dev/null || true
  df -k /System/Volumes/Data
  pmset -g batt
  pmset -g | grep -i lowpower
} > "$OUT/maquina.txt"

energia() { # arquivo de saída; roda até receber SIGINT
  sudo powermetrics --samplers cpu_power -i 1000 -o "$1" >/dev/null 2>&1
}

bateria() { # arquivo; "InstantAmperage=mA Voltage=mV" por segundo até ser morto
  while true; do
    ioreg -rn AppleSmartBattery | awk -F' = ' '/"InstantAmperage" =|"Voltage" =/ {gsub(/[" |]/, "", $1); printf "%s=%s ", $1, $2} END {print ""}'
    sleep 1
  done > "$1"
}

pct() { pmset -g batt | grep -Eo '[0-9]+%' | head -1; }

echo "Base ociosa: 30 s sem fazer nada."
sleep 10
energia "$OUT/base.power" & PM=$!
bateria "$OUT/base.bat" & BT=$!
sleep 30
sudo pkill -INT -x powermetrics || true
kill $BT 2>/dev/null || true
wait $PM $BT 2>/dev/null || true

rodar() { # nome, rodada, root (0/1), fundo (0/1)
  local d="$OUT/$1-$2"
  sleep 10  # CPU volta ao repouso entre rodadas
  uptime > "$d.carga"
  pct > "$d.pct"
  energia "$d.power" & PM=$!
  bateria "$d.bat" & BT=$!
  sleep 1
  # taskpolicy faz exec do sdtally: o rusage do time continua sendo o do próprio sdtally.
  local cmd=(/usr/bin/time -l)
  if [ "$4" = 1 ]; then cmd+=(taskpolicy -b); fi
  cmd+=("$BIN" --json)
  if [ "$3" = 1 ]; then
    sudo "${cmd[@]}" > "$d.json" 2> "$d.time"
  else
    "${cmd[@]}" > "$d.json" 2> "$d.time"
  fi
  sudo pkill -INT -x powermetrics || true
  kill $BT 2>/dev/null || true
  wait $PM $BT 2>/dev/null || true
  pct >> "$d.pct"
  echo "  $1 $2: $(grep -Eo '[0-9.]+ real' "$d.time")"
}

CENARIOS=${CENARIOS:-semsenha root}
for i in $(seq 1 "$N"); do
  echo "Rodada $i de $N"
  for c in $CENARIOS; do
    case $c in
      semsenha) rodar "$c" "$i" 0 0 ;;
      root) rodar "$c" "$i" 1 0 ;;
      semsenha-fundo) rodar "$c" "$i" 0 1 ;;
      root-fundo) rodar "$c" "$i" 1 1 ;;
      *) echo "cenário desconhecido: $c"; exit 1 ;;
    esac
  done
done

python3 - "$OUT" <<'PY' | tee "$OUT/resumo.txt"
import json, re, statistics, sys
from pathlib import Path

out = Path(sys.argv[1])

def potencias(f):
    # Apple Silicon: "Combined Power (CPU + GPU + ANE): N mW"; sem ela, "CPU Power: N mW".
    t = f.read_text(errors="replace")
    c = [int(x) for x in re.findall(r"Combined Power \(CPU \+ GPU \+ ANE\): (\d+) mW", t)]
    return c or [int(x) for x in re.findall(r"CPU Power: (\d+) mW", t)]

base = potencias(out / "base.power")
base_mw = statistics.mean(base) if base else 0.0

def bateria_w(f):
    # Linhas "InstantAmperage=mA Voltage=mV". O ioreg dá a corrente como inteiro sem sinal de
    # 64 bits (descarregando = negativa). W = -mA × mV / 1e6.
    w = []
    for l in f.read_text().split("\n") if f.exists() else []:
        d = dict(x.split("=", 1) for x in l.split() if "=" in x)
        if "InstantAmperage" in d and "Voltage" in d:
            a = int(d["InstantAmperage"])
            a = a - 2**64 if a >= 2**63 else a
            w.append(-a * int(d["Voltage"]) / 1e6)
    return w

na_bateria = "bateria" in out.name
base_bat = statistics.mean(bateria_w(out / "base.bat") or [0.0])

def campo(t, nome):
    m = re.search(r"(\d+)\s+" + nome, t)
    return int(m.group(1)) if m else None

linhas = {}
for f in sorted(out.glob("*.time")):
    nome = f.stem.rsplit("-", 1)[0]
    t = f.read_text()
    real, user, sys_ = (float(x) for x in re.search(r"([\d.]+) real\s+([\d.]+) user\s+([\d.]+) sys", t).groups())
    j = json.loads(f.with_suffix(".json").read_text())
    st = j.get("stage_seconds", {})
    p = potencias(f.with_suffix(".power"))
    acima = (statistics.mean(p) - base_mw) / 1000 if p else None
    linhas.setdefault(nome, []).append({
        "tempo (s)": real,
        "CPU total (s)": user + sys_,
        "CPU usuário (s)": user,
        "CPU sistema (s)": sys_,
        "núcleos em média": (user + sys_) / real,
        "memória pico (MB)": campo(t, "maximum resident set size") / 1e6,
        "memória real (MB)": (campo(t, "peak memory footprint") or 0) / 1e6,
        "instruções (bilhões)": (campo(t, "instructions retired") or 0) / 1e9,
        "leituras de disco": campo(t, "block input operations"),
        "energia acima do ocioso (J)": acima * real if acima is not None else None,
        "potência acima do ocioso (W)": acima,
        "bateria acima do ocioso (W)": (statistics.mean(bw) - base_bat) if (bw := bateria_w(f.with_suffix(".bat"))) and na_bateria else None,
        "bateria gasta acima do ocioso (J)": (statistics.mean(bw) - base_bat) * real if bw and na_bateria else None,
        "% de bateria (antes → depois)": f.with_suffix(".pct").read_text().split() if na_bateria else None,
        "etapa árvore (s)": st.get("arvore"),
        "etapa snapshots (s)": st.get("snapshots"),
        "completo": j.get("complete"),
    })

print(f"condição: {out.name.split('-', 2)[-1]}")
print(f"base ociosa: SoC {base_mw / 1000:.2f} W ({len(base)} amostras)" + (f" · bateria {base_bat:.2f} W" if na_bateria else ""))
for nome, rs in linhas.items():
    print(f"\n{nome} ({len(rs)} rodadas)")
    for k in rs[0]:
        v = [r[k] for r in rs if isinstance(r[k], (int, float)) and not isinstance(r[k], bool)]
        if not v:
            print(f"  {k}: {[r[k] for r in rs]}")
            continue
        print(f"  {k}: mediana {statistics.median(v):.2f} · mín {min(v):.2f} · máx {max(v):.2f}")
PY
echo "Arquivos em $OUT"

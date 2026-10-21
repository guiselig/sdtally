#!/usr/bin/env python3
"""Frente G: filhos diretos de pastas negadas, sem abri-las.

No APFS, lstat de uma pasta devolve st_nlink = filhos diretos + 2 (conferido em pastas
legíveis por --check). lstat só exige busca no pai, então funciona em pasta negada.
Entrada: saída do walk_par com SDC_DENIED=1 (linhas "[open errno N] caminho" / "[bulk errno N] caminho").
"""
import os, re, sys

if sys.argv[1] == "--check":
    bad = 0; n = 0
    for d in sys.argv[2:]:
        for dp, dn, fn in os.walk(d):
            n += 1
            if os.lstat(dp).st_nlink - 2 != len(dn) + len(fn):
                bad += 1
    print(f"pastas conferidas {n}, divergentes {bad}"); sys.exit(0)

rows = []
for line in open(sys.argv[1]):
    m = re.match(r"\s*\[(open|bulk) errno (\d+)\] (.*)$", line.rstrip("\n"))
    if not m: continue
    kind, e, p = m.groups()
    try:
        st = os.lstat(p); kids = st.st_nlink - 2
    except OSError as x:
        kids = -1
    rows.append((kids, kind, int(e), p))
rows.sort(reverse=True)
tot = sum(k for k, *_ in rows if k > 0)
print(f"pastas negadas {len(rows)} (open {sum(r[1]=='open' for r in rows)}, bulk {sum(r[1]=='bulk' for r in rows)}); filhos diretos somados {tot}")
for k, kind, e, p in rows[:int(os.environ.get('TOP', '25'))]:
    print(f"{k:>8}  {kind} errno {e:<2} {p}")

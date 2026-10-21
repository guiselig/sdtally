#!/usr/bin/env python3
"""Frente H: tamanho de cada asset do MobileAsset MEDIDO pelo próprio mobileassetd.

O mobileassetd (que tem acesso às pastas restritas) grava no log, a cada avaliação de
coleta (AssetGC), a linha "AssetGC determine | <pasta do asset> | dirSize: N | ... Result: ...".
Uma consulta de purgeable ao CacheDelete (cdprobe da frente B, urgência 1–4, só leitura)
dispara essa avaliação. Este script lê o log (sem sudo, usuário admin) e soma o último
dirSize de cada pasta, por tipo. uso: assetgc_dirsize.py [janela, padrão 2h]
"""
import collections, re, subprocess, sys
win = sys.argv[1] if len(sys.argv) > 1 else "2h"
out = subprocess.run(["/usr/bin/log", "show", "--last", win, "--style", "compact", "--predicate",
                      'process == "mobileassetd" AND eventMessage CONTAINS "AssetGC determine"'],
                     capture_output=True, text=True, timeout=280).stdout
last = {}
for l in out.splitlines():
    m = re.search(r'^(\S+ \S+).*AssetGC determine \| (\S+) \| dirSize: (\d+)', l)
    if m: last[m.group(2)] = (m.group(1), int(m.group(3)))
if not last: sys.exit("nenhuma linha AssetGC na janela; rode antes uma consulta ao CacheDelete")
by = collections.defaultdict(lambda: [0, 0, "9"])
for p, (t, s) in last.items():
    b = by[p.split("/")[4]]; b[0] += 1; b[1] += s; b[2] = min(b[2], t)
for k, (n, s, t) in sorted(by.items(), key=lambda x: -x[1][1]):
    print(f"{n:4d} {s:15d} {s/2**30:8.3f} GiB  {t[:19]}  {k}")

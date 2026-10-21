#!/usr/bin/env python3
"""Acha o event id do FSEvents que corresponde a um snapshot, sem root.
Entrada: saída do snapdiff para (X-1 -> X) e (X -> X+1), e o TSV do fsevhist (id, flags, inode, caminho).
O snapshot X fica entre a última criação de um objeto que já está em X e a primeira criação de um
objeto que só aparece em X+1. Uso: bracket.py diff_ant_X.txt diff_X_prox.txt fsevhist.tsv"""
import sys
CREATED = 0x100

def created(path):
    return {int(l.split()[1]) for l in open(path) if l.startswith('1 ')}

def main(prev_diff, next_diff, tsv):
    a, b = created(prev_diff), created(next_diff)
    first = {}  # inode -> primeiro event id com flag Created (flags de eventos coalescidos repetem Created)
    with open(tsv, errors='replace') as f:
        for line in f:
            p = line.split('\t', 3)
            if len(p) < 3 or not p[0].isdigit(): continue
            try: ev, fl, ino = int(p[0]), int(p[1], 16), int(p[2])
            except ValueError: continue
            if not fl & CREATED: continue
            if (ino in a or ino in b) and (ino not in first or ev < first[ino]): first[ino] = ev
    av = sorted(v for k, v in first.items() if k in a)
    bv = sorted(v for k, v in first.items() if k in b)
    if not av or not bv: print("histórico não cobre o intervalo"); return
    q = lambda v, f: v[min(len(v) - 1, int(f * len(v)))]
    # Criações repetidas por coalescência e inodes criados antes da janela deixam caudas;
    # o miolo das duas distribuições separa o snapshot.
    print(f"em_X: n={len(av)} p50={q(av,.5)} p99={q(av,.99)} max={av[-1]}")
    print(f"so_em_X+1: n={len(bv)} min={bv[0]} p01={q(bv,.01)} p50={q(bv,.5)}")
    inv = sum(1 for v in av if v > bv[0])
    print(f"since_conservador={min(q(av,.99), bv[0])} (objetos de X com criação após o min de X+1: {inv})")


if __name__ == '__main__':
    assert int('0x00410100', 16) & CREATED  # autoteste mínimo do parse de flags
    main(*sys.argv[1:4])

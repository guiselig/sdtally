#!/usr/bin/env python3
"""Dá nome aos objetos de um snapdiff usando o histórico do FSEvents (inode -> último caminho visto).
Para os que ainda existem no volume vivo, soma ALLOCSIZE atual (os.stat st_blocks*512).
Uso: nomes.py <snapdiff.txt> <fsevhist.tsv> [nivel_prefixo=4]
Saída: resumo por tipo e por prefixo de caminho; nomes com tamanho vivo quando houver."""
import os, sys
from collections import Counter, defaultdict

def main(diff, tsv, depth=4):
    objs = {}
    for l in open(diff):
        f = l.rstrip('\n').split(' ', 2)
        if len(f) < 2 or not f[0].isdigit() or not f[1].isdigit(): continue  # caminho com quebra de linha
        t, oid, *rest = f
        objs[int(oid)] = (int(t), rest[0] if rest and rest[0].startswith('/') else None)
    last = {}
    with open(tsv, errors='replace') as f:
        for line in f:
            p = line.rstrip('\n').split('\t', 3)
            if len(p) == 4 and p[2].lstrip('-').isdigit() and int(p[2]) in objs:
                ev = int(p[0])
                if p[2] not in last or ev > last[p[2]][0]: last[p[2]] = (ev, p[3])
    per = defaultdict(Counter); live_bytes = Counter(); named = Counter()
    for oid, (t, livepath) in objs.items():
        path = livepath or ('/' + last[str(oid)][1] if str(oid) in last else None)
        if path: named[t] += 1
        pre = '/'.join((path or '(sem nome)').split('/')[:depth + 1])
        per[t][pre] += 1
        if livepath:
            try: live_bytes[t] += os.lstat(livepath).st_blocks * 512
            except OSError: pass
    names = {1: 'criado', 2: 'modificado', 3: 'apagado'}
    for t in sorted(per):
        tot = sum(per[t].values())
        print(f"== tipo {t} ({names.get(t, '?')}): {tot} objetos, {named[t]} com nome, vivos somam {live_bytes[t]/2**30:.3f} GiB")
        for pre, n in per[t].most_common(15): print(f"   {n:7d}  {pre}")

if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2], int(sys.argv[3]) if len(sys.argv) > 3 else 4)

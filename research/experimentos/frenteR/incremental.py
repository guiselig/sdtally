#!/usr/bin/env python3
"""Frente R: a árvore atualizada só pelo FSEvents dá o mesmo total que uma travessia nova?

Só leitura. Conta Σ st_blocks × 512 com hardlink uma vez, sem cruzar volume (a regra do
STMSize e da travessia do sdtally, sem xattr grande).

  incremental.py inicio <raiz> <estado>   pega o event id ANTES de varrer, varre, grava o estado
  incremental.py fim <raiz> <estado>      aplica os eventos desde o id (relê só as pastas
                                          avisadas), varre de novo do zero e compara
Precisa do binário `fsev` (fsev.swift) no PATH ou em $FSEV.
"""
import ctypes
import os
import pickle
import subprocess
import sys
import time

FSEV = os.environ.get("FSEV", "fsev")
MUST_SCAN_SUB = 0x1  # kFSEventStreamEventFlagMustScanSubDirs
DROPPED = 0x2 | 0x4  # UserDropped | KernelDropped


SF_DATALESS = 0x40000000


def politicas():
    """Síntese §4.1 e §4.2b: não materializar dataless e não gravar atime ao listar. Falha aborta."""
    libc = ctypes.CDLL(None)
    for tipo in (3, 2):  # IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, _ATIME_UPDATES; escopo processo; OFF
        if libc.setiopolicy_np(tipo, 0, 1) != 0:
            sys.exit(f"setiopolicy_np({tipo}) falhou; abortando para não alterar nada")


def ler_pasta(p, dev):
    """Arquivos diretos (nome -> (ino, blocos)) e subpastas do mesmo volume; None se sumiu."""
    arqs, subs = {}, []
    try:
        it = os.scandir(p)
    except OSError:
        return None
    with it:
        for e in it:
            try:
                st = e.stat(follow_symlinks=False)
            except OSError:
                continue
            if st.st_dev != dev:
                continue
            if e.is_dir(follow_symlinks=False):
                if not st.st_flags & SF_DATALESS:  # §4.2: pasta só na nuvem não é aberta
                    subs.append(e.path)
            elif not e.is_symlink():
                arqs[e.name] = (st.st_ino, st.st_blocks * 512)
    return arqs, subs


def varrer(raiz, dev, estado):
    pilha = [raiz]
    while pilha:
        p = pilha.pop()
        r = ler_pasta(p, dev)
        if r is None:
            continue
        estado[p] = r[0]
        pilha.extend(r[1])


def total(estado):
    por_ino = {}
    for arqs in estado.values():
        for ino, b in arqs.values():
            por_ino[ino] = b
    return sum(por_ino.values()), len(por_ino)


def tirar_subarvore(estado, p):
    pref = p.rstrip("/") + "/"
    for k in [k for k in estado if k == p.rstrip("/") or k.startswith(pref)]:
        del estado[k]


def inicio(raiz, arq):
    ev = int(subprocess.check_output([FSEV, "agora"], text=True))
    dev = os.lstat(raiz).st_dev
    t = time.time()
    estado = {}
    varrer(raiz, dev, estado)
    with open(arq, "wb") as f:
        pickle.dump({"ev": ev, "raiz": raiz, "estado": estado, "quando": t}, f)
    tot, n = total(estado)
    print(f"inicio: ev {ev}, {len(estado)} pastas, {n} arquivos, {tot} B, {time.time() - t:.1f} s")


def fim(raiz, arq):
    with open(arq, "rb") as f:
        d = pickle.load(f)
    estado, dev = d["estado"], os.lstat(raiz).st_dev
    t = time.time()
    saida = subprocess.run([FSEV, "desde", str(d["ev"]), raiz], capture_output=True, text=True, timeout=600).stdout
    pastas, recursivas, perdidos = set(), set(), 0
    for linha in saida.splitlines():
        fl, _, p = linha.partition(" ")
        fl = int(fl, 16)
        p = p.rstrip("/") or "/"
        if fl & DROPPED:
            perdidos += 1
        (recursivas if fl & (MUST_SCAN_SUB | DROPPED) else pastas).add(p)
    for p in sorted(recursivas):
        tirar_subarvore(estado, p)
        varrer(p, dev, estado)
    filhos = {}
    for k in estado:
        filhos.setdefault(os.path.dirname(k), set()).add(k)
    relidas = 0
    for p in sorted(pastas - recursivas):
        r = ler_pasta(p, dev)
        relidas += 1
        if r is None:
            tirar_subarvore(estado, p)
            continue
        estado[p] = r[0]
        conhecidas = {k for k in filhos.get(p, ()) if k in estado}
        for s in r[1]:
            if s not in estado:
                varrer(s, dev, estado)  # pasta nova: varre inteira
        for s in conhecidas - set(r[1]):
            tirar_subarvore(estado, s)  # subpasta que sumiu
    t_inc = time.time() - t
    inc, n_inc = total(estado)

    t = time.time()
    novo = {}
    varrer(raiz, dev, novo)
    t_full = time.time() - t
    full, n_full = total(novo)
    print(f"eventos: {len(saida.splitlines())} ({len(pastas)} pastas, {len(recursivas)} recursivas, {perdidos} com perda)")
    print(f"incremental: {inc} B, {n_inc} arquivos, {relidas} pastas relidas, {t_inc:.1f} s")
    print(f"travessia nova: {full} B, {n_full} arquivos, {t_full:.1f} s")
    print(f"diferença: {inc - full:+d} B ({(inc - full) / full * 100 if full else 0:+.4f} %), {n_inc - n_full:+d} arquivos")
    difs = []
    for p in set(estado) | set(novo):
        a = sum(b for _, b in estado.get(p, {}).values())
        b = sum(b for _, b in novo.get(p, {}).values())
        if a != b:
            difs.append((abs(a - b), p, a, b))
    for _, p, a, b in sorted(difs, reverse=True)[:15]:
        print(f"  {a - b:+d}  {p}  (incremental {a}, nova {b})")


if __name__ == "__main__":
    politicas()
    {"inicio": inicio, "fim": fim}[sys.argv[1]](sys.argv[2], sys.argv[3])

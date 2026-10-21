#!/usr/bin/env python3
"""dir-stats do APFS (fsctl 0xC1104A71, lido pelo `sdtally why`) contra uma travessia simples
(Σ st_blocks × 512, hardlink uma vez, sem cruzar volume), pasta por pasta.

Uso (CI, com sudo para ler tudo): sudo python3 dirstats_cmp.py <binário sdtally>
Só leitura. Saída: tabela markdown em stdout.
No 27 (01/10): igual ao byte à travessia, menos arquivos comprimidos (UF_COMPRESSED) e o
conteúdo enxertado em .AssetData, que o dir-stats não conta; 0 onde o APFS não mantém dir-stats.
"""
import ctypes
import glob
import os
import re
import subprocess
import sys

DATA = "/System/Volumes/Data"


SF_DATALESS = 0x40000000


def politicas():
    """Síntese §4.1 e §4.2b: não materializar dataless e não gravar atime ao listar. Falha aborta."""
    libc = ctypes.CDLL(None)
    for tipo in (3, 2):  # IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, _ATIME_UPDATES; escopo processo; OFF
        if libc.setiopolicy_np(tipo, 0, 1) != 0:
            sys.exit(f"setiopolicy_np({tipo}) falhou; abortando para não alterar nada")


def dirstats(sdt, p):
    out = subprocess.run([sdt, "why", p, "--lang", "en"], capture_output=True, text=True, timeout=60)
    m = re.search(r"dir-stats \(APFS\)\s+(.+)", out.stdout)
    if not m:
        return None, (out.stderr or out.stdout).strip()[:80]
    v = m.group(1)
    n = re.match(r"([\d,]+) B", v)
    return (int(n.group(1).replace(",", "")), "") if n else (None, v.strip())


def travessia(p):
    """(total, arquivos, comprimidos, dentro de .AssetData): o dir-stats não conta os dois últimos."""
    dev = os.lstat(p).st_dev
    vistos, tot, n, comp, enx = set(), 0, 0, 0, 0
    for raiz, pastas, arqs in os.walk(p, onerror=lambda e: None):
        pastas[:] = [d for d in pastas if (lambda st: st.st_dev == dev and not st.st_flags & SF_DATALESS)(os.lstat(os.path.join(raiz, d)))]
        em_enxerto = "/.AssetData" in raiz + "/"
        for a in arqs:
            try:
                st = os.lstat(os.path.join(raiz, a))
            except OSError:
                continue
            if st.st_dev != dev:
                continue
            if st.st_nlink > 1:
                if st.st_ino in vistos:
                    continue
                vistos.add(st.st_ino)
            b = st.st_blocks * 512
            tot += b
            n += 1
            if em_enxerto:
                enx += b
            elif st.st_flags & 0x20:  # UF_COMPRESSED
                comp += b
    return tot, n, comp, enx


def main():
    politicas()
    sdt = sys.argv[1]
    alvos = sorted(glob.glob(f"{DATA}/System/Library/AssetsV2/*"))
    alvos += [p for p in (f"{DATA}/Library/Developer/CommandLineTools", f"{DATA}/private/var/db/uuidtext") if os.path.isdir(p)]
    print("| pasta | dir-stats | travessia | arquivos | comprimidos | em .AssetData | veredito |")
    print("|---|---:|---:|---:|---:|---:|---|")
    iguais = explicadas = com = 0
    for p in alvos:
        if not os.path.isdir(p) or os.path.islink(p):
            continue
        ds, erro = dirstats(sdt, p)
        tv, n, comp, enx = travessia(p)
        nome = p.replace(DATA, "")
        if ds is None:
            print(f"| {nome} | {erro} | {tv} | {n} | {comp} | {enx} | |")
            continue
        if ds == 0:
            v = "sem dir-stats"
        elif ds == tv:
            v, iguais = "igual", iguais + 1
        elif ds == tv - comp - enx:
            v, explicadas = "igual sem comprimidos e enxertos", explicadas + 1
        else:
            v = f"{ds - (tv - comp - enx):+d} sem explicação"
        com += ds > 0
        print(f"| {nome} | {ds} | {tv} | {n} | {comp} | {enx} | {v} |")
    print(f"\n{com} pastas com dir-stats mantido: {iguais} iguais ao byte à travessia, "
          f"{explicadas} iguais depois de tirar comprimidos (UF_COMPRESSED) e .AssetData.")

if __name__ == "__main__":
    main()

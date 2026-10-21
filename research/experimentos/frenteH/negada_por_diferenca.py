# Tamanho de pasta negada (EACCES) = dir-stats(pai) − Σ dir-stats(irmãs) − Σ alocado dos arquivos diretos
# do pai (não comprimidos; comprimido não entra no dir-stats). Só leitura. Só funciona se o pai e
# todas as irmãs responderem ao fsctl. Clone e hardlink seguem a regra do dir-stats (clone inteiro,
# hardlink 1x), por isso a subtração é consistente dentro do mesmo pai.
import os, sys, subprocess
DS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "dirstats")
def ds(p):
    r = subprocess.run([DS, p], capture_output=True, text=True)
    return None if r.returncode else tuple(int(x) for x in r.stdout.split("\t")[:2])
for alvo in sys.argv[1:]:
    pai = os.path.dirname(alvo); tp = ds(pai)
    if not tp: print(f"{alvo}: pai sem dir-stats"); continue
    resto_n, resto_b, falhas = tp[0], tp[1], []
    for e in os.scandir(pai):
        if e.path == alvo: continue
        try: st = e.stat(follow_symlinks=False)
        except OSError: falhas.append(e.name); continue
        resto_n -= 1
        if e.is_dir(follow_symlinks=False):
            t = ds(e.path)
            if t is None: falhas.append(e.name); continue
            resto_n -= t[0]; resto_b -= t[1]
        elif not (st.st_flags & 0x20):
            resto_b -= st.st_blocks * 512
    resto_n -= 1  # a própria pasta alvo
    msg = f"descendentes={resto_n} bytes={resto_b} ({resto_b/2**30:.3f} GiB)"
    print(f"{alvo}: {msg}" + (f"  INCERTO, irmãs sem dir-stats: {falhas}" if falhas else ""))

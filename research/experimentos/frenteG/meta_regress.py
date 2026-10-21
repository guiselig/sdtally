#!/usr/bin/env python3
"""Frente G: mede o overhead de metadados do APFS por objeto numa imagem descartável.

Cria uma imagem APFS esparsa (sem root), aplica passos cumulativos (arquivos vazios,
pastas, xattrs, arquivos de 1 byte, clones), e depois de cada passo lê do kernel:
  - ATTR_VOL_SPACEUSED (= fs_used do seletor 7 do AppleAPFSUserClient)
  - contadores do volume (seletor 0x1e): arquivos, pastas, nós omap/fsroot/extentref
Saída: CSV em stdout. Ejeta e apaga a imagem no fim.
Uso: meta_regress.py <dir_scratch> <caminho_apfsuc>
"""
import os, re, subprocess, sys, time, plistlib, shutil, ctypes, signal
# alarm/kill externos viram exceção para o finally ejetar e apagar a imagem
for sg in (signal.SIGALRM, signal.SIGTERM): signal.signal(sg, lambda *a: sys.exit(1))
libc = ctypes.CDLL(None, use_errno=True)
def setxattr(path, name, val):
    # macOS: setxattr(path, name, value, size, position, options)
    if libc.setxattr(path.encode(), name.encode(), val, len(val), 0, 0) != 0:
        raise OSError(ctypes.get_errno(), path)

scratch, apfsuc = sys.argv[1], sys.argv[2]
img = os.path.join(scratch, "g.sparseimage")
mnt = os.path.join(scratch, "mnt")
os.makedirs(mnt, exist_ok=True)
subprocess.run(["hdiutil", "create", "-quiet", "-type", "SPARSE", "-fs", "APFS", "-size", "8g",
                "-volname", "SDCG", img], check=True)
out = subprocess.run(["hdiutil", "attach", "-nobrowse", "-plist", "-mountpoint", mnt, img],
                     check=True, capture_output=True).stdout
ents = plistlib.loads(out)["system-entities"]
vol = next(e["dev-entry"] for e in ents if e.get("mount-point") == mnt)   # /dev/diskNs1
cont = re.match(r"/dev/(disk\d+)s\d+", vol).group(1)

def measure(label, n=0):
    os.sync(); time.sleep(6); os.sync(); time.sleep(1)   # deixa o APFS fechar a transação
    t = subprocess.run([apfsuc, cont, "0"], capture_output=True, text=True, timeout=30).stdout
    used = int(re.search(r"sel 7 idx 0 = 0x0: \d+ (\d+)", t).group(1))
    g = lambda k: int(re.search(r"%s +(\d+)" % k, t).group(1))
    row = [label, n, used, g("arquivos"), g("pastas"), g("nos omap"), g("nos fsroot"), g("nos extentref")]
    print(",".join(map(str, row)), flush=True)

def tree_alloc(root):
    # soma de st_blocks*512 (alocado) de tudo que foi criado
    s = 0
    for dp, dn, fn in os.walk(root):
        for f in fn:
            s += os.lstat(os.path.join(dp, f)).st_blocks * 512
    return s

print("passo,n,used,arquivos,pastas,omap,fsroot,extentref", flush=True)
try:
    measure("vazio")
    # A: arquivos vazios (alocado 0), nomes de 24 caracteres, cumulativo 1k -> 10k -> 100k
    d = os.path.join(mnt, "A"); os.mkdir(d); k = 0
    for target in (1000, 10000, 100000):
        while k < target:
            open(os.path.join(d, "arquivo_vazio_%010d" % k), "w").close(); k += 1
        measure("A_arquivos_vazios", target)
    # B: pastas vazias, cumulativo 1k -> 10k -> 50k
    d = os.path.join(mnt, "B"); os.mkdir(d); k = 0
    for target in (1000, 10000, 50000):
        while k < target:
            os.mkdir(os.path.join(d, "pasta_vazia_%012d" % k)); k += 1
        measure("B_pastas_vazias", target)
    # C: xattr de 100 bytes (embutido) em 10k e 100k dos arquivos de A
    k = 0
    for target in (10000, 100000):
        while k < target:
            setxattr(os.path.join(mnt, "A", "arquivo_vazio_%010d" % k), "user.sdc", b"x" * 100); k += 1
        measure("C_xattr_100B", target)
    # D: arquivos de 1 byte (1 bloco alocado cada), 10k -> 50k
    d = os.path.join(mnt, "D"); os.mkdir(d); k = 0
    for target in (10000, 50000):
        while k < target:
            with open(os.path.join(d, "arquivo_1byte_%010d" % k), "wb") as f: f.write(b"z")
            k += 1
        measure("D_arquivos_1B", target)
    print("# alocado arvore D (st_blocks) = %d" % tree_alloc(os.path.join(mnt, "D")), flush=True)
    # E: clones completos de um arquivo de 1 MiB (cp -c), 1k -> 10k
    d = os.path.join(mnt, "E"); os.mkdir(d)
    src = os.path.join(d, "orig.bin")
    with open(src, "wb") as f: f.write(os.urandom(1 << 20))
    measure("E_original_1MiB", 1)
    k = 0
    for target in (1000, 10000):
        while k < target:
            subprocess.run(["cp", "-c", src, os.path.join(d, "clone_%010d" % k)], check=True); k += 1
        measure("E_clones", target)
    # F: apagar tudo e ver o metadado voltar
    for sub in "ABDE":
        shutil.rmtree(os.path.join(mnt, sub))
    measure("F_apagado", 0)
finally:
    subprocess.run(["hdiutil", "detach", "-quiet", mnt])
    os.remove(img)

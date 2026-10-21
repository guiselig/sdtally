#!/usr/bin/env python3
"""Confere que os blocos físicos do conteúdo enxertado (.AssetData) estão dentro do
*_Cryptex.dmg do mesmo asset. Só leitura (open + fcntl F_LOG2PHYS_EXT).
Uso: graft_check.py [AssetsV2]"""
import fcntl, glob, os, struct, sys

F_LOG2PHYS_EXT, BLK = 65, 4096

def extents(path):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        size, off, out = os.fstat(fd).st_size, 0, []
        while off < size:
            r = fcntl.fcntl(fd, F_LOG2PHYS_EXT, struct.pack("=Iqq", 0, size - off, off))
            _, contig, dev = struct.unpack("=Iqq", r)
            if contig <= 0:
                break
            if dev >= 0:
                out.append((dev, -(-(dev + contig) // BLK) * BLK))
            off += contig
        return out
    finally:
        os.close(fd)

def inside(ex, host):  # host ordenado e sem sobreposição
    return all(any(hs <= s and e <= he for hs, he in host) for s, e in ex)

root = sys.argv[1] if len(sys.argv) > 1 else "/System/Volumes/Data/System/Library/AssetsV2"
tot_in = tot_out = files = 0
for ad in sorted(glob.glob(f"{root}/*/*/*.asset/.AssetData")):
    asset = os.path.dirname(ad)
    host = sorted(x for d in glob.glob(f"{asset}/AssetData/Restore/*_Cryptex.dmg") for x in extents(d))
    b_in = b_out = 0
    for dp, _, fs in os.walk(ad):
        for f in fs:
            p = os.path.join(dp, f)
            if os.path.islink(p):
                continue
            try:
                ex = extents(p)
            except OSError as e:
                print("  erro", e.errno, p)
                continue
            files += 1
            n = sum(e - s for s, e in ex)
            if inside(ex, host):
                b_in += n
            else:
                b_out += n
    tot_in += b_in
    tot_out += b_out
    print(f"{b_in:>13} dentro  {b_out:>9} fora  host {sum(e-s for s,e in host):>13}  {asset.split('/')[-3]}")
print(f"arquivos {files} | dentro do dmg {tot_in} B ({tot_in/2**30:.3f} GiB) | fora {tot_out} B")
assert tot_out == 0, "há bloco enxertado fora do dmg"

#!/usr/bin/env python3
"""icloud: estima "iCloud Drive" do Ajustes > Armazenamento (com.apple.CloudDocsDaemon.StorageManagement).

Fórmula lida nos binários (CloudDocsStorageManagement + fileproviderd + iCloudDriveCore):
  valor = A + B
  A = FPDaemonConnection calculateNonPurgeableSpaceUsageOfDomain (fileproviderd, FPFS):
      SUM(metadata_physical_size) do banco FPFS do domínio, só itens NÃO despejáveis
      (metadata_is_evictable & máscara = 0) e fora de "pacote dataless". Máscara exata não lida:
      aqui "= 0". Os valores vistos são 0, 5 e 0x50005; qualquer máscara que não pegue os bits 0 ou 2
      daria dezenas de GB, o que o log (1,9 GB e 246 MB) desmente, então "= 0" é o único resultado possível.
  B = nonPurgeable do bird = max(0, totalSize - P'), totalSize = dir-stats APFS da sessão do bird
      (~/Library/Application Support/CloudDocs/session; o bird usa outro fsctl, 0xC0284A20, aqui é o
      dir-stats "get" 0xC1104A71 com buffer zerado, o mesmo de frenteH/dirstats.c) e
      P' = graveyard (xattr com.apple.bird.graveyard.size#N) + stage x (liveItems) + stage u (uploads)
         + max(0, freelist de client.db e server.db - 1 MiB). ciconia: não medido.
  Conteúdo baixado e despejável do iCloud Drive NÃO entra (vai para Dados do Sistema/Documentos).
Só leitura: bancos copiados para temporário; fsctl só com buffer zerado; dataless e atime desligados.
uso: python3 icloud.py
"""
import ctypes
import glob
import os
import shutil
import sqlite3
import subprocess
import tempfile

HOME = os.path.expanduser("~")
SESSION = f"{HOME}/Library/Application Support/CloudDocs/session"
libc = ctypes.CDLL(None, use_errno=True)
# IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES=3 e IOPOL_TYPE_VFS_ATIME_UPDATES=2, escopo processo, OFF=1
for t in (3, 2):
    if libc.setiopolicy_np(t, 0, 1):
        raise SystemExit("setiopolicy_np falhou")


def copia(src):
    d = tempfile.mkdtemp()
    for f in glob.glob(glob.escape(src) + "*"):
        if os.path.isfile(f):
            shutil.copy2(f, d)
    return d, os.path.join(d, os.path.basename(src))


def fpfs_uuid():
    r = subprocess.run(["/usr/bin/fileproviderctl", "dump", "-l", "com.apple.CloudDocs.iCloudDriveFileProvider"],
                       capture_output=True, text=True, timeout=120)
    for ln in r.stdout.splitlines():
        if "+ database:" in ln and "UUID=" in ln:
            return ln.split("UUID=")[1].split()[0]
    return None


def parte_a():
    u = fpfs_uuid()
    if not u:
        return None, "UUID do banco FPFS não obtido"
    for db in glob.glob(f"{HOME}/Library/Application Support/FileProvider/*/database/db"):
        d, cp = copia(db)
        try:
            c = sqlite3.connect(cp)
            (uu,) = c.execute("SELECT db_uuid FROM engine_state").fetchone()
            if (uu.decode() if isinstance(uu, bytes) else str(uu)).upper() != u.upper():
                continue
            n, s = c.execute("SELECT COUNT(*), SUM(metadata_physical_size) FROM FS_snapshot WHERE metadata_is_evictable = 0 "
                             "AND NOT (metadata_is_dataless AND metadata_is_package)").fetchone()
            return s or 0, f"{n} itens não despejáveis em {os.path.dirname(os.path.dirname(db))}"
        except sqlite3.Error:
            continue
        finally:
            shutil.rmtree(d)
    return None, f"banco FPFS com UUID {u} não achado"


def dirstats(path):
    b = ctypes.create_string_buffer(0x110)          # zerado = "get"; buf+4 nunca é escrito
    if libc.fsctl(path.encode(), ctypes.c_ulong(0xC1104A71), b, 0):
        return None
    return int.from_bytes(b.raw[0x40:0x48], "little")


def alocado(path):
    t = 0
    for r, ds, fs in os.walk(path):
        for f in fs:
            try:
                t += os.lstat(os.path.join(r, f)).st_blocks * 512
            except OSError:
                pass
    return t


def graveyard():
    # os.getxattr não existe no Python do macOS: getxattr da libc, XATTR_NOFOLLOW=1.
    libc.getxattr.restype = ctypes.c_ssize_t
    libc.getxattr.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.c_uint32, ctypes.c_int]
    t, buf = 0, ctypes.create_string_buffer(64)
    for r, ds, fs in os.walk(SESSION):
        for n in ds + fs:
            k = libc.getxattr(os.path.join(r, n).encode(), b"com.apple.bird.graveyard.size#N", buf, 64, 0, 1)
            if k > 0:   # formato do valor não lido: número em texto ou inteiro binário
                raw = buf.raw[:k]
                t += int(raw) if raw.strip().isdigit() else int.from_bytes(raw[:8], "little")
    return t


def freelist(name):
    d, cp = copia(f"{SESSION}/db/{name}")
    try:
        c = sqlite3.connect(cp)
        return c.execute("PRAGMA freelist_count").fetchone()[0] * c.execute("PRAGMA page_size").fetchone()[0]
    finally:
        shutil.rmtree(d)


def main():
    a, msg = parte_a()
    print(f"A (FPFS não despejável) = {a if a is not None else 'não medido'}  ({msg})")
    tot = dirstats(SESSION)
    if tot is None:
        print("B: dir-stats da sessão não disponível: não medido")
        return
    g, x, u = graveyard(), alocado(f"{SESSION}/x"), alocado(f"{SESSION}/u")
    fl = max(0, freelist("client.db") + freelist("server.db") - (1 << 20))
    b = max(0, tot - (g + x + u + fl))
    print(f"B (bird) = {b}  (dir-stats {tot} - graveyard {g} - x {x} - u {u} - freelist {fl}; ciconia não medido)")
    if a is not None:
        print(f"iCloud Drive (estimativa) = {a + b}")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""musica: reproduz "Music" (e "TV") do Ajustes > Armazenamento lendo a biblioteca do app.

O que a extensão faz (lido nos binários, Frente P):
  MusicStorageExtension/TVStorageExtension -> AMPLFrameworkClient (AMPLibrary.framework) ->
  AMPLibraryAgent -storageInfoForDomains:withMediaKinds:ignorePurgeable:withReply: ->
  "totalBytes" = Σ FileSize gravado na biblioteca, só das faixas cujo arquivo existe no volume
  da pasta pessoal. Não é o tamanho da pasta: arquivo fora da biblioteca não entra.
O agente recusa clientes sem TCC "Mídia e Apple Music" e sem o entitlement restrito
com.apple.amp.library.client, então aqui a biblioteca é lida direto do disco (só leitura):
Library.musicdb/Library.tvdb = cabeçalho "hfma" + AES-128-ECB (chave pública do formato,
"BHUILuilfghuila3") nos primeiros max_crypt bytes + zlib. Faixa = registro "itma"; local =
"boma" subtipo 11 (URL file://); FileSize = u64 no offset 316 do primeiro "boma" subtipo 1.

uso: python3 musica.py [music|tv] [biblioteca.musiclibrary|.tvlibrary] [--lista]
saída: total igual ao "_SIZE_" da extensão; com --lista, "FileSize<TAB>caminho" por faixa.
"""
import os
import struct
import subprocess
import sys
import urllib.parse
import zlib

KEY = b"BHUILuilfghuila3"


def decode(path):
    d = open(path, "rb").read()
    if d[:4] != b"hfma":
        raise SystemExit(f"{path}: não é hfma")
    hl = struct.unpack_from("<I", d, 4)[0]
    mc = struct.unpack_from("<I", d, 0x54)[0]
    body = d[hl:]
    n = min(mc, len(body))
    n -= n % 16
    # openssl do sistema (LibreSSL) para não depender de pacote Python; timeout por regra.
    r = subprocess.run(["/usr/bin/openssl", "enc", "-d", "-aes-128-ecb", "-K", KEY.hex(), "-nopad"],
                       input=body[:n], capture_output=True, timeout=60)
    if r.returncode:
        raise SystemExit(f"openssl: {r.stderr.decode(errors='replace')}")
    return zlib.decompress(r.stdout + body[n:])


def tracks(d):
    """Devolve (FileSize, caminho ou None) de cada registro itma."""
    out, cur, i = [], None, 0
    while i < len(d) - 8:
        tag = d[i:i + 4]
        l1 = struct.unpack_from("<I", d, i + 4)[0]
        if tag == b"hsma":          # seção: entra nela
            i += l1
            continue
        if tag == b"boma":
            tot = struct.unpack_from("<I", d, i + 8)[0]
            st = struct.unpack_from("<I", d, i + 12)[0]
            if cur is not None:
                if st == 1 and cur[0] is None and tot >= 324:
                    cur[0] = struct.unpack_from("<Q", d, i + 316)[0]
                elif st == 11:
                    url = d[i + 36:i + tot].split(b"\0")[0].decode("utf-8", "replace")
                    cur[1] = urllib.parse.unquote(url)[7:].rstrip("/") if url.startswith("file://") else None
            i += tot
            continue
        cur = [None, None] if tag == b"itma" else None
        if cur is not None:
            out.append(cur)
        if l1 < 8:
            raise SystemExit(f"registro inválido em {i}")
        i += l1
    return out


def main():
    tv = len(sys.argv) > 1 and sys.argv[1] == "tv"
    home = os.path.expanduser("~")
    args = [a for a in sys.argv[2:] if not a.startswith("--")]
    lib = args[0] if args else (f"{home}/Movies/TV/TV Library.tvlibrary" if tv
                                else f"{home}/Music/Music/Music Library.musiclibrary")
    db = os.path.join(lib, "Library.tvdb" if tv else "Library.musicdb")
    home_dev = os.stat(home).st_dev
    total = n_ok = n_sem_local = n_ausente = n_outro_vol = 0
    lista = "--lista" in sys.argv
    for size, path in tracks(decode(db)):
        if not path:
            n_sem_local += 1
            continue
        try:
            st = os.stat(path)           # só stat: nada é aberto nem listado
        except OSError:
            n_ausente += 1
            continue
        if st.st_dev != home_dev:        # o agente compara o volume do arquivo com o da home
            n_outro_vol += 1
            continue
        n_ok += 1
        total += size or 0
        if lista:
            print(f"{size}\t{path}")
    print(f"{'TV' if tv else 'Music'} (como o Ajustes) = {total}  faixas contadas {n_ok}, "
          f"sem arquivo local {n_sem_local}, arquivo ausente {n_ausente}, outro volume {n_outro_vol}  [{db}]",
          file=sys.stderr if lista else sys.stdout)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""todas: roda os reprodutores da Frente P e monta a conta da tela do Ajustes > Armazenamento.

  Dados do Sistema = (usado do Data − purgeable) − Σ categorias      (síntese §1 e Frente M §2.3)
  usado do Data = CapacityInUse do volume de papel Data (diskutil apfs list -plist);
  purgeable = ImportantUsage − livre (stm_categorias volume).
O macOS não entra: a tela soma macOS em "usado" e tira de novo, então ele se cancela.
Categoria sem valor sai "nao_medido" e a conta fica "nao_medido" (nunca 0 no lugar).
iCloud Drive é estimativa (grau B): a linha final diz isso.

uso: python3 todas.py <pasta com os binários stm_categorias, fotos_pl e livros já compilados>
"""
import os
import plistlib
import re
import subprocess
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
BIN = sys.argv[1] if len(sys.argv) > 1 else AQUI


def roda(cmd, t):
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=t)
        return r.stdout
    except (subprocess.TimeoutExpired, OSError) as e:
        print(f"falhou {cmd[0]}: {e}", file=sys.stderr)
        return ""


def num(padrao, texto):
    m = re.search(padrao, texto, re.M)
    return int(m.group(1)) if m else None


stm = roda([f"{BIN}/stm_categorias", "developer", "otherusers", "trash", "ios", "garageband", "mail",
            "apps", "documents", "volume"], 600)
tot = {}
for c, v in re.findall(r"^(\w+)\s+\t(-?\d+)\tTOTAL$", stm, re.M):
    tot.setdefault(c, int(v))   # Mail sai 2x (com e sem cache do CacheDelete); vale a 1ª
fotos = roda([f"{BIN}/fotos_pl"], 300)
m = re.search(r"^lógico.*r2=(\d+) r3=\d+ r4=(\d+)", fotos, re.M)
py = [sys.executable]
musica = roda(py + [f"{AQUI}/musica.py", "music"], 120) + roda(py + [f"{AQUI}/musica.py", "tv"], 120)
bancos = roda(py + [f"{AQUI}/bancos.py", "messages", "podcasts"], 120)
livros = roda([f"{BIN}/livros"], 120)
icloud = roda(py + [f"{AQUI}/icloud.py"], 300)

cats = {
    "Aplicativos": tot.get("apps"),
    "Documentos": tot.get("documents"),
    "Música": num(r"^Music \(como o Ajustes\) = (\d+)", musica),
    "Fotos": int(m.group(1)) - int(m.group(2)) if m else None,
    "TV": num(r"^TV \(como o Ajustes\) = (\d+)", musica),
    "Livros": num(r"^Books \(como o Ajustes\) = (\d+)", livros),
    "Podcasts": num(r"^Podcasts = (\d+)", bancos),
    "Lixo": tot.get("trash"),
    "GarageBand": tot.get("garageband"),
    "Arquivos do iOS": tot.get("ios"),
    # Estas cinco são o agregado "Developer and Others" (DISK_MISC) do log.
    "Desenvolvedor": tot.get("developer"),
    "iCloud Drive (B)": num(r"^iCloud Drive \(estimativa\) = (\d+)", icloud),
    "Mail": tot.get("mail"),
    "Mensagens": num(r"^Messages \[sem ck_sync_state.*\]= (\d+)", bancos),
    "Outros usuários": tot.get("otherusers"),
}
purg = num(r"^volume\s+\t(-?\d+)\tpurgeable$", stm)
apfs = plistlib.loads(subprocess.run(["diskutil", "apfs", "list", "-plist"], capture_output=True, timeout=60).stdout)
dev = plistlib.loads(subprocess.run(["diskutil", "info", "-plist", "/System/Volumes/Data"], capture_output=True,
                                    timeout=60).stdout).get("DeviceIdentifier")
usado = next((v["CapacityInUse"] for c in apfs["Containers"] for v in c["Volumes"] if v["DeviceIdentifier"] == dev), None)

f = lambda v: "nao_medido" if v is None else f"{v:,}".replace(",", ".")
for k, v in cats.items():
    print(f"{k:<20}\t{f(v)}")
misc = [cats[k] for k in ("Desenvolvedor", "iCloud Drive (B)", "Mail", "Mensagens", "Outros usuários")]
print(f"{'Developer and Others':<20}\t{f(None if None in misc else sum(misc))}")
soma = None if None in cats.values() else sum(cats.values())
print(f"{'Σ categorias':<20}\t{f(soma)}")
print(f"{'usado do Data':<20}\t{f(usado)}\t({dev}, CapacityInUse)")
print(f"{'purgeable':<20}\t{f(purg)}\t(ImportantUsage − livre)")
sd = None if None in (soma, usado, purg) else usado - purg - soma
print(f"{'Dados do Sistema':<20}\t{f(sd)}\t(usado − purgeable − Σ; iCloud Drive é estimativa)")

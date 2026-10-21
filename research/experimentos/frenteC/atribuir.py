#!/usr/bin/env python3
"""atribuir.py - Frente C: dono verificavel de cada Container / Group Container. Somente leitura.

Para cada ~/Library/{Containers,Group Containers,Daemon Containers}/<x>:
  id = MCMMetadataIdentifier de .com.apple.containermanagerd.metadata.plist
  Regras, na ordem; a primeira que PROVA vence:
    LS           LaunchServices tem app com esse bundle id (ls_installed.rs)
    pluginkit    extensao registrada com esse id (pluginkit -mAD)
    sistema      Info.plist com esse CFBundleIdentifier dentro do volume de sistema selado
                 (/System, cryptex do Safari). pluginkit NAO lista todas as ExtensionKit.
    bundle       Info.plist ou assinatura (codesign Identifier=) com esse id dentro de um app
                 instalado. O prefixo do id so escolhe ONDE procurar; a prova e o arquivo.
    grupo        (Group Containers) um container COM DONO PROVADO declara o grupo em
                 com.apple.security.application-groups (entitlements gravados no metadata)
    entitlement  (Group Containers) um app instalado declara o grupo na propria assinatura
    platform     Owners.validationCategory == "platform" ou Daemon Containers
    sem-dono     nenhuma prova. NAO prova orfandade: significa "dono nao localizado".
Saida TSV: tipo  id  criador  validacao  regra  prova
Uso: python3 atribuir.py ./ls_installed > donos.tsv
Custo medido (macOS 27, 1.050 containers, ~100 apps): ~15 s, dominado por codesign.
"""
import json, os, plistlib, re, subprocess, sys

LS_BIN = sys.argv[1] if len(sys.argv) > 1 else "./ls_installed"
LIB = os.path.expanduser("~/Library")
META = ".com.apple.containermanagerd.metadata.plist"
SYSTEM_ROOTS = ["/System/Library", "/System/Applications", "/System/iOSSupport",
                "/System/Volumes/Preboot/Cryptexes/App", "/Library/Apple"]


def load_plist(path):
    try:
        with open(path, "rb") as f:
            return plistlib.load(f)
    except Exception:
        return None


def ls_lookup(ids):
    out = subprocess.run([LS_BIN], input="\n".join(ids) + "\n", capture_output=True, text=True).stdout
    return {c[0]: c[2] for c in (l.split("\t") for l in out.splitlines()) if c[1] != "0"}


def pluginkit_ids():
    out = subprocess.run(["pluginkit", "-mAD", "-v"], capture_output=True, text=True).stdout
    return {m.group(1) for m in re.finditer(r"^\s*[+!=-]?\s*([A-Za-z0-9._-]+)\(", out, re.M)}


def system_ids():
    """CFBundleIdentifier de todo bundle no volume selado (~7 s)."""
    ids = set()
    for base in SYSTEM_ROOTS:
        for root, _, files in os.walk(base):
            if "Info.plist" in files:
                p = load_plist(os.path.join(root, "Info.plist"))
                if isinstance(p, dict) and p.get("CFBundleIdentifier"):
                    ids.add(p["CFBundleIdentifier"])
    return ids


def entitlements(path):
    out = subprocess.run(["codesign", "-d", "--entitlements", "-", "--xml", path], capture_output=True).stdout
    try:
        return plistlib.loads(out)
    except Exception:
        return {}


def app_groups(apps):
    """grupo -> app instalado que o declara na assinatura."""
    g = {}
    for app in apps:
        for grp in entitlements(app).get("com.apple.security.application-groups") or []:
            g.setdefault(grp, app)
    return g


def inside_bundle(bid, ls):
    parts = bid.split(".")
    for k in range(len(parts) - 1, 1, -1):
        pid = ".".join(parts[:k])
        parent = ls.get(pid) or ls_lookup([pid]).get(pid)
        if not parent:
            continue
        for root, _, files in os.walk(parent):
            if "Info.plist" in files:
                p = load_plist(os.path.join(root, "Info.plist"))
                if isinstance(p, dict) and p.get("CFBundleIdentifier") == bid:
                    return os.path.join(root, "Info.plist")
        # ponytail: um codesign por executavel (lento); so roda para os poucos que chegam aqui
        for root, _, files in os.walk(parent):
            if "/Frameworks/" in root:
                continue
            for fn in files:
                fp = os.path.join(root, fn)
                if os.access(fp, os.X_OK) and not os.path.islink(fp):
                    r = subprocess.run(["codesign", "-dv", fp], capture_output=True, text=True).stderr
                    if f"Identifier={bid}\n" in r:
                        return fp + " (codesign)"
        return None
    return None


def main():
    rows = []
    for kind in ("Containers", "Group Containers", "Daemon Containers"):
        base = os.path.join(LIB, kind)
        for d in sorted(os.listdir(base)):
            m = load_plist(os.path.join(base, d, META))
            if not m:
                continue
            info = m.get("MCMMetadataInfo", {})
            owners = json.loads(info["Owners"]) if info.get("Owners") else [{}]
            ents = info.get("SandboxProfileDataValidationInfo", {}).get("Entitlements", {})
            rows.append(dict(kind=kind, id=m["MCMMetadataIdentifier"], creator=m.get("MCMMetadataCreator") or "",
                             vc=owners[0].get("validationCategory", ""),
                             groups=ents.get("com.apple.security.application-groups") or []))
    ls = ls_lookup(sorted({r["id"] for r in rows}))
    pk, sysids = pluginkit_ids(), system_ids()
    apps = subprocess.run(["mdfind", "kMDItemContentType == 'com.apple.application-bundle'"],
                          capture_output=True, text=True).stdout.split("\n")
    apps = [a for a in apps if a and not a.startswith("/System/")]
    g2app = app_groups(apps)
    g2c = {}
    for r in rows:
        for g in r["groups"]:
            g2c.setdefault(g, []).append(r["id"])
    owned = set()  # containers com dono provado; Containers vem antes de Group Containers em rows
    for r in rows:
        i, gc = r["id"], r["kind"] == "Group Containers"
        if i in ls:
            rule, proof = "LS", ls[i]
        elif i in pk:
            rule, proof = "pluginkit", i
        elif i in sysids:
            rule, proof = "sistema", "Info.plist no volume selado"
        elif gc and [c for c in g2c.get(i, []) if c in owned]:
            rule, proof = "grupo", ",".join([c for c in g2c[i] if c in owned][:3])
        elif gc and i in g2app:
            rule, proof = "entitlement", g2app[i]
        elif r["vc"] == "platform" or r["kind"] == "Daemon Containers":
            rule, proof = "platform", r["vc"] or "Daemon Containers"
        else:
            p = None if gc else inside_bundle(i, ls)
            rule, proof = ("bundle", p) if p else ("sem-dono", "")
        if rule != "sem-dono":
            owned.add(i)
        print(r["kind"], i, r["creator"], r["vc"], rule, proof, sep="\t")


if __name__ == "__main__":
    main()

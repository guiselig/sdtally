#!/usr/bin/env python3
"""Frente G: bytes gravados pelo próprio MobileAsset para cada asset automático.

Lê /System/Library/AssetsV2/persisted/AutoAssetDescriptors/*.state (legível sem root,
mesmo quando a pasta do asset em AssetsV2 é 'restricted'/EPERM). Cada arquivo é um plist
com um NSKeyedArchiver de MADAutoAssetDescriptor: assetType, assetID, isOnFilesystem,
downloadedFilesystemBytes e, em metadata, _UnarchivedSize. Nada é estimado aqui: são os
números que o mobileassetd gravou. Soma só isOnFilesystem = True, um por assetID.
"""
import glob, os, plistlib, sys
from collections import defaultdict

D = "/System/Library/AssetsV2/persisted/AutoAssetDescriptors"

def deref(o, v):
    return o[v.data] if isinstance(v, plistlib.UID) else v

seen = {}
for f in glob.glob(os.path.join(D, "*.state")):
    try:
        top = plistlib.load(open(f, "rb"))["SUCorePersistedStatePolicySecureCodedObjectsFields"]
        o = plistlib.loads(top["assetDescriptor"])["$objects"]
    except Exception as e:
        print("# ilegível:", f, e, file=sys.stderr); continue
    r = o[1]
    g = lambda k: deref(o, r.get(k))
    md = g("metadata") or {}
    meta = {}
    if isinstance(md, dict) and "NS.keys" in md:
        meta = {deref(o, k): deref(o, v) for k, v in zip(md["NS.keys"], md["NS.objects"])}
    seen[g("assetID")] = dict(tipo=g("assetType"), spec=g("AssetSpecifier"), no_disco=g("isOnFilesystem"),
                              fs=g("downloadedFilesystemBytes") or 0, unarch=meta.get("_UnarchivedSize") or 0)

por_tipo = defaultdict(lambda: [0, 0, 0])
for a in seen.values():
    if a["no_disco"] is True:
        t = por_tipo[a["tipo"]]; t[0] += 1; t[1] += a["fs"]; t[2] += a["unarch"]
print(f"descritores {len(seen)}; no disco {sum(a['no_disco'] is True for a in seen.values())}")
print(f"{'assets':>6} {'fsBytes GiB':>11} {'_Unarchived GiB':>15}  tipo")
tot = [0, 0, 0]
for t, (n, fs, un) in sorted(por_tipo.items(), key=lambda kv: -kv[1][1]):
    tot = [tot[0] + n, tot[1] + fs, tot[2] + un]
    print(f"{n:>6} {fs/2**30:>11.3f} {un/2**30:>15.3f}  {t}")
print(f"{tot[0]:>6} {tot[1]/2**30:>11.3f} {tot[2]/2**30:>15.3f}  TOTAL")

#!/usr/bin/env python3
"""Resumo do job `fontes` do CI (research/mapa-de-fontes.md §7.1): compara o que o painel do
Ajustes gravou no log com o que o frenteq e o sdtally medem, na mesma VM, minutos depois.

Lê da pasta atual (o que faltar sai como "ausente"): simbolos.json, painel.ndjson (log show
--style ndjson de com.apple.storage e com.apple.settings.Storage), categorias.json (frenteq),
sdtally.json (sdtally --json) e dirstats.md (dirstats_cmp.py). Saída: markdown em stdout.
"""
import json
import os


def ler_json(nome):
    try:
        with open(nome) as f:
            return json.load(f)
    except (OSError, ValueError) as e:
        return {"_erro": str(e)}


def eventos(nome):
    out = []
    try:
        with open(nome) as f:
            for linha in f:
                try:
                    d = json.loads(linha)
                except ValueError:
                    continue
                if isinstance(d, dict) and "eventMessage" in d:
                    out.append((d.get("timestamp", ""), d["eventMessage"]))
    except OSError:
        pass
    return out


def painel(evs):
    disk = cats = sys_size = None
    for ts, m in evs:
        if m.startswith("Disk: "):
            o = {"ts": ts}
            for parte in m[6:].split(" / "):
                if ": " in parte:
                    k, v = parte.split(": ", 1)
                    try:
                        o[k.strip()] = int(v.strip())
                    except ValueError:
                        pass
            disk = o
        elif "STMVolumeCategory" in m:
            lst = {}
            for chunk in m.split("<STMVolumeCategory:")[1:]:
                toks = chunk.split(">")[0].split()
                if len(toks) >= 3:
                    try:
                        lst[toks[1]] = (" ".join(toks[2:-1]), int(toks[-1]))
                    except ValueError:
                        pass
            if lst:
                cats = (ts, lst)
        elif m.startswith("System size set to "):
            try:
                sys_size = (ts, int(m.split()[4]))
            except (IndexError, ValueError):
                pass
    return disk, cats, sys_size


def gb(b):
    return "—" if b is None else f"{b / 1e9:.3f}"


def main():
    p = print
    sim = ler_json("simbolos.json")
    p(f"## Fontes no macOS {sim.get('macos', '?')}\n")
    p("### Símbolos privados\n")
    for k, v in sim.items():
        if k != "macos":
            p(f"- `{k}`: {v}")

    evs = eventos("painel.ndjson")
    disk, cats, sys_size = painel(evs)
    p(f"\n### Log do painel ({len(evs)} eventos)\n")
    p(f"- `Disk:` {'ausente' if not disk else disk}")
    p(f"- `System size set to`: {'ausente' if not sys_size else sys_size}")
    p(f"- `STMVolumeCategory`: {'ausente' if not cats else f'{len(cats[1])} categorias às {cats[0]}'}")

    fq = ler_json("categorias.json")
    medido = {c["id"]: c.get("bytes") for c in fq.get("categorias", [])}
    if cats:
        p("\n### Categorias: log do painel × frenteq\n")
        p("| id | nome | log | frenteq | diferença |")
        p("|---|---|---:|---:|---:|")
        for cid, (nome, b) in sorted(cats[1].items(), key=lambda x: -x[1][1]):
            m = medido.get(cid)
            dif = "" if m is None else ("igual" if m == b else f"{m - b:+d} ({(m - b) / b * 100 if b else 0:+.2f} %)")
            p(f"| `{cid}` | {nome} | {b} | {m if m is not None else '—'} | {dif} |")
        sem_log = sorted(set(medido) - set(cats[1]))
        if sem_log:
            p(f"\nMedidas pelo frenteq sem linha no log: {', '.join(sem_log)}")

    sd = ler_json("sdtally.json")
    tela = sd.get("settings_screen", {})
    mac = tela.get("macos", {}).get("bytes")
    p("\n### macOS e Dados do Sistema\n")
    p(f"- macOS reproduzido (Σ CapacityInUse − Data): {mac} ({gb(mac)} GB)")
    if sys_size:
        p(f"- macOS da tela (`System size set to`): {sys_size[1]} → {'igual' if mac == sys_size[1] else f'diferença {mac - sys_size[1]:+d}' if mac else '—'}")
    if disk:
        p(f"- `SYS` do serviço: {disk.get('SYS')} ({gb(disk.get('SYS'))} GB)")
    sd_log = cats[1].get("com.apple.StorageManagement.SystemData", (None, None))[1] if cats else None
    if sd_log is None and cats:
        sd_log = next((b for n, b in cats[1].values() if n == "System Data"), None)
    p(f"- Dados do Sistema do serviço (log): {gb(sd_log)} GB")
    if sd_log is not None and disk and disk.get("SYS") is not None and mac is not None:
        p(f"- Dados do Sistema da tela = log − (macOS − SYS): {gb(sd_log - (mac - disk['SYS']))} GB")
    fq_sd = fq.get("dados_do_sistema", {})
    p(f"- Dados do Sistema pela fórmula do frenteq: {gb(fq_sd.get('bytes'))} GB ({fq_sd.get('estado')})")
    ai = tela.get("apple_intelligence", {})
    p(f"- Apple Intelligence: {ai.get('state')} {ai.get('reason') or ''}")

    if os.path.exists("dirstats.md"):
        p("\n### dir-stats × travessia\n")
        with open("dirstats.md") as f:
            p(f.read())


if __name__ == "__main__":
    main()

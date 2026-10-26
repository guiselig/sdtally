#!/usr/bin/env python3
"""Look up how macOS itself translates UI terms (button and pane names) in every app language.

Usage: scripts/termos-macos.py "Open Anyway" "Login Items" [...]
Prints JSON {english: {lang: text}} using the app's language codes. Translators must quote these
exactly instead of guessing a system label. Searches the .loctable files under /System/Library and
keeps the most frequent translation; other variants go to stderr.
"""
import json, plistlib, subprocess, sys
from collections import Counter
from pathlib import Path

# Códigos do macOS para os do catálogo do app (Localizable.xcstrings).
MAPA = {"pt_BR": "pt-BR", "pt_PT": "pt-PT", "es_419": "es-419", "es_MX": "es-419", "zh_CN": "zh-Hans",
        "zh_TW": "zh-Hant", "no": "nb"}
LINGUAS = ("ar ca cs da de el en es es-419 fi fr he hi hr hu id it ja ko ms nb nl pl pt-BR pt-PT ro ru "
           "sk sl sv th tr uk vi zh-Hans zh-Hant").split()

def main(termos):
    arquivos = subprocess.run(["find", "/System/Library", "-name", "*.loctable"], capture_output=True, text=True).stdout.split()
    contagem = {t: {} for t in termos}
    for a in arquivos:
        try:
            d = plistlib.loads(Path(a).read_bytes())
        except Exception:
            continue
        en = d.get("en")
        if not isinstance(en, dict):
            continue
        for t in termos:
            for chave in [c for c, v in en.items() if v == t]:
                for loc, tabela in d.items():
                    lang = MAPA.get(loc, loc)
                    if lang in LINGUAS and isinstance(tabela, dict) and isinstance(tabela.get(chave), str):
                        # Sem o hífen invisível (U+00AD) que o macOS usa para quebrar palavras longas.
                        contagem[t].setdefault(lang, Counter())[tabela[chave].replace("\u00ad", "")] += 1
    achados = {t: {l: c.most_common(1)[0][0] for l, c in sorted(v.items())} for t, v in contagem.items()}
    json.dump(achados, sys.stdout, ensure_ascii=False, indent=1)
    print()
    for t, v in contagem.items():
        for l, c in sorted(v.items()):
            if len(c) > 1:
                print(f"variants {t!r} {l}: {dict(c.most_common())}", file=sys.stderr)
        faltam = sorted(set(LINGUAS) - set(v))
        if faltam:
            print(f"missing {t!r}: {' '.join(faltam)}", file=sys.stderr)

if __name__ == "__main__":
    main(sys.argv[1:])

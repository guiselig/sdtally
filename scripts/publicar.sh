#!/bin/sh
# Publica uma versão: monta o .app assinado, gera o DMG e cria o release no GitHub, que é onde o botão
# "Ver versões novas" das Configurações leva (o app não acessa a rede).
#   scripts/publicar.sh    usa a versão do Cargo.toml; recusa pré-lançamento (0.1.0-dev etc.)
# Precisa de: gh autenticado, dmgbuild (pipx install dmgbuild), árvore limpa e HEAD igual ao
# origin/main (a tag aponta para ele).
set -eu
command -v dmgbuild >/dev/null || { echo "falta o dmgbuild: pipx install dmgbuild" >&2; exit 1; }

repo=$(cd "$(dirname "$0")/.." && pwd)
versao=$(sed -n 's/^version = "\(.*\)"/\1/p' "$repo/Cargo.toml" | head -1)
case $versao in
    *-*) echo "versão $versao é pré-lançamento; troque no Cargo.toml antes" >&2; exit 1 ;;
esac
[ -z "$(git -C "$repo" status --porcelain)" ] || { echo "árvore com mudanças não commitadas" >&2; exit 1; }
git -C "$repo" fetch -q origin main
head=$(git -C "$repo" rev-parse HEAD)
[ "$head" = "$(git -C "$repo" rev-parse origin/main)" ] || { echo "HEAD não é o origin/main; faça push antes" >&2; exit 1; }

# montar.sh escreve o caminho do .app na última linha.
pacote=$("$repo/app/montar.sh" | tail -1)
# DMG com o app e um atalho para Aplicativos: abrir e arrastar. Fundo, tamanho da janela e posição
# dos ícones em scripts/dmg (o dmgbuild grava o .DS_Store sem abrir o Finder).
dmg=$(dirname "$pacote")/SDTally-$versao.dmg
dmgbuild -s "$repo/scripts/dmg/ajustes.py" -D app="$pacote" -D pasta="$repo/scripts/dmg" \
    "SDTally $versao" "$dmg"
hdiutil verify -quiet "$dmg"
sha=$(shasum -a 256 "$dmg" | cut -d" " -f1)

# 0.x leva "(beta)" no título. Não usa Pre-release: o GitHub esconde pre-release da caixa
# Releases da página do repositório.
titulo="SDTally $versao"
case $versao in 0.*) titulo="$titulo (beta)" ;; esac
gh release create "v$versao" "$dmg" -R guiselig/sdtally --target "$head" --title "$titulo" --latest \
    --generate-notes --notes "Download **SDTally-$versao.dmg**, open it and drag SDTally to Applications. Requires Apple Silicon and macOS 15 or later.

SDTally is not notarized by Apple. On first launch, open System Settings > Privacy & Security and click \"Open Anyway\".

SHA-256 of the DMG: `$sha` (check with `shasum -a 256 SDTally-$versao.dmg`)."

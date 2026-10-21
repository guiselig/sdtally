#!/bin/sh
# Publica uma versão: monta o .app assinado, zipa e cria o release no GitHub, que é onde o botão
# "Ver versões novas" das Configurações leva (o app não acessa a rede).
#   scripts/publicar.sh    usa a versão do Cargo.toml; recusa pré-lançamento (0.1.0-dev etc.)
# Precisa de: gh autenticado, árvore limpa e HEAD igual ao origin/main (a tag aponta para ele).
set -eu

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
zip=$(dirname "$pacote")/SDTally-$versao.zip
ditto -c -k --keepParent "$pacote" "$zip"

# 0.x leva "(beta)" no título. Não usa Pre-release: o GitHub esconde pre-release da caixa
# Releases da página do repositório.
titulo="SDTally $versao"
case $versao in 0.*) titulo="$titulo (beta)" ;; esac
gh release create "v$versao" "$zip" -R guiselig/sdtally --target "$head" --title "$titulo" --latest \
    --generate-notes --notes "Baixe o SDTally-$versao.zip, descompacte e arraste o SDTally.app para Aplicativos.
O app não é notarizado pela Apple: na primeira vez, abra Ajustes do Sistema > Privacidade e Segurança e clique em \"Abrir Mesmo Assim\".
Pelo Homebrew: brew install guiselig/sdtally/sdtally (linha de comando) ou brew install --cask guiselig/sdtally/sdtally (app)."

# Tap do Homebrew (guiselig/homebrew-sdtally): versão e sha256 da fórmula (código da tag) e do cask (zip).
tap=$(mktemp -d)
gh repo clone guiselig/homebrew-sdtally "$tap" -- -q
fonte=$(curl -fsSL "https://github.com/guiselig/sdtally/archive/refs/tags/v$versao.tar.gz" | shasum -a 256 | cut -d' ' -f1)
binario=$(shasum -a 256 "$zip" | cut -d' ' -f1)
sed -i '' -e "s|/tags/v[^/]*\.tar\.gz|/tags/v$versao.tar.gz|" -e "s|^  sha256 \".*\"|  sha256 \"$fonte\"|" "$tap/Formula/sdtally.rb"
sed -i '' -e "s|^  version \".*\"|  version \"$versao\"|" -e "s|^  sha256 \".*\"|  sha256 \"$binario\"|" "$tap/Casks/sdtally.rb"
git -C "$tap" commit -qam "sdtally $versao"
git -C "$tap" push -q
echo "tap atualizado para $versao"

#!/bin/sh
# Monta SDTally.app com o sdtally e o helper root embutidos e assina.
# Compila fora do iCloud: o repositório vive no iCloud Drive e .build/target lá sincronizariam.
#   app/montar.sh          monta em ~/Library/Caches/sdtally-build/SDTally.app
#   app/montar.sh --abrir  monta e abre
# Assinatura: a primeira identidade "Apple Development" do chaveiro (Personal Team gratuito:
# Xcode > Settings > Accounts > Manage Certificates > + > Apple Development), ou
# SDTALLY_ASSINATURA="<nome ou SHA-1>". Sem nenhuma, assina ad hoc: o app funciona, mas o helper
# root (SMAppService) não registra e o Acesso Total ao Disco se perde a cada build.
set -eu

app_dir=$(cd "$(dirname "$0")" && pwd)
repo=$(dirname "$app_dir")
saida=${SDTALLY_BUILD:-$HOME/Library/Caches/sdtally-build}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$saida/target}

id=${SDTALLY_ASSINATURA:-$(security find-identity -v -p codesigning 2>/dev/null | sed -n 's/.*) \([0-9A-F]\{40\}\) "Apple Development.*/\1/p' | head -1)}
id=${id:--}
[ "$id" = "-" ] && echo "aviso: sem identidade Apple Development; assinando ad hoc (helper root não registra)" >&2

cargo build --release --manifest-path "$repo/Cargo.toml" --bin sdtally
swift build -c release --package-path "$app_dir" --scratch-path "$saida/swift"

# Ícone: o SDTally.icon (Icon Composer, camadas de Liquid Glass) vira Assets.car, com as variantes
# Padrão, Escuro, Transparente e Colorido que o macOS 26+ aplica sozinho, e o SDTally.icns de
# reserva para o macOS anterior. Caminhos absolutos: o actool resolve os relativos pela pasta do .icon.
icone=$saida/icone
rm -rf "$icone"
mkdir -p "$icone"
xcrun actool "$app_dir/SDTally.icon" --compile "$icone" --platform macosx --target-device mac \
    --minimum-deployment-target 15.0 --app-icon SDTally --output-partial-info-plist "$icone/parcial.plist" \
    --output-format human-readable-text --errors >&2

pacote=$saida/SDTally.app
rm -rf "$pacote"
# Helpers, não MacOS: o APFS ignora maiúsculas e MacOS/sdtally colidiria com MacOS/SDTally.
# O helper do launchd fica em MacOS: em Helpers o SMAppService falha com -67028 (relato no fórum).
mkdir -p "$pacote/Contents/MacOS" "$pacote/Contents/Helpers" "$pacote/Contents/Library/LaunchDaemons" "$pacote/Contents/Resources"
cp "$icone/Assets.car" "$icone/SDTally.icns" "$pacote/Contents/Resources/"
cp "$saida/swift/release/SDTally" "$pacote/Contents/MacOS/SDTally"
# Traduções: o SwiftPM compila o Localizable.xcstrings em .lproj dentro do SDTally_SDTally.bundle;
# no Contents/Resources o Bundle.main as acha (o SwiftUI procura os textos no Bundle.main).
cp -R "$saida/swift/release/SDTally_SDTally.bundle/Contents/Resources/"*.lproj "$pacote/Contents/Resources/"
cp "$CARGO_TARGET_DIR/release/sdtally" "$pacote/Contents/Helpers/sdtally"
cp "$app_dir/LaunchDaemon.plist" "$pacote/Contents/Library/LaunchDaemons/io.github.guiselig.sdtally.helper.plist"
versao=$(sed -n 's/^version = "\(.*\)"/\1/p' "$repo/Cargo.toml" | head -1)
commit=$(git -C "$repo" rev-parse --short HEAD 2>/dev/null || echo "—")
sed -e "s/@VERSAO@/$versao/" -e "s/@BUILD@/${versao%%-*}/" -e "s/@COMMIT@/$commit/" "$app_dir/Info.plist" > "$pacote/Contents/Info.plist"

# De dentro para fora (assinar o app depois invalidaria a assinatura de dentro). Identifiers
# fixos: o helper confere o sdtally e o app pelo identifier + time (app/Sources/SDTallyXPC).
assinar() { codesign --force --options runtime --timestamp=none --sign "$id" "$@"; }
assinar --identifier io.github.guiselig.sdtally.cli "$pacote/Contents/Helpers/sdtally"
# O helper só executa este sdtally: o CDHash dele vai no Info.plist embutido (e assinado) do
# helper, que por isso é linkado aqui, depois de o sdtally ser assinado (HelperXPC.chaveCDHash).
# Um sdtally antigo assinado pelo mesmo time não passa. Pasta própria: o linker muda só aqui.
cdhash=$(codesign -dvvv "$pacote/Contents/Helpers/sdtally" 2>&1 | sed -n 's/^CDHash=//p')
[ ${#cdhash} -eq 40 ] || { echo "erro: não deu para ler o CDHash do sdtally" >&2; exit 1; }
printf '<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n<plist version="1.0"><dict><key>SDTallyCLICDHash</key><string>%s</string></dict></plist>\n' "$cdhash" > "$saida/helper-info.plist"
# O SwiftPM não vê o plist mudar: sem o executável, ele linka de novo.
rm -f "$saida/swift-helper/release/SDTallyHelper"
swift build -c release --package-path "$app_dir" --scratch-path "$saida/swift-helper" --product SDTallyHelper \
    -Xlinker -sectcreate -Xlinker __TEXT -Xlinker __info_plist -Xlinker "$saida/helper-info.plist"
cp "$saida/swift-helper/release/SDTallyHelper" "$pacote/Contents/MacOS/SDTallyHelper"
assinar --identifier io.github.guiselig.sdtally.helper "$pacote/Contents/MacOS/SDTallyHelper"
assinar "$pacote"
codesign --verify --strict --deep "$pacote"
codesign -dv "$pacote" 2>&1 | grep -E "^(Identifier|TeamIdentifier|Signature)" >&2 || true
echo "$pacote"

[ "${1:-}" = "--abrir" ] && open "$pacote"
exit 0

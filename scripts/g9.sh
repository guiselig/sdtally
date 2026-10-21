#!/bin/sh
# Carta anti-scareware (spec da fase 6, §7 G9): palavras proibidas nos textos e, no app, badge,
# vermelho e Toggle em O que fazer. Sai com 0 se nada aparecer; senão lista e sai com 1.
# --so-rust: só os fontes Rust (o teste tests/g9.rs usa esse modo).
set -u
cd "$(dirname "$0")/.." || exit 2
alvos="src/conselho.rs src/catalogo.rs src/voltou.rs src/report/i18n.rs"
app=app/Sources/SDTally
case "${1:-}" in
    --so-rust) app= ;;
    "") alvos="$alvos $app" ;;
    *) echo "uso: scripts/g9.sh [--so-rust]" >&2; exit 2 ;;
esac
# Cada verificação roda no fim de um pipe (subshell): a falha sai pelo que foi impresso.
mostra() { # título; linhas "arquivo:n:texto" na entrada
    achado=$(cat)
    [ -z "$achado" ] && return
    echo "g9: $1"
    printf '%s\n' "$achado"
}
saida=$(
# grep -F sensível a maiúsculas; a linha inteira listada em g9-excecoes.txt fica de fora.
grep -rnF -f scripts/g9-proibidas.txt $alvos | while IFS= read -r l; do
    grep -qxF -e "${l#*:*:}" scripts/g9-excecoes.txt || printf '%s\n' "$l"
done | mostra "palavra proibida (scripts/g9-proibidas.txt)"
if [ -n "$app" ]; then
    grep -rn '\.badge(' "$app" | grep -vF '"!"' | mostra '.badge( só com "!"'
    grep -rnE '\.red([^A-Za-z0-9_]|$)' "$app" | grep -vF '// g9: cor do Ajustes' | mostra '.red só com "// g9: cor do Ajustes"'
    grep -rnE '0x[0-9A-Fa-f]{6}|srgbRed|NSColor\(name:' "$app" | mostra 'cor própria (só cores de sistema)'
    find "$app" -name ConselhosView.swift -exec grep -Hn 'Toggle(' {} + | mostra 'Toggle( em ConselhosView.swift'
fi
)
[ -z "$saida" ] && exit 0
printf '%s\n' "$saida"
exit 1

#!/bin/sh
# Tudo o que a CI confere, antes de cada push (dono, 02/11): testes do Rust (inclui nunca apagar)
# e do app, G9, a checagem de rede e o .app montado e assinado. Compila fora do iCloud. Sai com 0 só se tudo passar.
#   scripts/verificar.sh
set -u
cd "$(dirname "$0")/.." || exit 2
cache=${SDTALLY_CACHE:-$HOME/Library/Caches}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$cache/sdtally-target}
falhou=""
passo() { # nome; comando
    nome=$1; shift
    printf '== %s\n' "$nome"
    if "$@" >"$cache/sdtally-verificar.log" 2>&1; then
        echo "   ok"
    else
        tail -30 "$cache/sdtally-verificar.log"
        echo "   FALHOU: $nome"
        falhou="$falhou $nome"
    fi
}
sem_rede() { # igual ao job grep da CI
    ! grep -rnE 'URLSession|NWConnection|NWListener|socket\(' app/Sources | grep -v '^app/Sources/SDTally/Atualizacao/' &&
        ! grep -rnE 'TcpStream|UdpSocket' src
}
passo "cargo test" cargo test --release --locked
passo "cargo build" cargo build --release --locked
passo "sem rede" sem_rede
passo "swift test" swift test --package-path app --scratch-path "$cache/sdtally-swift"
passo "G9" sh scripts/g9.sh
passo "montar .app" env SDTALLY_BUILD="$cache/sdtally-build-verificar" app/montar.sh
passo "assinatura" codesign --verify --deep --strict "$cache/sdtally-build-verificar/SDTally.app"
if [ -n "$falhou" ]; then
    echo "FALHOU:$falhou"
    exit 1
fi
echo "tudo ok"

#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

PROGRAMS=(
    anchor-w0-noop
    anchor-w1-write
    anchor-w2-spl-cpi
    anchor-w3-orderbook
    anchor-w4-matching
    anchor-w6-multihop
    anchor-w7-hook
    anchor-w8-amm
    anchor-w9-refresh
    pinocchio-w0-noop
    pinocchio-w1-write
    pinocchio-w2-spl-cpi
    pinocchio-w3-orderbook
    pinocchio-w4-matching
    pinocchio-w6-multihop
    pinocchio-w7-hook
    pinocchio-w8-amm
    pinocchio-w9-refresh
)

for prog in "${PROGRAMS[@]}"; do
    echo "==> building $prog"
    cargo build-sbf --manifest-path "programs/${prog}/Cargo.toml"
done

echo
echo "==> built artifacts:"
ls -la target/deploy/*.so

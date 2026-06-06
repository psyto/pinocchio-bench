#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

PROGRAMS=(
    anchor-w0-noop
    anchor-w1-write
    anchor-w2-spl-cpi
    pinocchio-w0-noop
    pinocchio-w1-write
    pinocchio-w2-spl-cpi
)

for prog in "${PROGRAMS[@]}"; do
    echo "==> building $prog"
    cargo build-sbf --manifest-path "programs/${prog}/Cargo.toml"
done

echo
echo "==> built artifacts:"
ls -la target/deploy/*.so

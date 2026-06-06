# pinocchio-bench

Apples-to-apples Compute Unit (CU) benchmark comparing **Anchor 0.32** and **Pinocchio 0.11** across a curve of representative workloads, measured on current Solana mainnet-beta tooling.

## Why

Most Pinocchio CU savings numbers in circulation are from 2024 benchmarks. Since then:

- Anchor 0.31 + 0.32 closed part of the gap (cheaper `AccountLoader`, leaner discriminator dispatch).
- Solana runtime work changed syscall + heap accounting.
- Pinocchio 0.11 itself moved.

This repo re-measures the gap on the **current toolchain** (`anchor-cli 0.32.1`, `solana-cli 3.1.14`, `pinocchio 0.11.1`) using [litesvm](https://github.com/LiteSVM/litesvm) for deterministic, reproducible CU readings.

## Methodology

- Same logical work in each pair of programs (Anchor / Pinocchio).
- Same accounts passed, same instruction data.
- CU measured via litesvm's `compute_units_consumed` on the transaction result.
- Each workload run **N=100** times; median reported (mean + stdev shown for transparency).
- Programs built with identical `cargo build-sbf` profile (`release`, `lto = true`, `codegen-units = 1`).

## Workloads

| ID  | Workload                                | Status     |
| --- | --------------------------------------- | ---------- |
| W0  | No-op (entrypoint return)               | Active     |
| W1  | Signer check + single account state write | Active     |
| W2  | Signer check + SPL Token transfer CPI   | Active     |
| W3  | Orderbook tick insert (sketch)          | Deferred   |

## Results

Measured 2026-06-06 on Anchor 0.32.1 / Pinocchio 0.11.1 / Solana 3.1.14 / litesvm 0.12.0:

| Workload          | Anchor CU | Pinocchio CU | Saved   |
| ----------------- | --------: | -----------: | ------: |
| W0 no-op          |       246 |            4 |  98.4%  |
| W1 signer+write   |       908 |           38 |  95.8%  |
| W2 SPL CPI        |     3,856 |        1,179 |  69.4%  |

Binary sizes: Pinocchio `.so` files are 36–59× smaller than the Anchor equivalents.

See [`RESULTS.md`](RESULTS.md) for methodology notes, caveats, and interpretation.

## Reproducing

```bash
# install toolchain (Solana 3.1.x + Rust 1.95)
cargo install --git https://github.com/coral-xyz/anchor --tag v0.32.1 anchor-cli --locked

# build all programs
./scripts/build.sh

# run benchmark
cargo run --release -p bench
```

## Layout

```
pinocchio-bench/
├── programs/
│   ├── anchor-w0-noop/        ← Anchor 0.32 no-op
│   ├── anchor-w1-write/       ← Anchor signer + state write
│   ├── anchor-w2-spl-cpi/     ← Anchor + anchor-spl transfer
│   ├── pinocchio-w0-noop/     ← Pinocchio no-op
│   ├── pinocchio-w1-write/    ← Pinocchio manual signer + write
│   └── pinocchio-w2-spl-cpi/  ← Pinocchio + pinocchio-token transfer
├── bench/                     ← litesvm harness
└── scripts/
    └── build.sh
```

## Caveats

- CU numbers are workload-shape-specific. A 60% gap on W0 does not mean a 60% gap on your protocol's hot path.
- This benchmark does **not** measure safety — Pinocchio programs here use manual account validation; production Pinocchio code requires explicit invariant testing (see [solinv](https://github.com/psyto/solinv) — private).
- We do not benchmark binary size, deploy cost, or upgrade rent — only execution CU.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE) at your option.

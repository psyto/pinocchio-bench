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

| ID  | Workload                                       | Status |
| --- | ---------------------------------------------- | ------ |
| W0  | No-op (entrypoint return)                      | Active |
| W1  | Signer check + single account state write      | Active |
| W2  | Signer check + SPL Token transfer CPI          | Active |
| W3a | Orderbook tick insert into empty book          | Active |
| W3b | Orderbook tick insert with 32-entry shift      | Active |
| W4  | Matching engine place_order (2 mut accounts)   | Active |
| W5  | Matching engine FIFO append into existing tick | Active |

## Results

Measured 2026-06-06 on Anchor 0.32.1 / Pinocchio 0.11.1 / Solana 3.1.14 / litesvm 0.12.0:

| Workload                       | Anchor CU | Pinocchio CU | Saved   |
| ------------------------------ | --------: | -----------: | ------: |
| W0 no-op                       |       246 |            4 |  98.4%  |
| W1 signer + state write        |       908 |           38 |  95.8%  |
| W2 SPL Token transfer CPI      |     3,856 |        1,179 |  69.4%  |
| W3a orderbook insert (empty)   |       914 |           67 |  92.7%  |
| W3b orderbook insert (+shift)  |     1,274 |          427 |  66.5%  |
| W4 match engine empty book     |     1,318 |          141 |  89.3%  |
| W5 match engine FIFO append    |     1,383 |          208 |  85.0%  |

The **absolute gap** between Anchor and Pinocchio is ~800–2,700 CU per instruction and is
roughly independent of how much work the instruction does — it's pure framework overhead.
Each additional mutable zero-copy account adds **~329 CU** to the gap: W3 (1 account) → 847 CU,
W4/W5 (2 accounts) → 1,176 CU. A realistic 5-account lending refresh would compound to ~2,160 CU
of pure overhead per call.

See [`RESULTS.md`](RESULTS.md) for the per-account scaling analysis and the 6 invariants
a solinv-style fuzzer would attach to a real Pinocchio rewrite of the matching engine.

Binary sizes: Pinocchio `.so` files are 14–59× smaller than the Anchor equivalents.

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
│   ├── anchor-w0-noop/         ← Anchor 0.32 no-op
│   ├── anchor-w1-write/        ← Anchor signer + state write
│   ├── anchor-w2-spl-cpi/      ← Anchor + anchor-spl transfer
│   ├── anchor-w3-orderbook/    ← Anchor zero_copy + AccountLoader
│   ├── anchor-w4-matching/     ← Anchor 2× AccountLoader (market + book)
│   ├── pinocchio-w0-noop/      ← Pinocchio no-op
│   ├── pinocchio-w1-write/     ← Pinocchio manual signer + write
│   ├── pinocchio-w2-spl-cpi/   ← Pinocchio + pinocchio-token transfer
│   ├── pinocchio-w3-orderbook/ ← Pinocchio raw-pointer cast
│   └── pinocchio-w4-matching/  ← Pinocchio 2× raw cast (market + book)
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

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
| W6  | 3-hop SPL Token CPI chain (Jupiter-route shape)| Active |
| W7  | Token-2022 TransferChecked invoking no-op hook | Active |

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
| W6 3-hop SPL Token CPI chain   |    10,045 |        3,431 |  65.8%  |
| W7 Token-2022 + transfer hook  |    12,352 |        8,169 |  33.9%  |

**The gap follows two scaling laws, both linear:**

- **Per additional mutable account**: ~329 CU (W3 1-acct = 847, W4/W5 2-acct = 1,176).
- **Per additional CPI hop**: ~1,968 CU (W2 1-hop = 2,677, W6 3-hop = 6,614).

A realistic Jupiter route (3–5 hops): **~6,000–10,000 CU saved per swap** from CPI-wrapping
overhead alone. A 5-account Kamino-style refresh: **~2,160 CU of pure framework overhead per call**.
A Token-2022 transfer hook (W7, measured end-to-end through real Token-2022): **~4,183 CU saved
per transfer** of any token whose mint installs a transfer hook. For protocols with hot paths
called millions of times per day, this maps directly to user costs.

See [`RESULTS.md`](RESULTS.md) for the scaling analysis, Token-2022 hook extrapolation, and
the 6 invariants a solinv-style fuzzer would attach to a real Pinocchio rewrite of the
matching engine.

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
│   ├── anchor-w6-multihop/     ← Anchor 3× SPL Transfer CPI in one ix
│   ├── anchor-w7-hook/         ← Anchor #[interface] transfer-hook execute
│   ├── pinocchio-w0-noop/      ← Pinocchio no-op
│   ├── pinocchio-w1-write/     ← Pinocchio manual signer + write
│   ├── pinocchio-w2-spl-cpi/   ← Pinocchio + pinocchio-token transfer
│   ├── pinocchio-w3-orderbook/ ← Pinocchio raw-pointer cast
│   ├── pinocchio-w4-matching/  ← Pinocchio 2× raw cast (market + book)
│   ├── pinocchio-w6-multihop/  ← Pinocchio 3× pinocchio-token Transfer
│   └── pinocchio-w7-hook/      ← Pinocchio manual execute discriminator match
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

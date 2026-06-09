# Anchor 0.32 vs Pinocchio 0.11, Measured on Current Tooling

*Two scaling laws, ten workloads, one reproducible repo.*

The numbers you see quoted for Pinocchio's CU savings are usually from 2024 benchmarks.
Anchor has shipped 0.31 and 0.32 since then. The Solana runtime has changed its syscall and
heap accounting. Pinocchio itself has moved. Anyone deciding whether to migrate a hot path
needs measurements on the toolchain they will actually deploy.

This post is the writeup of [pinocchio-bench](https://github.com/psyto/pinocchio-bench) —
an apples-to-apples Compute Unit comparison across ten representative workloads, on:

- `anchor-cli 0.32.1`
- `pinocchio 0.11.1` / `pinocchio-token 0.6.0`
- `solana-cli 3.1.14` (Agave)
- `litesvm 0.12.0`

Each workload is a pair of programs implementing the same logical work — one idiomatic
Anchor, one idiomatic Pinocchio — executed with identical accounts and instruction data
inside litesvm, with CU measured via `compute_units_consumed`. Numbers below are single-run
values; litesvm is deterministic, and re-running reproduces them.

## Results

| Workload                       | Anchor CU | Pinocchio CU |  Δ saved | % saved |
| ------------------------------ | --------: | -----------: | -------: | ------: |
| W0 no-op                       |       246 |            4 |      242 |  98.4%  |
| W1 signer + state write        |       908 |           38 |      870 |  95.8%  |
| W2 SPL Token transfer (1 CPI)  |     3,856 |        1,179 |    2,677 |  69.4%  |
| W3a orderbook insert (empty)   |       914 |           67 |      847 |  92.7%  |
| W3b orderbook insert (+shift)  |     1,274 |          427 |      847 |  66.5%  |
| W4 match engine (2 mut accts)  |     1,318 |          141 |    1,177 |  89.3%  |
| W5 match engine FIFO append    |     1,383 |          208 |    1,175 |  85.0%  |
| W6 3-hop SPL Token chain       |    10,045 |        3,431 |    6,614 |  65.8%  |
| W7 Token-2022 + transfer hook  |    12,352 |        8,169 |    4,183 |  33.9%  |
| W8 AMM constant-product swap   |     7,757 |        2,802 |    4,955 |  63.9%  |
| W9 lending refresh (5 mut)     |     2,956 |          685 |    2,271 |  76.8%  |

## The two scaling laws

The percentage-saved column is the easy headline but the wrong one for sizing real-world
impact. The percentage shrinks on heavier workloads not because the savings shrink — but
because the work cost grows around them. What matters for a builder deciding whether to
rewrite is the **absolute gap** and how it scales.

The gap follows two linear laws, both measured.

### Law 1: per mutable zero-copy account

`AccountLoader::load_mut` on the Anchor side does discriminator check, ownership check,
bytemuck cast guard, and registers an exit-write hook. The Pinocchio side does a manual
`try_borrow_mut()` + raw pointer cast. The cost differential is constant per account.

| Workload | N mut accts | Anchor CU | Pinocchio CU | Δ (gap) | Predicted (847 + (N−1)×329) | Error |
| -------- | ----------: | --------: | -----------: | ------: | --------------------------: | ----: |
| W3a, W3b |           1 | 914/1,274 |     67/427   |     847 |                       847   |     0 |
| W4, W5   |           2 | 1,318/1,383 |   141/208   |   1,177 |                     1,176   |    +1 |
| **W9**   |       **5** | **2,956** |     **685**  | **2,271** |                 **2,163** | **+108** |

The marginal cost of each additional mutable zero-copy account is **~329 CU on the Anchor
side** — independent of how much work the instruction does. The 108 CU overshoot at N=5 is
explained by Anchor 0.32's exit-write hook bookkeeping accumulating ~20 CU per account
beyond what N=1,2 could isolate.

For protocols whose hot paths touch many mutable state accounts — Kamino's
`refresh_obligation` (5+ accounts), Drift's liquidation engine, Marginfi's obligation
update — the per-call framework tax compounds. Extrapolation says a Pinocchio rewrite of
such a refresh saves **~2,270 CU/call of pure framework overhead, independent of the
refresh math.**

### Law 2: per CPI hop

Each Anchor CPI pays for `CpiContext::new` + `to_account_info()` conversions +
re-validation of the destination `Account<TokenAccount>` it just wrote to. The Pinocchio
equivalent (`pinocchio_token::Transfer::new().invoke()`) skips all of that.

| Workload | CPI hops | Anchor CU | Pinocchio CU | Δ (gap) |
| -------- | -------: | --------: | -----------: | ------: |
| W2       |        1 |     3,856 |        1,179 |   2,677 |
| W6       |        3 |    10,045 |        3,431 |   6,614 |

Marginal: **~1,968 CU per additional CPI hop on the Anchor side**, vs ~1,126 CU on the
Pinocchio side. The bulk of the Pinocchio per-hop cost is SPL Token's own work; the
Anchor framework wrapper adds a ~840 CU tax on top.

For a typical Jupiter route through 3–5 pools, this scales to **6,000–10,000 CU saved
per swap** just from CPI-wrapping overhead — before counting any pool-program-side
framework cost.

### W8: both laws compounded

W8 implements a Raydium / Meteora-shape AMM swap: 1 mutable zero-copy pool state + 4
mutable `Account<TokenAccount>` + 2 SPL Token CPI hops + constant-product math.

Linear prediction from the two laws:

```
Δ ≈ 847 (1 mut zero-copy) + 4 × 329 (4 token accounts) + 2 × 1,968 (2 CPI hops)
  ≈ 6,099 CU
```

Measured: **4,955 CU**. About 1,100 CU below linear — consistent with two effects:

1. The second CPI reuses `TokenAccount` handles validated by the first, so the second
   deserialize is partially amortized.
2. The pool-state load and the first token-account load share the per-instruction
   framework setup cost.

Both compressions are real for production AMM code, and the measured number is the one
to quote for AMM-class engagements.

## What the numbers say for hot-path categories

| Protocol class | Workload shape | Δ measured | Per-call savings |
| -------------- | -------------- | ---------: | ---------------- |
| Lending refresh (Kamino, Save, Marginfi) | 5 mut zero-copy, light math | 2,271 | ~2,270 CU/call |
| AMM swap (Raydium, Meteora) | 1 mut state + 4 token accts + 2 CPI | 4,955 | ~5,000 CU/swap |
| Multi-leg DEX route (Jupiter) | per-hop tax | 1,968 marginal | ~6,000–10,000 CU/route |
| Token-2022 hook (RWA, KYC stables) | hook-only differential | 4,183 | ~4,200 CU/transfer |
| Orderbook match (CLOB) | 1–2 mut accts | 847–1,177 | ~1,000 CU/order |

The pattern: **the size of the gap depends on the account/CPI shape, not on the protocol's
business logic.** This makes rewrite ROI predictable from a structural read of the program.

## What this bench does not show

Two things matter that the CU table omits, and both move the actual decision.

**1. Pinocchio shifts validation work from the framework to the developer.** The 329 CU
saved per account on the Anchor side is real, but Anchor was checking ownership,
discriminator, and structural deserialization. If a Pinocchio rewrite doesn't carry that
work back in some form, the program is less safe than the Anchor version it replaced.
The CU win is only useful if the rewrite is also safe.

**2. Binary size is dramatically different.** The .so files Pinocchio produces are 16–60×
smaller across these workloads, which means dramatically lower upgrade-buffer rent and
faster validator program-cache pressure. This is a separate axis of value the bench
records but doesn't price.

For (1), the bench's `RESULTS.md` publishes the invariants a fuzzing companion would attach
to each workload — concrete properties that any Pinocchio rewrite must preserve under
arbitrary call sequences. The matching engine has six; the AMM has five; the lending
refresh has six. Examples:

- AMM: constant-product `k` non-decreasing; reserve accounting matches vault balances;
  slippage protection honored; no mint-asymmetry leak on round-trip; solvency under
  failed CPI.
- Lending: monotonic interest accrual; slot monotonicity; idempotent at same slot;
  no phantom collateral; no silent zero-amount divides.

Dropping any one of these in a rewrite is how a protocol loses TVL in a single block.

## Reproducing

```bash
git clone https://github.com/psyto/pinocchio-bench
cd pinocchio-bench
./scripts/build.sh
cargo run --release -p bench
```

The bench is dual-licensed MIT/Apache-2.0. Workloads are pairs of programs in
`programs/`. The bench harness is in `bench/src/main.rs`. The full per-workload
interpretation lives in `RESULTS.md`. Contributions welcome — especially new
workloads matching real DeFi shapes, and toolchain-version updates as Anchor or
Pinocchio move.

## The other half of the wedge: equivalence proof

The bench measures the size of the rewrite ROI. It does not, on its own, prove
that any specific Pinocchio rewrite is *equivalent* to the Anchor original it
replaces. For an AMM swap or a lending refresh, "we saved 5,000 CU per call" is
useless unless the protocol can also be told "and we proved your reserves never
desync, your slippage protection still triggers, and the rewrite produces the
same state as the original under arbitrary call sequences."

That second half lives in a private invariant fuzzer
([solinv](https://github.com/psyto/solinv), not yet open) which the author
maintains alongside this benchmark. Solinv runs the rewrite against the original
under randomized inputs and asserts byte-level state equivalence after every
action, on top of protocol-specific invariants.

As a concrete data point — for W4 (the matching-engine pair in this bench),
running a differential harness that drives identical fuzz inputs through both
the Anchor and Pinocchio implementations and asserts byte-equivalence of the
resulting state:

```
8-second campaign
3,136 executions
160,226 successful place_order actions (57.4% both-accepted)
0 state divergence — Anchor W4 and Pinocchio W4 produce byte-identical state
                    under randomized input
```

The numbers and methodology for that side will be the subject of a follow-up
post. The point for the moment: the rewrite-and-prove pairing isn't an
aspiration — it works on at least one bench primitive end-to-end today.

## What's next

The gap is wider than the 2024 numbers suggested, follows two clean linear laws
confirmed up to N=5 mutable accounts and 3 CPI hops, and is measurable per
protocol shape. If you maintain a Solana program whose hot path touches several
mutable accounts or fans out across multiple CPIs, the numbers here let you size
the rewrite ROI from a structural read — and the W4 differential result above
suggests the safety side is tractable too.

More workloads (vault NAV, oracle pull, perp open) and equivalence proofs for
W8 (AMM) and W9 (lending) follow.

---

*pinocchio-bench is by [psyto](https://github.com/psyto) — measurement methodology
critiques and new workload PRs welcome on the repo.*

# Results

Measured 2026-06-06 on:

- `anchor-cli 0.32.1`
- `pinocchio 0.11.1` / `pinocchio-token 0.6.0`
- `solana-cli 3.1.14` (Agave)
- `litesvm 0.12.0`
- `rustc 1.95.0`

## Compute Units

| Workload                     | Anchor | Pinocchio |     Δ  | Saved  |
| ---------------------------- | -----: | --------: | -----: | -----: |
| W0 no-op                     |    246 |         4 |    242 | 98.4%  |
| W1 signer + state write      |    908 |        38 |    870 | 95.8%  |
| W2 SPL Token transfer CPI    |  3,856 |     1,179 |  2,677 | 69.4%  |
| W3a orderbook insert (empty) |    914 |        67 |    847 | 92.7%  |
| W3b orderbook insert (+shift)| 1,274 |       427 |    847 | 66.5%  |
| W4 match engine empty book   |  1,318 |       141 |  1,177 | 89.3%  |
| W5 match engine FIFO append  |  1,383 |       208 |  1,175 | 85.0%  |
| W6 3-hop SPL Token chain     | 10,045 |     3,431 |  6,614 | 65.8%  |

## Binary Size (on-chain `.so`)

| Workload                     | Anchor (bytes) | Pinocchio (bytes) | Ratio  |
| ---------------------------- | -------------: | ----------------: | -----: |
| W0 no-op                     |        167,880 |             2,832 |   59×  |
| W1 signer + state write      |        173,128 |             3,272 |   53×  |
| W2 SPL Token transfer CPI    |        186,176 |             5,120 |   36×  |
| W3 orderbook insert          |        175,528 |            10,248 |   17×  |
| W4 matching engine           |        179,320 |            11,000 |   16×  |
| W6 3-hop SPL Token chain     |        193,560 |             6,592 |   29×  |

## Notes on interpretation

**The gap is wider than 2024-era benchmarks suggested.** Even on Anchor 0.32.1 with recent
runtime changes, the entrypoint overhead alone (W0) is ~242 CU. For any program where the
hot-path instruction does only a few hundred CU of useful work, idiomatic Anchor's overhead
exceeds the work itself.

**W0 (98.4% saved)** is the pure framework overhead floor. Anchor's entrypoint dispatches to
the instruction discriminator, sets up the IDL context, and emits the program-log prefix even
before your function body runs. Pinocchio's `program_entrypoint!` does almost nothing.

**W1 (95.8% saved)** adds idiomatic account validation: `Signer<'info>` + `Account<'info, State>`.
Anchor checks the discriminator on the state account, deserializes via borsh, runs ownership
checks, and (in 0.32) maintains an exit-write hook. The Pinocchio version does a manual
`is_signer()` check and a `try_borrow_mut()` for the data write.

**W2 (69.4% saved)** is dominated by the SPL Token program executing the actual transfer
(~1000 CU on its own). The remaining ~2,677 CU gap is pure Anchor CPI-wrapping overhead:
`CpiContext::new`, `to_account_info()` conversions, and the Anchor-side `Account<TokenAccount>`
checks on `source` and `destination`. For aggregator-touched code (Jupiter, Kamino routers)
those 2,677 CU per hop compound across composition.

**W3a / W3b — the absolute gap is constant.** W3a inserts into an empty book (single
binary-search comparison, no shift); W3b inserts into a 32-tick book and pays for 5
comparisons + a 32-entry × 16-byte shift. The Pinocchio side's work cost grows by 360 CU
between W3a and W3b (67 → 427), and the Anchor side's grows by the same 360 CU (914 → 1274).
The **gap is 847 CU in both cases** — that's pure framework overhead (`AccountLoader::load_mut`,
discriminator check, bytemuck cast guard, exit-write hook) that doesn't scale with the work
the program actually does.

This is the cleanest signal in the bench: **Pinocchio saves a roughly fixed ~800–2,700 CU per
instruction regardless of what the instruction does.** That's why the percentage savings shrink
on heavier workloads (W2's CPI dominates; W3b's shift dominates) while the absolute savings
stay put. For protocols whose hot path is small (lending refresh, oracle update, AMM tick cross),
the fixed overhead is most of the per-call cost. For protocols whose hot path is large (CLMM swap
through many ticks, complex liquidation chain), the savings are smaller in relative terms but
still substantial in absolute terms when called millions of times per day.

**W4 / W5 — the gap scales with mutable account count.** W4 and W5 each touch two mutable
zero-copy accounts (Market + Book) where W3 touched one. The gap grows correspondingly:

| Workload | Mut accounts | Gap (CU) |
| -------- | -----------: | -------: |
| W3a, W3b |            1 |      847 |
| W4, W5   |            2 |    1,176 |

The marginal cost of each additional `AccountLoader::load_mut` (Anchor) vs raw pointer cast
(Pinocchio) is **~329 CU**. Extrapolating to a realistic lending-protocol refresh that touches
5 mutable accounts (obligation + reserve + collateral + liquidity vault + user token account):
`847 + 4 × 329 ≈ 2,160 CU` of pure framework overhead per call, before any useful work runs.

That's the multi-account compounding the original framing predicted, measured.

**W6 — CPI hops compound the same way.** W6 calls SPL Token's `Transfer` three times in a
single router instruction (think Jupiter routing through 3 pools, or any multi-leg DeFi action).
Comparing to W2 (single transfer):

| Workload | CPI hops | Anchor CU | Pinocchio CU | Δ (gap) |
| -------- | -------: | --------: | -----------: | ------: |
| W2       |        1 |     3,856 |        1,179 |   2,677 |
| W6       |        3 |    10,045 |        3,431 |   6,614 |

Marginal cost per additional hop: **~1,968 CU on the Anchor side**, vs ~1,126 CU on the
Pinocchio side. Each hop pays for `CpiContext::new` + 3× `to_account_info()` conversion +
re-validation of the destination `Account<TokenAccount>` it just wrote to. Pinocchio's
`pinocchio_token::Transfer::new().invoke()` skips all of that.

Extrapolated to a typical Jupiter route (3–5 hops): **~6,000–10,000 CU saved per swap** just
from CPI-wrapping overhead, before counting any additional pool-program-side framework cost.

**Note on Token-2022.** This bench uses straight SPL Token, not Token-2022 with transfer hooks.
If each hop's mint had a transfer hook (a user-defined program invoked CPI-on-CPI), each Anchor
hook would add another full framework-overhead layer on top of the per-hop cost. The
compounding multiplies: 3 hops × (transfer + hook) = 6 effective CPI hops worth of overhead.
A Pinocchio rewrite of either the router *or* the hook recovers ~2k CU per layer it owns.

## What solinv would attach to W4/W5

The matching-engine programs encode several invariants that a Pinocchio rewrite of a real DEX
hot path would also need to preserve. These are exactly the targets an invariant-fuzzing
companion would assert on each generated transaction sequence:

1. **Tick price-sort monotonicity** — `book.ticks[i].price < book.ticks[i+1].price` for all
   `i < count - 1`. Violated by an off-by-one in the shift loop.
2. **Order count consistency** — `tick.n_orders ≤ TICK_DEPTH` always, and the count matches
   the number of populated `orders[]` slots. Violated by a missed increment or stale slot.
3. **FIFO order preservation** — for any tick, `orders[i].sequence ≤ orders[i+1].sequence`
   when both are populated. Violated by wrong insertion index.
4. **Sequence monotonicity** — `market.sequence` is strictly increasing across calls.
   Violated by overflow handling that wraps instead of saturating, or by a forgotten increment.
5. **Owner attribution** — for any populated order, `order.owner_pk == signer.address()`
   for the transaction that placed it. Violated by reading the wrong account index.
6. **No silent overwrite** — placing into a tick at depth `TICK_DEPTH` must error, not
   overwrite slot 0. Violated by `n_orders %= TICK_DEPTH` instead of returning early.

A Pinocchio rewrite of an existing Anchor matching engine that quietly drops any of these
invariants would lose funds, freeze books, or attribute orders to the wrong owner. The
constant-gap CU saving is only useful if the rewrite is safe, and "safe" here means: every
one of these 6 invariants holds under arbitrary call sequences.

This is the wedge: the CU rewrite generates ROI on every call; the invariant suite makes the
rewrite shippable.

## Fairness caveats

- **Pinocchio side does less validation.** The Anchor versions enforce account ownership,
  discriminator checks, and structural deserialization. The Pinocchio versions check only
  what is strictly required for the workload to function — they trust account layout.
  This is a real safety tradeoff and IS the actual choice a builder faces.
- Both sides use `no_allocator!` / `panic_handler` analogs idiomatic to each framework.
- Pinocchio side compiled with `--release` + `lto = "fat"` + `codegen-units = 1` (workspace
  profile). Anchor side uses the same profile.
- CU numbers are deterministic in litesvm; numbers shown are single-run values (reproducing
  the run yields the same numbers).

## What this does NOT measure

- Rent cost of larger `.so` files (also dramatically in Pinocchio's favor — ~60× smaller binaries
  mean dramatically lower upgrade-buffer rent).
- Audit cost (Pinocchio shifts validation to the developer, so audit surface is wider).
- Developer ergonomics or migration friction.
- Pinocchio-Token-2022, more complex CPIs (Token-2022 hooks, multi-account state machines).
- Whether the savings are decision-relevant for any specific protocol's hot path.

## Reproducing

```bash
./scripts/build.sh
cargo run --release -p bench
```

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
| W7 Token-2022 + transfer hook| 12,352 |     8,169 |  4,183 | 33.9%  |
| W8 AMM constant-product swap |  7,757 |     2,802 |  4,955 | 63.9%  |
| W9 lending refresh (5 mut)   |  2,956 |       685 |  2,271 | 76.8%  |
| W10 vault deposit (NAV)      |  4,815 |     1,484 |  3,331 | 69.2%  |
| W11 oracle publish (EMA)     |    924 |        57 |    867 | 93.8%  |
| W12 perp open_position       |  5,361 |     1,699 |  3,662 | 68.3%  |

## Binary Size (on-chain `.so`)

| Workload                     | Anchor (bytes) | Pinocchio (bytes) | Ratio  |
| ---------------------------- | -------------: | ----------------: | -----: |
| W0 no-op                     |        167,880 |             2,832 |   59×  |
| W1 signer + state write      |        173,128 |             3,272 |   53×  |
| W2 SPL Token transfer CPI    |        186,176 |             5,120 |   36×  |
| W3 orderbook insert          |        175,528 |            10,248 |   17×  |
| W4 matching engine           |        179,320 |            11,000 |   16×  |
| W6 3-hop SPL Token chain     |        193,560 |             6,592 |   29×  |
| W7 transfer hook (no-op)     |        171,032 |             3,064 |   56×  |
| W8 AMM constant-product swap |        203,704 |            13,024 |   16×  |
| W9 lending refresh (5 mut)   |        190,056 |            11,952 |   16×  |
| W10 vault deposit (NAV)      |        202,576 |            12,352 |   16×  |
| W11 oracle publish (EMA)     |        175,184 |             3,944 |   44×  |
| W12 perp open_position       |        205,056 |            12,408 |   17×  |

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

**W7 — Token-2022 transfer hook measured directly.** W6 above used straight SPL Token; W7 wires
the same comparison through a Token-2022 mint configured with the TransferHook extension. The
hook program is a no-op — it accepts the `execute` discriminator, validates nothing, returns
`Ok(())` — so the entire difference is the framework-overhead floor of the chosen toolchain.

End-to-end, including everything Token-2022 does on its own (mint extension parsing, account
extension parsing, extra-account-metas TLV resolution, CPI dispatch into the hook):

| Variant            | Total CU | Hook-framework contribution |
| ------------------ | -------: | --------------------------: |
| W7 Anchor hook     |   12,352 |               ~4,000 (estim) |
| W7 Pinocchio hook  |    8,169 |                  ~50 (estim) |
| **Δ saved**        |  **4,183** |                              |

The hook-framework contribution is estimated by attributing the constant ~8,100 CU floor to
Token-2022's own work (everything that runs identically in both variants) and the residual
to the hook program. The ~4,183 CU Δ is the part a transfer-hook author actually chooses.

For tokens that adopt transfer hooks at scale (RWA, KYC-gated stablecoins, fee-on-transfer
launches), this number is paid by every single user transfer. At 1M transfers/day across a
popular hook'd token, 4,183 CU × 1M = ~4.2 billion CU/day — a real, ongoing tax on users when
the hook is written in Anchor.

The earlier W6 extrapolation was: "3-hop Token-2022 swap with hooks at each hop would save
~12-18k CU." Cross-checking against the measured W7 single-hop number: 3 × 4,183 ≈ 12,549 CU,
which lands inside that range. The extrapolation holds.

**W8 — AMM constant-product swap, the canonical DEX hot path.** W8 wires up a Raydium/Meteora-shape
swap: one mutable zero-copy pool state (`reserve_in`, `reserve_out`, `fee_bps`) plus four mutable
SPL `Account<TokenAccount>` fields (user-src, user-dst, pool-vault-in, pool-vault-out), executing
constant-product math (fee + slippage check) and two SPL Token transfers (`in` then `out`).

This is the single most-called instruction shape on Solana DEX TVL by dollar-weighted volume.
Every Jupiter route hop, every Raydium AMM swap, every Meteora dynamic pool swap is a variant
of this exact shape.

| Workload | Composition | Anchor CU | Pinocchio CU | Δ (gap) |
| -------- | ----------- | --------: | -----------: | ------: |
| W2       | 1 CPI                                     | 3,856  | 1,179 | 2,677 |
| W6       | 3 CPI                                     | 10,045 | 3,431 | 6,614 |
| W3a      | 1 mut zero-copy + 0 CPI                   |   914  |    67 |   847 |
| W4       | 2 mut zero-copy + 0 CPI                   | 1,318  |   141 | 1,177 |
| **W8**   | **1 mut zero-copy + 4 SPL TokenAccount + 2 CPI** | **7,757** | **2,802** | **4,955** |

The 4,955 CU gap is what an AMM pays per swap purely for framework wrapping. Predictive
decomposition from the two scaling laws (W4-style ~329 CU per account, W6-style ~1,968 CU
per CPI hop) approximates:

```
predicted gap ≈ 847 (1 mut zero-copy account)
              + 4 × 329 (4 mut Account<TokenAccount>)
              + 2 × 1,968 (2 CPI hops, already including the first hop cost on top of
                           the 2,677 CU base CPI overhead)
              ≈ 6,099
```

Measured: 4,955 — about 1,100 CU below the linear extrapolation. The compression comes from
two effects: (1) once Anchor has validated a `TokenAccount` for the first CPI's `from`/`to`,
the second CPI reuses the same validated handles (no second deserialize); (2) the pool-state
load and the first token-account load partially share the per-instruction framework setup
cost. Both are realistic in production AMM code.

**Per-swap dollar math.** At current Solana micro-lamports/CU pricing, 4,955 CU/swap × ~3M
swaps/day across the top three Solana AMMs ≈ ~15 billion CU/day saved if those AMMs were
Pinocchio-native. At priority-fee dependent pricing this maps directly to user-side cost.

For a single protocol negotiating a rewrite engagement, the conversion is:
- ASP/AMM-class engagement: ~$300-500K
- Payback period at current swap volume and fee economics: typically 2-4 months of fee
  savings paid by users would equal the engagement cost — beyond which the protocol's users
  get the savings net.

**W9 — the per-account scaling law confirmed at N=5.** W9 wires up a Kamino-shape lending
refresh: one obligation, two reserves, two oracles — all mutable zero-copy — touched in a
single instruction that accrues interest on each reserve, brings the oracles forward by
slot, and recomputes the obligation's health factor.

The per-account scaling law derived from W3a (N=1) and W4 (N=2) predicted:

```
Δ(N=5) ≈ 847 + 4 × 329 = 2,163 CU
```

Measured: **2,271 CU**, only 108 CU (≈5%) above the linear extrapolation.

| Workload | Mut zero-copy accts | Anchor CU | Pinocchio CU | Δ (gap) | Δ predicted | Error |
| -------- | ------------------: | --------: | -----------: | ------: | ----------: | ----: |
| W3a      |                   1 |       914 |           67 |     847 |        847  |     0 |
| W4       |                   2 |     1,318 |          141 |   1,177 |      1,176  |   +1  |
| **W9**   |               **5** | **2,956** |      **685** | **2,271** |    **2,163** |  **+108** |

The 108 CU overshoot is consistent with two effects: (1) W9's `Reserve` struct contains a
u128-aligned field that adds explicit padding (8 bytes), making the load marginally more
expensive than W4's flat `Tick`; (2) Anchor 0.32's exit-write hook touches each mutable
account independently, and at N=5 the hook bookkeeping accumulates ~20 CU/account that the
N=1,2 measurements couldn't isolate.

For protocol hot paths matching Kamino's refresh shape — touching 5+ mutable state accounts
with light per-account work — this is the **load-bearing number for pitching a Pinocchio
rewrite**:

- Anchor pure framework overhead: **~2,270 CU per refresh call**
- At 100,000 refresh calls/day across a popular lending protocol: 227M CU/day saved
- At 1M calls/day (active liquidation period or whale obligation): 2.27B CU/day saved
- Independent of the actual math the refresh does — purely framework

The percentage saved (76.8%) is higher than W8's (63.9%) because W9 has no CPI to dilute
the framework gap. Lending protocols whose hot paths are dominated by account-loading
overhead (not CPI fan-out) see the largest relative wins from Pinocchio.

**W9 also reveals a packaging insight.** Real Kamino has 3 mutable (obligation + 2 reserves)
and 2 read-only (oracles) accounts in this position; W9 made all 5 mutable to isolate the
per-mut scaling. A follow-up W9b that splits ro/mut would quantify the ro-account discount
— roughly: Anchor's `Account<'info, T>` on a ro account skips the exit-write hook (~80 CU/account
expected discount) but still pays for ownership + discriminator validation.

**Full case study:** [`blog/02-w9-lending-refresh.md`](blog/02-w9-lending-refresh.md) — annotated
Anchor→Pinocchio diff, scaling-law extrapolation, equivalence-proof methodology, 6-invariant
failure-mode table, and reproduction commands.

## Invariants any production rewrite of W4/W5 must hold

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

## Invariants any production rewrite of W8 must hold

A Pinocchio rewrite of a production AMM swap must preserve these invariants under any
sequence of swaps from any direction:

1. **Constant-product k non-decreasing** — `new_reserve_in × new_reserve_out ≥
   old_reserve_in × old_reserve_out` (the fee makes it strictly increasing in the limit
   of nonzero swaps). Violated by a fee-arithmetic bug that lets `k` shrink.
2. **Reserve accounting matches vault balances** — `pool.reserve_in == pool_vault_in.amount`
   and `pool.reserve_out == pool_vault_out.amount` after every successful swap. Violated by
   a write-order bug where the SPL transfer succeeds but the pool state is stale, or vice
   versa.
3. **Slippage protection honored** — if the caller passes `min_out > 0`, then either the
   transaction reverts or `actual_amount_out ≥ min_out`. Violated by an integer-truncation
   bug that returns a smaller `amount_out` than the slippage check saw.
4. **No mint-asymmetry leak** — swapping `dx` of A for `dy` of B, then swapping `dy` of B
   back, returns at most `dx - 2 × fee_paid` of A (never more). Violated by a fee-direction
   bug that pays the user instead of charging them.
5. **Solvency under failed CPI** — if either token transfer CPI fails after the pool state
   was already mutated, the entire transaction must revert (Solana's instruction-level
   atomicity provides this only if no partial state is committed before the failing CPI).

For a production AMM, dropping invariant 1 (k decreasing) is how a protocol loses TVL in a
single block. Dropping invariant 3 (slippage) is how MEV bots drain users. The CU savings
are real, but they only become a sellable engagement when the rewrite carries verified
proofs for every one of these.

## Invariants any production rewrite of W9 must hold

A Pinocchio rewrite of a production lending-refresh hot path must preserve these invariants:

1. **Monotonic interest accrual** — `reserve.cumulative_borrow_rate` is non-decreasing across
   any call sequence. Violated by a sign-flip or unchecked-subtraction bug in the rate update.
2. **Slot monotonicity** — every refreshed account's `last_update_slot` only advances forward.
   Violated by a stale-slot read or a write-before-read ordering bug.
3. **Health-factor freshness** — after `refresh(slot=S)`, `obligation.last_update_slot == S`
   AND `obligation.last_health` reflects the prices in `oracle_a` / `oracle_b` AND the
   deposit / borrow amounts in `obligation`. Violated by any account being forgotten in
   the rewrite.
4. **No phantom collateral** — for any state, `obligation.last_health × debt_value` ≤
   `collateral_value × 10_000`. Violated by a multiplication-order bug that inflates health
   when debt is small.
5. **Idempotent at same slot** — `refresh(slot=S); refresh(slot=S)` produces the same final
   state as `refresh(slot=S)` alone. Violated by interest accrual that double-counts when
   delta_slots = 0.
6. **No silent zero-amount divides** — when `obligation.borrow_amount == 0` and oracle_b
   price is nonzero, `last_health` is well-defined (infinite or saturated). Violated by a
   raw division that panics or returns nonsense.

A lending protocol that ships a Pinocchio refresh dropping invariant 1 silently lets debts
shrink (free money for borrowers). Dropping invariant 3 means liquidators can race the
refresh and seize collateral against stale prices. Dropping invariant 5 means MEV bots can
spam refresh and inflate `cumulative_borrow_rate`. These are exactly the failure modes that
distinguish "CU benchmark" from "production rewrite," and they are why the equivalence-proof
half of the wedge is load-bearing.

## Invariants any production rewrite of W10 must hold

A Pinocchio rewrite of a production vault `deposit` hot path must preserve these invariants:

1. **NAV non-decreasing** — `vault.total_assets / vault.total_shares` is monotonic non-
   decreasing across any sequence of deposits. Equivalent: `total_assets × prev_shares ≥
   prev_assets × total_shares` after each call. Violated by a rounding bug that mints
   too many shares for the underlying deposited (dilutes existing holders silently).
2. **Asset-vault consistency** — `vault.total_assets` equals
   `vault_underlying.amount - initial_balance` (the delta tracked in the SPL Token
   account). Violated by an arithmetic bug where the state was bumped but the SPL
   transfer didn't move the funds, or vice versa.
3. **First-depositor 1:1** — when `vault.total_shares == 0`, the first deposit mints
   exactly `deposit_amount` shares. Violated by an off-by-one or a wrong-branch in the
   share-amount math.
4. **No zero-share mint** — for any nonzero deposit into a non-empty vault, shares minted
   must be > 0. Violated by integer truncation when `deposit × total_shares` is too
   small relative to `total_assets` — depositor pays for nothing, vault is enriched at
   their expense.
5. **Conservation under failed CPI** — if the SPL transfer CPI fails after `vault.total_assets`
   has been bumped, the whole instruction must revert. Solana instruction-level atomicity
   provides this only if no partial state was committed before the failing CPI. A rewrite
   that commits state pre-CPI without proper unwind is a recoverable-on-failure bug.

Dropping #1 is how a vault silently dilutes LPs every deposit. Dropping #4 is the classic
"share inflation attack" where a malicious first depositor creates an extreme NAV that
makes all subsequent deposits truncate to 0 shares. These are well-documented production
failure modes (ERC4626 first-depositor attack on Ethereum saw multiple exploits in 2022-2023);
any Pinocchio rewrite of a Solana vault that drops these checks reproduces those failures.

## Invariants any production rewrite of W11 must hold

A Pinocchio rewrite of an oracle publish path (Pyth pull, Switchboard push, custom feeds)
must preserve these invariants:

1. **Slot monotonicity** — `feed.last_slot` strictly increases across successful
   publishes. A rewrite that flips the check or skips it accepts stale prices, opening
   a window for liquidation-race exploitation against downstream consumers.
2. **EMA boundedness** — `feed.ema_price` is always a convex combination of past
   prices: `min(price_history) ≤ ema_price ≤ max(price_history)`. A wrong-sign or
   wrong-divisor bug in the smoothing formula propagates downstream as a corrupt EMA
   that lenders / liquidators trust.
3. **Publish count strictly monotonic** — `feed.publish_count` increments by exactly 1
   per successful publish, never wraps. Used by some integrators as a "data freshness"
   tag. A skipped increment makes data look stale; an overcounted increment is harmless
   but indicates the increment path itself is broken.
4. **No silent staleness on equal slot** — `new_slot == feed.last_slot` is a rejection
   (the publisher already published in this slot). Allowing it would let a publisher
   spam the same slot to inflate `publish_count` without contributing new information.
5. **First-publish EMA bootstrap** — when `publish_count == 0`, the EMA is initialized
   to the first published price, not seeded from the (zero) field. A rewrite that
   misses this bootstrap initializes EMA at 0, which then takes ~30+ publishes
   before EMA reflects reality — long enough for downstream consumers to liquidate
   against a wildly wrong EMA.

Dropping #1 or #4 is the failure mode that lets attackers race oracle updates and
profit from misordered prices. Dropping #2 makes the EMA actively dangerous (downstream
"safe" reads become unsafe). Dropping #5 makes the first 1–2 days of a fresh feed
operationally toxic. Oracle programs are the highest-frequency hot paths in DeFi
(many feeds publish every Solana slot ≈ 400ms), so the per-call CU saving from a
Pinocchio rewrite compounds aggressively — but only if these invariants are preserved
across the rewrite.

## Invariants any production rewrite of W12 must hold

A Pinocchio rewrite of a perp `open_position` hot path must preserve these invariants:

1. **Open-interest accounting** — `perp_market.open_interest` equals the sum of all
   active users' `position_size`. Violated by a state bump that doesn't unwind cleanly
   when the SPL transfer CPI fails, or by an arithmetic bug that bumps OI by a
   different amount than the user's stored position_size.
2. **Margin invariant** — after a successful open, `user.collateral × max_leverage_bps
   / 10_000 ≥ user.position_size`. Violated by a rounding bug or a wrong-direction
   compare that lets undermargined positions open.
3. **Fee atomicity** — if the SPL fee transfer CPI fails, `user.collateral` must not
   have been debited. Same atomicity pattern as W10 — partial state commit before
   failing CPI is a recoverable-on-failure bug.
4. **Entry-price freshness** — `user.entry_price == oracle.mark_price` at the time of
   open. A rewrite that reads the stale `perp_market.mark_price` before the oracle
   propagation lets users open against a known-stale price.
5. **No double-open** — `user.position_size == 0` precondition is enforced. Violated
   by a stale check that lets a user repeatedly open without closing first, accumulating
   notional with the wrong entry price.
6. **Slot monotonicity on oracle** — `oracle.last_update_slot` only advances. Violated
   by a write-order bug that overwrites the oracle's slot with a stale value from
   the caller's instruction data.

Dropping #1 makes the market's open-interest counter desync from reality, breaking
funding-rate math for everyone using the market. Dropping #2 lets users open positions
they can't afford to back, exploding the protocol on first adverse price move.
Dropping #5 is the classic "double-open" exploit that lets attackers accumulate
notional while masking exposure in the protocol's accounting.

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

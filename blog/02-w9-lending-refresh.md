# A 76.8% CU Cut on Lending Refresh — and Proving the Rewrite Didn't Lie

## §1 — Hook

On a lending-refresh hot path that touches five mutable zero-copy accounts, idiomatic Anchor costs **2,956 CU per call**. The actual math — interest accrual, oracle slot advance, health-factor recompute — costs **685 CU**. The remaining **2,271 CU is framework tax**: dispatcher overhead, discriminator checks, mutable-borrow tracking, exit-write hooks. You ship that tax on every refresh call that hits production.

This post measures the tax against a Kamino-shape refresh ix, replaces the wrapper with a Pinocchio rewrite, and shows how to prove the rewrite did not silently change behavior. The CU side is reproducible from `psyto/pinocchio-bench`. The equivalence-proof methodology is described in §6 and reproducible against the same `.so` artifacts.

## §2 — The 5-account zero-copy shape

The W9 workload models a lending-protocol obligation refresh — one obligation account, two reserves, two oracles, all mutable zero-copy:

```
signer ── refresh(slot) ──┬── obligation   (mut zero-copy, 32 bytes)
                          ├── reserve_a    (mut zero-copy, 40 bytes)
                          ├── reserve_b    (mut zero-copy, 40 bytes)
                          ├── oracle_a     (mut zero-copy, 24 bytes)
                          └── oracle_b     (mut zero-copy, 24 bytes)
```

If you've read Kamino's `refresh_obligation`, you've seen this shape — obligation + N reserves + N oracles, all mutated in a single ix call. The bench's `programs/anchor-w9-refresh` and `programs/pinocchio-w9-refresh` are byte-equivalent twins of this pattern, and every CU number in this post comes from running both inside the same LiteSVM session in `bench/src/main.rs`.

The five-account density isn't arbitrary. Lending protocols' refresh paths commonly touch several mutable zero-copy accounts in a single call — exactly where the per-account framework cost (§4) compounds.

## §3 — Annotated Anchor → Pinocchio

The W9 programs are byte-equivalent on the math, divergent only on the framework wrapper. Strip the wrapper and the 76.8% saving falls out.

### The account struct

Both sides declare the same byte layout. Anchor wraps it for IDL emission and discriminator handling; Pinocchio takes the layout raw.

```rust
// programs/anchor-w9-refresh/src/lib.rs
#[account(zero_copy)]              // ← emits 8-byte discriminator + IDL stub
#[repr(C)]
pub struct Reserve {
    pub total_liquidity: u64,
    pub total_borrows: u64,
    pub cumulative_borrow_rate: u64,
    pub borrow_rate_bps: u32,
    pub _pad: u32,
    pub last_update_slot: u64,
}

// programs/pinocchio-w9-refresh/src/lib.rs
#[repr(C)]                         // ← layout only, no discriminator, no IDL
pub struct Reserve {
    pub total_liquidity: u64,
    pub total_borrows: u64,
    pub cumulative_borrow_rate: u64,
    pub borrow_rate_bps: u32,
    pub _pad: u32,
    pub last_update_slot: u64,
}
```

Identical field layout, 40 bytes either way. The Anchor side carries an 8-byte discriminator prefix in the on-chain account; the Pinocchio side does not.

### Loading 5 mutable accounts

**Anchor** — declarative struct + one `.load_mut()` per account:

```rust
#[derive(Accounts)]
pub struct Refresh<'info> {
    pub signer: Signer<'info>,
    #[account(mut)] pub obligation: AccountLoader<'info, Obligation>,
    #[account(mut)] pub reserve_a:  AccountLoader<'info, Reserve>,
    #[account(mut)] pub reserve_b:  AccountLoader<'info, Reserve>,
    #[account(mut)] pub oracle_a:   AccountLoader<'info, Oracle>,
    #[account(mut)] pub oracle_b:   AccountLoader<'info, Oracle>,
}

pub fn refresh(ctx: Context<Refresh>, current_slot: u64) -> Result<()> {
    let mut obligation = ctx.accounts.obligation.load_mut()?;  // ← discriminator + owner + RefMut tracking
    let mut reserve_a  = ctx.accounts.reserve_a.load_mut()?;   // ← same, per account
    let mut reserve_b  = ctx.accounts.reserve_b.load_mut()?;
    let mut oracle_a   = ctx.accounts.oracle_a.load_mut()?;
    let mut oracle_b   = ctx.accounts.oracle_b.load_mut()?;
    // ...math body...
}
```

**Pinocchio** — manual slot destructure + per-account size check + unsafe pointer cast:

```rust
pub fn process_instruction(
    _program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let [signer, obligation_acc, reserve_a_acc, reserve_b_acc,
         oracle_a_acc, oracle_b_acc, ..] = accounts
    else { return Err(ProgramError::NotEnoughAccountKeys); };

    if !signer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let current_slot = u64::from_le_bytes(
        instruction_data[0..8].try_into().unwrap()
    );

    let mut reserve_a_data = reserve_a_acc.try_borrow_mut()?;        // ← borrow check only
    if reserve_a_data.len() < core::mem::size_of::<Reserve>() {      // ← manual size guard
        return Err(ProgramError::AccountDataTooSmall);
    }
    let reserve_a = unsafe {
        &mut *(reserve_a_data.as_mut_ptr() as *mut Reserve)          // ← raw cast, no discriminator check
    };
    // ...repeat for the other 4 accounts...
    // ...math body...
}
```

### Where the CU goes

The bench measures the **Anchor-vs-Pinocchio gap** at two granularities — W3a (1 mut zero-copy account) and W4 (2 accounts) — and the difference between them gives the marginal per-account cost.

| Component | Anchor-vs-Pinocchio gap |
|---|---:|
| First mut zero-copy account (per-ix entry + first load) | ~847 CU |
| Each additional mut zero-copy account | ~329 CU |

The first-account cost rolls together Anchor's dispatcher + sighash check + the first `AccountLoader::load_mut` against Pinocchio's `program_entrypoint!` + manual instruction-data parse + the first `try_borrow_mut`. The per-account cost on subsequent loads is dominated by `load_mut` itself (discriminator check + ownership check + `RefMut` tracking + exit-write hook registration). Sub-decomposition of the 329 CU into those four named components is *not* directly measured — only the aggregate is.

Plugging W9's 5 mut zero-copy accounts into the scaling law: predicted gap = `847 + 4×329 = 2,163 CU`. Measured gap (Anchor 2,956 − Pinocchio 685) = **2,271 CU**, a 108 CU overshoot decomposed in §4.

### The math body — source-equivalent

```rust
let delta_a = current_slot.saturating_sub(reserve_a.last_update_slot);
reserve_a.cumulative_borrow_rate = reserve_a
    .cumulative_borrow_rate
    .saturating_add(delta_a.saturating_mul(reserve_a.borrow_rate_bps as u64));
reserve_a.last_update_slot = current_slot;
// ...identical for reserve_b, plus oracle slot writes and obligation health calc...
```

Diff `programs/anchor-w9-refresh/src/lib.rs` against `programs/pinocchio-w9-refresh/src/lib.rs` on the math region: zero divergence at the Rust source level. Compiled BPF may differ marginally (Anchor pulls in panic / error-conversion machinery the Pinocchio side's `no_std` + `nostd_panic_handler!` strips), but those compile-time differences are folded into the per-ix entry gap above — they don't accumulate per account. The 2,271 CU gap is paid entirely on the framework wrapper, not on the work the math does.

## §4 — 76.8% reduction broken down

The W9 number isn't a one-off; it falls out of two scaling laws measured on simpler workloads.

### Scaling-law extrapolation

| Workload | Mut zero-copy accts | Anchor CU | Pinocchio CU | Δ measured | Δ predicted | Error |
|---|---:|---:|---:|---:|---:|---:|
| W3a (single insert) | 1 | 914 | 67 | **847** | 847 (base) | 0 |
| W4 (matching, 2 accts) | 2 | 1,318 | 141 | **1,177** | 847 + 329 = 1,176 | +1 |
| **W9 (refresh, 5 accts)** | **5** | **2,956** | **685** | **2,271** | 847 + 4×329 = 2,163 | **+108** |

The W3a → W4 jump gives the marginal per-account framework cost on Anchor: **~329 CU**. Linear extrapolation to N=5 predicts a 2,163 CU framework gap; W9 measured 2,271, a 108 CU (≈5%) overshoot.

Two candidate explanations for the residual:

1. **Reserve struct size.** W9's `Reserve` is 40 bytes (carrying a `u32` + `u32` `_pad` pair around the `u64` `cumulative_borrow_rate`); W4's `Tick` is smaller. More bytes copied through `load_mut` is a plausible source of the per-account marginal cost being slightly above the W4-derived 329 CU.
2. **Exit-write hook bookkeeping.** Anchor's `AccountLoader` registers an exit-write callback per mutable account. At N=5 the cumulative hook bookkeeping is a plausible source of residual overshoot that N=1, 2 measurements couldn't isolate.

Neither has been independently measured to attribute the 108 CU precisely — they're the candidates the bench's existing decomposition makes available. What's confirmed: the scaling law predicts within 5% at N=5.

### Per-day economics

The framework gap (2,271 CU per refresh) is what Pinocchio strips. Scaled to a production refresh path:

| Refresh calls/day | Pinocchio savings |
|---:|---:|
| 100,000 | **227.1M CU/day** |
| 1,000,000 (active liquidation) | **2.271B CU/day** |

The 685 CU of useful work per refresh runs identically on both sides — Pinocchio doesn't make the math faster, it strips the wrapper added before the math starts.

### Why this percentage is high

W9's 76.8% is the highest CU-saving percentage in the bench's complex workloads. The reason is composition:

- **CPI hops** (W2, W6) dilute the framework gap because the called program does ~1000 CU of useful work that Pinocchio can't skip. W6's 65.8% saving reflects that dilution.
- **Pure account loading** (W9) has no dilution. Every CU above the work-floor is framework, and Pinocchio strips all of it.

Ix in the same shape class as W9 — mut-zero-copy-dominated, light or no CPI — sit in this percentage range. Examples from the bench: W4 matching 89.3%, W9 refresh 76.8%, W10 vault deposit 69.2%, W11 oracle publish 93.8%. CPI-heavy ix (DEX aggregators, multi-token settlement) sit closer to W6 (65.8%) or W8 (63.9%). Your own hot path's percentage depends on its account count and CPI density — the W9 figure is the upper end for protocols whose hot path is dominated by zero-copy account loading.

## §5 — Honest gotchas

A 76.8% CU saving doesn't come free. Pinocchio shifts the cost from runtime overhead to development discipline:

- **Hand-rolled layouts.** Every account type needs an explicit `#[repr(C)]` body with manual padding awareness. Anchor derives that from `#[account(zero_copy)]`; Pinocchio doesn't. Get the layout wrong and the bug is silent corruption, not a panic.
- **No IDL emission.** Anchor's IDL drives client SDKs, explorer UIs, integrators' generated bindings. A Pinocchio rewrite either ships its own IDL by hand or breaks every downstream consumer that depended on the Anchor IDL.
- **Debug experience.** Anchor's account validation produces named errors with field context (`AccountNotInitialized`, `ConstraintMut`, etc., tied to the field that failed). Pinocchio errors are positional and low-level (`AccountDataTooSmall at index 2`). Production triage gets harder unless every cast and check is wrapped with named-error helpers.
- **Who shouldn't do this.** Protocols where the hot-path ix isn't CU-bound — CPI-heavy DEX aggregators, ix where actual math dominates framework cost — the W9 percentage doesn't apply. Teams without an internal engineer who can own the byte-layout discipline long-term — the migration cost outweighs the runtime savings.

These are trade-offs, not marketing footnotes. The case for rewriting only stacks up when CU pressure is concrete and the team has the surface area to maintain raw account handling.

## §6 — The equivalence-proof half

A CU benchmark tells you the rewrite is **fast**. It does not tell you the rewrite is **right**. The whole point of a Pinocchio refresh ix is to behave indistinguishably from the Anchor original under every input — same final account state, same success/fail outcome, same internal accounting. An audit reads the rewrite and reasons about whether it looks right. Differential testing runs both programs against the same inputs and checks whether they actually agree.

### Methodology

The minimal differential harness has four steps:

1. **Matched fixtures.** Build the same initial state on both sides — Anchor account with the 8-byte discriminator prefix and body bytes B, Pinocchio account with body bytes B (no prefix). Same lamports, same owner-program, same data length modulo the discriminator.

2. **Drive the same action through both.** In one SVM session, send `refresh(slot=S)` to the Anchor program ID and the same `refresh(slot=S)` to the Pinocchio program ID. Use the same signer, same slot, same instruction-data shape adjusted only for whether Anchor wants its sighash prefix.

3. **Byte-compare account bodies.** After the action returns, read both sides' account data. Strip the 8-byte discriminator from the Anchor side. Compare the remaining bytes. Any mismatch is a divergence — flag and capture the input.

4. **Per-invariant assertions on both sides.** Byte equivalence catches the common bugs but not all of them; for any property the rewrite *must* preserve (§7), assert it independently on each side. A divergence in byte equivalence with a satisfied invariant means the invariant check is incomplete; a satisfied byte equivalence with a violated invariant means both sides share the bug.

### What the harness looks like

The exact harness varies by ix, but the shape is small enough to write inline. Conceptual sketch — written for this post, not copied from any library:

```rust
// Conceptual differential check.
for slot in fuzzer.random_slot_sequence() {
    let anchor_ok = send_anchor_refresh(slot);
    let pino_ok   = send_pino_refresh(slot);
    assert_eq!(anchor_ok, pino_ok, "execution parity");

    assert_eq!(
        read_anchor_body(obligation_acc),
        read_pino_body(obligation_acc),
    );
    // ...repeat for reserve_a, reserve_b, oracle_a, oracle_b...

    assert_monotonic_borrow_rate(reserve_a);
    assert_monotonic_borrow_rate(reserve_b);
    assert_slot_only_advances(obligation_acc);
}
```

That's the whole shape — initialize matched state, drive the same action, byte-compare, invariant-check. The fuzzer's job is to pick `slot` sequences that stress edge cases: zero, repeats, backward jumps, large values that exercise `saturating_sub`.

### Real-work signal

This methodology was applied to the bench's six mut zero-copy ix pairings — **W4 matching, W8 AMM, W9 refresh, W10 vault, W11 oracle, W12 perp**. Zero behavioral divergence observed in any pairing across the runs done to date. The W9 refresh is one of those pairings; the harness shape above is faithful to what was actually wired.

If you're considering a Pinocchio rewrite of your own production refresh ix, the same shape applies. §8 walks through reproducing it locally against the bench's W9 pair.

## §7 — 6 invariants → 6 failure modes

Byte equivalence catches the common bugs but not all of them. A rewrite that touches all the right accounts in the right order but, say, accrues `cumulative_borrow_rate` in the wrong direction will produce a byte-compare divergence — *only if* the fuzzer happens to hit an input where the wrong-direction accrual produces a different result from the correct one. For properties that must hold across **every** input, the right check is an invariant assertion on each side independently.

For W9 refresh, six invariants are load-bearing. Each maps to a production failure mode that has cost real lending protocols.

| # | Invariant | If dropped, you ship... |
|---|---|---|
| 1 | Monotonic interest accrual — `cumulative_borrow_rate` non-decreasing | silent debt shrinkage; borrowers get free money on every refresh |
| 2 | Slot monotonicity — `last_update_slot` only advances forward | stale-price liquidation race; bots replay older slots to seize collateral |
| 3 | Health-factor freshness — after `refresh(S)`, `last_health` reflects the prices and amounts visible at slot S | forgotten-account bug; liquidator triggers at a stale price the borrower never saw |
| 4 | No phantom collateral — `last_health × debt_value ≤ collateral_value × 10_000` | inflated health when debt is high and collateral is low; undercollateralized positions pass health checks |
| 5 | Idempotent at same slot — `refresh(S); refresh(S)` ≡ `refresh(S)` | MEV refresh-spam inflates `cumulative_borrow_rate`, distorting interest for everyone |
| 6 | No silent zero-amount divides — `last_health` well-defined when `borrow_amount == 0` | instruction reverts on edge inputs; failed refresh blocks downstream liquidation or settlement |

An audit reads the rewrite and reasons about whether each invariant should hold. Differential testing runs the rewrite under input sequences the author didn't pick, and lets violations emerge from data. The two are complementary: audit catches what the rewrite *might* miss; differential testing catches what the auditor *did* miss. A Pinocchio rewrite shipping to production needs both, not either alone.

## §8 — How to reproduce

The bench is at [`psyto/pinocchio-bench`](https://github.com/psyto/pinocchio-bench). Reproduce the CU numbers:

```bash
git clone https://github.com/psyto/pinocchio-bench
cd pinocchio-bench
./scripts/build.sh
cargo run --release -p bench
```

The bench prints all 14 workloads sequentially. Look for the W9 line; numbers should match §4's table within a few CU. Drift outside ±20 CU means your toolchain or `solana-cli` version has moved relative to the bench's `rust-toolchain.toml` — open an issue.

To reproduce the equivalence-proof methodology, wire a differential harness against `target/deploy/anchor_w9_refresh.so` and `target/deploy/pinocchio_w9_refresh.so` following the four steps in §6. The invariant list from §7 plugs in as the per-side assertion bank.

If you run this against your own refresh-shape ix and the numbers look different, or you find a behavioral divergence the methodology missed — file an issue on `psyto/pinocchio-bench`. The W9 case study is a starting point, not a closed result.

— psyto (saito.hiroyuki@gmail.com)

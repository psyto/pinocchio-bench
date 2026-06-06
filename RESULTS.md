# Results

Measured 2026-06-06 on:

- `anchor-cli 0.32.1`
- `pinocchio 0.11.1` / `pinocchio-token 0.6.0`
- `solana-cli 3.1.14` (Agave)
- `litesvm 0.12.0`
- `rustc 1.95.0`

## Compute Units

| Workload          | Anchor   | Pinocchio | Δ        | Saved   |
| ----------------- | -------: | --------: | -------: | ------: |
| W0 no-op          |      246 |         4 |      242 |  98.4%  |
| W1 signer+write   |      908 |        38 |      870 |  95.8%  |
| W2 SPL CPI        |    3,856 |     1,179 |    2,677 |  69.4%  |

## Binary Size (on-chain `.so`)

| Workload          | Anchor (bytes) | Pinocchio (bytes) | Ratio   |
| ----------------- | -------------: | ----------------: | ------: |
| W0 no-op          |        167,880 |             2,832 |   59×   |
| W1 signer+write   |        173,128 |             3,272 |   53×   |
| W2 SPL CPI        |        186,176 |             5,120 |   36×   |

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

# Roadmap

`pinocchio-bench` is an apples-to-apples Compute Unit comparison between
**Anchor 0.32** and **Pinocchio 0.11** on current Solana tooling, expanding
from synthetic primitives to representative DeFi hot paths.

## Active scope (current toolchain)

- anchor-cli 0.32.1
- solana-cli 3.1.14
- pinocchio 0.11.1
- litesvm 0.12.0
- N=100 runs per workload, median reported

See [`RESULTS.md`](RESULTS.md) for measured numbers.

## Workload roadmap

### Shipped

| ID  | Workload                                       | What it isolates |
| --- | ---------------------------------------------- | ---------------- |
| W0  | No-op entrypoint return                        | Pure framework overhead |
| W1  | Signer check + single state write              | Account write path |
| W2  | Signer check + SPL Token transfer CPI          | Single-hop CPI |
| W3a | Orderbook tick insert (empty book)             | Zero-copy account load |
| W3b | Orderbook tick insert (32-entry shift)         | Per-call work scaling vs framework constant |
| W4  | Matching engine `place_order` (2 mut accounts) | Per-account framework scaling |
| W5  | Matching engine FIFO append                    | Mutation on existing zero-copy state |
| W6  | 3-hop SPL Token CPI chain (Jupiter-route shape)| Per-CPI-hop framework scaling |
| W7  | Token-2022 TransferChecked + no-op hook        | Token-2022 hook overhead |
| W8  | AMM constant-product swap (Raydium/Meteora)    | Canonical DEX hot path: zero-copy state + 4 SPL accounts + 2 CPI |
| W9  | Lending refresh, 5 mut accounts (Kamino shape) | Confirms per-account scaling law linear to N=5 |
| W10 | Vault deposit + NAV share accounting (Yearn/ERC4626) | 2 mut zero-copy + 2 SPL + 1 CPI mid-shape composition |
| W11 | Pyth-style oracle publish (1 mut + EMA, no CPI)       | High-frequency hot path; reproduces W3a constant-gap law |
| W12 | Drift-style perp open_position (3 mut + 2 SPL + 1 CPI) | Largest combined surface; Phase 0 completion |

### Stretch

- Liquidation tick (health-factor check + collateral seize)
- CLMM swap with tick crossing
- Multi-instruction transaction (CPI fan-out)
- Solana Mobile Stack interaction surface

## What this repo is and is not

**Is:**
- A reproducible, public benchmark with versioned toolchain
- A set of paired Anchor and Pinocchio programs implementing the same logic
- Per-workload analysis distinguishing framework overhead from work cost

**Is not:**
- A migration tool (no automatic Anchor → Pinocchio conversion)
- A safety verifier (Pinocchio shifts validation work to the developer; this
  repo measures the gap, not the safety cost)
- A general-purpose Solana program template

## Storytelling and contributions

Two scaling laws are now measured:

| Dimension | Marginal Anchor overhead |
| --------- | ----------------------: |
| Per additional mutable zero-copy account | ~329 CU |
| Per additional CPI hop | ~1,968 CU |

Contributions welcome — especially:
- New workloads matching real DeFi shapes (please bring a target program in mind)
- Toolchain updates as Anchor / Pinocchio / Solana version
- Methodology critique (litesvm vs mainnet-beta drift, N choice, baseline subtraction)

## Companion artifacts

[`RESULTS.md`](RESULTS.md) includes a short list of invariants a fuzzer
attached to W4/W5 would enforce — illustrating the verification work that
*should* accompany any production rewrite. The fuzzer itself is intentionally
out of scope for this repo.

## License

Dual-licensed under MIT and Apache-2.0. See `LICENSE-MIT` and `LICENSE-APACHE`.

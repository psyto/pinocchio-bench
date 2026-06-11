# Blog 02 — Outline

Working title: **"A 76.8% CU Cut on Lending Refresh — and Proving the Rewrite Didn't Lie"**

Target file: `blog/02-w9-lending-refresh.md`

Voice match: blog 01 (`01-anchor-vs-pinocchio-measured.md`) — data-forward, "what the numbers say" framing, no marketing voice.

Total budget: ~2,550 words across 9 sections.

Constraints locked before drafting:

- **Kamino-shape, mapped** — name Kamino exactly once as the recognition anchor (§2), otherwise stay in the abstract "lending refresh" pattern. No klend code references, no implied endorsement.
- **Equivalence-proof half is methodology-only** — no private-tooling code excerpts, no private-tooling API names. Hand-author any code-shape demonstrations fresh for the post.
- **W9 program source is fair game** — `programs/{anchor,pinocchio}-w9-refresh/src/lib.rs` is already public on `psyto/pinocchio-bench`; excerpts welcome.

---

## §1 — Hook (~200 words)

- Open cold with the number: "On a lending-refresh hot path that touches 5 mutable zero-copy accounts, you pay **2,956 CU per call** on idiomatic Anchor. **685 CU** is the math. The other **2,271 CU is framework tax** you ship in production today."
- One-line scope: "This post measures the tax on a Kamino-shape refresh, replaces it with a Pinocchio rewrite, and shows how to prove the rewrite did not silently change behavior."
- No Kamino name yet — let the reader self-map. Three sentences max.
- **Artifact:** none.

## §2 — The 5-account zero-copy shape (~250 words)

- Topology diagram (ASCII): `obligation → 2 reserves → 2 oracles`, all mut zero-copy.
- "If you've read Kamino's `refresh_obligation`, you've seen this shape" — single Kamino reference, anchors recognition without dependence.
- Note the W9 bench programs use the same shape; numbers in this post come from `programs/{anchor,pinocchio}-w9-refresh`.
- **Artifact:** ASCII topology block.

## §3 — Annotated Anchor → Pinocchio (~400 words)

- Side-by-side: one mut-reserve load on each side, real source from `programs/anchor-w9-refresh/src/lib.rs` and `programs/pinocchio-w9-refresh/src/lib.rs`.
- **Anchor side**: `AccountLoader<'_, Reserve>` + `.load_mut()` → discriminator check + ownership check + exit-write hook registration + mutable-borrow tracking. Each enumerated.
- **Pinocchio side**: explicit `owner == PROGRAM_ID` assert + `unsafe { &mut *(data.as_mut_ptr() as *mut Reserve) }`. What you give up: derive-magic, IDL emission, automatic discriminator.
- Annotate each block: "← 240 CU (framework)" vs "← 137 CU (math)" — sourced from W3a/W4 single-account decomposition in `RESULTS.md`.
- **Artifact:** two-column code fence (50–70 lines total) with inline `// ← N CU` annotations.

## §4 — 76.8% reduction broken down (~350 words)

- **Table 1** — per-account cost decomposition:

  | Component | Anchor CU | Pinocchio CU |
  |---|---|---|
  | discriminator | ~80 | 0 |
  | ownership check | ~40 | ~10 |
  | AccountLoader/load | ~110 | 0 |
  | exit-write hook | ~99 | 0 |
  | (work) | varies | varies |

- **Table 2** — scaling-law extrapolation reproducing `RESULTS.md` W9 — W3a (N=1, gap=847) + W4 (N=2, gap=1,177) → predicted W9 = 847 + 4×329 = 2,163 CU, measured 2,271 CU, **108 CU overshoot** explained (Reserve u128 padding + exit-write at N=5).
- **Per-day math**: "100K refresh calls/day → 227M CU saved. 1M/day (active liquidation period) → 2.27B CU saved." Pure framework, before any oracle/math optimization.
- **Artifact:** 2 tables + reference to CU chart (Task #3).

## §5 — Honest gotchas (~300 words)

- This section is the trust-builder. Without it, blog reads like marketing.
- **What Pinocchio costs**: hand-rolled `#[repr(C)]` layouts, padding awareness, no derive macros, no IDL.
- **Migration cost**: every account type re-described in explicit byte layout; alignment bugs trade panic for silent corruption.
- **Debug experience**: Anchor errors name fields; Pinocchio errors are byte offsets.
- **Who shouldn't do this**: protocols where hot-path CU isn't the bottleneck, teams without an internal Solana-low-level engineer to own the layouts long-term.
- **Artifact:** none.

## §6 — The equivalence-proof half (~400 words, METHODOLOGY-ONLY)

- **The risk**: a Pinocchio rewrite that benchmarks fine but silently diverges on edge inputs (overflow paths, slot-equal cases, zero-amount edges). Audit catches code smell; differential testing catches *behavior*.
- **Methodology** (no tool name, no API surface):
  1. Build matched account fixtures — Anchor side with 8-byte discriminator prefix, Pinocchio side raw, same initial body bytes.
  2. Drive the same `refresh(slot)` action through both program IDs in one SVM session.
  3. After execution: strip Anchor's discriminator, compare every account body byte-for-byte.
  4. Per-invariant assert on **both** sides (not just equivalence — also that each side independently satisfies the property).
- **Hand-authored pseudocode** (~15 lines, no library/trait names):

  ```rust
  // Conceptual differential check — written for this post.
  for slot in fuzzer.sequence() {
      let anchor_ok = run_anchor_refresh(slot);
      let pino_ok   = run_pino_refresh(slot);
      assert_eq!(anchor_ok, pino_ok, "execution parity");
      assert_eq!(read_body(anchor_obligation), read_body(pino_obligation));
      assert_eq!(read_body(anchor_reserve_a),  read_body(pino_reserve_a));
      // ...5 accounts total
      assert_monotonic_borrow_rate_both_sides();
  }
  ```

- **Measurement citation**: "Running this against a fuzzer over random slot sequences: **551,000+ verified-equivalent actions** across all 5 accounts, **0 divergence**." No tool name.
- **Artifact:** hand-authored pseudocode block.

## §7 — 6 invariants → 6 failure modes (~250 words)

- Table rephrased from `RESULTS.md` "What solinv would attach to W9" — drop tool-name framing. Re-cast as: "Invariants any production refresh rewrite must hold."

  | # | Invariant | If dropped, you ship... |
  |---|---|---|
  | 1 | Monotonic interest accrual | silent debt shrinkage (free money for borrowers) |
  | 2 | Slot monotonicity | stale-price liquidation race |
  | 3 | Health-factor freshness | forgotten-account bug, liquidator steals at stale price |
  | 4 | No phantom collateral | inflated health under low debt → undercollateralized loans pass check |
  | 5 | Idempotent at same slot | MEV refresh-spam inflates `cumulative_borrow_rate` |
  | 6 | No silent zero-amount divides | panic on edge inputs, instruction reverts in production |

- **Artifact:** 6-row table.

## §8 — Service shape (~250 words)

- First public price + time naming (Phase 2 outreach gate).
- **Engagement**: 4–6 weeks, **$300–500K**.
- **Deliverable bundle**: migration plan + Pinocchio rewrite + equivalence harness + 6-invariant attestation + deployment-ready binary + 30-day post-deploy support.
- **Fit**: live mainnet protocol, hot-path ix with 3+ mutable account loads, $5M+ TVL ("CU savings have to compound to dollars").
- **Not fit**: pre-launch protocols, teams with existing Pinocchio expertise (just hire), hot paths dominated by CPI (W2/W6 territory — different optimization stack).
- **Artifact:** none.

## §9 — Pilot CTA (~150 words)

- 1–2 Tier 2 pilot slots: **half-price** + reference rights.
- Constraints reaffirmed (mainnet, W9-shape, $5M+ TVL).
- Honest scope: "This is the only outreach happening. No cold email, no sales team. If the post hit your problem, reply."
- Contact: email + X handle.
- **Artifact:** none.

---

## Cross-cutting choices — still open

| Choice | Default | Where it surfaces |
|---|---|---|
| Real source or stylized code in §3? | Real (W9 programs already public) | §3 |
| Author byline (Hiro / psyto / handle)? | psyto, with email contact | §9 |
| EN-only or EN+JA? | EN first, JA polish pass if Tier 2 outreach includes JP protocols | end of pipeline |
| Distribution timing — pair with blog 01? | Yes (ship 01+02 as 2-part series) | post-publish |
| §8 price/time anchor — commit to $300–500K / 4–6 weeks publicly? | Yes — first public pricing anchor | §8 |

## Sequencing after outline approval

1. CU chart — 1 hour, gives §4 its visual anchor
2. Draft §3 + §4 (numeric core, lowest creative load)
3. Draft §6 + §7 (load-bearing differentiator, longest review pass)
4. Draft §1 + §2 + §5 + §8 + §9 (framing + trust + CTA)
5. Self-review pass, then user review

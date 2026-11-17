# Goal Journal — rnl-001-review-2

### TODO-001: Round 1 — reproduce both second-review findings

- **Task**: Probe `from_parts_with` bit budget and construct the `RatioValue` counterexample in an
  isolated fixture. No fixes.
- **Priority**: P0
- **Status**: In progress

### RISK-001: Bit-budget test may hit the preflight rather than the final check

- **Risk**: A `Limits::bits(1)` case may be refused by `check_digits` only if a digit limit is also
  present; the test must isolate bit-only so the current implementation returns `Ok`.
- **Mitigation**: Use `2/1` with `Limits::bits(1)`, assert `BitBudgetExceeded`; verify the pre-fix
  result is `Ok` before applying the fix.

### OBS-001: `from_parts_with` ignored bit-only budgets

- **Source**: `lovelace-rational/src/lib.rs:356-380` at `428f3ba`; `docs/goal-docs-rnl-001-review-2/round-1/bit-budget.md`
- **Fact**: Before the fix, `from_parts_with(2, 1, &Limits::bits(1), &NeverCancel)` returned `Ok(2/1)`; the required result is `BitBudgetExceeded { limit: 1, observed: 3 }`. The zero branch also returned before any limit check; `bits(0)` on zero returned `Ok(0/1)`.
- **Implications**: Second-review Finding 1 is a real bug. The digit preflight is not a substitute for checking the normalized value.
- **Confidence**: High
- **Related**: HYP-001

### OBS-002: A two-method partial duplicate kernel evaded the checker

- **Source**: `tools/check-architecture.py:893-915` at `428f3ba`; `docs/goal-docs-rnl-001-review-2/round-1/partial-duplicate.md`
- **Fact**: Before the fix, `RatioValue { n: Int, d: Nat }` with `add` and `div` produced 0 violations (the arbitrary-name heuristic required 4 methods). After adding `minimumRepresentationMethods: 2`, the same fixture exits 1 with `duplicate-numeric-representation`.
- **Implications**: Second-review Finding 2 is a real enforcement gap; the ordinary incremental duplicate is now caught without a whole-crate exemption.
- **Confidence**: High
- **Related**: HYP-002

### OBS-003: Both fixes are green in the final tree

- **Source**: commands run by the orchestrator on 2026-11-17
- **Fact**: `cargo test --workspace --no-fail-fast` reports 45 targets, **6280 passed, 0 failed, 0 ignored**; `lovelace-rational` is 34 (16 unit + 16 property + 2 oracle). Real architecture scan is 14 crates/332 files/0 violations; mutation suite is **21/21**; the valid fixture is 7/7/0.
- **Implications**: D1–D4 pass locally. CI remains to be run after the commit/push.
- **Confidence**: High
- **Related**: EVD-001..EVD-006

### VAL-001: Second-review fixes

- **Target**: HYP-001, HYP-002
- **Method**: Read the pre-fix implementations; ran the new tests/mutations before/after; re-ran the full workspace, checker, mutation suite, valid fixture and fmt.
- **Evidence examined**: the round-1 reports and the commands above.
- **Result**: Supported
- **Conclusion**: Both second-review findings are fixed. No existing test or scenario was removed; the mutation count increased from 20 to 21.

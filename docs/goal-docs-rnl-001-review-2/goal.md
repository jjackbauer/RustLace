# Goal — RNL-001 second-review remediation

**Outcome (one sentence):** PR #9's second review has no remaining blocking/P1 defect because
`Rat::from_parts_with` enforces bit-only budgets and the architecture checker rejects a partial
duplicate numeric kernel, both proven by failing-first tests/mutations and green CI.

## User request (verbatim excerpts)

> ### **1. Bit-budget enforcement is missing from the new constructor**
> `Rat::from_parts_with()` accepts `Limits`, which supports both decimal-digit and bit budgets.
> However, its implementation calls `limits.check_digits()` before normalization and returns
> `Rat::reduce_by()` without ever calling `limits.check()` on the result.
> **Correction:** Check both configured limits before returning the normalized value. Add tests for
> digit-only, bit-only, and combined budgets, including zero and already-reduced inputs.

> ### **2. A partially implemented duplicate kernel can still evade detection**
> ... a domain-local type called `RatioValue`, with fields `n: Int` and `d: Nat`, implementing only
> `add` and `div`, would not match the reserved names, recognized field pairs, or four-method
> threshold. **Correction:** Add a mutation test for this scenario and implement a stronger
> structural signal for new scalar representations. It could combine numeric storage, arithmetic
> methods, and duplication of an existing abstraction, while allowing explicit reviewed domain
> types.

## Mechanisms (all KEPT)

| # | Mechanism | Disposition | Where |
|---|---|---|---|
| M1 | `from_parts_with` checks bit and digit limits on the normalized value | KEPT | `lovelace-rational/src/lib.rs` |
| M2 | Tests for bit-only, digit-only, combined, zero, already-reduced | KEPT | `lovelace-rational/tests/properties.rs` |
| M3 | Partial duplicate kernel (`RatioValue` with `n`/`d`, add/div) rejected | KEPT | `tools/check-architecture.py` + mutation fixture |
| M4 | Explicit reviewed domain types remain permitted | KEPT | `permittedRepresentations` and valid fixture |
| M5 | Green local gates and PR CI | KEPT | checks below |

## Definition of done

| ID | Check | Command / observable |
|---|---|---|
| D1 | Bit-only budget enforced | `cargo test -p lovelace-rational normalization_enforces_bit_budget` fails before the fix (`Ok`), passes after (`BitBudgetExceeded`) |
| D2 | All budget combinations covered | tests for digit-only, bit-only, combined, zero and already-reduced pass; no pre-fix fallback |
| D3 | Partial duplicate rejected | `python tools/test-architecture-gates.py` has a `partial-duplicate-kernel` scenario that exits 1 with `duplicate-numeric-representation` (or the named rule) |
| D4 | No false positive regression | real workspace `python tools/check-architecture.py` is 0 violations; the valid fixture stays 0 |
| D5 | Full local + CI green | `cargo test --workspace`, checker/mutation suite, docs gates and all seven PR checks pass |

## Constraints

- No weakening/deleting existing tests or scenarios; no broadening a policy exemption.
- Keep digit-only and bit-only preflights documented; the fix is the missing final check.
- Do not alter the PR base/head or merge.
- Record exact command output; a claim without output is not evidence.

## Stop criteria

D1–D5 met and reproduced; round cap 4.

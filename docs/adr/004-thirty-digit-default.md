# ADR-004: 30-digit default precision in the desktop

**Status:** accepted (user-directed deviation DEV-009)

## Context

The Rust engine's own default is a 1000-digit computation budget
(`lovelace-lang/src/lib.rs:836`). The C# extension host's untouched knob was **30 computation / 15
display** digits, and 30 sits inside the shared fixed-width gate that is `<= 37` digits
(`lovelace_lang::complex::FAST_PATH_MAX_DIGITS`); at 1000 every precision-selected fast path is
skipped.

## Decision

`lovelace-desktop` defaults a fresh settings document and a new document to **30** digits
(`lovelace-desktop/src/engine.rs:45`–`54`, `pub const DEFAULT_PRECISION: usize = 30`). A persisted
`active_precision` keeps its stored value: loading settings never rewrites it, so an installation
that stored 1000 keeps 1000 (`precision_default.rs:82`–`93`).

The choice is evidenced three ways in `lovelace-desktop/src/precision_default.rs`:
the constant and the fresh-default paths; a **representation witness** — `movingavg([2.5], 3)`
renders the reference's fixed-width `0.(8)` at 30 and 37 and the arbitrary-precision `0.8(3)` at 38
and 1000 — and a timing observation that `sqrt(2)` at 30 digits is materially faster than at 1000.

The witness proves **which code path ran**. It does not prove that either rendering is correct; in
fact the 30-digit one is wrong. See the correction below.

## Consequences

- The desktop matches the extension host's budget rather than the engine's raw default.
- No settings migration: the schema is unchanged, and only fresh defaults moved.
- Revisit if parity with a host that wants 1000 digits is required; the constant is the single knob.
- The 30-digit default selects the reference's fixed-width path, and that path carries a reproduced
  rendering defect. "Uses exact decimal storage" is therefore not by itself a correctness claim about
  a 30-digit result.

## Correction (2026-11-15): the `movingavg` witness is a defect, not a correctness claim

**The mathematics.** `2.5 / 3 = 5/6 = 0.8(3)` exactly. The fixed-width rendering `0.(8)` is `8/9`.
Those are different numbers, and the gap is not a last-digit approximation: `8/9 - 5/6 = 1/18`, about
5.6%. The first significant digit is wrong. This ADR previously cited the pair as evidence for the
precision default without saying so; this section records the correction.

**The mechanism.** `dsp_lreal128_division_render` (`lovelace-lang/src/builtins.rs:5701`) emulates the
reference's fixed-width long division by tracking the remainder history and the accumulated digit
signature. It records each remainder *before* emitting the digit that remainder produces, so when the
remainder recurs the recorded `period_start` is the position before the digit that was just written.
The signature at that moment already contains that digit. For a sample whose decimal exponent is
negative (`2.5` is `25 × 10^-1`), the digit string therefore holds the integer digit `8` followed by
one fractional digit `3`, while `frac_len` is computed as 2. The period is anchored at fractional
position 0 instead of position 1, and the renderer emits `0.(8)`.

At `precision <= 37` the correct `Real::div` quotient is computed first and then **replaced** by this
render (`builtins.rs:5820`–`5835`), because the two strings differ. At `precision >= 38` the swap is
skipped and the correct `0.8(3)` survives. That is why the witness flips at exactly 38 — the flip
measures path selection, and the ≤ 37 branch is the wrong one.

**It is not a single bad input.** `movingavg([0.1], 3)` renders `0.(0)` — which is zero — where
`0.1/3 = 1/30 = 0.0(3)`. Any fractional sample whose quotient has a short period and a non-empty
pre-period can be shifted this way. `movingavg([1/3], 3)` happens to come out right (`0.(1)` = `1/9`)
because its pre-period is empty, which is why the defect is easy to miss in a spot check.

**It is not even marked inexact.** `dsp_movingavg` flags a result `inexact` only when the *sample* was
periodic (`fixed_periodic`, `builtins.rs:5836`–`5838`). `2.5` has a terminating expansion, so
`movingavg([2.5], 3)` is returned as an **exact** periodic real, carrying exact provenance, holding
`8/9` where the answer is `5/6`. The input this ADR used as its witness is therefore the worst case:
a wrong value presented as an exact one. Only `movingavg([1/3], 3)` receives the inexact marker — and
that case happens to be correct, which is exactly why a spot check can pass.

**Observed.** On Linux x86_64, v0.4.1, with `lovelace-run`:

```console
$ lovelace-run --eval "2.5/3" --text
= 0.8(3) (Real)
$ lovelace-run --eval "movingavg([2.5], 3)" --text
= [0.(8)] (Vector)
$ lovelace-run --eval "setprecision(38); movingavg([2.5], 3)" --text
= [0.8(3)] (Vector)
$ lovelace-run --eval "movingavg([0.1], 3)" --text
= [0.(0)] (Vector)
```

The wire envelopes for both `movingavg` results carry `"exact": true`; `movingavg([1/3], 3)` alone
carries `"exact": false`. The exactness split is therefore inverted relative to correctness, which is
why the marker cannot be cited as a mitigation.

The regression test
`dsp_round20::movingavg_below_38_reports_a_reference_defect_where_the_quotient_is_exact` now pins
both sides in one place — the exact quotients (`5/6`, `1/30`) and the rendered results — so the
mathematical oracle and the parity expectation have to move together or not at all.

**What this ADR does not do.** It does not change the arithmetic. Matching a reference value is
*behavioural parity*; producing `5/6` is *mathematical correctness*. Which one the shipped product
must do is a product decision, and it needs:

1. a mathematical oracle test independent of the C# fixtures, asserting `5/6` and `1/30` for these two
   inputs, so that parity and correctness are pinned separately and neither can drift unnoticed;
2. an explicit decision on whether the malformed `< 37` rendering is preserved for compatibility or
   fixed in both the engine and the parity fixtures (a fixture change, not a test weakening);
3. a precision-limited statement in the reference docs for every DSP builtin that divides by a
   non-exact constant, rather than the blanket phrase "exact 30-place decimal".

## Resolution (2026-11-15): the emulation is removed, not preserved

The product decision above was taken in favour of **mathematical correctness**. Byte-parity with a
reference that returns `8/9` for `5/6` is not worth having, and the port had already improved on the
reference elsewhere.

**What changed.** `dsp_movingavg` no longer computes or substitutes the reference's fixed-width
rendering. It divides through `Real::div` at every precision. `dsp_lreal128_division_render`,
`dsp_lreal128_convertible`, `dsp_finite_decimal_parts` and `dsp_period_block_len` were deleted — 162
lines of emulation whose only purpose was to reproduce the defect.

**Observed after the change** (Linux x86_64, v0.4.1, `lovelace-run`):

```console
$ lovelace-run --eval "movingavg([2.5], 3)" --text
= [0.8(3)] (Vector)
$ lovelace-run --eval "movingavg([0.1], 3)" --text
= [0.0(3)] (Vector)
$ lovelace-run --eval "movingavg([1/3], 3)" --text
= [0.(1)] (Vector)
$ lovelace-run --eval "movingavg([1/3], 7)" --text
= [0.(047619)] (Vector)
```

All four carry `"exact": true`, which is now correct: each is an exact rational. The previous
inversion — wrong values marked exact, the right value marked inexact — is gone, because nothing
re-derives a quotient through a lossy expansion.

**Tests.** `movingavg_matches_exact_division_at_every_precision` replaces the defect-pinning test and
asserts the same values at the default, at 37, at 38 and at 1000 digits, against direct division as
the independent oracle. `movingavg_given_periodic_samples_is_exact_at_every_precision` now asserts
exactness. The desktop's DEV-009 witness
(`the_precision_gated_fixed_width_path_is_selected_at_30_and_not_at_1000`) has been retired and
replaced by `movingavg_is_precision_independent_after_the_fixed_width_emulation_was_removed`: the
observable it relied on no longer differs across precisions, so the 30-digit default is now a
performance choice that costs no correctness.

**Consequence for this ADR's original decision.** The 30-digit default stands on observations 1 and 3
of `precision_default.rs` (the constant and the default paths, and the digit-budget timing). The
representation witness cited in "Decision" above is historical and no longer describes the product.

## Independence from the deleted extension host (2026-11-17)

ADR-009 deletes the Modus plugin host. The engine's `dsp_precision()` helper — 30 places until
`setprecision` is called, then the engine knob — is unchanged by that deletion and still drives
every DSP builtin (`lovelace-lang/src/builtins.rs`). The desktop's own 30-digit default and this
engine-level DSP default are separate decisions that happen to share a value; neither depends on a
loaded family. The runner's `dsp_plugin_untouched_knob_computes_at_fast_budget_and_promotes` test
pins the engine behaviour, and `movingavg_matches_exact_division_at_every_precision` pins that the
digit budget is a performance choice that costs no correctness.



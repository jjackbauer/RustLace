# `lovelace-real`

Arbitrary-precision reals with exact periodic decimals.

## Purpose

A value is `int * 10^exp` plus optional period metadata: when the value has a repeating decimal
expansion, `period_start`/`period_len` describe the block that repeats forever and the stored digits
contain that block exactly once (`lovelace-real/src/lib.rs`). `1/3` is stored as the digit `3` with
exponent −1 and a period of length one, and renders `0.(3)` — never a rounding.

Arithmetic on terminating values is done directly on the scaled integers, which keeps `sqrt`/`pi`
fast. Arithmetic involving a repeating value converts to an exact rational first, so `1/3 + 1/3` is
`0.(6)` and not `0.6`.

## Inputs

### Public surface

- `Real`: `zero`, `from_i64`, `from_int`, `from_str`, `from_fraction(num, den, max_digits)`,
  `to_decimal_fixed`, `to_rational`, `is_exact`, `is_integer`, `to_int_exact`, `period_start`,
  `period_length`, `add`, `sub`, `mul`, `div(max_digits)`, `neg`, `abs`, `pow_int`, `sqrt(digits)`,
  `pi(digits)`.
- Display scoping: `display_decimal_places()` and `with_display_decimal_places(places, f)` — the
  process-wide fractional-digit cap the engine sets around the wire projection, so library code keeps
  every computed digit while a rendering is bounded (the reference's
  `AsyncLocal Real.DisplayDecimalPlaces`).
- `fast_path` — the fixed-width kernel used while the requested precision is inside the shared
  `<= 37` digit gate.

### Dependencies

`lovelace-nat`, `lovelace-int`.

## Outputs

`add`, `sub`, `mul`, `neg`, `abs`, `sqrt` and `pi` return a `Real` directly; `from_fraction`, `div`,
`pow_int`, `from_str` and `to_int_exact` return `Option` because the request can be refused.
`to_decimal_fixed(digits)` returns a fixed-point decimal string, and `Display`/`to_string` render the
documented form (`0.(3)`, `1.414...`); `to_rational` returns the exact `(Int, Nat)` pair behind the
value. The metadata accessors answer the representation: `is_exact`, `is_integer`, `period_start`,
`period_length`, `is_zero`. `truncated_frac(digits)` returns the value truncated to a fractional
count, and the display scoping helpers return whatever their closure returns.

## Precision behaviour

A `Real` is exact when its decimal expansion terminates or recurs, and precision-bounded when it does
not. A terminating expansion is expanded in full at any budget; a non-terminating one is bounded by
`max_digits`, and a value cut at that bound is marked inexact. `Real::DEFAULT_COMPUTATION_DECIMAL_PLACES`
is 1000, and `from_fraction` applies the reference's two accounting rules: a quotient below one does
not spend its budget on leading zeros once the run alone fills it, and a period that closes exactly
at the cap is never discovered — the quotient is then the budgeted truncation, inexact, with no
period metadata. `sqrt(digits)` and `pi(digits)` are precision-bounded at the requested count.
`with_display_decimal_places` bounds how many fractional digits a non-periodic value publishes while
the scope is active and restores the previous cap afterwards; library code defaults to no cap, so
internal arithmetic keeps every computed digit. An `inexact` marker is provenance, not an error
bound (see the [editorial guide](../editorial.md#the-three-kinds-of-claim)).

## Errors

- `div` returns `None` on a zero divisor.
- `from_fraction` returns `None` when the denominator is zero (the underlying `Nat::div_rem` returns
  `None`); a period longer than the budget is not an error — it is a truncated, inexact value.
- `from_str` returns `None` for text that is not a decimal or periodic literal.
- `to_int_exact` returns `None` when the value is not an integer; it never rounds.
- `pow_int` returns `None` for `0` raised to a negative exponent (the reciprocal division refuses);
  a zero exponent answers `1`.
- `sqrt` of a negative value answers `Real::zero()`; it is not an error, and there is no complex
  result from this method.

## Limitations

- **`sqrt` of a negative value is zero.** The method has no error channel and no complex type, so a
  caller that needs the refusal must test the sign before calling.
- **No non-integer powers.** `pow_int` takes an `i64` exponent; a fractional power is not
  expressible here, and the language refuses `2^(1/2)` at its own layer.
- **Exactness depends on the budget.** A period longer than `max_digits` is truncated and marked
  inexact, so the same rational can be exact at one budget and bounded at another; a period that
  closes exactly at the cap is not discovered at all.
- **`to_int_exact` refuses rather than rounds.** A non-integer has no integer conversion, so a caller
  that wants truncation uses `truncated_frac` first.
- **The display cap is scoped state, not a property of the value.** `with_display_decimal_places`
  changes rendering for the duration of the closure and restores the previous cap; it does not
  truncate the stored digits.
- **No decimal-to-binary bridge.** The type is decimal-backed (`int * 10^exp`); there is no method
  that returns a binary floating-point value, and no precision knob expressed in bits.

## Examples

### Using it

```rust
use lovelace_real::Real;

let third = Real::from_fraction(&1u64.into(), &3u64.into(), 100).unwrap();
assert_eq!(third.to_string(), "0.(3)");
let two = Real::from_i64(2);
assert_eq!(two.sqrt(30).to_string(), "1.414213562373095048801688724209"); // desktop default
```

### Tests

2487 `#[test]` functions across the crate, the largest suite in the workspace. They include the
parity rounds (`real_parity_round7.rs`, `round10`…), the transcendental suite
(`transcendental.rs`, `round25`, `round26`), the fixed-width path (`fast_path_tests.rs`), the
`LReal`/`LReal128` forms (`lreal_tests.rs`) and the precision benchmark helper
(`round36_precbench.rs`).

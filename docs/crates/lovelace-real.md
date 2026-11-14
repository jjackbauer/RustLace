# `lovelace-real`

Arbitrary-precision reals with exact periodic decimals.

## Purpose

A value is `int * 10^exp` plus optional period metadata: when the value has a repeating decimal
expansion, `period_start`/`period_len` describe the block that repeats forever and the stored digits
contain that block exactly once (`lovelace-real/src/lib.rs:1`). `1/3` is stored as the digit `3` with
exponent −1 and a period of length one, and renders `0.(3)` — never a rounding.

Arithmetic on terminating values is done directly on the scaled integers, which keeps `sqrt`/`pi`
fast. Arithmetic involving a repeating value converts to an exact rational first, so `1/3 + 1/3` is
`0.(6)` and not `0.6`.

## Public surface

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

## Dependencies

`lovelace-nat`, `lovelace-int`.

## Tests

2487 `#[test]` functions across the crate, the largest suite in the workspace. They include the
parity rounds (`real_parity_round7.rs`, `round10`…), the transcendental suite
(`transcendental.rs`, `round25`, `round26`), the fixed-width path (`fast_path_tests.rs`), the
`LReal`/`LReal128` forms (`lreal_tests.rs`) and the precision benchmark helper
(`round36_precbench.rs`).

## Using it

```rust
use lovelace_real::Real;

let third = Real::from_fraction(&1u64.into(), &3u64.into(), 100).unwrap();
assert_eq!(third.to_string(), "0.(3)");
let two = Real::from_i64(2);
assert_eq!(two.sqrt(30).to_string(), "1.414213562373095048801688724209"); // desktop default
```

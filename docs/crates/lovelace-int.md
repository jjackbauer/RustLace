# `lovelace-int`

Arbitrary-precision signed integers built on `lovelace-nat`.

## Purpose

An `Int` is a sign-magnitude pair. The sign is always −1, 0 or +1, and zero is always non-negative:
there is no negative zero (`lovelace-int/src/lib.rs`). Magnitude arithmetic is delegated to
`Nat`; the sign rules (XOR for multiplication, same-sign addition, magnitude comparison for
subtraction) live here.

Division uses C# truncating semantics: the quotient rounds toward zero and the remainder takes the
sign of the dividend. Division by zero returns `None`.

## Inputs

### Public surface

- `Int` with `zero`, `from_i64` (without losing `i64::MIN`), `from_decimal`, `to_decimal`,
  `is_zero`, `is_negative`, `magnitude`, `negate`, `add`, `sub`, `mul`, `div_rem`, `pow`,
  `factorial`.
- `pub use lovelace_nat::Nat` — the crate re-exports the magnitude type so a consumer needs one
  import.
- `#![forbid(unsafe_code)]`.

### Dependencies

`lovelace-nat` only.

## Outputs

`add`, `sub`, `mul`, `negate`, `pow` and `factorial` return `Int`; `zero` and `from_i64` construct
one. `div_rem_signed` returns `Option<(Int, Int)>` — quotient and remainder — while `div_rem`
divides by a `Nat` magnitude and returns `Option<(Int, Int)>`. `magnitude()` borrows the backing
`Nat`, `is_zero`/`is_negative`/`sign` report the sign, and `to_decimal` (also `Display`) renders the
canonical decimal text: a leading `-` only when negative, and `"0"` with no sign for zero.
`from_decimal` returns `Option<Int>`.

## Precision behaviour

Every operation is exact. The magnitude is arbitrary precision and the sign is carried separately,
so there is no rounding, no scale and no display budget in the type. Division is the one place where
semantics rather than precision matter: the quotient truncates toward zero and the remainder takes
the dividend's sign, so `-17 / 5` is `-3` with remainder `-2` rather than floor division.

## Errors

- `div_rem` and `div_rem_signed` return `None` on a zero divisor; division by zero is not a panic and
  carries no `LangError`.
- `from_decimal` returns `None` for an empty payload, a leading `+`, whitespace, or any non-digit
  character; a leading `-` is accepted, and `-0` normalises to ordinary zero.
- `to_decimal` cannot fail. `from_i64` accepts every `i64` including `i64::MIN` without losing it.

## Limitations

- **No negative zero and no unsigned form.** The sign is one of −1, 0 or +1 by construction; a
  caller that needs a natural number uses `lovelace-nat` directly.
- **Division truncates, it does not floor.** `-17 / 5` is `-3` remainder `-2`; a caller that wants
  floor semantics must adjust for a negative remainder.
- **No rational or floating-point type.** There is no decimal exponent here, so a fractional value
  cannot be represented at this layer.
- **Machine-bounded interfaces.** `pow` takes a `u64` exponent and `factorial` a `u64` input, so a
  negative power is not expressible even though the values themselves are arbitrary precision.
- **It inherits `lovelace-nat`'s limits.** Magnitude arithmetic is unsigned, `Nat::sub` underflows
  to `None`, and `Nat::from_decimal` refuses a sign, so those refusals surface below this layer.

## Examples

### Using it

```rust
use lovelace_int::Int;

let q = Int::from_i64(-17).div_rem(&Int::from_i64(5)).unwrap();
assert_eq!((q.0.to_decimal(), q.1.to_decimal()), ("-3".into(), "-2".into()));
assert!(Int::from_i64(7).div_rem(&Int::from_i64(0)).is_none());
```

The truncating rule is what the language's `divrem` builtin publishes, and it is why `-17 / 5` is
`-3` with remainder `-2` rather than a floor division.

### Tests

135 `#[test]` functions in `src/`, including the small-domain proof harness
(`proof_small_domain.rs`).

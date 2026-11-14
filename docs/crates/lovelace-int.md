# `lovelace-int`

Arbitrary-precision signed integers built on `lovelace-nat`.

## Purpose

An `Int` is a sign-magnitude pair. The sign is always −1, 0 or +1, and zero is always non-negative:
there is no negative zero (`lovelace-int/src/lib.rs:1`). Magnitude arithmetic is delegated to
`Nat`; the sign rules (XOR for multiplication, same-sign addition, magnitude comparison for
subtraction) live here.

Division uses C# truncating semantics: the quotient rounds toward zero and the remainder takes the
sign of the dividend. Division by zero returns `None`.

## Public surface

- `Int` with `zero`, `from_i64` (without losing `i64::MIN`), `from_decimal`, `to_decimal`,
  `is_zero`, `is_negative`, `magnitude`, `negate`, `add`, `sub`, `mul`, `div_rem`, `pow`,
  `factorial`.
- `pub use lovelace_nat::Nat` — the crate re-exports the magnitude type so a consumer needs one
  import.
- `#![forbid(unsafe_code)]`.

## Dependencies

`lovelace-nat` only.

## Tests

135 `#[test]` functions in `src/`, including the small-domain proof harness
(`proof_small_domain.rs`).

## Using it

```rust
use lovelace_int::Int;

let q = Int::from_i64(-17).div_rem(&Int::from_i64(5)).unwrap();
assert_eq!((q.0.to_decimal(), q.1.to_decimal()), ("-3".into(), "-2".into()));
assert!(Int::from_i64(7).div_rem(&Int::from_i64(0)).is_none());
```

The truncating rule is what the language's `divrem` builtin publishes, and it is why `-17 / 5` is
`-3` with remainder `-2` rather than a floor division.

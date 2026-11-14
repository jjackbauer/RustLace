# `lovelace-nat`

Arbitrary-precision natural numbers over 64-bit binary limbs.

## Purpose

`Nat` is a little-endian array of base-2^64 limbs with no most-significant zero limb; zero is the
empty array (`lovelace-nat/src/lib.rs:1`). This mirrors the reference implementation, which stores
`ulong[]` limbs and converts to decimal text only at the boundary.

## Public surface

- `Nat` with `zero`, `from_u64`, `from_decimal`, `to_decimal`, `is_zero`, `is_one`, `add`, `sub`
  (returns `None` on underflow), `mul`, `div_rem`, `pow`, `factorial`, `mul_pow10`, `to_u64`, and the
  `Add`/`Sub`/`Mul`/`Div`/`Rem` operator family.
- `trait Number` — the Rust equivalent of the reference's `INumber<Natural>` identity surface
  (`zero`, `one`, `is_zero`, `radix`).
- `pub type Natural = Nat` — the reference's type name, kept for source compatibility.
- `DigitStore` / `DigitStoreError` — the decimal-digit view the wire contract publishes
  (`digit_store.rs`).

Internal engines, not part of the public surface: `ntt.rs` (NTT multiplication for large operands),
`bz.rs` (Burnikel–Ziegler division), `newton.rs` (integer square root). A worker budget can be pinned
with `LOVELACE_THREADS`, and the NTT crossover with `LOVELACE_NTT_THRESHOLD`
(`lovelace-nat/src/lib.rs:85`, `:105`).

## Dependencies

None. It is the base of `lovelace-int`, `lovelace-real` and everything above them.

## Tests

271 `#[test]` functions in `src/`, including the small-domain proof harness
(`proof_small_domain.rs`) and the public-API batch (`round39_natural_api.rs`). Two example programs
exist: `examples/verify_mul.rs` and `examples/bz_probe.rs`.

## Using it

```rust
use lovelace_nat::Nat;

let a = Nat::from_u64(2).pow(100);
assert_eq!(a.to_decimal(), "1267650600228229401496703205376");
assert_eq!(Nat::from_decimal("10").unwrap().div_rem(&Nat::from_u64(3)),
           Some((Nat::from_u64(3), Nat::from_u64(1))));
```

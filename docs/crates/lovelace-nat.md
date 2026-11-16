# `lovelace-nat`

Arbitrary-precision natural numbers over 64-bit binary limbs.

## Purpose

`Nat` is a little-endian array of base-2^64 limbs with no most-significant zero limb; zero is the
empty array (`lovelace-nat/src/lib.rs`). This mirrors the reference implementation, which stores
`ulong[]` limbs and converts to decimal text only at the boundary.

## Inputs

### Public surface

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
(`thread_budget`, `ntt_threshold`).

### Dependencies

None. It is the base of `lovelace-int`, `lovelace-real` and everything above them.

## Outputs

`add`, `mul`, `pow`, `factorial` and `mul_pow10` return `Nat`; `sub` and `div_rem` return
`Option<Nat>` / `Option<(Nat, Nat)>`; `from_u64` and the `From`-style constructors build values.
`to_decimal` (and `Display`) return the canonical decimal text, and `as_decimal_str` exposes the
cached rendering as a borrowed slice so repeated calls return the same allocation. `to_u64`,
`to_i64`, `try_get_exact_i64` and `from_decimal` return `Option`, and `from_i64` returns
`Option<Nat>`. `DigitStore` renders decimal digits (`digit_count`, `byte_count`, digit-level
accessors) and reports `DigitStoreError` for a rejected position.

## Precision behaviour

Arithmetic is exact by construction: values are base-2^64 limb arrays, there is no rounding, no
binary floating point and no precision knob that changes a stored value. Decimal text is a boundary
conversion, and the NTT and Burnikel–Ziegler paths are exact algorithms: above the divide-and-conquer
threshold a Burnikel–Ziegler answer is verified (`q*d + r == n` and `r < d`) before it is published,
falling back to long division, so the result is exact whatever the fast path does. `Nat::precision`
and `Nat::display_digits` reproduce the reference's global configuration values (with `-1` meaning
unset or no limit); on this side they are stored configuration, not an arithmetic or rendering cap,
and the decimal renderer does not consult them.

## Errors

- `sub` returns `None` when the result would be negative (the crate never wraps).
- `div_rem` returns `None` on a zero divisor; `decrement` returns `false` at zero.
- `from_decimal` returns `None` for empty or whitespace-only input, for a sign, and for any
  non-digit character; leading zeroes are accepted and canonicalised.
- `from_i64` returns `None` for a negative value; `to_u64`/`to_i64` return `None` when the value does
  not fit the target width.
- `DigitStore::set_digit` returns `DigitStoreError::PositionOutOfRange { position, digit_count }` for
  a position outside `0..=digit_count` (negative values included).

## Limitations

- **Unsigned only.** There is no negative `Nat`: `sub` underflows to `None` and `from_i64` refuses a
  negative value. Signed arithmetic is `lovelace-int`'s job.
- **Machine-bounded arguments.** `pow` takes a `u64` exponent, `factorial` a `u64`, and
  `mul_pow10` a `usize`; the values are arbitrary precision but the interface is not.
- **Division by zero is `None`, not an error type.** A caller must handle the missing answer; nothing
  panics and no `Result` carries a reason.
- **The decimal-digit store is a separate representation.** `DigitStore` and `Nat` meet only through
  canonical decimal text and explicit limb helpers; they never share a backing buffer, so moving
  between them copies.
- **`DigitStore` can hold non-canonical states.** Its low-level mutators (`grow_digits`,
  `set_bitwise`, `set_digit_count`, `set_is_zero`) deliberately reproduce the reference store's
  escape hatches and can create states the value API never produces; the module documents this as a
  caveat of those mutators.
- **The configuration statics do nothing here.** `Nat::precision`/`Nat::display_digits` store the
  reference's globals for parity, but no arithmetic or rendering path in this crate reads them.

## Examples

### Using it

```rust
use lovelace_nat::Nat;

let a = Nat::from_u64(2).pow(100);
assert_eq!(a.to_decimal(), "1267650600228229401496703205376");
assert_eq!(Nat::from_decimal("10").unwrap().div_rem(&Nat::from_u64(3)),
           Some((Nat::from_u64(3), Nat::from_u64(1))));
```

### Tests

271 `#[test]` functions in `src/`, including the small-domain proof harness
(`proof_small_domain.rs`) and the public-API batch (`round39_natural_api.rs`). Two example programs
exist: `examples/verify_mul.rs` and `examples/bz_probe.rs`.

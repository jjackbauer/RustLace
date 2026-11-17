# `lovelace-rational`

The canonical exact rational implementation for the whole workspace: arbitrary-precision numerators
and denominators, full GCD normalization, checked arithmetic, explicit growth budgets and a
four-part exactness report.

## Purpose

`lovelace-rational` owns the one general-purpose `Rat`. Every other crate consumes it:
`lovelace-symbolics` re-exports it, `lovelace-systems` publishes it as `Q`, `lovelace-know`'s
`ExactNumber` is a facade over it, and `lovelace-real` owns the `Rat`↔`Real` conversions. The
architecture policy and checker enforce that ownership; a second rational implementation fails CI
even if its unit tests pass (`docs/architecture-ownership.md`, ADR-011).

The canonical form is part of the contract: the denominator is strictly positive and non-zero, the
sign lives only in the numerator, zero is `0/1`, and the numerator and denominator are divided by
their full arbitrary-precision GCD. Normalization and arithmetic never go through decimal text:
`add`, `sub`, `mul`, `div`, `cmp` and the canonical constructor use `Nat`/`Int` directly.

## Inputs

- `Rat` — a canonical rational. Infallible constructors: `zero`, `one`, `from_i64`, `from_u64`,
  `from_nat`, `from_int`, `from_decimal` (a parsing boundary; also `FromStr`). The fallible
  constructor `from_parts(Int, Nat) -> Result<Rat, RationalError>` returns
  `RationalError::ZeroDenominator`; there is no public constructor that can build an invalid value.
- Arithmetic — `add`, `sub`, `mul`, `neg`, `abs`, `cmp`, `div -> Option<Rat>`,
  `checked_div -> Result<Rat, RationalError>`, `pow_i64 -> Option<Rat>` and
  `checked_pow(i64, &Limits, &dyn Cancel)`. Multiplication and division cross-cancel before forming
  products; addition uses the least common multiple of the denominators.
- Budgets — `Limits::digits(n)`, `Limits::bits(n)`, `Limits::unlimited()` and the `*_with` forms.
  `RationalError::DigitBudgetExceeded` and `BitBudgetExceeded` are the deterministic refusals; a
  budget failure never returns a partially computed value.
- Cancellation — the `Cancel` trait and `NeverCancel`; `checked_pow` polls it between squaring
  steps.
- Exactness — `provenance::{ComputationExactness, InputFidelity, RepresentationExactness,
  ExactnessProvenance, Conversion}`. The four claims stay independent: computation exactness, input
  fidelity, representation exactness and render precision.
- Conversions — `from_nat`, `from_int`, `to_int_exact`, `to_i64`, `to_u64`; the `Real` side lives
  in `lovelace-real` as `Real::from_rational` and `Real::to_rat`.

## Outputs

`Rat` arithmetic returns a canonical `Rat`; `checked_div` and `checked_pow` return
`Result<Rat, RationalError>`. `numerator()` returns `&Int` (the sign carrier) and `denominator()`
returns `&Nat` (strictly positive). `decimal_digits()` and `bit_length()` are size measures derived
from the binary length, not decimal renderings. `Display` renders an integral value as its decimal
integer and any other value as `numerator/denominator`.

A `Conversion<T>` carries `value: T`, `lossless: bool` and an `ExactnessProvenance`. The lossless
flag is true only when neither the input fidelity nor the representation lost anything.

## Precision behaviour

`Rat` is exact; there is no precision knob and no rounding. A computation is either exact or it
returns an error. `Limits` bounds growth but does not approximate: a value over the budget is
refused, not truncated. A renderer that needs a decimal projection asks `lovelace-real` for a
`Real`; the conversion reports `RepresentationExactness::Truncated { precision }` when the period
did not close inside the budget. A `Real` converted back to a `Rat` is an exact representation of
its stored digits and reports `InputFidelity::Truncated` when the value had already lost digits.

## Errors

`RationalError` exposes `code()` and `category()`:

| Failure | Code | Category |
|---|---|---|
| Zero denominator in a constructor | `ZeroDenominator` | `DomainError` |
| Division by zero | `DivisionByZero` | `DomainError` |
| Zero to a negative power | `NegativePowerOfZero` | `DomainError` |
| `i64::MIN` for a non-unit base, or an unrepresentable exponent | `ExponentOutOfRange` | `DomainError` |
| Decimal/bit growth budget exceeded | `RationalDigitBudgetExceeded` / `RationalBitBudgetExceeded` | `BudgetExceeded` |
| Caller cancellation | `Cancelled` | `BudgetExceeded` |
| An invariant in a non-zero-denominator step | `InternalInvariantFailure` | `InternalInvariantFailure` |

`Rat::div` and `Rat::pow_i64` are the `Option`-returning compatibility forms; they propagate the
same refusals and never substitute zero.

## Limitations

- The kernel does not render decimals. Decimal/periodic projection belongs to `lovelace-real`, so a
  caller must depend on that crate to render a rational. This is deliberate: the kernel must not
  depend on a representation built on top of it.
- `decimal_digits()` is an upper bound derived from the bit length; it can over-count by a digit or
  two. It is a budget measure, not a printed digit count.
- `Limits::unlimited()` is an explicit opt-out; with it, a legitimately huge power can exhaust
  memory. `pow_i64` uses the default one-million-digit budget.
- The exactness types describe provenance, not correctness: an exact rational computed from a
  wrong model is still exact under that model.

## Examples

```rust
use lovelace_int::Int;
use lovelace_nat::Nat;
use lovelace_rational::{Limits, NeverCancel, Rat};

let half = Rat::from_parts(Int::from_i64(1), Nat::from_u64(2)).expect("non-zero");
let quarter = Rat::from_parts(Int::from_i64(-1), Nat::from_u64(4)).expect("non-zero");
assert_eq!(
    half.checked_div(&quarter).expect("non-zero divisor"),
    Rat::from_i64(-2)
);

let third = Rat::from_parts(Int::from_i64(1), Nat::from_u64(3)).expect("non-zero");
assert_eq!(third.add(&third).to_string(), "2/3");
assert_eq!(third.checked_pow(3, &Limits::digits(100), &NeverCancel).unwrap().to_string(), "1/27");
assert!(third.checked_div(&Rat::zero()).is_err());
```

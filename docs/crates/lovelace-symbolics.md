# `lovelace-symbolics`

Symbolic expressions: canonical construction, term ordering, printing, differentiation, series,
polynomial algebra and the cooperative cancellation token.

## Purpose

An expression is rebuilt into a canonical sum of terms, like terms are collected, and the printer
reproduces the documented forms exactly (`y + 2*x`, `x^2 - 2*x + 1`, `-1/2/(x + 1)`)
(`lovelace-symbolics/src/lib.rs`). The crate also owns `cancel`, the token every long kernel in the
workspace polls.

## Inputs

### Public surface

- `Expr` — the canonical expression type; `Rat` — an exact rational coefficient (`Int` over `Nat`).
- Modules: `assumptions` (the tri-state assumption kernel), `cancel` (deadline + live flag),
  `polynomials`, `rational_functions`, `groebner`, `symbolic_matrix`, `solution_set`,
  `series_kink`, `parameter_domain`, `rewrite_registry`, `real_literal`, `proof_bridge`.
- Cancellation: `cancel::check()`, `cancel::arm(deadline)`, `cancel::arm_with_flag(deadline, flag)`,
  `cancel::observed()`. `lovelace-lang` re-exports the module as `lovelace_lang::cancel`
  (`lovelace-lang/src/lib.rs`).

### Dependencies

`lovelace-nat`, `lovelace-int`.

## Outputs

`Expr` constructors (`num`, `real`, `rational`, `sym`, `complex`, `add`, `mul`, `pow`, `neg`, `sub`,
`div`, `relation`, `and_all`, `or_all`, `logical_not`, `piecewise`, `sqrt`, `derivative`) return an
`Expr` in canonical form, and `diff` returns the derivative as an `Expr`. Predicates and projections
return simple values: `free_of`, `contains`, `contains_fun`, `contains_sym`, `has_imaginary_unit`,
`has_algebraic_power`, `has_pole_risk`, `is_constant` return `bool`, while `as_piecewise` and
`as_derivative` return `Option` for the shape they recognise. `Rat` accessors return its
`numerator()`/`denominator()` and arithmetic returns `Rat`; `div`, `pow_i64` and `to_i64` return
`Option`. `Display`/`to_string` render the canonical printed form, and the cancellation functions
answer `bool` (`check`, `expired`, `observed`, `poll`) with `arm` returning a `Scope` guard that
restores the previous armed state when dropped.

## Precision behaviour

All coefficients are exact rationals: a `Rat` is an `Int` numerator over a `Nat` denominator, so
construction, arithmetic and differentiation neither round nor lose a term. A `Real` literal enters
through `Expr::real(text)`, which keeps the decimal text rather than converting to a binary float.
A series carries its O-term as part of the expression, so the order is visible in the value rather
than implied. Precision policy above this layer (the solver's 40-place working precision, display
formatting) belongs to `lovelace-lang`; this crate has no precision knob and no floating-point path.

## Errors

The crate has no error enum. A request that cannot be represented is answered with an `Option`:

- `Rat::div` returns `None` on a zero divisor.
- `Rat::pow_i64` returns `None` for `0` raised to a negative exponent.
- `Rat::to_i64` returns `None` when the exact value does not fit an `i64`.
- `Expr::as_piecewise` / `Expr::as_derivative` return `None` for an expression of another shape.

Cancellation is not an error here either: `cancel::check()` and `cancel::poll(iteration)` answer
`true` when the deadline or live flag has expired, and the kernel returns its identity or partial
value. The evaluation above the kernel is what turns that into a structured `Cancelled` refusal.

## Limitations

- **Canonicalisation is lossy for presentation.** The input's textual form, term order and grouping
  are not preserved: `x*2 + 3*x` is `5*x` and `x + 0` is `x`, and the printer emits the canonical
  form, not the form that was typed.
- **It is an algebra, not a numeric evaluator.** The crate constructs and rewrites expressions; it
  has no root-finding, quadrature or numerical evaluation entry point.
- **Cancellation is cooperative.** A kernel must call `poll`/`check`; a deadline cannot interrupt
  arbitrary code, and the check's granularity is `POLL_INTERVAL` iterations for a hot loop.
- **Cancellation state is thread-local.** A deadline belongs to the evaluation that armed it;
  another thread cannot observe it, and a leaked scope would be the only way to inherit one (the
  `Scope` guard prevents that).
- **The rational layer inherits the integer limits.** `Rat` is `Int` over `Nat`, so the `Nat`
  restriction to naturals applies to a denominator and an overflowing `i64` conversion is `None`
  rather than a wrapped value.

## Examples

### Using it

```rust
use lovelace_symbolics::{diff, Expr};

let x = Expr::sym("x");
let f = Expr::add(Expr::pow(x, Expr::num(3)), Expr::num(1));
assert_eq!(diff(&f, "x").to_string(), "3*x^2");
```

The desktop reads this crate's expression type only to project the inspector tree; it never mutates
it (`lovelace-desktop/Cargo.toml`, the `lovelace-symbolics` dependency comment).

### Tests

197 `#[test]` functions in `src/`, covering canonicalisation, printing, differentiation, series,
solving, Gröbner bases and the assumption kernel.

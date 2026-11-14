# `lovelace-symbolics`

Symbolic expressions: canonical construction, term ordering, printing, differentiation, series,
polynomial algebra and the cooperative cancellation token.

## Purpose

An expression is rebuilt into a canonical sum of terms, like terms are collected, and the printer
reproduces the documented forms exactly (`y + 2*x`, `x^2 - 2*x + 1`, `-1/2/(x + 1)`)
(`lovelace-symbolics/src/lib.rs:1`). The crate also owns `cancel`, the token every long kernel in the
workspace polls.

## Public surface

- `Expr` — the canonical expression type; `Rat` — an exact rational coefficient (`Int` over `Nat`).
- Modules: `assumptions` (the tri-state assumption kernel), `cancel` (deadline + live flag),
  `polynomials`, `rational_functions`, `groebner`, `symbolic_matrix`, `solution_set`,
  `series_kink`, `parameter_domain`, `rewrite_registry`, `real_literal`, `proof_bridge`.
- Cancellation: `cancel::check()`, `cancel::arm(deadline)`, `cancel::arm_with_flag(deadline, flag)`,
  `cancel::observed()`. `lovelace-lang` re-exports the module as `lovelace_lang::cancel`
  (`lovelace-lang/src/lib.rs:47`).

## Dependencies

`lovelace-nat`, `lovelace-int`.

## Tests

197 `#[test]` functions in `src/`, covering canonicalisation, printing, differentiation, series,
solving, Gröbner bases and the assumption kernel.

## Using it

```rust
use lovelace_symbolics::{diff, Expr};

let x = Expr::sym("x");
let f = Expr::add(Expr::pow(x, Expr::num(3)), Expr::num(1));
assert_eq!(diff(&f, "x").to_string(), "3*x^2");
```

The desktop reads this crate's expression type only to project the inspector tree; it never mutates
it (`lovelace-desktop/Cargo.toml`, the `lovelace-symbolics` dependency comment).

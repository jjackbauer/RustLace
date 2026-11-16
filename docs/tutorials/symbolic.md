# Differentiate symbolically, then cross into decimals

## Problem

You want to work with an expression rather than a number: differentiate it, try to simplify a
rational expression, and then evaluate an irrational constant at a precision you choose. The
interesting part is the boundary. Symbolic rewriting stays exact, but `sqrt(2)` cannot be written
as a finite decimal, so at some point the answer stops being exact and becomes a budgeted
approximation.

This tutorial differentiates `x^3 + 2*x`, watches `simplify` refuse a cancellation, sees what makes
`simplify` fire in a case it can prove, and then asks `evalf` for 20 decimal places.

## Source

```lovelace
x = symbol("x")
f = x^3 + 2*x
df = diff(f, x)
refused = simplify((x^2 - 1)/(x - 1))
assume(x != 1)
still_refused = simplify((x^2 - 1)/(x - 1))
cancelled = cancel((x^2 - 1)/(x - 1))
assume(x != 0)
fired = simplify(x/x)
at_2 = subs(df, x, 2)
approx = evalf(sqrt(2), 20)
```

## Output

```text
= 1.4142135623730950488 (Real)
  _ = 1.4142135623730950488
  approx = 1.4142135623730950488
  at_2 = 14
  cancelled = x + 1
  df = 3*x^2 + 2
  f = x^3 + 2*x
  fired = 1
  refused = (x^2 - 1)/(x - 1)
  still_refused = (x^2 - 1)/(x - 1)
  x = x
```

## Reading the result

`x = symbol("x")` is required. Without that declaration, `diff(x^3 + 2*x, x)` fails with
`Undefined variable 'x'`; there is no implicit symbolic variable.

- `f = x^3 + 2*x` and `df = 3*x^2 + 2` are exact symbolic expressions. `diff` applies the power and
  sum rules and returns a new expression; no decimal approximation happens.
- `refused` is `(x^2 - 1)/(x - 1)`, unchanged. `simplify` only applies rewrites it can justify, and
  it does not silently cancel a factor that could be zero. That refusal is the correct behaviour:
  the expression equals `x + 1` only where `x != 1`.
- `still_refused` is unchanged after `assume(x != 1)`. The assumption alone does not enable the
  rewrite: in this build `simplify` has a rule for `x/x` but no rule that factors `x^2 - 1`, so
  there is nothing to fire even with the condition assumed.
- `cancelled = x + 1` is the result of `cancel`, the explicit polynomial-cancellation operation. It
  does the algebra `simplify` will not do. The removed factor is `x - 1`, so the identity holds only
  where `x != 1`.
- `fired = 1` shows the other side of the boundary. With `assume(x != 0)` active,
  `simplify(x/x)` returns `1`: a matching rule exists and its side condition is provable. That is
  what “firing” looks like. For `(x^2 - 1)/(x - 1)`, even `assume(x != 1)` does not produce a
  result because no rule matches the quotient.
- `at_2 = 14` comes from `subs(df, x, 2)`: substitute exactly and the value is the integer 14.
  Substitution does not introduce an approximation.
- `approx = 1.4142135623730950488` is a `Real` from `evalf(sqrt(2), 20)`. `sqrt(2)` is irrational,
  so this is where symbolic exactness ends and a precision budget begins. The request is for 20
  decimal places; the 20th place is zero, and that trailing zero is not rendered, so 19 digits are
  visible. Asking for 21 places prints `1.414213562373095048801`.

## Assumptions

- RustLace 0.4.1, run through the debug CLI at `/tmp/rl-target/debug/lovelace-run` on WSL Ubuntu.
- Assumptions are session state. `assume(x != 1)` and `assume(x != 0)` remain active for the rest
  of the run; `assume_clear()` would remove them.
- `simplify` is the safe rewriter used here; `cancel` is a separate, explicit algebraic operation.
  Use `cancel_full` when you need the side condition reported alongside the result.
- `evalf` evaluates closed forms. It does not do numeric root-finding or quadrature.
- The script is deterministic, and the command writes no files.

## Precision

Labels follow [the editorial guide](../editorial.md): **exact**, **precision-bounded** or
**parity**.

| Value | Label | Justification |
|---|---|---|
| `3`, `2` in `x^3 + 2*x` and `3*x^2 + 2` | **exact** | Integer coefficients and exponent. |
| `1` in `x - 1`, `x + 1` and `fired = 1` | **exact** | Integer. |
| `14` in `at_2` | **exact** | `3*2^2 + 2` is the integer 14; substitution preserved exactness. |
| `20` in `evalf(sqrt(2), 20)` | **exact** | The requested decimal-place count is an integer parameter, not a measured value. |
| `19` in the visible-digit count | **exact** | An integer count of rendered fractional digits in this run; the 20th digit is zero. |
| `1.4142135623730950488` in `approx` | **precision-bounded** | `sqrt(2)` is irrational; the value is computed to 20 decimal places. The 20th place is zero and is not rendered, so 19 digits are visible. |
| `21` in the 21-place comparison | **exact** | An integer parameter chosen for the demonstration. |

No **parity** values appear in this run. `approx` is the only precision-bounded value; every other
number is exact.

## Verification

Run this from any directory. It prints exactly the output above and writes no files.

```console
/tmp/rl-target/debug/lovelace-run --stdin --text <<'EOF'
x = symbol("x")
f = x^3 + 2*x
df = diff(f, x)
refused = simplify((x^2 - 1)/(x - 1))
assume(x != 1)
still_refused = simplify((x^2 - 1)/(x - 1))
cancelled = cancel((x^2 - 1)/(x - 1))
assume(x != 0)
fired = simplify(x/x)
at_2 = subs(df, x, 2)
approx = evalf(sqrt(2), 20)
EOF
```

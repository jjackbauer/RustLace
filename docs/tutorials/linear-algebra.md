# Solve a linear system and prove the answer

## Problem

You have a small linear system, `A * x = b`, and you want the solution together with a check that
it really solves the system. In a floating-point stack that check is a tolerance: subtract `b` from
`A * x` and accept a small residual. RustLace carries exact decimal values, so the same check can
come back as an exact zero matrix.

This tutorial builds a 2 x 2 system, inverts the matrix, solves with `matmul`, and inspects the
determinant, the solution and the residual gap.

## Source

```lovelace
A = [[2, 1], [1, 3]]
b = [[5], [6]]
Inv = inv(A)
sol = matmul(Inv, b)
residual = matmul(A, sol)
gap = residual - b
d = det(A)
residual
```

The same script is checked in as [linear-algebra.ls](examples/linear-algebra.ls).

## Output

```text
= [[5], [6]] (Array)
  A = [[2, 1], [1, 3]]
  Inv = [[0.6, -0.2], [-0.2, 0.4]]
  _ = [[5], [6]]
  b = [[5], [6]]
  d = 5
  gap = [[0], [0]]
  residual = [[5], [6]]
  sol = [[1.8], [1.4]]
```

## Reading the result

`--text` prints the value of the last statement first — `= [[5], [6]] (Array)` — and then every
variable in scope, sorted by name. `_` is the previous result, so it repeats `residual`.

- `A` is `[[2, 1], [1, 3]]` and `b` is the column vector `[[5], [6]]`. The column shape matters:
  `matmul` multiplies a 2 x 2 matrix by a 2 x 1 matrix, so `b` is written `[[5], [6]]`, not `[5, 6]`.
- `Inv` is `[[0.6, -0.2], [-0.2, 0.4]]`. These are 3/5, -1/5, -1/5 and 2/5. Each is a terminating
  decimal, so the display is the value, not a rounded approximation.
- `sol` is `[[1.8], [1.4]]`, that is (9/5, 7/5). Substitute it back:
  `[[2, 1], [1, 3]] * [[9/5], [7/5]] = [[5], [6]]`.
- `residual` is `[[5], [6]]`: the matrix times the computed solution is exactly the original
  right-hand side.
- `gap` is `[[0], [0]]`. `gap = residual - b` is the exact check: the difference is the zero matrix,
  not a value near zero. There is no epsilon in this script or in the subtraction that produces
  `gap`.
- `d = 5` is the determinant. It is non-zero, so the inverse exists. The value is displayed as a
  `Real` even though it is an integer; `residual` stores exact Real elements with denominator 1.

`matmul(A, [5, 6])` would not work here: a flat vector does not have the column shape the product
requires. Write the right-hand side as a column.

## Assumptions

- RustLace 0.4.1, run from the repository root with the project-root Cargo command
  `cargo run -p lovelace-run -- --file docs/tutorials/examples/linear-algebra.ls --text` on
  Windows, Linux or WSL. `--text` selects the human format shown above. The first run compiles the
  runner.
- `A` is invertible over the rationals; `det(A) = 5` is non-zero.
- `inv` computes the rational inverse exactly. Here every inverse entry has denominator 5, so the
  decimal expansion terminates.
- No tolerance, iterative refinement or floating-point fallback is involved.
- The script is deterministic; every run prints the same values.
- The command writes no files.

## Precision

The four claim kinds are **exact**, **precision-limited**, **error-bounded** and
**parity-verified**, following [the editorial guide](../editorial.md#the-four-kinds-of-claim).

| Value | Label | Justification |
|---|---|---|
| `2`, `1`, `1`, `3` in `A` | **exact** | Integers, stored without a fractional part. |
| `5`, `6` in `b` | **exact** | Integers. |
| `0.6`, `-0.2`, `-0.2`, `0.4` in `Inv` | **exact** | Terminating decimals: 3/5, -1/5, -1/5, 2/5. Each is the exact rational inverse. |
| `1.8`, `1.4` in `sol` | **exact** | Terminating decimals: 9/5 and 7/5. |
| `5`, `6` in `residual` | **exact** | Integers, with denominator 1 in the structured projection. |
| `0`, `0` in `gap` | **exact** | The exact integer zero in each entry. |
| `5` in `d` | **exact** | Integer-valued determinant, displayed as a `Real`. |

Every value in this run is **exact**: the inputs are integers, and every inverse and solution entry
is a terminating rational held without a dropped digit. No value is **precision-limited**, because
no division is truncated and no transcendental is evaluated. No value is **error-bounded**, and none
needs to be: `gap` is the exact zero vector, not a bound on an approximation. No value is
**parity-verified**: the results come from the engine's own rational arithmetic and are not a
reproduction of a reference implementation.

Exactness here is a property of this arithmetic, not a claim that every future division terminates:
a system whose determinant has a prime factor other than 2 or 5 produces a recurring decimal. When
the period fits the active budget the engine stores it exactly; when the period does not fit, the
quotient is truncated and becomes a **precision-limited** value. This run has no recurring quotient.

## Verification

Run this from the repository root. It prints the output above on standard output and the script
writes no files. The command is the same in bash, PowerShell and Command Prompt because the script
is a checked-in file, not a shell here-document.

```console
cargo run -p lovelace-run -- --file docs/tutorials/examples/linear-algebra.ls --text
```

`bash tools/run-tutorials.sh` reruns all four tutorials on Ubuntu and Windows (Git Bash), compares
this transcript with the executed script, and recomputes `det(A)` and `A * sol = b` with
[integer arithmetic that does not call the engine](../../tools/run-tutorials.sh).

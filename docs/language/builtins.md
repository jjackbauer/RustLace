# Builtins

The engine ships **134 builtins** in **12 categories**. This page is the catalogue: one table per
category with each builtin's name, declared arity, and parameter names, followed by prose for the
important families.

The numbers in this page were read from the live registry of a debug `lovelace-run` built from
the current tree:

```console
target/debug/lovelace-run.exe --functions --function-limit 500
```

The observed envelope reports `"total": 134`, `"returned": 134`, `"truncated": false`. The
category counts below sum to 134.

## Purpose

### Registry at a glance

| Category | Builtins | Main families |
|---|---:|---|
| Numerics | 10 | integer predicates, `pi`/`e`, `sqrt`, `evalf`, `setprecision` |
| Symbolics | 40 | symbolic algebra, trigonometry, assumptions, domains, relations |
| Arrays | 23 | construction, shape, reductions, element-wise helpers |
| Linear Algebra | 8 | determinants, inverses, solves, rank, trace, symmetric eigensolver |
| Calculus | 10 | derivatives, integrals, limits, series, Jacobian, Hessian |
| Solving | 4 | scalar and polynomial-system solving |
| Compilation | 5 | MathIR lowering, compilation, kernel evaluation |
| DSP | 16 | transforms, convolution, filters, signal constructors |
| Introspection | 5 | `type`, `inspect`, `latex`, `print`, `capabilities` |
| Language | 1 | `plot` |
| Systems | 10 | queueing, operational laws, open networks, exact MVA, bottlenecks, sweeps |
| Optimization | 2 | expression optimization and its structured form |
| **Total** | **134** | |

## Inputs

### Reading the catalogue

The arity column is derived from the registry's `minArity` and `parameterCount`:

- `1` means exactly one argument.
- `0-1` means one optional argument (zero or one).
- `2-3` means two or three arguments.
- `1+` means variadic: at least one argument, with no upper bound declared.
- `2+` means variadic: at least two.

The parameters column lists the declared parameter names in order. They are documentation names,
not keywords; calls are positional. Every builtin is declared in `lovelace-lang/src/registry.rs`
and dispatched from `lovelace-lang/src/builtins.rs`.

A call with a count outside the declared range is refused uniformly, before the implementation
runs, with `InvalidArgument` / `TypeMismatch` and a `signature` on the JSON envelope:

```lovelace
sqrt(1, 2)
```
```text
Error [InvalidArgument/TypeMismatch]: sqrt(): expected 1 argument; got 2.
```

Wrong argument kinds are refused by the implementation with a sentence naming the parameter and
the kind that arrived (usually `InvalidArgument` / `TypeMismatch` for an argument-shape problem,
or `InvalidOperation` / `DomainError` for a value that is outside the operation's domain).

Builtins are names, not values. `sqrt` without a call is an undefined variable, and a user
function with the same name shadows the builtin because user functions are resolved first. The one
static registry exposes all 134 names in every engine: `Engine::new()` is the only constructor and
there is no reduced or bare engine.

### Full catalogue

#### Numerics (10 builtins)

| Name | Arity | Parameters |
|---|---|---|
| `abs` | 1 | x |
| `divrem` | 2 | a, b |
| `e` | 0-1 | digits |
| `evalf` | 2 | f, digits |
| `is_even` | 1 | x |
| `is_odd` | 1 | x |
| `pi` | 0-1 | digits |
| `setprecision` | 1 | digits |
| `sign` | 1 | x |
| `sqrt` | 1 | x |

#### Symbolics (40 builtins)

| Name | Arity | Parameters |
|---|---|---|
| `acos` | 1 | x |
| `acosh` | 1 | x |
| `and` | 2 | a, b |
| `apart` | 2 | f, x |
| `asin` | 1 | x |
| `asinh` | 1 | x |
| `assume` | 1 | relation |
| `assume_clear` | 0 | &mdash; |
| `assume_integer` | 1 | x |
| `assume_negative` | 1 | x |
| `assume_nonnegative` | 1 | x |
| `assume_positive` | 1 | x |
| `assume_real` | 1 | x |
| `assumptions` | 0 | &mdash; |
| `atan` | 1 | x |
| `atanh` | 1 | x |
| `cancel` | 1 | f |
| `cancel_full` | 1 | f |
| `collect` | 2 | f, x |
| `complex` | 0 | &mdash; |
| `cos` | 1 | x |
| `cosh` | 1 | x |
| `exp` | 1 | x |
| `expand` | 1 | f |
| `factor` | 1 | f |
| `inf` | 0 | &mdash; |
| `integer` | 0 | &mdash; |
| `log` | 1 | x |
| `not` | 1 | a |
| `or` | 2 | a, b |
| `rational` | 0 | &mdash; |
| `real` | 0 | &mdash; |
| `simplify` | 1 | f |
| `simplify_full` | 1 | f |
| `sin` | 1 | x |
| `sinh` | 1 | x |
| `subs` | 3 | f, x, value |
| `symbol` | 1-2 | name, domain |
| `tan` | 1 | x |
| `tanh` | 1 | x |

#### Arrays (23 builtins)

| Name | Arity | Parameters |
|---|---|---|
| `append` | 2 | a, b |
| `concat` | 2-3 | a, b, axis |
| `cross` | 2 | a, b |
| `dot` | 2 | a, b |
| `eye` | 1-2 | rows, cols |
| `flatten` | 1 | a |
| `len` | 1 | v |
| `matmul` | 2 | a, b |
| `max` | 1-2 | a, axis |
| `mean` | 1-2 | a, axis |
| `min` | 1-2 | a, axis |
| `ndims` | 1 | a |
| `norm` | 1-2 | a, axis |
| `numel` | 1 | a |
| `ones` | 1+ | dims |
| `prod` | 1-2 | a, axis |
| `rank` | 1 | a |
| `reshape` | 2+ | a, dims |
| `shape` | 1 | a |
| `squeeze` | 1 | a |
| `sum` | 1-2 | a, axis |
| `transpose` | 1-2 | a, perm |
| `zeros` | 1+ | dims |

#### Linear Algebra (8 builtins)

| Name | Arity | Parameters |
|---|---|---|
| `det` | 1 | m |
| `eig_sym` | 1 | a |
| `inv` | 1 | x |
| `inv_full` | 1 | x |
| `linsolve` | 2 | a, b |
| `linsolve_full` | 2 | a, b |
| `matrix_rank` | 1 | a |
| `trace` | 1 | m |

#### Calculus (10 builtins)

| Name | Arity | Parameters |
|---|---|---|
| `diff` | 2 | f, x |
| `hessian` | 2 | f, vars |
| `integrate` | 2 | f, x |
| `integrate_full` | 2 | f, x |
| `jacobian` | 2 | fs, vars |
| `limit` | 3 | f, x, x0 |
| `limit_full` | 3 | f, x, x0 |
| `limit_left` | 3 | f, x, x0 |
| `limit_right` | 3 | f, x, x0 |
| `series` | 4 | f, x, x0, order |

#### Solving (4 builtins)

| Name | Arity | Parameters |
|---|---|---|
| `solve` | 2-3 | f, x, domain |
| `solve_full` | 2-3 | f, x, domain |
| `solve_system` | 2 | eqs, vars |
| `solve_system_full` | 2 | eqs, vars |

#### Compilation (5 builtins)

| Name | Arity | Parameters |
|---|---|---|
| `compile` | 2 | f, params |
| `compile_full` | 2 | f, params |
| `evalir` | 3 | ir, values, digits |
| `evalir_batch` | 3 | ir, values, digits |
| `lower` | 2 | f, params |

#### DSP (16 builtins)

| Name | Arity | Parameters |
|---|---|---|
| `conj` | 1 | x |
| `conv` | 2 | x, h |
| `cosine` | 3 | freq, phase, n |
| `delay` | 2 | x, k |
| `dft` | 1-2 | x, n |
| `exponential` | 2 | c, n |
| `fft` | 1 | x |
| `filter` | 3 | a, b, x |
| `im` | 1 | x |
| `impulse` | 1 | n |
| `movingavg` | 2 | x, w |
| `noise` | 3-4 | scale, disp, n, seed |
| `powerseries` | 3 | k, a, n |
| `re` | 1 | x |
| `scale` | 2 | x, c |
| `step` | 1 | n |

#### Systems (10 builtins)

Queueing and operations-research models backed by `lovelace-systems` (exact rational arithmetic;
no floating point). Arguments are canonical seconds and jobs per second; every result is a
structured record with its solver provenance.

| Name | Arity | Parameters |
|---|---|---|
| `sys_analyze` | 1-2 | network, precision |
| `sys_bottlenecks` | 1 | network |
| `sys_closed` | 3 | population, think_time, stations |
| `sys_delay` | 2 | lambda, mu |
| `sys_mm1` | 2 | lambda, mu |
| `sys_mmc` | 3 | lambda, mu, servers |
| `sys_mva` | 1-2 | network, precision |
| `sys_open` | 2 | arrival_rate, stations |
| `sys_station` | 3-4 | name, service_time, visits, servers_or_delay |
| `sys_sweep` | 4 | network, from, to, step |

#### Introspection (5 builtins)

| Name | Arity | Parameters |
|---|---|---|
| `capabilities` | 0 | &mdash; |
| `inspect` | 1 | x |
| `latex` | 1 | expr |
| `print` | 0+ | values |
| `type` | 1 | x |

#### Language (1 builtin)

| Name | Arity | Parameters |
|---|---|---|
| `plot` | 1-3 | x, y, title |

#### Optimization (2 builtins)

| Name | Arity | Parameters |
|---|---|---|
| `optimize` | 2 | f, params |
| `optimize_full` | 2 | f, params |

## Outputs

### Numerics (10)

The numeric builtins preserve the exact representation where the operation has one and otherwise
compute at the session's computation precision. A truncated result is precision-limited, and the
`exact` flag is provenance, not an error bound (see the
[editorial guide](../editorial.md#the-four-kinds-of-claim)).

| Builtin | What it does |
|---|---|
| `abs(x)` | Absolute value; preserves Natural/Integer/Real, complex magnitude for Complex. |
| `divrem(a, b)` | Quotient and remainder as a `DivRemResult` record. |
| `e([digits])` | Euler's number at the requested precision (capped by the engine precision). |
| `evalf(f, digits)` | Numerically evaluates a symbolic expression to `digits` decimal places (capped at 1000). |
| `is_even(x)`, `is_odd(x)` | Integer predicates returning Boolean. |
| `pi([digits])` | π at the requested precision (capped by the engine precision). |
| `setprecision(n)` | Sets computation and display precision; returns `Void`. |
| `sign(x)` | `-1`, `0`, or `1` as an Integer. |
| `sqrt(x)` | Principal square root; perfect squares are exact Reals, symbolic arguments stay symbolic. |

`pi(n)` and `e(n)` compute at most the engine's current computation precision; requesting more is
an `InvalidArgument` / `TypeMismatch` refusal naming the cap. `evalf` has its own 1000-place
computation cap and marks a clamped inexact result with `truncated` metadata on the wire.
Precision is covered in [Precision and formatting](precision-and-formatting.md).

### Symbolics (40)

The Symbolics category is the largest. It supplies:

- elementary functions: `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`,
  `asinh`, `acosh`, `atanh`, `exp`, `log`, `abs`;
- algebra: `simplify`, `simplify_full`, `expand`, `factor`, `collect`, `apart`, `cancel`,
  `cancel_full`;
- assumptions: `assume`, `assume_clear`, `assume_integer`, `assume_negative`,
  `assume_nonnegative`, `assume_positive`, `assume_real`, `assumptions`;
- domains and symbols: `symbol`, `real`, `complex`, `integer`, `rational`, `inf`;
- relations and substitution: `and`, `or`, `not`, `subs`, `evalf`.

`simplify` applies only universally valid rules and rules whose side conditions the active
assumptions prove. `simplify_full` returns a `TransformResult` recording the applied rules and
their conditions. `cancel` removes common factors without carrying side conditions; use
`cancel_full` when the definedness delta matters. `integrate_full`, `solve_full`,
`linsolve_full`, `inv_full`, and `limit_full` expose structured records for the same operations.

`and`, `or`, and `not` operate on symbolic relations, not on two Boolean values. `subs(f, x, v)`
substitutes a value; `evalf` numerically evaluates. `inf` is the positive-infinity symbolic
constant.

### Arrays (23)

Array builtins cover construction (`zeros`, `ones`, `eye`), shape (`shape`, `rank`, `ndims`,
`numel`, `len`), reductions (`sum`, `prod`, `min`, `max`, `mean`, `norm`), manipulation
(`reshape`, `flatten`, `squeeze`, `transpose`, `concat`, `append`), and vector/matrix helpers
(`dot`, `cross`, `matmul`). Reductions accept an optional `axis`; `zeros`, `ones`, and `reshape`
are variadic over their shape arguments.

`len` returns the first dimension; `numel` the product of all dimensions; `norm` the Euclidean
norm. `reshape` requires the element count to agree with the new shape. Array arithmetic itself
is not a builtin: `+ - * / % ^` are language operators with right-aligned broadcasting, described
in [Expressions and operators](expressions-and-operators.md#arrays-and-broadcasting).

### Linear Algebra (8)

`det`, `inv`, `trace`, `linsolve`, `matrix_rank`, the symmetric eigensolver `eig_sym`, plus the
structured `inv_full` and `linsolve_full`. The short forms are exact where the inputs are exact;
`inv` of a scalar is a Real reciprocal, and `inv` of a matrix uses exact elimination.
`linsolve(a, b)` solves `a * x = b` and is valid under the side condition `det(a) != 0`;
`linsolve_full` returns that condition in a `MatrixSolveResult`.

`eig_sym(a)` takes a real symmetric square matrix and returns an `EigenResult` record: `values`
(ascending eigenvalues), `vectors` (`vectors[i][j]` is component `i` of eigenvector `j`), `sweeps`
(Jacobi sweeps used), `residual` (the largest `||A v_j - lambda_j v_j||` recomputed from the
original matrix) and `exact` (always `false`, because the rotations introduce square roots). It is a
RustLace-only builtin and it is core, so it works on a bare engine.

The solver is real-symmetric only: a non-square matrix, an asymmetric one, a complex entry or a 0x0
matrix is an `InvalidArgument` / `TypeMismatch` refusal naming the offending shape or entry. The
residual tracks the ambient computation precision, so a higher `setprecision` buys a smaller
residual.

`matrix_rank` uses exact rational elimination for matrices of exact numeric values, including rank-deficient and rectangular matrices. For matrices containing free symbols it retains the legacy determinant-based generic estimate, which is not an unconditional rank certificate and may require assumptions. Precision-limited numeric entries are refused rather than silently promoted to exact values. `trace` sums the diagonal entries.
Symbolic matrices take the same builtins; `det([[x, 1], [y, x]])` is `x^2 - y` symbolically.

### Calculus (10)

`diff`, `integrate`, `integrate_full`, `limit`, `limit_full`, `limit_left`, `limit_right`,
`series`, `jacobian`, and `hessian`. `diff` differentiates with respect to a symbolic variable;
`integrate` returns an antiderivative and self-verifies closed forms; `series` returns a power
series with an O-term; `jacobian` and `hessian` take a vector of expressions and a vector of
variables.

`limit` returns the same `LimitResult` record as `limit_full`; `limit_left`/`limit_right`
return the same record with one-sided values. A limit that does not exist is represented in the
record (status/exists), not necessarily as an exception.

### Solving (4)

`solve`, `solve_full`, `solve_system`, and `solve_system_full`. `solve(f, x [, domain])` solves
an equation and returns a `SolveResult` record: status, variable, domain, completeness, solution
list (each with conditions, multiplicity, exactness), parametric families, counts, and
diagnostics. The default domain is Complex; pass `real` for real solutions only. `solve_system`
uses Gröbner elimination and returns a `SystemSolveResult` with bindings.

`SolveStatus` values include `Solved`, `NoSolutions`, and `Unevaluated`; `Completeness` reports
whether the represented set is complete. An inconsistent real-domain equation such as
`solve(x^2 + 1 == 0, x, real)` is a `NoSolutions` result with a diagnostic, not an exception.
See [Values and types](values-and-types.md#record) for the record shape.

### Compilation (5)

`lower`, `compile`, `compile_full`, `evalir`, and `evalir_batch`. `lower(f, params)` produces
MathIR text; `compile` produces a validated serialized kernel; `compile_full` returns a
`CompilationResult` with parameter domains, result type, precision and optimization policy, target,
MathIR version, and exactness. `evalir` evaluates one point; `evalir_batch` evaluates flat
row-major values.

The IR header is `#!mathir 2`; the registry reports `"mathIrVersion": 2` and
`"symbolicFormatVersion": "#!lovelace-sym 1"`. Compilation is exactness-aware: a kernel records
its parameter domains and result domain rather than assuming floating point.

### DSP (16)

`fft`, `dft`, `conv`, `filter`, `movingavg`, `impulse`, `step`, `cosine`, `exponential`,
`powerseries`, `noise`, `delay`, `scale`, `re`, `im`, and `conj`. Signals are Vectors; transforms
and filters return Complex or Real vectors. The DSP family computes at the engine's precision once
`setprecision` has been used explicitly, and at a 30-place fast default before that. A complex
value has no literal; it enters the language through these builtins and is bridged back with
`re`, `im`, `conj`, and `abs`.

`fft` requires a power-of-two length; `dft` does not and accepts an optional output length.
`conv(x, h)` is linear convolution. `filter(a, b, x)` is the difference equation; when the third
argument is a count it produces the impulse response of that length (`filter([1], [1], 4)` is
`[1, 0, 0, 0]`). `noise` is deterministic for a given seed; its declared parameters are
`scale`, `disp`, `n`, `seed`, with the shorter call forms filling in the defaults. `cosine`,
`exponential`, and `powerseries` are sequence constructors.

### Introspection (5)

`type(x)` returns the value-kind name (or record/enum type name). `inspect(x)` returns an
`Inspection` record with type, domain, exactness, free symbols, node count, canonical and pretty
forms, shape, rank, element domain, assumptions, and members. `latex(expr)` renders a symbolic
expression as LaTeX source using the same printer decisions as the pretty form. `print(values…)`
writes display forms separated by spaces and returns `Void`; with a Symbolic first argument and a
mode string as the second argument it selects `"pretty"`, `"canonical"`, `"debug"`, or `"latex"`.
`capabilities()` returns a large `CapabilitiesResult` describing accepted solver domains and known
unsupported operation classes.

`inspect` is the reliable way to read whether a Real's stored digits were truncated; the display
form does not carry that flag. It is provenance, not an error bound. `capabilities()` is intentionally verbose on the wire; prefer reading its record fields
rather than parsing its prose.

### Language (1)

`plot` is the only Language-category builtin. It renders a 2-D line plot to an SVG file and
returns the absolute path as Text. See [Plots](plots.md) for the full contract.

### Optimization (2)

`optimize(f, params)` applies equivalence-preserving transformations for evaluation: common
subexpression elimination and policy-gated Hornerization. `optimize_full` returns an
`OptimizationResult` with the original and optimized expressions, estimated costs before and
after, shared subtrees, Horner rewrites, and the applied transformation list.

Optimization is a symbolic rewrite, not a numeric approximation; exactness is preserved. The
structured form records the target (`mathir`) and policy, so a caller can inspect what changed
before compiling the expression.

## Precision behaviour

The catalogue publishes numbers under the four claim kinds of the
[editorial guide](../editorial.md#the-four-kinds-of-claim); the per-family sections above state
which case a builtin is in.

| Claim kind | Where it appears in this catalogue |
|---|---|
| **Exact** | `Natural`/`Integer` arithmetic and rationals; integer predicates and `divrem`; array reductions over exact elements; a `Real` whose decimal expansion terminates or recurs within the computation budget, rendered with its period (`1/3` is `0.(3)`, never a rounding). |
| **Precision-limited** | `sqrt`, `pi`, `e`, `evalf`, `norm` and every irrational or transcendental result: the value is truncated at a decimal-place budget. The budget is not a proven final-error bound, especially for iterative, transcendental or cumulative algorithms. |
| **Error-bounded** | No entry in this catalogue publishes a final-error bound. A precision-limited value may happen to be close, but the digit budget alone does not establish a bound; a page that states one must name the bound and its evidence. |
| **Parity-verified** | This catalogue documents no live parity artifact. The historical `movingavg` defect is retained only as a historical record on the [DSP page](../extensions/dsp.md#historical-defect-fixed-movingavg-below-38-digits) and in ADR-004; its arithmetic was changed to the exact quotient rather than preserved for byte-parity. A parity value is evidence about the reference, not a mathematical result. |

- `setprecision(n)` sets the engine's computation and display precision. `pi(n)` and `e(n)` compute
  at most the current computation precision; requesting more is an `InvalidArgument` /
  `TypeMismatch` refusal naming the cap.
- `evalf(f, digits)` has its own 1000-place computation cap. A result that was cut at the cap is
  marked `truncated` with `truncationReason: digit-cap` and `budget: 1000` on the wire; a request
  the builtin can honour in full is not reduced.
- `format` and `format_typed` are renderings. The display does not carry the exactness bit: for a
  `Real`, ask `inspect` (its record reports exactness) rather than reading the printed form.
- The catalogue has no binary floating-point value type: every numeric value is decimal-backed.
  That is a statement about representation, not a correctness guarantee, and it does not make a
  precision-limited result exact.

## Errors

Builtins report failures as `LangError`, whose message, position, code and category are published
on the wire. The position is a UTF-16 code-unit offset into the caller's source. The seven
categories in `lovelace-lang/src/diagnostic.rs` are frozen: `ParseError`, `DomainError`,
`UnsupportedOperation`, `BudgetExceeded`, `NoSolution`, `TypeMismatch`,
`InternalInvariantFailure`.

| Failure | Code / category |
|---|---|
| Argument count outside the declared range | `InvalidArgument` / `TypeMismatch`, refused at one call site from the registry declaration before the implementation runs, with the declared signature on the envelope |
| Argument of the wrong kind | `InvalidArgument` / `TypeMismatch`, a sentence naming the parameter and the kind that arrived |
| Value outside the operation's domain | `InvalidOperation` / `DomainError` |
| A `pi`/`e` request above the engine computation precision | `InvalidArgument` / `TypeMismatch`, naming the cap |
| `eig_sym` of a non-square, asymmetric or 0x0 matrix, or a matrix with a complex entry | `InvalidArgument` / `TypeMismatch`, naming the offending shape or entry |
| A builtin used as a value (`sqrt` without a call) | `InvalidOperation` / `DomainError`, the undefined-variable failure |
| A name outside the registry | `UnknownFunction` / `UnsupportedOperation`, with suggestions, hints and the registry size on the error |

Three outcomes that look like errors are documented results:

- An inconsistent real-domain equation such as `solve(x^2 + 1 == 0, x, real)` is a `NoSolutions`
  `SolveResult` with a diagnostic, not an exception.
- A limit that does not exist is represented in the `LimitResult` (`status`/`exists`), not thrown.
- An integral with no closed form is returned as the unevaluated `integrate(...)` value, and the
  process exits 0.

The arity refusal is demonstrated in the Inputs section (`sqrt(1, 2)`); the shape refusals and the
non-error results are demonstrated in the Examples section.

## Limitations

- **No implicit symbolic variable.** `x` is an undefined variable until `x = symbol("x")`; an
  expression such as `diff(x^3, x)` fails without that binding.
- **`eig_sym` is real-symmetric only.** A general eigen-decomposition is not available: there is no
  `eigen`, SVD, QR, LU or Cholesky builtin in the catalogue.
- **`solve` accepts `real` and `complex` only.** `integer` and `rational` are refused with the
  accepted set named in the message; the solver's default domain is Complex.
- **`and`, `or` and `not` are symbolic-relation operators**, not two-valued Boolean connectives.
- **There is no complex literal.** A complex value enters through the DSP builtins and is bridged
  back with `re`, `im`, `conj` and `abs`.
- **Builtins are names, not values.** `sqrt` cannot be passed as an argument or stored; a user
  function with the same name shadows the builtin because user functions resolve first.
- **One static registry.** `Engine::new()` exposes all 134 builtins; the descriptor's `module`
  label groups a function for discovery and never gates it.
- **Two caps bound the numeric surface.** `evalf` stops at 1000 decimal places, and `pi`/`e` stop at
  the engine computation precision; requesting more is a refusal, never a silently clamped answer.
- **Array arithmetic is not a builtin.** `+ - * / % ^` are language operators with right-aligned
  broadcasting; the catalogue contributes construction, shape, reductions and manipulation.

## Examples

### Numerics examples

```lovelace
abs(-5)
```
```text
= 5 (Integer)
  _ = 5
```

```lovelace
divrem(17, 5)
```
```text
= DivRemResult(quotient: 3, remainder: 2) (DivRemResult)
  _ = DivRemResult(quotient: 3, remainder: 2)
```

```lovelace
sqrt(4)
```
```text
= 2 (Real)
  _ = 2
```

### Symbolics examples

Creating a symbolic variable is explicit; there is no implicit single-letter symbol:

```lovelace
x = symbol("x"); simplify(sin(x)^2 + cos(x)^2)
```
```text
= 1 (Symbolic)
  _ = 1
  x = x
```

```lovelace
x = symbol("x"); factor(x^4 - 5*x^2 + 4)
```
```text
= (x - 2)*(x - 1)*(x + 1)*(x + 2) (Symbolic)
  _ = (x - 2)*(x - 1)*(x + 1)*(x + 2)
  x = x
```

### Arrays examples

```lovelace
sum([[1, 2], [3, 4]])
```
```text
= 10 (Natural)
  _ = 10
```

```lovelace
shape(zeros(2, 3))
```
```text
= [2, 3] (Vector)
  _ = [2, 3]
```

```lovelace
matmul([[1, 2], [3, 4]], [[5, 6], [7, 8]])
```
```text
= [[19, 22], [43, 50]] (Array)
  _ = [[19, 22], [43, 50]]
```

### Linear Algebra examples

```lovelace
det([[1, 2], [3, 4]])
```
```text
= -2 (Integer)
  _ = -2
```

```lovelace
inv([[1, 2], [3, 4]])
```
```text
= [[-2, 1], [1.5, -0.5]] (Array)
  _ = [[-2, 1], [1.5, -0.5]]
```

```lovelace
setprecision(30); eig_sym([[2, 1], [1, 2]])
```
```text
= EigenResult(values: [1, 3], vectors: [[0.707106781186547524400844362104, 0.707106781186547524400844362104], [-0.707106781186547524400844362104, 0.707106781186547524400844362104]], sweeps: 1, residual: 0, exact: False) (EigenResult)
  _ = EigenResult(values: [1, 3], vectors: [[0.707106781186547524400844362104, 0.707106781186547524400844362104], [-0.707106781186547524400844362104, 0.707106781186547524400844362104]], sweeps: 1, residual: 0, exact: False)
```

```lovelace
setprecision(30); eig_sym([[4, 1], [1, 3]]).values
```
```text
= [2.381966011250105151795413165634, 4.618033988749894848204586834365] (Vector)
  _ = [2.381966011250105151795413165634, 4.618033988749894848204586834365]
```

### Calculus examples

```lovelace
x = symbol("x"); diff(x^3, x)
```
```text
= 3*x^2 (Symbolic)
  _ = 3*x^2
  x = x
```

```lovelace
x = symbol("x"); integrate(2*x*cos(x^2), x)
```
```text
= sin(x^2) (Symbolic)
  _ = sin(x^2)
  x = x
```

```lovelace
x = symbol("x"); series(sin(x)/x, x, 0, 6)
```
```text
= 1 - 1/6*x^2 + 1/120*x^4 + O(x^6) (Symbolic)
  _ = 1 - 1/6*x^2 + 1/120*x^4 + O(x^6)
  x = x
```

### Solving examples

```lovelace
x = symbol("x"); solve(x^2 - 4 == 0, x).status
```
```text
= Solved
  _ = Solved
  x = x
```

```lovelace
x = symbol("x"); solve(x^2 - 4 == 0, x).represented_count
```
```text
= 2 (Integer)
  _ = 2
  x = x
```

### Compilation examples

```lovelace
x = symbol("x"); evalir(compile(x^2 + 1, [x]), [3], 40)
```
```text
= 10 (Integer)
  _ = 10
  x = x
```

### DSP examples

```lovelace
fft([1, 0, 0, 0])
```
```text
= [1, 1, 1, 1] (Vector)
  _ = [1, 1, 1, 1]
```

```lovelace
abs(fft([0, 1, 0, 0]))
```
```text
= [1, 1, 1, 1] (Vector)
  _ = [1, 1, 1, 1]
```

### Introspection examples

```lovelace
type(42)
```
```text
= Natural
  _ = Natural
```

```lovelace
x = symbol("x"); y = symbol("y"); latex((x + 1)/y)
```
```text
= \frac{x + 1}{y}
  _ = \frac{x + 1}{y}
  x = x
  y = y
```

### Language examples

```lovelace
type(plot([1, 2, 3]))
```
```text
= Text
  _ = Text
```

### Optimization examples

```lovelace
x = symbol("x"); optimize(x^2 + x, [x])
```
```text
= x*(x + 1) (Symbolic)
  _ = x*(x + 1)
  x = x
```

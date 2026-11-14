# Symbolics (`Lovelace.Symbolics`)

**59 builtins.** The largest family in the engine — exact computer algebra, from `diff` to
Gröbner-basis system solving.

## What it is

Symbolics is the exact algebra family: expression construction (`symbol`, the four domain values),
calculus (`diff`, `integrate`, `series`, `limit`), rewriting (`simplify`, `expand`, `factor`,
`cancel`, `apart`, `collect`), equation solving (`solve`, `solve_system`), a few numeric bridges
(`evalf`, `subs`), optimization for evaluation (`optimize`), and introspection (`capabilities`,
`latex`).

Every value it produces is a `Value::Symbolic` holding a canonical expression from
[`lovelace-symbolics`](../../lovelace-symbolics/src/lib.rs). Structure is canonical, not textual:
`x*2 + 3*x` is `5*x`, and `x + 0` is `x`. There is no floating point anywhere in the path; the
numeric bridge `evalf` produces exact `Real` decimal expansions, not doubles.

It contributes **59** of the engine's 124 builtins. It is declared by the `plugin` field in
[`lovelace-lang/src/registry.rs`](../../lovelace-lang/src/registry.rs): 59
`plugin: Some("Lovelace.Symbolics")` tags, the first at line 69 and the last at line 1678.

One trap when counting: the family is **not** the `Symbolics` category. Its 59 builtins are spread
over six categories — 40 `Symbolics`, 10 `Calculus`, 4 `Solving`, 2 `Optimization`, 2
`Introspection` (`capabilities`, `latex`) and 1 `Numerics` (`evalf`). `diff` and `integrate` are
Calculus, `optimize` is Optimization, yet all are this family. The `plugin` field is the family key;
`category` is a browsing label. The `--functions` listing prints only `category`, so it cannot be
used to reconstruct family membership — the execution envelope and the registry source carry
`plugin`.

## How it is enabled

`lovelace-run` always builds `Engine::new()`, so all 59 names are live in every CLI run; there is no
CLI flag that disables a family. On the library side, `Engine::new()` leaves `enabled_plugins` as
`None`, which makes `Engine::plugin_enabled` answer `true` for every registered name
([`lovelace-lang/src/lib.rs`](../../lovelace-lang/src/lib.rs), lines 990-998).

`Engine::new_bare()` (`lib.rs:875`) sets `enabled_plugins = Some(empty)`, so the family disappears
and the call fails the way an unknown name fails. Observed from
[`bare_engine.rs`](../../lovelace-lang/examples/bare_engine.rs) (run it with `cargo run -p lovelace-lang --example bare_engine`):

```text
bare       symbol("x")              => [UnknownFunction] Unknown function 'symbol'.
bare+sym   symbol("x")              => x
bare+sym   diff(x^2, x)             => [InvalidOperation] Undefined variable 'x'.
```

That is `code = UnknownFunction`, `category = UnsupportedOperation` (`builtins.rs:4279`), raised at
the gate (`lib.rs:2146-2150`). It is the same failure an unknown name gives; there is no separate
"plugin not loaded" error. The third line is the important one: loading the family makes the
*builtins* callable, it does not bind any symbol — `x` is still undefined until the script assigns
it.

Re-enabling on a bare engine means loading a plugin whose `name()` is exactly `"Lovelace.Symbolics"`:

```rust
use lovelace_lang::modus::{ModusHost, ModusPlugin};
use lovelace_lang::{Engine, LangError};

struct EnableSymbolics;
impl ModusPlugin for EnableSymbolics {
    fn name(&self) -> &str { "Lovelace.Symbolics" }
    fn register(&mut self, _host: &mut ModusHost) -> Result<(), LangError> { Ok(()) }
}

let mut engine = Engine::new_bare();
assert!(engine.eval("symbol(\"x\")").is_err());
engine.load_plugin(&mut EnableSymbolics).unwrap();
assert_eq!(engine.eval("symbol(\"x\")").unwrap().display(), "x");
```

Because nothing in the engine "installs" the family, a **script** always needs the symbol bound
before use. `diff(x^2, x)` without a binding is not a symbolic operation, it is an undefined
variable:

```lovelace
diff(x^2, x)
```

```text
Error [InvalidOperation/DomainError]: Undefined variable 'x'.
```

The binding is `x = symbol("x")`, and it must come first in the same session.

## Every function it contributes

Arity is `min-max` accepted arguments. Parameters and descriptions are the registry's declared
metadata; every row was executed and its typed result is in the [verified-call appendix](#verified-calls).

| Builtin | Arity | Parameters | Returns | What it does |
| --- | --- | --- | --- | --- |
| `acos` | 1 | `x` | Symbolic | The principal arccosine. |
| `acosh` | 1 | `x` | Symbolic | The inverse hyperbolic cosine, principal branch (x >= 1). |
| `and` | 2 | `a`, `b` | Boolean \| Symbolic | Logical conjunction of two conditions. |
| `apart` | 2 | `f`, `x` | Symbolic | Partial fraction decomposition of a rational function in x. |
| `asin` | 1 | `x` | Symbolic | The principal arcsine. |
| `asinh` | 1 | `x` | Symbolic | The inverse hyperbolic sine, principal branch. |
| `assume` | 1 | `relation` | Symbolic | Assumes a relation (or conjunction of relations) with a constant bound for the session (the engine this plugin was loaded into), e.g. x > 5. |
| `assume_clear` | 0 | - | Text | Clears every active assumption. |
| `assume_integer` | 1 | `x` | Symbolic | Assumes x is an integer for the rest of the session. |
| `assume_negative` | 1 | `x` | Symbolic | Assumes x < 0 for the rest of the session. |
| `assume_nonnegative` | 1 | `x` | Symbolic | Assumes x >= 0 for the rest of the session. |
| `assume_positive` | 1 | `x` | Symbolic | Assumes x > 0 for the rest of the session. |
| `assume_real` | 1 | `x` | Symbolic | Assumes x is real-valued for the rest of the session. |
| `assumptions` | 0 | - | AssumptionSet | Lists the active assumptions as a structured AssumptionSet: the human rendering in display and the same atoms projected as structured conditions in assumptions. |
| `atan` | 1 | `x` | Symbolic | The principal arctangent. |
| `atanh` | 1 | `x` | Symbolic | The inverse hyperbolic tangent, principal branch (-1 < x < 1). |
| `cancel` | 1 | `f` | Symbolic | Cancels common polynomial factors in a rational function. The result is a bare value and CANNOT carry the side conditions the cancellation requires: removing a common factor g extends the domain (the input is undefined where g = 0), so the value holds only off that pole. Use cancel_full when the definedness delta matters. |
| `cancel_full` | 1 | `f` | CancelResult | Structured cancel: a CancelResult record with status (Exact/Conditional), the reduced expression, and the conditions it requires (the removed common factor must be nonzero — the same condition the simplify rewrite path attaches to rat.cancel-x-over-x). An Exact reduction reports no conditions at all. |
| `capabilities` | 0 | - | CapabilitiesResult | Reports which domain values the solver's domain argument accepts (solve_domains_accepted) and refuses (solve_domains_refused), and the known-unsupported operation classes, each with the exact error code and ErrorCategory member its live call produces. The domain pair is scoped to solve(f, x, domain): symbol(name, domain), assume_* and a SolutionFamily's parameter_domain accept every domain value. |
| `collect` | 2 | `f`, `x` | Symbolic | Collects coefficients of powers of x. |
| `complex` | 0 | - | Domain | The complex domain value (the solve default). |
| `cos` | 1 | `x` | Symbolic | The cosine function. |
| `cosh` | 1 | `x` | Symbolic | The hyperbolic cosine. |
| `diff` | 2 | `f`, `x` | Symbolic | Derivative of f with respect to x. |
| `evalf` | 2 | `f`, `digits` | Real \| Complex | Numerically evaluates a symbolic expression to the given number of decimal places. The count is honoured for an already-numeric argument too: such an argument is truncated to the requested count instead of being passed through at the ambient precision, so evalf(sqrt(2), 5) and evalf(1/3, 5) answer alike. The DECIMAL PLACES are COMPUTED, not merely printed: the evaluation runs at the requested count plus the integer digits of the result plus a guard, so a value with k integer digits keeps all N of its decimals (evalf(sinh(34/3), 30) answers 41780.548053564065925188446077567339), and the published count is bounded by the builtin's own 1000-place computation cap: a request above 1000 answers a value carrying at most 1000 decimal places (a request the builtin CAN honour in full is never reduced, and an exact answer stays exact), so the value that was cut says so on the wire — truncated: true, truncationReason: digit-cap, budget: 1000 — instead of crossing as if the count had been honoured. |
| `exp` | 1 | `x` | Symbolic | The exponential function, exp(x). |
| `expand` | 1 | `f` | Symbolic | Expands products and powers into a sum of monomials. |
| `factor` | 1 | `f` | Symbolic | Factors a polynomial over the rationals. |
| `hessian` | 2 | `f`, `vars` | Array | Hessian matrix (second derivatives) of f in the given variables. |
| `inf` | 0 | - | Symbolic | Positive infinity (the symbolic constant). |
| `integer` | 0 | - | Domain | The integer domain value (symbol(name, integer)). |
| `integrate` | 2 | `f`, `x` | Symbolic | Indefinite integral of f with respect to x; every closed form is differentiated and verified. |
| `integrate_full` | 2 | `f`, `x` | IntegrationResult | Structured integration: an IntegrationResult record with status (SolvedExact/SolvedConditional/Unevaluated), the antiderivative, its conditions, and the self-verification flag. |
| `jacobian` | 2 | `fs`, `vars` | Array | Jacobian matrix (first derivatives) of the function vector in the given variables. |
| `latex` | 1 | `expr` | Text | Renders an expression as LaTeX source. This is the printer's LaTeX mode, not a second printer: it shares the precedence table and the structural decisions of the pretty form, so the two can never describe different expressions. |
| `limit` | 3 | `f`, `x`, `x0` | LimitResult | Two-sided limit of f as x → x0, returned as the SAME LimitResult record limit_full returns: status, exists, the two-sided value and its conditions, the left/right one-sided values each with the side constraint it holds under, exactness and diagnostics. Nothing has to be parsed out of prose. |
| `limit_full` | 3 | `f`, `x`, `x0` | LimitResult | Structured limit: a LimitResult record with status, exists, the two-sided value and its conditions, the left/right one-sided values each with the side constraint they hold under, exactness, and diagnostics. |
| `limit_left` | 3 | `f`, `x`, `x0` | LimitResult | One-sided limit from the left as x approaches x0, returned as the SAME LimitResult record limit_full returns: status, exists, the one-sided value, exactness and diagnostics. |
| `limit_right` | 3 | `f`, `x`, `x0` | LimitResult | One-sided limit from the right as x approaches x0, returned as the SAME LimitResult record limit_full returns: status, exists, the one-sided value, exactness and diagnostics. |
| `log` | 1 | `x` | Symbolic | The natural logarithm (principal branch). |
| `not` | 1 | `a` | Boolean \| Symbolic | Logical negation of a condition. |
| `optimize` | 2 | `f`, `params` | Symbolic | Equivalence-preserving optimization for evaluation (CSE accounting, policy-gated Hornerization). Use optimize_full for the cost trace. |
| `optimize_full` | 2 | `f`, `params` | OptimizationResult | Structured optimization: an OptimizationResult with the original and optimized expressions, estimated costs before/after, and the applied transformations. |
| `or` | 2 | `a`, `b` | Boolean \| Symbolic | Logical disjunction of two conditions. |
| `rational` | 0 | - | Domain | The rational domain value (symbol(name, rational)). |
| `real` | 0 | - | Domain | The real domain value (solve(..., real), symbol(name, real)). |
| `series` | 4 | `f`, `x`, `x0`, `order` | Symbolic | Power series of f around x = x0 to the given order (with an O-term). |
| `simplify` | 1 | `f` | Symbolic | Safe simplification: applies only universal rules and rules whose side conditions are already provable from the active assumptions (conditions are never silently discarded). |
| `simplify_full` | 1 | `f` | TransformResult | Structured simplify: a TransformResult record with the rewritten expression, required side conditions, and the applied rule steps (stable rule ids and classifications). |
| `sin` | 1 | `x` | Symbolic | The sine function. |
| `sinh` | 1 | `x` | Symbolic | The hyperbolic sine. |
| `solve` | 2-3 | `f`, `x`, `domain` | SolveResult | Solves an equation for x and returns the SAME SolveResult record solve_full returns: status, domain (a Domain value), complete/completeness, per-solution conditions/multiplicity/exactness, parametric families and diagnostics. The default domain is Complex; pass real for real solutions only. |
| `solve_full` | 2-3 | `f`, `x`, `domain` | SolveResult | Structured solve: a SolveResult record with status, domain (a Domain value), complete flag, per-solution conditions/multiplicity/exactness, parametric families, and diagnostics. status is Solved only when the represented set is complete over the requested domain. |
| `solve_system` | 2 | `eqs`, `vars` | SystemSolveResult | Solves a polynomial equation system (Gröbner elimination into triangular form, verified solutions) and returns the SAME SystemSolveResult record solve_system_full does. |
| `solve_system_full` | 2 | `eqs`, `vars` | SystemSolveResult | Structured system solve: a SystemSolveResult with status, domain, complete, per-solution Binding records (name, value) and conditions. |
| `subs` | 3 | `f`, `x`, `value` | Symbolic | Substitutes value for the symbol x in f. |
| `symbol` | 1-2 | `name`, `domain` | Symbolic | Creates a symbolic variable; an optional domain (integer/rational/real/complex) is assumed for it. |
| `tan` | 1 | `x` | Symbolic | The tangent function. |
| `tanh` | 1 | `x` | Symbolic | The hyperbolic tangent. |

## Worked examples

Differentiation of a polynomial, the canonical form:

```lovelace
x = symbol("x")
diff(x^3 + 2*x^2 + 5*x + 7, x)
```

```text
= 3*x^2 + 4*x + 5 (Symbolic)
```

`solve` returns a structured `SolveResult`, not a bare list, so the domain, completeness and
per-solution exactness travel with the answer:

```lovelace
x = symbol("x")
solve(x^2 - 4 == 0, x)
```

```text
= SolveResult(status: Solved, variable: x, domain: complex, complete: True, completeness: Complete, solutions: [Solution(value: -2, conditions: [], multiplicity: 1, exactness: Exact), Solution(value: 2, conditions: [], multiplicity: 1, exactness: Exact)], families: [], common_conditions: [], represented_count: 2, unrepresented_count: 0, unrepresented_reason: , diagnostics: []) (SolveResult)
```

Universal identities simplify with no assumptions needed:

```lovelace
x = symbol("x")
simplify(sin(x)^2 + cos(x)^2)
```

```text
= 1 (Symbolic)
```

An assumption is engine-scoped state, and `simplify` uses it: `sqrt(x^2)` is `x` only because `x`
was declared positive.

```lovelace
x = symbol("x")
assume_positive(x)
simplify(sqrt(x^2))
```

```text
= x (Symbolic)
```

`series` carries an explicit order term:

```lovelace
x = symbol("x")
series(sin(x)/x, x, 0, 8)
```

```text
= 1 - 1/6*x^2 + 1/120*x^4 - 1/5040*x^6 + O(x^8) (Symbolic)
```

Not every integral has a closed form. `integrate` then returns the **unevaluated** integral as a
value and the process exits 0 — a documented pass-through, not an error:

```lovelace
x = symbol("x")
integrate(exp(x^2), x)
```

```text
= integrate(exp(x^2), x) (Symbolic)
```

The solver supports `real` and `complex` only. A refused domain is an error, and the message names
the accepted set:

```lovelace
x = symbol("x")
solve(x^2 + 1 == 0, x, integer)
```

```text
Error [InvalidOperation/DomainError]: solve(): currently supports domains real and complex; got integer.
```

Non-integer powers are not implemented, and the refusal is the kernel's own message:

```lovelace
2^(1/2)
```

```text
Error [UnsupportedOperation/UnsupportedOperation]: Non-integer exponents are not yet supported.
```

## Precision, assumptions and limits

- **Symbols must be bound.** There is no implicit symbolic variable. `x` is an undefined variable
  until `x = symbol("x")` (or `symbol("x", real)` to carry a domain). This is the single most common
  way a Symbolics example fails.
- **Assumptions are session state, not a parameter.** `assume_*` / `assume` store into the engine;
  `assume_clear()` empties the set; `assumptions()` reads it back as an `AssumptionSet`. Nothing is
  carried across engines, and the engine does not reject contradictory relations: after
  `assume(x > 5)`, evaluating `x < 3` simply answers `x < 3` (observed).
- **`evalf` has a 1000-place computation cap and a 100-place display default.** A request above the
  cap is truncated and says so in the structured payload: for `evalf(pi, 5000)` the observed record
  carries `truncated: true`, `truncationReason: "digit-cap"`, `budget: 1000`, `exact: false`, a
  `value` holding 1000 digits, and a `display`/`typed` holding the 100-digit rendering. The display
  bound is separate from the computation bound and is 100 until `setprecision` moves it.
- **Domains.** `solve` accepts `real` and `complex`; `integer` and `rational` are refused with the
  message quoted above. `capabilities()` publishes exactly that split
  (`solve_domains_accepted: [real, complex]`, `solve_domains_refused: [integer, rational]`).
- **Exactness is reported, not assumed.** `solve`/`limit`/`integrate_full`/`cancel_full`/
  `simplify_full` return records whose fields say whether the answer is `Exact` or `BestEffort`, and
  which conditions it holds under. `cancel(f)` is the bare-value form and explicitly cannot carry
  those conditions — use `cancel_full` when the domain delta matters.
- **`capabilities()` is the machine-readable limit list.** It returns a `CapabilitiesResult` naming
  every known-unsupported operation class with the exact code, category and message its live call
  produces (non-integer powers, unsupported solve domains, complex algebraic `RootOf`,
  unevaluated integration diagnostics, symbolic `plot` arguments, non-symbolic variables, and more).
- **Rewrite budget.** A transformation applies at most 400 rules (`REWRITE_MAX_STEPS`,
  `builtins.rs:12013`); a rewrite that would exceed it stops rather than looping.
- **Solver working precision.** Constant folding and candidate verification in the solver run at 40
  decimal places (`builtins.rs:7989`), independent of the display precision.

## Where the code lives

| Path | What is there |
| --- | --- |
| `lovelace-lang/src/registry.rs` | the 59 descriptors; plugin tags at lines 69-1678 |
| `lovelace-lang/src/builtins.rs` | dispatch arms and the symbolic builtin implementations |
| `lovelace-lang/src/lib.rs` | `Engine` assumption state, the plugin gate (990), `load_plugin` (883) |
| `lovelace-lang/src/modus.rs` | the typed payload channel a host plugin registers through |
| `lovelace-symbolics/src/lib.rs` | canonical `Expr`, the algebra |
| `lovelace-symbolics/src/assumptions.rs` | the tri-state assumption kernel |
| `lovelace-symbolics/src/{polynomials,groebner,rational_functions,series_kink,symbolic_matrix}.rs` | the algorithms behind `factor`/`solve_system`/`apart`/`series`/`linsolve` |
| `lovelace-symbolics/src/rewrite_registry.rs` | stable rewrite rule ids used by `simplify_full` |

Tests that cover it (`cargo test -p lovelace-lang`, modules that drive the symbolic surface):

| Test module | Tests |
| --- | --- |
| `symbolics_tail_round34` | 66 |
| `suite_engine_batch` | 42 |
| `round28_batch` | 22 |
| `assumptions_round35` | 17 |
| `round90_symbolic_families` | 12 |
| `round97_tail` | 12 |
| `symbolics_round23` | 8 |
| `round92_symbolic_families` | 6 |
| `round67_modus` | 22 |

The `lovelace-symbolics` crate carries its own batch modules (`round67_polynomial_algebra`,
`round90_symbolic_families`, `round92_symbolic_families`, `tail_round34`, `assumptions`,
`series_kink`, `groebner`) which are the unit-level layer beneath these.

## What is NOT in this extension

- **No linear algebra.** `det`, `eig_sym`, `inv`, `inv_full`, `linsolve`, `linsolve_full`,
  `matrix_rank`, `trace` and `matmul` are all **core** (`plugin: None`), even though `linsolve` is
  the symbolic Bareiss solver. They are callable on a bare engine and are not part of the 59.
- **No general eigen-decomposition.** `eig_sym` is the core, real-symmetric-only eigensolver; there
  is no `eigen`, `eigenvalue`, `eigenvector` or general decomposition builtin anywhere in the
  registry — and the C# Studio it descends from had none either. Do not expect SVD/QR/LU/cholesky
  either.
- **No differential equations.** No `dsolve`, no `ode`, no Laplace or Fourier *transform* builtins
  (`fft`/`dft` are discrete numeric transforms in DSP, not symbolic ones).
- **No symbolic summation or products.** No `summation`, `product`, `telescope`.
- **No non-integer powers.** `2^(1/2)`, `4^(1/2)`, `0.5^(1/2)` are all refused with
  `Non-integer exponents are not yet supported.` (`capabilities()` documents the exact scope: a
  negative base under the principal square root *is* supported, e.g. `sqrt(-4)` → `2*i`).
- **No numeric root-finding or quadrature.** `evalf` evaluates closed forms; there is no Newton
  iteration or adaptive integration entry point.
- **`inspect`, `type`, `print` are core** Introspection builtins, not Symbolics — only `capabilities`
  and `latex` belong to the family.
- **`plot` is core `Language`**, and `capabilities()` advertises it as refusing symbolic
  arguments (`plot.symbolic-expression`); this family draws nothing.

## Verified calls

Every builtin in the table above was executed through the built CLI. Each row is the exact one-line
call (statements joined with `;`) and the typed result observed. Results longer than 150 characters
are marked `...`; the worked examples above show full records.

| Builtin | Exact call | Observed typed result |
| --- | --- | --- |
| `acos` | `x = symbol("x"); acos(x)` | = acos(x) (Symbolic) |
| `acosh` | `acosh(1)` | = 0 (Symbolic) |
| `and` | `x = symbol("x"); assume(x > 0); and(x > 0, x < 1)` | = x < 1 and x > 0 (Symbolic) |
| `apart` | `x = symbol("x"); apart(1/(x^2 - 1), x)` | = -1/2/(x + 1) + 1/2/(x - 1) (Symbolic) |
| `asin` | `x = symbol("x"); asin(x)` | = asin(x) (Symbolic) |
| `asinh` | `asinh(0)` | = 0 (Symbolic) |
| `assume` | `x = symbol("x"); assume(x > 5)` | = x > 5 (Symbolic) |
| `assume_clear` | `assume_clear()` | = assumptions cleared |
| `assume_integer` | `x = symbol("x"); assume_integer(x)` | = x (Symbolic) |
| `assume_negative` | `x = symbol("x"); assume_negative(x)` | = x (Symbolic) |
| `assume_nonnegative` | `x = symbol("x"); assume_nonnegative(x)` | = x (Symbolic) |
| `assume_positive` | `x = symbol("x"); assume_positive(x)` | = x (Symbolic) |
| `assume_real` | `x = symbol("x"); assume_real(x)` | = x (Symbolic) |
| `assumptions` | `assumptions()` | = AssumptionSet(display: , assumptions: []) (AssumptionSet) |
| `atan` | `x = symbol("x"); atan(x)` | = atan(x) (Symbolic) |
| `atanh` | `atanh(0)` | = 0 (Symbolic) |
| `cancel` | `x = symbol("x"); cancel((x^2 - 1)/(x - 1))` | = x + 1 (Symbolic) |
| `cancel_full` | `x = symbol("x"); cancel_full(x/x)` | = CancelResult(status: Conditional, original: x/x, expression: 1, changed: True, conditions: [x != 0]) (CancelResult) |
| `capabilities` | `capabilities()` | = CapabilitiesResult(solve_domains_accepted: [real, complex], solve_domains_refused: [integer, rational], unsupported_operations: [UnsupportedCapabili... |
| `collect` | `x = symbol("x"); y = symbol("y"); collect(x*y + x + y + 1, x)` | = 1 + y + x*(y + 1) (Symbolic) |
| `complex` | `x = symbol("x"); solve(x^2 + 1 == 0, x, complex)` | = SolveResult(status: Solved, variable: x, domain: complex, complete: True, completeness: Complete, solutions: [Solution(value: i, conditions: [], mul... |
| `cos` | `x = symbol("x"); cos(x)` | = cos(x) (Symbolic) |
| `cosh` | `x = symbol("x"); cosh(x)` | = cosh(x) (Symbolic) |
| `diff` | `x = symbol("x"); diff(x^3 + 2*x^2 + 5*x + 7, x)` | = 3*x^2 + 4*x + 5 (Symbolic) |
| `evalf` | `evalf(sqrt(2), 30)` | = 1.414213562373095048801688724209 (Real) |
| `exp` | `x = symbol("x"); exp(x)` | = exp(x) (Symbolic) |
| `expand` | `x = symbol("x"); expand((x+1)^3)` | = x^3 + 3*x^2 + 3*x + 1 (Symbolic) |
| `factor` | `x = symbol("x"); factor(x^4 - 5*x^2 + 4)` | = (x - 2)*(x - 1)*(x + 1)*(x + 2) (Symbolic) |
| `hessian` | `x = symbol("x"); y = symbol("y"); hessian(x*y, [x, y])` | = [[0, 1], [1, 0]] (Array) |
| `inf` | `inf` | = inf (Symbolic) |
| `integer` | `symbol("n", integer)` | = n (Symbolic) |
| `integrate` | `x = symbol("x"); integrate(2*x*cos(x^2), x)` | = sin(x^2) (Symbolic) |
| `integrate_full` | `x = symbol("x"); integrate_full(x^2, x)` | = IntegrationResult(status: SolvedExact, expression: 1/3*x^3, conditions: [], verified: True, method: table, verification_method: differentiation+expa... |
| `jacobian` | `x = symbol("x"); y = symbol("y"); jacobian([x*y, x+y], [x, y])` | = [[y, x], [1, 1]] (Array) |
| `latex` | `x = symbol("x"); y = symbol("y"); latex((x + 1)/y)` | = \frac{x + 1}{y} |
| `limit` | `x = symbol("x"); limit(sin(x)/x, x, 0)` | = LimitResult(status: Value, exists: True, value: 1, left: , left_conditions: [], right: , right_conditions: [], conditions: [], exactness: Exact, dia... |
| `limit_full` | `x = symbol("x"); limit_full(1/x, x, 0)` | = LimitResult(status: DoesNotExist, exists: False, value: , left: -inf, left_conditions: [x < 0], right: inf, right_conditions: [x > 0], conditions: [... |
| `limit_left` | `x = symbol("x"); limit_left(1/x, x, 0)` | = LimitResult(status: MinusInfinity, exists: True, value: -inf, left: , left_conditions: [], right: , right_conditions: [], conditions: [], exactness:... |
| `limit_right` | `x = symbol("x"); limit_right(1/x, x, 0)` | = LimitResult(status: PlusInfinity, exists: True, value: inf, left: , left_conditions: [], right: , right_conditions: [], conditions: [], exactness: E... |
| `log` | `x = symbol("x"); log(x)` | = log(x) (Symbolic) |
| `not` | `x = symbol("x"); not(x > 0)` | = not x > 0 (Symbolic) |
| `optimize` | `x = symbol("x"); optimize(x^5 + 2*x^4 + 3*x^3 + x^2 + x + 1, [x])` | = 1 + x*(1 + x*(1 + x*(3 + x*(x + 2)))) (Symbolic) |
| `optimize_full` | `x = symbol("x"); optimize_full(x^5 + 2*x^4 + 3*x^3 + x^2 + x + 1, [x])` | = OptimizationResult(original: x^5 + 2*x^4 + 3*x^3 + x^2 + x + 1, optimized: 1 + x*(1 + x*(1 + x*(3 + x*(x + 2)))), estimated_cost_before: 19, estimat... |
| `or` | `x = symbol("x"); or(x > 0, x < -1)` | = (x < -1 or x > 0) (Symbolic) |
| `rational` | `symbol("q", rational)` | = q (Symbolic) |
| `real` | `x = symbol("x"); solve(x^2 + 1 == 0, x, real)` | = SolveResult(status: NoSolutions, variable: x, domain: real, complete: True, completeness: Complete, solutions: [], families: [], common_conditions: ... |
| `series` | `x = symbol("x"); series(sin(x)/x, x, 0, 8)` | = 1 - 1/6*x^2 + 1/120*x^4 - 1/5040*x^6 + O(x^8) (Symbolic) |
| `simplify` | `x = symbol("x"); simplify(sin(x)^2 + cos(x)^2)` | = 1 (Symbolic) |
| `simplify_full` | `x = symbol("x"); simplify_full(x/x)` | = TransformResult(status: Satisfied, original: x/x, expression: 1, changed: True, conditions: [x != 0], steps: [RewriteStep(rule_id: rat.cancel-x-over... |
| `sin` | `x = symbol("x"); sin(x)` | = sin(x) (Symbolic) |
| `sinh` | `x = symbol("x"); sinh(x)` | = sinh(x) (Symbolic) |
| `solve` | `x = symbol("x"); solve(x^2 - 4 == 0, x)` | = SolveResult(status: Solved, variable: x, domain: complex, complete: True, completeness: Complete, solutions: [Solution(value: -2, conditions: [], mu... |
| `solve_full` | `x = symbol("x"); solve_full(x^2 - 4 == 0, x)` | = SolveResult(status: Solved, variable: x, domain: complex, complete: True, completeness: Complete, solutions: [Solution(value: -2, conditions: [], mu... |
| `solve_system` | `x = symbol("x"); y = symbol("y"); solve_system([x^2 + y^2 - 1 == 0, x*y == 0], [x, y])` | = SystemSolveResult(status: Solved, domain: complex, complete: True, completeness: Complete, solutions: [SystemSolution(bindings: [Binding(name: x, va... |
| `solve_system_full` | `x = symbol("x"); y = symbol("y"); solve_system_full([x^2 + y^2 - 1 == 0, x*y == 0], [x, y])` | = SystemSolveResult(status: Solved, domain: complex, complete: True, completeness: Complete, solutions: [SystemSolution(bindings: [Binding(name: x, va... |
| `subs` | `x = symbol("x"); subs(x^2 + 1, x, 3)` | = 10 (Symbolic) |
| `symbol` | `symbol("x")` | = x (Symbolic) |
| `tan` | `x = symbol("x"); tan(x)` | = tan(x) (Symbolic) |
| `tanh` | `x = symbol("x"); tanh(x)` | = tanh(x) (Symbolic) |

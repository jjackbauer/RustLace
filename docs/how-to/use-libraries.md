# Work with Lovelace libraries

RustLace includes libraries for arrays, linear algebra, symbolic algebra,
signal processing, compilation, and queueing models. Their functions are
**built into the language**: you call them by name and do not import a
module, install an extension, or load a Rust crate.

This guide helps you choose an operation, understand the kinds it accepts and
returns, and find the authoritative reference. It is a task-oriented companion
to the [complete builtin catalogue](../language/builtins.md), not an exhaustive
function list.

**Output convention:** the examples below show `--text --omit-variables`,
which displays only the result (or refusal). To reproduce one from the
repository root, put its Lovelace source in a `.ls` file and run
`cargo run -p lovelace-run -- --file example.ls --text --omit-variables`.
Plain `--text` additionally prints variables in scope.

**Before you start:** finish the [first-run guide](../start-here.md) if
`Natural`, `Real`, `Vector`, and `Array` are unfamiliar. All commands and
outputs here refer to the checked-out revision; see
[version applicability](../version-applicability.md).

## Choose a library by task

| Your task | Library family / functions | Input and output kinds | Detailed reference |
|---|---|---|---|
| Build or reduce a multidimensional dataset | Core: `zeros`, `shape`, `sum` | [Array / Vector](../language/values-and-types.md#vector-and-array) and [numeric kinds](../language/values-and-types.md#numeric-kinds-and-widening) | [Arrays](../language/builtins.md#arrays-23) |
| Multiply matrices, solve equations, check rank | Core: `matmul`, `det`, `inv`, `matrix_rank` | [Array](../language/values-and-types.md#vector-and-array) and numeric scalar | [Linear algebra](../language/builtins.md#linear-algebra-8) |
| Rearrange symbolic expressions or differentiate | Symbolics: `symbol`, `factor`, `diff`, `subs` | [Symbolic](../language/values-and-types.md#symbolic) | [Symbolics library](../extensions/symbolics.md) |
| Transform or filter a sampled signal | DSP: `fft`, `movingavg`, `convolve` | [Vector](../language/values-and-types.md#vector-and-array) of [Real](../language/values-and-types.md#numeric-kinds-and-widening) or [Complex](../language/values-and-types.md#complex) | [DSP library](../extensions/dsp.md) |
| Evaluate a compiled expression repeatedly | MathIR: `compile`, `evalir`, `evalir_batch` | [Symbolic](../language/values-and-types.md#symbolic), [Text](../language/values-and-types.md#text), [Vector](../language/values-and-types.md#vector-and-array) | [MathIR library](../extensions/mathir.md) |
| Estimate utilization, waiting, or throughput | Systems: `sys_mm1`, `sys_station`, `sys_analyze` | Numeric values and [Record](../language/values-and-types.md#record) results | [Systems library](../extensions/systems.md) |

These are **language-level families**. The `lovelace-*` names in the
[workspace crate inventory](../README.md#per-crate-pages) describe Rust
implementation units, not namespaces that a Lovelace program imports.

## Arrays: distinguish a vector from a matrix

**Task:** compute the sum of a 2-by-2 dataset.

```lovelace
sum([[1, 2], [3, 4]])
```
```text
= 10 (Natural)
```

The input is an [Array](../language/values-and-types.md#vector-and-array)
with shape `[2, 2]`. The result is a
[Natural](../language/values-and-types.md#numeric-kinds-and-widening):
the reduction consumes the collection and produces a scalar. This is different
from a one-dimensional [Vector](../language/values-and-types.md#vector-and-array)
such as `[1, 2, 3, 4]`, even when both contain four numbers.

For constructing arrays, inspecting shapes and understanding broadcasting, see
[Arrays](../language/builtins.md#arrays-23) and
[operators on arrays](../language/expressions-and-operators.md#arrays-and-broadcasting).

## Linear algebra: inspect shape and verify the answer

**Task:** multiply two two-dimensional arrays.

```lovelace
matmul([[1, 2], [3, 4]], [[5, 6], [7, 8]])
```
```text
= [[19, 22], [43, 50]] (Array)
```

Both inputs and the result have kind
[Array](../language/values-and-types.md#vector-and-array).
`matmul` is matrix multiplication, **not** elementwise multiplication.
For a system `A*x=b`, represent `b` as a column array (`[[5],[6]]`),
not a flat vector (`[5,6]`). The
[linear algebra tutorial](../tutorials/linear-algebra.md) demonstrates solving
and recomputing the residual. For rank, a
[Symbolic](../language/values-and-types.md#symbolic) matrix may have a
parameter-dependent rank; read the
[rank reference](../language/builtins.md#linear-algebra-8)
before treating a generic symbolic result as an unconditional certificate.

## Symbolics: an expression is not yet a numerical value

**Task:** factor a polynomial in a symbolic variable.

```lovelace
x = symbol("x"); factor(x^4 - 5*x^2 + 4)
```
```text
= (x - 2)*(x - 1)*(x + 1)*(x + 2) (Symbolic)
```

`symbol("x")` creates a [Symbolic](../language/values-and-types.md#symbolic)
value. Merely writing the name `x` without assigning it does not create a
symbol. `factor` returns a **symbolic expression** rather than a vector of
roots. If you want to evaluate or differentiate the expression, consult
[Symbolics](../extensions/symbolics.md) and
[the guided differentiation tutorial](../tutorials/symbolic.md).
Algebraic cancellation may require domain conditions: a cancellation valid
for `x != 1` need not be valid at `x = 1`.

## DSP: input samples can produce complex output

**Task:** transform a four-sample impulse with the FFT.

```lovelace
fft([0, 1, 0, 0])
```
```text
= [1, -1i, -1, 1i] (Vector)
```

The input is a [Vector](../language/values-and-types.md#vector-and-array)
of numeric samples; the output is still a Vector, but some elements are
[Complex](../language/values-and-types.md#complex).
The displayed Gaussian-integer coefficients for this four-point case are
exact. Longer transforms may involve irrational coefficients and
precision-limited results. Check the power-of-two input requirement and
precision policy in the [DSP reference](../extensions/dsp.md), then follow the
[signal-processing tutorial](../tutorials/signal-processing.md).

## MathIR: compile expressions for repeated evaluation

**Task:** compile a symbolic formula and evaluate the resulting kernel.

MathIR accepts a [Symbolic expression](../language/values-and-types.md#symbolic)
and a [Vector](../language/values-and-types.md#vector-and-array) of formal
parameters. `compile` produces versioned IR text
([Text](../language/values-and-types.md#text));
`compile_full` adds a structured
[Record](../language/values-and-types.md#record) describing the program,
parameters and precision policy. `evalir` evaluates it against a vector of
inputs at an explicit precision budget.

Start with the fully executable
[MathIR examples](../extensions/mathir.md#examples): they show the kernel,
evaluation, parameter mismatch errors and version incompatibility. In
particular, a compiled kernel is **not** the same type as a callable Lovelace
user function; do not assume arbitrary function closure semantics.

## Systems: read both projected and exact values

**Task:** estimate utilization of an M/M/1 station.

```lovelace
sys_mm1(8, 10).utilization
```
```text
= 0.8 (Real)
```

Here `8` and `10` are arrival and service rates. `sys_mm1` returns a
[Record](../language/values-and-types.md#record), which supports
[member access](../language/expressions-and-operators.md#member-access).
`.utilization` extracts its [Real](../language/values-and-types.md#numeric-kinds-and-widening)
decimal projection; the underlying exact ratio is `4/5`.
The current `Systems` API also publishes
`exact_values` [rational records](../extensions/systems.md#precision-behaviour)
for consumers who need numerator and denominator.

For more realistic workloads, model a network with
[`sys_station` and `sys_open`](../extensions/systems.md#per-builtin-reference),
then analyze it with `sys_analyze`.
The [systems analysis guide](../systems-analysis.md) explains units, assumptions,
capacity bounds and unsupported models. Older releases may not have the
`exact_values` sidecar; check [version applicability](../version-applicability.md).

## Inspect a function instead of guessing

The [full builtin catalogue](../language/builtins.md) is the readable index;
the CLI exposes the same live registry used by the editor:

```console
cargo run -p lovelace-run -- --functions --function-limit 500
cargo run -p lovelace-run -- --function matmul
```

The default listing limit is 50. An automated consumer should inspect
`truncated`, `returned`, and `total` rather than assuming the first page
is the complete inventory. `--function` supplies arity, parameter names,
module and declared return kinds. It does not guarantee mathematical
correctness or fully specify every domain edge case.

To understand what a displayed number proves, finish with
[Verify a numerical result](verify-a-result.md).
For automation, read [Agent usage](../agent-usage.md) and the
[JSON error contract](../language/errors-and-diagnostics.md).

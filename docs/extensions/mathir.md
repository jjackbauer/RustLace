# Compilation / MathIR (`Lovelace.MathIR`)

**5 builtins.** The bridge from a symbolic expression to a typed, validated computational DAG and
back out at a caller-chosen precision.

## Purpose

### What it is

MathIR is the intermediate representation the engine lowers symbolic expressions into. `lower`
produces the text form of the DAG, `compile` produces it through the validating compiler, `compile_full`
returns the same kernel as a structured `CompilationResult` record, and `evalir` / `evalir_batch`
evaluate a kernel at a point or over a flat batch at a precision the caller supplies.

The IR is a small typed SSA program in a versioned line format. Its first line is the format header
(`#!mathir 2`), followed by the parameter declarations, a constant pool, and one line per node
referencing earlier nodes by index. The op set is 35 kinds — `Constant`, `Parameter`, the arithmetic
`Add`/`Sub`/`Mul`/`Div`/`Negate`/`Reciprocal`, powers and roots `PowInt`/`Pow`/`Sqrt`, the
elementary functions `Exp`/`Log`/`Sin`/`Cos`/`Tan`/`Asin`/`Acos`/`Atan`/`Sinh`/`Cosh`/`Tanh`, the
scalar `Abs`/`Sign`/`Floor`/`Ceil`, `Min`/`Max`, the six comparisons and `Select`
([`lovelace-lang/src/mathir.rs`](../../lovelace-lang/src/mathir.rs), enum at lines 39-110).

It contributes **5** of the engine's 134 builtins, all with category `Compilation`, and is declared
by 5 `module: "MathIR"` descriptors in
[`lovelace-lang/src/registry.rs`](../../lovelace-lang/src/registry.rs). Here
`category` and family happen to coincide — that is a coincidence, not the rule: the Symbolics family
spans six categories.

The family is exactly the compile/evaluate loop. It contributes no expression construction, no
rewriting and no optimization: `compile` lowers what it is given, and the record says so
(`optimization_policy: none`).

### Where the code lives

| Path | What is there |
| --- | --- |
| `lovelace-lang/src/registry.rs` | the 5 descriptors, `module: "MathIR"` |
| `lovelace-lang/src/mathir.rs` | `IrOpKind` (39-110), `IrProgram`, serialization (335), the version gate (375), validation |
| `lovelace-lang/src/builtins.rs` | the `compile`/`compile_full`/`lower`/`evalir`/`evalir_batch` dispatch arms |
| `lovelace-lang/src/numeric.rs` | the exact numeric backend the evaluator calls |

Tests that cover it:

| Test module | Tests |
| --- | --- |
| `mathir_round54` | 13 |
| `round97_tail` | 12 (incl. the 134-builtin registry count) |
| `mathir_typing_round55` | 7 |
| `round84_evalir_multi` | part of the `lovelace-lang` batches |

`round97_tail` is where the published count is pinned: one static registry publishes 134 builtins,
which is `44 core + 59 Symbolics + 5 MathIR + 16 DSP`.

## Inputs

### How it is available

`Engine::new()` exposes every registered builtin, so all five names are live in every engine,
`lovelace-run` invocation and desktop session; there is no flag that removes a family and no
enabled-family set.

One session mixes families freely because every family is compiled in: compiling a symbolic
expression needs both `MathIR` and `Symbolics`, and `Engine::new()` has both.

## Outputs

### Every function it contributes

Arity is `min-max` accepted arguments. Parameters and descriptions are the registry's declared
metadata; every row was executed and its typed result is in the [verified-call appendix](#verified-calls).

| Builtin | Arity | Parameters | Returns | What it does |
| --- | --- | --- | --- | --- |
| `compile` | 2 | `f`, `params` | Text | Compiles a symbolic expression to a validated MathIR kernel (serialized IR text). Use compile_full for the structured kernel metadata. |
| `compile_full` | 2 | `f`, `params` | CompilationResult | Structured compile: a CompilationResult record with the IR, per-parameter name and domain, result domain and type, the precision and optimization policies, target, MathIR version, and exactness. |
| `evalir` | 3 | `ir`, `values`, `digits` | Real \| Complex | Evaluates a compiled kernel (IR text or CompilationResult) at one point at the given precision. |
| `evalir_batch` | 3 | `ir`, `values`, `digits` | Vector | Batch-evaluates a compiled kernel (IR text or CompilationResult) over flat row-major values at the given precision. |
| `lower` | 2 | `f`, `params` | Text | Lowers a symbolic expression to MathIR (the typed, validated computational DAG). |

## Precision behaviour

### Precision, limits and the version

- **Format version 2.** `mathir::FORMAT_VERSION` is `2` (`lovelace-lang/src/mathir.rs`), the header is `#!mathir 2`
  (`IrProgram::serialize`), and any other version is refused with
  `Unsupported MathIR format version '<v>' (expected 2).` (`IrProgram::deserialize`). The wire envelope
  advertises the same number as `mathIrVersion: 2`. There is no migration path — the gate is
  deliberately strict.
- **Precision is the caller's.** The kernel stores no precision policy; the record says
  `precision_policy: caller_supplied`, and `evalir`'s third argument is the working precision.
  `digits` accepts 1 to 2147483647; `0` is refused with
  `evalir(): argument 3 (digits) must be a Natural or Integer between 1 and 2147483647; got 0.`
- **Exact, not floating point.** The constant pool holds rationals (`const (rat 1 1)`), the evaluator
  runs on the exact numeric kernels, and `exact: True` is reported. `evalir` returns
  `Real | Complex` when the program needs them and an exact scalar otherwise (`10 (Integer)` above).
- **35 op kinds, and they are closed.** No `Mod`, no `Atan2`, no reductions, no complex-specific
  nodes. If an expression needs an operation outside the set, lowering fails rather than inventing a
  node.
- **Validation happens before evaluation.** Every node's pool and operand indices are checked:
  a hand-written `#!mathir 2` program with a dangling constant index answers
  `IR validation: Constant node 0 references out-of-range pool index 0.` (observed).
- **The parameter list must be symbols and must cover the expression.** `compile`/`compile_full`
  require at least one parameter; a free symbol missing from the list fails with
  `Node kind Symbol cannot be lowered to MathIR.` (observed for `compile(x^2 + 1, [y])` and for
  `lower(x^2 + 1, [])`). `lower` differs from `compile` in one respect: it accepts an empty parameter
  list when the expression is a constant (`lower(1, [])` answers a two-line program).
- **No optimization.** `compile` emits the lowered DAG as-is (`optimization_policy: none`). Horner
  form and CSE are a separate Symbolics transform (`optimize`), not part of compilation.

## Errors

The surface reports failures through `LangError`, and the parsing and evaluation failures keep the
codes the rest of the engine uses.

| Failure | Code / category |
|---|---|
| Wrong argument count at the call site | `InvalidArgument` / `TypeMismatch`, with the declared signature |
| `compile`/`compile_full` with an empty parameter list | `InvalidArgument` / `TypeMismatch`, message `Compilation requires at least one parameter. (Parameter 'parameters')` |
| An IR text whose header is not `#!mathir 2` | `InvalidInput` / `ParseError`, message `Unsupported MathIR format version '<v>' (expected 2).` |
| A malformed IR line, or text that is not IR | `InvalidInput` / `ParseError`, message `Malformed IR line: <line>` |
| A kernel whose node or constant indices do not validate | `InvalidOperation` / `DomainError`, message `IR validation: ...` |
| A value list whose length does not match the kernel | `InvalidOperation` / `DomainError`, message `evalir(): kernel expects <n> parameters; got <m> values.` |
| A name from an unloaded family | `UnknownFunction` / `UnsupportedOperation`, the ordinary unknown-name failure |

A refused `digits` value (`evalir(..., 0)`) is an `InvalidArgument` / `TypeMismatch` argument
refusal, and an expression that cannot be lowered is a `LangError` naming the node kind rather than
a panic. The worked examples below show the version, arity and empty-parameter messages verbatim.

## Limitations

### What is NOT in this extension

- **No JIT, no native or GPU code generation.** The only target the record advertises is
  `target: mathir`; `evalir` is an interpreter over the DAG, and nothing emits machine code or
  source in another language.
- **No optimizer.** `optimization_policy: none`. There is no constant folding pass beyond the
  lowering, no loop restructuring, no CSE — `optimize` belongs to Symbolics and runs on expressions,
  not on IR.
- **No symbolic work.** This family does not build, rewrite or solve expressions; it needs Symbolics
  (or a host-supplied expression) to have produced one first.
- **No persistence or caching builtin.** The IR is text returned as a `Text` value; storing it,
  keying it and invalidating it are the host's job. There is no binary format.
- **No automatic differentiation, no adjoints, no gradients** through the kernel.
- **`evalir` does not accept a function definition or a bare expression** — it wants IR text or a
  `CompilationResult`; passing `"not-ir"` answers `Malformed IR line: not-ir` (observed).
- **No unevaluated or partial application.** The value list must match the parameter count exactly.

## Examples

### Worked examples

`compile` lowers a symbolic expression to IR. The parameter list is the kernel's argument order:

```lovelace
x = symbol("x")
compile(x^2 + 1, [x])
```

```text
= #!mathir 2
param x
const (rat 1 1)
const (rat 2 1)
Constant 0 0
Parameter 0 0
Constant 0 1
PowInt 0 1,2
Add 0 0,3
```

`lower` emits the same program without the compiler's record:

```lovelace
x = symbol("x")
lower(x^2 + 1, [x])
```

```text
= #!mathir 2
param x
const (rat 1 1)
const (rat 2 1)
Constant 0 0
Parameter 0 0
Constant 0 1
PowInt 0 1,2
Add 0 0,3
```

`evalir` takes the IR (or the record `compile_full` returns), the values in parameter order, and the
working precision. `x = 3` gives `9 + 1`, and the typed result is an exact `Integer`, not a `Real`,
because nothing in the program needed a division:

```lovelace
x = symbol("x")
evalir(compile(x^2 + 1, [x]), [3], 40)
```

```text
= 10 (Integer)
```

`evalir_batch` takes flat row-major values, so a one-parameter kernel consumes one value per call:

```lovelace
x = symbol("x")
k = compile(x^2 + 1, [x])
evalir_batch(k, [1, 2, 3], 40)
```

```text
= [2, 5, 10] (Vector)
```

`compile_full` is the structured form: the IR plus the kernel's parameters, domains, policies and
version.

```lovelace
x = symbol("x")
compile_full(x^2 + 1, [x])
```

```text
= CompilationResult(ir: #!mathir 2
param x
const (rat 1 1)
const (rat 2 1)
Constant 0 0
Parameter 0 0
Constant 0 1
PowInt 0 1,2
Add 0 0,3, parameters: [ParameterInfo(name: x, domain: complex)], result_domain: complex, result_type: Integer, precision_policy: caller_supplied, optimization_policy: none, target: mathir, mathir_version: 2, exact: True, diagnostics: []) (CompilationResult)
```

A kernel from a *newer* format version is refused rather than guessed at:

```lovelace
evalir("#!mathir 3", [3], 40)
```

```text
Error [InvalidInput/ParseError]: Unsupported MathIR format version '3' (expected 2).
```

Evaluation checks the arity of the value list against the kernel's parameters before running:

```lovelace
x = symbol("x")
evalir(compile(x^2 + 1, [x]), [1, 2], 40)
```

```text
Error [InvalidOperation/DomainError]: evalir(): kernel expects 1 parameters; got 2 values.
```

Compiling with an empty parameter list is refused outright:

```lovelace
x = symbol("x")
compile(x^2 + 1, [])
```

```text
Error [InvalidArgument/TypeMismatch]: Compilation requires at least one parameter. (Parameter 'parameters')
```

### Verified calls

Every builtin in the table above was executed through the built CLI. Each row is the exact one-line
call and the typed result observed. IR results are shown whitespace-collapsed here; the worked
examples above show the line format verbatim.

| Builtin | Exact call | Observed typed result |
| --- | --- | --- |
| `compile` | `x = symbol("x"); compile(x^2 + 1, [x])` | = #!mathir 2 param x const (rat 1 1) const (rat 2 1) Constant 0 0 Parameter 0 0 Constant 0 1 PowInt 0 1,2 Add 0 0,3 |
| `compile_full` | `x = symbol("x"); compile_full(x^2 + 1, [x])` | = CompilationResult(ir: #!mathir 2 param x const (rat 1 1) const (rat 2 1) Constant 0 0 Parameter 0 0 Constant 0 1 PowInt 0 1,2 Add 0 0,3, parameters:... |
| `evalir` | `x = symbol("x"); evalir(compile(x^2 + 1, [x]), [3], 40)` | = 10 (Integer) |
| `evalir_batch` | `x = symbol("x"); k = compile(x^2 + 1, [x]); evalir_batch(k, [1, 2, 3], 40)` | = [2, 5, 10] (Vector) |
| `lower` | `x = symbol("x"); lower(x^2 + 1, [x])` | = #!mathir 2 param x const (rat 1 1) const (rat 2 1) Constant 0 0 Parameter 0 0 Constant 0 1 PowInt 0 1,2 Add 0 0,3 |

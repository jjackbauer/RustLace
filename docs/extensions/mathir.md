# Compilation / MathIR (`Lovelace.MathIR`)

**5 builtins.** The bridge from a symbolic expression to a typed, validated computational DAG and
back out at a caller-chosen precision.

## What it is

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

It contributes **5** of the engine's 124 builtins, all with category `Compilation`, and is declared
by 5 `plugin: Some("Lovelace.MathIR")` tags in
[`lovelace-lang/src/registry.rs`](../../lovelace-lang/src/registry.rs) at lines 352-1055. Here
`category` and family happen to coincide — that is a coincidence, not the rule: the Symbolics family
spans six categories.

The family is exactly the compile/evaluate loop. It contributes no expression construction, no
rewriting and no optimization: `compile` lowers what it is given, and the record says so
(`optimization_policy: none`).

## How it is enabled

`lovelace-run` always builds `Engine::new()`, so all five names are live in every CLI run; no CLI
flag disables a family. In the library, `Engine::new()` leaves `enabled_plugins` as `None`, which
makes the gate answer `true` for every registered name
([`lovelace-lang/src/lib.rs`](../../lovelace-lang/src/lib.rs), lines 990-998).

`Engine::new_bare()` (`lib.rs:875`) sets `enabled_plugins = Some(empty)`. Observed from
[`bare_engine.rs`](../../lovelace-lang/examples/bare_engine.rs) (run it with `cargo run -p lovelace-lang --example bare_engine`), the relevant lines are:

```text
bare       lower(1, [])             => [UnknownFunction] Unknown function 'lower'.
bare+ir    lower(1, [])             => #!mathir 2
const (rat 1 1)
Constant 0 0
```

The bare failure is code `UnknownFunction`, category `UnsupportedOperation` (`builtins.rs:4279`),
raised at the gate (`lib.rs:2146-2150`) — the same shape as any unknown name, with no
plugin-specific error.

Re-enabling is a name-keyed load. A `ModusPlugin` whose `name()` is `"Lovelace.MathIR"` flips the
gate for these five descriptors, even if it registers nothing of its own:

```rust
use lovelace_lang::modus::{ModusHost, ModusPlugin};
use lovelace_lang::{Engine, LangError};

struct EnableMathIr;
impl ModusPlugin for EnableMathIr {
    fn name(&self) -> &str { "Lovelace.MathIR" }
    fn register(&mut self, _host: &mut ModusHost) -> Result<(), LangError> { Ok(()) }
}

let mut engine = Engine::new_bare();
assert!(engine.eval("lower(1, [])").is_err());
engine.load_plugin(&mut EnableMathIr).unwrap();
assert!(engine.eval("lower(1, [])").unwrap().display().starts_with("#!mathir 2"));
```

The same session can then mix families freely — compiling a symbolic expression needs both MathIR and
Symbolics loaded, which is exactly what `Engine::new()` gives you.

## Every function it contributes

Arity is `min-max` accepted arguments. Parameters and descriptions are the registry's declared
metadata; every row was executed and its typed result is in the [verified-call appendix](#verified-calls).

| Builtin | Arity | Parameters | Returns | What it does |
| --- | --- | --- | --- | --- |
| `compile` | 2 | `f`, `params` | Text | Compiles a symbolic expression to a validated MathIR kernel (serialized IR text). Use compile_full for the structured kernel metadata. |
| `compile_full` | 2 | `f`, `params` | CompilationResult | Structured compile: a CompilationResult record with the IR, per-parameter name and domain, result domain and type, the precision and optimization policies, target, MathIR version, and exactness. |
| `evalir` | 3 | `ir`, `values`, `digits` | Real \| Complex | Evaluates a compiled kernel (IR text or CompilationResult) at one point at the given precision. |
| `evalir_batch` | 3 | `ir`, `values`, `digits` | Vector | Batch-evaluates a compiled kernel (IR text or CompilationResult) over flat row-major values at the given precision. |
| `lower` | 2 | `f`, `params` | Text | Lowers a symbolic expression to MathIR (the typed, validated computational DAG). |

## Worked examples

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

## Precision, limits and the version

- **Format version 2.** `mathir::FORMAT_VERSION` is `2` (`mathir.rs:35`), the header is `#!mathir 2`
  (`mathir.rs:335-340`), and any other version is refused with
  `Unsupported MathIR format version '<v>' (expected 2).` (`mathir.rs:375-379`). The wire envelope
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

## Where the code lives

| Path | What is there |
| --- | --- |
| `lovelace-lang/src/registry.rs` | the 5 descriptors; plugin tags at lines 352-1055 |
| `lovelace-lang/src/mathir.rs` | `IrOpKind` (39-110), `IrProgram`, serialization (335), the version gate (375), validation |
| `lovelace-lang/src/builtins.rs` | the `compile`/`compile_full`/`lower`/`evalir`/`evalir_batch` dispatch arms |
| `lovelace-lang/src/lib.rs` | the gate (990), `load_plugin` (883) |
| `lovelace-lang/src/numeric.rs` | the exact numeric backend the evaluator calls |

Tests that cover it:

| Test module | Tests |
| --- | --- |
| `mathir_round54` | 13 |
| `round97_tail` | 12 (incl. the Symbolics + MathIR = 108 builtin count) |
| `mathir_typing_round55` | 7 |
| `round84_evalir_multi` | part of the `lovelace-lang` batches |

`round97_tail` is where the published count is pinned: a `SuiteEngine` with Symbolics + MathIR
publishes 108 builtins, which is `44 core + 59 Symbolics + 5 MathIR`.

## What is NOT in this extension

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

## Verified calls

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

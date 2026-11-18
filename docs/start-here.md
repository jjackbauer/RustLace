# Get started with Lovelace

Lovelace is the scripting language provided by RustLace. This guide takes you from a
first calculation to using collections and functions. You do **not** need to
understand Rust crates, the interpreter, or the documentation harness to write a
script. Allow about 10 minutes.

If you already know the language, go straight to the
[language reference](language/README.md), the
[library guide](how-to/use-libraries.md), or a
[complete problem tutorial](tutorials/README.md).

## 1. Run your first expression

**Prerequisites:** Rust 1.85 or newer, Cargo, and a checkout of this repository.
On Windows the command-line build needs the MSVC build tools; you do not need a
desktop GUI. See [installation and troubleshooting](build-and-run.md) if the
runner does not build.

From the repository root:

```console
cargo run -p lovelace-run -- --eval "1 / 3" --text
```

The Lovelace expression and its output are:

```lovelace
1 / 3
```
```text
= 0.(3) (Real)
  _ = 0.(3)
```

The first line is the result of the last statement. The following `_` entry
is the last result, also available to later statements in the same execution.
`0.(3)` means an **exact repeating decimal** (one third); it is not the
rounded `0.333`. This is a [Real](language/values-and-types.md#numeric-kinds-and-widening)
value. Learn why exact representation and mathematical correctness are different
in [exactness](language/values-and-types.md#exactness) and
[precision](language/precision-and-formatting.md).

**Shell tip:** write anything longer than a one-liner in a `.ls` file to avoid
quoting problems across PowerShell, bash and Command Prompt. To run a file:

```console
cargo run -p lovelace-run -- --file my-script.ls --text
```

The CLI's default output is **JSON**, intended for programs. `--text` is the
human-readable rendering used throughout these guides; use JSON and the
[wire/diagnostic reference](language/errors-and-diagnostics.md) for automation.

## 2. Understand values before using a library

The language is dynamically typed: values have kinds, but variables do not
need type declarations. `type(value)` returns the name of the runtime kind.

```lovelace
[type(1), type(1/2), type([1,2]), type([[1,2]])]
```
```text
= [Natural, Real, Vector, Array] (Vector)
  _ = [Natural, Real, Vector, Array]
```

What you just used:

| Value | Kind | Why it matters |
|---|---|---|
| `1` | [Natural](language/values-and-types.md#numeric-kinds-and-widening) | An arbitrary-size nonnegative integer. |
| `1/2` | [Real](language/values-and-types.md#numeric-kinds-and-widening) | Exact rational result represented in the decimal model; not a binary float. |
| `[1,2]` | [Vector](language/values-and-types.md#vector-and-array) | Rank-1 sequence; many signal and array operations accept this. |
| `[[1,2]]` | [Array](language/values-and-types.md#vector-and-array) | Rank-2 array with one row, not the same shape as a vector. |

Numbers can [widen automatically](language/values-and-types.md#numeric-kinds-and-widening).
A list is not an arbitrary Rust `Vec`: read the language's
[collection rules](language/values-and-types.md#vector-and-array)
before relying on mixed-element or multidimensional behavior.

## 3. Use a function with a shaped collection

The built-in `zeros(rows, columns)` constructs an
[Array](language/values-and-types.md#vector-and-array). `shape` returns its
dimensions as a [Vector](language/values-and-types.md#vector-and-array):

```lovelace
shape(zeros(2, 3))
```
```text
= [2, 3] (Vector)
  _ = [2, 3]
```

This is a 2-row, 3-column array. Shapes matter: passing a flat vector where
an operation requires a column array is a common error in matrix calculations.
Read the [array API](language/builtins.md#arrays-23) and then try the
[linear-system tutorial](tutorials/linear-algebra.md).

## 4. Define a function

A function's parameters are local to the call. Functions use `func`;
there is no separate type declaration:

```lovelace
func square(x) = x^2; square(9)
```
```text
= 81 (Natural)
  _ = 81
```

The `=` in `func square(x) = ...` defines the expression body; elsewhere
assignment binds a name, while `==` compares values. See
[functions and scope](language/functions-and-scope.md#defining-a-function)
and [assignment](language/statements-and-control-flow.md#assignment-versus-definition).
Functions can also have statement blocks, conditions and recursion.

## 5. Learn to read a refusal

A wrong number of arguments is not a numerical result:

```lovelace
sqrt(1, 2)
```
```text
Error [InvalidArgument/TypeMismatch]: sqrt(): expected 1 argument; got 2.
```

In the human format the diagnostic is printed instead of a result.
An automated caller must check the **exit status and structured diagnostic**
rather than parse that sentence. See
[errors and diagnostics](language/errors-and-diagnostics.md#the-error-envelope).
To discover a builtin's accepted parameters before calling it:

```console
cargo run -p lovelace-run -- --function sqrt
```

## 6. Choose your next topic

For arrays and linear algebra, proceed to
[solve a linear system](tutorials/linear-algebra.md). For symbolic expressions
and assumptions, see [symbolic differentiation](tutorials/symbolic.md).
For signals and complex values, continue with
[signal processing](tutorials/signal-processing.md). To drive RustLace from
another program, use [reproducible JSON output](tutorials/reproducible.md).

If you want to know **which library function fits your problem**, use
[Working with libraries](how-to/use-libraries.md). It distinguishes the
public, language-level families from the internal
[Rust implementation crates](crates/lovelace-lang.md): you **do not import**
a Rust crate to call `fft`, `matmul`, `diff`, or `sys_mm1` in a Lovelace script.

For syntax, semantics and exhaustive limits, the authoritative pages are the
[language reference](language/README.md) and
[builtin catalogue](language/builtins.md).

# The Lovelace Language Reference

This is the language reference for **RustLace**, the Rust implementation of the Lovelace
scripting language. It documents the syntax and semantics of the language as implemented by the
`lovelace-lang` engine and exercised by the `lovelace-run` command-line runner. Every example in
these pages is a real script, and every `text` block under an example is the output observed from
that script on this tree.

The reference is organised as a set of focused pages rather than one enormous file. Read the
first four in order if you are new to the language; the remaining pages are reference material
that can be consulted independently.

## Find what you need

| Goal | Page |
|---|---|
| First time writing a `.ls` script? | [Get started](../start-here.md) — a short, executable introduction to expressions, values, arrays, functions and errors |
| Working with a library? | [Working with libraries](../how-to/use-libraries.md) — task-driven examples with input and output type links |
| Looking up a value kind? | [Values and types](values-and-types.md) — the public kinds, conversions, exactness and collection shapes |
| Looking up a function? | [Builtins](builtins.md) — complete catalogue; the per-function CLI descriptor is available through `--function` |
| Want an end-to-end worked problem? | [Tutorials](../tutorials/README.md) — select by topic |
| Calling the runner from a program? | [JSON errors and diagnostics](errors-and-diagnostics.md) and [consumer-agent guide](../agent-usage.md) |

**Conventions:** `lovelace` blocks are executable input; the adjacent `text`
blocks are observed `--text` output. In prose, a type name links to its
definition, and a library operation links to the relevant catalogue or family
reference. The examples describe the checked-out source revision, not an
unspecified older binary (see [version applicability](../version-applicability.md)).

## What RustLace is

RustLace is a complete scripting language for arbitrary-precision mathematics. One engine serves
every surface: the native desktop workbench and the headless `lovelace-run` runner. The
engine has no external crates under it and uses arbitrary-precision decimal arithmetic: there is no
floating-point type in the value path, no fixed-width integer hiding behind the syntax, and no silent
rounding. Exactness is representational: `1 / 3` is the exact periodic decimal `0.(3)` and
`2 ^ 100` is a 31-digit natural, while `sqrt(2)` is a precision-limited value computed to the
precision you ask for.

The desktop workbench runs on Windows x86_64 and on Linux x86_64 (X11) — the language is not tied to
a platform. See [ADR-008](../adr/008-linux-gui-support.md) for what is certified and what is not,
and the Linux section of [build-and-run.md](../build-and-run.md#linux) for the GUI runtime packages.

The language is dynamically typed. Values are inspected with `type()` and `inspect()` rather than
declared, and the numeric kinds widen automatically (`Natural` → `Integer` → `Real`). Arrays are
first-class: a nested list literal is a rank-`k` array, arithmetic broadcasts, and a large
builtin catalogue covers linear algebra, calculus, symbolic manipulation, polynomial solving,
digital signal processing and MathIR compilation.

## Running a script

The runner accepts an inline script, a file, or standard input:

```console
cargo run -p lovelace-run -- --eval "1 / 3"
cargo run -p lovelace-run -- --file script.ls
cargo run -p lovelace-run -- --file script.ls --text
```

Scripts use the `.ls` extension by convention (`examples/hello.ls`, and every `.ls` fixture under
`docs/csharp-oracle/`). The runner does not require an extension: it runs whatever path it is given,
so an older `.lv` file still works, but new material should use `.ls`.

JSON is the default output mode; `--text` selects the human rendering used by the examples in
this reference. A successful run prints the result (and any captured `print` output), a failed
run exits with status `1` and publishes the typed error envelope. `--functions` lists the builtin
registry and `--function <name>` describes one entry.

`--eval` is convenient for shell one-liners, but embedded double quotes are mangled by some
shells (notably Windows PowerShell 5.1). The examples here were produced through `--file`, which
is also what the example runner uses.

## Running the examples

Every page has been executed by the runner in `examples/run-examples.ps1`. From the repository
root:

```console
pwsh -File docs/language/examples/run-examples.ps1
```

The script extracts every `lovelace` fenced block from `docs/language/*.md`, runs it with
`--file` and `--text`, and compares the normalised output with the `text` block that follows. It
prints the number of examples that passed and exits non-zero on the first mismatch. Set
`LOVELACE_RUN` to point at a different binary if you do not want the default
`target/debug/lovelace-run.exe`. On a host with a restrictive PowerShell execution policy, run it
as `pwsh -ExecutionPolicy Bypass -File docs/language/examples/run-examples.ps1` (or sign the
script).

## The language in ten lines

1. A program is a sequence of statements separated by `;` or a newline; the value of the program
   is the value of its last statement, also stored in `_`.
2. Numbers are decimal, not binary floating point: `Natural`, `Integer`, and `Real`
   (arbitrary-precision decimal with repeating-period metadata). Exactness is representational — a
   computed result can be precision-limited or carry an `inexact` provenance flag — and the numerics
   widen `Natural → Integer → Real`.
3. A value is one of those numerics, `Boolean`, `Text`, `Vector` (rank 1), `Array` (rank ≥ 2),
   `Complex`, `Symbolic`, `Record`, `Domain`, `Enum`, `Function`, or `Void`.
4. `=` assigns; it is not equality. Assignment yields the assigned value and is right-associative.
   `==`, `!=`, `<`, `<=`, `>`, `>=` compare and produce `Boolean`.
5. `func f(x) = expr` and `func f(x) { … }` define functions. Calls are by name; functions are
   recursive and have lexical locals but no closures.
6. Control flow is `if`/`else`, `while`, `for … in`, `return`, `break`, `continue`. Conditions
   must be `Boolean`; there is no truthiness.
7. Expressions are parsed by precedence, tightest first: postfix `!`/index/member, unary
   `-`/`+`/`not`, range `..`, power `^`, `* / %`, `+ -`, comparisons, assignment.
8. `1..5` is an inclusive range with step 1; `1..2..7` uses step 2. Ranges and list literals are
   vectors; `[[1,2],[3,4]]` is a rank-2 array.
9. The engine ships 134 builtins in 12 categories, from `abs` and `sum` to `diff`, `solve`,
   `fft`, `compile` and `plot`. See [builtins.md](builtins.md).
10. Failures are a typed envelope with a stable `code`, a frozen `category`, a sentence, and a
    1-based `line`/`column`; precision is session state, and the default differs between the CLI
    engine and the desktop.

## A quick tour

```lovelace
2 ^ 100
```
```text
= 1267650600228229401496703205376 (Natural)
  _ = 1267650600228229401496703205376
```

```lovelace
1 / 3
```
```text
= 0.(3) (Real)
  _ = 0.(3)
```

```lovelace
func fib(n) { if (n < 2) { n } else { fib(n - 1) + fib(n - 2) } }; fib(10)
```
```text
= 55 (Natural)
  _ = 55
```

```lovelace
1..5
```
```text
= [1, 2, 3, 4, 5] (Vector)
  _ = [1, 2, 3, 4, 5]
```

## Table of contents

| Page | What it covers |
|---|---|
| [Lexical structure](lexical-structure.md) | Source encoding, whitespace, comments (there are none), identifiers, every literal form, separators, and the complete token set. |
| [Values and types](values-and-types.md) | Every runtime value kind, exactness, periodic decimals, widening, truthiness, and the names `type()` reports. |
| [Expressions and operators](expressions-and-operators.md) | The full prefix/infix/postfix operator set, the precedence table derived from the parser, associativity, indexing, slicing, calls, and member access. |
| [Statements and control flow](statements-and-control-flow.md) | Assignment versus definition, blocks, `if`/`else`, `while`, `for`, `return`, `break`, `continue`, `print`, `plot`, `setprecision`, and what the last expression means. |
| [Functions and scope](functions-and-scope.md) | User functions, parameters, recursion, lexical scopes, shadowing, arity errors, and how a user function differs from a builtin. |
| [Builtins](builtins.md) | The complete 134-entry catalogue in 12 categories, with arity and parameter names, plus prose for each family. |
| [Errors and diagnostics](errors-and-diagnostics.md) | The error envelope, the seven frozen categories, stable codes, positions, exit statuses, and five fully quoted failures. |
| [Precision and formatting](precision-and-formatting.md) | Computation versus display precision, `setprecision`, `pi(n)`/`e(n)`, truncation, periodic rendering, and number formatting. |
| [Plots](plots.md) | `plot(x, y, title)`, the SVG contract, the returned path, and the plot artifact in the JSON envelope. |

## Scope and conventions

The pages were originally authored against commit `2152218` (the `feat/eig-sym` merge on `main`),
with a debug binary built from that tree; that is where the recorded transcripts come from, not a
claim that current behaviour is still pinned to that commit. To check the pages against the current
branch, build the runner once with `cargo build -p lovelace-run`, then run
`bash docs/language/examples/run-examples.sh` from the repository root: it extracts every `lovelace`
fence, runs it with `lovelace-run --file … --text`, and compares the output with the `text` block
that follows. `docs/language/examples/run-examples.ps1` is the same check for a Windows PowerShell
host. The engine's defaults are 1000 computation digits and 100 display
digits; the desktop starts new documents at 30 digits. Those host differences are called out in
[Precision and formatting](precision-and-formatting.md), because they change what a fresh session
prints without changing the language itself.

Examples show the complete `--text` output. That output always starts with `= <value>`; a line
`_ = …` repeats the value stored in the `_` variable, and any other `name = …` lines are the
session's variables. For a `Void` result the first line is `= (void)`. Error examples show the
one-line `--text` failure and, where the exact wire envelope matters, the JSON that
`--json` produced.

For the implementation rather than the language, see the repository documentation:
[architecture](../architecture.md), [the `lovelace-lang` crate](../crates/lovelace-lang.md), and
[build and run](../build-and-run.md).

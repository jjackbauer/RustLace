# Errors and diagnostics

A failed evaluation is not a stringly-typed exception. The engine produces a `LangError` with a
stable `code`, a frozen `category`, a complete-sentence `message`, and a source position; the
runner projects that into a JSON envelope with a `diagnostics` array. This page documents the
envelope, the categories, the common codes, the exit statuses, and five failures captured from
real runs.

## The error envelope

A failure envelope is a JSON object printed on stdout, with exit status `1`:

| Field | Meaning |
|---|---|
| `ok` | `false` on failure. |
| `protocolVersion` | Runner protocol version (`1` on this tree). |
| `mathIrVersion` | MathIR version (`2`). |
| `symbolicFormatVersion` | Symbolic wire format marker (`#!lovelace-sym 1`). |
| `code` | Stable failure code, e.g. `ParseError`, `DivisionByZero`, `InvalidArgument`. |
| `category` | One of the seven frozen categories. |
| `message` | The same sentence repeated from the first diagnostic. |
| `diagnostics` | Array of `{ message, position, line, column }` objects. |
| `recoverable` | Whether the caller may continue with another evaluation (ordinary failures are `true`). |
| `signature` | Present for a builtin argument refusal: function name, parameters, expected/actual arity. |
| `registrySize` | Present for an unresolved-name failure: how many names the engine knows. |
| `suggestions`, `hints` | Optional arrays for an unresolved name. |
| `elapsed`, `elapsedTime` | Wall-clock measurement; volatile between runs. |
| `timings` | Per-statement ledger; a statement that failed may still appear. |
| `output` | Captured `print` lines emitted before the failure. |
| `partialVariables` | Variables the failed evaluation had committed, when the host publishes them. |
| `artifacts` | Files the evaluation wrote (normally empty on failure). |

`position` is a **0-based UTF-16 code-unit offset** into the caller's text. `line` and `column`
are **1-based** values computed from that offset; CRLF counts as one line break. A diagnostic at
the start of the first line therefore has `position: 0`, `line: 1`, `column: 1`.

The same failure in `--text` mode goes to **stderr** in one line and leaves stdout empty:

```text
Error [<code>/<category>]: <message>
```

The exit status is the same `1`. Usage errors (bad flags, no script) exit `2`; success exits `0`.
The JSON envelope is the machine contract; the text line is a convenience.

## The seven categories

The vocabulary is frozen and ordered in `lovelace-lang/src/diagnostic.rs`; no additional
category may be added.

| Category | Meaning |
|---|---|
| `ParseError` | The source could not be parsed. |
| `DomainError` | The value or operation is outside the operation's domain. |
| `UnsupportedOperation` | The runtime does not offer the operation. |
| `BudgetExceeded` | A declared budget was exhausted, including cancellation. |
| `NoSolution` | The equation or system has no solution. |
| `TypeMismatch` | An argument's kind or shape does not match the declared contract. |
| `InternalInvariantFailure` | An internal invariant was violated. |

`NoSolution` is normally carried inside a structured result (for example a `SolveResult` with a
`solve.no-solutions` diagnostic) rather than raised as a top-level failure; the category still
exists for the frozen vocabulary. `InternalInvariantFailure` is reserved for genuine engine
invariant violations and is not a normal script outcome.

## Codes you will meet

`code` is finer-grained than `category`. These are the codes observed in ordinary use:

| Code | Category | Typical trigger |
|---|---|---|
| `ParseError` | `ParseError` | Unexpected token or character; missing separator. |
| `InvalidInput` | `ParseError` | Unterminated string literal. |
| `InvalidOperation` | `DomainError` | Undefined variable, index out of range, non-Boolean condition, non-numeric comparison, `break` outside a loop. |
| `DivisionByZero` | `DomainError` | `/` or `%` with a zero divisor. |
| `UnsupportedOperation` | `UnsupportedOperation` | Non-integer exponent; an operation the kernel does not offer. |
| `UnknownFunction` | `UnsupportedOperation` | A call to a name that is neither a user function nor a builtin. |
| `InvalidArgument` | `TypeMismatch` | Builtin arity, argument kind, argument range, or shape refusal. |
| `DepthExceeded` | `BudgetExceeded` | A parser, evaluation, symbolic, or value nesting budget. |
| `Cancelled` | `BudgetExceeded` | A host deadline or cancel flag stopped the run. |
| `ArithmeticError` | `DomainError` | An integer conversion or exponent outside the supported range. |
| `FileReadError` | `TypeMismatch` | The script file could not be read or decoded. |
| `PlotDirectoryError` | `TypeMismatch` | The plot directory is empty or cannot be created. |
| `PlotFileError` | `TypeMismatch` | The SVG file cannot be written at the requested path. |

The list is representative, not a promise that no other code exists. The stable contract is the
pair of strings; consumers should branch on `code` (and fall back to `category`) rather than
parsing the message.

## Where a diagnostic points

- A parser failure points at the token that could not be used.
- A builtin argument failure points at the **call site** (the callee's source position), not at
  the closing parenthesis, and carries the declared `signature`.
- A failure raised inside a function body without its own position is published at the
  **statement that called the function**, because the evaluator fills in a zero position with the
  current top-level statement's start.
- A value/domain failure inside a top-level expression is positioned at that statement's start.

For example, `sqrt(1,2)` written after `1 + ` reports `position: 4`, `column: 5`, because the call
starts at the `s`. A division by zero on the third line of a script reports `line: 3`,
`column: 1`, because the failing statement starts there.

## Worked example 1: syntax error

The script `1 +` ends in the middle of an additive expression. The parser reaches end of input
where a primary expression is required.

```lovelace
1 +
```
```text
Error [ParseError/ParseError]: Unexpected token '' at position 3: expected a number, string, identifier, '[', or '('.
```
```json
{"artifacts":[],"category":"ParseError","code":"ParseError","diagnostics":[{"column":4,"line":1,"message":"Unexpected token '' at position 3: expected a number, string, identifier, '[', or '('.","position":3}],"elapsed":"148.4 \u00b5s","elapsedTime":{"unit":"\u00b5s","value":148.4},"mathIrVersion":2,"message":"Unexpected token '' at position 3: expected a number, string, identifier, '[', or '('.","ok":false,"output":[],"partialVariables":[],"protocolVersion":1,"recoverable":true,"symbolicFormatVersion":"#!lovelace-sym 1","timings":[]}
```

There is no `timings` entry because the program never produced an executable statement list.

## Worked example 2: unknown name

An unresolved **variable** is a `DomainError` / `InvalidOperation`; the message names the missing
name. This is distinct from an unresolved **function**, which is
`UnknownFunction` / `UnsupportedOperation` and carries `registrySize`.

```lovelace
foo + 1
```
```text
Error [InvalidOperation/DomainError]: Undefined variable 'foo'.
```
```json
{"artifacts":[],"category":"DomainError","code":"InvalidOperation","diagnostics":[{"column":1,"line":1,"message":"Undefined variable 'foo'.","position":0}],"elapsed":"195 \u00b5s","elapsedTime":{"unit":"\u00b5s","value":195},"mathIrVersion":2,"message":"Undefined variable 'foo'.","ok":false,"output":[],"partialVariables":[],"protocolVersion":1,"recoverable":true,"symbolicFormatVersion":"#!lovelace-sym 1","timings":[{"elapsed":{"unit":"ns","value":0},"hasOutput":false,"position":0,"resultKind":"Void"}]}
```

The failing statement still gets a `timings` entry with `resultKind: "Void"`.

## Worked example 3: division by zero

This script has three statements on three lines; the failure is on the third. The positions show
that diagnostics are computed from the caller's text, not from a normalised copy.

```lovelace
1
2
1/0
```
```text
Error [DivisionByZero/DomainError]: Cannot divide by zero.
```
```json
{"artifacts":[],"category":"DomainError","code":"DivisionByZero","diagnostics":[{"column":1,"line":3,"message":"Cannot divide by zero.","position":4}],"elapsed":"244.2 \u00b5s","elapsedTime":{"unit":"\u00b5s","value":244.2},"mathIrVersion":2,"message":"Cannot divide by zero.","ok":false,"output":[],"partialVariables":[],"protocolVersion":1,"recoverable":true,"symbolicFormatVersion":"#!lovelace-sym 1","timings":[{"elapsed":{"unit":"ns","value":0},"hasOutput":false,"position":0,"resultKind":"Natural"},{"elapsed":{"unit":"ns","value":0},"hasOutput":false,"position":2,"resultKind":"Natural"},{"elapsed":{"unit":"ns","value":0},"hasOutput":false,"position":4,"resultKind":"Void"}]}
```

`position: 4` is the zero-based UTF-16 offset of the `1` in `1/0` (the two newlines each occupy
one unit). `line: 3`, `column: 1` is the 1-based view of the same place.

## Worked example 4: arity error

A builtin called with too many arguments is refused before the implementation runs. The envelope
includes the declared `signature`, which is what a repair tool should read.

```lovelace
sqrt(1, 2)
```
```text
Error [InvalidArgument/TypeMismatch]: sqrt(): expected 1 argument; got 2.
```
```json
{"artifacts":[],"category":"TypeMismatch","code":"InvalidArgument","diagnostics":[{"column":1,"line":1,"message":"sqrt(): expected 1 argument; got 2.","position":0}],"elapsed":"430.7 \u00b5s","elapsedTime":{"unit":"\u00b5s","value":430.7},"mathIrVersion":2,"message":"sqrt(): expected 1 argument; got 2.","ok":false,"output":[],"partialVariables":[],"protocolVersion":1,"recoverable":true,"signature":{"actual":2,"expectedMax":1,"expectedMin":1,"function":"sqrt","parameters":["x"],"variadic":false},"symbolicFormatVersion":"#!lovelace-sym 1","timings":[{"elapsed":{"unit":"ns","value":0},"hasOutput":false,"position":0,"resultKind":"Void"}]}
```

The same refusal is used for `min()`, `plot()` with four arguments, `compile_full(x^2 + 1)`, and
every other builtin whose declared arity the call violates. User-function arity errors use the
`DomainError` taxonomy instead; see [Functions and scope](functions-and-scope.md#arity-and-errors).

## Worked example 5: domain error

An argument that is a valid value but outside the operation's domain is a `DomainError`. Indexing
a length-3 vector at index 5 is the canonical case.

```lovelace
[1, 2, 3][5]
```
```text
Error [InvalidOperation/DomainError]: Index 5 is out of range for vector of length 3.
```
```json
{"artifacts":[],"category":"DomainError","code":"InvalidOperation","diagnostics":[{"column":1,"line":1,"message":"Index 5 is out of range for vector of length 3.","position":0}],"elapsed":"267.4 \u00b5s","elapsedTime":{"unit":"\u00b5s","value":267.4},"mathIrVersion":2,"message":"Index 5 is out of range for vector of length 3.","ok":false,"output":[],"partialVariables":[],"protocolVersion":1,"recoverable":true,"symbolicFormatVersion":"#!lovelace-sym 1","timings":[{"elapsed":{"unit":"ns","value":0},"hasOutput":false,"position":0,"resultKind":"Void"}]}
```

Other domain failures in this family include a non-Boolean `if` condition, comparing two
non-numeric values that are not structurally equal, `break` outside a loop, `divrem` on a Real,
and `log`/`sqrt` domain edges that cannot be represented.

## Volatile fields

`elapsed`, `elapsedTime`, and the per-statement `timings` values are measurements, not contract
data: they differ between runs and machines. The JSON examples above quote one observed run
verbatim, including those fields, so that the exact envelope shape is visible. A consumer must
not assert on their values; assert on `ok`, `code`, `category`, `message`, `diagnostics`, and
`signature`.

The `functions` array is present on success envelopes (unless `--omit-functions`) and absent on
failure envelopes. `variables` is present on success; failures publish `partialVariables`
instead. `plot` and `artifacts` describe files the evaluation wrote; a failure before the write
leaves them empty.

## Error text versus error JSON

The two surfaces are projections of the same `LangError`:

```text
Error [DivisionByZero/DomainError]: Cannot divide by zero.
```

is the text projection of

```text
code: DivisionByZero
category: DomainError
message: Cannot divide by zero.
```

Do not parse the text line in a tool that can read JSON. The text line is intentionally stable
and readable, but it does not carry positions, signatures, registry hints, or timing data.

## Cancellation and budgets

A host can arm a deadline (`--cancel-after <ms>`) or a cooperative cancel flag. If a kernel
observes it, the evaluation fails with `code: "Cancelled"`, `category: "BudgetExceeded"`, and the
message `the evaluation was cancelled by the caller.`; the diagnostic is placed at the statement
that was running. If the run finished between polls, the success envelope reports the overrun
truthfully in its cancellation ledger instead of pretending to be cancelled. Nesting budgets
(`DepthExceeded`) use the same `BudgetExceeded` category and name the stage and limit in the
sentence.

See the repository's protocol notes for the full wire contract; this page documents the language
surface that scripts see.

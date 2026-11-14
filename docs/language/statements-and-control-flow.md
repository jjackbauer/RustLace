# Statements and control flow

A Lovelace program is a sequence of statements. This page covers the statement forms, how
assignment and definition differ, how blocks scope names, how `if`/`while`/`for` behave, what
`return`/`break`/`continue` do, and what the program's final value means. It also lists the control
flow constructs the parser does **not** have.

## Program structure

Statements are separated by `;` or a newline. Consecutive separators are allowed, and leading or
trailing separators are ignored. The program's value is the value of its last executed statement;
each statement's value is also available to the surrounding block or loop, as described below.

| Statement | Syntax | Value |
|---|---|---|
| Expression | `expr` | The expression's value. |
| Assignment | `name = expr` | The assigned value. |
| Block | `{ statements }` | The last statement's value, or `Void` for an empty block. |
| Conditional | `if (cond) stmt [else stmt]` | The executed branch's value, or `Void` if no branch runs. |
| While loop | `while (cond) stmt` | The last completed body value, or `Void`. |
| For loop | `for name in expr stmt` | The last completed body value, or `Void`. |
| Return | `return [expr]` | Terminates the program or function with the value. |
| Break | `break` | Exits the nearest loop. |
| Continue | `continue` | Starts the next iteration of the nearest loop. |
| Function definition | `func name(params) = expr` or `func name(params) stmt` | `Void`. |
| Empty | `;` / newline | `Void`. |

A trailing `;` after an `if` or loop belongs to the enclosing statement list; the parser is
careful to keep `if (c) { … }; next` working. You do not need to memorise that detail; just write
separators normally.

## Assignment versus definition

There is no declaration keyword. `name = expr` means:

- if `name` already has a binding in the innermost scope or an enclosing scope, update it;
- otherwise define `name` in the innermost scope.

At top level, the innermost scope is the global store, so the first assignment defines a global.
Inside a braced block, a new name is local to that block; assigning an existing outer name updates
the outer binding instead of creating a local.

```lovelace
x = 1; x = x + 1; x
```
```text
= 2 (Natural)
  _ = 2
  x = 2
```

```lovelace
x = 1; { x = 2 }; x
```
```text
= 2 (Natural)
  _ = 2
  x = 2
```

```lovelace
{ y = 3 }; y
```
```text
Error [InvalidOperation/DomainError]: Undefined variable 'y'.
```

Inside a function, the parameter frame is the innermost scope. A new name is local to the frame;
an existing **global** name is updated, because function frames are chained to the global store,
not to the caller's locals. This is a deliberate scope model, not a closure; see
[Functions and scope](functions-and-scope.md#scope-and-shadowing).

Assignment is an expression: it yields the assigned value, can be chained, and can appear inside
a larger expression. The target must be a plain identifier; there is no indexed or field
assignment. Compound assignment does not exist.

```lovelace
x = y = 5; x + y
```
```text
= 10 (Natural)
  _ = 10
  x = 5
  y = 5
```

The `_` variable is special only in that the engine stores the final non-`Void` program value in
it after a successful evaluation. It is not a live "last expression" register during the run: an
expression cannot read its own eventual value from `_`, and a `Void` result leaves `_` unchanged.
In a fresh one-shot run there is no previous `_`.

```lovelace
1; 2; 3
```
```text
= 3 (Natural)
  _ = 3
```

The intermediate values `1` and `2` are evaluated for their side effects and discarded. To keep a
value, assign it.

## Blocks

A block is `{ ... }`. It has three jobs: it groups statements, it evaluates to its last
statement's value, and it introduces a lexical scope.

```lovelace
{ x = 1; x + 2 }
```
```text
= 3 (Natural)
  _ = 3
```

`x` does not appear in the variable list because the block created it locally and the scope was
popped. A block is a statement, not an expression: `x = { 1; 2 }` is a parse error, because the
right-hand side of an assignment must be an expression and `{` cannot start one.

```lovelace
x = { 1; 2 }
```
```text
Error [ParseError/ParseError]: Unexpected token '{' at position 4: expected a number, string, identifier, '[', or '('.
```

Newlines work inside a block because the block parser skips statement separators between the
statements:

```lovelace
{
  x = 1
  x + 2
}
```
```text
= 3 (Natural)
  _ = 3
```

## Conditions and `if`/`else`

`if (condition) then-statement [else else-statement]` evaluates the condition first. It must be a
`Boolean`; numbers, Text, arrays, and records are not truthy.

```lovelace
if (1 < 2) { 10 } else { 20 }
```
```text
= 10 (Natural)
  _ = 10
```

The value of an `if` statement is the value of the executed branch. A `false` condition with no
`else` produces `Void`:

```lovelace
if (1 > 2) { 1 }
```
```text
= (void)
```

A non-Boolean condition is a `DomainError` with a sentence naming the kind that arrived:

```lovelace
if (1) { 2 }
```
```text
Error [InvalidOperation/DomainError]: if condition must be Boolean, but got 'Natural'.
```

Because `true` is a Natural literal in this language, the same failure occurs for the spelling
that looks most natural:

```lovelace
if (true) { 2 }
```
```text
Error [InvalidOperation/DomainError]: if condition must be Boolean, but got 'Natural'.
```

`else if` is ordinary nesting: the `else` branch is another `if` statement.

```lovelace
x = 2
if (x == 1) { 10 } else if (x == 2) { 20 } else { 30 }
```
```text
= 20 (Natural)
  _ = 20
  x = 2
```

`if` is a statement, not an expression. `x = if (1 < 2) { 1 }` cannot work: after the `=` the
parser starts an expression, treats `if` as a variable name, and then refuses the `{`. Wrap the
logic in a function or assign inside each branch instead.

## `while`

`while (condition) body` repeats the body while the condition is true. The condition is
re-evaluated before every iteration and must be Boolean. The loop's value is the last completed
body value; a loop that never runs a body, or whose body produces no value, is `Void`.

```lovelace
i = 0; while (i < 3) { i = i + 1 }; i
```
```text
= 3 (Natural)
  _ = 3
  i = 3
```

`break` exits the loop, and `continue` jumps to the next condition check. `break` wins over the
body value; a break that happens after some completed iterations leaves the last completed value
as the loop's value.

```lovelace
i = 0; while (i < 10) { i = i + 1; if (i == 3) { break } }; i
```
```text
= 3 (Natural)
  _ = 3
  i = 3
```

A non-Boolean `while` condition has its own sentence: "while condition must be Boolean, but got
'<kind>'." Both loops poll the cancellation flag between iterations, so a long loop can be
stopped by the host.

## `for ... in`

`for name in expression body` evaluates the expression once, requires it to be a `Vector`, and
binds each element to `name` in order. The canonical iterable is a range; a list literal works
too. A rank ≥ 2 array is refused: "for loop needs a range, but got 'Array'."

```lovelace
s = 0; for i in 1..5 { s = s + i }; s
```
```text
= 15 (Natural)
  _ = 15
  i = 5
  s = 15
```

```lovelace
for x in [10, 20] { print(x) }
```
```text
= (void)
10
20
  x = 20
```

The loop variable is defined in the **current** scope, not in a fresh per-iteration scope. At top
level that makes it global after the loop (`i = 5` above); inside a block or function it stays in
that scope. The loop's value is the last completed body value, so `for i in 1..3 { i * 10 }` is
`30`; a `continue` skips the value of the skipped iteration, and a `break` stops the loop.

```lovelace
for i in 1..3 { i * 10 }
```
```text
= 30 (Natural)
  _ = 30
  i = 3
```

`for` has no C-style form: there is no initializer/condition/update header, no multiple loop
variables, no `for (;;)`, and no iteration over arbitrary objects. Write `for x in some_vector`
and build the vector first.

## `return`

`return` exits the current function with the optional expression's value. With no expression it
returns `Void`. At top level there is no function to return from, so `return` simply terminates
the program with that value; statements after it do not run.

```lovelace
return 5; 6
```
```text
= 5 (Natural)
  _ = 5
```

`return` inside a loop leaves the enclosing function, not just the loop. The function page has
the recursion and early-return examples.

## `break` and `continue`

Both are only valid inside a loop. `break` leaves the nearest enclosing loop; `continue` starts
the next iteration of the nearest enclosing loop. They do not cross a function boundary.

```lovelace
s = 0; for i in 1..4 { if (i == 2) { continue }; s = s + i }; s
```
```text
= 8 (Natural)
  _ = 8
  i = 4
  s = 8
```

A `break` outside a loop is a `DomainError` naming the problem:

```lovelace
break
```
```text
Error [InvalidOperation/DomainError]: 'break' is only valid inside a loop.
```

`continue` outside a loop is refused with the same sentence (the implementation reuses the
break-outside-loop message; this is an observed quirk, not a documentation typo):

```lovelace
continue
```
```text
Error [InvalidOperation/DomainError]: 'break' is only valid inside a loop.
```

## Function definitions

`func` is a statement, not an expression. It has two syntactic forms:

```text
func name(param, ...) = expression
func name(param, ...) statement
```

The expression form accepts exactly one expression. It cannot contain `if`, `while`, `for`, or
`return`, because those are statements. Use the block form for anything that needs control flow:

```lovelace
func square(x) = x * x; square(9)
```
```text
= 81 (Natural)
  _ = 81
```

```lovelace
func absdiff(a, b) { if (a < b) { b - a } else { a - b } }; absdiff(3, 10)
```
```text
= 7 (Natural)
  _ = 7
```

The definition itself evaluates to `Void`. Definitions are registered when the statement
executes, so a call before the definition is an unknown function. See
[Functions and scope](functions-and-scope.md).

## `print`, `plot`, and `setprecision`

These are ordinary builtin calls, not statement keywords, and they are useful for their side
effects: `print` appends a line to the engine's captured output and returns `Void`, `plot` writes
an SVG file and returns its path as `Text`, and `setprecision` changes the session's precision and
returns `Void`.

```lovelace
print("hi", 42, [1,2])
```
```text
= (void)
hi 42 [1, 2]
```

`print` separates arguments with a single space and renders each with its display form (no type
suffix). Its output appears after the `= (void)` line in `--text` mode and in the `output` array
of the JSON envelope. `plot` is the one of the three that returns a value (Text); see
[Plots](plots.md). `setprecision` is covered in
[Precision and formatting](precision-and-formatting.md).

```lovelace
setprecision(10); sqrt(2)
```
```text
= 1.4142135623 (Real)
  _ = 1.4142135623
```

## What the language does not have

The following are absent, and scripts that try them fail at parse time or as unknown names:

- `switch`/`case`, `match`, guards, and pattern matching;
- `do … while`, `repeat … until`, and loop `else` clauses;
- `try`/`catch`/`finally`, `throw`, and user-raised errors;
- `goto`, labels, and non-local exits other than `return`;
- `import`/`use`/`include` and modules;
- classes, objects, interfaces, traits, and user-defined records or enums;
- lambdas, anonymous functions, and function literals;
- `async`/`await` and generators;
- truthiness-based conditions;
- C-style `for`, `foreach` over arbitrary iterables, and iterators;
- `++`/`--`, compound assignment, and destructuring assignment.

Control flow is deliberately small: blocks, `if`/`else`, `while`, `for … in`, `return`,
`break`, and `continue`. Everything else is an expression, a function call, or absent.

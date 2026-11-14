# Functions and scope

Functions are the only user-defined abstraction in the language. This page covers the two
definition forms, parameter passing, call resolution, the scope chain, recursion, arity errors,
and the differences between a user function and a builtin.

## Defining a function

A function definition is a statement:

```text
func name(param, ...) = expression
func name(param, ...) { statements }
```

The `=` form accepts exactly one **expression**. The braced form accepts a statement, usually a
block. There is no `return` type, no default parameter value, no variadic parameter, and no
parameter type annotation.

```lovelace
func square(x) = x^2; square(9)
```
```text
= 81 (Natural)
  _ = 81
```

```lovelace
func inc(x) { return x + 1 }; inc(5)
```
```text
= 6 (Natural)
  _ = 6
```

The expression form cannot contain `if`, `while`, `for`, `return`, or multiple statements,
because those are statements rather than expressions. Use the braced form:

```lovelace
func absdiff(a, b) { if (a < b) { b - a } else { a - b } }; absdiff(3, 10)
```
```text
= 7 (Natural)
  _ = 7
```

A function whose body finishes without `return` returns the value of its last executed statement,
so the braced form is also an expression-bodied function with extra room:

```lovelace
func f() { 1; 2 }; f()
```
```text
= 2 (Natural)
  _ = 2
```

A bare `return` (no expression) returns `Void`. A `return` inside a loop returns from the
function, not just the loop.

## Calling a function

A call is `name(arguments)`. The callee must be a name; calls are resolved at run time, statement
by statement. That means a function called before its definition does not exist yet:

```lovelace
f(1); func f(x) = x + 1
```
```text
Error [UnknownFunction/UnsupportedOperation]: Unknown function 'f'.
```

Arguments are evaluated left to right before the call. An argument may contain an assignment,
which is visible after the call:

```lovelace
func f(a) = a; f(x = 7); x
```
```text
= 7 (Natural)
  _ = 7
  x = 7
```

There is no pass-by-reference. Values are cloned into the parameter frame; mutating a parameter
does not mutate the caller's binding. Mutating a **global** from inside a function is a different
matter and is covered below.

## Arity and errors

A user function has an exact arity: the number of declared parameters. Too few or too many
arguments is a `DomainError` with a sentence in the form `'name' expects N argument(s), but M
were given.`:

```lovelace
func f(a, b) = a + b; f(1)
```
```text
Error [InvalidOperation/DomainError]: 'f' expects 2 argument(s), but 1 were given.
```

Builtins use the registry's declared arity and a different taxonomy:
`InvalidArgument` / `TypeMismatch`, with a signature on the JSON envelope. `sqrt` takes one
argument:

```lovelace
sqrt(1, 2)
```
```text
Error [InvalidArgument/TypeMismatch]: sqrt(): expected 1 argument; got 2.
```

There is no overloading. A second definition of the same name replaces the first; the last
definition wins.

```lovelace
func f() = 1; func f() = 2; f()
```
```text
= 2 (Natural)
  _ = 2
```

Duplicate parameter names are not rejected. The parameter frame is built in order, so the last
occurrence wins:

```lovelace
func f(a, a) = a; f(1, 2)
```
```text
= 2 (Natural)
  _ = 2
```

This is a sharp edge rather than a feature; give parameters distinct names.

## Scope and shadowing

The engine has three kinds of scope:

1. the **global** store (top-level variables and function definitions);
2. **block** scopes created by `{ ... }`;
3. **function frames** created by a call.

Lookup walks the live scope chain from innermost to outermost, then the globals. Assignment
updates the nearest existing binding; a new name is defined in the innermost scope. A parameter
is a binding in the function frame, so it shadows a global with the same name:

```lovelace
x = 10; func f(x) = x + 1; f(1); x
```
```text
= 10 (Natural)
  _ = 10
  x = 10
```

The call returned `2`; the final `x` is still the global `10`. If the function body assigns a name
that already exists globally and is not shadowed, the assignment updates the global. There is no
`local` keyword and no way to opt out of this:

```lovelace
x = 10; func f() { x = 20 }; f(); x
```
```text
= 20 (Natural)
  _ = 20
  x = 20
```

A name first assigned inside a function body is local to the frame (or to an inner block of the
body) and does not leak:

```lovelace
func f() { y = 5; y }; f(); y
```
```text
Error [InvalidOperation/DomainError]: Undefined variable 'y'.
```

Block scopes are lexical and nested. A block-local name disappears when the block exits, while an
assignment to an outer name updates the outer binding. The full block rules are on
[Statements and control flow](statements-and-control-flow.md#blocks).

### Function frames are not closures

This is the most important scope rule: **a function frame's parent is the global scope, not the
caller's frame**. A callee cannot see the caller's block locals or parameters. The engine saves
the caller's scope chain, installs a fresh frame whose only parent is the global store, runs the
body, and restores the caller's chain.

```lovelace
{ a = 1; func f() = a; f() }
```
```text
Error [InvalidOperation/DomainError]: Undefined variable 'a'.
```

`f` was defined inside a block that binds `a`, but `f` does not capture `a`. This also means there
are no closures, no captured environments, and no higher-order functions. The only way for a
function to see caller state is through a global binding.

### Function definitions are global, not lexical

Despite the scope chain, the `funcs` table is global. Defining a function inside a block or
inside another function registers it globally and makes it callable after the definition
executes:

```lovelace
func outer() { func inner(x) = x + 1; inner(1) }; outer(); inner(2)
```
```text
= 3 (Natural)
  _ = 3
```

The first call `inner(1)` runs inside `outer`; the second `inner(2)` is a top-level call after
`outer` returned. Both resolve the same global definition. The definition still happens at run
time: a call before the defining statement is unknown.

## Recursion

Functions can call themselves. There is no `rec` keyword and no special syntax; the name is
resolved in the global function table.

```lovelace
func fact(n) { if (n <= 1) { 1 } else { n * fact(n - 1) } }; fact(6)
```
```text
= 720 (Natural)
  _ = 720
```

```lovelace
func fib(n) { if (n < 2) { n } else { fib(n - 1) + fib(n - 2) } }; fib(10)
```
```text
= 55 (Natural)
  _ = 55
```

Recursion is bounded by the evaluation-depth budget, not by a fixed call-count limit. Each call
adds a frame's units plus the statements and descending expressions its body executes. A
sufficiently deep recursion is refused with `DepthExceeded` / `BudgetExceeded` before the native
stack is exhausted; the observed maximum is 512 depth units per top-level statement.

```lovelace
func f(n) { if (n == 0) { 0 } else { f(n - 1) } }; f(200)
```
```text
Error [DepthExceeded/BudgetExceeded]: evaluation depth 513 exceeds the maximum supported evaluation depth of 512. Rewrite the input with less nesting (the engine refuses it before a recursive walk can exhaust the process stack).
```

The exact argument at which this fires depends on the body's statement and expression shape. If
you need a larger recursion depth, rewrite the algorithm iteratively (loops are not depth-priced
the same way) rather than relying on a particular threshold.

## User functions versus builtins

| Property | User function | Builtin |
|---|---|---|
| Declaration | `func name(params) …` in the script | Registered in the engine (`registry.rs` / a plugin) |
| Name resolution | Checked before builtins, so a user function shadows a builtin of the same name | Resolved after user functions |
| Arity | Exact parameter count; no defaults, no variadics | Declared `minArity` and `parameterCount`; some are variadic |
| Arity failure | `InvalidOperation` / `DomainError` | `InvalidArgument` / `TypeMismatch` with a `signature` |
| First-class value | Yes: a bare name evaluates to a `Function` value | No: a bare builtin name is an undefined variable |
| Callable through a variable | No | No |
| Recursion | Yes | Implemented internally |
| Closures | No | Not applicable |
| Lookup before definition | Unknown function | Available from engine start |

User functions shadow builtins because call resolution checks the user table first:

```lovelace
func sqrt(x) = 99; sqrt(4)
```
```text
= 99 (Natural)
  _ = 99
```

A builtin is not a value. `sqrt` without a call is an undefined variable:

```lovelace
sqrt
```
```text
Error [InvalidOperation/DomainError]: Undefined variable 'sqrt'.
```

A user function is a value when referenced without a call, but that value cannot be invoked
through a variable. Calls are always by name and always look in the function tables, not in the
variable store:

```lovelace
func f(x) = x + 1; g = f; g(1)
```
```text
Error [UnknownFunction/UnsupportedOperation]: Unknown function 'g'.
```

```lovelace
func f(x) = x + 1; f
```
```text
= <function f> (Function)
  _ = <function f>
```

This is the practical distinction: builtins are callable names, user functions are callable names
**and** first-class values, and neither can be called through an arbitrary expression.

## Explicit absence

The function system has no:

- closures or captured environments;
- lambdas, anonymous functions, or function literals;
- default parameters, optional parameters, named arguments, or variadic user functions;
- overloading (same name with different arities) — the last definition replaces the first;
- generics, type parameters, or type annotations;
- `async`/`await`, generators, or coroutines;
- pass-by-reference parameters;
- higher-order builtins that take a function value;
- tail-call optimisation (recursion is still depth-bounded).

If you need a callback-like abstraction, encode the choice in data and use `if`/`else` or a
dispatch function written with explicit branches.

# Values and types

Lovelace is dynamically typed: a name holds a value, and the value carries its kind. There is no
declaration syntax and no type annotation. This page lists every runtime value kind, the name
`type()` reports for it, how exactness works, how numeric values widen, and what the language does
**not** provide (truthiness, a separate rational kind, function literals).

## The kind table

`type(x)` returns a `Text` value whose content is the kind or record-type name. The typed display
(`<value> (Kind)`) uses the same name, except for `Text` and enum values, which print without a
suffix.

| Value kind | `type(x)` | Example | Notes |
|---|---|---|---|
| Natural | `Natural` | `42` | Arbitrary-precision non-negative integer. |
| Integer | `Integer` | `-5` | Arbitrary-precision signed integer; `-0` is Integer 0. |
| Real | `Real` | `0.(3)`, `3.14` | Arbitrary-precision decimal with optional exact period metadata. |
| Boolean | `Boolean` | `1 < 2` | Produced by comparisons and `not`; there is no Boolean literal. |
| Text | `Text` | `"hello"` | Strings and interpolated strings; displayed without a kind suffix. |
| Vector | `Vector` | `[1, 2, 3]`, `1..5` | Rank-1 array. |
| Array | `Array` | `[[1, 2], [3, 4]]` | Rank ≥ 2 array. |
| Complex | `Complex` | `fft([0,1,0,0])[1]` | Pair of Reals; no complex literal in the grammar. |
| Symbolic | `Symbolic` | `symbol("x")^2 + 1` | Symbolic expression tree. |
| Record | its declared name, e.g. `DivRemResult` | `divrem(17, 5)` | Named fields; member access with `.`. |
| Enum | its declared enum name, e.g. `SolveStatus` | `solve(…).status` | Declared variant; displays as the bare member name. |
| Domain | `Domain` | `real`, `complex` | The four domain constants used by `solve` and `symbol`. |
| Function | `Function` | `func f(x) = x + 1; f` | User-defined function value; builtins are not values. |
| Void | `Void` | `print("hi")`, `setprecision(20)` | Absence of a value; displayed as `(void)` at top level. |

A single script can demonstrate most of the scalar and container names:

```lovelace
[type(1), type(-1), type(1/2), type(1<2), type("hi"), type([1,2]), type([[1,2]]), type(divrem(7,2)), type(real), type(true)]
```
```text
= [Natural, Integer, Real, Boolean, Text, Vector, Array, DivRemResult, Domain, Natural] (Vector)
  _ = [Natural, Integer, Real, Boolean, Text, Vector, Array, DivRemResult, Domain, Natural]
```

The remaining kinds:

```lovelace
[type(fft([0,1,0,0])[1]), type(symbol("x")), type(solve_full(symbol("x")^2 - 4 == 0, symbol("x")))]
```
```text
= [Complex, Symbolic, SolveResult] (Vector)
  _ = [Complex, Symbolic, SolveResult]
```

Notice the last two entries: a structured result's `type()` is its **record type name**, not
`Record`, and an enum's `type()` is its **enum type name**, not `Enum`.

```lovelace
x = symbol("x"); s = solve(x^2 - 4 == 0, x); [s.status, type(s.status)]
```
```text
= [Solved, SolveStatus] (Vector)
  _ = [Solved, SolveStatus]
  s = SolveResult(status: Solved, variable: x, domain: complex, complete: True, completeness: Complete, solutions: [Solution(value: -2, conditions: [], multiplicity: 1, exactness: Exact), Solution(value: 2, conditions: [], multiplicity: 1, exactness: Exact)], families: [], common_conditions: [], represented_count: 2, unrepresented_count: 0, unrepresented_reason: , diagnostics: [])
  x = x
```

## Numeric kinds and widening

The three numeric kinds form a widening chain:

```text
Natural → Integer → Real
```

Arithmetic picks the wider kind of its operands and returns that kind, with two refinements:

- Subtraction of two Naturals widens to Integer only when the mathematical difference is
  negative; otherwise it stays Natural.
- Division of two exact integers returns a Natural/Integer when the quotient is exact, and a
  Real otherwise. `100 / 4` is Natural `25`; `1 / 4` is Real `0.25`; `1 / 3` is Real `0.(3)`.

```lovelace
[type(1 + 2), type(1 - 2), type(1 + 0.5), type(100 / 4), type(2 ^ -2), type(-0)]
```
```text
= [Natural, Integer, Real, Natural, Real, Integer] (Vector)
  _ = [Natural, Integer, Real, Natural, Real, Integer]
```

A negative literal is unary minus applied to a Natural, so `-5` is Integer `-5`; `-0` is Integer
`0`. A negative exponent always produces a Real (`2 ^ -2` is `0.25`) because the result is a
reciprocal, while a non-negative integer exponent preserves the base kind (`2 ^ 100` is Natural).

Inside one list literal, all numeric elements are promoted to the widest numeric kind present.
That keeps a vector homogeneous:

```lovelace
[type([1, 2.5, 3][0]), type([1, 2.5, 3][1])]
```
```text
= [Real, Real] (Vector)
  _ = [Real, Real]
```

Lists are not otherwise type-homogeneous: a vector can hold a Natural and a Text, and an array can
hold mixed rows as long as every row has the same shape.

```lovelace
[1, "a"]
```
```text
= [1, a] (Vector)
  _ = [1, a]
```

## Exactness

`Natural` and `Integer` are always exact. A `Real` is exact when it is a finite decimal or a
periodic decimal the engine can carry exactly. It is inexact when it is a truncation of an
irrational or transcendental computation, or when a division's period did not fit inside the
active computation budget.

The `inspect()` builtin exposes an `exact` field:

```lovelace
[inspect(1/3).exact, inspect(sqrt(2)).exact, inspect(1.5).exact, inspect(pi()).exact, inspect(2).exact]
```
```text
= [True, False, True, False, True] (Vector)
  _ = [True, False, True, False, True]
```

The display form does not show exactness; `0.(3)` and `sqrt(2)` are both rendered as `Real`.
Inspect the value, or read the `structured.exact` field of the JSON envelope, when the
distinction matters. `RealClamped` is an internal wire variant used when a builtin's own digit cap
cut a value; it reports `Real` at every language surface.

Periodic decimals are exact rational values in disguise, and arithmetic on them closes:

```lovelace
[0.(3) * 3, 0.(3) + 0.(6), 1/3 == 0.(3)]
```
```text
= [1, 1, True] (Vector)
  _ = [1, 1, True]
```

### Is there a Rational kind?

No. There is no `Value::Rational`, `type()` never reports `Rational`, and a rational literal such
as `1/3` is a `Real` carrying exact period metadata. The name `rational` is a **Domain** value
used to declare symbolic variables and to talk about solver domains:

```lovelace
[real, complex, integer, rational, type(real), type(complex), type(integer), type(rational)]
```
```text
= [real, complex, integer, rational, Domain, Domain, Domain, Domain] (Vector)
  _ = [real, complex, integer, rational, Domain, Domain, Domain, Domain]
```

`symbol("q", rational)` produces a Symbolic variable with that domain, not a Rational value.
Exact rational arithmetic inside symbolic expressions is carried by the Symbolic tree (rational
constants), and the numeric tier represents exact rationals as periodic Reals.

## Boolean

`Boolean` has exactly two values, `True` and `False`. It is produced by comparison operators and
by the `not` builtin; there is no way to write a Boolean literal. Lowercase `true` and `false` are
numeric literals in disguise (see below), and capitalized `True`/`False` are undefined names.

```lovelace
[1 < 2, 2 < 1, 1 == 1, 1 != 2]
```
```text
= [True, False, True, True] (Vector)
  _ = [True, False, True, True]
```

Comparisons are numeric in the general case. Equality falls back to structural equality for
non-numeric values, but a **non-equal** comparison of non-numerics is a domain error rather than a
`False`: `[1, 2] == [1, 2]` is `True`, while `[1, 2] == [1, 3]` and `"a" < "b"` fail with
"Comparison needs two numeric values." Comparison chains such as `1 < 2 < 3` parse as
`(1 < 2) < 3` and fail at evaluation for the same reason.

There is no truthiness. `if`, `while`, and the prefix `not` operator require an actual Boolean:

```lovelace
if (1) { 2 }
```
```text
Error [InvalidOperation/DomainError]: if condition must be Boolean, but got 'Natural'.
```

The lowercase spellings of the booleans are the most surprising corner of the language:

```lovelace
[true, false, type(true)]
```
```text
= [1, 0, Natural] (Vector)
  _ = [1, 0, Natural]
```

`true` evaluates to Natural 1 and `false` to Natural 0. Consequently `if (true)` fails with the
same "must be Boolean" sentence. Write conditions as comparisons.

`and` and `or` are **symbolic relation constructors**, not Boolean combinators. They expect
symbolic expressions (typically relations over a symbolic variable) and refuse two Boolean
values:

```lovelace
and(1 < 2, 2 < 3)
```
```text
Error [InvalidOperation/DomainError]: Logical operators need expressions.
```

Use nested `if`/`else` to combine Boolean conditions. `not` does accept a Boolean and returns its
negation: `not(1 < 2)` is `False`. The prefix form `not b` and the call form `not(b)` are both
available; `not` is also a builtin.

## Text

Text values are double-quoted strings or interpolated strings. They display without quotes and
without a kind suffix, which is why a result line for a Text value is just the text itself:

```lovelace
"hello"
```
```text
= hello
  _ = hello
```

`type("hi")` is `Text`. Interpolation embeds the **display** rendering of an expression, so no
`(Kind)` suffix leaks into the text. A `Text` value can be indexed and measured with `len`, but it
is not a `Vector`; use `len`, not array indexing, for its length.

## Vector and Array

A list literal is a `Vector` when it is rank 1, and an `Array` when nesting makes it rank ≥ 2.
`1..5` is a Vector; `[[1, 2], [3, 4]]` is an Array; `[[[1,2],[3,4]],[[5,6],[7,8]]]` is a rank-3
Array. A nested literal must be rectangular: ragged rows are refused rather than padded.

```lovelace
[[1, 2], [3]]
```
```text
Error [InvalidOperation/DomainError]: Ragged nested list literal: every row must have the same shape.
```

`rank()` (or `ndims()`) reports the number of dimensions; `shape()` returns a Vector of extents;
`numel()` returns the total element count; `len()` returns the first dimension's extent. A
rank-0 array produced by `squeeze` still holds one element. `Vector` and `Array` are distinct for
`type()` but share the same arithmetic: operators apply element-wise with right-aligned
broadcasting and scalar broadcast. See [Expressions and operators](expressions-and-operators.md#arrays-and-broadcasting).

Arrays can contain values of any kind, not only numbers. The display shows their display forms:

```lovelace
[[1, 2], [3, "a"]]
```
```text
= [[1, 2], [3, a]] (Array)
  _ = [[1, 2], [3, a]]
```

## Complex

The grammar has no imaginary literal: `1+2i` is a parse error, and `i` is not a predefined
variable. Complex values enter the language through the DSP and complex-math builtins. They are
pairs of exact Reals, and `re`, `im`, `conj`, and `abs` bridge them back to the numeric kinds.

```lovelace
fft([0, 1, 0, 0])
```
```text
= [1, -1i, -1, 1i] (Vector)
  _ = [1, -1i, -1, 1i]
```

The display rules omit a zero part: a Complex with zero imaginary part renders as its real
number (with kind `Complex` at the typed level), a pure imaginary value renders as `<im>i`, and a
general value renders as `<re> ± <im>i`. `type(fft([0,1,0,0])[1])` is `Complex`.

## Symbolic

Symbolic values are expression trees. They are created explicitly with `symbol("x")`; an
unbound single-letter name is an ordinary undefined variable, not an implicit symbol. Symbolic
arithmetic builds expressions rather than evaluating them, and the calculus, simplification,
solving, compilation and optimization builtins consume and return them.

```lovelace
x = symbol("x"); x^2 + 1
```
```text
= x^2 + 1 (Symbolic)
  _ = x^2 + 1
  x = x
```

Symbolic values have their own `exact` flag (visible through `inspect`) and their own domain
metadata. They do not widen to numeric kinds; use `subs` to substitute a numeric value and
`evalf` to numerically evaluate an expression. `inf` is the symbolic positive-infinity constant.

## Record

A record is a named product with named fields. Structured builtins return records rather than
sentences, so a program can branch on structure: `divrem` returns `DivRemResult(quotient,
remainder)`, `solve` returns `SolveResult`, `limit` returns `LimitResult`, and the `*_full`
variants return the richer variants of the same records.

```lovelace
divrem(17, 5)
```
```text
= DivRemResult(quotient: 3, remainder: 2) (DivRemResult)
  _ = DivRemResult(quotient: 3, remainder: 2)
```

Member access reads a field; a missing field names the available ones:

```lovelace
divrem(17, 5).remainder
```
```text
= 2 (Natural)
  _ = 2
```

`type(divrem(17, 5))` is `DivRemResult`. Member access is only for records; applying `.` to a
Symbolic or numeric value is an error with a targeted message.

## Enum

Enums are declared variants such as `SolveStatus.Solved`, `Completeness.Complete`, and
`SolutionExactness.Exact`. They are values with a declared type name and a member name; `type()`
returns the declared type, and display returns the member name.

```lovelace
x = symbol("x"); s = solve(x^2 - 4 == 0, x); s.status
```
```text
= Solved
  _ = Solved
  s = SolveResult(status: Solved, variable: x, domain: complex, complete: True, completeness: Complete, solutions: [Solution(value: -2, conditions: [], multiplicity: 1, exactness: Exact), Solution(value: 2, conditions: [], multiplicity: 1, exactness: Exact)], families: [], common_conditions: [], represented_count: 2, unrepresented_count: 0, unrepresented_reason: , diagnostics: [])
  x = x
```

Enums are compared structurally (`s.status == s.status` is `True`) but cannot be constructed by
the script; they originate in structured results.

## Function

Only user-defined functions have value form. A `func` name used without a call evaluates to a
`Function` value whose display is `<function name>`:

```lovelace
func f(x) = x + 1; [f, type(f)]
```
```text
= [<function f>, Function] (Vector)
  _ = [<function f>, Function]
```

Builtins are **not** first-class values: `sqrt` without a call is an undefined variable, not a
Function. Function values are not callable either; `g = f; g(1)` is an unknown-function call
because calls are resolved by name. See [Functions and scope](functions-and-scope.md).

## Void

`Void` means "no value". It is the result of statements that produce nothing: `print`,
`setprecision`, a `func` definition, an `if` with no matching branch, and a loop with no
completed body value. At top level `Void` displays as `(void)` and is not stored in `_`.

```lovelace
print("hi")
```
```text
= (void)
hi
```

`type(print("hi"))` is `Void`, and the printed line is captured separately in the `output` array
of the JSON envelope.

## What the type system does not have

- No truthiness: numbers, strings, arrays and records are not conditions.
- No rational value kind: exact rationals are Reals with periods or symbolic constants.
- No IR value kind: `lower`/`compile` return the serialized MathIR as `Text`, and `compile_full`
  returns a `CompilationResult` record containing it. MathIR is a wire format, not a script value.
- No implicit symbol creation: `x` must be assigned or created with `symbol("x")`.
- No Boolean literals: `true`/`false` are Naturals 1/0; use comparisons.
- No function literals, lambdas, or higher-order calls: function values exist but cannot be
  invoked through a variable.
- No user-defined records, enums, classes, or interfaces: those types are produced by builtins.
- No implicit conversion between Text and numbers and no truthiness-based coercion. Builtins
  perform their own checked argument conversions, and mismatches are typed errors.

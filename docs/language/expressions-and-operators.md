# Expressions and operators

Expressions are built from literals, names, calls, indexing, member access, and a small set of
prefix, infix, and postfix operators. This page gives the complete operator table derived from the
parser in `lovelace-lang/src/syntax.rs`, proves each precedence level with an executed example,
and describes the semantics of every operator family.

## Grammar

The parser is a recursive-descent parser. In approximate EBNF, the expression grammar is:

```text
expression   := assignment
assignment   := IDENT "=" expression | comparison
comparison   := additive (("==" | "!=" | "<" | "<=" | ">" | ">=") additive)*
additive     := multiplicative (("+" | "-") multiplicative)*
multiplicative := power (("*" | "/" | "%") power)*
power        := ("-" | "+") unary ("^" power)? (".." unary (".." unary)?)?
              | range ("^" power)?
range        := unary (".." unary (".." unary)?)?
unary        := ("-" | "+") unary | "not" unary | postfix
postfix      := primary ("(" arguments ")" | "[" index "]" | "!" | "." IDENT)*
primary      := NUMBER | PERIODIC | STRING | INTERPOLATED
              | IDENT | "true" | "false" | "(" expression ")" | "[" list "]"
```

This is close to the reference grammar, with one shape that surprises readers: `range` is parsed
as the operand of `power`, so a range can appear on the **left** of `^` and also as the exponent's
own expression. The sign handling at the start of `power` is described below.

## Precedence and associativity

From loosest binding to tightest:

| Level | Operators | Associativity | Parser rule |
|---|---|---|---|
| 1 | `=` assignment | right | `assignment` (target must be a plain identifier) |
| 2 | `==` `!=` `<` `<=` `>` `>=` | left (chains parse, then usually fail) | `comparison` |
| 3 | `+` `-` binary | left | `additive` |
| 4 | `*` `/` `%` | left | `multiplicative` |
| 5 | `^` power | right | `power` / `power_tail` |
| 6 | `..` range | non-associative (one or two `..`) | `range` / `range_tail` |
| 7 | unary `-` `+`, prefix `not` | right | `unary` |
| 8 | postfix `!`, `[ … ]`, `.name` | left-to-right | `postfix` |
| 9 | primary: literals, `( … )`, `[ … ]`, names | — | `primary` |

The assignment level is not an expression operator in the usual sense: the parser only recognises
`IDENT =` at the start of an expression. That makes `x = y = 5` right-associative, while
`(x = 1) + 2` is accepted because a parenthesised expression may itself be an assignment.

### Proving the levels

Arithmetic precedence:

```lovelace
2 + 3 * 4
```
```text
= 14 (Natural)
  _ = 14
```

Left associativity of subtraction and division:

```lovelace
[10 - 3 - 2, 100 / 10 / 2]
```
```text
= [5, 5] (Vector)
  _ = [5, 5]
```

Right associativity of power:

```lovelace
2 ^ 3 ^ 2
```
```text
= 512 (Natural)
  _ = 512
```

Multiplication binds looser than power:

```lovelace
[2 * 3 ^ 2, 2 ^ 3 * 2]
```
```text
= [18, 16] (Vector)
  _ = [18, 16]
```

Comparisons bind looser than arithmetic:

```lovelace
1 + 2 < 4
```
```text
= True (Boolean)
  _ = True
```

Assignment is loosest and right-associative:

```lovelace
x = y = 5; x + y
```
```text
= 10 (Natural)
  _ = 10
  x = 5
  y = 5
```

A range binds tighter than surrounding arithmetic. `1 + 1..3` is `1 + (1..3)`, not
`(1 + 1)..3`:

```lovelace
1 + 1..3
```
```text
= [2, 3, 4] (Vector)
  _ = [2, 3, 4]
```

```lovelace
2 * 1..5
```
```text
= [2, 4, 6, 8, 10] (Vector)
  _ = [2, 4, 6, 8, 10]
```

The power operator sees the range as its operand, so `1..10 ^ 2` squares the vector:

```lovelace
1..10 ^ 2
```
```text
= [1, 4, 9, 16, 25, 36, 49, 64, 81, 100] (Vector)
  _ = [1, 4, 9, 16, 25, 36, 49, 64, 81, 100]
```

Postfix factorial binds tightest:

```lovelace
[3!!, 2^3!, -3!]
```
```text
= [720, 64, -6] (Vector)
  _ = [720, 64, -6]
```

`3!!` is `(3!)!` = `720`; `2^3!` is `2^(3!)` = `64`; `-3!` is `-(3!)` = `-6`.

### The sign rule at power level

A leading `-` or `+` at the start of a power expression is consumed by the **power** parser, not
the unary parser. Its operand is a unary expression, and the sign is applied after any `^` that
follows:

- `-2^2` is `-(2^2)` = `-4`, not `(-2)^2`.
- `2^-2` is `2^(-2)` = `0.25`; the exponent may carry its own sign.
- `-1..2` is `(-1)..2`: the sign applies to the first range endpoint, not to the whole range.
- `-3!` is `-(3!)`, because postfix factorial is part of the operand.
- Everywhere else, `-` and `+` are ordinary right-associative unary operators.

```lovelace
[-2^2, (-2)^2, 2^-2, 2^3!]
```
```text
= [-4, 4, 0.25, 64] (Vector)
  _ = [-4, 4, 0.25, 64]
```

Parenthesise when you mean the other grouping. `(-2)^2` is `4`, and `-(2^2)` is `-4`.

The one special interaction with a range is worth stating separately: `2^3..4` parses as
`2^(3..4)`, because the exponent parser accepts a full range. It is refused as a non-integer
exponent rather than producing `(2^3)..4`:

```lovelace
2^3..4
```
```text
Error [UnsupportedOperation/UnsupportedOperation]: Non-integer exponents are not yet supported.
```

Write `(2^2)..6` for a range of powers:

```lovelace
(2^2)..6
```
```text
= [4, 5, 6] (Vector)
  _ = [4, 5, 6]
```

### `not` is a unary operator, not a comparison modifier

`not` binds at unary level, tighter than comparisons. `not 1 == 2` is `(not 1) == 2`, which fails
because `1` is not Boolean; `not (1 == 2)` either calls the `not` builtin (when the parenthesis
follows the name) or negates the grouped Boolean. Parenthesise the comparison:

```lovelace
not (1 == 2)
```
```text
= True (Boolean)
  _ = True
```

```lovelace
not 1 == 2
```
```text
Error [InvalidOperation/DomainError]: not needs a Boolean, but got 'Natural'.
```

## Assignment

`name = expression` evaluates the expression, stores it in the nearest existing binding, or
defines it in the current scope when the name is new. The assignment expression yields the stored
value, so it can be chained or used as an argument:

```lovelace
func f(a) = a; f(x = 7); x
```
```text
= 7 (Natural)
  _ = 7
  x = 7
```

Only a plain identifier can be assigned. `a[0] = 3`, `record.field = 3`, and `x += 1` are not
assignments in this grammar; they fail at parse time (the first two because the target is not an
identifier, the last because `+=` is two tokens, `+` and `=`).

```lovelace
a = [1, 2]; a[0] = 3
```
```text
Error [ParseError/ParseError]: Expected ';' or end of input but found '=' at position 17.
```

There is no destructuring assignment and no declaration keyword. See
[Statements and control flow](statements-and-control-flow.md#assignment-versus-definition) for the
scope rules.

## Arithmetic

### Addition, subtraction, multiplication

`+`, `-`, and `*` apply element-wise to arrays with broadcasting and a scalar broadcast. On two
scalars they widen to the wider numeric kind. Natural subtraction that would go negative widens to
Integer. A subtraction of Naturals that stays non-negative stays Natural.

```lovelace
[1 + 2, 7 - 3, 3 - 5, 2 * 3]
```
```text
= [3, 4, -2, 6] (Vector)
  _ = [3, 4, -2, 6]
```

There is no Text concatenation with `+` and no implicit numeric/Text conversion:

```lovelace
"a" + "b"
```
```text
Error [InvalidOperation/DomainError]: Cannot apply '+' to 'Text' and 'Text'.
```

### Division

Division is exact. Two exact integers with an exact quotient narrow back to a Natural or Integer;
otherwise the result is a Real. A non-terminating decimal is stored with its exact period:
`1 / 3` is `0.(3)`, not a rounded value. `1 / 4` is `0.25` (a Real, because the operands were not
both exact-integer quotients that narrow; the value is still exact).

Division by zero is a `DivisionByZero` / `DomainError` failure. The sentence differs slightly by
tier: two exact operands say "Cannot divide by zero.", while a Real operand says "Cannot divide a
Real by zero."

### Remainder

`%` is the remainder. For integers its sign follows the **quotient**, not the dividend:

```lovelace
[-7 % 3, 7 % -3, 17 % -5]
```
```text
= [-1, -1, -2] (Vector)
  _ = [-1, -1, -2]
```

For Reals it is the truncated remainder `left - trunc(left / right) * right`. Division or
remainder by zero is refused with the division-by-zero taxonomy.

### Power

`^` requires an integer exponent in general. A negative integer exponent produces the Real
reciprocal: `2 ^ -2` is `0.25`. A non-integer exponent is refused:

```lovelace
2^0.5
```
```text
Error [UnsupportedOperation/UnsupportedOperation]: Non-integer exponents are not yet supported.
```

Two exact special cases are supported. A zero base under a positive non-integer exponent is the
Real zero, and a negative base under the principal square root becomes a Symbolic complex value:

```lovelace
[0^0.5, (-4)^0.5]
```
```text
= [0, 2*i] (Vector)
  _ = [0, 2*i]
```

A zero base under a negative exponent is an `InvalidArgument` / `TypeMismatch` refusal ("Base
cannot be zero."). An exponent that does not fit the supported integer range is refused by the
implementation rather than silently wrapped.

### Factorial

Postfix `!` is defined for Natural and non-negative Integer values. `0!` is `1`. Factorial of a
negative integer has its own domain sentence; factorial of a Real is refused by kind. Repeated
postfix factorial is allowed (`3!!` = `720`).

```lovelace
[5!, 0!]
```
```text
= [120, 1] (Vector)
  _ = [120, 1]
```

```lovelace
(-3)!
```
```text
Error [InvalidOperation/DomainError]: Factorial is not defined for negative integers.
```

## Comparison and equality

`==`, `!=`, `<`, `<=`, `>`, and `>=` return Boolean. Numeric comparisons are exact across the
widening chain: `1 == 1.0` is `True`, and a rational/periodic value compares equal to any value
with the same exact numeric content.

Comparisons are **left-associative in the parser** but not chainable in the usual mathematical
sense. `1 < 2 < 3` parses as `(1 < 2) < 3` and fails because a Boolean is not numeric:

```lovelace
1 < 2 < 3
```
```text
Error [InvalidOperation/DomainError]: Comparison needs two numeric values.
```

For non-numeric values the runtime falls back to structural equality, but only when the two values
are equal. A non-equal comparison of non-numerics is a domain error, not `False`:

```lovelace
[1, 2] == [1, 2]
```
```text
= True (Boolean)
  _ = True
```

```lovelace
[1, 2] == [1, 3]
```
```text
Error [InvalidOperation/DomainError]: Comparison needs two numeric values.
```

The same applies to Text and records. This is a runtime quirk worth knowing when writing generic
code: test equality only on values whose kinds you know, or use `type()` first.

## Ranges

`a..b` builds the inclusive range from `a` to `b` with step 1. `a..step..b` uses an explicit step.
The result is always a `Vector`. The default step is always `+1`, so a descending pair with no
explicit negative step is empty (`1..0` is `[]`); use `5..-1..1` to descend.

```lovelace
1..2..7
```
```text
= [1, 3, 5, 7] (Vector)
  _ = [1, 3, 5, 7]
```

```lovelace
1..0
```
```text
= [] (Vector)
  _ = []
```

```lovelace
5..-1..1
```
```text
= [5, 4, 3, 2, 1] (Vector)
  _ = [5, 4, 3, 2, 1]
```

A list literal whose elements are themselves vectors is promoted to a rank-2 array, so the rows
must have equal length. Three ranges of different lengths cannot be written as one literal; keep
them as separate values or make the rows the same length.

Range bounds must be exact Natural or Integer values. An exactly integral Real is refused by kind
and told to be written as an integer:

```lovelace
1.0..3
```
```text
Error [InvalidArgument/TypeMismatch]: range(): argument 'bound' must be a Natural or Integer; got the Real 1, which is integral — write it as 1.
```

The step may be a Real, including a fractional step. A zero step is refused; a range longer than
one million elements is refused with "Range is too large." A stepped range's integral elements are
Integer values and its fractional elements are Real values, so the result can be a mixed Vector:

```lovelace
1..0.5..3
```
```text
= [1, 1.5, 2, 2.5, 3] (Vector)
  _ = [1, 1.5, 2, 2.5, 3]
```

A range has at most two `..` tokens. `1..2..3` means start 1, step 2, end 3; `1..2..3..4` is a
parse error. An open range such as `1..` or `..5` is not a value.

## Indexing and slicing

Indexing is postfix `target[spec]`. For an array, the brackets contain one spec per axis,
separated by commas. A spec is one of:

| Spec | Meaning |
|---|---|
| `i` | One index on this axis (0-based, must be an exact non-negative integer). |
| `start:stop` | A slice, stop exclusive. |
| `start:stop:step` | A strided slice; step must be a positive integer. |
| `:` | The whole axis. |
| `start:` / `:stop` / `::step` | Any bound or step may be omitted. |

A single index removes the axis from the result; a slice or `:` keeps it. Fewer specs than axes
leaves the trailing axes intact. A vector consumes one axis; extra bracket arguments on a vector
are ignored (the vector path reads only the first spec). More indices than an array's rank is
"Too many indices for this array."

```lovelace
[10, 20, 30][1]
```
```text
= 20 (Natural)
  _ = 20
```

```lovelace
[0, 1, 2, 3, 4][::2]
```
```text
= [0, 2, 4] (Vector)
  _ = [0, 2, 4]
```

```lovelace
[[1, 2], [3, 4]][1, 0]
```
```text
= 3 (Natural)
  _ = 3
```

A partial index returns a lower-rank value:

```lovelace
[[1, 2], [3, 4]][1]
```
```text
= [3, 4] (Vector)
  _ = [3, 4]
```

A slice keeps the axis:

```lovelace
[[1, 2, 3], [4, 5, 6]][:, 1]
```
```text
= [2, 5] (Vector)
  _ = [2, 5]
```

Single indices do **not** accept negative values; out-of-range indices are errors. Slice bounds,
by contrast, may be negative and are adjusted by the axis length, then clipped to the axis:

```lovelace
[1, 2, 3, 4][-2:]
```
```text
= [3, 4] (Vector)
  _ = [3, 4]
```

```lovelace
[1, 2, 3][-1]
```
```text
Error [InvalidOperation/DomainError]: Index -1 is out of range for vector of length 3.
```

Slice steps must be positive. Zero and negative steps are refused with the range-step argument
sentence (the slice implementation routes the step through the same positive-count contract):

```lovelace
[1, 2, 3][0:3:-1]
```
```text
Error [InvalidArgument/TypeMismatch]: range(): argument 'step' must be a Natural or Integer that is a positive count (1 or more, at most 9223372036854775807); got -1.
```

Text is not indexable with brackets; `len("abc")` works, but `"abc"[1]` is "Cannot index a value
of kind 'Text'." A scalar is likewise not indexable.

## Calls

A call is `name(arguments)`. The callee must be a **name**, not an arbitrary expression: the
parser rejects `"x"(2)` and `[1, 2](3)` with "Only a named function can be called." Arguments are
comma-separated expressions and may themselves contain assignments. A trailing comma is not
allowed.

User functions and builtins are both called by name. Calls are checked for arity: builtins use
the registry's declared minimum/maximum and return `InvalidArgument` / `TypeMismatch` on a count
mismatch, while user functions use their parameter count and return
`InvalidOperation` / `DomainError`. See [Functions and scope](functions-and-scope.md#arity-and-errors).

The tokenizer's periodic-literal rule has one side effect at the call site: `1(2)` is tokenized
as a periodic literal `1(2)`, not as a call of `1` with argument `2`, and fails as an invalid
number literal. Names are the only callable heads.

## Member access

`record.field` reads a named record field. It is a postfix operator and can be chained when a
field is itself a record or array. It is not a namespace separator and does not work on Symbolic
values, arrays, numbers, or Text:

```lovelace
divrem(17, 5).remainder
```
```text
= 2 (Natural)
  _ = 2
```

A missing field names the record and its available members. A non-record target gets a
"member access requires a record result" sentence; for Symbolic values the engine adds a hint to
call the `*_full` builtin instead.

## Arrays and broadcasting

`+ - * / % ^` and comparisons apply element-wise to arrays. Scalars broadcast across arrays, and
two arrays broadcast right-aligned: matching extents, or an extent of 1 on either side. A
mismatch is a `DomainError` naming both shapes.

```lovelace
[1, 2, 3] * 10
```
```text
= [10, 20, 30] (Vector)
  _ = [10, 20, 30]
```

```lovelace
[1, 2] + [[1, 2], [3, 4]]
```
```text
= [[2, 4], [4, 6]] (Array)
  _ = [[2, 4], [4, 6]]
```

```lovelace
[1, 2] + [1, 2, 3]
```
```text
Error [InvalidOperation/DomainError]: Operands could not be broadcast together with shapes [2] and [3].
```

A vector/vector operation stays a Vector; any operation involving a rank ≥ 2 Array produces an
Array. Division on containers narrows to all-integer elements only when every element is an exact
integer; otherwise the container is all Real, so an element's kind never depends on the data.

## Operators that do not exist

- No `&&`, `||`, or `!` negation; `!` is postfix factorial only.
- No bitwise operators (`&`, `|`, `^` is power, `~`, `<<`, `>>`).
- No increment/decrement (`++`, `--`) and no compound assignment (`+=`, `-=`, …).
- No ternary `?:` and no null-coalescing operator.
- No string concatenation operator; use interpolation or a host-side join.
- No user-overloadable operators.
- No comma operator; a comma separates arguments, list elements, and index specs.

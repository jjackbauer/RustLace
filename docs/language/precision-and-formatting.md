# Precision and formatting

Lovelace numbers use an exact decimal representation until an operation has to truncate: division
with a period longer than the computation budget, an irrational square root, a transcendental
function, `pi`, `e`, or an `evalf`. That representation is not a proof of numerical correctness: a
digit budget bounds how many digits a kernel may store, and an iterative, transcendental or
cumulative computation can accumulate error beyond its last stored digit. This page explains the two
precision knobs, their host-specific defaults, how `setprecision`, `pi(n)`, `e(n)` and `evalf`
interact with them, and how numbers are rendered.

## Three numbers to keep apart

| Concept | What it controls | Where it lives |
|---|---|---|
| Computation precision | How many decimal places a Real-producing kernel may compute before it truncates | `Engine::precision`, moved by `setprecision` |
| Display precision | How many fractional digits a non-periodic Real may **render** | `Engine::display_precision`, also moved by `setprecision` |
| Host default | The precision a fresh session starts at | The host chooses: the raw engine and `lovelace-run` use 1000/100; the desktop uses 30 |

The engine's raw defaults are **1000 computation digits** and **100 display digits**. A new
`Engine` starts there, and `lovelace-run` uses a new `Engine`. The desktop deliberately starts new
documents at **30 digits** (`lovelace-desktop/src/engine.rs`, `DEFAULT_PRECISION: usize = 30`,
recorded in ADR-004), because 30 is the extension host's historical budget and sits inside the
fixed-width fast path. A persisted desktop setting keeps its stored value; only a fresh document
takes 30.

That difference explains a common surprise: the same script can print more digits in the CLI than
in a fresh desktop document. The language is the same; the host's starting precision is not.

A **computation precision** is a budget, not an error bound. `setprecision(30)` permits kernels to
store 30 decimal places; it does not prove that an arbitrary iterative, transcendental or cumulative
result is within `10^-30` of the true value. A stated final-error bound is a separate claim and needs
its own derivation or independent evidence; the categories are defined in the
[editorial guide](../editorial.md#the-four-kinds-of-claim).

## `setprecision(n)`

`setprecision(n)` requires a positive exact integer and sets **both** the computation and display
precision. It returns `Void`.

```lovelace
setprecision(30); sqrt(2)
```
```text
= 1.414213562373095048801688724209 (Real)
  _ = 1.414213562373095048801688724209
```

The new precision applies to future computations. It does not retroactively add digits to a value
that was already computed; it does change how that value is displayed, because the display cap
also moved.

```lovelace
setprecision(30); a = sqrt(2); setprecision(20); a
```
```text
= 1.41421356237309504880 (Real)
  _ = 1.41421356237309504880
  a = 1.41421356237309504880
```

`a` still stores the 30 digits it was computed with, but `setprecision(20)` lowered the display
cap to 20, so both the result line and the variable line show 20 digits. Re-raising the precision
would reveal the stored digits up to the computation cap.

A zero, negative, or non-integer count is refused:

```lovelace
setprecision(0)
```
```text
Error [InvalidArgument/TypeMismatch]: setprecision(): argument 1 (digits) must be a Natural or Integer that is a positive count (1 or more, at most 9223372036854775807); got 0.
```

An exactly integral Real is refused by kind and told to be written as an integer; a fractional
Real is a plain `DomainError`:

```lovelace
setprecision(2.5)
```
```text
Error [InvalidOperation/DomainError]: setprecision(): argument 1 (digits) must be a Natural or Integer that denotes an exact integer; got the Real 2.5.
```

The accepted upper bound is the signed 64-bit maximum. Setting a very large precision is accepted;
the cost is paid by the next computation, not by the setter.

## `pi(n)` and `e(n)`

`pi()` and `e()` compute the constant at the engine's **computation** precision and render it at
the display cap. With an argument, they compute to `n` decimal places, but `n` may not exceed the
current computation precision. The result is a `Real`; the computation is a truncation, not a
rounding.

```lovelace
pi(5)
```
```text
= 3.14159 (Real)
  _ = 3.14159
```

At the raw default, `pi()` computes 1000 digits and renders 100:

```lovelace
pi()
```
```text
= 3.1415926535897932384626433832795028841971693993751058209749445923078164062862089986280348253421170679 (Real)
  _ = 3.1415926535897932384626433832795028841971693993751058209749445923078164062862089986280348253421170679
```

After `setprecision(30)`, `pi()` computes and renders 30 digits:

```lovelace
setprecision(30); pi()
```
```text
= 3.141592653589793238462643383279 (Real)
  _ = 3.141592653589793238462643383279
```

Requesting more digits than the engine precision is refused, naming the cap:

```lovelace
pi(2000)
```
```text
Error [InvalidArgument/TypeMismatch]: pi(): argument 1 (digits) must be a Natural or Integer between 1 and 1000; got 2000.
```

```lovelace
setprecision(10); e(20)
```
```text
Error [InvalidArgument/TypeMismatch]: e(): argument 1 (digits) must be a Natural or Integer between 1 and 10; got 20.
```

`setprecision(30); pi(50)` is refused for the same reason: the cap is the current engine
precision, not a fixed constant. The default cap is 1000 because that is the raw engine default.

## `evalf`

`evalf(f, digits)` numerically evaluates a symbolic expression to `digits` decimal places. The
digit count is **computed**, not merely printed: the evaluation runs at the requested count plus
guard digits and then truncates, so an expression with integer digits keeps its requested
fractional places. The builtin's own computation cap is 1000 places; a request above the cap
publishes a `truncated` marker with `truncationReason: "digit-cap"` and `budget: 1000` instead of
pretending the full request was honoured.

```lovelace
evalf(sqrt(2), 10)
```
```text
= 1.4142135623 (Real)
  _ = 1.4142135623
```

```lovelace
evalf(1/3, 10)
```
```text
= 0.3333333333 (Real)
  _ = 0.3333333333
```

An exact answer stays exact: `evalf(1/2, 40)` is `0.5`, not a 40-digit approximation padded with
zeros. `evalf` is the bridge from `Symbolic` to `Real`; it is not a general "print more digits"
button for an already numeric value, because a value only stores the digits it was computed with.

## Display: truncation, not rounding

A non-periodic Real stores `int * 10^exp` digits. The renderer takes the stored fractional digit
string and truncates it at the active display cap; it does not round. Trailing zeros that are part
of the stored scale are visible up to the cap, which is why a truncated value can end in `00`.

```lovelace
setprecision(2); [1.005, 1.999]
```
```text
= [1.00, 1.99] (Vector)
  _ = [1.00, 1.99]
```

Both values are stored exactly (`1.005` and `1.999`); only their rendering is truncated. A
computed value is truncated at generation time as well, so `setprecision(5); 1/7` stores the
5-place truncation, not the exact `1/7`:

```lovelace
setprecision(5); 1/7
```
```text
= 0.14285 (Real)
  _ = 0.14285
```

At 7 places the period fits and the value reports exact again:

```lovelace
setprecision(7); 1/7
```
```text
= 0.(142857) (Real)
  _ = 0.(142857)
```

The provenance difference is observable: `inspect(...).exact` records whether a digit was dropped,
not whether the value is mathematically correct.

```lovelace
setprecision(5); [1/7, inspect(1/7).exact]
```
```text
= [0.14285, False] (Vector)
  _ = [0.14285, False]
```

```lovelace
setprecision(7); [1/7, inspect(1/7).exact]
```
```text
= [0.(142857), True] (Vector)
  _ = [0.(142857), True]
```

An integer-valued Real renders without a decimal point (`1.0` is `1 (Real)`), and a decimal
literal with trailing zeros normalises them away (`1.2300` is `1.23`). A small value keeps its
leading zeros (`0.000001`), because those are significant to its position.

## Periodic rendering

A Real with period metadata renders its finite prefix, then the repeating block in parentheses:

```lovelace
[1/3, 1/6, 1/8, 22/7]
```
```text
= [0.(3), 0.1(6), 0.125, 3.(142857)] (Vector)
  _ = [0.(3), 0.1(6), 0.125, 3.(142857)]
```

- `1/3` is the pure period `0.(3)`.
- `1/6` is a preperiod `1` followed by period `6`: `0.1(6)`.
- `1/8` terminates, so it has no period: `0.125`.
- `22/7` has integer part `3` and period `142857`: `3.(142857)`.

Period blocks are rendered in full even when the block is longer than the display cap. The
display cap applies to **finite** expansions only. A period that does not fit inside the
computation budget is not stored as a period at all; the value becomes a finite truncation and
reports `exact: False`. That flag is provenance, not an error bound. A period that closes exactly at
the cap is also not discovered: `1/7` has a six-digit period, and at `setprecision(6)` it is the
inexact truncation `0.142857`, while at `setprecision(7)` it is `0.(142857)`.

For example, `1/17` has a 16-digit period. At `setprecision(10)` it is a truncated finite value;
at `setprecision(20)` the period fits and it renders as `0.(0588235294117647)`.

## Printing versus the result line

There are two renderings of the same value:

- The **result line** (`= …`) uses the engine's display precision. At the raw default,
  `sqrt(2)` shows 100 of its 1000 stored digits.
- `print` and string interpolation use `Value::display()` with no display cap. At the raw
  default, `print(sqrt(2))` writes all 1000 stored digits.

This is why `setprecision` changes both surfaces at once (it raises or lowers the computation
precision, so the stored digit count changes), but a value computed at 1000 and printed shows more
digits than the result line:

```lovelace
setprecision(10); print(sqrt(2))
```
```text
= (void)
1.4142135623
```

At the raw default the same call prints 1000 digits. Use `setprecision` when you want a shorter
stored value, and use the result line rather than `print` when you want the display cap applied.

The JSON envelope exposes all three useful forms:

| Field | Rendering |
|---|---|
| `result.display` | Display form at the display cap. |
| `result.typed` | Display form plus `(Kind)` at the display cap. |
| `result.structured.value` | Stored digits as text (subject to the wire projection's own rules). |
| `result.structured.exact` | Exactness flag. |

## Formatting summary

| Value | Example input | Rendered |
|---|---|---|
| Natural | `42` | `42` |
| Integer | `-5` | `-5` |
| Real, integer-valued | `1.0` | `1` |
| Real, terminating | `1/8` | `0.125` |
| Real, periodic | `1/6` | `0.1(6)` |
| Real, truncated | `setprecision(2); 1.999` | `1.99` |
| Complex, pure imaginary | `fft([0,1,0,0])[1]` | `-1i` |
| Complex, general | `1 + 2i` is not a literal; a computed pair with both parts | `re + imi` or `re - imi` |
| Text | `"hello"` | `hello` (no quotes, no suffix) |
| Record | `divrem(17,5)` | `DivRemResult(quotient: 3, remainder: 2)` |
| Void | `print("hi")` | `(void)` at top level |

The renderer is deliberately not locale-sensitive: decimal point is `.`, there are no thousands
separators, and no scientific notation. Very large integers are printed in full decimal; very
small Reals print their leading zeros rather than switching to exponent notation.

## A compact mental model

1. Computation precision decides how many digits a kernel may produce; display precision decides
   how many of the stored fractional digits a **result line** may show.
2. `setprecision(n)` moves both. The raw engine starts at 1000/100; the desktop starts at 30.
3. `pi(n)`/`e(n)` compute `n` digits up to the computation cap; `evalf(f, n)` computes `n` decimal
   places up to its own 1000-place cap.
4. Rendering truncates at the display cap; it never rounds.
5. Periodic blocks render in full; finite expansions obey the cap.
6. `print`/interpolation show the full stored value; the result line obeys the display cap.

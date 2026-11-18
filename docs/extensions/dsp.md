# DSP (`Lovelace.Dsp`)

**16 builtins.** Each is a signal operation over numeric vectors whose samples are decimal-backed
`Real` values (or `Complex` pairs of them). No binary floating point enters the
representation, but that is a property of the representation, not a guarantee that every result is
mathematically exact: several DSP builtins compute irrational values at the active digit budget, and
a division whose repeating period exceeds that budget is truncated and marked `inexact`.

## Purpose

### What it is

DSP is the signal-processing family: discrete transforms (`fft`, `dft`), convolution and
difference-equation filtering (`conv`, `filter`, `movingavg`, `delay`), signal generators
(`impulse`, `step`, `cosine`, `exponential`, `powerseries`, `noise`), and complex accessors
(`re`, `im`, `conj`, `scale`).

It is declared by the `module` field of 16 descriptors in
[`lovelace-lang/src/registry.rs`](../../lovelace-lang/src/registry.rs) (`module: "DSP"`). The
implementations are the `dsp_*` functions in
[`lovelace-lang/src/builtins.rs`](../../lovelace-lang/src/builtins.rs)
(`dsp_filter` at line 5996, `dsp_noise` at 6108, `dsp_cosine` at 5915, and so on) plus the exact
complex helpers beside them.

Samples are decimal-backed, not binary floating point. A sample is a `Real` (an integer scaled by a
power of ten, carrying periodic-decimal metadata when its expansion repeats) or a `Complex` of two
`Real`s, so `fft([0, 1, 0, 0])` answers exactly `[1, -1i, -1, 1i]`. The transform kernels build
their roots of unity with the engine's decimal arithmetic; there is no libm and no external crate in
the path.

Four different properties are often described as "exact" and they must not be collapsed into one
claim (the categories are defined in the [editorial guide](../editorial.md#the-four-kinds-of-claim)):

- **Exact value.** Integer results and rationals whose decimal expansion terminates or recurs within
  the budget are represented exactly, and the arithmetic that produced them is exact:
  `fft([0, 1, 0, 0])` is `[1, -1i, -1, 1i]`. Exact representation is not by itself a correctness
  proof: the historical `movingavg` defect recorded below stored a wrong rational and marked it
  exact.
- **Precision-limited value.** `sqrt(2)`, `pi` and any irrational or transcendental result are
  truncated at the requested number of places. "30-place" names the *budget*, not a proven
  `10^-30` final-error bound: the accumulated error of an iterative or transcendental routine needs
  its own analysis. Printing more of the stored digits does not make the value exact.
- **Error-bounded value.** A DSP result may be described as error-bounded only when a specific bound
  and the argument for it are stated. No DSP page currently publishes such a bound; a digit budget
  alone is not one.
- **Parity-verified behaviour.** A value that reproduces the C# implementation, including a defect,
  is evidence about the reference, not a mathematical result. The known case, `movingavg`, was fixed
  by removing the fixed-width substitution; this page records no live parity artifact, and its wrong
  values survive only as the labelled historical record below and in
  [ADR-004](../adr/004-thirty-digit-default.md).

The decimal representation is a property of the values, not a correctness guarantee: it is **not**
a proof that every 30-place result is mathematically correct, and the sections below say which case
a builtin is in.

The `exact` / `inexact` flag is provenance: it records whether a digit was dropped, not how far the
stored value is from the true result, and it is neither an error bound nor a correctness
certificate.

A signal is a rank-1 vector. A scalar is accepted where a signal is expected and widened to a
one-sample signal (`fft(1)` answers `[1]`, `conj(1)` answers `1 (Complex)`), and a nested array is
flattened to a vector before the transform (`fft([[[1, 2]]])` answers `[3, -1]`, the two-point
transform of `[1, 2]`).

### Where the code lives

| Path | What is there |
| --- | --- |
| `lovelace-lang/src/registry.rs` | the 16 descriptors, `module: "DSP"` |
| `lovelace-lang/src/builtins.rs` | `dsp_*` implementations and the exact complex helpers, ~line 5336-6700 |
| `lovelace-lang/src/lib.rs` | `dsp_precision()` (the 30-place DSP default) |
| `lovelace-lang/src/fixed_dsp.rs` | the fixed-width whole-array tier (`Cplx<W>`, `to_complex`) |
| `lovelace-lang/src/signals.rs` | `Sequence`, `Sum`, `Product` value types |
| `lovelace-lang/src/complex.rs` | exact complex arithmetic and fixed-width conversion |

Tests that cover it (module names, `cargo test -p lovelace-lang`):

| Test module | Tests |
| --- | --- |
| `dsp_round21` | 60 |
| `dsp_round20` | 39 |
| `fixed_dsp_tests` | 17 |
| `dsp_family_round61` | 7 |
| `suite_engine_batch` (DSP rows) | part of 42 |
| `round28_batch` (DSP rows) | part of 22 |

`lovelace-run` also carries `dsp_round21` (5 C# DSP test rows) as an integration-level batch.

## Inputs

### How it is available

`Engine::new()` exposes every registered builtin, so `fft` is callable in every engine,
`lovelace-run` invocation and desktop session; there is no flag that removes a family and no
enabled-family set. The `module: "DSP"` label only groups the 16 descriptors for discovery:
`lovelace-run --functions` reports all 134 builtins, and `--function fft` prints
`Module: DSP`.

The family stays independently testable without the interpreter because the DSP kernels are ordinary
Rust functions; the tests in the table above and the runner's `dsp_round21` batch call the same
engine entry point every host uses.

## Outputs

### Every function it contributes

Arity is `min-max` accepted arguments; `+` marks variadic. Parameters and descriptions are the
registry's declared metadata. All 16 were executed; the call and its exact typed result are in the
[verified-call appendix](#verified-calls) below.

| Builtin | Arity | Parameters | Returns | What it does |
| --- | --- | --- | --- | --- |
| `conj` | 1 | `x` | Complex \| Vector | Complex conjugate of a complex value or signal. |
| `conv` | 2 | `x`, `h` | Vector | Linear convolution of two signals. |
| `cosine` | 3 | `freq`, `phase`, `n` | Vector | Cosine sequence of length n at the given frequency and phase. |
| `delay` | 2 | `x`, `k` | Vector | Delays a signal by k samples. |
| `dft` | 1-2 | `x`, `n` | Vector | Discrete Fourier transform; an optional length n zero-pads or truncates the signal. |
| `exponential` | 2 | `c`, `n` | Vector | Exponential sequence c^k of length n. |
| `fft` | 1 | `x` | Vector | Fast Fourier transform of a signal. |
| `filter` | 3 | `a`, `b`, `x` | Vector | Difference-equation filter of a signal (or the impulse response of length n). |
| `im` | 1 | `x` | Real \| Vector | Imaginary part of a complex value or signal. |
| `impulse` | 1 | `n` | Vector | Unit impulse sequence of length n. |
| `movingavg` | 2 | `x`, `w` | Vector | Moving average of a signal with window w. |
| `noise` | 3-4 | `scale`, `disp`, `n`, `seed` | Vector | Deterministic seeded noise sequence (the same seed always yields the same signal). |
| `powerseries` | 3 | `k`, `a`, `n` | Vector | Geometric power series of length n. |
| `re` | 1 | `x` | Real \| Vector | Real part of a complex value or signal. |
| `scale` | 2 | `x`, `c` | Vector | Scales every sample of a signal by the scalar c. |
| `step` | 1 | `n` | Vector | Unit step sequence of length n. |

## Precision behaviour

### Precision and limits

- **30-place fast default.** Until `setprecision` is called, every DSP builtin computes at 30
  decimal places: `engine.dsp_precision()` returns `30` when `precision_explicit` is false and the
  engine knob otherwise ([`dsp_precision`](../../lovelace-lang/src/lib.rs)). The two `im(dft(...))`
  runs above are the observed consequence (31 printed digits vs 60). This is a working budget, not a
  final-error bound (see the four claim kinds above).
- **No fixed-width substitution remains in `movingavg`.** `movingavg` divides through `Real::div` at
  every precision. The former fixed-width rendering path (`dsp_lreal128_division_render`) was
  deleted; it was not a value-preserving fallback, because it replaced a correct quotient with the
  defective rendering recorded in the historical defect subsection below.
- **Long periods are truncated, not preserved.** `Real::div` builds its quotient through
  `Real::from_fraction` (`lovelace-real/src/lib.rs`). A terminating quotient is expanded in full at
  any budget; a non-terminating one is bounded by the budget. A repeating period longer than the
  budget is published as a finite truncation at the budget, with no period metadata and with
  `inexact` provenance. A period that closes exactly at the cap is also not discovered, because the
  loop's position test runs before the repeated-remainder test. At a larger budget the same rational
  can keep its period: `setprecision(2); movingavg([1], 7)` is `[0.14]`, and at 7 places the same
  call is `[0.(142857)]`. At 6 places it is `[0.142857]` and inexact precisely because the 6-digit
  period closes at the cap.
- **18/37-place working width in the separate fixed-width tier.** The `fixed_dsp` tier (tested but
  not called by any builtin; see Limitations) expands a repeating operand to 18 fractional places
  at 64-bit width and 37 at 128-bit ([`fixed_dsp.rs`](../../lovelace-lang/src/fixed_dsp.rs)).
- **`fft` is power-of-two only; `dft` is not.** `dft(x, n)` zero-pads or truncates to `n`; `n = 0`
  answers `[]`.
- **Argument counts and ranges are refused, not clamped.** Negative counts and a non-positive
  `movingavg` window raise `InvalidArgument`/`TypeMismatch` with the declared bound in the message.
  `delay(x, k)` with `k` past the end does *not* error: it answers zeros, because the missing
  samples are the zero history.
- **Symbolic values are refused.** A value such as `sqrt(-4)` is `Symbolic`, not a numeric complex,
  and every DSP builtin answers `DSP builtins expect numeric/complex elements, but got 'SymbolExpr'.`
- **Two registry descriptions disagree with the implementation**, so trust the observed column:
  `exponential(c, n)` is `e^(c*k)` (not `c^k`) — `exponential(1, 3)` is `[1, e, e^2]`; and `noise`
  reads its 4th argument as the sample count and its 3rd as the seed, i.e. the implemented order is
  `noise(scale, disp, seed, n)` while the descriptor prints `scale, disp, n, seed`. Observed:
  `noise(1, 0, 4, 3)` answers 3 samples and `noise(1, 0, 3, 4)` answers 4.
- **`powerseries(k, a, n)` is the weighted series** `k * a^i * i` for `i = 0..n-1`, which is why
  `powerseries(2, 0.5, 4)` is `[0, 1, 1, 0.75]`.

## Errors

Every refusal is an ordinary `LangError` with a stable code and category; there is no special
family-specific error type.

| Failure | Code / category |
|---|---|
| Wrong argument count at the call site | `InvalidArgument` / `TypeMismatch`, with the declared signature |
| `fft` of a non-power-of-two length | `InvalidArgument` / `TypeMismatch`, naming the length and `(Parameter 'x')` |
| Negative count, or a non-positive `movingavg` window | `InvalidArgument` / `TypeMismatch`, with the declared bound |
| A signal element that is neither numeric nor complex (for example a `SymbolExpr`) | `InvalidOperation` / `DomainError`, message `DSP builtins expect numeric/complex elements, but got '<kind>'.` |

An out-of-range index such as `fft([0, 1, 0, 0])[9]` is the language's index failure, not a DSP
one, and `delay(x, k)` with `k` past the end is not an error at all: it answers zeros because the
missing samples are the zero history.

The observed messages are in the Examples section below: `cosine(0.25, 0)` for the arity refusal
and `fft([1, 2, 3])` for the power-of-two refusal.

## Limitations

### What is NOT in this extension

- **No filter design.** There is no Butterworth/Chebyshev/FIR/IIR *design* builtin, no `hann`,
  `hamming`, `blackman`, `window`, `spectrogram`, `correlate`, `fir` or `iir` name anywhere in the
  registry. `filter` *applies* coefficients you supply; it does not compute them.
- **No `FixedDsp` language surface.** `fixed_dsp.rs` is a ported, tested tier, but no builtin calls
  it: its own module docs say "no caller in this workspace drives them yet"
  ([`fixed_dsp.rs`](../../lovelace-lang/src/fixed_dsp.rs)). The 17 `fixed_dsp_tests` exercise it
  directly, not through a script.
- **No streaming or stateful signals.** Every builtin takes a whole vector and returns a new one;
  `signals::Sequence` is a value type with no engine surface.
- **No statistics.** There is no mean/variance/PSD/correlation analysis family in RustLace at all;
  `mean`/`sum`/`norm` are core `Arrays` reductions, not DSP.
- **`re`/`im`/`conj` are trivial accessors**, not a complex-number namespace: there is no `arg`,
  `magnitude`, or complex-exponential builtin here (`abs` is core `Numerics`).

### Historical defect (fixed): `movingavg` below 38 digits

This subsection is a historical record, not current behaviour. The "Before (historical, wrong)"
column gives values observed from the removed fixed-width substitution; they must not be cited as
expected results. The "Now (current)" column is the current arithmetic.

Earlier builds of this port reproduced the reference implementation's fixed-width division rendering
for `movingavg` below 38 digits, and that rendering misplaces the recurring period. The emulation has
been **removed**: `movingavg` now divides through the engine's `Real::div` at every precision.

| Call | Before (historical, wrong) | Now (current) |
|---|---|---|
| `movingavg([2.5], 3)` | `[0.(8)]` = 8/9 | `[0.8(3)]` = 5/6 |
| `movingavg([0.1], 3)` | `[0.(0)]` = 0 | `[0.0(3)]` = 1/30 |
| `movingavg([1/3], 3)` | `[0.(1)]`, reported **inexact** | `[0.(1)]` = 1/9, reported **exact** |

The old incorrect fixed-width substitution was removed, and the demonstrated rational examples are
correct and produce the same values at every budget tested here: 30, 37, 38 and 1000 decimal places.
This is a scoped statement about those examples, not a claim that every `movingavg` result is exact
at every budget. A repeating period longer than the budget may be truncated and marked inexact, in accordance
with `Real::from_fraction`; the "Precision and limits" rule above states that behaviour and gives an
observed example.

The former incorrect `movingavg([2.5], 3)` result is no longer produced for that input. `0.(8)` is
the legitimate representation of `8/9` and still renders for `8/9`; it is simply not the
`movingavg([2.5], 3)` result. `0.(0)` is likewise the representation of zero. Neither historical row
is an expected result.

The incident, its mechanism and the decision to prefer arithmetic soundness over byte-parity with a
defective reference are recorded in [ADR-004](../adr/004-thirty-digit-default.md).

## Examples

### Understand the kinds in these examples

A [Vector](../language/values-and-types.md#vector-and-array) is the
one-dimensional collection of samples passed to `fft`, `dft`,
`movingavg` and most of this family. Input samples commonly have
[Real](../language/values-and-types.md#numeric-kinds-and-widening) values.
Transforms can return a Vector with
[Complex](../language/values-and-types.md#complex) elements, even when the
input consists only of real numbers; the outer result kind is still
`Vector`.

Don't confuse the shape of a [Vector](../language/values-and-types.md#vector-and-array)
with a rank-2 [Array](../language/values-and-types.md#vector-and-array), or
the displayed digits of a complex coefficient with a guaranteed error bound.
The [worked DSP tutorial](../tutorials/signal-processing.md) explains which
coefficients in a four-point example are exact and where a longer FFT becomes
precision-limited. The [library guide](../how-to/use-libraries.md#dsp-input-samples-can-produce-complex-output)
shows a smaller first call.


### Worked examples

Each block is a script run through the built CLI, with the observed output pasted beneath it. The
runner in [`examples/run-examples.ps1`](examples/run-examples.ps1) re-executes every pair and
compares.

The transform pair: `fft` requires a power-of-two length, `dft` accepts any length and an optional
length to zero-pad to.

```lovelace
fft([0, 1, 0, 0])
```

```text
= [1, -1i, -1, 1i] (Vector)
```

```lovelace
dft([1, 2, 3], 4)
```

```text
= [6, -2 - 2i, 2, -2 + 2i] (Vector)
```

The same four-point transform written the other way round produces the same values, which is the
cheapest check that `dft`'s optional length pads rather than truncates:

```lovelace
fft([1, 2, 3, 0])
```

```text
= [6, -2 - 2i, 2, -2 + 2i] (Vector)
```

`filter` with a scalar third argument returns the impulse response of the difference equation.
`[1, -0.5]`/`[1]` is `y[n] = x[n] - 0.5 y[n-1]`, so the response is the geometric sequence:

```lovelace
filter([1, -0.5], [1], 4)
```

```text
= [1, 0.5, 0.25, 0.125] (Vector)
```

`movingavg` uses a causal window with zero history, which is why the first output is half the first
sample rather than the first sample:

```lovelace
movingavg([1, 2, 3, 4], 2)
```

```text
= [0.5, 1.5, 2.5, 3.5] (Vector)
```

Precision follows the DSP knob (ADR-004). Untouched, a DSP builtin computes at 30 places;
`setprecision` promotes it. Both runs below are real:

```lovelace
im(dft([1, 2, 3])[1])
```

```text
= 0.8660254037844386467637231707525 (Real)
```

```lovelace
setprecision(60)
im(dft([1, 2, 3])[1])
```

```text
= 0.866025403784438646763723170752936183471402626905190314027903 (Real)
```

`noise` is deterministic: the same seed gives the same sequence, twice.

```lovelace
noise(1, 0, 7, 2)
```

```text
= [0.386036098849418483807514309047 + 0.721100815986105975812879586163i, 0.704101820856791041843313485152 + 0.364228199414505507753532038161i] (Vector)
```

Two failure modes worth knowing. A non-power-of-two `fft` is refused by argument validation, while
`dft` above accepted length 3:

```lovelace
fft([1, 2, 3])
```

```text
Error [InvalidArgument/TypeMismatch]: FFT length must be a power of two, but got 3. (Parameter 'x')
```

Arity is enforced at the call site from the registry declaration:

```lovelace
cosine(0.25, 0)
```

```text
Error [InvalidArgument/TypeMismatch]: cosine(): expected 3 arguments; got 2.
```

### Verified calls

Every builtin in the table above was executed. Each row is the exact one-line call (statements
joined with `;`, which is a valid separator) and the typed result the runner observed. Results
longer than 150 characters are marked `...`.

| Builtin | Exact call | Observed typed result |
| --- | --- | --- |
| `conj` | `conj(fft([0, 1, 0, 0])[1])` | = 1i (Complex) |
| `conv` | `conv([1, 2], [1, 1])` | = [1, 3, 2] (Vector) |
| `cosine` | `cosine(0.25, 0, 8)` | = [1, 0, -1, 0, 1, 0, -1, 0] (Vector) |
| `delay` | `delay([1, 2, 3], 1)` | = [0, 1, 2] (Vector) |
| `dft` | `dft([0, 1, 0, 0])` | = [1, -1i, -1, 1i] (Vector) |
| `exponential` | `exponential(0.5, 4)` | = [1, 1.6487212707001281468486507878141635716537760393860552423690659505907263702979744082624533553446847255, 2.71828182845904523536028747135266249775... |
| `fft` | `fft([0, 1, 0, 0])` | = [1, -1i, -1, 1i] (Vector) |
| `filter` | `filter([1], [1], [1, 2, 3])` | = [1, 2, 3] (Vector) |
| `im` | `im(fft([0, 1, 0, 0])[1])` | = -1 (Real) |
| `impulse` | `impulse(4)` | = [1, 0, 0, 0] (Vector) |
| `movingavg` | `movingavg([1, 2, 3], 2)` | = [0.5, 1.5, 2.5] (Vector) |
| `noise` | `noise(1, 0, 4, 3)` | = [0.895400644725357554783864003163 + 0.659009310258470365578553993139i, 0.6571076258144525494748694196 + 0.627150026264065409045375589879i, 0.7024296... |
| `powerseries` | `powerseries(2, 0.5, 4)` | = [0, 1, 1, 0.75] (Vector) |
| `re` | `re(fft([0, 1, 0, 0])[1])` | = 0 (Real) |
| `scale` | `scale([1, 2, 3], 2)` | = [2, 4, 6] (Vector) |
| `step` | `step(4)` | = [1, 1, 1, 1] (Vector) |

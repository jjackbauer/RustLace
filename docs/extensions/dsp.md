# DSP (`Lovelace.Dsp`)

**16 builtins.** Every one is a signal operation over exact numeric vectors.

## What it is

DSP is the signal-processing family: discrete transforms (`fft`, `dft`), convolution and
difference-equation filtering (`conv`, `filter`, `movingavg`, `delay`), signal generators
(`impulse`, `step`, `cosine`, `exponential`, `powerseries`, `noise`), and complex accessors
(`re`, `im`, `conj`, `scale`).

It is declared by the `plugin` field of 16 descriptors in
[`lovelace-lang/src/registry.rs`](../../lovelace-lang/src/registry.rs) — the first
`plugin: Some("Lovelace.Dsp")` tag is at line 407 (on `conj`'s descriptor) and the last at line
1595. The implementations are the `dsp_*` functions in
[`lovelace-lang/src/builtins.rs`](../../lovelace-lang/src/builtins.rs)
(`dsp_filter` at line 5996, `dsp_noise` at 6108, `dsp_cosine` at 5915, and so on) plus the exact
complex helpers beside them.

Nothing is floating point. A sample is a `Real` (exact decimal, possibly periodic) or an exact
`Complex` of two `Real`s, so `fft([0, 1, 0, 0])` answers exactly `[1, -1i, -1, 1i]` and
`im(dft([1, 2, 3])[1])` answers an exact 30-place decimal rather than a rounded double. The
transform kernels build their roots of unity from exact arithmetic; there is no libm and no
external crate in the path.

A signal is a rank-1 vector. A scalar is accepted where a signal is expected and widened to a
one-sample signal (`fft(1)` answers `[1]`, `conj(1)` answers `1 (Complex)`), and a nested array is
flattened to a vector before the transform (`fft([[[1, 2]]])` answers `[3, -1]`, the two-point
transform of `[1, 2]`).

## How it is enabled

`lovelace-run` always builds `Engine::new()`, which leaves `enabled_plugins` as `None`; the gate
`Engine::plugin_enabled` then answers `true` for every registered name
([`lovelace-lang/src/lib.rs`](../../lovelace-lang/src/lib.rs), lines 990-998). There is no CLI flag
that removes a family: the usage block lists no plugin option, and `--functions` reports all 124
builtins in every run.

The opt-out is the library seam. `Engine::new_bare()` sets `enabled_plugins = Some(empty)`, so only
`plugin: None` descriptors resolve. The demo
[`bare_engine.rs`](../../lovelace-lang/examples/bare_engine.rs) shows both sides (run it with `cargo run -p lovelace-lang --example bare_engine`); these are its observed lines
for this family:

```text
bare       fft([0, 1, 0, 0])        => [UnknownFunction] Unknown function 'fft'.
bare+dsp   fft([0, 1, 0, 0])        => [1, -1i, -1, 1i]
bare+dsp   det([[1, 2], [3, 4]])    => -2
bare+dsp   symbol("x")              => [UnknownFunction] Unknown function 'symbol'.
second load: [InvalidOperation] Plugin 'Lovelace.Dsp' is already loaded.
```

`det` keeps working on the bare engine because it is core; `symbol` stays unknown because it belongs
to a different family. The full three-family transcript is in the [extensions README](README.md#the-plugin-model).

Calling a builtin from an unloaded family answers the ordinary vocabulary failure, not a special
"plugin missing" error: `[UnknownFunction] Unknown function 'fft'.` — code `UnknownFunction`,
category `UnsupportedOperation` (`builtins.rs:4279`), raised at the gate in `lib.rs:2146-2150`.

To re-enable the family on a bare engine, load a plugin whose `name()` is exactly the registry
string. The gate keys on that name, so the demo's `EnableDsp` registers nothing and the engine's own
16 DSP builtins become callable again:

```rust
use lovelace_lang::modus::{ModusHost, ModusPlugin};
use lovelace_lang::{Engine, LangError};

struct EnableDsp;
impl ModusPlugin for EnableDsp {
    fn name(&self) -> &str { "Lovelace.Dsp" }
    fn register(&mut self, _host: &mut ModusHost) -> Result<(), LangError> { Ok(()) }
}

let mut engine = Engine::new_bare();
engine.load_plugin(&mut EnableDsp).unwrap();
assert_eq!(engine.eval("fft([0, 1, 0, 0])").unwrap().display(), "[1, -1i, -1, 1i]");
```

`Engine::load_plugin` is `lib.rs:883`; a second load is refused with
`[InvalidOperation] Plugin 'Lovelace.Dsp' is already loaded.` `Engine::register_builtin`
(`lib.rs:892`) is the other install path: a dynamically registered name is dispatched at
`lib.rs:2141` *before* the gate, so it is callable even on a bare engine.

## Every function it contributes

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

## Worked examples

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

Precision follows the plugin knob. Untouched, a DSP builtin computes at 30 places; `setprecision`
promotes it. Both runs below are real:

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

## Precision and limits

- **30-place fast default.** Until `setprecision` is called, every DSP builtin computes at 30
  decimal places: `engine.dsp_precision()` returns `30` when `precision_explicit` is false and the
  engine knob otherwise ([`lib.rs:2168-2177`](../../lovelace-lang/src/lib.rs)). The two `im(dft(...))`
  runs above are the observed consequence (31 printed digits vs 60).
- **38-digit fixed-width conversion bound.** `dsp_lreal128_convertible` accepts a value only while
  its decimal magnitude fits 38 significant digits (`builtins.rs:5686`); wider values stay on the
  arbitrary-precision path. This is a value-preserving fallback, never a rounding.
- **18/37-place working width for periodic operands.** The fixed-width tier expands a repeating
  operand to 18 fractional places at 64-bit width and 37 at 128-bit
  ([`fixed_dsp.rs:17-20`](../../lovelace-lang/src/fixed_dsp.rs)).
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

## Where the code lives

| Path | What is there |
| --- | --- |
| `lovelace-lang/src/registry.rs` | the 16 descriptors; plugin tags at lines 408-1611 |
| `lovelace-lang/src/builtins.rs` | `dsp_*` implementations and the exact complex helpers, ~line 5336-6700 |
| `lovelace-lang/src/lib.rs` | `dsp_precision()` (2168), the gate (990), `load_plugin` (883) |
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

`lovelace-run` also carries `dsp_round21` (5 C# `DspPluginTests` rows) as an integration-level batch.

## What is NOT in this extension

- **No filter design.** There is no Butterworth/Chebyshev/FIR/IIR *design* builtin, no `hann`,
  `hamming`, `blackman`, `window`, `spectrogram`, `correlate`, `fir` or `iir` name anywhere in the
  registry. `filter` *applies* coefficients you supply; it does not compute them.
- **No `FixedDsp` language surface.** `fixed_dsp.rs` is a ported, tested tier, but no builtin calls
  it: its own module docs say "no caller in this workspace drives them yet"
  ([`fixed_dsp.rs:36-41`](../../lovelace-lang/src/fixed_dsp.rs)). The 17 `fixed_dsp_tests` exercise it
  directly, not through a script.
- **No streaming or stateful signals.** Every builtin takes a whole vector and returns a new one;
  `signals::Sequence` is a value type with no engine surface.
- **No statistics.** There is no mean/variance/PSD/correlation analysis family in RustLace at all;
  `mean`/`sum`/`norm` are core `Arrays` reductions, not DSP.
- **`re`/`im`/`conj` are trivial accessors**, not a complex-number namespace: there is no `arg`,
  `magnitude`, or complex-exponential builtin here (`abs` is core `Numerics`).

## Verified calls

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

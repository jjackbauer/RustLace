# Build a signal, smooth it, and read its spectrum

## Problem

You have a short synthetic signal and you want to do the two standard things: apply a DSP operation
in the time domain, and inspect the same signal in the frequency domain. Two details decide whether
the result means anything: the FFT length, and the precision at which the transform is computed.

This tutorial smooths an impulse with a moving average, takes a length-4 FFT, shows the
power-of-two length rule, pads a length-3 signal to satisfy it, and compares an 8-point FFT computed
at the DSP default with the same transform computed at 50 places.

## Source

```lovelace
signal = [0, 1, 0, 0]
smoothed = movingavg(signal, 2)
spectrum = fft(signal)
padded_spectrum = fft(append([1, 2, 3], [0]))
wave = [1, 2, 3, 4, 5, 6, 7, 8]
default_spectrum = fft(wave)
setprecision(50)
wide_spectrum = fft(wave)
```

The same script is checked in as
[signal-processing.ls](examples/signal-processing.ls).

## Output

```text
= [36, -4 + 9.65685424949238019520675489683879231427868750150776i, -4 + 4i, -4 + 1.65685424949238019520675489683879231427868750150776i, -4, -4 - 1.65685424949238019520675489683879231427868750150776i, -4 - 4i, -4 - 9.65685424949238019520675489683879231427868750150776i] (Vector)
  _ = [36, -4 + 9.65685424949238019520675489683879231427868750150776i, -4 + 4i, -4 + 1.65685424949238019520675489683879231427868750150776i, -4, -4 - 1.65685424949238019520675489683879231427868750150776i, -4 - 4i, -4 - 9.65685424949238019520675489683879231427868750150776i]
  default_spectrum = [36, -4 + 9.656854249492380195206754896836i, -4 + 4i, -4 + 1.656854249492380195206754896836i, -4, -4 - 1.656854249492380195206754896836i, -4 - 4i, -4 - 9.656854249492380195206754896836i]
  padded_spectrum = [6, -2 - 2i, 2, -2 + 2i]
  signal = [0, 1, 0, 0]
  smoothed = [0, 0.5, 0.5, 0]
  spectrum = [1, -1i, -1, 1i]
  wave = [1, 2, 3, 4, 5, 6, 7, 8]
  wide_spectrum = [36, -4 + 9.65685424949238019520675489683879231427868750150776i, -4 + 4i, -4 + 1.65685424949238019520675489683879231427868750150776i, -4, -4 - 1.65685424949238019520675489683879231427868750150776i, -4 - 4i, -4 - 9.65685424949238019520675489683879231427868750150776i]
```

## Reading the result

- `signal = [0, 1, 0, 0]` is a four-sample impulse: one unit at sample 1, zeros elsewhere.
- `smoothed = movingavg(signal, 2)` is `[0, 0.5, 0.5, 0]`. The window is 2 samples with zero
  history before the signal, so each output is the mean of the current sample and the one before it
  (treated as 0 at the start). The halves are exact.
- `spectrum = fft(signal)` is `[1, -1i, -1, 1i]`: bin 0 is 1, bin 1 is -i, bin 2 is -1, bin 3 is i.
  These are exact Gaussian integers, not rounded trigonometric values.
- `padded_spectrum = fft(append([1, 2, 3], [0]))` is `[6, -2 - 2i, 2, -2 + 2i]`. The input is
  `[1, 2, 3]`, length 3, which is not a power of two, so a zero is appended to make length 4 before
  the transform. The result is still exact.
- `wave = [1, 2, 3, 4, 5, 6, 7, 8]` is an 8-sample ramp. Its FFT needs the irrational twiddle
  factors that a length-8 transform implies, so the imaginary components are not exact integers.
- `default_spectrum` was computed before `setprecision(50)`, at the DSP default of 30 decimal places.
  `wide_spectrum` was computed afterwards, at 50 places. Their real parts agree exactly
  (`36`, `-4`, `-4 + 4i`, and so on); their irrational imaginary components differ in the tail:
  `...896836` at the 30-place budget, `...8968387923...` at 50. The 30-place tail is therefore a
  budgeted approximation, not a rounded exact value.

The FFT length rule is enforced, not inferred. A length-3 transform is refused:

```text
Error [InvalidArgument/TypeMismatch]: FFT length must be a power of two, but got 3. (Parameter 'x')
```

That refusal came from `--eval 'fft([1, 2, 3])' --text`. Pad with zeros to the next power of two,
or design the signal at a power-of-two length; trimming would compute a different signal.
`append([1, 2, 3], [0])` above is the smallest padding example for this input.

## Assumptions

- The checked-out RustLace development revision, run from the repository root with the project-root Cargo command
  `cargo run -p lovelace-run -- --file docs/tutorials/examples/signal-processing.ls --text` on
  Windows, Linux or WSL. The DSP builtins are part of the CLI's builtin registry, and the first run
  compiles the runner.
- A DSP builtin computes at 30 decimal places until `setprecision` moves the engine precision; that
  default is documented in [the DSP reference](../extensions/dsp.md). `setprecision(50)` raises both
  the computation and display precision for subsequent computations.
- `setprecision` does not retroactively add digits to a value computed earlier. `default_spectrum`
  keeps the 30 digits it was computed with; it is displayed with those stored digits.
- FFT input length must be a power of two. The padded result is the 4-point DFT of the padded
  signal; it is not a 3-point DFT. The engine refuses a length-3 input, so there is no unpadded
  result to compare against.
- `movingavg` here is applied to an integer impulse with a window of 2, so its causal averages are
  exact halves. This call is exact at every precision: the defect that once made `movingavg` wrong
  on fractional signals below 38 digits has been fixed, and the values are no longer rendered through
  the reference implementation's fixed-width path. See
  [ADR-004](../adr/004-thirty-digit-default.md).
- The command writes no files.

## Precision

The four claim kinds are **exact**, **precision-limited**, **error-bounded** and
**parity-verified**, following [the editorial guide](../editorial.md#the-four-kinds-of-claim).

| Value | Label | Justification |
|---|---|---|
| `0`, `1` in `signal` and the appended `0` | **exact** | Integers. |
| `2` in `movingavg(signal, 2)` | **exact** | Integer window length. |
| `0`, `0.5` in `smoothed` | **exact** | 0 and 1/2; 1/2 is a terminating decimal. |
| `1`, `-1`, `1i`, `-1i` in `spectrum` | **exact** | Gaussian integers: real and imaginary parts have denominator 1. |
| `6`, `-2`, `2`, `-2` in `padded_spectrum` | **exact** | Integer real parts and imaginary coefficients. |
| `4` in the padded FFT length | **exact** | Integer sample count after padding. |
| `1` through `8` in `wave` | **exact** | Integers; `8` is also the transform length. |
| `36` and `-4` in both spectra | **exact** | Integer real components: the DFT of this real ramp has integer real parts, and the stored values are exactly those integers. |
| `4` in `-4 + 4i` | **exact** | Integer imaginary coefficient. |
| `9.656854249492380195206754896836` and `1.656854249492380195206754896836` (default) | **precision-limited** | Irrational components computed at the DSP 30-place budget; the displayed tail is a truncation at that budget, not an exact value and not a final-error bound. |
| `9.65685424949238019520675489683879231427868750150776` and `1.65685424949238019520675489683879231427868750150776` (wide) | **precision-limited** | The same irrational components computed at the 50-place budget; the tail moves with the budget, so it is not an error-bounded value. |
| `30` in the DSP default budget | **exact** | Integer decimal-place budget documented by the DSP reference. |
| `50` in `setprecision(50)` | **exact** | Integer precision parameter. |
| `3` in the refused length-3 FFT | **exact** | Integer sample count. |

Every integer-valued component is **exact**, including the real parts of both spectra. The
irrational imaginary components of `default_spectrum` and `wide_spectrum` are
**precision-limited**: each is computed at a stated decimal-place budget, and the stored tail moves
when the budget moves. A complex value that contains one of those tails is precision-limited as a
whole even when its real component is exact. Neither tail is **error-bounded**: the page publishes no
final-error bound for either transform, and a decimal-place budget is not a bound. No value is
**parity-verified**: `movingavg` here is the engine's corrected arithmetic, not a reproduction of the
historical fixed-width rendering, which [ADR-004](../adr/004-thirty-digit-default.md) records as a
defect rather than an expected result.

## Verification

Run this from the repository root. It prints the output above on standard output and the script
writes no files. The command is the same in bash, PowerShell and Command Prompt.

```console
cargo run -p lovelace-run -- --file docs/tutorials/examples/signal-processing.ls --text
```

`bash tools/run-tutorials.sh` reruns all four tutorials on Ubuntu and Windows (Git Bash) and
compares this transcript with the executed script.

The independent checks are the transform definition and the budget pair: a unit impulse at sample 1
transforms to the four fourth roots of unity (`1`, `-1i`, `-1`, `1i`), the ramp's DC bin is its
sample sum `36`, and the irrational tails change with the budget from 30 to 50 places. The
[tutorial runner](../../tools/run-tutorials.sh) checks the recording; it does not re-derive the
transform.

The length-3 refusal quoted above is reproducible on both platforms:

```console
cargo run -p lovelace-run -- --eval 'fft([1, 2, 3])' --text
```

PowerShell and bash both accept the single quotes around the expression; in Command Prompt write
`--eval "fft([1, 2, 3])"` instead.

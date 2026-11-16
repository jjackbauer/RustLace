# `lovelace-bench`

The micro-benchmark harness, shaped like the C# reference harness so the two can be compared on one
machine with the same operations and the same operand sizes.

## Purpose

`lovelace-bench` prints a machine-readable result line plus a digest so two runs can be compared
byte-for-byte. The digest is what proves that changing the thread budget changes nothing
(`lovelace-bench/src/main.rs`).

## Inputs

### Command line

```text
lovelace-bench <op> <digits> [reps]
  op: add | sub | mul | div | factorial | tostring | parse | pi | sqrt | eig_sym_5 | eig_sym_8
```

`eig_sym_5` and `eig_sym_8` run the engine's `eig_sym` on a dense 5x5 / 8x8 symmetric matrix with
spectrum 1..n at the requested precision. The engine is built and warmed up outside the timed
region, so the reported milliseconds cover the `eig_sym` evaluation (including parsing the generated
matrix literal), not engine construction.

Output: `RESULT <op> <digits> <mean_ms> <median_ms>` plus a `DIGEST` line.

### Dependencies

`lovelace-nat`, `lovelace-int`, `lovelace-real`, `lovelace-lang`.

## Outputs

One `RESULT <op> <digits> <mean_ms> <median_ms>` line and one `DIGEST` line per run, on stdout.
`reps` defaults to 20 timed repetitions; the engine and the operand values are prepared outside the
timed region. The process exit status is 0 when the measurement completed. The digest is computed
from the operation's result during warm-up, so it describes the result, not the timing.

## Precision behaviour

Timings are wall-clock observations of the host and the build: mean and median milliseconds are not a
property of the crate, and they are not comparable across machines. The digest is the exact artefact
— it is stable across runs and across `LOVELACE_THREADS` settings, which is what makes a thread-budget
change provably result-neutral. The numeric work is the engine's: `digits` selects the precision an
operation runs at, `eig_sym` is precision-limited at that count, and no benchmark value is stored or
published beyond the digest.

## Errors

- Fewer than two arguments prints `usage: lovelace-bench <op> <digits> [reps]` to stderr and exits
  2.
- An unknown `op` prints `unknown op <op>` and exits 2.
- A non-numeric `digits` is not reported as a usage error: it panics at `expect("digits")`.
- A non-numeric `reps` is silently ignored and the default 20 is used.

## Limitations

- **Eleven operations, fixed.** There is no registry or plugin point for another operation and no
  size sweep: one run measures one operation at one digit count.
- **Mean and median only.** There is no variance, percentile, memory or allocation reporting, and no
  automatic comparison of two runs; the digest is the comparison mechanism.
- **No assertions.** The crate has no `#[test]` functions and is not a library: it reports numbers
  and relies on the digest staying equal.
- **Argument handling is thin.** A malformed `digits` panics rather than producing a diagnostic, and
  a malformed `reps` falls back to the default instead of failing.
- **Timings are host-dependent.** The numbers describe this build on this machine at this moment;
  they are evidence for the digest claim, not a published performance figure.

## Examples

### Using it

```console
$ cargo run --release -p lovelace-bench -- mul 10000 3
$ LOVELACE_THREADS=1 cargo run --release -p lovelace-bench -- mul 10000 3
```

Both runs print the same `DIGEST`; only the timings differ. That is the point of the tool.

### Tests

No `#[test]` functions: the crate is a measurement tool, not a library. Its correctness bar is that
the digest of a fixed operation is stable across runs and across `LOVELACE_THREADS` settings.

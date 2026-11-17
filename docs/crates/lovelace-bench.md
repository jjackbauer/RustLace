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
      | systems_mmc | systems_jackson | systems_mva
      | rational_add | rational_mul | rational_div_neg | rational_pow | rational_normalize
      | symbolics_diff
```

`eig_sym_5` and `eig_sym_8` run the engine's `eig_sym` on a dense 5x5 / 8x8 symmetric matrix with
spectrum 1..n at the requested precision. `systems_mmc` is the Erlang C path; `systems_jackson`
and `systems_mva` are the open-network and closed-MVA paths. The `rational_*` operations measure
the canonical kernel directly (construction, cross-cancelled multiplication/division, squaring
power and full-GCD normalization); `symbolics_diff` differentiates an exact expression through the
engine. The engine is built and warmed up outside the timed
region, so the reported milliseconds cover the `eig_sym` evaluation (including parsing the generated
matrix literal), not engine construction.

Output: an `ENV` line (host, architecture, profile, compiler, digit count, repetitions), a
`RESULT <op> <digits> <mean_ms> <median_ms>` line and a `DIGEST` line.

### Dependencies

`lovelace-nat`, `lovelace-int`, `lovelace-rational`, `lovelace-real`, `lovelace-lang`,
`lovelace-systems`.

## Outputs

One `ENV` line, one `RESULT <op> <digits> <mean_ms> <median_ms>` line and one `DIGEST` line per
run, on stdout. The `ENV` line is part of the result: a timing without the host, compiler and
profile is not comparable.
`reps` defaults to 20 timed repetitions; the engine and the operand values are prepared outside the
timed region. The process exit status is 0 when the measurement completed. The digest is computed
from the operation's result during warm-up, so it describes the result, not the timing.

## Precision behaviour

Timings are wall-clock observations of the host and the build: mean and median milliseconds are not a
property of the crate, and they are not comparable across machines; the `ENV` line says which
build produced them. The digest is the exact artefact — it is stable across runs, which is what
makes a result-neutral change provable. A rational digest is the canonical numerator/denominator,
never a decimal projection; a systems digest is the exact projected metric; a symbolic digest is
the exact expression. The numeric work is the engine's or the canonical kernel's: `digits` selects
the precision an operation runs at, `eig_sym` and the projected systems fields are
precision-limited at that count, and the exact values behind them are checked by the unit and
oracle tests rather than by the digest.

## Errors

- Fewer than two arguments prints `usage: lovelace-bench <op> <digits> [reps]` to stderr and exits
  2.
- An unknown `op` prints `unknown op <op>` and exits 2.
- A non-numeric `digits` is not reported as a usage error: it panics at `expect("digits")`.
- A non-numeric `reps` is silently ignored and the default 20 is used.

## Limitations

- **A fixed operation list.** There is no registry or plugin point for another operation and no
  size sweep: one run measures one operation at one digit count.
- **Mean and median only.** There is no variance, percentile, memory or allocation reporting, and no
  automatic comparison of two runs; the digest is the comparison mechanism.
- **No timing assertions.** The crate has no `#[test]` functions and is not a library: it reports
  numbers and relies on the digest staying equal. CI does not gate on elapsed time (RNL-CI-011
  requires reviewed measurements, not an assumed threshold).
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

Both runs print the same `DIGEST`; only the timings differ. The suite of representative
rational, symbolic, Erlang C, Jackson and MVA workloads and the measured comparison against the
pre-migration kernels are recorded in [../benchmarks-numerics.md](../benchmarks-numerics.md).

### Tests

No `#[test]` functions: the crate is a measurement tool, not a library. Its correctness bar is that
the digest of a fixed operation is stable across runs and across `LOVELACE_THREADS` settings.

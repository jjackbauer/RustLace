# `lovelace-bench`

The micro-benchmark harness, shaped like the C# reference harness so the two can be compared on one
machine with the same operations and the same operand sizes.

## Purpose

`lovelace-bench` prints a machine-readable result line plus a digest so two runs can be compared
byte-for-byte. The digest is what proves that changing the thread budget changes nothing
(`lovelace-bench/src/main.rs:1`).

## Command line

```text
lovelace-bench <op> <digits> [reps]
  op: add | sub | mul | div | factorial | tostring | parse | pi | sqrt | eig_sym_5 | eig_sym_8
```

`eig_sym_5` and `eig_sym_8` run the engine's `eig_sym` on a dense 5x5 / 8x8 symmetric matrix with
spectrum 1..n at the requested precision. The engine is built and warmed up outside the timed
region, so the reported milliseconds cover the `eig_sym` evaluation (including parsing the generated
matrix literal), not engine construction.

Output: `RESULT <op> <digits> <mean_ms> <median_ms>` plus a `DIGEST` line.

## Dependencies

`lovelace-nat`, `lovelace-int`, `lovelace-real`, `lovelace-lang`.

## Tests

No `#[test]` functions: the crate is a measurement tool, not a library. Its correctness bar is that
the digest of a fixed operation is stable across runs and across `LOVELACE_THREADS` settings.

## Using it

```console
$ cargo run --release -p lovelace-bench -- mul 10000 3
$ LOVELACE_THREADS=1 cargo run --release -p lovelace-bench -- mul 10000 3
```

Both runs print the same `DIGEST`; only the timings differ. That is the point of the tool.

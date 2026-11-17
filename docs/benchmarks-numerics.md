# Numerical-kernel benchmarks

This page records measured evidence for the rational consolidation (RNL-NUM-007, RNL-CI-011). A
timing without the environment is an anecdote; the `ENV` line printed by `lovelace-bench` is part of
the result.

## How to reproduce

```console
cargo build --release -p lovelace-bench
target/release/lovelace-bench rational_add 200 20
target/release/lovelace-bench rational_mul 200 20
target/release/lovelace-bench rational_div_neg 200 20
target/release/lovelace-bench rational_pow 200 20
target/release/lovelace-bench rational_normalize 200 20
target/release/lovelace-bench symbolics_diff 200 20
target/release/lovelace-bench systems_mmc 200 20
target/release/lovelace-bench systems_jackson 200 20
target/release/lovelace-bench systems_mva 200 20
```

Each line is:

```text
ENV op=<op> digits=<n> reps=<n> os=<os> arch=<arch> profile=<debug|release> compiler=<rustc --version>
RESULT <op> <digits> <mean_ms> <median_ms>
DIGEST <op> <digest-length>:<first 24 characters>
```

The `DIGEST` is the canonical numerator/denominator for a rational workload, the exact symbolic
expression for `symbolics_diff`, and the exact projected metrics for a systems workload. Run the
same op twice and require the same digest; a timing improvement with a changed digest is a
correctness change, not an optimization.

## Environment

| Field | Value |
|---|---|
| Host OS / architecture | Windows, x86_64 |
| Compiler | rustc 1.100.0 (0ef1bc386 2026-11-09) |
| Profile | release (`opt-level = 3`, thin LTO, one codegen unit) |
| Operands | 200 decimal digits (`a` 200 digits, `b` 100 digits) |
| Repetitions | 20 |
| Date | 2026-11-17 |

## Consolidated kernel against the two pre-migration implementations

The baseline is commit `8b124d1` (before the RNL-001 working tree). A standalone release binary in a
temporary `git worktree` called `lovelace_symbolics::Rat` and `lovelace_systems::Q` with the same
operand construction, digit count and repetition count. The numbers below are mean milliseconds.

| Operation | `symbolics::Rat` (old) | `systems::Q` (old) | `lovelace-rational::Rat` |
|---|---:|---:|---:|
| add | 0.009 | 0.192 | 0.239 |
| mul | 0.001 | 0.007 | 0.003 |
| div by a negative divisor | 0.005 | 0.116 | 0.109 |
| pow(17) | 0.033 | 13.319 | 2.135 |
| normalize a common factor | 0.001 | 0.007 | 0.003 |

**How to read the table.** The old `symbolics::Rat` columns are *not* a like-for-like baseline: that
implementation reduces only when both parts fit `u64`, so at 200 digits it skips the GCD that the
canonical kernel performs. Its speed is the cost of the reduced-only-when-`u64` limitation
(RNL-NUM-003), not evidence that it is a faster canonical kernel. The `systems::Q` column is
semantically comparable: it reduced with `Nat::gcd` after every operation.

Against `systems::Q`, the consolidated kernel is:

- about 2× faster for multiplication (0.007 ms vs 0.003 ms; cross-cancellation replaces the unreduced
  product and the post-cancel GCD is no longer repeated),
- essentially unchanged for division by a negative divisor (0.116 ms vs 0.109 ms),
- slower for addition (0.192 ms vs 0.239 ms), because it pays one extra denominator GCD to use the
  least common multiple,
- about 6× faster for `pow(17)` (exponentiation by squaring replaces seventeen multiplications), and
- also about 2× faster for normalize (0.007 ms vs 0.003 ms).

These measurements are from one host and one date. They are evidence for a review, not a CI timing
gate: the repository has no wall-clock threshold, and a regression must be investigated with a new
measurement at the commit under review.

## Systems and symbolic workloads

The harness already covered Erlang C (`systems_mmc`), Jackson (`systems_jackson`) and MVA
(`systems_mva`); `symbolics_diff` and the rational operations are the additions. Their digests are
exact values, so they also serve as cross-run correctness checks:

| Op (200 digits, 3 reps) | Mean ms | Digest head |
|---|---:|---|
| `systems_mmc` | 0.039 | `0.7(1)\|0.8\|0.(5)` |
| `systems_jackson` | 0.085 | `0.1(380952)\|2.(761904)\|0` |
| `systems_mva` | 3.418 | `24.999999964249311811305` |
| `symbolics_diff` | 0.096 | `10*x^9 + 15*x^4 + 7` |

The MVA digest is a decimal projection (30-digit rendering); its exact rational value is checked by
`lovelace-systems`'s unit tests and the independent oracle, not by the digest.

## Limitations

- Timing is single-threaded and depends on the host, allocator and CPU frequency; the `ENV` line
  exists so a reader can decide whether two numbers are comparable.
- The pre-migration Symbolics numbers measure a different algorithm (no large-operand reduction) and
  must not be quoted as a faster canonical kernel.
- The benchmark does not gate CI on time. It gates correctness through the digest and the numerical
  tests; a performance regression needs a reviewed measurement, as RNL-CI-011 requires.

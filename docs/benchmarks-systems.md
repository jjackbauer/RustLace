# Systems benchmarks

This page records measured `lovelace-systems` work, not a promise. A number here is a measurement
on the host named below, in the release profile, with the command and repetition count stated. No
wall-clock number is a correctness claim and no cross-machine comparison is implied.

## Method

`lovelace-bench` gained three operations:

| Op | Model | What the timed region does |
|---|---|---|
| `systems_mmc` | λ=8, μ=5, c=2 | build the exact rational model and solve M/M/2 (Erlang C) |
| `systems_jackson` | λ=20; FCFS api (S=1/50), db (S=1/100, V=2), search (S=1/200, V=3), delay cache (S=1/20, V=1) | build the four-station open Jackson network and solve every station + the network aggregate |
| `systems_mva` | closed N=20, Z=1/10; FCFS cpu (S=1/100), FCFS disk (S=1/50, V=2), delay net (S=1/500) | run the exact single-class MVA recurrence, 20 populations × 3 stations |

The `<digits>` argument selects the decimal rendering precision. The math is exact rational and
does not depend on it; the digest (rendered through `Q::to_decimal`) is printed so two runs can be
compared. `RESULT <op> <digits> <mean_ms> <median_ms>` is the machine-readable line.

```console
$ cargo build --release -p lovelace-bench
$ ./target/release/lovelace-bench systems_mmc 30 20
$ ./target/release/lovelace-bench systems_jackson 30 20
$ ./target/release/lovelace-bench systems_mva 30 20
```

## Host and profile

| Fact | Value |
|---|---|
| CPU | AMD Ryzen 9 5900X 12-Core Processor |
| OS | Microsoft Windows 11 Pro |
| Compiler | rustc 1.100.0 (0ef1bc386 2026-11-09) |
| Profile | `--release` (workspace `opt-level = 3`, thin LTO, codegen-units = 1) |
| Repetitions | 20 per row, warm-up outside the timed region |
| Date | 2026-11-17 |

## Measured

| Op | Digits | Mean ms | Median ms | Digest (first 24 chars) |
|---|---:|---:|---:|---|
| `systems_mmc` | 30 | 0.044 | 0.042 | `0.7(1)|0.8|0.(5)` |
| `systems_mmc` | 1000 | 0.043 | 0.042 | `0.7(1)|0.8|0.(5)` |
| `systems_jackson` | 30 | 0.113 | 0.108 | `0.1(380952)|2.(761904)|0` |
| `systems_jackson` | 1000 | 0.106 | 0.105 | `0.1(380952)|2.(761904)|0` |
| `systems_mva` | 30 | 4.433 | 4.489 | `24.999999964249311811305` |
| `systems_mva` | 1000 | 4.347 | 4.581 | `24.999999964249311811305` |

The digest length differs between the MVA rows (99 vs 3010 characters) because a longer period is
discovered inside a larger digit budget; the mathematical values are identical. The `systems_mmc`
and `systems_jackson` digests are identical at both budgets because their expansions close early.

## Reading the numbers

- The single-station solve is a fixed handful of exact operations; the four-station Jackson solve
  adds one station kernel and one forced-flow check per station and remains sub-millisecond.
- The closed MVA is the expensive case by design: `O(N*K)` exact rational operations with growing
  denominators. At N=20, K=3 the recurrence performs 60 station updates; 4.4 ms mean is the
  measured cost of that exact arithmetic on this host, not a target.
- `SolverOptions::max_rational_digits` (default 4096) bounds intermediate growth; a model whose
  exact intermediate exceeds it is refused with `RationalBudgetExceeded` instead of being rounded.
- The intermediate-population storage is opt-in: the final result keeps `O(K)` state, and
  `collect_populations` retains `N*K` entries only when `N*K <= max_state_space`. The test
  `the_collected_population_size_is_n_times_k` pins that shape without a timing assertion.

## What this page does not claim

- No speed comparison with the C# reference: the two harnesses share operation names, not a
  measurement environment.
- No absolute performance promise across CPUs, operating systems or profiles.
- No claim that an exact result is fast. The MVA row shows the cost of exactness; a caller needing
  a faster approximate multiclass solver has to wait for the P1 algorithm (none exists in P0).

# `lovelace-know`

The MGIR observation-driven behavioural graph — the Rust port of `Lovelace.Knowledge`.

## Purpose

The pipeline has five stages, each in its own module (`lovelace-know/src/lib.rs:1`):

1. `proposal` — the explicit proposal distribution q(z): deterministic one-dimensional sweeps, seeded
   Monte Carlo draws, frontier-guided bisection and held-out validation probes. Every draw is a pure
   function of the config and the seed.
2. `runner` — each script is executed by the **real** `lovelace-run` process over stdin/stdout;
   nothing is simulated.
3. `observation` — canonicalisation of the runner's JSON envelope into the observation sigma and the
   behavioural plane class.
4. `reducer` — plane clustering, boundary detection by finite differences, guard fitting and
   frontier marking.
5. `convergence` — the C1–C4 metrics and the explicit stopping criterion.

`graph` persists the product of a run, `query` renders read-only reports, and `cli` implements the
JSON-over-stdio command protocol (`lovelace-know/src/main.rs`).

## Public surface

Modules `cli`, `converge`, `convergence`, `exact`, `graph`, `json`, `model`, `observation`,
`proposal`, `query`, `random`, `reducer`, `runner`. `#![forbid(unsafe_code)]`.

## Runner selection

The runner binary is located by, in order: an explicit `runner` request field, the `LOVELACE_RUN`
environment variable, and an executable on `PATH`/beside the binary. If none is found the stage fails
with "lovelace-run binary not found; pass the 'runner' argument or set LOVELACE_RUN"
(`lovelace-know/src/runner.rs:201`–`248`).

## Dependencies

`lovelace-json`, `lovelace-nat`, `lovelace-int`.

## Tests

56 `#[test]` functions in `tests/` across 10 files: `convergence_tests.rs`, `reducer_tests.rs`,
`observation_tests.rs`, `exact_tests.rs`, `proposal_tests.rs`, `seed_reproduction.rs`,
`graph_tests.rs`, `json_format_tests.rs`, `cli_stdio_tests.rs` and `common/mod.rs`. The seed
reproduction test pins the frozen C# graph (seed 20240617).

## Using it

```console
$ cargo run -p lovelace-know -- --help
```

The binary speaks JSON over stdio; the desktop and the runner are not involved, but a run needs a
`lovelace-run` binary (`LOVELACE_RUN=<path>`).

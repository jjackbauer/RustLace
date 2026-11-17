# `lovelace-know`

The MGIR observation-driven behavioural graph — the Rust port of `Lovelace.Knowledge`.

## Purpose

The pipeline has five stages, each in its own module (`lovelace-know/src/lib.rs`):

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

## Inputs

### Public surface

Modules `cli`, `converge`, `convergence`, `exact`, `graph`, `json`, `model`, `observation`,
`proposal`, `query`, `random`, `reducer`, `runner`. `#![forbid(unsafe_code)]`.

### Command line and request fields

The binary takes one request, by `--eval <json>`, `--file <request.json>` or `--stdin`; `--help`/`-h`
prints the usage. A request carries `command`, `config`, `graphPath`, `runner`, `seed`, `batchSize`,
`maxSamples` and `query`; the commands are `config`, `sample`, `reduce`, `converge`, `query` and
`help`. The `runner` field is the per-request override described below.

### Runner selection

The runner binary is located by, in order: an explicit `runner` request field, the `LOVELACE_RUN`
environment variable, and an executable on `PATH`/beside the binary. If none is found the stage fails
with "lovelace-run binary not found; pass the 'runner' argument or set LOVELACE_RUN"
(`resolve_runner`).

### Dependencies

`lovelace-json`, `lovelace-nat`, `lovelace-int`, `lovelace-rational`. `ExactNumber` is a facade
over the canonical `Rat`: it owns the Lovelace literal grammar and renderer, but no numerator/
denominator storage, GCD normalization or arithmetic.

## Outputs

One JSON response document per request on stdout, with the full field set and nulls for unused
members (the C# `CliResponse` shape). A successful run returns its result — the config, a sample
batch, the reduced graph, the convergence metrics or a query report — and persists the behavioural
graph through `graph` when the command produces one. The process exit status is 0 on success, 1 on a
command or execution error and 2 on a usage error; an error response is still a JSON document, so a
caller can parse the failure instead of reading stderr.

## Precision behaviour

Every proposal draw is a pure function of the config and the seed, so a run reproduces exactly: the
same seed and config give the same samples, the same graph and the same metrics. Observations are
canonicalised from the real runner's JSON envelope, so exactness is the envelope's and the graph
records the observed behaviour rather than re-deriving it. The seed-reproduction test pins the frozen
C# graph at seed 20240617; changing the seed produces a different graph, not a perturbed version of
the same one. The convergence metrics (C1–C4) are computed over the observed graph and are compared
as values, not rounded.

## Errors

- A usage error (unknown argument, missing request, `--eval`/`--file` without its argument) prints
  `Error: <message>` and the usage to stderr and exits 2.
- A command or execution error returns a JSON `CliResponse` error document on stdout and exits 1.
- A request file or stdin that cannot be read produces an error response with `cannot read request
  file: <error>` / `cannot read stdin: <error>` and exits 1.
- A run with no resolvable runner fails with the named `resolve_runner` message above, not by
  simulating the operation.

## Limitations

- **A run needs a `lovelace-run` binary.** The crate does not evaluate scripts itself, so `LOVELACE_RUN`
  (or the `runner` field, or `PATH`) must resolve one before a sample stage can run.
- **The surface is the JSON-over-stdio protocol.** Embedding the crate means speaking that protocol or
  calling the same modules directly; there is no GUI or desktop integration path.
- **Six fixed commands.** `config`, `sample`, `reduce`, `converge`, `query` and `help`; there is no
  plugin point for another command and no streaming protocol.
- **Determinism is seed-scoped.** A seed or config change is a different graph; it is not a
  perturbation of the previous one, and results are not interchangeable across seeds.
- **No network path.** The graph is built from local runner processes and persisted to a local file;
  nothing here fetches a remote corpus or reports.
- **The crate is not the desktop or the runner.** It shells out to `lovelace-run` and parses its
  envelope, so a change to the envelope is a change to this crate's input contract.

## Examples

### Using it

```console
$ cargo run -p lovelace-know -- --help
```

The binary speaks JSON over stdio; the desktop and the runner are not involved, but a run needs a
`lovelace-run` binary (`LOVELACE_RUN=<path>`).

### Tests

56 `#[test]` functions in `tests/` across 10 files: `convergence_tests.rs`, `reducer_tests.rs`,
`observation_tests.rs`, `exact_tests.rs`, `proposal_tests.rs`, `seed_reproduction.rs`,
`graph_tests.rs`, `json_format_tests.rs`, `cli_stdio_tests.rs` and `common/mod.rs`. The seed
reproduction test pins the frozen C# graph (seed 20240617).

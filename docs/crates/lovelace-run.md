# `lovelace-run`

The non-interactive script runner and the agent-facing wire protocol.

## Purpose

The contract this binary keeps is the one an external harness depends on: one JSON document on
stdout and nothing else, a stable field set, and three exit codes (0 success, 1 script or diagnostic
failure, 2 usage). The `--text` flag selects human output. Positions in the envelope are UTF-16
code-unit offsets into the caller's text, which is what a consumer slices
(`lovelace-run/src/main.rs:1`).

## Public surface

The crate is a binary (`[[bin]] name = "lovelace-run"`) plus internal modules: `script_source`
(origin of the text), `schemas` (the declared envelope schema), `repl`, `golden_batch`,
`wire_batch`, `cancellation_batch`, `error_taxonomy_batch`, `record_schema_batch`, `plot_surface_wire`
and the parity batches. Two integration tests live in `tests/`: `repl.rs` and `recursion_depth.rs`.

## Command line

```text
lovelace-run (--eval <src> | --file <path> | --stdin | <path>) [options]
  --plot-dir <dir>      directory for plot output
  --plot-file <name>    plot file name (default plot.svg)
  --cancel-after <ms>   cancel the evaluation after the given time, returning the partial result
  --print-budget <n>    node budget for structured print output
  --omit-functions      publish an empty functions array
  --omit-variables      publish an empty variables array
  --functions [pattern] list the builtin registry (no evaluation)
  --function <name>     describe one builtin (no evaluation)
  --function-limit <n>  bound a registry listing
  --repl                interactive session (stdin/stdout; Ctrl+C exits)
  --json | --text       machine (default) or human output
```

(`main.rs:70`–`94`.)

## Dependencies

`lovelace-json`, `lovelace-lang`, `lovelace-int`, `lovelace-real` (the last two for the registry and
record surfaces). `lovelace-nat` is a dev-dependency.

## Tests

408 `#[test]` functions in `src/` plus 12 in `tests/`. Among them: the golden wire envelopes carried
from the C# oracle's fixtures (`docs/csharp-oracle/Lovelace.Run.Tests/fixtures/*`, driven by
`golden_batch.rs`), the cancellation ledger, the error taxonomy, and the record-schema declarations.

## Using it

```console
$ cargo run -p lovelace-run -- --eval "1/3" --text
= 0.(3) (Real)
```

Without `--text` the same evaluation prints one JSON document whose `result`, `variables`,
`functions`, `output`, `plot` and `diagnostics` fields are the published envelope.

# `lovelace-run`

The non-interactive script runner and the agent-facing wire protocol.

## Purpose

The contract this binary keeps is the one an external harness depends on: one JSON document on
stdout and nothing else, a stable field set, and three exit codes (0 success, 1 script or diagnostic
failure, 2 usage). The `--text` flag selects human output. Positions in the envelope are UTF-16
code-unit offsets into the caller's text, which is what a consumer slices
(`lovelace-run/src/main.rs`).

## Inputs

### Public surface

The crate is a binary (`[[bin]] name = "lovelace-run"`) plus internal modules: `script_source`
(origin of the text), `schemas` (the declared envelope schema), `repl`, `golden_batch`,
`wire_batch`, `cancellation_batch`, `error_taxonomy_batch`, `record_schema_batch`, `plot_surface_wire`
and the parity batches. Two integration tests live in `tests/`: `repl.rs` and `recursion_depth.rs`.

### Command line

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

(`usage`).

### Dependencies

`lovelace-json`, `lovelace-lang`, `lovelace-int`, `lovelace-real` (the last two for the registry and
record surfaces). `lovelace-nat` is a dev-dependency.

## Outputs

In the default `--json` mode the process writes one JSON envelope on stdout. The envelope's `result`
carries the value's `display`, `kind`, `typed` and `structured` forms; `variables`, `functions`,
`output`, `plot` and `diagnostics` are the other published fields, with `protocolVersion`,
`mathIrVersion`, `symbolicFormatVersion`, `revision`, `elapsed`/`elapsedTime`, `timings` and
`artifacts` describing the run. `--text` prints the human form (`= 0.(3) (Real)`) instead; the REPL
shares the binary but not the envelope. `--functions`/`--function` publish a registry listing or one
descriptor without evaluating anything, and `--omit-functions`/`--omit-variables` send an empty array
for the field they name.

## Precision behaviour

Every numeric value in the envelope is the engine's: its `display`/`typed` rendering, its `kind` and
its `structured` projection are what `lovelace-lang` produced, and the exactness a consumer sees in
`structured` is the value's, not the runner's. Positions are **UTF-16 code-unit offsets** into the
caller's text — byte offsets and character offsets are both wrong for a consumer that slices.
`--print-budget` bounds structured print output by node count, and `--cancel-after` bounds evaluation
time and returns the partial result; neither changes a value that was already produced.

## Errors

- Exit 0: the script ran and no diagnostic failed the run.
- Exit 1: a script or diagnostic failure. In `--json` mode the envelope is still printed (with its
  `diagnostics`), and the process then exits 1; in `--text` mode the error line goes to stderr.
- Exit 2: a usage error — conflicting or missing source flags, an unknown option, a missing option
  argument. The message and the usage block go to stderr.
- An evaluation error carries the engine's `code`, `category`, `message` and UTF-16 `position` in the
  envelope's diagnostics; `--cancel-after` reports cancellation as a structured refusal, not as an
  empty success.

## Limitations

- **One document on stdout.** The contract is one JSON document and nothing else; a script's own
  output travels in `output`, and a consumer that expects a stream gets a single envelope.
- **One script per invocation.** Apart from `--repl`, a run takes exactly one source and ends. The
  REPL is interactive and does not produce the envelope.
- **No plugin or family selection.** The engine always loads all 134 builtins; there is no flag that
  removes a family, and `--functions` reports all of them.
- **UTF-16 positions.** A consumer must slice by UTF-16 code units, not bytes or Unicode scalar
  values.
- **Cancellation is post hoc in the envelope.** `--cancel-after` returns the partial result and a
  structured refusal; it does not roll the session back, and state produced before the cancel
  remains in the envelope.
- **Registry listing is bounded by default.** `--function-limit` bounds a listing (50 by default), so
  a caller that wants every name must raise it or use `--function` for one descriptor.

## Examples

### Using it

```console
$ cargo run -p lovelace-run -- --eval "1/3" --text
= 0.(3) (Real)
```

Without `--text` the same evaluation prints one JSON document whose `result`, `variables`,
`functions`, `output`, `plot` and `diagnostics` fields are the published envelope.

### Tests

Tests live in `src/` and `tests/`. Among them: the golden wire envelopes carried
from the C# oracle's fixtures (`docs/csharp-oracle/Lovelace.Run.Tests/fixtures/*`, driven by
`golden_batch.rs`), the cancellation ledger, the error taxonomy, and the record-schema declarations.

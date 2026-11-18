# `lovelace-corpus`

Deterministic corpus support and the documentation-corpus parity gate.

## Purpose

The binary is the parity gate: it runs every example in `parity/corpus/*.json` through the engine and
compares the observed rendering with the documented one. Exit code 0 means every example matched; 1
means at least one diverged; 2 means the corpus could not be loaded. "The gate is deliberately loud:
a missing corpus file is a failure, never an empty pass" (`lovelace-corpus/src/main.rs`).

The library adds two additive surfaces used to reproduce the C# `Lovelace.Symbolics.Tests`
property corpora exactly: `dotnet_random` is a faithful port of the seeded `System.Random`
subtractive generator that .NET keeps for compatibility, and `property_corpus` builds the seven
generated corpora with the same draw order as the C# tests
(`lovelace-corpus/src/lib.rs`).

### Provenance of the corpus

`parity/corpus/lang.json` (111 records) and `parity/corpus/symbolics.json` were extracted from
`Language.md` in the original Lovelace suite repository (the external `Lovelace.Suite` tree, not a path
in this repository); each record carries `{id, doc, heading, script, kind, text}` with
`kind` ∈ value | error | prints | plot. `parity/corpus/systems.json` (17 records) is the systems
family's own fixture: the documented `sys_*` examples with their observed typed results, including
unstable and unsupported-model refusals. The corpus is the yardstick the whole port is measured
against; it was carried into this repository so the gate runs on a clean clone
(`docs/rustlace-notes.md`).

## Inputs

### Public surface

- Binary `lovelace-corpus`: `[filter] [--verbose]`, corpus read from
  `<crate>/../parity/corpus/{lang,symbolics}.json` (resolved from `CARGO_MANIFEST_DIR`, so it works
  from any working directory).
- Library: `dotnet_random`, `property_corpus`, and the crate-private `fnv1a64` regression checksum
  used to compare transcripts by hash.

### Dependencies

`lovelace-json`, `lovelace-lang`, `lovelace-symbolics`.

## Outputs

The binary writes one summary line per run — `corpus: N passed, M failed, T total`, with
`(filter <prefix>)` when a filter is given — and, with `--verbose`, one `PASS <id> | <value>` line
per matching record. The exit status is the result: 0 when every selected record matched, 1 when at
least one diverged, 2 when the corpus could not be loaded. The library returns the .NET-compatible
draws from `dotnet_random`, the seven generated corpora from `property_corpus`, and the checksum
used to compare transcripts by hash.

## Precision behaviour

The gate compares the **documented rendering**, as text, with the observed one. A change in a display
form is therefore a failure even when the underlying value is equal, and a numerical claim in a
corpus record is only ever as good as the record: the gate proves reproduction, not mathematical
correctness. `dotnet_random` reproduces the seeded `System.Random` subtractive generator draw for
draw, which is what makes the property corpora comparable with the C# tests; `property_corpus` uses
the same draw order, so an added draw shifts every later value exactly as it would in the reference.
`fnv1a64` is a checksum over the transcript, not a numeric result.

## Errors

- Exit 1: at least one corpus record diverged; the run reports how many passed and failed.
- Exit 2: the corpus could not be loaded — a missing or malformed corpus file is a failure, never an
  empty pass. An **empty corpus** (zero records in the files) is also a `2` with `corpus is empty:
  refusing a vacuous pass`.
- A filter that matches nothing is not an error: the run prints `corpus: 0 passed, 0 failed, 232
  total (filter <prefix>)` and exits 0, so a mistyped selector passes vacuously while selecting
  nothing.
- The library functions return their values directly; there is no error enum on that surface.

## Limitations

- **The corpus is a fixed fixture.** Three JSON files, 232 records in total; the gate does not generate
  or update a corpus, and there is no record mode — adding a record is a source change plus a JSON
  edit.
- **The filter is a prefix.** The positional argument selects records whose `id` starts with it
  (`lang/101`, `lang/`), so it cannot express a predicate or a negative selection. A selector that
  matches nothing exits 0 with `0 passed`, so the loud-failure rule protects against a missing
  corpus, not against an empty selection.
- **The corpus path is fixed.** It is resolved from `CARGO_MANIFEST_DIR` and the binary has no flag
  to point at another corpus, which keeps the gate honest but makes it unfit for ad-hoc corpus runs.
- **Two options only.** `[filter] [--verbose]`; there is no machine-readable per-record output, no
  timeout control and no way to select the error kind.
- **The library is purpose-built.** `dotnet_random` and `property_corpus` exist to reproduce the C#
  property corpora; they are not a general RNG or corpus generator.
- **Parity, not correctness.** A matching record proves the implementation reproduces the documented
  text; it does not independently verify the mathematics.

## Examples

### Using it

```console
$ cargo run --release -p lovelace-corpus
corpus: 232 passed, 0 failed, 232 total

$ cargo run --release -p lovelace-corpus -- --verbose
PASS lang/001 | 42
...

$ cargo run --release -p lovelace-corpus -- lang/101
corpus: 1 passed, 0 failed, 232 total (filter lang/101)
```

The positional argument is an **id prefix** (`id.starts_with(filter)`), so `lang/101` selects one
record and `lang/` selects the language corpus.

### Tests

19 `#[test]` functions in `src/` (the property-corpus assertions and the .NET RNG reproduction).

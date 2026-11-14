# `lovelace-corpus`

Deterministic corpus support and the documentation-corpus parity gate.

## Purpose

The binary is the parity gate: it runs every example in `parity/corpus/*.json` through the engine and
compares the observed rendering with the documented one. Exit code 0 means every example matched; 1
means at least one diverged; 2 means the corpus could not be loaded. "The gate is deliberately loud:
a missing corpus file is a failure, never an empty pass" (`lovelace-corpus/src/main.rs:1`).

The library adds two additive surfaces used to reproduce the C# `Lovelace.Symbolics.Tests`
property corpora exactly: `dotnet_random` is a faithful port of the seeded `System.Random`
subtractive generator that .NET keeps for compatibility, and `property_corpus` builds the seven
generated corpora with the same draw order as the C# tests
(`lovelace-corpus/src/lib.rs:1`).

## Public surface

- Binary `lovelace-corpus`: `[filter] [--verbose]`, corpus read from
  `<crate>/../parity/corpus/{lang,symbolics}.json` (resolved from `CARGO_MANIFEST_DIR`, so it works
  from any working directory).
- Library: `dotnet_random`, `property_corpus`, and the crate-private `fnv1a64` regression checksum
  used to compare transcripts by hash.

## Provenance of the corpus

`parity/corpus/lang.json` (111 records) and `parity/corpus/symbolics.json` were extracted from the C#
`Lovelace.Suite/docs/Language.md`; each record carries `{id, doc, heading, script, kind, text}` with
`kind` ∈ value | error | prints | plot. The corpus is the yardstick the whole port is measured
against; it was carried into this repository so the gate runs on a clean clone
(`docs/rustlace-notes.md`).

## Dependencies

`lovelace-json`, `lovelace-lang`, `lovelace-symbolics`.

## Tests

19 `#[test]` functions in `src/` (the property-corpus assertions and the .NET RNG reproduction).

## Using it

```console
$ cargo run --release -p lovelace-corpus
corpus: 214 passed, 0 failed, 214 total

$ cargo run --release -p lovelace-corpus -- --verbose
PASS lang/001 | 42
...

$ cargo run --release -p lovelace-corpus -- lang/101
corpus: 1 passed, 0 failed, 214 total (filter lang/101)
```

The positional argument is an **id prefix** (`id.starts_with(filter)`), so `lang/101` selects one
record and `lang/` selects the language corpus.

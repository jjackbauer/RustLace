# RustLace: repository notes

This repository is the Rust workspace that lived under `rust/` in the
LovelaceSharp repository, promoted to the repository root. The history is the
history of that subtree only; the C#-era commits are not part of it.

## Fixtures carried out of the Rust subtree

A few tests read files that lived outside `rust/`. They are carried here so the
tests run on a clean clone (the alternative would be deleting or weakening the
tests):

- `docs/goal-desktop-v4/journeys.md`, `platform-matrix.md`, `reference/*`, and
  `round-64/REF-cancel.json` — read by `lovelace-desktop`'s journey, workbench,
  cancel-parity and platform-claims tests. The relative expressions that used to
  climb out of `rust/` lost one `..`.
- `docs/csharp-oracle/Lovelace.Symbolics/README.md` and
  `docs/csharp-oracle/Lovelace.Symbolics.Tests/UsageExamples.cs.txt` — read by
  the ported `DocsSyncTests` in `lovelace-lang`. They are C#-side documentation
  oracles; the checks they feed are unchanged. The usage-examples file keeps its
  original bytes but carries a `.txt` suffix so the repository contains no `.cs`
  path.
- `docs/csharp-oracle/Lovelace.Run.Tests/fixtures/*` — the golden wire
  envelopes and scripts that `lovelace-run`'s golden batch compares against.
- `docs/symbolics/dsh-protocol.md` — the protocol examples
  `lovelace-run`'s `wire_batch` test checks against the binary.
- `parity/corpus/{lang,symbolics}.json` — the corpus `lovelace-corpus` runs; the
  binary is the parity gate and fails loudly when the corpus is missing.

## Known residual: two C# oracle scans have no inputs

`lovelace-lang/src/dsp_round21.rs` ports three C# tests that scan the
`Lovelace.Dsp`, `Lovelace.Complex` and `Lovelace.Real` C# sources for forbidden
constructs. Those C# sources are not part of this repository, so the scans walk
no files and the assertions pass vacuously. They are kept because deleting them
would change the test surface; the original C# sources and their history remain
in the LovelaceSharp repository. The scan root is resolved against this
repository's root so the test never reads outside the clone.

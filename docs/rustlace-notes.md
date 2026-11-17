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

## Replaced: the two C# source scans

`lovelace-lang/src/dsp_round21.rs` originally ported two C# tests that scanned the
`Lovelace.Dsp`, `Lovelace.Complex` and `Lovelace.Real` C# sources. Those sources are not part of
this repository, so the scans walked no files and passed vacuously. They are now **replaced** by
scans of the Rust product files that carry the same contract: a missing or empty file panics, and
every `f32`/`f64` token in the selected DSP/complex/real value-path modules is a violation. The
interchange helper (`from_f64`) and the native-float benchmark are deliberately outside the scanned
list because they exist to compare against binary floats, not to compute. The original C# sources
and their history remain in the LovelaceSharp repository.

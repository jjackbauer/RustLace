# Testing

RustLace is tested in five layers. Each layer answers a different question, and none of them is
sufficient alone.

## 1. Unit tests (in-crate `#[test]`)

Every crate keeps its tests beside the code as `#[cfg(test)]` modules: parity rounds
(`roundNN_*.rs`), focused suites and, where a port needed it, a falsification harness
(`lovelace-lang/src/falsification_round46/`). The counts below are the ones observed on this tree
(`cargo test --workspace --no-fail-fast`):

| Crate | Passed | Ignored |
|---|---|---|
| `lovelace-real` | 2487 | 0 |
| `lovelace-lang` | 1737 | 0 |
| `lovelace-desktop` | 716 | 0 |
| `lovelace-run` | 438 | 0 |
| `lovelace-nat` | 271 | 0 |
| `lovelace-symbolics` | 197 | 0 |
| `lovelace-int` | 135 | 0 |
| `lovelace-array` | 61 | 0 |
| `lovelace-corpus` (lib) | 18 | 1 |
| `lovelace-json` | 7 | 0 |
| `lovelace-bench` | 0 | 0 |
| **Total** | **6135** | **1** |

The single ignored test is `property_assertions::mathir_evaluates_like_symbolic`, annotated in place:
"blocked: numeric::evaluate and IrEvaluator differ by 1 ulp at 50 places (lovelace-lang)".

Those totals are composed from two runs on this tree: `cargo test --workspace --no-fail-fast`
reported **6130 passed, 5 failed** before the frozen knowledge-graph fixture was carried
(`e87ca2a`); the five failures were `lovelace-know`'s `convergence_tests` (2), `graph_tests` (2) and
`seed_reproduction` (1), all failing on the missing `knowledge-graph-demo.json`. With that fixture
present, `cargo test -p lovelace-know` passes (exit 0) and the total is **6135 passed, 1 ignored**.
If you clone a revision without the fixture, expect those five failures and nothing else.

`cargo test -p lovelace-desktop` alone reports `716 passed; 0 failed; 0 ignored`, which is the desktop
verification gate.

## 2. Oracle and golden tests

- **Documentation corpus** — `parity/corpus/{lang,symbolics}.json` (214 records) extracted from the
  C# language reference. `lovelace-corpus` runs every script and compares the rendered text:

  ```console
  $ cargo run --release -p lovelace-corpus
  corpus: 214 passed, 0 failed, 214 total
  ```

  Run the gate **in release**: on this host the debug binary overflows its main-thread stack at
  `lang/061` (`func fact(n) ... fact(5)`), while the release build of the same sources passes all 214.
  A missing corpus file is exit code 2, never a pass.
- **Golden wire envelopes** — `docs/csharp-oracle/Lovelace.Run.Tests/fixtures/*` compared by
  `lovelace-run`'s `golden_batch`. `LOVELACE_RECORD_GOLDENS=1` rewrites them; the switch is inert
  otherwise.
- **C# documentation oracle** — `lovelace-lang`'s `docs_sync` tests read the carried
  `docs/csharp-oracle/Lovelace.Symbolics/README.md` and `UsageExamples.cs.txt`.
- **Property corpora** — `lovelace-corpus` reproduces the C# `System.Random` stream and the seven
  generated corpora, then compares transcripts by FNV-1a hash.
- **Frozen knowledge graph** — `knowledge-graph-demo.json` (seed 20240617) is re-reduced by
  `lovelace-know`'s convergence, graph and seed-reproduction tests.

## 3. Desktop GUI harness

The window itself is driven two ways:

- **Headless run path** — `--gui-selftest` drives the same `Workbench` methods the window calls and
  prints one line per check:

  ```console
  $ cargo run -p lovelace-desktop -- --gui-selftest
  GUI_SELFTEST summary checks=20 failed=0
  ```

  This needs a writable `APPDATA`; with the default `%APPDATA%` denied, two checks
  (`plot`, `plot-dir`) fail with "the app-scoped directory is unusable" and the mode exits 1. That
  is the environment speaking, not the product.
- **Journeys** — `--journey J-02 J-03 ... --out <dir>` executes the recorded journeys against
  `docs/goal-desktop-v4/journeys.md` and their reference captures under
  `docs/goal-desktop-v4/reference/`.

The screenshots in `docs/images` were captured from the release binary with `PrintWindow`
(`docs/images/capture.ps1`), which is the only way to get a real frame when the desktop session is
locked or the window is occluded.

## 4. Source and design audits

| Mode | What it asserts |
|---|---|
| `--ui-audit` | no file under `src/ui` evaluates, constructs an engine or names the compiler |
| `--thread-audit` | every thread spawn, sleep and synchronisation primitive is registered |
| `--glyph-check` | every mathematical glyph the engine emits is drawable or has a documented fallback |
| `--contrast-check` | every token pair reaches 4.5:1 (text) or 3:1 (large text/non-text); passes with `contrast: every required pair passes` |
| `--legibility-check` | every type role is at least 11 pt, 1.2× line ratio, 4.5:1 |
| `--visual-audit` | the design-system tables are printed from the same constants the window paints |
| `--platform-parity`, `--platform-claims` | the platform behaviour report and the release matrix's claims |

## 5. Benchmarks

`lovelace-bench` (see [`crates/lovelace-bench.md`](crates/lovelace-bench.md)) prints a `RESULT` line
and a `DIGEST`; the digest is what proves a thread-budget change does not change the answer.

## Running a subset

```console
$ cargo test -p lovelace-lang round67            # one round's tests
$ cargo test -p lovelace-real -- sqrt            # a name filter across the crate
$ cargo test -p lovelace-desktop -- --nocapture  # show test output
$ cargo test -p lovelace-run --test repl         # one integration target
$ cargo test -p lovelace-corpus -- --nocapture   # the property assertions
```

## What cannot run in CI

- **The GUI and its journeys.** They need a Windows desktop session, a real window and UI Automation.
  The workflow runs the engine, runner, corpus and knowledge suites; the desktop's unit tests build
  only on Windows because the GUI stack is a Windows-only dependency.
- **Timing-sensitive probes.** The `--measure-*` and `--perf-*` modes report measurements, not
  pass/fail, and the timing assertions inside `precision_default.rs` are generous bounds (2×) rather
  than machine-speed lotteries.
- **Anything that needs `%APPDATA%` writable** in a locked-down runner: redirect `APPDATA` first.

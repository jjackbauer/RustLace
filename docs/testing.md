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

`cargo test -p lovelace-desktop` alone reports `716 passed; 0 failed; 0 ignored` on Windows, which is
the desktop verification gate. On Linux the same command reports **423 passed** (420 unit tests plus
3 CLI integration tests): the `src/ui/**` modules are compiled only on Windows, so the Linux binary
carries the headless application core without the rendered editor, panes or plot rasteriser.
Two unit tests are Windows-only by premise and do not run there: the atomic-save read-only-target
test in `workbench.rs` (Unix renames over a read-only file successfully, because directory
permissions govern) and the PE-import self-check in `netcheck.rs` (the Linux test binary is ELF,
not a PE image). The cross-platform failed-write test in `document_io.rs` and the crate-source
network scan still cover both behaviours on Linux.

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

  The Linux build reports `checks=19 failed=0`: the editor's undo/redo check is compiled only on
  Windows, because the editor is part of the egui surface. Everything else in the run path is
  platform-independent and runs headlessly on Linux.

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

## 6. The Linux desktop job

The `linux` CI job runs the engine gates and then:

```console
$ cargo build --release -p lovelace-desktop
$ cargo test -p lovelace-desktop
```

and uploads `target/release/lovelace-desktop` as the artifact **`lovelace-desktop-linux-x86_64`**
(the Windows job keeps its own `lovelace-desktop-windows-x86_64` executable). The Linux binary is
the same headless application core: `--help`, `--version`, `--selftest` (14 checks, 0 failed on
Linux — see below), `--gui-selftest` (19 checks, 0 failed), `--ui-audit`, `--thread-audit`,
`--shortcuts` and `--platform-claims` all work. It has no window: the GUI stack is declared under
`[target.'cfg(windows)'.dependencies]`, so no `eframe`/`egui` code is compiled and the binary
prints the unsupported-platform notice and exits 2 when launched without a mode.

The job exists because the crate had never been compiled on Linux before it: the first
`cargo build --release -p lovelace-desktop` failed with

```text
error[E0599]: no method named `mode` found for struct `OpenOptions`
   --> lovelace-desktop/src/document_io.rs:505:17
```

on the `#[cfg(unix)]` atomic-save path, which called `OpenOptions::mode` without importing
`std::os::unix::fs::OpenOptionsExt`. That is a real portability defect the Windows-only job could
not have found, so the Linux build is not a cosmetic addition.

`--selftest` prints a named skip on Linux instead of silently dropping a check:

```text
SELFTEST skip no-network-imports: the check reads a PE import table and this build is not a PE image
SELFTEST summary checks=14 skipped=1 failed=0
```

`--help` on Linux lists only the modes that build actually contains (the Windows-only modes such
as `--journey` and `--raster-svg` are not advertised), and an unknown flag exits 2 with the usage
on stderr. `lovelace-desktop/tests/linux_cli.rs` asserts all three.

## What cannot run in CI

- **The GUI and its journeys.** They need a Windows desktop session, a real window and UI Automation.
  The Linux job builds and unit-tests the headless desktop crate, but the GUI shell, the editor
  buffer, the plot pane's rasteriser and every `--journey` run are exercised only on Windows. The
  headless checks are not GUI evidence and are not counted as any.
- **The PE-import proof off Windows.** `--selftest`'s `no-network-imports` check reads a Windows
  PE import table; on the Linux ELF binary it is reported as a skip with its reason. The
  crate-source scan for networking APIs still runs on both platforms.
- **Timing-sensitive probes.** The `--measure-*` and `--perf-*` modes report measurements, not
  pass/fail, and the timing assertions inside `precision_default.rs` are generous bounds (2×) rather
  than machine-speed lotteries.
- **Anything that needs `%APPDATA%` writable** in a locked-down runner: redirect `APPDATA` first.

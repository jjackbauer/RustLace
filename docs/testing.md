# Testing

RustLace is tested in five layers. Each layer answers a different question, and none of them is
sufficient alone.

No layer establishes mathematical correctness by itself. Automated wording and link checks can verify
consistency, but they cannot establish mathematical correctness: a numerical claim needs an
independent oracle, a reference calculation, or a stated error bound, and a suite that asserts nothing
about the value is a consistency check, not a proof.

## 1. Unit tests (in-crate `#[test]`)

Every crate keeps its tests beside the code as `#[cfg(test)]` modules: parity rounds
(`roundNN_*.rs`), focused suites and, where a port needed it, a falsification harness
(`lovelace-lang/src/falsification_round46/`). The counts below were observed on this tree with
`cargo test --workspace --no-fail-fast`; they are a carried observation, not a re-measurement in
this environment, and the `lovelace-desktop` row is the Windows figure.

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

At v0.4.1, `cargo test -p lovelace-desktop` (measured on Linux under `xvfb-run`) reports
**711 passed** in the binary plus **4 passed** in `tests/linux_cli.rs` — **715** in total, with
0 failed. That is the desktop verification gate. The Windows count was **not measured in this
environment**, and it is not inferred from the 711: ten tests in the crate do not compile on Linux.
The `src/ui/**` modules compile and run there; the ten tests that do not are Windows by premise or
live in a module still gated to Windows:

- `platform_parity` (3 tests): the PLF-010 report drives Win32 modifiers, dialogs, clipboard, the
  HWND IME state and DPI; the module is `#[cfg(windows)]`.
- `win32/shell.rs` (3 tests): the legacy Win32 shell is not compiled on Linux.
- `document_io.rs` — `a_failed_rename_keeps_the_read_only_original_byte_identical` and
  `a_successful_save_preserves_the_targets_file_attributes` (2 tests): Windows refuses to rename
  over a read-only target and exposes the hidden file attribute; on Unix a rename over a read-only
  file succeeds, because directory permissions govern.
- `workbench.rs` — `a_failed_atomic_save_keeps_the_original_and_its_recorded_snapshot` (1 test):
  the atomic-save failure premise is that same read-only rename.
- `netcheck.rs` — `this_test_binary_imports_no_networking_dll` (1 test): the test binary is an ELF
  image, not a PE one.

The cross-platform failed-write test in `document_io.rs` and the crate-source network scan still
cover both behaviours on Linux, and `--selftest` prints the PE check as a named skip with its
reason.

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

  The Linux build reports the same `checks=20 failed=0`: the editor's undo/redo check used to be
  Windows-only when the egui surface was, and it is unconditional since the Linux GUI port
  (`gui_selftest.rs`). Linux CI runs this under Xvfb and asserts the exact summary line, so the two
  platforms cannot silently diverge.

  This needs a writable application directory (`%APPDATA%` on Windows, `$XDG_CONFIG_HOME/lovelace`
  or `~/.config/lovelace` on Linux); with it denied, two checks (`plot`, `plot-dir`) fail with "the
  app-scoped directory is unusable" and the mode exits 1. That is the environment speaking, not the
  product.
- **Journeys** — `--journey J-02 J-03 ... --out <dir>` executes the recorded journeys against
  `docs/goal-desktop-v4/journeys.md` and their reference captures under
  `docs/goal-desktop-v4/reference/`. The mode and the journey unit tests are cross-platform since
  the Linux GUI port; CI runs the unit tests, not the `--journey` mode itself.

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

## 6. The Linux GUI job

The `linux` CI job is the Linux acceptance gate. After the engine gates it installs the GUI runtime
(the packages and why are in [build-and-run.md](build-and-run.md#linux)), then:

```console
$ cargo build --release -p lovelace-desktop          # the workbench, not a headless stub
$ xvfb-run -a -s "-screen 0 1600x1000x24" ./target/release/lovelace-desktop --gui-selftest
GUI_SELFTEST summary checks=20 failed=0
$ xvfb-run -a -s "-screen 0 1600x1000x24" cargo test -p lovelace-desktop
# 711 passed (bin) + 4 passed (tests/linux_cli.rs); see §1 for the measured total
```

The selftest step requires exit 0 **and** the exact `checks=20 failed=0` summary, so a Linux build
that quietly lost the undo/redo check fails CI even if every remaining check passes. The test step
is what compiles and runs the editor, pane, theme, plot-rasteriser and journey tests on Linux; they
did not exist there before the port. Both steps run under the same 1600x1000 Xvfb screen.

The job uploads `target/release/lovelace-desktop` as **`lovelace-desktop-linux-gui-x86_64`**
(renamed from `lovelace-desktop-linux-x86_64`, which described the pre-port headless build). The
Windows job is untouched and still uploads `lovelace-desktop-windows-x86_64`.

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

`--help` on Linux now advertises every mode the Linux build compiles, including `--journey` and
`--raster-svg`; it names the three Win32-only modes as needing the Windows build, and an unknown
flag exits 2 with the usage on stderr. `lovelace-desktop/tests/linux_cli.rs` asserts that help
contract, the named refusal of each Win32-only mode (exit 2, nothing on stdout) and the usage
error.

## What cannot run in CI

The Linux job now covers the workbench code and its run path; these are the boundaries it still does
not cover.

- **A real Windows window and its captures.** The Windows job compiles and tests the workspace,
  including the Windows build of the workbench, and builds the release executable, but no CI job
  opens or captures a window on Windows: the recorded frames and UI Automation captures come from
  the documenting host. On Linux the job runs the workbench code under Xvfb, not a mapped window;
  the one real Linux frame is `docs/images/gui-linux.png`, captured on WSLg. A `--gui-selftest` pass
  is a run-path check, not a window capture, and is not counted as one.
- **A captured frame from either renderer.** The Windows job compiles and tests the wgpu path and the
  Linux job proves the glow path's code, but neither job captures a rendered frame. The renderer
  split is evidenced by the Linux window capture, the Windows release build and the 20-check
  selftest, not by a per-frame CI image comparison.
- **The ten Windows-only tests listed above, and the three Win32-only modes on Linux.** They are
  Win32 premise or Win32 code, gated, and refused by name off Windows.
- **The PE-import proof off Windows.** `--selftest`'s `no-network-imports` check reads a Windows
  PE import table; on the Linux ELF binary it is reported as a skip with its reason. The
  crate-source scan for networking APIs still runs on both platforms.
- **Timing-sensitive probes.** The `--measure-*` and `--perf-*` modes report measurements, not
  pass/fail, and the timing assertions inside `precision_default.rs` are generous bounds (2×) rather
  than machine-speed lotteries.
- **Anything that needs a writable application directory** in a locked-down runner: redirect
  `APPDATA` on Windows, or `XDG_CONFIG_HOME`/`HOME` on Linux, first.

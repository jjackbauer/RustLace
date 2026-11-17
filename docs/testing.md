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
(`lovelace-lang/src/falsification_round46/`).

### The current aggregate, measured

Every count in the current table is an **exact** observation of a named revision; none of the rows
is carried forward as a current total. The aggregate below was measured on **Windows 11 x86_64**
on **2026-11-17**, in the `fix/release-provenance` working tree that contains the PR #9 review
remediation on top of
**`32ea6cd0f185e4e8f28d53369837085933ec6e65`** (`32ea6cd`, the RNL-001 implementation), with:

```console
$ cargo test --workspace --no-fail-fast
```

The run reports **45 test targets: 6276 passed, 0 failed, 0 ignored**. The per-crate rows below
are the same commands grouped by crate (`cargo test -p <crate>`, with the desktop under its normal
test path), measured in the same working tree; they sum to the same 6276 passed and 0 ignored.

| Crate | Test targets | Passed | Failed | Ignored |
|---|---:|---:|---:|---:|
| `lovelace-real` | 2 | 2493 | 0 | 0 |
| `lovelace-lang` | 3 | 1747 | 0 | 0 |
| `lovelace-desktop` (Windows) | 2 | 729 | 0 | 0 |
| `lovelace-run` | 3 | 454 | 0 | 0 |
| `lovelace-nat` | 2 | 271 | 0 | 0 |
| `lovelace-symbolics` | 2 | 197 | 0 | 0 |
| `lovelace-int` | 2 | 135 | 0 | 0 |
| `lovelace-array` | 2 | 61 | 0 | 0 |
| `lovelace-systems` | 3 | 55 | 0 | 0 |
| `lovelace-know` | 12 | 56 | 0 | 0 |
| `lovelace-corpus` | 5 | 28 | 0 | 0 |
| `lovelace-rational` | 4 | 30 | 0 | 0 |
| `lovelace-json` | 2 | 20 | 0 | 0 |
| `lovelace-bench` | 1 | 0 | 0 | 0 |
| **Total** | **45** | **6276** | **0** | **0** |

Rows that compose more than one target:

- `lovelace-rational` is 16 unit tests, 12 seeded property campaigns and 2 oracle comparisons, plus
  one zero-test doctest target.
- `lovelace-systems` is 51 unit tests, 3 independent-oracle tests and 1 doctest, plus one
  zero-test integration target.
- `lovelace-lang` is 1742 in-crate tests plus 5 cross-library semantics tests.
- `lovelace-run` is 442 in `src/main.rs` (including the additive `exact_values` wire test), 1 in
  `tests/recursion_depth.rs` and 11 in `tests/repl.rs`.
- `lovelace-know` is 56 across nine integration targets, with two zero-test unit targets and one
  zero-test doctest target. Its CLI-stdio tests spawn the real `lovelace-run` binary, so a
  standalone `cargo test -p lovelace-know` in a fresh target directory needs `cargo build -p
  lovelace-run` first (the test helper prints that instruction when the binary is missing);
  `cargo test --workspace` builds it as part of `lovelace-run`'s own test targets.

The earlier `fa87ecc` measurement counted one ignored test,
`property_assertions::mathir_evaluates_like_symbolic`, annotated at the time as "blocked:
numeric::evaluate and IrEvaluator differ by 1 ulp at 50 places (lovelace-lang)". The M0.2 work
re-enabled it: the divergence was an **evaluation-order** difference in the test's direct arm, which
called `substitute` before evaluation. Substitution folded `5/3 + 2/3` into `7/3`, so the test
compared `7/3 + B` against MathIR's original `(5/3 + B) + 2/3`; the separate 50-place truncations
differ by 1 ulp. Both arms now bind `x` and call `evaluate_bound`, and
`mathir_matches_direct_for_folded_constant_regression` pins item 28 at x=1/3. The current
`lovelace-corpus` suite has **zero ignored tests** and reports **28 passed** across five targets
(the property assertions, the lib unit test, the corpus-worker recursion test, the unsafe audit and
the workload tests); the 6,147 passed / 1 ignored figures remain an exact observation of
`fa87ecc`, not a current count.

### The reconciled historical table: 6,067 + 68 = 6,135

Before this edit the page listed eleven rows that summed to **6,067** while its Total row said
**6,135**. The 68-test difference is real, and row-level measurement splits it into the two test
targets the table omitted. It is not one crate:

| Crate | Passed | Ignored | Provenance of the row |
|---|---:|---:|---|
| `lovelace-real` | 2487 | 0 | row as recorded |
| `lovelace-lang` | 1737 | 0 | measured at `7ceeeba`, the revision this table describes |
| `lovelace-desktop` | 716 | 0 | **recorded on Windows; the record names no date or commit, and it is not measured here** |
| `lovelace-run` (`src/main.rs`) | 438 | 0 | row as recorded; unit-test target only |
| `lovelace-run` (`tests/`) | 12 | 0 | **added row**: 1 `recursion_depth` + 11 `repl`; 12 passed at v0.4.1 and at `fa87ecc` |
| `lovelace-nat` | 271 | 0 | row as recorded |
| `lovelace-symbolics` | 197 | 0 | row as recorded |
| `lovelace-int` | 135 | 0 | row as recorded |
| `lovelace-array` | 61 | 0 | row as recorded |
| `lovelace-corpus` (lib) | 18 | 1 | row as recorded |
| `lovelace-json` | 7 | 0 | row as recorded |
| `lovelace-bench` | 0 | 0 | row as recorded |
| `lovelace-know` | 56 | 0 | **added row**: 56 tests at `7ceeeba` (`--list`); 56 passed at v0.4.1 with the runner built and at `fa87ecc` |
| **Total** | **6135** | **1** | recorded aggregate, Windows |

The arithmetic is explicit:

```text
rows as originally written    2487+1737+716+438+271+197+135+61+18+7+0 = 6067
omitted: lovelace-know                                                   56
omitted: lovelace-run tests/                                             12
                                                                        ---
reconciled total                                                       6135
```

The earlier prose attributed the whole 68 to `lovelace-know` by subtraction. That attribution is
wrong: `lovelace-know` has 56 tests at both `7ceeeba` and `fa87ecc`, and the remaining 12 are
`lovelace-run`'s two integration-test targets, which the 438 row does not include. Subtracting the
rows from the total gave the right gap and the wrong split; listing every target makes the
composition visible.

`7ceeeba32d800317205a22b923f5954c6ce67389` (2026-11-14, "docs: document the whole RustLace
workspace") is the revision the old table was written for; `cargo test -p lovelace-lang` at that
revision measures **1737 passed, 0 failed, 0 ignored** on this host (2026-11-16), so 1737 is a
real snapshot of that commit rather than a stale count for v0.4.1 or for this branch. The 716
desktop row is carried from a Windows host with no date or commit attached to it, so the
**6,135 total is the page's recorded Windows aggregate, not a re-measured one**, and it is not
presented as current.

That historical composition also contains the fixture story the page used to carry:
`cargo test --workspace --no-fail-fast` reported **6130 passed, 5 failed** before commit
`e87ca2a` carried `knowledge-graph-demo.json`; the five failures were `lovelace-know`'s
`convergence_tests` (2), `graph_tests` (2) and `seed_reproduction` (1), all on the missing
fixture. With the fixture present, the same composition is **6135 passed, 1 ignored**. Those are
two runs behind the recorded historical total, not a current count.

### 1,737 versus 1,750: the `lovelace-lang` delta

The two numbers are measurements of different revisions and must not be mixed:

| Count | Revision | Command and host | Date |
|---:|---|---|---|
| 1737 | `7ceeeba32d800317205a22b923f5954c6ce67389` (2026-11-14) | `cargo test -p lovelace-lang`, Linux WSL Ubuntu 24.04.4 | 2026-11-16 |
| 1749 | `c7d9840cd87d9b488c29e52535516bebf4c53f1f` (v0.4.1, 2026-11-15) | `cargo test -p lovelace-lang`, Linux WSL Ubuntu 24.04.4 | 2026-11-16 |
| 1750 | `fa87ecc6abe41d3e174d62b211dd6b6fac2e29b7` (2026-11-15) | `cargo test -p lovelace-lang`, Linux WSL Ubuntu 24.04.4 | 2026-11-16 |

The +13 delta from 1737 to 1750 is accounted for by two commits:

- **+12** `eigen_jacobi_round` tests added by
  `5b5fe1087ee17ceb3ffb85ab7cf72085479433a0` ("feat: add eig_sym Jacobi symmetric eigensolver",
  2026-11-14), between `7ceeeba` and v0.4.1.
- **+3 − 2 = +1** movingavg tests: `1f114c9e5b76d5f8dd9e9ea50a96253543466f09`
  ("fix(movingavg): return the exact quotient instead of a defective reference rendering",
  2026-11-15) replaces the two fixed-width-rendering assertions with three exact-quotient tests.

The test-name lists at the three revisions were compared with `cargo test --lib -- --list`: the
only differences are the twelve `eigen_jacobi_round` names and five `dsp_round20::movingavg_*`
names (3 added, 2 removed). The 1737 row is therefore a snapshot of `7ceeeba`, not a current count.

Moving the recorded table to this branch's measured table is a two-row change:
`6,135 + 13` (the `lovelace-lang` tests above) `− 1` (the measured Linux desktop 715 replaces the
recorded Windows 716) `= 6,147`, which was the independently measured total at `fa87ecc`. The
6,135 side of that sentence is the recorded aggregate; it is not a Windows measurement made here.

### Windows and Linux are separate observations

- **Linux desktop, measured:** 711 passed in `src/main.rs` plus 4 in `tests/linux_cli.rs` —
  **715** in total, 0 failed, 0 ignored, at `fa87ecc` on 2026-11-16 under the command above. The
  earlier v0.4.1 record reports the same 711 + 4 split; this run re-measures it at the commit
  named above.
- **Windows desktop, recorded at `fa87ecc`:** **716 passed**, 0 failed, 0 ignored. That earlier
  measurement was carried from a Windows host, not inferred from 715. The RNL-001 working-tree
  measurement in §1 is the newer Windows observation: **729 passed** after the rational
  consolidation's added tests. Conversely, the Linux 715 is not inferred from either Windows row.

Ten tests in the crate do not compile on Linux, which is why the two totals differ. The
`src/ui/**` modules compile and run there; the ten tests that do not are Windows by premise or
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

- **Documentation corpus** — `parity/corpus/{lang,symbolics,systems}.json` (231 records) extracted from the
  C# language reference. `lovelace-corpus` runs every script and compares the rendered text:

  ```console
  $ cargo run --release -p lovelace-corpus
  corpus: 231 passed, 0 failed, 231 total
  ```

  Run the gate **in release** in CI. The corpus runs its scripts on a 256 MiB worker: the debug
  build used to overflow its main-thread stack at `lang/061` (`func fact(n) ... fact(5)`), and the
  worker removes that crash while keeping the same evaluation semantics.
  `lang_061_finishes_on_the_corpus_worker_stack` pins the recursive case in both profiles. A missing
  corpus file is exit code 2, never a pass. Both CI jobs run this command as a **blocking step**
  before building the release desktop, so a mismatch, panic, missing file or non-zero exit fails the
  workflow.
- **Golden wire envelopes** — `docs/csharp-oracle/Lovelace.Run.Tests/fixtures/*` compared by
  `lovelace-run`'s `golden_batch`. `LOVELACE_RECORD_GOLDENS=1` rewrites them; the switch is inert
  otherwise.
- **C# documentation oracle** — `lovelace-lang`'s `docs_sync` tests read the carried
  `docs/csharp-oracle/Lovelace.Symbolics/README.md` and `UsageExamples.cs.txt`.
- **Property corpora** — `lovelace-corpus` reproduces the C# `System.Random` stream and the seven
  generated corpora, then compares transcripts by FNV-1a hash.
- **Frozen knowledge graph** — `knowledge-graph-demo.json` (seed 20240617) is re-reduced by
  `lovelace-know`'s convergence, graph and seed-reproduction tests.
- **Independent rational oracle** — `tools/rational-oracle.py` generates
  `parity/rational/oracle.tsv` with Python's `fractions.Fraction` (seed 20261117);
  `lovelace-rational/tests/oracle.rs` recomputes every recorded case through the Rust API. The
  fixture is checked for staleness by `--check` in CI.
- **Independent systems oracle** — `tools/systems-oracle.py` re-derives M/M/1, Erlang C, MVA and
  open-Jackson metrics from their textbook definitions over `Fraction`; `lovelace-systems/tests/oracle.rs`
  compares the production models with `parity/systems/oracle.tsv`.
- **Cross-library consistency** — `lovelace-lang/tests/cross_library.rs` performs the same signed
  division and systems computation through the canonical kernel, Symbolics, Systems and the
  language, and compares exact values, signs, precision classification and error codes.
- **Architecture policy** — `tools/check-architecture.py` reads
  `tools/architecture-policy.json`, asks `cargo metadata` for the real graph, and scans Rust items.
  `tools/test-architecture-gates.py` has **20 scenarios**: the 11 original rules plus nine
  review-remediation scenarios (three owner-crate duplicates — a `Rat` in `lovelace-lang`, a `Rat`
  in `lovelace-real`, a second `Real` in `lovelace-real` — plus a split `impl`, a generic
  arithmetic trait, a free-function kernel, a renamed representation, a broad exception scope and
  an unresolved arithmetic impl). Each injects the
  forbidden change into an isolated fixture and requires a nonzero exit with the named rule; the
  valid fixture (7 crates, 7 files) proves domain formulas, typed units, a canonical alias and the
  permitted representations are accepted. Ownership is abstraction-specific through
  `reservedTypeNames` and `permittedRepresentations`; whole-crate `ownerCrates` is retired and
  rejected. Both run as the `architecture-policy` CI job, on Linux and Windows, independently of
  the unit tests; `.github/CODEOWNERS` declares the owner of the policy, checker and mutation
  suite.

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

`lovelace-bench` (see [`crates/lovelace-bench.md`](crates/lovelace-bench.md)) prints an `ENV` line,
a `RESULT` line and a `DIGEST`. The `ENV` line records the host, architecture, compiler, profile,
digit count and repetitions; the digest is what proves a result-neutral change (a thread-budget
change, an optimization) does not change the answer. The rational, symbolic, Erlang C, Jackson and
MVA workloads and the measured comparison against the two pre-migration kernels are in
[`benchmarks-numerics.md`](benchmarks-numerics.md). Timings are host observations, not a CI gate;
a regression needs a reviewed measurement at the commit under test (RNL-CI-011).

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
# measured at fa87ecc: 711 (bin) + 4 (tests/linux_cli.rs) = 715; §1 records the host and date
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

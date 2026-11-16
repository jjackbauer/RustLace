# Round 2 — implementation report

- **Agent**: Implementer (delegated by session-c00dfbb4), 2026-11-15
- **Tree**: `C:\Users\ricar\dev\RustLace`, git `bb9d21a`, version 0.4.1 (working tree already dirty before this round)
- **Environment**: WSL Ubuntu; runner `/tmp/rl-target/debug/lovelace-run`; suite run with `/home/ricardo/.cargo/bin/cargo` and `CARGO_TARGET_DIR=/tmp/rl-target`
- **Scope actually touched**: the seven originally listed files plus `docs/build-and-run.md`, `docs/extending.md` and `docs/editorial.md` (widened by the orchestrator mid-round). Nothing under `docs/goal-desktop-v4/`, `docs/goal-docs-v1/` (other than this report), any `.rs` file, or `README.md` was edited.

## 1. DOC-009 — one script extension

### 1.1 Before

Exact command (run in WSL from the repository root):

```console
$ grep -rn "\.lv\b" docs | grep -v goal-desktop-v4
```

```text
docs/build-and-run.md:57:$ cargo run -p lovelace-desktop -- notes.lv      # opened on a file
docs/build-and-run.md:148:$ cargo run --release -p lovelace-desktop -- notes.lv
docs/crates/lovelace-desktop.md:64:$ cargo run -p lovelace-desktop -- notes.lv     # the window, opened on a file
docs/editorial.md:80:   and fixtures use `.ls`. Do not introduce `.lv` in new material.
docs/extending.md:92:- **Golden wire envelope** – add `<name>.lv` and `<name>.json` under
docs/extensions/examples/run-examples.ps1:7:    For each pair, the snippet is written to a .lv file and evaluated with
docs/extensions/examples/run-examples.ps1:80:    $file = Join-Path $WorkDir ('snippet-' + [guid]::NewGuid().ToString('N') + '.lv')
docs/extensions/README.md:165:The runner writes each snippet to a `.lv` file and runs `lovelace-run --file <path> --text
docs/goal-docs-v1/deliverables.md:51:| DOC-009 `.lv` removal | TODO-004 |
docs/goal-docs-v1/evidence.md:16:| EVD-010 | `.lv` appears in 8 current documents (build-and-run, extending, journeys, extensions/README, language/README, plots, crates/lovelace-desktop, plus two `.ps1` runners), and 0 `.lv` files exist against 21 `.ls` files | `grep -rn "\.lv\b" docs`; `Get-ChildItem -Recurse -Include *.ls/*.lv` | Ran both; counted | High | 0 | TODO-004 |
docs/goal-docs-v1/goal.md:29:> | DOC-009 | P1 | **Establish a canonical script convention.** Document `.ls` and `.lv` behavior and consistently prefer one extension across introductory material. |
docs/goal-docs-v1/goal.md:82:| D10 | DOC-009 | `grep -rn "\.lv\b" docs README.md` classified into *usage* vs *documentation* | `.lv` documented exactly once (canonical `.ls`, `.lv` accepted for compatibility) and used in **zero** example command lines |
docs/goal-docs-v1/intent-audit.md:11:| 9 DOC-009 `.ls`/`.lv` convention | KEPT | mechanism ledger row DOC-009; D10 | goal.md:55; :80 D10 `zero hits in current material` | D10 gates only the "prefer one extension" half; requiring zero `.lv` hits removes the required documentation of `.lv` behavior instead of stating it. |
docs/goal-docs-v1/journal.md:56:- **Decision**: Current documentation uses `.ls` exclusively; `.lv` is mentioned only as a tolerated older extension in one place.
docs/goal-docs-v1/journal.md:59:- **Rationale**: EVD-010 — 21 `.ls` files, 0 `.lv` files; every committed fixture and `examples/hello.ls` use `.ls`.
docs/goal-docs-v1/journal.md:60:- **Alternatives considered**: canonicalizing `.lv` (rejected: would require renaming every fixture).
docs/goal-docs-v1/journal.md:123:### TODO-004: Remove `.lv` from current documentation
docs/goal-docs-v1/journal.md:125:- **Task**: Replace `.lv` with `.ls` in `docs/crates/lovelace-desktop.md:64`, `docs/extensions/README.md:165`, `docs/language/plots.md:188,191`, and confirm `docs/language/README.md:44` reads as a historical note rather than a second convention.
docs/goal-docs-v1/journal.md:236:- **Conclusion**: 17 KEPT / 1 REPLACED (#18 automated docs CI pipeline) / 1 REMOVED (#19 epistemic limit) is accurate. A third finding the summary understated: **six** dimensions are narrowed rather than removed (D10 deletes `.lv` behaviour instead of documenting it; D12 verifies existence, not enforcement; D13 checks links but not code references; D14 gates nothing on provenance isolation; D16 gates the line-number form of DOC-015 but not dead symbol citations; D3 greps two phrases rather than auditing claims). Those are corrected in goal.md before dispatch.
docs/goal-docs-v1/journal.md:245:- **Alternatives considered**: asking the user to approve the removal of #19 (rejected: the user named it, and it costs one paragraph and one grep to keep); leaving D10 as a zero-hit grep (rejected: it would delete the `.lv` documentation the user asked for).
docs/goal-docs-v1/state.md:52:Round 2: DOC-009 `.lv` removal (TODO-004), DOC-010 versioned counts in `docs/testing.md` (D11),
docs/images/capture.ps1:513:            $script = Write-Fixture 'squares.lv' @(
docs/images/capture.ps1:537:            $script = Write-Fixture 'plot.lv' @(
docs/images/capture.ps1:557:            $script = Write-Fixture 'symbolic.lv' @(
docs/images/capture.ps1:576:            $script = Write-Fixture 'find.lv' @(
docs/images/capture.ps1:629:            $script = Write-Fixture 'slow.lv' @(
docs/language/examples/run-examples.ps1:111:    $file = Join-Path $scratch ("case-$index.lv")
docs/language/plots.md:188:target/debug/lovelace-run.exe --file plot.lv --text --plot-dir target/example-plots --plot-file plot2.svg
docs/language/plots.md:191:where `plot.lv` contains `plot([0, 1, 2, 3], [0, 1, 4, 9], "squares")`, wrote
docs/language/README.md:44:so an older `.lv` file still works, but new material should use `.ls`.
```

**30 hits.** Inventory at the same time: `find . -name '*.ls' -not -path './target/*' | wc -l` → **21**; `... -name '*.lv' ...` → **0**. The golden fixture directory holds 20 `.ls` + 0 `.lv` (e.g. `absent_field.ls` beside `absent_field.json`).

### 1.2 After

Same command, after the edits, with this report file excluded (the report quotes the evidence):

```text
docs/extensions/examples/run-examples.ps1:7:    For each pair, the snippet is written to a .lv file and evaluated with
docs/extensions/examples/run-examples.ps1:80:    $file = Join-Path $WorkDir ('snippet-' + [guid]::NewGuid().ToString('N') + '.lv')
docs/goal-docs-v1/deliverables.md:51:| DOC-009 `.lv` removal | TODO-004 |
docs/goal-docs-v1/evidence.md:16:| EVD-010 | `.lv` appears in 8 current documents (build-and-run, extending, journeys, extensions/README, language/README, plots, crates/lovelace-desktop, plus two `.ps1` runners), and 0 `.lv` files exist against 21 `.ls` files | `grep -rn "\.lv\b" docs`; `Get-ChildItem -Recurse -Include *.ls/*.lv` | Ran both; counted | High | 0 | TODO-004 |
docs/goal-docs-v1/goal.md:29:> | DOC-009 | P1 | **Establish a canonical script convention.** Document `.ls` and `.lv` behavior and consistently prefer one extension across introductory material. |
docs/goal-docs-v1/goal.md:82:| D10 | DOC-009 | `grep -rn "\.lv\b" docs README.md` classified into *usage* vs *documentation* | `.lv` documented exactly once (canonical `.ls`, `.lv` accepted for compatibility) and used in **zero** example command lines |
docs/goal-docs-v1/intent-audit.md:11:| 9 DOC-009 `.ls`/`.lv` convention | KEPT | mechanism ledger row DOC-009; D10 | goal.md:55; :80 D10 `zero hits in current material` | D10 gates only the "prefer one extension" half; requiring zero `.lv` hits removes the required documentation of `.lv` behavior instead of stating it. |
docs/goal-docs-v1/journal.md:56:- **Decision**: Current documentation uses `.ls` exclusively; `.lv` is mentioned only as a tolerated older extension in one place.
docs/goal-docs-v1/journal.md:59:- **Rationale**: EVD-010 — 21 `.ls` files, 0 `.lv` files; every committed fixture and `examples/hello.ls` use `.ls`.
docs/goal-docs-v1/journal.md:60:- **Alternatives considered**: canonicalizing `.lv` (rejected: would require renaming every fixture).
docs/goal-docs-v1/journal.md:123:### TODO-004: Remove `.lv` from current documentation
docs/goal-docs-v1/journal.md:125:- **Task**: Replace `.lv` with `.ls` in `docs/crates/lovelace-desktop.md:64`, `docs/extensions/README.md:165`, `docs/language/plots.md:188,191`, and confirm `docs/language/README.md:44` reads as a historical note rather than a second convention.
docs/goal-docs-v1/journal.md:236:- **Conclusion**: 17 KEPT / 1 REPLACED (#18 automated docs CI pipeline) / 1 REMOVED (#19 epistemic limit) is accurate. A third finding the summary understated: **six** dimensions are narrowed rather than removed (D10 deletes `.lv` behaviour instead of documenting it; D12 verifies existence, not enforcement; D13 checks links but not code references; D14 gates nothing on provenance isolation; D16 gates the line-number form of DOC-015 but not dead symbol citations; D3 greps two phrases rather than auditing claims). Those are corrected in goal.md before dispatch.
docs/goal-docs-v1/journal.md:245:- **Alternatives considered**: asking the user to approve the removal of #19 (rejected: the user named it, and it costs one paragraph and one grep to keep); leaving D10 as a zero-hit grep (rejected: it would delete the `.lv` documentation the user asked for).
docs/goal-docs-v1/journal.md:314:- **Rationale**: The implementer reported that D10 ("no example command line in `docs/` uses `.lv`") cannot be satisfied inside the seven files I listed, because `docs/build-and-run.md:57,148` and `docs/extending.md:92` also carry `.lv`. The scope list was defective; the requirement is not. Widening the *round's* file set restores the named mechanism, whereas dropping the requirement would narrow the goal.
docs/goal-docs-v1/state.md:52:Round 2: DOC-009 `.lv` removal (TODO-004), DOC-010 versioned counts in `docs/testing.md` (D11),
docs/images/capture.ps1:513:            $script = Write-Fixture 'squares.lv' @(
docs/images/capture.ps1:537:            $script = Write-Fixture 'plot.lv' @(
docs/images/capture.ps1:557:            $script = Write-Fixture 'symbolic.lv' @(
docs/images/capture.ps1:576:            $script = Write-Fixture 'find.lv' @(
docs/images/capture.ps1:629:            $script = Write-Fixture 'slow.lv' @(
docs/language/examples/run-examples.ps1:111:    $file = Join-Path $scratch ("case-$index.lv")
docs/language/README.md:44:so an older `.lv` file still works, but new material should use `.ls`.
```

**23 hits; zero example command lines in current documentation.** Every remaining hit is one of:

- `docs/language/README.md:44` — the single behavioural compatibility statement, deliberately unchanged.
- `.ps1` scripts — `docs/extensions/examples/run-examples.ps1` (2), `docs/language/examples/run-examples.ps1` (1), `docs/images/capture.ps1` (5): these create temporary `.lv` files; explicitly out of scope per the orchestrator.
- `docs/goal-docs-v1/*` — this harness's own records; not documentation of the product. `journal.md:314` was added by the orchestrator during this round, not by me.

`docs/editorial.md:79` no longer appears in the sweep: it now reads "**`.ls` is the script extension.** Use it in every example; the compatibility note for other extensions lives in the language reference." That is prescriptive for contributors and states no acceptance behaviour, so the only behavioural statement of "the runner accepts any path" is `docs/language/README.md:43-44`.

This report itself quotes the before/after blocks, so a later `grep` over `docs/` will also match this page; that is evidence text, not a documentation example.

## 2. DOC-010 — one measured test count

### 2.1 The measurement

I re-ran the suite myself in this round (not just cited EVD-026):

```console
$ cd /mnt/c/Users/ricar/dev/RustLace
$ export CARGO_TARGET_DIR=/tmp/rl-target
$ xvfb-run -a -s "-screen 0 1600x1000x24" /home/ricardo/.cargo/bin/cargo test -p lovelace-desktop
```

Observed (exit 0):

```text
test result: ok. 711 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.45s
...
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

**711 + 4 = 715** on Linux under `xvfb-run` at v0.4.1. No Windows count was measured or inferred.

### 2.2 `docs/testing.md` — before/after

Before (lines 38-40 and 146):

```text
`cargo test -p lovelace-desktop` alone reports `716 passed; 0 failed; 0 ignored` on Windows, which is
the desktop verification gate. On Linux the same command reports **706 passed** in the binary plus
**4 passed** in `tests/linux_cli.rs` (710 in total): the `src/ui/**` modules compile there too, and
the ten tests that do not are Windows by premise or live in a module still gated to Windows:
```
```text
# 706 passed (bin) + 4 passed (tests/linux_cli.rs)
```

After (lines 39-44 and 149):

```text
At v0.4.1, `cargo test -p lovelace-desktop` (measured on Linux under `xvfb-run`) reports
**711 passed** in the binary plus **4 passed** in `tests/linux_cli.rs` — **715** in total, with
0 failed. That is the desktop verification gate. The Windows count was **not measured in this
environment**, and it is not inferred from the 711: ten tests in the crate do not compile on Linux.
The `src/ui/**` modules compile and run there; the ten tests that do not are Windows by premise or
live in a module still gated to Windows:
```
```text
# 711 passed (bin) + 4 passed (tests/linux_cli.rs); see §1 for the measured total
```

The measured total **715 appears once** in the page. The per-crate table above it still carries
`| lovelace-desktop | 716 | 0 |` — a workspace-run figure the brief says not to guess at — so the
table intro now marks it: "they are a carried observation, not a re-measurement in this
environment, and the `lovelace-desktop` row is the Windows figure." The table itself and the
workspace total of 6135 were left numerically untouched.

### 2.3 `AGENTS.md` — before/after

Before (lines 65-68 and 218):

```text
`cargo test -p lovelace-desktop` gives 716 passed on Windows and 710 on Linux (706 in the binary
plus 4 CLI integration tests; the ten tests that do not compile there are the Windows-only
`platform_parity` module and `win32/shell.rs`, and four Windows-premise tests in `document_io.rs`,
`workbench.rs` and `netcheck.rs`); `--gui-selftest` gives 20 checks, 0 failed on both platforms;
```
```text
One logical change per commit. "All tests pass" is not evidence; "716 tests, release x2 + debug
green" is.
```

After (lines 63-68 and 218):

```text
Observed counts on this tree: `cargo test --workspace` gives 6135 passed and 1 ignored (the
ignored test is `property_assertions::mathir_evaluates_like_symbolic`, annotated in place); the
desktop suite's measured count is recorded in
[docs/testing.md](docs/testing.md#1-unit-tests-in-crate-test) — measured on Linux under `xvfb-run`
at v0.4.1, with the Windows count not measured in this environment; `--gui-selftest` gives 20
checks, 0 failed on both platforms;
```
```text
One logical change per commit. "All tests pass" is not evidence; "a measured test count, release x2 + debug
green" is.
```

No 716/710/706 remains in `AGENTS.md`.

### 2.4 `docs/crates/lovelace-desktop.md` — before/after

Before (lines 54-55):

```text
716 `#[test]` functions on Windows, 706 plus the 4 in `tests/linux_cli.rs` on Linux, run by
`cargo test -p lovelace-desktop`. They cover the execution host, the
```

After (lines 54-56):

```text
The suite's measured counts are in [testing.md](../testing.md#1-unit-tests-in-crate-test): the Linux
run was measured under `xvfb-run` at v0.4.1, and the Windows count was not measured in this
environment. `cargo test -p lovelace-desktop` runs it; the tests cover the execution host, the
```

## 3. DOC-013 — the personal plot path

### 3.1 What was run

The example is `plot([0, 1, 2, 3], [0, 1, 4, 9], "squares")`. Script written to
`/tmp/doc013/work/plot.ls`; command run from the repository root:

```console
$ cd /mnt/c/Users/ricar/dev/RustLace
$ printf 'plot([0, 1, 2, 3], [0, 1, 4, 9], "squares")\n' > /tmp/doc013/work/plot.ls
$ /tmp/rl-target/debug/lovelace-run --file /tmp/doc013/work/plot.ls --text \
    --plot-dir target/example-plots --plot-file plot2.svg
```

Real observed stdout (exit 0), unedited:

```text
= /mnt/c/Users/ricar/dev/RustLace/target/example-plots/plot2.svg
  _ = /mnt/c/Users/ricar/dev/RustLace/target/example-plots/plot2.svg
```

`stat -c '%s' target/example-plots/plot2.svg` → **15263** bytes, matching the size the page already
documents.

### 3.2 Before/after in `docs/language/plots.md`

Before (lines 45-46):

```text
= C:\Users\ricar\dev\LovelaceSharp\rust\target-desktop\tmp\language-work\target\example-plots\plot2.svg
  _ = C:\Users\ricar\dev\LovelaceSharp\rust\target-desktop\tmp\language-work\target\example-plots\plot2.svg
```

After (lines 45-46):

```text
= <plot-dir>/plot2.svg
  _ = <plot-dir>/plot2.svg
```

The paragraph under it now says: "`--plot-dir <plot-dir> --plot-file plot2.svg`, run from the
repository root; `<plot-dir>` stands for the absolute directory passed to `--plot-dir` (this run
passed `target/example-plots`)." `<plot-dir>` is visibly a placeholder, so the block stays truthful
without a machine-specific path.

The two JSON envelope examples carried the same personal path (URL-escaped with double
backslashes); they now also use `"path":"<plot-dir>/plot2.svg"`, and the sentence above the first
one says "with the path shortened to the `<plot-dir>` placeholder". `bytes` stays 15263, the value
measured above. `grep -n 'ricar\|LovelaceSharp' docs/language/plots.md` → **no matches**.

## 4. `docs/architecture.md:38` — "non-Windows half"

Before:

```text
**Invariant.** No floating point appears in any value path. `#![forbid(unsafe_code)]` guards
`lovelace-array`, `lovelace-int`, `lovelace-know` and the non-Windows half of `lovelace-desktop`.
```

After:

```text
**Invariant.** No floating point appears in any value path. `#![forbid(unsafe_code)]` guards
`lovelace-array`, `lovelace-int` and `lovelace-know`; `lovelace-desktop` carries
`#![cfg_attr(not(windows), forbid(unsafe_code))]`, so the whole crate forbids unsafe code on
non-Windows targets.
```

Checked against `lovelace-desktop/src/main.rs:39`: `#![cfg_attr(not(windows), forbid(unsafe_code))]`.

## 5. Diff

`git diff --stat` (Windows Git, `core.autocrlf=true`, which is this checkout's convention):

```text
 AGENTS.md                       | 12 ++++++------
 docs/architecture.md            |  4 +++-
 docs/build-and-run.md           |  4 ++--
 docs/crates/lovelace-desktop.md |  7 ++++---
 docs/extending.md               |  2 +-
 docs/extensions/README.md       |  2 +-
 docs/language/plots.md          | 23 +++++++++++++----------
 docs/testing.md                 | 17 ++++++++++-------
 8 files changed, 40 insertions(+), 31 deletions(-)
```

Line-ending note: `HEAD` blobs are LF, while this checkout's working tree is CRLF throughout (it
was already CRLF before this round, and `docs/language/README.md` and other pre-existing modified
files show the same). Windows Git normalises that; WSL Git without `core.autocrlf` reports every
line as changed. The same semantic stat is reproducible in WSL with:

```console
$ git diff --ignore-cr-at-eol --stat -- <the same eight files>
# identical 8-file stat, 40 insertions(+), 31 deletions(-)
```

`docs/language/README.md` is in scope but **unchanged by this round** (its `M` status pre-dates it):
its lines 43-44 remain the single behavioural compatibility statement.

`docs/editorial.md` is untracked in this tree (it was already `??` before this round), so it does not
appear in `git diff --stat`. Its edit is the bullet at line 79. Before:

```text
6. **One script extension: `.ls`.** The runner accepts any path, but introductory material, examples
   and fixtures use `.ls`. Do not introduce `.lv` in new material.
```

After:

```text
6. **`.ls` is the script extension.** Use it in every example; the compatibility note for other extensions lives in the language reference.
```

## 6. One line per file

- `docs/crates/lovelace-desktop.md` — replaced the duplicated 716/706/710 counts with a citation of `testing.md`; changed the `notes.lv` launch example to `notes.ls` — one measured source, one extension.
- `docs/extensions/README.md` — line 165 described `run-examples.ps1` writing a `.lv` file; changed to "a temporary script file" rather than assert `.ls`, because the `.ps1` (out of scope) still writes `.lv` — removes `.lv` from current prose without a false claim.
- `docs/language/plots.md` — `plot.lv` → `plot.ls` in the worked example; replaced the personal Windows path in the observed-output block and both JSON envelopes with the `<plot-dir>` placeholder, keeping the re-measured 15263-byte output truthful.
- `docs/language/README.md` — untouched by this round (pre-existing `M`); lines 43-44 still say the runner accepts any path and `.ls` is the convention.
- `docs/editorial.md` — replaced the two-line bullet that restated the runner's acceptance behaviour with one prescriptive line: `.ls` is the script extension and the compatibility note lives in the language reference; no `.lv`, no second behavioural statement.
- `docs/testing.md` — records 711 (binary) + 4 (`tests/linux_cli.rs`) = 715 once, at v0.4.1, measured on Linux under `xvfb-run`; Windows explicitly not measured; table row labelled as the carried Windows figure; CI comment updated 706 → 711 with a pointer to §1.
- `AGENTS.md` — replaced the 716/710/706 desktop counts with a citation of `testing.md`; removed the stale 716 from the commit-message example; the workspace total 6135 was left alone.
- `docs/architecture.md` — corrected "the non-Windows half of `lovelace-desktop`" to the whole crate forbidding unsafe code on non-Windows targets via `cfg_attr`.
- `docs/build-and-run.md` — two `notes.lv` example command lines → `notes.ls` (scope widened by the orchestrator).
- `docs/extending.md` — golden wire-envelope fixture naming `<name>.lv` → `<name>.ls` (scope widened; the fixture directory is 20 `.ls`, 0 `.lv`), matching the corrected claim.

## 7. What I could not do / residuals

- **Windows test count**: not measured (no Windows Rust here) and not inferred. `docs/testing.md` says so; `AGENTS.md` and the crate page cite it.
- **Workspace-wide total**: left untouched (6135 / 1 ignored), as instructed.
- **`.lv` in `.ps1` scripts**: `docs/extensions/examples/run-examples.ps1`, `docs/language/examples/run-examples.ps1` and `docs/images/capture.ps1` still create temporary `.lv` files. Script behaviour, not documented convention; explicitly out of scope.
- **`C:\Users` placeholders**: after the plots fix, the pattern survives only as the placeholders `C:\Users\<you>` (`docs/build-and-run.md:65`) and `C:\Users\<someone>` (`docs/editorial.md:77`), plus this goal directory's records; no `ricar`/`LovelaceSharp` path remains in `docs/language/plots.md`. A literal zero-hit D14 grep would still match those two placeholders.
- **`docs/goal-docs-v1/`** remains full of `.lv` mentions by design (this harness's records). The after sweep above excludes this report, which necessarily quotes `.lv` in its evidence blocks.
- **Untracked file**: `docs/editorial.md` is not in Git, so the tracked `git diff --stat` cannot show it; its before/after bullet is recorded in §5.

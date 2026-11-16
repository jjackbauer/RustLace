# Round 7 — brittle source-citation repair (DOC-015 / D16)

Date: 2026-11-15
Owner slice: `docs/build-and-run.md`, `docs/extensions/{README,dsp,mathir,symbolics}.md`, and every
page under `docs/crates/`.

## What changed

Every checker-visible `.rs:NNN` line-suffix citation in the owned files was replaced with the
symbol that lives at that line, or with the bare repository-relative file path where the citation
pointed at crate-level documentation and no single symbol owns the claim. Each cited location was
read before choosing the replacement; no symbol was guessed. The two `.rs:NNN` comments inside
non-executed `` ```rust `` fences in `docs/extensions/README.md` were normalised the same way. No
`` ```lovelace `` block and no `` ```text `` block was modified; all 27 executable pairs still
match.

Total: **70 checker-visible line-suffix citations** (21 + 19 + 10 + 9 + 6 + 5), **2 fenced
code-comment citations**, and **1 broken external code reference** (the corpus provenance path).
The task brief said 18 citations across the crate pages; `check-docs.sh --symbols` listed 19
line-suffix citations there (plus the broken code reference). All 19 were fixed.

## Per-file conversion and prose changes

| File | Converted | Prose correction / note |
|---|---|---|
| `docs/build-and-run.md` | 21 citations at 20 sites | No claim changed. The `--legacy-win32` / `--ime-probe` / `--platform-parity` citation (`main.rs:148`, `:174`, `:287`) was consolidated to the `main` mode dispatch. Two stale cites were re-pointed to the real code (see below). |
| `docs/crates/lovelace-array.md` | 1 | `:1` crate doc → bare `lovelace-array/src/lib.rs`. |
| `docs/crates/lovelace-bench.md` | 1 | `:1` crate doc → bare path. |
| `docs/crates/lovelace-corpus.md` | 2 + 1 broken ref | Provenance sentence reworded: `Language.md` is now stated as an artifact of the original external `Lovelace.Suite` repository, not a path in this tree. |
| `docs/crates/lovelace-desktop.md` | 2 | `engine.rs:4` pointed at module prose, not the trait; now the `Evaluator` trait (declared at `engine.rs:380`). |
| `docs/crates/lovelace-int.md` | 1 | `:1` crate doc → bare path. |
| `docs/crates/lovelace-json.md` | 1 | `:1` crate doc → bare path. |
| `docs/crates/lovelace-know.md` | 2 | Crate doc → bare path; runner selection → `resolve_runner`. |
| `docs/crates/lovelace-lang.md` | 2 | Crate doc → bare path; `lib.rs:871` was the wrong line for `load_plugin` (actual 883). |
| `docs/crates/lovelace-nat.md` | 2 | Crate doc → bare path; worker/NTT knobs → `thread_budget`, `ntt_threshold`. |
| `docs/crates/lovelace-real.md` | 1 | `:1` crate doc → bare path. |
| `docs/crates/lovelace-run.md` | 2 | Crate doc → bare path; option block → `usage`. |
| `docs/crates/lovelace-symbolics.md` | 2 | Crate doc → bare path; re-export line (47 is the doc comment) → bare `lovelace-lang/src/lib.rs`. |
| `docs/crates/lovelace-desktop-spike.md` | 0 | No line-suffix citations. |
| `docs/extensions/README.md` | 10 + 2 fenced | Counts doc → `builtin_count_for_plugins`; `Engine::new`/`new_bare`/`load_plugin`; `ModusPlugin`; gate failure → `unknown_function_among`; test-module declarations and `StatisticsPlugin` → file-path/symbol cites. |
| `docs/extensions/dsp.md` | 9 | Gate/raise site → `unknown_function_among` + `Engine::call`; `dsp_precision`; `register_builtin` install path → bare `lovelace-lang/src/lib.rs`; module-level fixed-width claims → bare `fixed_dsp.rs`. |
| `docs/extensions/mathir.md` | 6 | `FORMAT_VERSION` definition → bare `mathir.rs`; header/version gate → `IrProgram::serialize`, `IrProgram::deserialize`; gate → `Engine::call`. |
| `docs/extensions/symbolics.md` | 5 | Gate → `unknown_function_among` + `Engine::call`; rewrite budget → `REWRITE_MAX_STEPS`; 40-place solver precision → `constant_real`, `numeric_residual_is_zero`. |

## Wrong citations found (claim verified, line was stale)

Each claim below was checked against the code and is accurate; only the line citation was wrong.

1. `docs/crates/lovelace-lang.md:54` — `lib.rs:871` for `load_plugin`; the function is at
   `lib.rs:883` (871 is inside `new_bare`).
2. `docs/crates/lovelace-desktop.md:40` — `engine.rs:4` for the `Evaluator` trait; line 4 is the
   module doc, the trait is at `engine.rs:380`.
3. `docs/build-and-run.md:209` — `ui/app.rs:999` for `LOVELACE_PLOT_FAULT`; line 999 is unrelated.
   The seam is `faulted_capture_svg` (`ui/app.rs:9656`).
4. `docs/build-and-run.md:210` — `ui/app.rs:996` for `LOVELACE_LOG_FLOOD`; line 996 is
   `ime_probe_run_state`. The switch is read in `launch` (`ui/app.rs:1012`) and implemented by
   `Workbench::flood_log_active` (`workbench.rs:2404`).
5. `docs/build-and-run.md:255` — `settings.rs:1` for `Settings::from_json`; line 1 is a module doc.
   The parser is at `settings.rs:613`.
6. `docs/extensions/symbolics.md:273` — `builtins.rs:12013` for `REWRITE_MAX_STEPS`; the constant
   is at `builtins.rs:12037`.
7. `docs/extensions/symbolics.md:275` — `builtins.rs:7989` for the solver's 40-place working
   precision; 7989 is `enum SolveBranch`. The two 40-place sites are `constant_real`
   (`builtins.rs:8012`) and `numeric_residual_is_zero` (`builtins.rs:9419`).
8. `docs/crates/lovelace-symbolics.md:21` — `lib.rs:47` is the doc comment for `pub mod cancel`
   at line 48. Replaced with a bare `lovelace-lang/src/lib.rs` citation.
9. `docs/build-and-run.md:186` — the three cited lines were in the order platform-parity,
   ime-probe, legacy-win32 while the prose lists them legacy, ime, parity; all three are branches
   in `main`. Consolidated to `main`.
10. `docs/extensions/dsp.md:91`, `docs/extensions/mathir.md:49` and
    `docs/extensions/symbolics.md:49` — `lib.rs:2146-2150` points inside `Engine::call` (the
    `plugin_enabled` gate); named as `Engine::call` rather than leaving an anonymous range.

## Not converted

- None of the checker-visible citations remain.
- Plain-prose line ranges that are not `.rs:NNN` citations (e.g. "lines 990-998" in the extensions
  pages, "plugin tags at lines 408-1611") were left untouched: they are outside the checker's
  citation pattern, and changing them would be a prose rewrite rather than a citation-format
  repair.
- The `docs/crates/lovelace-corpus.md` path is now external prose; no repository path remains.

## Verification output

### 1. Requested filter — prints nothing

```console
$ cd /mnt/c/Users/ricar/dev/RustLace
$ bash tools/check-docs.sh --symbols 2>&1 | grep -E 'build-and-run|extensions/|crates/'
pipeline exit: 1          # grep: no match; no output above
```

The checker's full summary at final capture (the 20 counted line-suffix citations are all in
`docs/adr/`, which the checker exempts as historical records):

```text
check-docs: 55 Markdown files, 147 relative links, 199 code references, 20 line-suffix citations, 17 symbol citations
check-docs: OK
```

### 2. Executable examples

```text
pages: 4   examples checked: 27   passed: 27   failed: 0   unpaired: 0
OK: 27 examples match the documented output.
```

### 3. `git diff --stat` (owned files)

Raw output (the working tree was already CRLF-converted relative to the index before this task, so
every line of every file shows as changed):

```text
 docs/build-and-run.md                 | 510 ++++++++++++++++++++-------------
 docs/crates/lovelace-array.md         |  98 +++---
 docs/crates/lovelace-bench.md         |  84 ++--
 docs/crates/lovelace-corpus.md        | 115 +++---
 docs/crates/lovelace-desktop-spike.md |  78 ++--
 docs/crates/lovelace-desktop.md       | 155 +++----
 docs/crates/lovelace-int.md           |  88 ++--
 docs/crates/lovelace-json.md          |  80 ++--
 docs/crates/lovelace-know.md          | 106 +++---
 docs/crates/lovelace-lang.md          | 108 +++---
 docs/crates/lovelace-nat.md           |  92 ++--
 docs/crates/lovelace-real.md          | 100 +++--
 docs/crates/lovelace-run.md           | 116 +++---
 docs/crates/lovelace-symbolics.md     |  86 ++--
 docs/extensions/README.md             | 380 ++++++++--------
 docs/extensions/dsp.md                | 650 +++++++++++++++-----------
 docs/extensions/mathir.md             | 566 +++++++++++++---------
 docs/extensions/symbolics.md          | 792 +++++++++++++++---------------
 18 files changed, 2120 insertions(+), 2084 deletions(-)
```

The same stat with CR-at-EOL ignored (my edits plus the pre-existing `dsp.md` movingavg section
that was already in the working tree when I started):

```text
 docs/build-and-run.md             | 44 +++++++++++++--------------
 docs/crates/lovelace-array.md     |  2 +-
 docs/crates/lovelace-bench.md     |  2 +-
 docs/crates/lovelace-corpus.md    |  9 +++---
 docs/crates/lovelace-desktop.md   | 11 +++---
 docs/crates/lovelace-int.md       |  2 +-
 docs/crates/lovelace-json.md      |  2 +-
 docs/crates/lovelace-know.md      |  4 +--
 docs/crates/lovelace-lang.md      |  4 +--
 docs/crates/lovelace-nat.md       |  4 +--
 docs/crates/lovelace-real.md      |  2 +-
 docs/crates/lovelace-run.md       |  4 +--
 docs/crates/lovelace-symbolics.md |  4 +--
 docs/extensions/README.md         | 26 ++++++++--------
 docs/extensions/dsp.md            | 60 +++++++++++++++++++++++++++++---------
 docs/extensions/mathir.md         | 12 ++++----
 docs/extensions/symbolics.md      | 12 ++++----
 17 files changed, 120 insertions(+), 84 deletions(-)
```

### 4. Residual `.rs:NNN` scan (fences included)

```text
$ grep -nE '\.rs:[0-9]' docs/build-and-run.md docs/extensions/*.md docs/crates/*.md
(none)
```

# Round 6 — reader experience, tutorial evidence and provenance

**Branch:** `docs/documentation-quality-remediation`
**HEAD:** `d946316`
**Recorded:** 2026-11-16T13:35Z, WSL Ubuntu, commands run from the repository root
**Objective:** close RQ-017's remnants (EVD-048), RQ-019, RQ-020 and RQ-021 with reproduced evidence.
**Scope guard:** only `README.md`, `docs/README.md`, `docs/editorial.md`,
`docs/adr/004-thirty-digit-default.md`, `docs/crates/lovelace-bench.md`,
`docs/crates/lovelace-lang.md`, `docs/language/README.md` and `docs/tutorials/**` were edited.
`tools/**`, `.github/**`, `.dsh/**`, `AGENTS.md`, `docs/goal-docs-v1/**` and
`docs/goal-docs-v2/**` (except this report) were not touched, nor were
`docs/crates/lovelace-int.md`, `docs/crates/lovelace-json.md` and `docs/crates/lovelace-real.md`.

## 1. RQ-017 — obsolete assertions and the retired term

### 1.1 Tutorial and how-to rows in `docs/editorial.md`

Before (lines 71-72):

```text
| **Tutorial** | "Teach me by doing." | a new `docs/tutorials/` page (target shape; none exist yet) |
| **How-to** | "How do I do this one thing?" | a new `docs/how-to/` page (target shape; none exist yet) |
```

After:

```text
| **Tutorial** | "Teach me by doing." | [docs/tutorials/](tutorials/) — four end-to-end tutorials today; a new page goes there |
| **How-to** | "How do I do this one thing?" | a new `docs/how-to/` page (no how-to pages exist yet) |
```

Evidence:

- `docs/tutorials/` exists and holds `README.md` plus `linear-algebra.md`, `symbolic.md`,
  `signal-processing.md` and `reproducible.md`.
- `docs/how-to/` does not exist, so the how-to row now says that no how-to pages exist yet instead
  of presenting the tutorial note as a target shape.
- The Tutorial row now links the directory; `tools/check-docs.sh` resolves the link and the full
  gate below is green.

### 1.2 The retired term → `precision-limited`

The three surviving uses and their replacements:

```text
docs/crates/lovelace-bench.md:45           `eig_sym` is precision-bounded at that count
docs/crates/lovelace-lang.md:58            ... and precision-bounded when it is computed to the requested places
docs/adr/004-thirty-digit-default.md:100   a precision-bounded error statement in the reference docs ...
```

| File | After | Why |
|---|---|---|
| `docs/crates/lovelace-bench.md` | "`eig_sym` is precision-limited at that count" | Current practice: it describes the precision the benchmark operation runs at. |
| `docs/crates/lovelace-lang.md` | "precision-limited when it is computed to the requested places" | Current practice: the engine's exact / precision-limited value taxonomy. |
| `docs/adr/004-thirty-digit-default.md` | "a precision-limited statement in the reference docs for every DSP builtin that divides by a non-exact constant" | Current-practice guidance, not a quotation: see the decision below. |

**ADR-004 decision — changed, not left as history.** The sentence is item 3 of "What this ADR does
not do", and it prescribes what the reference documentation must say for DSP builtins that divide by
a non-exact constant. It does not quote the retired term; the only historical wording it quotes is
`"exact 30-place decimal"`, which stays because ADR-004 is one of the pages allowed to cite the
prohibited phrases and the item exists to replace that phrase. The item's subject is documentation
that exists today, so its vocabulary is current practice and takes the current term. No part of the
correction's arithmetic, history or resolution was rewritten.

Sweep for the retired term over current documentation (output is empty; the pipeline exits 1
because the second grep selects no line):

```console
$ grep -rn precision-bounded docs --include=*.md | grep -v goal-docs
$ echo $?
1
$ grep -rn precision-bounded docs --include=*.md | grep -v goal-docs | wc -l
0
```

What remains, and why:

- Every raw hit is under a harness working-memory tree: `docs/internal/archive/goal-docs-v1/`
  (15 lines, the archived first goal cycle) or `docs/goal-docs-v2/` (29 lines, seven of them this
  report's quotations). All those paths contain `goal-docs`, so this task's filter removes them and
  `tools/check-docs.sh` treats them as evidence rather than reader-facing navigation. They are
  outside the files this task owns and are kept as historical evidence. No hit survives in current
  documentation.
- `tools/check-docs.sh` still prints the term in two of its own diagnostic strings. `tools/**` is
  out of scope for this task, so it was not edited; recorded here so the gate's vocabulary can be
  aligned in a `tools/**` change.

## 2. RQ-021 — language-page provenance

`docs/language/README.md`, "Scope and conventions", before:

```text
The pages describe the engine tree at commit `2152218` (the `feat/eig-sym` merge on `main`) and
were verified with a debug binary built from that tree.
```

After:

```text
The pages were originally authored against commit `2152218` (the `feat/eig-sym` merge on `main`),
with a debug binary built from that tree; that is where the recorded transcripts come from, not a
claim that current behaviour is still pinned to that commit. To check the pages against the current
branch, build the runner once with `cargo build -p lovelace-run`, then run
`bash docs/language/examples/run-examples.sh` from the repository root: it extracts every `lovelace`
fence, runs it with `lovelace-run --file … --text`, and compares the output with the `text` block
that follows. `docs/language/examples/run-examples.ps1` is the same check for a Windows PowerShell
host.
```

The paragraph now separates three things: the commit the pages were authored against, the claim
that the current branch is checked against them, and the command a reader can run to re-verify
today. The existing "Running the examples" section is unchanged; the new command names the bash
runner CI uses on both platforms, and the PowerShell variant is named beside it.

Re-verification actually run on this tree (not a citation of an old run):

```console
$ bash docs/language/examples/run-examples.sh
README.md: 4 example(s) passed
lexical-structure.md: 19 example(s) passed
values-and-types.md: 23 example(s) passed
expressions-and-operators.md: 44 example(s) passed
statements-and-control-flow.md: 26 example(s) passed
functions-and-scope.md: 22 example(s) passed
builtins.md: 25 example(s) passed
errors-and-diagnostics.md: 5 example(s) passed
precision-and-formatting.md: 18 example(s) passed
plots.md: 7 example(s) passed
catalogue: 124 builtins in 11 categories match lovelace-run --functions --function-limit 500

All 193 examples passed.
```

## 3. RQ-019 — tutorial review

All four pages already carry the seven-section shape (Problem, Source, Output, Reading the result,
Assumptions, Precision, Verification). The review below records where each required element is; the
only edits were one oracle sentence per page in `## Verification`, and one existing oracle sentence
turned into a link. `## Source`, `## Output`, `## Reading the result`, `## Assumptions` and
`## Precision` were not edited, and no `.ls` example was touched.

| Tutorial | Goal | Runnable example | Interpretation | Limitations | Precision | Verification command | Independent check (now linked) |
|---|---|---|---|---|---|---|---|
| linear-algebra | "Solve a linear system and prove the answer" (Problem) | `## Source` = `docs/tutorials/examples/linear-algebra.ls` | `## Reading the result`, seven bullets | `matmul(A, [5, 6])` does not work; no tolerance or refinement; a non-terminating quotient would become precision-limited (Precision, last paragraph) | `## Precision` per-value table | `cargo run -p lovelace-run -- --file docs/tutorials/examples/linear-algebra.ls --text` | integer `det`/`A * sol = b` recomputation in `tools/run-tutorials.sh`, linked from the Verification paragraph |
| symbolic | "Differentiate symbolically, then cross into decimals" | `## Source` = `docs/tutorials/examples/symbolic.ls` | `## Reading the result`, eight bullets | no factoring rule for `x^2 - 1` even with `assume`; `evalf` does closed forms only | `## Precision` per-value table | `cargo run -p lovelace-run -- --file docs/tutorials/examples/symbolic.ls --text` | hand differentiation, `3*2^2 + 2 = 14`, and the standard `sqrt(2)` expansion (new sentence) |
| signal-processing | "Build a signal, smooth it, and read its spectrum" | `## Source` = `docs/tutorials/examples/signal-processing.ls` | `## Reading the result`, five bullets | FFT length must be a power of two and is refused otherwise; `setprecision` is not retroactive | `## Precision` per-value table | `cargo run -p lovelace-run -- --file docs/tutorials/examples/signal-processing.ls --text` | transform definition (`1`, `-1i`, `-1`, `1i`), the ramp's DC bin `36`, and the 30→50-place budget pair (new sentence) |
| reproducible | "Run a script, make a plot, and read the JSON envelope" | `## Source` = `docs/tutorials/examples/reproducible.ls` | `## Reading the result`, three pipeline steps plus field meanings | envelope numbers are strings; `elapsed`/`timings` are host measurements; the temporary path is host-specific | `## Precision` per-value table | the main JSON command plus the two field selectors in `## Verification` | `det(A) = 2*3 - 1*1 = 5` by hand, plus the structural plot-artifact checks in the runner (new sentence) |

Why this is not a proof dump:

- There is no pasted independent-oracle transcript. The only text fence outside `## Output` is the
  length-3 FFT refusal in `signal-processing.md`, and the paragraph after it says where that refusal
  comes from and how to reproduce it.
- Each `## Output` block is the required recorded engine transcript, and each is interpreted in
  `## Reading the result`; the four pages contain five text fences in total (four Output blocks and
  that one quoted refusal).
- The oracle detail is a sentence naming the check plus a link to the script that performs it
  (`tools/run-tutorials.sh`), not a copied transcript. The pages say plainly that the runner checks
  the recording, not the mathematics.

`tools/run-tutorials.sh` re-extracts `## Source` and `## Output` from the current pages, compares
the Source fence byte-for-byte with the checked-in `.ls`, executes it and diffs the output: 4
tutorials, 8 fences, 1 independent oracle — see the transcript in §6. The recorded values are
therefore unchanged by this round.

## 4. RQ-020 — reader-first navigation

Mapping the five items a newcomer needs:

| Need | Where it is now |
|---|---|
| (a) Supported platforms | `README.md` opening paragraph ("Windows and Linux (X11)"), the `**Linux support.**` paragraph, and the release-asset table (`Windows x86_64`, `Linux x86_64`); `docs/README.md` "New to RustLace?" paragraph. |
| (b) A first runnable script | `README.md` "Try a calculation" (before `## Current limitations`) and "Quick start"; `docs/README.md` points to the README and to the tutorials. |
| (c) The numerical model's limits | `README.md` "Current limitations" precision bullet, now linking `docs/language/precision-and-formatting.md` and `docs/extensions/dsp.md`; `docs/README.md` points at both. |
| (d) The tutorials | `README.md` "Get started" link and a new Documentation-table row; `docs/README.md` table row and the new paragraph. |
| (e) API references | `README.md` crate table (one page per crate) and Documentation table; `docs/README.md` "crate pages" and "language reference" links. |

Changes at the top of `README.md`:

- Removed the CI sentence from `**Linux support.**`: "Linux CI builds the GUI, runs
  `--gui-selftest` (20 checks, 0 failed) and the desktop test suite under `xvfb-run`, and uploads
  the binary as the `lovelace-desktop-linux-gui-x86_64` artifact." That is internal CI detail; the
  same observations remain in `docs/testing.md` §6, `docs/release.md` and the release-asset table.
- Replaced the remediation-process phrasing "stated explicitly here rather than discovered later"
  with a pointer to `Current limitations`.
- Removed "CI runs" from the Linux limitation bullet: "on Linux the workbench selftest and the
  desktop suite run headlessly rather than opening a window" states the limitation without the CI
  internals.
- Added the tutorials to "Get started" and to the Documentation table, and added the precision
  page next to the DSP guide in the precision limitation.

Deliberately kept: the CI badge, the release-asset table and checksum commands (installation
material a reader needs), the screenshots and their provenance note, the three-entry-point table,
the Quick start and the Testing section. Nothing was rewritten for style.

`docs/README.md` gained one paragraph:

```text
New to RustLace? The [README](../README.md) names the supported platforms (Windows x86_64 and
Linux x86_64/X11) and gives a first runnable script; the [tutorials](tutorials/) run four complete
scripts end to end; [precision and formatting](language/precision-and-formatting.md) and the
[DSP guide](extensions/dsp.md) state the numerical model's limits; the
[language reference](language/) and the [crate pages](crates/) are the API references.
```

That paragraph, the existing table rows and the README links give every one of (a)-(e) without
opening a migration or remediation record.

## 5. Per-file changelog

| File | Change | Evidence |
|---|---|---|
| `README.md` | Tutorials added to "Get started" and the Documentation table; precision bullet links the language precision page; the two CI sentences at the top removed or de-internalised; the pre-1.0 sentence points at `Current limitations` | `check-docs.sh --readme` OK |
| `docs/README.md` | One "New to RustLace?" paragraph mapping platforms, first script, numerical limits, tutorials and references | `check-docs.sh` OK |
| `docs/editorial.md` | Tutorial row links `docs/tutorials/`; how-to row keeps "no how-to pages exist yet" | `check-docs.sh` OK |
| `docs/adr/004-thirty-digit-default.md` | One word: the retired term → "precision-limited statement" (current-practice guidance; the quoted old phrase stays) | sweep: 0 current-doc hits |
| `docs/crates/lovelace-bench.md` | One word: the retired term → "precision-limited" | sweep: 0 current-doc hits |
| `docs/crates/lovelace-lang.md` | One word: the retired term → "precision-limited" | sweep: 0 current-doc hits |
| `docs/language/README.md` | Scope paragraph rewritten: authored against `2152218` vs checked against this branch; re-verification command named | `run-examples.sh`: 193 examples passed |
| `docs/tutorials/linear-algebra.md` | `## Verification`: oracle sentence now links the integer oracle | `run-tutorials.sh` PASS + ORACLE |
| `docs/tutorials/symbolic.md` | `## Verification`: one sentence naming the independent mathematical check; runner link | `run-tutorials.sh` PASS |
| `docs/tutorials/signal-processing.md` | `## Verification`: one sentence naming the transform definition / budget checks; runner link | `run-tutorials.sh` PASS |
| `docs/tutorials/reproducible.md` | `## Verification`: one sentence naming the hand determinant; runner link | `run-tutorials.sh` PASS |
| `docs/tutorials/README.md` | Not changed — its seven-section, precision and runner claims were already accurate | `run-tutorials.sh` PASS |
| `docs/goal-docs-v2/round-6/reader-experience.md` | This report | — |

No `## Output` fence and no `docs/tutorials/examples/*.ls` file was modified.

## 6. Verification transcripts

```console
$ bash tools/check-docs.sh
check-docs: 71 Markdown files (58 docs/, 3 root, 10 agent instructions), 251 relative links, 49 heading anchors, 442 code references
check-docs: OK
DOCREPORT gate=check-docs status=ok pages=71 links=251 anchors=49 refs=442 citations=0 symbols=0 claims=0 tutorials=0 sections=0 readme=0 modes=none failures=0
exit=0

$ bash tools/check-docs.sh --symbols
check-docs: 71 Markdown files (58 docs/, 3 root, 10 agent instructions), 251 relative links, 49 heading anchors, 442 code references, 21 line-suffix citations, 22 symbol citations
check-docs: OK
DOCREPORT gate=check-docs status=ok pages=71 links=251 anchors=49 refs=442 citations=21 symbols=22 claims=0 tutorials=0 sections=0 readme=0 modes=symbols failures=0
exit=0

$ bash tools/check-docs.sh --claims
check-docs: 71 Markdown files (58 docs/, 3 root, 10 agent instructions), 251 relative links, 49 heading anchors, 442 code references
check-docs: claims audit: 95 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: OK
DOCREPORT gate=check-docs status=ok pages=71 links=251 anchors=49 refs=442 citations=0 symbols=0 claims=0 tutorials=5 sections=5 readme=0 modes=claims failures=0
exit=0

$ bash tools/check-docs.sh --readme
check-docs: 71 Markdown files (58 docs/, 3 root, 10 agent instructions), 251 relative links, 49 heading anchors, 442 code references
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
DOCREPORT gate=check-docs status=ok pages=71 links=251 anchors=49 refs=442 citations=0 symbols=0 claims=0 tutorials=0 sections=0 readme=4 modes=readme failures=0
exit=0

$ bash tools/check-reference-pages.sh
check-reference-pages: 27 scoped page(s): 16 manifested, 11 declared non-reference, every page classified
check-reference-pages: 112 required sections in order and non-empty
check-reference-pages: OK
DOCREPORT gate=check-reference-pages status=ok pages=27 manifested=16 non_reference=11 unclassified=0 sections=112 failed_pages=0 failures=0
exit=0

$ bash tools/run-tutorials.sh
run-tutorials: expecting 4 tutorials: linear-algebra symbolic signal-processing reproducible
run-tutorials: runner: /mnt/c/Users/ricar/dev/RustLace/target/debug/lovelace-run
PASS linear-algebra: docs/tutorials/linear-algebra.md Source -> Output match
ORACLE linear-algebra: det(A) = 5; A * sol = b exactly at scale 10^1; gap = [0, 0]
PASS symbolic: docs/tutorials/symbolic.md Source -> Output match
PASS signal-processing: docs/tutorials/signal-processing.md Source -> Output match
PASS reproducible: docs/tutorials/reproducible.md fields display/kind/exact/plot match; plot artifact exists outside the repository

run-tutorials: 4 tutorials checked (linear-algebra symbolic signal-processing reproducible)
run-tutorials: 1 independent oracle(s)
run-tutorials: OK
DOCREPORT gate=run-tutorials status=ok tutorials=4 fences=8 oracles=1 failures=0
exit=0

$ grep -rn precision-bounded docs --include=*.md | grep -v goal-docs
$ echo $?
1
$ grep -rn precision-bounded docs --include=*.md | grep -v goal-docs | wc -l
0
```

The `95 pages under docs/` in the claims audit counts harness working memory as well as current
pages; the link/symbol scan above it reports the 71 current Markdown files it actually checks.

Local evidence only: hosted GitHub Actions runs (RQ-013) are not claimed by this report.

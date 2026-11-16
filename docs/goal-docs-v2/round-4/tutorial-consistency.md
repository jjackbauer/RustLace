# Round 4 — tutorial consistency (RQ-003.4, RQ-008.4, `.ls` convention)

- **Date:** 2026-11-16 (America/Sao_Paulo)
- **Branch head at the run:** `d9463162fd27cf8a6c485ef14f3867ee6219262a` (`d946316`, working tree
  dirty by design; this report is an uncommitted deliverable)
- **Host:** WSL2 Ubuntu 24.04.4 LTS x86_64; cargo 1.100.0; `CARGO_TARGET_DIR=/tmp/rl-target`;
  repository at `/mnt/c/Users/ricar/dev/RustLace`.
- **Owner files changed:** `docs/tutorials/**` (the four pages, `README.md` and the four examples),
  `tools/run-tutorials.sh`, `docs/reference-template.md`, `docs/contributing.md`, and this report.
- **Not touched:** `.github/**`, `tools/check-crate-examples.sh`, both `docs/*/examples/run-examples.sh`
  files, `docs/extensions/**`, `docs/crates/**`, `docs/language/**`, `docs/editorial.md`, `README.md`,
  and `docs/goal-docs-v2/**` other than this report. No `lovelace-*/src` or test file was edited.

## 1. Task 1 — the `.lv` → `.ls` convention regression

### 1.1 The rename

At HEAD the four example files were untracked: `git ls-files docs/tutorials` listed only the five
Markdown pages, and `git status --short` showed `?? docs/tutorials/examples/`. The round-3 example
files therefore had no blob in HEAD. I staged each source file and used `git mv` for every pair:

```console
$ git add docs/tutorials/examples/linear-algebra.lv docs/tutorials/examples/symbolic.lv \
    docs/tutorials/examples/signal-processing.lv docs/tutorials/examples/reproducible.lv
$ git mv docs/tutorials/examples/linear-algebra.lv docs/tutorials/examples/linear-algebra.ls
$ git mv docs/tutorials/examples/symbolic.lv docs/tutorials/examples/symbolic.ls
$ git mv docs/tutorials/examples/signal-processing.lv docs/tutorials/examples/signal-processing.ls
$ git mv docs/tutorials/examples/reproducible.lv docs/tutorials/examples/reproducible.ls
$ git status --short docs/tutorials/examples
A  docs/tutorials/examples/linear-algebra.ls
A  docs/tutorials/examples/reproducible.ls
A  docs/tutorials/examples/signal-processing.ls
A  docs/tutorials/examples/symbolic.ls
```

Git reports `A`, not `R`, because the `.lv` paths never existed in a commit; there is no pre-rename
blob for `git log --follow` to traverse. `git mv` was used, and the old paths are gone from the index;
the absence of recorded rename history is a property of the round-3 files being uncommitted, and is
recorded again in §6.1 rather than claimed as satisfied.

The examples on disk are now:

```console
$ ls docs/tutorials/examples
linear-algebra.ls
reproducible.ls
signal-processing.ls
symbolic.ls
```

### 1.2 Every reference was updated

The four tutorial pages referenced `.lv` 14 times (3 in `linear-algebra.md`, 5 in
`reproducible.md`, 3 in `signal-processing.md`, 3 in `symbolic.md`). All 14 now name `.ls`, including
the Source links, the Assumptions commands and the Verification commands. `docs/tutorials/README.md`
had no `.lv` path to change; its selector paragraph was updated for Task 4. `tools/run-tutorials.sh`
had four `.lv` occurrences (the Verification-path assertion and one example path in each of the text
and reproducibility runners), all now `.ls`.

```console
$ grep -rn '\.lv\b' docs/tutorials tools/run-tutorials.sh
$ echo "exit=$?"
exit=1
```

The acceptance grep is empty. `bash -n tools/run-tutorials.sh` exits 0.

### 1.3 The runner still passes

```console
$ CARGO_TARGET_DIR=/tmp/rl-target bash tools/run-tutorials.sh
run-tutorials: expecting 4 tutorials: linear-algebra symbolic signal-processing reproducible
run-tutorials: runner: /tmp/rl-target/debug/lovelace-run
PASS linear-algebra: docs/tutorials/linear-algebra.md Source -> Output match
ORACLE linear-algebra: det(A) = 5; A * sol = b exactly at scale 10^1; gap = [0, 0]
PASS symbolic: docs/tutorials/symbolic.md Source -> Output match
PASS signal-processing: docs/tutorials/signal-processing.md Source -> Output match
PASS reproducible: docs/tutorials/reproducible.md fields display/kind/exact/plot match; plot artifact exists outside the repository

run-tutorials: 4 tutorials checked (linear-algebra symbolic signal-processing reproducible)
run-tutorials: 1 independent oracle(s)
run-tutorials: OK
[exit=0]
```

The runner's comparisons were not weakened: `## Source` fences, recorded `## Output` blocks, the
independent integer oracle, the JSON field comparison and the plot-artifact checks are unchanged. No
recorded `## Output` block was edited.

## 2. Task 2 — the four-category vocabulary in the tutorials

### 2.1 What every page now says

Each of the four `## Precision` sections now opens with the same claim vocabulary:

> The four claim kinds are **exact**, **precision-limited**, **error-bounded** and
> **parity-verified**, following [the editorial guide](../editorial.md#the-four-kinds-of-claim).

The tables keep their existing value/justification columns. The closing paragraphs now say which
categories occur and which do not, and why. No page uses "precision-bounded" or "parity" as the old
three-way label any more.

| Page | Exact values | Precision-limited values | Error-bounded | Parity-verified |
|---|---|---|---|---|
| `linear-algebra.md` | all tabulated inputs, `Inv`, `sol`, `residual`, `gap`, `d` | none: every division terminates and no transcendental is evaluated | none claimed; `gap` is an exact zero vector, not an error bound | none: engine rational arithmetic, not a reference reproduction |
| `symbolic.md` | coefficients, `f`, `df`, `cancelled`, `fired`, `at_2 = 14`, the integer parameters `20`, `19`, `21` | `approx = 1.4142135623730950488` and the 21-place comparison: `sqrt(2)` at a stated working budget; the budget is not a final-error bound | none claimed: no final-error bound is published for `evalf(sqrt(2), 20)` | none: the engine's own evaluation, not a named reference implementation |
| `signal-processing.md` | integer samples, `0.5` halves, Gaussian-integer `spectrum`, `padded_spectrum`, the transform lengths and budgets, and the integer real components `36` / `-4` / `4` | the 30-place and 50-place irrational imaginary tails of the two 8-point spectra | none claimed: neither transform has a published final-error bound | none: `movingavg` here is the corrected engine arithmetic; the historical fixed-width rendering is a defect in ADR-004, not a reproduced result |
| `reproducible.md` | `A`, `b`, plot coordinates, `det(A) = 5`, numerator 5 and denominator 1 | none: the script's one division (`inv(A)`) terminates, and no transcendental is evaluated | none claimed: the determinant and coordinates are exact integers | none: the JSON envelope reports the engine's own result |

### 2.2 Per-page rationale

- **Linear algebra.** The old table already classified the values correctly; the change is the
  vocabulary and the explicit boundary. The closing paragraph now says exactness is a property *of
  this arithmetic*: a recurring quotient that fits the budget is stored exactly, one that does not
  fit is truncated and becomes **precision-limited**. No such quotient occurs in this system.
- **Symbolic.** `approx` was previously called "precision-bounded". It is now **precision-limited**,
  with the explicit warning that the 20-place budget is not a final-error bound, and the 21-place
  comparison is called precision-limited for the same reason. `at_2 = 14` stays **exact**, because
  substitution of the integer 2 into `3*x^2 + 2` is exact and the output is the integer 14.
- **Signal processing.** The integer-valued components remain **exact**; the irrational imaginary
  tails are **precision-limited**, and the page now says a complex value containing one of those
  tails is precision-limited as a whole even when its real component is exact. The old "parity"
  sentence is replaced by an explicit statement that `movingavg` here is the corrected arithmetic and
  that ADR-004 records the historical fixed-width rendering as a defect.
- **Reproducible.** Every extracted value is **exact**. The page now says `result.structured.exact`
  is a provenance flag, not by itself a correctness certificate, and that `elapsed` / `timings` are
  host measurements rather than mathematical results, so they are not classified or compared.

### 2.3 Vocabulary count

```console
$ grep -c 'precision-limited' docs/tutorials/*.md
docs/tutorials/README.md:2
docs/tutorials/linear-algebra.md:3
docs/tutorials/reproducible.md:2
docs/tutorials/signal-processing.md:5
docs/tutorials/symbolic.md:4
```

`docs/tutorials/README.md`'s own `## Precision` paragraph was updated to the same four categories; it
now states that no page publishes a final-error bound and no page reproduces a reference
implementation.

## 3. Task 3 — reference template and contributor guide

`docs/reference-template.md` kept its structure and changed only the claim-strength wording:

- Precision-behaviour row: "whether it is exact, precision-bounded or parity" → "whether it is
  **exact, precision-limited, error-bounded or parity-verified**".
- Limitations row: "parity artifacts" → "**parity-verified artifacts**".
- Rule 5: "Every numeric statement says whether it is exact, precision-bounded or parity, per
  [the editorial guide](editorial.md#the-three-kinds-of-claim)" → the four categories, per
  `editorial.md#the-four-kinds-of-claim`.
- Skeleton: `<exact / precision-bounded / parity, the budget and its cap>` → `<exact /
  precision-limited / error-bounded / parity-verified, the budget and its cap>`.

`docs/contributing.md` changed one sentence in "Documentation requirements". The stored line now
reads (single line, wrapped here for the report):

```text
every numerical claim says whether it is exact, precision-limited, error-bounded or
parity-verified, and every example uses a repository-relative path or a placeholder instead of a
path from one machine.
```

It previously read `exact, precision-bounded or parity` in that sentence; the rest of the paragraph
is unchanged. The file arrived in the working tree with CRLF endings; it was normalised to the stored
LF form, so `git diff` shows one changed line (one insertion, one deletion) rather than a whole-file
line-ending rewrite.

## 4. Task 4 — the reproducibility page's Windows route

### 4.1 The documented routes

Windows is now the first documented route, and it is plain Windows PowerShell 5.1 (shipped with
Windows 10/11) or PowerShell 7. `ConvertFrom-Json`, `ForEach-Object`, `[pscustomobject]` and
`ConvertTo-Json` are built in; no Python, bash, Unix path or here-document is required. The pipeline
is deliberately one line because Windows PowerShell 5.1 does not continue a pipeline after a trailing
`|`:

```powershell
cargo run -p lovelace-run -- --file docs/tutorials/examples/reproducible.ls --omit-functions --omit-variables | ConvertFrom-Json | ForEach-Object { [pscustomobject]@{ display = $_.result.display; kind = $_.result.kind; exact = $_.result.structured.exact; plot = $_.artifacts[0].path } } | ConvertTo-Json -Compress
```

The Linux/WSL route is explicitly the optional second-shell route and states its real prerequisite:
`python3` on `PATH`. The page's Assumptions section also states that the main command needs only Rust
1.85+ and cargo, and that `bash tools/run-tutorials.sh` needs bash, awk and grep but no Python.

### 4.2 Windows PowerShell 5.1 execution

The host has Windows PowerShell 5.1.26100.4202 but no Windows Rust toolchain (`Get-Command cargo`
returns nothing), so the engine stage was a `cargo.cmd` shim that emitted a real envelope captured by
the Linux runner. This runs the **exact** one-line selector and the exact JSON shape from the page:

```console
PS> $PSVersionTable.PSVersion.ToString()
5.1.26100.4202
PS> $env:PATH = "C:\Users\ricar\dev\RustLace\.tmp\rq4\bin;" + $env:PATH
PS> cargo run -p lovelace-run -- --file docs/tutorials/examples/reproducible.ls --omit-functions --omit-variables | ConvertFrom-Json | ForEach-Object { [pscustomobject]@{ display = $_.result.display; kind = $_.result.kind; exact = $_.result.structured.exact; plot = $_.artifacts[0].path } } | ConvertTo-Json -Compress
{"display":"5","kind":"Real","exact":true,"plot":"/tmp/lovelace-plots/plot.svg"}
```

The `plot` value is the Linux temp path because the envelope was captured on Linux; a Windows engine
run would produce a `C:\...` path, which the page describes (and whose backslashes JSON escapes). The
selector itself ran under 5.1 and produced the recorded four-field object.

The optional Linux route was also run against the real Linux engine:

```console
$ python3 --version
Python 3.12.3
$ /tmp/rl-target/debug/lovelace-run --file docs/tutorials/examples/reproducible.ls \
    --omit-functions --omit-variables | python3 -c 'import json, sys; e = json.load(sys.stdin); r = e["result"]; print(json.dumps({"display": r["display"], "kind": r["kind"], "exact": r["structured"]["exact"], "plot": e["artifacts"][0]["path"]}, separators=(",", ":")))'
{"display":"5","kind":"Real","exact":true,"plot":"/tmp/lovelace-plots/plot.svg"}
```

The page is executable on both supported platforms by a new reader: Windows needs only the shell and
cargo; Linux/WSL needs the shell, cargo and Python 3 for the optional field selector. The static
runner checks that guard the page are unchanged: no `/tmp`, no `<<`, `ConvertFrom-Json` present, and
`python3` present; the page states the Python prerequisite next to that route.

## 5. Verification transcripts

### 5.1 Tutorial runner

The transcript is in §1.3: exit 0, four named tutorials, one independent oracle, `run-tutorials: OK`.

### 5.2 `.lv` grep

```console
$ grep -rn '\.lv\b' docs/tutorials tools/run-tutorials.sh
$ echo "exit=$?"
exit=1
```

### 5.3 Precision vocabulary

```console
$ grep -c 'precision-limited' docs/tutorials/*.md
docs/tutorials/README.md:2
docs/tutorials/linear-algebra.md:3
docs/tutorials/reproducible.md:2
docs/tutorials/signal-processing.md:5
docs/tutorials/symbolic.md:4
```

### 5.4 Documentation gates

```console
$ bash tools/check-docs.sh --claims
check-docs: 57 Markdown files, 171 relative links, 241 code references
check-docs: claims audit: 90 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: OK
[exit=0]

$ bash tools/check-docs.sh --readme
check-docs: 57 Markdown files, 171 relative links, 241 code references
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
[exit=0]

$ bash tools/check-docs.sh --symbols
check-docs: 57 Markdown files, 171 relative links, 241 code references, 20 line-suffix citations, 20 symbol citations
check-docs: OK
[exit=0]

$ bash tools/check-reference-pages.sh
check-reference-pages: 16 page(s), 112 required sections in order
check-reference-pages: OK
[exit=0]
```

`git diff --check` over the changed documentation exits 0, and `bash -n tools/run-tutorials.sh` exits
0.

## 6. What was not verified

### 6.1 Rename history

`git mv` was used, but the four source files had never been committed, so Git has no pre-rename blob
and reports the `.ls` files as additions. A `git log --follow` or `R` status cannot be produced
without first committing the old paths, which this task did not do. The acceptance condition (no
`.lv` path and a passing runner) is met; the history-follows intent is not observable in this working
tree.

### 6.2 A real Windows engine run

No Windows Rust toolchain exists on this host; `Get-Command cargo` is empty. The PowerShell route was
executed end-to-end with a cargo stand-in that emitted a real Linux-captured envelope. The selector,
JSON parsing, field selection and compressed output are verified under Windows PowerShell 5.1; the
Windows engine value itself is CI's evidence, not this report's.

### 6.3 Hosted CI

No hosted run was made; `.github/**` is out of scope for this task. The local gates and the tutorial
runner pass, but RQ-013 remains the parent round's evidence.

### 6.4 Error-bounded and parity-verified examples

No tutorial value has a demonstrated final-error bound or reproduces a named reference
implementation's behaviour, so no page claims those categories. That is the correct outcome, not a
missing classification: the sections define all four kinds and say explicitly that the other two do
not occur.

### 6.5 macOS

The bash selector and the tutorials were not run on macOS. The repository's supported product
platforms are Windows and Linux/WSL, which are the two routes the page documents.

## 7. Per-file changelog

| File | Change |
|---|---|
| `docs/tutorials/examples/linear-algebra.lv` → `.ls` | `git add` + `git mv`; content unchanged; old path removed from the index. Git shows `A` because the source was untracked at HEAD (§6.1). |
| `docs/tutorials/examples/symbolic.lv` → `.ls` | Same rename; content unchanged. |
| `docs/tutorials/examples/signal-processing.lv` → `.ls` | Same rename; content unchanged. |
| `docs/tutorials/examples/reproducible.lv` → `.ls` | Same rename; content unchanged. |
| `docs/tutorials/linear-algebra.md` | Three `.lv` → `.ls` references (Source link, Assumptions, Verification). `## Precision` rewritten around the four categories: every value exact; no precision-limited, error-bounded or parity-verified value; recurring-quotient boundary explained. No `## Output` change. |
| `docs/tutorials/symbolic.md` | Three `.lv` → `.ls` references. `## Precision` rewritten: `approx` and the 21-place comparison are precision-limited (`sqrt(2)` at a 20-place budget, not a final-error bound); `at_2`, coefficients, parameters and the digit count are exact; no error-bounded or parity-verified value. No `## Output` change. |
| `docs/tutorials/signal-processing.md` | Three `.lv` → `.ls` references. `## Precision` rewritten: integer components, halves, Gaussian integers, lengths and budgets are exact; the 30- and 50-place irrational tails are precision-limited; no error-bounded or parity-verified value; historical `movingavg` explicitly not parity. No `## Output` change. |
| `docs/tutorials/reproducible.md` | Five `.lv` → `.ls` references. `## Precision` rewritten: extracted values are exact; no precision-limited, error-bounded or parity-verified value; `exact` flag described as provenance, not a certificate; timing fields excluded as measurements. `## Reading the result` step 3 now names the PowerShell route first and the optional bash route second. Output note explains the Windows `C:\...` JSON escaping. Assumptions now state Rust/cargo for the main command, Windows PowerShell 5.1/7 with no Python or bash, and `python3` as the explicit prerequisite for the optional Linux/WSL selector. Verification now documents the one-line PowerShell 5.1 route first, then the optional bash + Python 3 route, with no `/tmp`, no `<<` and no required Unix path on Windows. No `## Output` change. |
| `docs/tutorials/README.md` | Prerequisites now name the Windows PowerShell route as the no-Python path and Python 3 only for the optional Linux/WSL selector. Command paragraph says the tutorials share commands but the reproducible page has one platform-specific field selector. `## Precision` updated from the three-way labels to exact / precision-limited / error-bounded / parity-verified, with the no-error-bound and no-parity statements. |
| `tools/run-tutorials.sh` | Four `.lv` → `.ls` occurrences: the `## Verification` path assertion and the `example=` assignment in both the text and reproducibility runners. No comparison, oracle, normalisation or static check was weakened or removed. |
| `docs/reference-template.md` | Precision-behaviour row, Limitations row, rule 5 and the skeleton now use exact / precision-limited / error-bounded / parity-verified, with the link to `editorial.md#the-four-kinds-of-claim`. Structure unchanged. |
| `docs/contributing.md` | One sentence in the Documentation requirements now uses the four categories; LF normalisation makes the diff one changed line. |
| `docs/goal-docs-v2/round-4/tutorial-consistency.md` (this file) | The report. |

## 8. Reproduction index

```console
# repository state
git rev-parse HEAD
git status --short docs/tutorials/examples

# the convention and the contract
grep -rn '\.lv\b' docs/tutorials tools/run-tutorials.sh
bash -n tools/run-tutorials.sh
CARGO_TARGET_DIR=/tmp/rl-target bash tools/run-tutorials.sh

# the vocabulary
grep -c 'precision-limited' docs/tutorials/*.md

# the documentation gates
bash tools/check-docs.sh --claims
bash tools/check-docs.sh --readme
bash tools/check-docs.sh --symbols
bash tools/check-reference-pages.sh

# the page's own Windows route (PowerShell 5.1; cargo stand-in for the missing Windows toolchain)
cargo run -p lovelace-run -- --file docs/tutorials/examples/reproducible.ls --omit-functions --omit-variables | ConvertFrom-Json | ForEach-Object { [pscustomobject]@{ display = $_.result.display; kind = $_.result.kind; exact = $_.result.structured.exact; plot = $_.artifacts[0].path } } | ConvertTo-Json -Compress

# the page's optional Linux route
cargo run -p lovelace-run -- --file docs/tutorials/examples/reproducible.ls --omit-functions --omit-variables | python3 -c 'import json, sys; e = json.load(sys.stdin); r = e["result"]; print(json.dumps({"display": r["display"], "kind": r["kind"], "exact": r["structured"]["exact"], "plot": e["artifacts"][0]["path"]}, separators=(",", ":")))'
```

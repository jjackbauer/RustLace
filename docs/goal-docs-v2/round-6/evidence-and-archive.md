# Round 6 — machine-readable evidence and the archive boundary

- Branch: `docs/documentation-quality-remediation`
- HEAD at measurement: `d946316` (`style: apply cargo fmt to the movingavg and platform-claims changes`)
- Scope: RQ-023 (machine-readable documentation evidence) and RQ-024 (separate harness
  journals from reader-facing documentation). Host: WSL Ubuntu, `CARGO_TARGET_DIR=/tmp/rl-target`,
  `/home/ricardo/.cargo/bin` on `PATH`.

## 1. What changed

| File | Change |
|---|---|
| `tools/check-docs.sh` | final `DOCREPORT` line; `docs/internal/archive/*` added to the excluded class; counters for citations, symbols, claims, tutorials, precision sections and README requirements |
| `tools/check-reference-pages.sh` | final `DOCREPORT` line with scoped/manifested/non-reference/unclassified/section counts |
| `tools/run-tutorials.sh` | final `DOCREPORT` line with tutorials, verified fences and oracles |
| `tools/check-crate-examples.sh` | final `DOCREPORT` line with pages, fences, compiled, excluded and failures |
| `tools/docs-report.sh` | new aggregator: runs the four gates (check-docs once with all three audit flags), prints each summary, then the total; a gate without a summary is a failure |
| `tools/test-docs-gates.sh` | mutation 27 (gate exits 0 with no summary) and a CI-wiring literal for the report step; now 27/27 |
| `.github/workflows/ci.yml` | docs job gains an unconditional report step and a per-OS transcript artifact; every existing step kept |
| `docs/internal/README.md` | new; what the archive is, that it is not documentation, that it must not be linked from the public index, and why v2 is still outside it |
| `docs/goal-docs-v1/` → `docs/internal/archive/goal-docs-v1/` | `git mv`; 16 files, 287,349 bytes, contents untouched |

No gate was weakened, no exit status changed, and no human output line was removed. The summary is
appended after the existing human lines, so the last line on stdout is the machine-readable one. The
crate-example gate also summarizes its two mid-run failure paths (annotation audit and compile
failure) as `status=fail` with counts, so a build break is still described rather than silent; the
exit statuses on those paths are unchanged.

## 2. The report format

Every gate ends with one line:

```text
DOCREPORT gate=<name> status=<ok|fail> <key>=<n> ...
```

| Gate | Keys |
|---|---|
| `check-crate-examples` | `pages`, `fences`, `compiled`, `excluded`, `failures` |
| `run-tutorials` | `tutorials`, `fences`, `oracles`, `failures` |
| `check-docs` | `pages`, `links`, `anchors`, `refs`, `citations`, `symbols`, `claims`, `tutorials`, `sections`, `readme`, `modes`, `failures` |
| `check-reference-pages` | `pages`, `manifested`, `non_reference`, `unclassified`, `sections`, `failed_pages`, `failures` |
| `docs-report` total | `gates`, `ok`, `failed`, `commit`, `os` |

`docs-report.sh` forwards each gate's human output, then prints the gate's own summary line exactly
once, then `docs-report: gate=<name> exit=<rc> summaries=<n>` as the exit-code record. The last line
is always:

```text
DOCREPORT total gates=<n> ok=<n> failed=<n> commit=<sha> os=<uname>
```

Failure rules in the aggregator: no `DOCREPORT` line is a failure even when the gate exited 0 (a
synthesized `DOCREPORT gate=<name> status=fail summary=missing exit=<rc>` line is printed); more than
one summary line is a failure; a summary naming a different gate is a failure; `status=ok` with a
non-zero exit is a failure. The total therefore cannot report `ok` for a silent gate.

## 3. Green report transcript

Command:

```console
export PATH=/home/ricardo/.cargo/bin:/usr/local/bin:/usr/bin:/bin
export CARGO_TARGET_DIR=/tmp/rl-target
bash tools/docs-report.sh
```

Exit status `0`.

```text
===== gate 1: bash tools/check-crate-examples.sh =====
check-crate-examples: cargo: /home/ricardo/.cargo/bin/cargo (from PATH)
PASS docs/crates/lovelace-array.md:83
PASS docs/crates/lovelace-int.md:71
PASS docs/crates/lovelace-json.md:69
PASS docs/crates/lovelace-lang.md:95
PASS docs/crates/lovelace-nat.md:87
PASS docs/crates/lovelace-real.md:100
PASS docs/crates/lovelace-symbolics.md:84
EXCLUDED (none)
check-crate-examples: 13 page(s), 7 rust fence(s): 7 compiled and ran, 0 excluded
check-crate-examples: OK
docs-report: gate=check-crate-examples exit=0 summaries=1
DOCREPORT gate=check-crate-examples status=ok pages=13 fences=7 compiled=7 excluded=0 failures=0
===== gate 2: bash tools/run-tutorials.sh =====
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
docs-report: gate=run-tutorials exit=0 summaries=1
DOCREPORT gate=run-tutorials status=ok tutorials=4 fences=8 oracles=1 failures=0
===== gate 3: bash tools/check-docs.sh --symbols --claims --readme =====
check-docs: 71 Markdown files (58 docs/, 3 root, 10 agent instructions), 251 relative links, 49 heading anchors, 442 code references, 21 line-suffix citations, 22 symbol citations
check-docs: claims audit: 93 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
docs-report: gate=check-docs exit=0 summaries=1
DOCREPORT gate=check-docs status=ok pages=71 links=251 anchors=49 refs=442 citations=21 symbols=22 claims=0 tutorials=5 sections=5 readme=4 modes=symbols,claims,readme failures=0
===== gate 4: bash tools/check-reference-pages.sh =====
check-reference-pages: 27 scoped page(s): 16 manifested, 11 declared non-reference, every page classified
check-reference-pages: 112 required sections in order and non-empty
check-reference-pages: OK
docs-report: gate=check-reference-pages exit=0 summaries=1
DOCREPORT gate=check-reference-pages status=ok pages=27 manifested=16 non_reference=11 unclassified=0 sections=112 failed_pages=0 failures=0
DOCREPORT total gates=4 ok=4 failed=0 commit=d946316 os=Linux
```

The 71 docs pages are the 70 previously scanned pages plus the new `docs/internal/README.md`; the
16 archived pages are excluded, which is why the number is not 87.

## 4. Missing summary is a failure

`tools/test-docs-gates.sh` gained mutation 27: a scratch copy of the real `docs-report.sh` runs
against stub gates that each print a valid summary; one stub is replaced with a gate that exits 0 and
prints only a human line. The report must exit non-zero and the total must record the gate.

```text
CTRL docs-report: complete summaries: clean fixture passed
PASS docs-report: gate exits 0 with no DOCREPORT line (exit 1)

test-docs-gates: 27 mutation(s), 27 passed, 0 failed
test-docs-gates: OK (27 mutations, every gate failed for its intended reason)
```

The fixture also asserts `DOCREPORT total gates=4 ok=4 failed=0` on the clean run and
`DOCREPORT total gates=4 ok=3 failed=1` after the mutation.

## 5. Break and restore proof

One gate was broken deliberately: a probe link was appended to `docs/internal/README.md`, the one
current docs page this round owns. The link does not resolve, so `check-docs` fails; the file was then
restored byte-for-byte and the report was run again.

```text
=== BROKEN RUN ===
broken_rc=1
14:DOCREPORT gate=check-crate-examples status=ok pages=13 fences=7 compiled=7 excluded=0 failures=0
28:DOCREPORT gate=run-tutorials status=ok tutorials=4 fences=8 oracles=1 failures=0
30:docs/internal/README.md:43: broken link 'no-such-internal-page.md' (no such file or directory: docs/internal/no-such-internal-page.md)
36:DOCREPORT gate=check-docs status=fail pages=71 links=252 anchors=49 refs=442 citations=21 symbols=22 claims=0 tutorials=5 sections=5 readme=4 modes=symbols,claims,readme failures=1
42:DOCREPORT gate=check-reference-pages status=ok pages=27 manifested=16 non_reference=11 unclassified=0 sections=112 failed_pages=0 failures=0
44:DOCREPORT total gates=4 ok=3 failed=1 commit=d946316 os=Linux
=== RESTORED: diff against the saved copy (no output means identical) ===
page-identical
=== GREEN RUN AFTER RESTORE ===
restored_rc=0
14:DOCREPORT gate=check-crate-examples status=ok pages=13 fences=7 compiled=7 excluded=0 failures=0
28:DOCREPORT gate=run-tutorials status=ok tutorials=4 fences=8 oracles=1 failures=0
35:DOCREPORT gate=check-docs status=ok pages=71 links=251 anchors=49 refs=442 citations=21 symbols=22 claims=0 tutorials=5 sections=5 readme=4 modes=symbols,claims,readme failures=0
41:DOCREPORT gate=check-reference-pages status=ok pages=27 manifested=16 non_reference=11 unclassified=0 sections=112 failed_pages=0 failures=0
42:DOCREPORT total gates=4 ok=4 failed=0 commit=d946316 os=Linux
```

The broken run exits 1 with `failed=1`; the restored run exits 0 with `failed=0`, and the restored
file is byte-identical to the saved copy.

## 6. The CI step

Added to the `docs` job in `.github/workflows/ci.yml`, after the existing mutation step. Every
existing step is kept, both matrix legs run it, and there is no condition, no failure suppression
and no `continue-on-error`:

```yaml
      # One transcript of every gate above, with a machine-readable DOCREPORT
      # line per gate and a total carrying the commit and OS (RQ-023). It is
      # additive: every gate still runs as its own step, so a failure stays
      # attributable, and this step fails when any gate fails or a gate does
      # not print its summary. Both matrix legs run it unconditionally and
      # upload their own transcript below.
      - name: Report documentation evidence
        shell: bash
        run: |
          set -uo pipefail
          bash tools/docs-report.sh | tee "docs-report-${{ matrix.os }}.log"

      # The per-OS transcript is the evidence artifact for this leg. Like every
      # step in this job it is unconditional: no condition, no failure
      # suppression, so a red report fails the job instead of handing over a
      # green artifact.
      - name: Upload the documentation evidence transcript
        uses: actions/upload-artifact@v4
        with:
          name: docs-report-${{ matrix.os }}
          path: docs-report-${{ matrix.os }}.log
```

`set -o pipefail` is what keeps `tee` from masking the report's exit status. The artifact names are
`docs-report-ubuntu-latest` and `docs-report-windows-latest`.

The mutation self-test's CI-wiring guard now also requires `bash tools/docs-report.sh` in the docs
job, so deleting the step fails the self-test:

```text
WIRE ci.yml: the docs job runs the crate-example and tutorial gates, both example runners, the three check-docs modes, the reference-page gate, the evidence report and this mutation self-test on the Ubuntu+Windows matrix; no conditional skip
```

## 7. The archive move

`docs/goal-docs-v1/` is now `docs/internal/archive/goal-docs-v1/`. The move used `git mv`, so git
records 16 renames and the history follows each file. The size is unchanged at 287,349 bytes, no
archived file was edited, and the old path no longer exists.

```text
$ git status --short docs/goal-docs-v1 docs/internal
A  docs/internal/README.md
R  docs/goal-docs-v1/deliverables.md -> docs/internal/archive/goal-docs-v1/deliverables.md
R  docs/goal-docs-v1/evidence.md -> docs/internal/archive/goal-docs-v1/evidence.md
R  docs/goal-docs-v1/goal.md -> docs/internal/archive/goal-docs-v1/goal.md
R  docs/goal-docs-v1/intent-audit.md -> docs/internal/archive/goal-docs-v1/intent-audit.md
R  docs/goal-docs-v1/journal.md -> docs/internal/archive/goal-docs-v1/journal.md
R  docs/goal-docs-v1/round-1/implementation.md -> docs/internal/archive/goal-docs-v1/round-1/implementation.md
R  docs/goal-docs-v1/round-2/implementation.md -> docs/internal/archive/goal-docs-v1/round-2/implementation.md
R  docs/goal-docs-v1/round-3/implementation.md -> docs/internal/archive/goal-docs-v1/round-3/implementation.md
R  docs/goal-docs-v1/round-4/implementation.md -> docs/internal/archive/goal-docs-v1/round-4/implementation.md
R  docs/goal-docs-v1/round-5/implementation.md -> docs/internal/archive/goal-docs-v1/round-5/implementation.md
R  docs/goal-docs-v1/round-6/implementation.md -> docs/internal/archive/goal-docs-v1/round-6/implementation.md
R  docs/goal-docs-v1/round-7/prose-repair.md -> docs/internal/archive/goal-docs-v1/round-7/prose-repair.md
R  docs/goal-docs-v1/round-7/reference-repair.md -> docs/internal/archive/goal-docs-v1/round-7/reference-repair.md
R  docs/goal-docs-v1/round-8/reference-pages.md -> docs/internal/archive/goal-docs-v1/round-8/reference-pages.md
R  docs/goal-docs-v1/round-9/claim-readme-checks.md -> docs/internal/archive/goal-docs-v1/round-9/claim-readme-checks.md
R  docs/goal-docs-v1/state.md -> docs/internal/archive/goal-docs-v1/state.md

$ test -d docs/goal-docs-v1 && echo present || echo absent
absent

$ git ls-files docs/goal-docs-v1 | wc -l
0

$ find docs/internal/archive/goal-docs-v1 -type f | wc -l
16

$ du -sb docs/internal/archive/goal-docs-v1
287349	docs/internal/archive/goal-docs-v1
```

The gate exclusion was updated for the new location. `tools/check-docs.sh` now excludes
`docs/internal/archive/*` in addition to `docs/goal-desktop-v4/*` and `docs/goal-docs-*`; only the
archive inside `docs/internal/` is excluded, so the new `docs/internal/README.md` is scanned and
passes. `docs/goal-docs-v2/` was deliberately not moved, because that goal is still running; the new
`docs/internal/README.md` says so, explains what the archive holds, that it is not documentation,
that it must not be linked from the public index, and why two goal trees existed. The public index
`docs/README.md` contains no reference to either goal tree.

## 8. Notes and residuals

- `docs/reference-pages.md:77` still describes `docs/goal-docs-v1/` as archived in prose. That page is
  outside this implementer's files, and the sentence is not a link, so no gate fails; it should be
  retargeted to the new path by whoever owns the reference-page manifest.
- The report step and its upload are two steps, because a single `run:` step cannot use
  `actions/upload-artifact`. Neither carries an authored condition, as required. GitHub skips later
  steps when an earlier step fails, so a failing report's transcript stays in the job log rather than
  in the artifact; adding `if: always()` would both violate the no-condition requirement and fail the
  mutation self-test's CI-wiring guard. No gate is optional: the report step itself fails the job.
- `tools/check-docs.sh` was run with all three audit flags together in the report; the docs CI job
  still runs the three modes as separate steps, so a red job still names the failing mode.

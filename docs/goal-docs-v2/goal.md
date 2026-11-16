# Goal — RustLace documentation quality remediation (follow-up)

**Slug:** `goal-docs-v2`
**Opened:** 2026-11-16
**Branch:** `docs/documentation-quality-remediation`
**Reviewed revision:** `fa87ecc6`
**Toolchain:** WSL Ubuntu, `/home/ricardo/.cargo/bin/cargo`; repo at `/mnt/c/Users/ricar/dev/RustLace`.
`gh` CLI at `C:\Users\ricar\dev\.tools\gh\bin\gh.exe` (needed for RQ-013).

## Goal (one sentence)

The remediation branch satisfies requirements RQ-001 through RQ-024 with reproduced evidence —
including green host-ed GitHub Actions runs on Ubuntu and Windows at a named commit — and no
requirement is closed as a residual.

## Verbatim anchor — the user's governing rules

> **P1 — Completion gate:** must be resolved and evidenced before the documentation-remediation
> branch is called complete.
>
> **Source authority:** executable implementation, actual test output, and verified CI results take
> precedence over a stale prose statement. A passing string/link check does not prove numerical
> correctness.
>
> **No regressions:** do not restore the old fixed-width `movingavg` calculation, preserve an
> incorrect value to achieve C# parity, change default precision without a separate requirement, or
> weaken platform-support invariants to make prose pass.
>
> **Validation honesty:** static source inspection and locally recorded runs are evidence, but
> neither substitutes for actual GitHub Actions results on Windows and Linux.

## Mechanism ledger — every requirement is KEPT

No requirement is replaced, narrowed, or dropped. Each maps to a check below.

| ID | Requirement (title, verbatim) | Check | Priority |
|---|---|---|---|
| RQ-001 | Keep the repaired moving-average mathematics intact | C1 | P1 |
| RQ-002 | Correct the scope of moving-average precision claims | C2 | P1 |
| RQ-003 | Make numerical-claim categories mathematically defensible | C3 | P1 |
| RQ-004 | Remove the single-supported-platform instruction regression | C4 | P1 |
| RQ-005 | Fix three known factual errors in crate reference pages | C5 | P1 |
| RQ-006 | Run every implemented documentation-consistency mode in CI | C6 | P1 |
| RQ-007 | Eliminate zero-selected-test and zero-example false positives | C7 | P1 |
| RQ-008 | Remove host-specific paths from the four tutorials | C8 | P1 |
| RQ-009 | Make all four tutorials executable CI contracts | C9 | P1 |
| RQ-010 | Compile and execute public Rust reference examples | C10 | P1 |
| RQ-011 | Reconcile test counts and attach them to an actual revision and host | C11 | P1 |
| RQ-012 | Bring release instructions into alignment with actual CI | C12 | P1 |
| RQ-013 | Obtain actual hosted cross-platform verification | C13 | P1 |
| RQ-014 | Cross-platform builtin-catalog synchronization | C14 | P2 |
| RQ-015 | Validate maintained Markdown links and anchors, including root docs | C15 | P2 |
| RQ-016 | Keep the reference-page manifest synchronized with actual pages | C16 | P2 |
| RQ-017 | Remove current-documentation assertions that are already obsolete | C17 | P2 |
| RQ-018 | Audit exactness wording across language and DSP documentation | C18 | P2 |
| RQ-019 | Strengthen mathematical example evidence without turning documentation into a proof dump | C19 | P2 |
| RQ-020 | Restore reader-first README and reference navigation coherence | C20 | P2 |
| RQ-021 | Clarify provenance and current verification of older language pages | C21 | P2 |
| RQ-022 | Verify agent instructions as operational artifacts | C22 | P2 |
| RQ-023 | Create a compact, machine-readable documentation evidence summary | C23 | P2 |
| RQ-024 | Separate user-facing documentation from harness work logs | C24 | P3 |

## Definition of done — falsifiable checks

`met` requires the observable reproduced by me, not reported by an agent.

| # | Check command | Expected observable |
|---|---|---|
| C1 | `cargo test -p lovelace-lang movingavg` | nonzero matched tests, 0 failed; `movingavg([2.5],3)` → `[0.8(3)]`, `([0.1],3)` → `[0.0(3)]`, `([1/3],3)` → `[0.(1)]` exact, `([1/3],7)` → `[0.(047619)]`; identical at 30/37/38/1000 |
| C2 | `grep -n "no longer depends on the digit budget" docs/extensions/dsp.md` | 0 hits; scoped replacement present; no claim that `0.(8)` is unreachable for any input; long-period truncation documented against `Real::from_fraction` |
| C3 | `grep -c` of the four claim categories in `docs/editorial.md` | exact, precision-limited, error-bounded, parity-verified all defined; `inexact` documented as provenance only; same vocabulary in dsp, real crate page and all four tutorials |
| C4 | `bash tools/check-docs.sh --claims`; `grep -rn "exactly one supported\|one-supported-row" docs .dsh .github AGENTS.md` excluding `docs/goal-docs-*/` (harness memory quotes the phrase as evidence) | 0 hits of the single-row claim in current instructions; `sync-docs` requires per-platform coverage and cites `platform_claims`; `cargo test -p lovelace-desktop platform_claims` passes |
| C5 | open the three crate pages | int example uses a valid divisor and asserts quotient/remainder; json page says sorted `BTreeMap` order consistently; real page says thread-local scoped, not process-wide |
| C6 | `.github/workflows/ci.yml` docs job | invokes all three `check-docs` modes and `check-reference-pages`; both runners on ubuntu and windows; each mode has a recorded mutation test |
| C7 | a `0 tests` / `0 examples` / missing page / bad filter run | each fails non-zero with a diagnostic; skill uses a filter that selects `docs_sync` and asserts a nonzero count |
| C8 | grep tutorials for `/tmp/rl-target` | 0 hits; commands use project-root `cargo run -p lovelace-run`; temp dirs OS-appropriate; windows route documented |
| C9 | `bash tools/run-tutorials.sh` (new) | four tutorials verified; fails if one is omitted; one independent numerical oracle; corruption of an expected block makes it fail |
| C10 | `bash tools/check-crate-examples.sh` (new) or documented-test route | the maintained Rust fences compile and assert; syntax drift fails the build |
| C11 | `docs/testing.md` arithmetic + provenance block | rows sum to the stated total or the gap is explained; 1737 vs 1750 explained by commit; 715 Linux and 716 Windows kept separate; each aggregate names commit, OS, command, counts, date |
| C12 | release skill review | covers Windows, Linux and docs jobs (both OS variants); both binaries and both checksums; platform-separated commands; no fixed `v0.1.0` instruction |
| C13 | `gh run list` / `gh pr checks` at the final commit | Ubuntu **and** Windows docs job green, plus the platform build/test jobs; run IDs recorded |
| C14 | builtin-registry sync during `run-examples` | removing/renaming a documented builtin fails the job on both platforms |
| C15 | `bash tools/check-docs.sh` extended | root Markdown, `docs/`, agent instructions scanned; heading fragments verified; broken anchor fails; historical exclusions documented |
| C16 | reference manifest check | unlisted callable-surface page fails; listed-but-missing page fails; empty mandatory section fails; the 16 manifested pages still pass |
| C17 | cross-file inventory greps | no "none exist yet" tutorial note; README skill count matches `docs/skills.md`; no obsolete source locations in current docs |
| C18 | grep for "Numbers are exact" / "over exact numeric vectors" across `docs/language/`, `docs/extensions/dsp.md` and the DSP guide | 0 unqualified hits; representation vs algorithm separation present in both the language pages and the DSP pages |
| C19 | tutorial review | each states goal, runnable example, interpretation, limitations, precision, verification; oracle source named; no unannotated transcripts |
| C20 | README **and navigation** review | value proposition, quick start, limits, links first; CI/release detail moved out; no conflicting counts; tutorials and reference pages are reachable from `docs/README.md` without reading migration history |
| C21 | `docs/language/README.md` provenance | distinguishes "authored against" from "verified on this branch" |
| C22 | command audit over `AGENTS.md`, copilot instructions, `.dsh/skills/**`, contributing/release guides | every command valid in its stated shell; no nonexistent file or stale flag; zero-test success rejected |
| C23 | docs CI summary artifact | a **machine-readable** record (per-line `key=value` or JSON) of commit, OS, command, checked-page and checked-example counts, skipped checks and exit codes, uploaded or printed by the docs job; no bare "passed" |
| C24 | docs index + archive layout | `docs/goal-docs-v1/` preserved but not dominating navigation; public index highlights tutorials, references, limits |

## Hard constraints (gates)

| ID | Constraint | Provenance |
|---|---|---|
| K1 | The `movingavg` fix and its exactness provenance stay exactly as verified; no fixed-width substitution returns | user (RQ-001) |
| K2 | Never weaken, delete, skip or relax a test to reach green | repo (AGENTS.md) |
| K3 | A passing link/string check never counts as proof of numerical correctness | user |
| K4 | Local evidence never substitutes for hosted CI evidence (RQ-013) | user |
| K5 | Historical records may describe former defects but must be labelled historical; current instructions must not present them as expected behaviour | user |
| K6 | `main` is not touched; work stays on `docs/documentation-quality-remediation` | user (target branch) |

## Stop criteria

1. **C1–C24 all reproduce** — the user decided that every one of the 24 requirements gates
   completion (DEC-001 in `journal.md`), **and**
2. RQ-013 is satisfied by actual hosted runs at the final commit, **and**
3. no C14–C24 item is left as an accepted residual; each is met with evidence of its own.

Round cap: 12. The cap is a report point, never convergence.

## Deliberate non-goals

- Restarting the documentation rewrite. RQ-001 explicitly preserves the repaired mathematics and the
  scope section forbids re-litigating it.
- Changing default precision (excluded by RQ-001's "no regressions" clause).
- Achieving C# parity for any value that is mathematically wrong.

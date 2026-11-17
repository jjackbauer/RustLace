# Round 2 — PR #9 documentation remediation

- **Goal**: make PR #9's documentation match the review remediation now in the working tree.
- **Branch**: `fix/release-provenance`, base `8ec1aaf`.
- **Host and date**: Windows 11 x86_64, 2026-11-17 (America/Sao_Paulo), in the shared workspace.
- **Scope**: the 11 tracked pages named by the task, plus this record. No Rust, Python, JSON
  policy/fixture or `.gitignore` file was edited, and no commit was made.
- **Author**: documentation implementer subagent.

## 1. Pages updated

| Page | Change |
|---|---|
| `docs/architecture-ownership.md` | Replaced the crate-level description with abstraction-specific ownership (`reservedTypeNames`, `permittedRepresentations`, retired and rejected `ownerCrates`); added split/generic/free-function detection and the `review-required` diagnostic; made wildcard exception scopes an explicit rejection (`*`, `?`, `[`, `]`); recorded 20 mutation scenarios, the 14/332/0 real-workspace scan, the 7/7/0 valid fixture, the seven required PR checks, the negative-probe PR #10, CODEOWNERS, and the single-collaborator limitation (DEC-002). |
| `docs/crates/lovelace-rational.md` | Documented `from_parts_with`, `Limits::check_digits`, the add/sub/mul/div and power preflights, cancellation before and after normalization, and the `Nat::gcd` granularity limit. The page states no test count, so no number was changed. |
| `docs/crates/lovelace-systems.md` | Documented the additive `exact_values` sidecar on the `Station` and `QueueNetwork` model-construction records, the parser's consume-first/absent-only-fallback rule, the 1/97 open and closed paths, and the malformed-sidecar refusal. |
| `docs/extensions/systems.md` | Same construction-record transport and parser rule in the language reference, with the analysis-result wording kept; added the 1/97 family regressions to the tests note. |
| `docs/language/precision-and-formatting.md` | Added one prose paragraph: exact model construction is transported through `exact_values`, not the decimal projection. No new fenced example. |
| `docs/rnl-001-evidence.md` | 6267 → 6276; 9 → 12 rational property campaigns; 11 → 20 mutation scenarios; replaced the "branch protection is an administrative setting" limitation with the configured seven-check protection and the CODEOWNERS limitation; added §6 mapping Findings 1–7 to the fixes. |
| `docs/testing.md` | Aggregate 45 targets / 6276 passed / 0 ignored; `lovelace-lang` 1747 and `lovelace-rational` 30; row-composition notes (16 unit, 12 properties, 2 oracle; 1742 in-crate + 5 cross-library); architecture bullet now 20 scenarios. |
| `docs/contributing.md` | Added the CODEOWNERS requirement for architecture-policy/ownership changes, the `review-required` diagnostic, the configured branch protection, and the 20-scenario definition of done. |
| `CONTRACT.md` | Added `exact_values` to the `Station`/`QueueNetwork` model-record contract, its parser consumption, and the checker's abstraction-specific ownership and 20-scenario gate. |
| `AGENTS.md` | Architecture checker/CI bullets now say 20 scenarios and CODEOWNERS; the one-foundation rule is unchanged. |
| `docs/adr/011-shared-rational-kernel.md` | Appended "Post-review correction (2026-11-17)" covering model-transport exactness, abstraction-specific ownership and the code-owner limitation; the original decision is untouched. |

## 2. Source of the documented facts

The numbers and mechanisms come from the round-2 records
[`model-transport.md`](model-transport.md), [`checker.md`](checker.md),
[`budgets.md`](budgets.md) and [`hygiene.md`](hygiene.md), plus the orchestrator's
[`evidence.md`](../evidence.md). No count was inferred, and none was changed beyond the task's
listed corrections (6267 → 6276, 11 → 20 mutation scenarios, 9 → 12 property campaigns, 26 → 30
rational tests, 1742 → 1747 language tests; the last three follow from the same final suite).

## 3. Required gates, observed

Run from the repository root as `bash tools/check-docs.sh <mode>`; the exit code was captured from
the command itself.

```console
$ bash tools/check-docs.sh --symbols
check-docs: 80 Markdown files (68 docs/, 3 root, 9 agent instructions), 265 relative links, 48 heading anchors, 537 code references, 21 line-suffix citations, 22 symbol citations
check-docs: OK
DOCREPORT gate=check-docs status=ok pages=80 links=265 anchors=48 refs=537 citations=21 symbols=22 claims=0 tutorials=0 sections=0 readme=0 modes=symbols failures=0
EXIT=0
```

```console
$ bash tools/check-docs.sh --claims
check-docs: 80 Markdown files (68 docs/, 3 root, 9 agent instructions), 265 relative links, 48 heading anchors, 537 code references
check-docs: claims audit: 159 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s), 0 retired-term use(s)
check-docs: OK
DOCREPORT gate=check-docs status=ok pages=80 links=265 anchors=48 refs=537 citations=0 symbols=0 claims=0 tutorials=5 sections=5 readme=0 modes=claims failures=0
EXIT=0
```

```console
$ bash tools/check-docs.sh --readme
check-docs: 80 Markdown files (68 docs/, 3 root, 9 agent instructions), 265 relative links, 48 heading anchors, 537 code references
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
DOCREPORT gate=check-docs status=ok pages=80 links=265 anchors=48 refs=537 citations=0 symbols=0 claims=0 tutorials=0 sections=0 readme=4 modes=readme failures=0
EXIT=0
```

```console
$ bash tools/check-reference-pages.sh
check-reference-pages: 30 scoped page(s): 19 manifested, 11 declared non-reference, every page classified
check-reference-pages: 133 required sections in order and non-empty
check-reference-pages: OK
DOCREPORT gate=check-reference-pages status=ok pages=30 manifested=19 non_reference=11 unclassified=0 sections=133 failed_pages=0 failures=0
EXIT=0
```

```console
$ bash tools/check-crate-examples.sh
check-crate-examples: cargo: /home/ricardo/.cargo/bin/cargo (from $HOME/.cargo/bin/cargo)
PASS docs/crates/lovelace-array.md:83
PASS docs/crates/lovelace-int.md:71
PASS docs/crates/lovelace-json.md:110
PASS docs/crates/lovelace-json.md:128
PASS docs/crates/lovelace-lang.md:93
PASS docs/crates/lovelace-nat.md:87
PASS docs/crates/lovelace-rational.md:109
PASS docs/crates/lovelace-real.md:118
PASS docs/crates/lovelace-symbolics.md:88
PASS docs/crates/lovelace-systems.md:113
EXCLUDED (none)
check-crate-examples: 15 page(s), 10 rust fence(s): 10 compiled and ran, 0 excluded
check-crate-examples: OK
DOCREPORT gate=check-crate-examples status=ok pages=15 fences=10 compiled=10 excluded=0 failures=0
EXIT=0
```

## 4. Gate notes

The three `check-docs` modes first failed while the goal working-memory directory was still
`docs/goal-rnl-001-review/`: that path is not one of `check-docs.sh`'s excluded `docs/goal-docs-*`
trees, so the round-1/round-2 evidence records' source line suffixes and unresolved probe paths were
scanned as reader-facing documentation. After the rename to `docs/goal-docs-rnl-001-review/`, all
three modes pass. This report lives in that excluded tree, so it does not add itself to the scan; the
21 line-suffix citations and 22 symbol citations the passing run reports are in locations the
checker permits.

## 5. Not verified here

- No Rust test, corpus run, architecture checker run, mutation-suite run or `gh` protection read-back
  was executed by this task. The 6276 aggregate, the 20-scenario suite, the 14/332/0 scan and the
  branch-protection/PR #10 facts are quoted from the round-2 records and `evidence.md`
  (EVD-011, EVD-015, EVD-019, OBS-006–OBS-011), not re-measured here.
- `docs/crates/lovelace-rational.md` states no test count, so the conditional 30-test update did not
  apply and no count was invented for it.
- `docs/crates/lovelace-lang.md` still says "1737 `#[test]` functions" (line 107 of that page); it is
  outside this task's scope, so it was not edited, but it now disagrees with the 1742 in-crate
  figure in `docs/testing.md`.
- No commit was made; the changes are working-tree edits plus this record.

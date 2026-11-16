# Harness State — goal-docs-v2

- **Round**: 5 (dispatched)
- **Goal**: The branch satisfies RQ-001…RQ-024 with reproduced evidence, including green hosted
  GitHub Actions runs on Ubuntu and Windows at a named commit, with no requirement closed as a
  residual.
- **Definition of done**: 13/24 met (C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11, C12, C18),
  11 open
- **Stop criteria**: (1) C1–C24 all reproduce — **unmet**; (2) hosted runs recorded — **unmet**;
  (3) no residual — **unmet**

## Gate status

| Gate | Status | Detail |
|---|---|---|
| G0 Intent | PASS | audit 27/27 KEPT; DEC-001 (all 24 gate) |
| G1 Evidence | PASS | 38 EVD rows, each reproduced by the orchestrator |
| G2 Falsification | PASS | VAL-002 and VAL-004 falsified; both remediated or superseded |
| G3 Coverage | FAIL | 11 checks open |
| G4 Reproduction | PASS | rounds 1–4 re-run; six falsifications with hash-verified restores |
| G5 Honesty | PASS | my formatting miss, two measurement errors and three TODOs are all recorded |

## Round 5 objective (recorded before dispatch)

Close the coverage-integrity group:
- **RQ-015** extend the consistency checker beyond `docs/` — root Markdown, agent instructions, and
  heading **anchors** (`#fragment`), not just files; honour declared historical exclusions.
- **RQ-016** make the reference manifest two-way: an unlisted new callable-surface page fails, a
  listed-but-missing page fails, an empty mandatory section fails.
- **RQ-014** verify the builtin catalogue against the runtime `--functions` output on **both**
  platforms (today the cross-check is PowerShell-only).
- **RQ-022** audit every command in `AGENTS.md`, the Copilot instructions, `.dsh/skills/**`,
  `contributing.md` and `release.md`: valid in its stated shell, no nonexistent file, no stale flag,
  no zero-test success.
- **TODO-004 / TODO-005 / TODO-006**: the "eight task skills" claim, the wiring guard's blind spot
  for the two new CI steps, and the stale `AGENTS.md` CI table.

Split so no two agents share a file:
- Agent J owns `tools/check-docs.sh`, `tools/check-reference-pages.sh`, `docs/reference-pages.md`,
  `tools/test-docs-gates.sh` → C15, C16, TODO-005. **No `ci.yml` edits.**
- Agent K owns `docs/language/examples/run-examples.sh`, `AGENTS.md`,
  `.github/copilot-instructions.md`, `.dsh/skills/**`, `docs/contributing.md`, `README.md`,
  `.github/workflows/ci.yml` → C14, C22, TODO-004, TODO-006.

Coordination rule given to J: the wiring guard asserts a **subset** of required `ci.yml` steps, so
Agent K may add a step without breaking it; J must not require an exact step count.

## Dimension status

| # | Check | Status |
|---|---|---|
| C1–C12, C18 | — | **Met** (13 with C3) |
| C13 | hosted runs at the final commit | None — last round by design |
| C14 | builtin-registry sync | in flight |
| C15 | root Markdown + anchors | in flight |
| C16 | manifest two-way | in flight |
| C17 | obsolete assertions | Todo-004 in flight |
| C19, C20, C21, C24 | reader experience | None — round 6 |
| C22 | agent-instruction audit | in flight |
| C23 | machine-readable summary | None — round 6 |

## Objective ledger

| Round | Objective | Mode | Agents | New EVD | Falsified | Gates | Outcome |
|---|---|---|---|---|---|---|---|
| 0 | Phase 0 contract + intent audit | Probe | 1 | 8 | 0 | G0✓ G1✓ G3✗ | complete |
| 1 | RQ-004, 005, 007, 012 | Probe | 2 | 7 | 1 | G1✓ G2✓ G4✓ | **verified** |
| 2 | RQ-006, 007.5, 010, 011 | Probe | 2 | 7 | 1 | G1✓ G2✓ G4✓ | **verified** |
| 3 | RQ-002, 003, 008, 009 + README | Probe | 2 | 7 | 0 | G1✓ G2✓ G4✓ | **verified** |
| 4 | TODO-003 + CI wiring | Probe | 2 | 5 | 0 | G1✓ G2✓ G4✓ | **verified** |
| 5 | RQ-014, 015, 016, 022 + TODOs | Probe | 2 | — | — | pending | dispatched |

## Next objective

Round 6: RQ-023 (machine-readable evidence summary), RQ-019 (example evidence without a proof
dump), RQ-020 (README/navigation coherence), RQ-021 (language-page provenance), RQ-024 (separate
harness logs from reader navigation).

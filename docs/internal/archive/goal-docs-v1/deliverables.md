# Deliverables — goal-docs-v1

Every entry is an `ART` with at least three supporting IDs and an observation the orchestrator made.
Anything not listed here is a `TODO`, not a delivery.

### ART-001: Platform corrections across the six named documents

- **Path**: `docs/adr/001`, `docs/adr/002`, `docs/adr/008-linux-gui-support.md`, `CONTRACT.md`, `.github/copilot-instructions.md`, `.dsh/skills/release-and-publish/SKILL.md`, `docs/skills.md`, `docs/language/README.md`
- **Type**: document
- **Supporting evidence**: OBS-001, EVD-012, EVD-013, EVD-007, EVD-008
- **Confidence**: High
- **Verified by**: `Select-String "only under .cfg\(windows\)"` over every `.md` → **0 hits**; the release skill names both artifacts and both checksum files
- **Last updated**: 2026-11-15

### ART-002: `movingavg` mathematical oracle test

- **Path**: `lovelace-lang/src/dsp_round20.rs`
- **Type**: test-suite
- **Supporting evidence**: EVD-002, EVD-003, EVD-004, EVD-005, HYP-001, VAL-001
- **Confidence**: High
- **Verified by**: `cargo test -p lovelace-lang movingavg` → 9 passed; full crate → 1750 passed
- **Last updated**: 2026-11-15

### ART-003: Editorial guide with the three-claim taxonomy

- **Path**: `docs/editorial.md`
- **Type**: document
- **Supporting evidence**: DEC-001, DEC-004, EVD-004, EVD-016
- **Confidence**: High
- **Verified by**: file exists, carries the exact / precision-bounded / parity table, the review checklist and the "agent instructions are code" rule; indexed from `docs/README.md`
- **Last updated**: 2026-11-15

### ART-004: ADR-004 correction with reproduced mechanism and evidence

- **Path**: `docs/adr/004-thirty-digit-default.md`
- **Type**: document
- **Supporting evidence**: EVD-001, EVD-002, EVD-003, EVD-004, OBS-002, VAL-001
- **Confidence**: High
- **Verified by**: the correction section carries the console transcript I reproduced in WSL and names the regression test that pins both sides
- **Last updated**: 2026-11-15

### ART-005: Two-platform `platform_claims` and release matrix

- **Path**: `lovelace-desktop/src/platform_claims.rs`, `docs/goal-desktop-v4/platform-matrix.md`
- **Type**: implementation + document
- **Supporting evidence**: EVD-018, EVD-022, EVD-023, EVD-024, EVD-025
- **Confidence**: High
- **Verified by**: `cargo test -p lovelace-desktop platform_claims` → **10 passed, 0 failed**; rebuilt Linux binary prints `advertised=true`, `supported[linux/x86_64]`, no Linux refusal, macOS refusal naming both, `result PASS`, exit 0; diff shows the invariant strengthened, not relaxed
- **Last updated**: 2026-11-15

### ART-006: Four executable tutorials

- **Path**: `docs/tutorials/linear-algebra.md`, `symbolic.md`, `signal-processing.md`, `reproducible.md`, `README.md`
- **Type**: document
- **Supporting evidence**: EVD-030, EVD-031, OBS-004, OBS-005
- **Confidence**: High
- **Verified by**: a verifier that extracts each file's `## Verification` block, runs it, and diffs against its `## Output` block → four exact matches, exit 0, seven required headings in order in every file; the four surprising symbolic sub-claims re-run independently and confirmed
- **Last updated**: 2026-11-15

### ART-007: Documentation example gate on both CI platforms

- **Path**: `docs/language/examples/run-examples.sh`, `docs/extensions/examples/run-examples.sh`, `.github/workflows/ci.yml` (`docs` job)
- **Type**: implementation
- **Supporting evidence**: EVD-032, EVD-033, EVD-034, VAL-006
- **Confidence**: High
- **Verified by**: I ran both runners (193 + 27 = 220 examples, exit 0), then corrupted `docs/language/lexical-structure.md:58` and observed exit 1 with `FAIL ...:54 (exit 1): output mismatch` and `1 of 193 example(s) failed`; restored byte-identically (`cmp` 0) and re-ran to green with no git residue
- **Last updated**: 2026-11-15

### ART-008: `sync-docs` agent skill with the dependent-document map

- **Path**: `.dsh/skills/sync-docs/SKILL.md`, indexed in `docs/skills.md` and `AGENTS.md`
- **Type**: document
- **Supporting evidence**: EVD-038, DEC-016, VAL-007
- **Confidence**: High
- **Verified by**: file exists; indexed at `docs/skills.md:18` and in AGENTS.md's skill table; the harness's own skill catalog refreshed to list `sync-docs`, which is independent proof the skill is discoverable by an agent
- **Last updated**: 2026-11-15

### ART-009: Archival boundary and the epistemic limit

- **Path**: `docs/goal-desktop-v4/journeys.md`, `docs/editorial.md`, `docs/testing.md`, `AGENTS.md`, `docs/contributing.md`
- **Type**: document
- **Supporting evidence**: EVD-035, EVD-036, EVD-037, DEC-015
- **Confidence**: High
- **Verified by**: journey tests still **7 passed, 0 failed** after the banner (the file is machine-read); the limit sentence is at `docs/editorial.md:47` and `docs/testing.md:7`; the editorial requirement is stated in `AGENTS.md` and `docs/contributing.md:78`
- **Last updated**: 2026-11-15

### ART-011: `tools/check-docs.sh` documentation consistency checker

- **Path**: `tools/check-docs.sh`, plus the `--symbols` step appended to the `docs` CI job
- **Type**: implementation
- **Supporting evidence**: EVD-039, EVD-040, VAL-008
- **Confidence**: High
- **Verified by**: I ran both modes: `55 Markdown files, 147 relative links, 224 code references` → exit 1 with 4 failures; `--symbols` → exit 1 with 145. The implementer falsified both a broken link and a broken code reference (caught, then restored byte-identically) and proved the exit-0 and unknown-symbol paths on a synthetic tree. **Not yet green** — see TODO-016.
- **Last updated**: 2026-11-15

### ART-012: Reference-page contract, manifest and gate

- **Path**: `docs/reference-template.md`, `docs/reference-pages.md`, `tools/check-reference-pages.sh`, the 16 restructured pages, plus the CI step
- **Type**: document + implementation
- **Supporting evidence**: EVD-045, DEC-006, DEC-019
- **Confidence**: High
- **Verified by**: `bash tools/check-reference-pages.sh` → `16 page(s), 112 required sections in order` / `OK`, exit 0; I renamed `## Outputs` in `docs/crates/lovelace-know.md` and it failed with two precise defects, then restored to an **identical sha256** and it passed; the extensions example gate still reports 27/27 and the language gate 193
- **Last updated**: 2026-11-15

### ART-013: Claim audit and README structure checks

- **Path**: `tools/check-docs.sh` (`--claims`, `--readme`), `README.md`, `docs/tutorials/README.md`
- **Type**: implementation + document
- **Supporting evidence**: EVD-045, EVD-046, DEC-019
- **Confidence**: High
- **Verified by**: `bash tools/check-docs.sh --claims` and `--readme` both exit 0; the implementer falsified six conditions individually (each failed, restored byte-identically, passed). The check found two real defects: `docs/tutorials/README.md` had no `## Precision`, and the README used an H3 where the contract requires an H2.
- **Last updated**: 2026-11-15

## Not yet delivered (tracked as TODO, not claimed)

| Mechanism | Blocking item |
|---|---|
| DOC-008 `platform-matrix.md` marker | resolved by DEC-015 (it is current, not historical) |
| DOC-011 enforcement path | D12 partial (TODO-012) |
| DOC-012 link and code-reference validation | TODO-013 |
| DOC-014 reference-page template + gate | TODO-014 (scope decided, DEC-006) |
| DOC-015 line-reference sweep | TODO-007 |
| Documentation-sync skill | TODO-011 (D17) |
| The `docs` CI job | TODO-012 (D18) |
| Epistemic limit stated | D19 |

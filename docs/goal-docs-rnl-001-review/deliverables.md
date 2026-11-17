# Deliverables — rnl-001-review

| ART | Artifact | Path | Evidence | Confidence |
|---|---|---|---|---|
| ART-001 | Exact model-construction transport and 1/97 regressions | `lovelace-lang/src/systems_family.rs` | EVD-012, EVD-013, EVD-014 | High |
| ART-002 | Abstraction-specific checker, policy and 20-scenario mutation suite | `tools/check-architecture.py`, `tools/architecture-policy.json`, `tools/test-architecture-gates.py`, `tools/architecture-fixtures/base/` | EVD-015, EVD-020 | High |
| ART-003 | Budget preflights and cancellable normalization | `lovelace-rational/src/lib.rs`, `lovelace-rational/tests/properties.rs` | EVD-016 | High |
| ART-004 | Benchmark prose and Python-cache hygiene | `docs/benchmarks-numerics.md`, `.gitignore` | EVD-017 | High |
| ART-005 | Required checks and negative-probe evidence | `.github/workflows/ci.yml`, `tools/architecture-policy.json`, branch protection | EVD-011, EVD-018, EVD-020 | High |
| ART-006 | Code-owner declaration and user-approved limitation | `.github/CODEOWNERS`, DEC-002 | EVD-011, DEC-002 | High |
| ART-007 | Documentation remediation | `CONTRACT.md`, `AGENTS.md`, `docs/architecture-ownership.md`, ADR-011, crate/extension/precision/testing/contributing pages, `docs/rnl-001-evidence.md` | EVD-021 | High |
| ART-008 | Harness memory | `docs/goal-docs-rnl-001-review/` | this file | High |

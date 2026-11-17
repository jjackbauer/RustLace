# Deliverables — rnl-001-review-2

| ART | Artifact | Path | Evidence | Confidence |
|---|---|---|---|---|
| ART-001 | `from_parts_with` enforces both limits | `lovelace-rational/src/lib.rs` | EVD-001, EVD-002 | High |
| ART-002 | Budget coverage tests | `lovelace-rational/tests/properties.rs` | EVD-003 | High |
| ART-003 | Partial-representation checker signal | `tools/check-architecture.py`, `tools/architecture-policy.json`, fixture policy | EVD-004..EVD-006 | High |
| ART-004 | 21-scenario mutation suite | `tools/test-architecture-gates.py` | EVD-005 | High |
| ART-005 | Documentation and measured counts | `docs/testing.md`, `docs/rnl-001-evidence.md`, `docs/architecture-ownership.md`, `CONTRACT.md`, `AGENTS.md`, `docs/contributing.md`, `docs/crates/lovelace-rational.md` | EVD-007, docs gates | High |
| ART-006 | Green PR evidence | commits `1729ef5`/`93fbf38`, CI run 44140509356 | EVD-008 | High |

# Deliverables — `systems`

Each `ART` needs a path, a type, at least three supporting evidence IDs, a confidence, and the
command that verified it. An artifact below `Verified by: —` is a TODO, not a deliverable.

---

<!-- ART entries are appended as rounds deliver them. -->

---

### ART-001: Modus removal (code, tests, wire, desktop)

- **Path**: `lovelace-lang/src/{lib.rs,registry.rs,builtins.rs,help.rs,structured.rs,parameter_domain_family.rs}`, `lovelace-run/src/**`, `lovelace-desktop/src/**`, `docs/csharp-oracle/Lovelace.Run.Tests/fixtures/*.json`
- **Type**: implementation
- **Supporting evidence**: EVD-017, EVD-018, EVD-020, EVD-021
- **Confidence**: High
- **Verified by**: `git grep` for every removed symbol (exit 1, no output); `cargo test --workspace` exit 0; `--function fft` reports `module` and no `plugin`; corpus 231/0
- **Last updated**: 2026-11-17

### ART-002: `lovelace-systems` standalone crate

- **Path**: `lovelace-systems/`
- **Type**: implementation
- **Supporting evidence**: EVD-019, EVD-022, EVD-025, EVD-020
- **Confidence**: High
- **Verified by**: `cargo test -p lovelace-systems` (49 unit + 1 doctest, exit 0); `cargo tree` shows only the three numeric crates; float/unsafe grep exit 1
- **Last updated**: 2026-11-17

### ART-003: `sys_*` adapter and structured language surface

- **Path**: `lovelace-lang/src/systems_family.rs`, `lovelace-lang/src/registry.rs`, `lovelace-lang/src/builtins.rs`
- **Type**: implementation
- **Supporting evidence**: EVD-018, EVD-025, EVD-022, EVD-020
- **Confidence**: High
- **Verified by**: `cargo test -p lovelace-lang systems_family` (8 tests); `sys_mm1(8,10)` JSON carries exact `4/5`; `--function sys_mm1` shows the declared parameters
- **Last updated**: 2026-11-17

### ART-004: Systems corpus, docs pages and reference manifest

- **Path**: `parity/corpus/systems.json`, `docs/extensions/systems.md`, `docs/crates/lovelace-systems.md`, `docs/reference-pages.md`, `CONTRACT.md`, `docs/adr/009-remove-modus-static-families.md`, `docs/architecture.md`
- **Type**: document + dataset
- **Supporting evidence**: EVD-021, EVD-023, EVD-026, EVD-018
- **Confidence**: High
- **Verified by**: release corpus 231/0; extensions examples 31/31; crate fences 9/9; reference pages 18 manifested and OK; docs diagnostics filtered to zero outside the concurrent goal
- **Last updated**: 2026-11-17

### ART-005: Systems benchmark harness and measured report

- **Path**: `lovelace-bench/src/main.rs`, `docs/benchmarks-systems.md`
- **Type**: benchmark
- **Supporting evidence**: EVD-019, EVD-022, EVD-027
- **Confidence**: Medium (one host, one profile, 20 repetitions; the page says so)
- **Verified by**: `cargo build --release -p lovelace-bench`; the three ops ran at 30 and 1000 digits with identical digests for the closed-form models and the documented means
- **Last updated**: 2026-11-17

### ART-006: Harness record

- **Path**: `docs/goal-docs-systems/`
- **Type**: report
- **Supporting evidence**: EVD-017, EVD-023, EVD-028
- **Confidence**: High
- **Verified by**: every EVD row names a command that was run or a line that was read in this session; the intent audit is an independent artifact
- **Last updated**: 2026-11-17
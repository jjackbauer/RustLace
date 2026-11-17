# Goal — RustLace native systems modeling, complete Modus removal

- **Slug**: `systems`
- **Created**: 2026-11-17
- **Repository**: `F:\dev\RustLace` at `619df33` (`feat/desktop-logo-icon`)
- **Harness**: stretch-goal-harness (orchestrator + independent verification)

## Goal (one sentence)

RustLace ships `lovelace-systems` as a first-class, statically integrated exact queueing /
operations-research crate exposed through the one static builtin registry, while every trace of the
Modus plugin mechanism is deleted and all 124 pre-existing builtins (DSP, Symbolics, MathIR, core)
remain available in an ordinary engine.

## User request, verbatim (scope and priority)

> **Scope:** Remove the unused Modus plugin mechanism; deliver `lovelace-systems` as a first-class, statically integrated native numerical library.
> **Priority:** P0 required for initial delivery; P1 follow-on; P2 explicitly deferred.

> **OUT-001 [P0]** Deliver a standalone public Rust crate `lovelace-systems` for analytical systems modeling and performance analysis, callable directly from Rust and through the existing Lovelace language/CLI/desktop engine.
> **OUT-002 [P0]** Delete the Modus extension mechanism completely, not merely disable it. No `ModusHost`, `ModusPlugin`, `Payload`, `PluginArgs`, `FieldKernel` integration, `Engine::load_plugin`, fake extension loading, optional plugin family gating, or runtime plugin registration remains as an advertised capability.
> **OUT-003 [P0]** Keep all pre-existing mathematical functions usable in an ordinary engine. DSP, Symbolics and MathIR remain statically compiled built-in functionality and must not be removed by this change.
> **OUT-005 [P0]** Reuse RustLace's arbitrary-precision numeric types and existing language/array/plot infrastructure. No floating-point numerical value path, no new third-party engine dependencies and no unsafe code in the systems crate.

## Named mechanisms — intent table

Every named mechanism is preserved or executed as the user's text requires. `DEFERRED` rows are
the user's own P1/P2 priorities, not an agent scope cut; they stay out of the P0 delivery and are
listed as follow-ons.

| Mechanism named by the request | Verdict | Where it lands |
|---|---|---|
| Delete `ModusHost`, `ModusPlugin`, `Payload`, `PluginArgs`, `FieldKernel`, `Engine::load_plugin`, `new_bare`, `enabled_plugins`, plugin gating | KEPT (deleted) | PR A |
| One static builtin registration/dispatch mechanism, all pre-existing math families available | KEPT | PR A, registry `module` label (non-executable) |
| `lovelace-systems` standalone public crate | KEPT | PR B, `lovelace-systems/` |
| Typed models: `Station`, `ServiceDiscipline`, `Workload`, `OpenWorkload`, `ClosedWorkload`, `QueueNetwork`, `SolverOptions`, `AnalysisResult`, stable ids | KEPT | PR B, `model.rs` / `results.rs` |
| Modules: units, model, validation, queueing, operational_laws, networks, mva, capacity, sweep, results, errors | KEPT | PR B |
| M/M/1, M/M/c (Erlang C), M/M/∞ delay center | KEPT | PR B |
| Operational laws (Little, utilization, forced flow, interactive response), bottleneck and capacity bounds | KEPT | PR B |
| Open Jackson network via visits / validated traffic equations | KEPT | PR B |
| Exact single-class MVA with visit ratios and think time | KEPT | PR B |
| Sweeps and plotted output with existing SVG plot facility | KEPT | PR C |
| Static builtin family `sys_mm1`, `sys_mmc`, `sys_station`, `sys_closed`, `sys_open`, `sys_analyze`, `sys_mva`, `sys_bottlenecks`, `sys_sweep` | KEPT | PR C |
| Structured records + solver provenance, not display strings | KEPT | PR C, `structured.rs` |
| Wire compatibility decision: remove `functions[].plugin`, bump protocol, document migration | KEPT | PR A, `protocolVersion` 2 |
| Baseline 124 builtins preserved; Systems builtins additive | KEPT | gate D2 |
| `lovelace-lang -> lovelace-systems -> numeric crates` direction | KEPT | PR C manifests |
| No new external dependency, no floating point value path, no `unsafe` in systems | KEPT | gates D4/D9 |
| P1: multiclass exact/approximate MVA, M/G/1, G/G/1, Amdahl, fitting | DEFERRED (user P1) | follow-on |
| P2: simulation, fork-join, mixed networks, GUI editor | DEFERRED (user P2) | follow-on |

## Definition of done — falsifiable dimensions

Each check names an exact command and its expected observable. A dimension is `Covered` only when
the command was run by the orchestrator with the recorded result.

| # | Dimension | Command | Expected observable |
|---|---|---|---|
| D1 | Modus scaffolding gone | `git grep -n -i modus -- '*.rs' '*.toml' '*.yml' '*.sh' '*.ps1'` (plus `rg` over `docs/` for active-architecture mentions) | No production/test hits except labelled history; `lovelace-lang/src/modus.rs` absent; no `new_bare`, `load_plugin`, `enabled_plugins`, `plugin_name_of`, `plugin_enabled`, `register_builtin`, `register_kernel`, `try_dispatch_kernel`, `registered_descriptor`, `register_assumption_context` symbols remain |
| D2 | Baseline math preserved | `cargo test --workspace`; `cargo run --release -p lovelace-corpus`; built runner `--functions` | exit 0; corpus `214 passed, 0 failed` (or more, never fewer); function count >= 124 with DSP/Symbolics/MathIR families present |
| D3 | Wire contract migrated | `cargo run -p lovelace-run -- --eval "1/3" --json`; goldens; `CONTRACT.md` | `protocolVersion` = 2, `functions[]` carry `module` not `plugin`, migration documented, golden fixtures regenerated with `LOVELACE_RECORD_GOLDENS=1` |
| D4 | Systems crate standalone and dependency-clean | `cargo test -p lovelace-systems`; `cargo tree -p lovelace-systems` | exit 0; dependency tree has `lovelace-nat/int/real`, no `lovelace-lang`, no `lovelace-desktop`, no external crate; `#![forbid(unsafe_code)]` |
| D5 | Golden queueing math exact | `cargo test -p lovelace-systems` (golden tests named `test_001`..`test_006`) | TEST-001 `rho=4/5`, `W=1/2`, `Wq=2/5`, `L=4`, `Lq=16/5`; TEST-002 `Pw=32/45`, `Wq=16/45`, `W=5/9`; TEST-003 `X=10`, `R=1/100`, `U=1/10`, `Q=1/10`; unstable lambda>=mu refused |
| D6 | Language integration end to end | built `lovelace-run --eval "sys_mm1(8, 10)" --json`; `--functions`; `--function sys_mm1`; sample script | structured record with exact rational fields and solver provenance; sys names in discovery/help/autocomplete; samples pinned by executable docs |
| D7 | Networks, laws, MVA, bottlenecks, sweeps | `lovelace-run` over the sample corpus + systems fixtures | conservation laws hold exactly; bottleneck under matching assumptions; sweep arrays; unsupported models refuse |
| D8 | Documentation rewritten | `tools/check-docs.sh --symbols --claims --readme`, `tools/check-reference-pages.sh`, example runners | exit 0; extension docs describe static library families; no active Modus architecture claim |
| D9 | Whole-workspace gates | `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets`; full test suite; desktop headless selftest | exit 0; desktop `--gui-selftest` `checks=20 failed=0` |
| D10 | Independent mathematical verification | new CTMC/reference cross-check + randomized property tests; MVA complexity test | M/M/1, M/M/c, M/M/∞ and MVA values match an independently derived oracle; O(N*K) behaviour asserted without wall-clock promises |

## Hard constraints (gates, not preferences)

| Constraint | Provenance |
|---|---|
| No external crate enters an engine crate; `lovelace-systems` uses only workspace numeric crates | repo (ADR-001) + user OUT-005 |
| No floating point in a value path; no `unsafe` outside the documented Win32 lane | repo + user OUT-005 |
| Never weaken, delete or skip a test to reach green; move fixtures only with a stated contract move | repo guardrail |
| Baseline builtin count 124 must not shrink; Systems builtins are additive | user REM-010 |
| Protocol v1 is not silently changed; wire field removal requires a version bump and migration note | user REM-005 |
| Independent symbolic `assume`/`assumptions` behavior survives; DSP 30-digit default preserved or deliberately revised with tests + ADR | user REM-006/REM-007 |
| One static registration/dispatch mechanism; no replacement plugin framework, DLL/SO/WASM/IPC/package manager | user OUT-002/OUT-004 |
| `lovelace-systems` depends on numeric crates only, never on `lovelace-lang` or desktop | user ARC-001 |

## Preferences (tie-breakers)

- Exact rational arithmetic throughout the systems crate; decimal rendering through `lovelace-real`
  (`Real::from_fraction`) so periodic decimals keep their metadata.
- One cohesive `sys_*` family over dozens of one-formula builtins.
- Metadata fields named `module` (Core/DSP/Symbolics/MathIR/Systems) rather than `plugin`.
- Prefer adding `lovelace-systems` as a workspace member over a new rational crate; audit
  `lovelace-symbolics::Rat` first (`NUM-001`) and record the decision.

## Stop criteria

- D1–D10 all `Covered` at High confidence, zero Falsified rows, zero P0 open questions.
- Round cap: 20 rounds. Stop early only when every dimension is covered.

## Baseline facts (orchestrator-verified at Phase 0)

- `lovelace-lang/src/registry.rs` declares **124** `BuiltinInfo` entries: 80 with
  `plugin: Some(..)` and 44 `plugin: None`.
- `parity/corpus/lang.json` has 111 examples, `symbolics.json` 103: **214** total.
- Protocol version is `"1"` (`lovelace-run/src/main.rs:17`).
- Toolchain: `cargo 1.100.0` at `%USERPROFILE%\.cargo\bin\cargo.exe`; `cargo` is not on the
  default PATH of this shell, so every dispatch states the full path.

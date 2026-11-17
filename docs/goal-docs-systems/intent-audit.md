# Intent audit — `lovelace-systems` proposal vs. user request

Sources read: `docs/goal-docs-systems/goal.md`, `docs/goal-docs-systems/requirements.md`.

Verdict key: **KEPT** = the proposal delivers the mechanism at the user's stated priority;
**KEPT AS FOLLOW-ON** = the proposal defers it at the user's own P1/P2 priority, so it is not
removed; **REPLACED BY x** = the proposal substitutes a different mechanism; **REMOVED** = absent
from both proposal files.

| Mechanism | Verdict | Proposal location (section or "absent") | Consequence |
|---|---|---|---|
| 1. Delete `ModusHost` / `ModusPlugin` / `Payload` / `PluginArgs` / `FieldKernel` / `load_plugin` / `new_bare` / `enabled_plugins` / plugin gating / dynamic dispatch | KEPT | goal.md "Named mechanisms" row 1 (PR A), D1; requirements.md §13 "Deletion rules (PR A)" | The named Modus surface and the `modus.call` / `plugin_enabled` dynamic dispatch path are deleted outright, not disabled. |
| 2. One static registry keeping all 124 pre-existing DSP / Symbolics / MathIR / core builtins | KEPT | goal.md row 2, D2, baseline facts; requirements.md §12 (count stays 124), §11 ("No dynamic registration") | A single static `registry::BUILTINS` dispatch keeps the 124 baseline builtins; Systems names are additive. |
| 3. Standalone `lovelace-systems` crate | KEPT | goal.md row 3, D4; requirements.md §1 "Crate and dependency direction" | New workspace member `lovelace-systems/`, version/edition/license inherited from `[workspace.package]`. |
| 4. Typed `Station` / `ServiceDiscipline` / `Workload` / `QueueNetwork` / `SolverOptions` / `AnalysisResult` | KEPT | goal.md row 4; requirements.md §5 (model), §10 (results), §2 (module map) | Concrete structs/enums in `model` and `results`; `AnalysisResult` carries aggregate + provenance. |
| 5. M/M/1 | KEPT | goal.md row 8, D5 TEST-001; requirements.md §6 | Closed-form rational metrics with `lambda < mu` stability check. |
| 6. M/M/c Erlang C | KEPT | goal.md row 8, D5 TEST-002; requirements.md §6 | `P_wait` by exact stable recurrence; `c >= 2` multi-server discipline. |
| 7. M/M/∞ (delay center) | KEPT | goal.md row 8, D5 TEST-003; requirements.md §6 | `Delay` discipline distinguished by model class, `Wq = 0`, `L = a`. |
| 8. Operational laws (Little, utilization, forced flow, interactive response) | KEPT | goal.md row 9; requirements.md §7 | `operational_laws` module with `LawCheck` results. |
| 9. Bottleneck / capacity bounds | KEPT | goal.md row 9; requirements.md §7, §2 `capacity` module | `X_bound = 1/max(D_i)` plus capacity-adjusted form; names the station and bound kind. |
| 10. Open Jackson networks | KEPT | goal.md row 10; requirements.md §8 "Open networks (NET-001)" | Visits/traffic-equation solution, per-station `X_i`, network `R` and `L`. |
| 11. Exact single-class MVA | KEPT | goal.md row 11, D5; requirements.md §9 "Exact single-class MVA (NET-002, MVA-001..MVA-004)" | Closed single-class recurrence with visit ratios and think time, all invariants checked exactly. |
| 12. Sweeps | KEPT | goal.md row 12; requirements.md §2 `sweep` module, §11 `sys_sweep` | Population/arrival-rate sweeps with bounded point counts, returned as records/arrays. |
| 13. Structured records + provenance | KEPT | goal.md row 14; requirements.md §10 "Results and provenance", §11 | Native `Value::Record` results with provenance; no display string is parsed. |
| 14. Language / CLI / desktop integration | KEPT | goal.md row 13, D6; requirements.md §11 (adapter, `--functions`, `--function`, desktop parity API-004), §12 | `sys_*` family registered statically in `lovelace-lang` and visible through CLI discovery and the workbench snapshots. |
| 15. Protocol version bump + migration | KEPT | goal.md row 15, D3; requirements.md §12 "Wire and protocol migration (REM-005)" | `plugin` field removed, `module` added, `PROTOCOL_VERSION` = `"2"`, migration documented, goldens regenerated. |
| 16. No replacement plugin framework | KEPT | goal.md hard constraints ("no replacement plugin framework, DLL/SO/WASM/IPC/package manager"); requirements.md §13 last bullet | No trait-object registry, no `Box<dyn Fn>` table, no runtime registration. |
| 17. No floating point | KEPT | goal.md OUT-005, constraint row, D4/D9; requirements.md §1, §3 (NUM-001..NUM-006) | Exact `Q` rationals only; no `f32`/`f64` in crate or adapter. |
| 18. No external engine dependency | KEPT | goal.md hard constraints (ADR-001 + OUT-005); requirements.md §1, §3 | Crate depends on `lovelace-nat`/`int`/`real` only; enforced by `cargo tree`. |
| 19. No `unsafe` | KEPT | goal.md constraint row, D4; requirements.md §1 (`#![forbid(unsafe_code)]`) | `#![forbid(unsafe_code)]` in the systems crate. |
| 20. P1: multiclass MVA, M/G/1, G/G/1, Amdahl, fitting | KEPT AS FOLLOW-ON | goal.md "Named mechanisms" row "P1: ... | DEFERRED (user P1) | follow-on" | Out of P0 by the user's own priority list; listed as follow-on work, not removed. |
| 21. P2: simulation, fork-join, mixed networks, GUI editor | KEPT AS FOLLOW-ON | goal.md "Named mechanisms" row "P2: ... | DEFERRED (user P2) | follow-on" | Out of P0 by the user's own priority list; listed as follow-on work, not removed. |

KEPT 19, KEPT AS FOLLOW-ON 2 (P1 row 20, P2 row 21), REPLACED 0, REMOVED 0.

Question: NONE

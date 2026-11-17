# Modus / plugin extension surface — census at `619df33`

Command, run from `F:\dev\RustLace` at `619df33a4934cadd212800fff99bce7edd7b2d32`; the working tree and the
commit give the same result (33 files, 293 hits):

```text
git grep -n -i -E 'modus|new_bare|load_plugin|enabled_plugins|plugin_name_of|plugin_enabled|builtin_count_for_plugins' -- . ':(exclude)docs/goal-docs-systems' ':(exclude)target'
```

`Hits` = matching lines, so `git grep -c` and `git grep -n` agree. Categories are the seven named in the
round brief; `docs/internal/archive/` and `docs/goal-docs-v2/` are `archived-doc`.
`Must change in PR A?` follows the `PR A` row of `docs/goal-docs-systems/goal.md` (delete the Modus mechanism, one
static registry, remove `functions[].plugin`): `yes` when the line names a Rust symbol, registry field or
envelope field that row removes, `no` when the line is C#-reference or archive history it leaves standing.

## 1. Summary

| Category | Files | Hits |
|---|---:|---:|
| production-code | 4 | 71 |
| test-code | 5 | 71 |
| active-doc | 15 | 79 |
| archived-doc | 6 | 27 |
| tooling | 1 | 1 |
| example | 1 | 17 |
| skill | 1 | 27 |

## 2. Files

| Path | Hits | Category | What the hits are (symbols/field names) | Must change in PR A? |
|---|---:|---|---|---|
| `lovelace-lang/src/lib.rs` | 51 | production-code | `pub mod modus;` (27); `modus: modus::ModusHost` field (812); `enabled_plugins` (815, 865, 877, 885); `new_bare` (875); `load_plugin` (883); `plugin_name_of` (912, 937, 982-984, 1358); `plugin_enabled` (990, 1226, 2146, 2148); `mod round67_modus;` / `mod round69_parameter_domain;` (2793-2801) | yes |
| `lovelace-lang/src/modus.rs` | 18 | production-code | The seam module itself: `ModusHost` (656, 664, 670, 672), `load` (686), `plugin_name_of` (706), `pub trait ModusPlugin` + `register` (927-932), doc-comment `Modus` (1, 4, 11, 287, 390, 651, 655) | yes |
| `lovelace-lang/src/registry.rs` | 1 | production-code | `pub fn builtin_count_for_plugins(plugins: &[&str]) -> usize` (38), filtering `BUILTINS` on `info.plugin` | yes |
| `lovelace-desktop/src/engine.rs` | 1 | production-code | Doc comment on `DEFAULT_PRECISION` citing the C# host: `Lovelace.Suite/ModusHost.cs:50,53` (48) | no |
| `lovelace-lang/src/round19_batch.rs` | 1 | test-code | Module doc listing rows with no Rust analogue, among them "a Modus plugin registration" (6) | no |
| `lovelace-lang/src/round67_modus.rs` | 55 | test-code | `ModusPlugin` impls `StatisticsPlugin` (67), `ScalarResultPlugin` (140), `DspPlugin` (165), `ArityPlugin` (250), `EnumEchoPlugin` (459), `RecordPlugin` (500), `SymbolicsContextPlugin` (596); `new_bare` (180, 192); `load_plugin` (83-656); `plugin_name_of` (200, 201) | yes |
| `lovelace-lang/src/round69_parameter_domain.rs` | 12 | test-code | `use crate::modus::{ModusHost, ModusPlugin, Payload, ScalarResult, ...}` (10); `impl ModusPlugin for ParameterDomainPlugin` (17); `register(&mut ModusHost)` (22); the two `modus_seam_*` tests (34, 54) | yes |
| `lovelace-lang/src/round95_engine_singletons.rs` | 2 | test-code | `crate::modus::BuiltinDescriptor::new` (315) and `crate::modus::Payload::Natural` (326) in `arityproperty_a_host_descriptor_carries_its_declaration` | yes |
| `lovelace-lang/src/round97_tail.rs` | 1 | test-code | `crate::registry::builtin_count_for_plugins(&["Lovelace.Symbolics", "Lovelace.MathIR"])` (207) | yes |
| `AGENTS.md` | 3 | active-doc | Crate table "Modus plugin host" (154); "a Modus plugin" row (221); skills index row (294) | yes |
| `CONTRACT.md` | 2 | active-doc | Crate table "Modus" (31); `pub fn new_bare() -> Engine;` in the published API block (80) | yes |
| `README.md` | 3 | active-doc | Crate table "Modus" (147); extending row (200); glossary row (204) | yes |
| `docs/README.md` | 1 | active-doc | `extending.md` index row, "add a Modus plugin" (16) | yes |
| `docs/architecture.md` | 2 | active-doc | Mermaid crate label "Modus" (10); "the Modus plugin host" (62) | yes |
| `docs/crates/lovelace-lang.md` | 7 | active-doc | "Modus" in the crate summary (3); `new_bare` (18, 83-84); `registry::builtin_count_for_plugins` (27); `modus` module list (28); `load_plugin` (105) | yes |
| `docs/extending.md` | 6 | active-doc | Section 2 "Add a Modus plugin": `ModusPlugin`/`ModusHost` (33-34); round67 model (36); `new_bare`+`load_plugin` (39); `builtin_count_for_plugins` (42) | yes |
| `docs/extensions/dsp.md` | 10 | active-doc | `load_plugin` line reference (72, 139); `enabled_plugins`/`plugin_enabled` gate prose (94-95); `new_bare` + `enabled_plugins = Some(empty)` (100); `ModusPlugin`/`ModusHost` example (125-135) | yes |
| `docs/extensions/mathir.md` | 9 | active-doc | `load_plugin` line reference (40); `enabled_plugins` gate prose (60, 64); `ModusPlugin` re-enable example (78-93) | yes |
| `docs/extensions/README.md` | 19 | active-doc | `builtin_count_for_plugins` (17); `plugin_enabled` signature + `enabled_plugins` (50-56, 67); `new_bare`/`load_plugin` table rows and transcript headings (70-71, 102-118); `ModusPlugin` trait block (77-88); "no production `ModusPlugin` implementation" (130-131, 180) | yes |
| `docs/extensions/symbolics.md` | 11 | active-doc | `load_plugin`/`modus.rs` line references (39-40); `round67_modus` 22-test row (58); `enabled_plugins`/`plugin_enabled`/`new_bare` gate prose (69-73); `ModusPlugin` example (92-103) | yes |
| `docs/glossary.md` | 2 | active-doc | "Modus / ModusHost" and "ModusPlugin" glossary rows (15-16) | yes |
| `docs/language/builtins.md` | 2 | active-doc | Two `Engine::new_bare()` core-only statements (69, 487) | yes |
| `docs/parity-with-studio.md` | 1 | active-doc | Plugin-model row `ModusHost` → `modus::ModusHost` (14) | yes |
| `docs/skills.md` | 1 | active-doc | `add-modus-plugin` skill row (11) | yes |
| `docs/goal-docs-v2/round-5/registry-and-audit.md` | 4 | archived-doc | `add-modus-plugin/SKILL.md` heading (244); `round67_modus` filter row "22 passed, 0 failed" (248); truncated `--functions` notes (357-358) | no |
| `docs/internal/archive/goal-docs-v1/evidence.md` | 1 | archived-doc | EVD-043 records the `load_plugin` line correction, 871 to 883 (48) | no |
| `docs/internal/archive/goal-docs-v1/round-6/implementation.md` | 10 | archived-doc | Line-suffix citation audit rows for `modus.rs`/`round67_modus.rs` (274-332); "`new_bare` x8" tally (346); diff-stat rows (630, 924, 959) | no |
| `docs/internal/archive/goal-docs-v1/round-7/prose-repair.md` | 4 | archived-doc | Symbol list including `modus` and `ModusPlugin` (91-92); `ModusPlugin`/`load_plugin` locator removals (99, 101) | no |
| `docs/internal/archive/goal-docs-v1/round-7/reference-repair.md` | 5 | archived-doc | `load_plugin` line corrections (34, 49-50); `Engine::new`/`new_bare`/`load_plugin` count row (40); `plugin_enabled` gate (72) | no |
| `docs/internal/archive/goal-docs-v1/round-8/reference-pages.md` | 3 | archived-doc | `new_bare` as a derivation source (38, 49); `modus.rs` error-message citations (39) | no |
| `.github/copilot-instructions.md` | 1 | tooling | File-map row naming `lovelace-lang/src/modus.rs` as the plugin seam (65) | yes |
| `lovelace-lang/examples/bare_engine.rs` | 17 | example | `ModusPlugin` impl (8-20); `new_bare()` per family (44-76); `load_plugin` including the duplicate-load refusal (55-60); `plugin_enabled` comment (11) | yes |
| `.dsh/skills/add-modus-plugin/SKILL.md` | 27 | skill | Frontmatter name/description/whenToUse (2-4); reading list (`modus.rs`, `new_bare`, `load_plugin`, `plugin_name_of`, round67) (9-24); procedure using `new_bare`/`load_plugin`/`plugin_name_of`/`enabled_plugins` (28-85); success criteria (95-102) | yes |

TOTAL: 33 files, 293 hits

## Field counts outside the hit pattern

The pattern matches none of the bare `plugin` field/string occurrences, so these were counted separately.

| Item | Count | Where |
|---|---:|---|
| `plugin: Some(` in `lovelace-lang/src/registry.rs` | 80 | `BuiltinInfo` literals |
| `plugin: None` in `lovelace-lang/src/registry.rs` | 44 | `BuiltinInfo` literals |
| `plugin:` lines in `lovelace-lang/src/registry.rs` | 125 | the 124 literals plus `pub plugin: Option<&'static str>` (20) |
| `"plugin"` JSON field in `lovelace-run/` | 1 | `lovelace-run/src/round50_resweep.rs:181`, a test asserting `get(descriptor, "plugin")` is `"Lovelace.Symbolics"` |
| `"plugin"` JSON field in `lovelace-desktop/` | 3 | `lovelace-desktop/src/engine.rs:180` (production match arm `FunctionOrigin::Plugin => "plugin"`), `lovelace-desktop/src/engine.rs:942` (test), `lovelace-desktop/src/workspace_table.rs:921` (test) |

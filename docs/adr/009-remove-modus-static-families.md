# ADR-009: delete the Modus extension host; families are static, the wire is v2

**Status:** accepted (2026-11-17)

## Context

`lovelace-lang` carried a dynamic extension host (a 37 KB source file, deleted in this change): a
`ModusPlugin` trait, a `ModusHost` that stored host-registered builtins, a `Payload`/`PluginArgs`
value channel parallel to
`Value`, `FieldKernel` dispatch, an enabled-plugin set consulted by `Engine::call`, a plugin
assumption scope applied for one evaluation, and a bare-engine constructor. The port had no
production `ModusPlugin` implementation: every implementation lived in a `#[cfg(test)]` module, and
the three shipped families (DSP, Symbolics, MathIR) were static registry descriptors that the host
merely *gated* by name. The mechanism was therefore unused for its advertised purpose while adding
a second value system, a second dispatch path, a second precision scope and a second assumption
scope.

## Decision

1. **Delete the host completely.** `modus.rs`, `ModusHost`, `ModusPlugin`, `Payload`, `PluginArgs`,
   `ScalarResult`, `FieldKernel`, `KernelScalar`, `Engine::load_plugin`, `Engine::new_bare`,
   `Engine::register_*`, `Engine::registered_descriptor`, `Engine::plugin_name_of`,
   `Engine::plugin_enabled`, `Engine::register_assumption_context`, the dynamic dispatch branch and
   the enabled-set gate are removed. No replacement trait-object registry, `Box<dyn Fn>` table or
   runtime registration is introduced (OUT-002, OUT-004).
2. **Keep one static registry.** Every `registry::BuiltinInfo` is available on `Engine::new()`. The
   former `plugin: Option<&'static str>` becomes `module: &'static str` (`Core`, `DSP`,
   `Symbolics`, `MathIR`, `Systems`): a browsing group for help/discovery, never consulted by the
   dispatcher. `StateFunction.plugin` becomes `StateFunction.module`; user definitions carry
   `"User"`.
3. **Bump the wire and document the migration.** `protocolVersion` becomes `2`;
   `functions[].plugin` is removed and `functions[].module` is added. Every other envelope field and
   key is unchanged. `CONTRACT.md` and `docs/symbolics/dsh-protocol.md` state the migration, and
   the twenty golden envelopes are re-recorded with `LOVELACE_RECORD_GOLDENS=1` through the
   documented recorder test.
4. **Preserve independent behaviour.** DSP's 30-place fast default stays (ADR-004), the
   `assume`/`assumptions` session state and the typed `context::Context` API stay, and the general
   guarantees the plugin tests incidentally touched (records, enums, argument refusals, discovery,
   cancellation, assumption isolation) remain pinned by core tests. The deletion ledger is
   [the harness record](../goal-docs-systems/deleted-tests.md).
5. **Family extension path.** Adding a family means compiling a workspace crate and a thin adapter
   into the tree (see `docs/extending.md` §2), not registering at run time.

## Consequences

- The language has one value system, one dispatch table, one precision scope and one assumption
  store.
- `--functions` still reports 124 builtins; the DSP, Symbolics and MathIR functions are reachable in
  every engine, and `--function <name>` now prints `Module: <family>` instead of `Plugin: <name>`.
- The twenty golden envelopes and the REPL help transcript change (protocol 2); these are stated
  contract moves, not weakened assertions.
- 25 plugin-seam tests are deleted with their subject; the independent invariants they touched are
  covered without a plugin, and the baseline suite stays green.
- A host can no longer add a builtin at run time. That is the point: a capability is auditable in
  the tree rather than in a caller.

## Alternatives considered

- **Keep the host disabled** (leave the code, never load): rejected — OUT-002 requires deletion, and
  dead dispatch still advertises the capability.
- **Feature-gate the host behind a Cargo feature**: rejected — a feature is a disguised replacement
  framework and adds a build matrix for code with no production user.
- **WASM/IPC plugins**: rejected by OUT-004 and by ADR-001 (no external engine dependency).
- **Keep `plugin` as a label and skip the version bump**: rejected — REM-005 forbids silently
  changing v1, and the label preserves the deleted vocabulary.

# Tests deleted with the Modus plugin mechanism

This file records every test removed by PR A, the independent guarantee (if any) it also carried,
and where that guarantee is still tested. No independent invariant was dropped: the only deleted
assertions are the ones whose subject was the extension host itself, which no longer exists.

Baseline: `cargo test -p lovelace-lang` reported 1750 passed before the removal and 1725 after —
exactly the 25 plugin-seam tests below. `cargo test -p lovelace-run` moves from 438 to 439 because
the on-demand golden recorder test is added.

## `lovelace-lang/src/round67_modus.rs` — deleted (22 tests)

Every test in this module drove the removed surface: `ModusHost` registration, the
`Payload`/`PluginArgs` channel, load-time refusals, `FieldKernel` dispatch, descriptor-carried
arity and the plugin-instance assumption scope. The general guarantees they incidentally touched
are covered without a plugin by existing core tests:

| Guarantee | Still tested by |
|---|---|
| records and enums are first-class values | `round78_structural_equality`, `round84_evalir_multi`, `suite_engine_batch` |
| argument refusals carry code/category/signature | `arity_taxonomy_round73`, `argument_contract` |
| function discovery/help/autocomplete agree | `help` tests, `docs_sync`, `name_suggestions` |
| cancellation is observed and published | `cancellation_round18`, `kernel_cancel_round9` |
| engine-local `assume`/`assumptions` isolation | `assumptions_round35`, `round78_scoped_eval`, `context_round56` |

## `lovelace-lang/src/round69_parameter_domain.rs` — 2 tests deleted, 1 migrated

- Deleted `modus_seam_maps_every_declared_arm_to_the_integer_domain_payload` and
  `modus_seam_refuses_an_undeclared_parameter_domain`: both exist only to cross the payload
  channel. The mapper itself, `parameter_domain_payload_of_ordinal`, lived in `modus.rs` and is
  deleted; the independent `ParameterDomain::from_ordinal` refusal is tested by
  `lovelace-symbolics`' own parameter-domain tests.
- Migrated `solve_full_family_parameter_domain_is_the_projected_integer_domain` unchanged (minus
  the Modus imports) into `lovelace-lang/src/parameter_domain_family.rs`. It asserts that
  `solve_full(...).families[0].parameter_domain` is `Domain("integer")`, which has nothing to do
  with the extension host.

## `lovelace-lang/src/round95_engine_singletons.rs` — 1 test deleted, 1 migrated

- Deleted `arityproperty_a_host_descriptor_carries_its_declaration`: it registered a host builtin
  through `Engine::register_builtin_descriptor`, which no longer exists.
- Migrated `arityproperty_declared_arity_travels_with_every_plugin_builtin` to
  `arityproperty_declared_arity_travels_with_every_builtin`: it now checks all 124 builtins
  (previously only the 80 plugin descriptors) and also asserts the `module` label travels.

## `lovelace-lang/examples/bare_engine.rs` — deleted

The example existed to demonstrate fake extension loading (`Engine::new_bare()` + `load_plugin`).
There is no bare engine and no loader to demonstrate; `Engine::new()` is the one ordinary engine.

## Unchanged independent tests

`ASSERT-*` tests for DSP's 30-digit default (`dsp_round20`, `dsp_round21`, runner
`dsp_plugin_untouched_knob_computes_at_fast_budget_and_promotes`) are kept and still pass. They
test a precision decision, not the extension host; only their prose was reworded.

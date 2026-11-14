# `lovelace-lang`

The Lovelace scripting language: values, lexer, parser, tree-walking interpreter, builtins, the Modus
plugin host, MathIR, complex arithmetic and SVG plotting.

## Purpose

"Rendering is the contract: every value renders with its documented display form and reports a kind,
and the engine reports errors as a message plus a UTF-16 position into the source"
(`lovelace-lang/src/lib.rs:1`). Everything above this crate — the runner, the corpus gate, the
knowledge graph, the desktop — talks to the engine through `Engine`, and nothing re-implements
language behaviour.

## Public surface

- `Engine` and its methods: `new` (all builtins), `new_bare` (core builtins plus explicitly loaded
  plugins), `eval`, `eval_with_output`, `eval_with_budget`, `set_precision`/`precision`,
  `set_display_precision`/`display_precision`, `set_plot_dir`, `set_cancel_after`/`set_cancel_flag`,
  `set_progress_sink`, `format`/`format_typed`, `variables`/`variable_values`, `functions`,
  `capture_state`, `take_output`, `last_plot`, `revision`, `set_variable`/`try_get_variable`.
- `Value` with `kind_name`, `display`, `typed`, `display_unicode`, `is_numeric`.
- `LangError { message, position, code, category, signature, suggestions, hints, registry_size }` and
  `ErrorSignature`.
- `registry` — the data table of builtin descriptors (`registry::BUILTINS`, `registry::find`,
  `registry::builtin_count_for_plugins`).
- `modus` — `ModusPlugin`, `ModusHost`, `Payload`, `BuiltinDescriptor`, `FieldKernel`,
  `KernelScalar`.
- `cancel` (re-export of `lovelace_symbolics::cancel`), `progress`, `plot_surface`, `mathir`,
  `complex`, `structured`, `help`, `diagnostic`.

## Dependencies

`lovelace-nat`, `lovelace-int`, `lovelace-real`, `lovelace-array`, `lovelace-json`,
`lovelace-symbolics`.

## Tests

1737 `#[test]` functions in `src/`, organised as parity rounds (`suite_parity_batch.rs`,
`roundNN_*.rs`), the falsification harness (`falsification_round46/`) and focused suites for
printing, cancellation, complex arithmetic, DSP, MathIR and the panic sweep.

## How to use it

```rust
use lovelace_lang::Engine;

let mut engine = Engine::new();
engine.set_precision(30);
let value = engine.eval("1 / 3").expect("evaluates");
assert_eq!(engine.format_typed(&value), "0.(3) (Real)");
```

The same engine backs the CLI and the desktop; a host that wants the reference's opt-in plugin model
starts from `Engine::new_bare()` and calls `load_plugin` (`lib.rs:871`).

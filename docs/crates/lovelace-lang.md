# `lovelace-lang`

The Lovelace scripting language: values, lexer, parser, tree-walking interpreter, builtins, the Modus
plugin host, MathIR, complex arithmetic and SVG plotting.

## Purpose

"Rendering is the contract: every value renders with its documented display form and reports a kind,
and the engine reports errors as a message plus a UTF-16 position into the source"
(`lovelace-lang/src/lib.rs`). Everything above this crate — the runner, the corpus gate, the
knowledge graph, the desktop — talks to the engine through `Engine`, and nothing re-implements
language behaviour.

## Inputs

### Public surface

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

### Dependencies

`lovelace-nat`, `lovelace-int`, `lovelace-real`, `lovelace-array`, `lovelace-json`,
`lovelace-symbolics`.

## Outputs

`eval` returns `Result<Value, LangError>`; `eval_with_budget` returns the same type under a time
budget, and `eval_with_output` writes the script's `print` output to a caller-supplied writer and
returns the value. `format` and `format_typed` return the rendered string with and without the kind.
`Value` answers `kind_name`, `display`, `typed`, `display_unicode` and `is_numeric`. The session
surface returns state: `variables` returns `(name, display)` pairs and `variable_values` the
`(name, Value)` pairs behind them, `functions` lists builtin names and parameter names,
`capture_state` returns a `StateSnapshot`, `take_output` drains the print buffer, `last_plot` returns
the last `PlotCapture`, and `revision` advances on every state change. A failed call returns a
`LangError` whose fields are the published wire payload.

## Precision behaviour

The engine has two bounds. The default computation precision is 1000 decimal places and the default
display precision is 100; `set_precision(n)` moves both and marks the precision explicit, while
`set_display_precision(n)` pins only the rendering bound. That separation matters: a host that asks
for 1 100 stored digits can still render 100, and the structured projection keeps publishing every
digit the value stores, so the two bounds cannot make one value cross as two different payloads. A
value is exact when its representation is exact (integers, terminating or recurring decimals) and
precision-bounded when it is computed to the requested places; an `inexact` marker is provenance, not
an error bound. Positions are UTF-16 code-unit offsets into the source text.

## Errors

Failures are `LangError`s built with `LangError::coded(message, position, code, category)`: the
message is a complete sentence ending with a period, `position` is a UTF-16 code-unit offset, and the
code is stable. The seven categories are frozen: `ParseError`, `DomainError`, `UnsupportedOperation`,
`BudgetExceeded`, `NoSolution`, `TypeMismatch`, `InternalInvariantFailure`. Argument refusals are
`InvalidArgument` / `TypeMismatch`, carry the declared signature, and are enforced at one call site
from the registry declaration. An unresolved name is `UnknownFunction` / `UnsupportedOperation` with
`suggestions`, `hints` and `registry_size` filled in. A cancellation or budget exhaustion is
`BudgetExceeded` with the structured `Cancelled` code, and the partial output and variables are still
published by the runner.

## Limitations

- **No external crates (ADR-001).** The engine and its dependencies are workspace-only; a feature
  that needs a crate registry cannot be added here.
- **It is a tree-walking interpreter.** There is no bytecode, JIT or native code generation; the
  interpreter evaluates the parsed tree directly.
- **One engine, one mutable driver.** Every evaluation method takes `&mut self`, so a host serialises
  access to an engine rather than sharing one across threads.
- **Cancellation is cooperative.** `set_cancel_after`/`set_cancel_flag` stop a run only where a
  kernel polls (`cancel::poll`/`check`); a deadline cannot interrupt arbitrary code.
- **Plugins are opt-in by name.** `Engine::new()` exposes all 124 builtins; `Engine::new_bare()`
  exposes core builtins only until a `ModusPlugin` whose `name()` matches a registry family is
  loaded, and that load enables the family's builtins — it does not bind any symbol.
- **Builtin-level caps remain.** `evalf` stops at 1000 decimal places and `pi`/`e` stop at the engine
  computation precision; those are refusals, not silent clamps.
- **Plots are files.** `plot` writes an SVG and returns its path, so the engine needs a writable plot
  directory (`set_plot_dir`); there is no in-memory plot value.

## Examples

### How to use it

```rust
use lovelace_lang::Engine;

let mut engine = Engine::new();
engine.set_precision(30);
let value = engine.eval("1 / 3").expect("evaluates");
assert_eq!(engine.format_typed(&value), "0.(3) (Real)");
```

The same engine backs the CLI and the desktop; a host that wants the reference's opt-in plugin model
starts from `Engine::new_bare()` and calls `load_plugin` (`lovelace-lang/src/lib.rs`).

### Tests

1737 `#[test]` functions in `src/`, organised as parity rounds (`suite_parity_batch.rs`,
`roundNN_*.rs`), the falsification harness (`falsification_round46/`) and focused suites for
printing, cancellation, complex arithmetic, DSP, MathIR and the panic sweep.

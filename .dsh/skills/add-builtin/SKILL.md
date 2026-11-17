---
name: add-builtin
description: Add a Lovelace language builtin end to end with a registry descriptor, an implementation arm, arity handling, documentation, a test and a parity fixture.
whenToUse: Use when adding or changing a callable builtin in lovelace-lang, or when a new function must appear in the registry, help, completion, the wire functions array or the corpus.
---

# Add a builtin

A builtin is three things that must agree: a registry descriptor, a dispatch arm, and a test whose
expected text is the documented rendering. The registry is the single source of arity and discovery,
so the descriptor is not optional metadata.

## Files you will touch

- `lovelace-lang/src/registry.rs` - `BuiltinInfo` and the `BUILTINS` table.
- `lovelace-lang/src/builtins.rs` - `pub fn call` dispatch and the implementation arm.
- `lovelace-lang/src/lib.rs` - only if the builtin needs a new helper on `Engine` or a new module.
- `lovelace-lang/src/suite_parity_batch.rs` or `lovelace-lang/src/arity_taxonomy_round73.rs` - where
  the focused and property tests live.
- `parity/corpus/lang.json` (or `parity/corpus/symbolics.json` for a symbolic builtin) - the parity
  fixture.
- `docs/crates/lovelace-lang.md` - the public-surface page.

## Procedure

1. **Read the table before writing.** Open `lovelace-lang/src/registry.rs` and read several
   `BuiltinInfo` entries. Note the fields: `name`, `parameters`, `min_arity`, `parameter_count`,
   `variadic`, `plugin`, `category`, `summary`, `return_kind`, `related`, `examples`. `BUILTINS` is
   kept in name order; `find` and `matching` rely on that order.

2. **Add the descriptor in name order.** For a core builtin use `plugin: None`. For a builtin that
   belongs to an advertised plugin use `plugin: Some("Lovelace.Symbolics")` (or `Lovelace.Dsp`,
   `Lovelace.MathIR`) and keep the implementation reachable in `Engine::new()` as the DSP builtins
   are. Declare the real parameter names and the exact or minimum arity; do not declare a wider
   range than the body accepts.

3. **Implement the arm.** In `lovelace-lang/src/builtins.rs`, add an arm to the `match name` in
   `pub fn call`. Read the neighbouring arms for the argument-kind conventions. Call `arity(n)?`
   when the algorithm only makes sense at exactly `n` arguments; the call-site guard already
   refuses counts outside the declared range, so a second contradictory check is a bug. Validate
   argument kinds explicitly and return `Value`.

4. **Refuse in the house style.** Use
   `LangError::coded(message, position, "InvalidArgument", "TypeMismatch")` for a wrong argument
   shape, or the code and category that actually applies. The message is a complete sentence
   ending with a period; `position` is a UTF-16 code-unit offset. Do not add an error category: the
   seven in `lovelace-lang/src/diagnostic.rs` are frozen. If the argument is a list, reuse
   `argument_list` and `shape_failure` rather than an ad-hoc message.

5. **Check the arity contract.** `builtins::call` enforces `[min_arity, parameter_count]` before
   the arm runs, and removes the upper bound when `variadic` is true. The property test
   `every_builtin_refuses_every_count_outside_its_declared_arity` sweeps every registry entry, so a
   descriptor with the wrong bounds fails the suite even if the happy path passes.

6. **Test at the public surface.** Add a `#[test]` to `lovelace-lang/src/suite_parity_batch.rs` or a
   new round module. Use `Engine::new()`, evaluate a concrete source string and assert the exact
   typed rendering:

   ```rust
   let mut engine = Engine::new();
   let value = engine.eval("my_builtin(2, 3)").expect("evaluates");
   assert_eq!(engine.format_typed(&value), "6 (Natural)");
   ```

   Add a refusal test that asserts the exact message, code and category. If the test goes in a new
   file, declare it as a `mod` in `lovelace-lang/src/lib.rs` (test modules are declared there, not
   auto-discovered).

7. **Add the parity fixture.** Append a record to `parity/corpus/lang.json` (or
   `parity/corpus/symbolics.json`) with the next sequential ID - `lang/112` is the next free ID
   after the current `lang/111` - and the fields `id`, `doc`, `heading`, `script`, `kind`, `text`.
   `kind` is `value`, `error`, `prints` or `plot`; `text` is the exact typed rendering the engine
   must produce. The corpus is a gate, so a wrong `text` is a failure, not a snapshot update.

8. **Check ownership and cross-library semantics.** A builtin that exposes a numeric, symbolic or
   systems value is a consumer of the canonical kernel, not a place to add a numeric type. Read
   `docs/architecture-ownership.md`; run `python tools/check-architecture.py` and
   `cargo test -p lovelace-lang --test cross_library`. If the builtin returns an exact systems value,
   keep the decimal field as a projection and expose the exact numerator/denominator through the
   additive `exact_values` record. If the builtin seems to need a new arithmetic type, stop and
   improve the owner (RNL-ARC-001); do not add a second kernel.

9. **Update the prose.** If the public surface changed, update `docs/crates/lovelace-lang.md`. If
   the builtin introduces a term, add it to `docs/glossary.md`. If it is wire-visible, add its code
   to the taxonomy test that publishes the code.

10. **Prove discovery.** The same descriptor feeds `--functions`, `--function`, the help dialog and
    completion. If the descriptor is not in `BUILTINS`, the builtin is invisible to every host even
    when the evaluator can call it.

## Verify

`<name>` below is the builtin you added; X and Y are its arguments. Substitute real values before
running, and read each `test result:` line: a filter that matches nothing prints `running 0 tests`
and still exits 0, which is a failed check rather than a pass.

```console
cargo test -p lovelace-lang <name>
cargo test -p lovelace-lang every_builtin_refuses_every_count_outside_its_declared_arity
cargo test -p lovelace-lang --test cross_library
python tools/check-architecture.py
cargo run -p lovelace-run -- --function <name>
cargo run -p lovelace-run -- --eval "<name>(X, Y)" --text
cargo run --release -p lovelace-corpus -- lang/112
```

Observable success: the focused test names the builtin and reports at least one passed test with
`0 failed`; the arity property test reports `1 passed` (it sweeps every registry entry); `--function`
prints the descriptor JSON with `parameters`, `minArity`, `parameterCount`, `summary` and, for a
plugin builtin, the plugin name; `--eval` prints the exact typed value you chose, for example
`6 (Natural)`; the corpus filter ends with `corpus: 1 passed, 0 failed, 231 total (filter lang/112)`
and exit 0 — a filter that matches no record prints `0 passed` and also exits 0, so the count is the
check.

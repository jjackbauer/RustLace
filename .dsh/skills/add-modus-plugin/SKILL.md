---
name: add-modus-plugin
description: Add a Modus plugin with its registration, capability descriptor, tests, and an explicit answer to whether the CLI or desktop can see it.
whenToUse: Use when adding an opt-in extension package to the Lovelace engine through the Modus host, or when deciding why a plugin builtin is or is not visible to lovelace-run or lovelace-desktop.
---

# Add a Modus plugin

Modus is the opt-in plugin seam. A plugin is a `ModusPlugin` value with a unique `name()` and a
`register()` method that installs builtins, descriptors or elementwise kernels into the host.
Loading a plugin is a host decision, not a language feature.

## Files you will touch

- `lovelace-lang/src/modus.rs` - the `ModusPlugin` trait, `ModusHost` registration methods and
  `BuiltinDescriptor`.
- `lovelace-lang/src/lib.rs` - `Engine::new_bare`, `Engine::load_plugin`, `Engine::plugin_name_of`
  and the function-state projection.
- `lovelace-lang/src/round67_modus.rs` - the complete in-tree plugin examples (`StatisticsPlugin`,
  `DspPlugin`, `ArityPlugin`, `RecordPlugin`, `SymbolicsContextPlugin`).
- `lovelace-lang/src/round69_parameter_domain.rs` - the parameter-domain plugin example.
- `lovelace-desktop/src/engine/real.rs` - the desktop's `RealEngine::new`, the place a host would
  load a plugin if the desktop must see it.
- `docs/crates/lovelace-lang.md` and `docs/extending.md` - the Modus prose and walkthrough.

## Procedure

1. **Read the trait and one example.** `ModusPlugin` requires `fn name(&self) -> &str` and
   `fn register(&mut self, context: &mut ModusHost) -> Result<(), LangError>`. Read
   `StatisticsPlugin` in `lovelace-lang/src/round67_modus.rs` for the smallest kernel example and
   `ArityPlugin` for descriptor registration. Copy the shape, not a remembered API.

2. **Choose the registration channel.** `ModusHost` exposes:
   - `register_builtin(name, parameters, implementation)` for an exact-arity builtin;
   - `register_builtin_scalar` for the typed `ScalarResult` return channel;
   - `register_builtin_descriptor(descriptor, implementation)` when the builtin must carry
     discoverability metadata;
   - `register_builtin_descriptor_scalar` for both;
   - `register_array_builtin` for the array-to-array convention;
   - `register_kernel::<T>(Box::new(kernel))` for exact-field elementwise dispatch.

3. **Declare capability with a descriptor.** Build
   `BuiltinDescriptor::new(name, parameters, category, summary, return_kind)` and use
   `.with_min_arity(n)` for an optional trailing parameter or `.with_variadic(true)` when the last
   parameter repeats. The descriptor is the advertisement: it feeds the host's function state and
   the help surface. A name that already exists in `registry::BUILTINS` or in the host is refused
   with "already registered"; plugins cannot shadow.

4. **Put the implementation where a host can reach it.** There is no production plugin directory in
   this tree; every current `impl ModusPlugin` lives in a test module. For a real plugin, add a
   module beside `lovelace-lang/src/modus.rs` and declare it with `mod` in
   `lovelace-lang/src/lib.rs`, or implement it in the host crate that will load it. Keep the
   implementation out of the registry unless it is meant to be advertised by the static registry.

5. **Load it explicitly in tests.** Create a bare engine, load the plugin and prove both the
   opt-in and the call:

   ```rust
   let mut engine = Engine::new_bare();
   engine.load_plugin(&mut MyPlugin).expect("load");
   assert_eq!(engine.eval("my_plugin(2)").unwrap().typed(), "4 (Natural)");
   assert_eq!(engine.plugin_name_of("my_plugin"), Some("MyPlugin"));
   ```

   `load_plugin` records the plugin name before `register` runs, so a failed registration leaves the
   plugin marked loaded; a second load of the same name is refused. Test both refusals.

6. **State the visibility truth.** `Engine::new()` advertises every static descriptor
   (`enabled_plugins == None`) but does not load any runtime plugin. `Engine::new_bare()` hides
   descriptor-advertised names until the plugin is loaded. `lovelace-run` and the desktop's
   `RealEngine::new()` both construct `Engine::new()` and load no custom plugin. Therefore:
   - a builtin implemented in `lovelace-lang/src/builtins.rs` with a static registry descriptor is
     visible to the CLI and the desktop, including `--functions`;
   - a builtin registered only through `ModusHost` is visible only where the host calls
     `load_plugin`; it appears in `capture_state`/the workspace function table after loading, but
     not in `--functions` or the help catalog;
   - to make a runtime plugin visible in the desktop, load it in `RealEngine::new()` and test that
     path. To make it visible in the CLI, load it in `lovelace-run` or give it a static descriptor.

7. **Test the seam.** Model the tests on `lovelace-lang/src/round67_modus.rs`: registration, one
   successful call with an exact typed value, `plugin_name_of`, duplicate-load refusal, core-name
   collision refusal, arity refusal through the descriptor, and kernel dispatch if you registered a
   kernel. Use `Engine::eval` and assert the exact message or typed string.

8. **Document the decision.** Update `docs/crates/lovelace-lang.md` (Modus section) and the plugin
   walkthrough in `docs/extending.md`. If the plugin is visible in the desktop, also update
   `docs/crates/lovelace-desktop.md`.

## Verify

`<name>` is the name you registered and `<the test you added>` is your new test. Read each
`test result:` line: a filter that matches nothing prints `running 0 tests` and still exits 0.

```console
cargo test -p lovelace-lang round67_modus
cargo test -p lovelace-lang <the test you added>
cargo test -p lovelace-desktop                      # on Linux: xvfb-run -a -s "-screen 0 1600x1000x24" …
cargo run -p lovelace-run -- --function <name>
cargo run -p lovelace-run -- --functions --function-limit 500
```

Observable success: the plugin tests pass; `plugin_name_of` returns the plugin name; the duplicate
load refusal names "already loaded"; the collision refusal names "already registered"; the desktop
suite still passes; `--function <name>` prints the descriptor only when a static registry descriptor
publishes it, and `--functions --function-limit 500` either lists the name (static descriptor) or
does not (runtime plugin), matching the design decision recorded in step 6. A bare `--functions`
listing is truncated by default — today it returns 50 of 124 with `"truncated":true` — so do not read
an absent name there as proof that a static descriptor is missing.

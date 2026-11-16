# Extensions

RustLace ships three extension families on top of a 44-builtin core. This directory documents each
one: what it adds, how it is enabled, every builtin it contributes with the result it actually
returns, worked examples, and the limits that are specific to it.

| Extension | Builtins | Builtin categories it spans | Page |
| --- | --- | --- | --- |
| DSP | 16 | `DSP` | [dsp.md](dsp.md) |
| Symbolics | 59 | `Symbolics` 40, `Calculus` 10, `Solving` 4, `Optimization` 2, `Introspection` 2, `Numerics` 1 | [symbolics.md](symbolics.md) |
| Compilation (MathIR) | 5 | `Compilation` | [mathir.md](mathir.md) |
| **Core** (not an extension) | 44 | `Arrays` 23, `Numerics` 9, `Linear Algebra` 8, `Introspection` 3, `Language` 1 | — |

`44 + 16 + 59 + 5 = 124`, which is exactly what `lovelace-run --functions` reports (`total: 124`)
and what the execution envelope's `functions` array contains. The registry's own doc comment pins two
of those numbers independently: a Symbolics + MathIR engine publishes 108, a runner with DSP
publishes 124 (`builtin_count_for_plugins`).

## Which builtins belong to which extension

The family key is the **`plugin` field**, not `category`:

```rust
// lovelace-lang/src/registry.rs (the plugin field)
/// The plugin that registered it, when it is not core.
pub plugin: Option<&'static str>,
```

| Family | `plugin` value | Count | Tag lines in `registry.rs` |
| --- | --- | --- | --- |
| Symbolics | `"Lovelace.Symbolics"` | 59 | first 69, last 1678 |
| DSP | `"Lovelace.Dsp"` | 16 | first 408, last 1611 |
| Compilation | `"Lovelace.MathIR"` | 5 | first 352, last 1055 |
| Core | `None` | 44 | 56 … 1732 |

Two counting traps, both verified:

- **`category` is a browsing label, not the family.** Symbolics spans six categories; `diff` is
  `Calculus` and `optimize` is `Optimization` yet both are `Lovelace.Symbolics`. DSP (16) and MathIR
  (5) happen to be one-category families, which makes the trap easy to miss.
- **The `--functions` listing does not print `plugin`.** Its JSON objects carry `name`, `category`,
  `minArity`, `parameterCount`, `parameters`, `variadic` — and nothing else, so family membership
  cannot be reconstructed from that listing. The *execution* envelope (`--eval ... --json`) does
  include `plugin`, `summary`, `returnKind`, `related` and `examples` per function. The counts above
  were taken from the envelope and cross-checked against the source.

## The plugin model

An extension is a **named set of registry descriptors**, gated at the call site by
`Engine::plugin_enabled`:

```rust
// lovelace-lang/src/lib.rs (the plugin gate)
fn plugin_enabled(&self, name: &str) -> bool {
    let Some(enabled) = &self.enabled_plugins else {
        return true;
    };
    match crate::registry::find(name).and_then(|info| info.plugin) {
        Some(plugin) => enabled.contains(plugin),
        None => true,
    }
}
```

Three states follow from that:

| Engine | `enabled_plugins` | Callable builtins |
| --- | --- | --- |
| `Engine::new()` (`Engine::new`) | `None` | all 124 — core plus every family |
| `Engine::new_bare()` (`Engine::new_bare`) | `Some(empty)` | 44 core only |
| `new_bare()` + `load_plugin` (`load_plugin`) | `Some({name})` | core plus the descriptors tagged with that name |

`Engine::default()` is `Engine::new()`, and `lovelace-run` always uses it: the CLI's usage block has
no plugin flag, so **every CLI run has all 124 builtins available** and there is no CLI way to see a
bare engine.

The `ModusPlugin` trait (declared in `lovelace-lang/src/modus.rs`) is the dynamic seam for a host
(including a test) that wants to add builtins of its own:

```rust
pub trait ModusPlugin {
    fn name(&self) -> &str;
    fn register(&mut self, context: &mut ModusHost) -> Result<(), LangError>;
}
```

A `load_plugin` call does two things: it runs `register` (which may add builtins and kernels,
inside `load_plugin`), and it inserts `plugin.name()` into `enabled_plugins`. Because the gate compares
that string against the *registry descriptor's* tag, a plugin whose name is `"Lovelace.Dsp"` makes
the engine's own 16 DSP builtins callable **without registering any of them**. The demo
[`bare_engine.rs`](../../lovelace-lang/examples/bare_engine.rs) does exactly that for all three families:

```text
-- Engine::new() --
new        fft([0, 1, 0, 0])        => [1, -1i, -1, 1i]
new        det([[1, 2], [3, 4]])    => -2
new        simplify(x/x)            => [InvalidOperation] Undefined variable 'x'.
new        symbol("x")              => x
new        lower(1, [])             => #!mathir 2
const (rat 1 1)
Constant 0 0
-- Engine::new_bare() --
bare       abs(-3)                  => 3
bare       det([[1, 2], [3, 4]])    => -2
bare       fft([0, 1, 0, 0])        => [UnknownFunction] Unknown function 'fft'.
bare       symbol("x")              => [UnknownFunction] Unknown function 'symbol'.
bare       lower(1, [])             => [UnknownFunction] Unknown function 'lower'.
-- new_bare() + load_plugin(EnableFamily("Lovelace.Dsp")) --
bare       fft([0, 1, 0, 0])        => [UnknownFunction] Unknown function 'fft'.
bare+dsp   fft([0, 1, 0, 0])        => [1, -1i, -1, 1i]
bare+dsp   det([[1, 2], [3, 4]])    => -2
bare+dsp   symbol("x")              => [UnknownFunction] Unknown function 'symbol'.
second load: [InvalidOperation] Plugin 'Lovelace.Dsp' is already loaded.
-- new_bare() + load_plugin(EnableFamily("Lovelace.Symbolics")) --
bare       symbol("x")              => [UnknownFunction] Unknown function 'symbol'.
bare+sym   symbol("x")              => x
bare+sym   diff(x^2, x)             => [InvalidOperation] Undefined variable 'x'.
-- new_bare() + load_plugin(EnableFamily("Lovelace.MathIR")) --
bare       lower(1, [])             => [UnknownFunction] Unknown function 'lower'.
bare+ir    lower(1, [])             => #!mathir 2
const (rat 1 1)
Constant 0 0
```

Three things to read out of that transcript: core builtins (`abs`, `det`) never depend on a family;
an unloaded family fails as `UnknownFunction` / `Unknown function '<name>'.` (category
`UnsupportedOperation`, `unknown_function_among`), not with a plugin-specific error; and enabling a family
does not bind symbols or pre-evaluate anything (`diff(x^2, x)` is still an undefined variable).

**There is no production `ModusPlugin` implementation in this tree.** Every `impl ModusPlugin` lives
in a `#[cfg(test)]` module — `round67_modus.rs` and `round69_parameter_domain.rs` (both declared in
`lovelace-lang/src/lib.rs`). The shipped families are registry declarations
gated by name, exactly as described above; `AGENTS.md` states the same ("there is no production
plugin directory yet"). A host that wants a *new* family implements the trait; nothing in the
shipped three needs it to be loaded.

### Capability advertisement

`capabilities()` is a Symbolics builtin that returns a `CapabilitiesResult` record — the
machine-readable statement of what the engine will refuse, with the exact code, category and message
each live call produces:

```text
= CapabilitiesResult(solve_domains_accepted: [real, complex], solve_domains_refused: [integer, rational],
  unsupported_operations: [UnsupportedCapability(operation_class: pow.non-integer-exponent-of-a-positive-base,
  code: UnsupportedOperation, category: UnsupportedOperation, message: Non-integer exponents are not yet
  supported., ...), ...], exactness: BestEffort) (CapabilitiesResult)
```

That is a trimmed rendering — the live record lists 19 operation classes, including
`fft.non-power-of-two-length`, `solve.unsupported-domain`, `plot.symbolic-expression` and
`dsp.symbolic-element`. Run `capabilities()` (or `lovelace-run --function capabilities`) for the
full record. Nothing else advertises capability: there is no separate plugin manifest or version
negotiation beyond `mathIrVersion` in the envelope.

## Running the examples

Every `lovelace` block in these pages is executed and checked against the `text` block that follows
it:

```console
powershell -NoProfile -ExecutionPolicy Bypass -File docs/extensions/examples/run-examples.ps1
```

The runner writes each snippet to a temporary script file and runs `lovelace-run --file <path> --text
--omit-variables`, then compares the combined stdout/stderr with the documented block after
whitespace normalisation. It prints one line per example, a total, and exits 0 only when every
example matches. It sets `TEMP`/`APPDATA` to a scratch directory inside the clone so nothing leaks
into the user profile.

Use `--file` rather than `--eval` for anything containing a string literal: Windows PowerShell 5.1
strips embedded double quotes when it builds a native command line, so `--eval 'symbol("x")'`
actually delivers `symbol(x)` and the engine correctly answers `Undefined variable 'x'.` That is a
shell artefact, not a language behaviour.

## What is not an extension

- **Statistics does not exist.** There is no Statistics family, no `Statistics` category, and no
  `StatisticsPlugin` in production. The only `StatisticsPlugin` in the tree is a test double inside
  `#[cfg(test)] mod round67_modus` (`lovelace-lang/src/round67_modus.rs`), which exists to exercise the plugin seam.
  Names like `mean`/`sum`/`min`/`max`/`norm` are **core** `Arrays` reductions.
- **Linear algebra is core.** `det`, `eig_sym`, `inv`, `inv_full`, `linsolve`, `linsolve_full`,
  `matrix_rank`, `trace` and `matmul` are `plugin: None`; they work on a bare engine.
- **`plot` is core** (`category: Language`), and it takes numeric vectors only.
- **Only the symmetric eigensolver exists.** `eig_sym(a)` is the RustLace-only core builtin that
  returns an `EigenResult` for a real symmetric matrix; a non-square, non-symmetric or complex
  matrix is refused. There is no general `eigen`, `eigenvalue` or `eigenvector` builtin and no
  `svd`, `qr`, `lu` or `cholesky` either, and the C# Studio it descends from had none. The same is
  true of differential equations (`dsolve`), symbolic summation, and window functions (`hann`,
  `hamming`, `butter`, `iir`).

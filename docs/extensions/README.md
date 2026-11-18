# Library families

RustLace ships five statically compiled builtin families on top of a 44-builtin core. They are not
extensions, plugins or loadable modules: every family is compiled into `lovelace-lang`, every
descriptor is present in `Engine::new()`, and the one static dispatcher answers all of them. This
directory documents each family: what it contributes, what it refuses, worked examples, and its
specific limits.

## Using a library from Lovelace

As a language user, you do **not** need to know the corresponding Rust crate
or the implementation's module boundaries. Every operation listed here is
available by name in a regular `.ls` script.

For a problem-oriented entry point, start with
[Working with libraries](../how-to/use-libraries.md): it shows worked calls
and links the [Vector, Array, Symbolic, Complex and Record kinds](../language/values-and-types.md)
used by the examples. For syntax and exhaustive argument names, consult the
[builtin catalogue](../language/builtins.md); for one live descriptor, run
`cargo run -p lovelace-run -- --function fft`.

| Family (`module`) | Builtins | Builtin categories it spans | Page |
| --- | --- | --- | --- |
| DSP (`DSP`) | 16 | `DSP` | [dsp.md](dsp.md) |
| Symbolics (`Symbolics`) | 59 | `Symbolics` 40, `Calculus` 10, `Solving` 4, `Optimization` 2, `Introspection` 2, `Numerics` 1 | [symbolics.md](symbolics.md) |
| MathIR (`MathIR`) | 5 | `Compilation` | [mathir.md](mathir.md) |
| Systems (`Systems`) | 10 | `Systems` | [systems.md](systems.md) |
| Core (`Core`) | 44 | `Arrays` 23, `Numerics` 9, `Linear Algebra` 8, `Introspection` 3, `Language` 1 | — |

`44 + 16 + 59 + 5 + 10 = 134`, which is exactly what `lovelace-run --functions` reports
(`total: 134`) and what the execution envelope's `functions` array contains.

## The `module` field

The family key is the registry's **`module` field**, a static browsing label:

```rust
// lovelace-lang/src/registry.rs
/// The static family label (`Core`, `DSP`, `Symbolics`, `MathIR`, `Systems`): discovery
/// metadata only, never a load or dispatch gate.
pub module: &'static str,
```

| Family | `module` value | Count |
| --- | --- | --- |
| Symbolics | `"Symbolics"` | 59 |
| DSP | `"DSP"` | 16 |
| MathIR | `"MathIR"` | 5 |
| Core | `"Core"` | 44 |

Two facts about how it is used:

- **`module` is not `category`.** Symbolics spans six categories; `diff` is `Calculus` and
  `optimize` is `Optimization` yet both are `Symbolics`. DSP (16) and MathIR (5) happen to be
  one-category families, which makes the trap easy to miss.
- **The `--functions` listing does not print `module`.** Its JSON objects carry `name`, `category`,
  `minArity`, `parameterCount`, `parameters` and `variadic`. `--function <name>` (and the full
  discovery descriptor) adds `module`, `summary`, `returnKind`, `related` and `examples`.

No code path consults `module`: `Engine::call` resolves a user function, then the static registry,
then reports the vocabulary failure. There is no enabled-family set, no `load`, no bare engine and
no host registration; a compiled family is always available to `Engine::new()`, `lovelace-run` and
the desktop alike.

## Capability advertisement

`capabilities()` is a Symbolics builtin that returns a `CapabilitiesResult` record — the
machine-readable statement of what the engine will refuse, with the exact code, category and message
each live call produces:

```text
= CapabilitiesResult(solve_domains_accepted: [real, complex], solve_domains_refused: [integer, rational],
  unsupported_operations: [UnsupportedCapability(operation_class: pow.non-integer-exponent-of-a-positive-base,
  code: UnsupportedOperation, category: UnsupportedOperation, message: Non-integer exponents are not yet
  supported., ...), ...], exactness: BestEffort) 
```

That is a trimmed rendering — the live record lists 19 operation classes, including
`fft.non-power-of-two-length`, `solve.unsupported-domain`, `plot.symbolic-expression` and
`dsp.symbolic-element`. Run `capabilities()` (or `lovelace-run --function capabilities`) for the
full record. Nothing else advertises capability: there is no plugin manifest or family version
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

## What is not a family

- **Statistics does not exist.** There is no Statistics family, no `Statistics` category, and no
  statistics builtin. Names like `mean`/`sum`/`min`/`max`/`norm` are **core** `Arrays` reductions.
- **Linear algebra is core.** `det`, `eig_sym`, `inv`, `inv_full`, `linsolve`, `linsolve_full`,
  `matrix_rank`, `trace` and `matmul` are `Core`; they are available to every engine.
- **`plot` is core** (`category: Language`), and it takes numeric vectors only.
- **Only the symmetric eigensolver exists.** `eig_sym(a)` is the RustLace-only core builtin that
  returns an `EigenResult` for a real symmetric matrix; a non-square, non-symmetric or complex
  matrix is refused. There is no general `eigen`, `eigenvalue` or `eigenvector` builtin and no
  `svd`, `qr`, `lu` or `cholesky` either, and the C# Studio it descends from had none. The same is
  true of differential equations (`dsolve`), symbolic summation, and window functions (`hann`,
  `hamming`, `butter`, `iir`).
- **No dynamic extension mechanism exists.** `modus.rs`, `ModusPlugin`, `ModusHost`, `Payload`,
  `Engine::load_plugin` and `Engine::new_bare` were deleted in v0.6; a capability is added by
  compiling a crate and an adapter into the workspace, as [extending.md](../extending.md) describes.

# AGENTS.md - contributor guide for RustLace

RustLace is the Rust-only successor of the LovelaceSharp stack: arbitrary-precision exact
arithmetic, an N-dimensional array library, the Lovelace scripting language, a non-interactive
runner and a native Windows IDE, all in one workspace with no external crates under the engine.
The engine is `lovelace-lang`'s `Engine`, and every host drives the same engine rather than
re-implementing language behaviour. This file is the portable entry point; task skills are in
`.dsh/skills/` and are indexed at [docs/skills.md](docs/skills.md).

## Three entry points

| Surface | What it is | Start it |
|---|---|---|
| Engine library | `Engine`, the values, lexer, parser, interpreter, builtins and plugins | `cargo build --workspace`; source: `lovelace-lang/src/lib.rs` |
| `lovelace-run` CLI | non-interactive runner and the stable JSON wire envelope | `cargo run -p lovelace-run -- --eval "1/3" --text`; source: `lovelace-run/src/main.rs` |
| `lovelace-desktop` GUI | native Windows workbench plus headless acceptance modes | `cargo run -p lovelace-desktop`; source: `lovelace-desktop/src/main.rs` |

## Commands that matter

Run from the repository root. The engine crates build anywhere; the desktop is Windows-only.
Requires Rust 1.85+ (the tree was built with 1.99.1) and, on Windows, the MSVC build tools.

```console
# build
cargo build --workspace
cargo build --release -p lovelace-desktop

# test: whole workspace, one crate, one test or one module
cargo test --workspace
cargo test -p lovelace-lang
cargo test -p lovelace-lang round67
cargo test -p lovelace-desktop
cargo test -p lovelace-desktop lay_011

# format and lint
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --workspace --all-targets

# parity gate (release: the debug binary overflows its stack at lang/061 on this host)
cargo run --release -p lovelace-corpus
cargo run --release -p lovelace-corpus -- lang/001

# desktop headless modes (use a writable APPDATA)
cargo run -p lovelace-desktop -- --gui-selftest
cargo run -p lovelace-desktop -- --journey J-02 --out .tmp\journey
cargo run -p lovelace-desktop -- --selftest
cargo run -p lovelace-desktop -- --contrast-check
cargo run -p lovelace-desktop -- --legibility-check
cargo run -p lovelace-desktop -- --visual-audit
cargo run -p lovelace-desktop -- --glyph-check
cargo run -p lovelace-desktop -- --ui-audit
cargo run -p lovelace-desktop -- --thread-audit
cargo run -p lovelace-desktop -- --shortcuts
cargo run -p lovelace-desktop -- --platform-parity
cargo run -p lovelace-desktop -- --raster-svg in.svg --out out.png --scale 2
cargo run -p lovelace-desktop -- --help
```

Observed counts on this tree: `cargo test --workspace` gives 6135 passed and 1 ignored (the
ignored test is `property_assertions::mathir_evaluates_like_symbolic`, annotated in place);
`cargo test -p lovelace-desktop` gives 716 passed; `--gui-selftest` gives 20 checks, 0 failed;
the corpus gives 214 passed, 0 failed. A run that needs `%APPDATA%\Lovelace` needs that directory
writable: set `APPDATA` to a scratch directory first.

### CI jobs

`.github/workflows/ci.yml` has two jobs and runs on pushes to `main`, pull requests and manual
dispatch:

| Job | Commands |
|---|---|
| Windows (full workspace) | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`, `cargo build --release -p lovelace-desktop`, then upload `target/release/lovelace-desktop.exe` as `lovelace-desktop-windows-x86_64` |
| Linux (engine crates) | `cargo fmt --all -- --check`, `cargo clippy -p lovelace-lang -p lovelace-real -p lovelace-nat -p lovelace-symbolics --all-targets`, `cargo test -p lovelace-lang -p lovelace-real -p lovelace-symbolics` |

The Linux job deliberately does not build or test `lovelace-desktop`: its eframe/egui stack is a
Windows-only dependency. Clippy is run without `-D warnings` because the workspace has pre-existing
warnings; do not add new ones silently.

## Layout

One line per crate, and what it owns:

| Crate | Owns |
|---|---|
| `lovelace-json` | JSON reader/writer; numbers kept as their source text |
| `lovelace-nat` | arbitrary-precision naturals over base-2^64 limbs, NTT multiplication, Burnikel-Ziegler division, Newton square root |
| `lovelace-int` | sign + magnitude integers; truncating division; no negative zero |
| `lovelace-real` | reals as `int * 10^exp` with exact periodic decimal metadata; `sqrt`, `pi`, display-precision scoping |
| `lovelace-array` | generic row-major `NdArray<T>`, shape algebra, indexing, slicing, reductions |
| `lovelace-symbolics` | canonical expressions, differentiation, series, polynomial algebra, Groebner bases, the cooperative cancellation token |
| `lovelace-lang` | values, the active lexer/parser (`src/syntax.rs`), interpreter and builtins (`src/lib.rs`, `src/builtins.rs`), registry, Modus plugin host, MathIR, plots |
| `lovelace-run` | the CLI, the JSON wire envelope, registry discovery and golden envelopes |
| `lovelace-know` | the observation-driven behavioural graph, JSON persistence and convergence tests |
| `lovelace-corpus` | the documentation-corpus parity gate over `parity/corpus/*.json` |
| `lovelace-bench` | the micro-benchmark harness shaped like the C# reference harness |
| `lovelace-desktop` | the native workbench, execution host, incremental cache, persistence, accessibility and headless acceptance modes |
| `lovelace-desktop-spike` | the measured UI spike that chose eframe/egui; deliberately outside the workspace |

`docs/` is the long-form documentation: start at `docs/README.md`, then `docs/architecture.md`,
`docs/testing.md`, `docs/extending.md`, `docs/release.md`, the per-crate pages under `docs/crates/`,
and the decision records under `docs/adr/`. The screenshots and their capture method are in
`docs/images/`.

### Fixtures, and why C# fixtures are here

Fixtures live where the consuming test resolves them:

- `parity/corpus/lang.json` and `parity/corpus/symbolics.json` - the 214-record documentation
  corpus read by `lovelace-corpus`; `knowledge-graph-demo.json` at the root is the frozen knowledge
  graph (seed 20240617) read by `lovelace-know`.
- `docs/goal-desktop-v4/journeys.md`, `platform-matrix.md`, `reference/REF-*.json` and
  `round-64/REF-cancel.json` - read by the desktop journey, workbench, cancel-parity and
  platform-claims tests.
- `docs/csharp-oracle/` - the C# documentation oracles and golden wire fixtures read by
  `lovelace-lang`'s docs-sync tests and `lovelace-run`'s golden batch.
- `docs/symbolics/dsh-protocol.md` - protocol examples checked by `lovelace-run`'s wire batch.

The C# fixtures exist because the ported tests read files that used to live outside the `rust/`
subtree. They were carried here instead of deleting or weakening those tests, with paths retargeted
to this repository. `docs/csharp-oracle/Lovelace.Symbolics.Tests/UsageExamples.cs.txt` keeps the
original bytes but a `.txt` suffix so the repository contains no `.cs` path. Two C# DSP source scans
have no inputs here and pass vacuously; that residual is recorded in `docs/rustlace-notes.md`.
`docs/goal-desktop-v4/journeys.md` still cites capture rounds that live only in the LovelaceSharp
repository; that is deliberate and documented, not a link to fix.

## Conventions

### Errors

Use `LangError::coded(message, position, "StableCode", "Category")`; the message is a complete
sentence ending with a period, and `position` is a UTF-16 code-unit offset into the caller's source.
The seven categories in `lovelace-lang/src/diagnostic.rs` are frozen (`ParseError`, `DomainError`,
`UnsupportedOperation`, `BudgetExceeded`, `NoSolution`, `TypeMismatch`,
`InternalInvariantFailure`); do not add one. Keep the code stable and publish it in the taxonomy
tests when it is wire-visible. Argument refusals are `InvalidArgument` / `TypeMismatch`, carry the
declared signature, and are enforced at one call site from the registry declaration.

### Tests

Write tests as concrete input -> concrete assertion: an exact source string, then the exact typed
display, error message, code/category, position or period metadata. A test that asserts "something
happened" is not evidence. Put in-crate unit tests in `#[cfg(test)]` modules and batch files
(`roundNN_*.rs`, `suite_*_batch.rs`); use the existing `Engine::eval` and `format_typed` surface.
For a bug, add the regression test first and watch it fail on the old code.

### Where new things go

| You are adding | Put it here |
|---|---|
| a builtin | descriptor in `lovelace-lang/src/registry.rs`, implementation arm in `lovelace-lang/src/builtins.rs`, test beside the existing batch files, corpus record in `parity/corpus/` |
| a Modus plugin | a `ModusPlugin` implementation using the API in `lovelace-lang/src/modus.rs`; model it on `lovelace-lang/src/round67_modus.rs` (there is no production plugin directory yet) |
| a workbench pane | focus group in `lovelace-desktop/src/keyboard.rs`, layout and paint in `lovelace-desktop/src/ui/app.rs`, empty state in `lovelace-desktop/src/ui/empty.rs`, tokens in `lovelace-desktop/src/ui/theme.rs` |

### Fixture naming

- Corpus records: sequential ID matching the file prefix, zero-padded to three digits (`lang/112`,
  `sym/104`), with `id`, `doc`, `heading`, `script`, `kind`, `text`.
- Golden wire envelopes: `<case>.ls` plus `<case>.json` under
  `docs/csharp-oracle/Lovelace.Run.Tests/fixtures/`; the pair name matches the case, not the date.
- Desktop reference captures: `REF-<topic>.json` under `docs/goal-desktop-v4/reference/`.
- Knowledge graph: the fixture name states the seed (`knowledge-graph-demo.json`, seed 20240617).

### Commit messages

The history uses imperative subjects under about 72 characters, an optional area prefix, and
evidence in the subject or body. Examples:

```text
ci: add Windows full-suite and Linux engine-only GitHub Actions
test: carry the frozen knowledge graph reference and retarget its path
Desktop v4: RUN-007 certified - cancellation parity with the web Studio reference (709 tests)
style: apply cargo fmt across the workspace
```

One logical change per commit. "All tests pass" is not evidence; "716 tests, release x2 + debug
green" is.

## Guardrails

- Never weaken a test to make a change pass: do not delete it, `#[ignore]` it, relax an assertion,
  or drop a fixture. If a contract must move, move the fixture with it and say so in the commit.
- Never invent capability. If a code path does not exist, report it as missing rather than
  describing it as implemented; an unsigned release is unsigned and a headless check is not a GUI
  journey pass.
- Never commit build outputs (`target/`, `.tmp/`), `%APPDATA%` state, certificates, `.pfx` files,
  private keys or secrets. Screenshots belong in `docs/images/` only when they document a shipped
  state.
- Never hand-edit generated or captured artifacts without regenerating them: `docs/images/*.png`
  come from `docs/images/capture.ps1`; the golden wire fixtures are rewritten only with
  `LOVELACE_RECORD_GOLDENS=1`; `docs/goal-desktop-v4/reference/REF-*.json` are captured from a real
  run. Re-run the consuming test after regenerating.
- Never push a feature branch to `main` without a green CI run. Branch, open a pull request, and let
  the Windows and Linux jobs pass; `main` moves only through that gate.
- No external crate may enter an engine crate (ADR-001). The desktop's GUI stack is the only
  external dependency, and it is Windows-only.
- No floating point in a value path, and no `unsafe` outside the documented Win32 lane.
- The UI never evaluates, constructs an engine or compiles: `--ui-audit` enforces this. Pane data
  arrives through `WorkspaceSnapshot` / `DocumentView`.
- `LOVELACE_*_FAULT`, `LOVELACE_LOG_FLOOD` and the other acceptance seams must never be reachable
  from a script or on by default.

## If you want to X, read the skill

| Goal | Skill |
|---|---|
| add a language builtin end to end | [add-builtin](.dsh/skills/add-builtin/SKILL.md) |
| add a Modus plugin | [add-modus-plugin](.dsh/skills/add-modus-plugin/SKILL.md) |
| add or change a workbench pane | [add-ui-pane](.dsh/skills/add-ui-pane/SKILL.md) |
| change the lexer, parser or interpreter | [add-language-feature](.dsh/skills/add-language-feature/SKILL.md) |
| add a headless acceptance mode or journey | [add-acceptance-mode](.dsh/skills/add-acceptance-mode/SKILL.md) |
| change `lovelace-real` / `lovelace-nat` arithmetic | [change-numeric-kernel](.dsh/skills/change-numeric-kernel/SKILL.md) |
| reproduce and fix a desktop defect with evidence | [debug-desktop-issue](.dsh/skills/debug-desktop-issue/SKILL.md) |
| build, tag and publish a release | [release-and-publish](.dsh/skills/release-and-publish/SKILL.md) |

The same index, with one-line descriptions and when-to-use guidance, is
[docs/skills.md](docs/skills.md). For the prose reference, start at
[docs/extending.md](docs/extending.md) and [docs/contributing.md](docs/contributing.md).

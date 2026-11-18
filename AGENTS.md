# AGENTS.md - contributor guide for RustLace

RustLace is the Rust-only successor of the LovelaceSharp stack: arbitrary-precision exact
arithmetic, an N-dimensional array library, the Lovelace scripting language, a non-interactive
runner and a native desktop IDE for Windows and Linux, all in one workspace with no external crates
under the engine.
The engine is `lovelace-lang`'s `Engine`, and every host drives the same engine rather than
re-implementing language behaviour. This file is the portable entry point; task skills are in
`.dsh/skills/` and are indexed at [docs/skills.md](docs/skills.md).

## Three entry points

| Surface | What it is | Start it |
|---|---|---|
| Engine library | `Engine`, the values, lexer, parser, interpreter, builtins and static library families | `cargo build --workspace`; source: `lovelace-lang/src/lib.rs` |
| `lovelace-run` CLI | non-interactive runner and the stable JSON wire envelope | `cargo run -p lovelace-run -- --eval "1/3" --text`; source: `lovelace-run/src/main.rs` |
| `lovelace-desktop` GUI | native workbench (Windows and Linux) plus headless acceptance modes | `cargo run -p lovelace-desktop`; source: `lovelace-desktop/src/main.rs` |

## Commands that matter

Run from the repository root. The engine crates build anywhere; the desktop workbench builds and
runs on Windows and on Linux (X11). Linux needs the X11/GL/font packages and, on a display-less
machine, Xvfb - see [Build on Linux](#build-on-linux).
Requires Rust 1.85+ (`rust-version` in `Cargo.toml`; the tree was built with 1.100.0) and, on
Windows, the MSVC build tools. A per-user rustup install keeps `cargo.exe` and `rustc.exe` under
`%USERPROFILE%\.cargo\bin`; use that full path when a shell's `PATH` predates the install (a
long-running terminal or agent host keeps the environment it started with).

```console
# build
cargo build --workspace
cargo build --release -p lovelace-desktop

# test: whole workspace, one crate, one test or one module
cargo test --workspace
cargo test -p lovelace-lang
cargo test -p lovelace-lang systems_family
cargo test -p lovelace-desktop
cargo test -p lovelace-desktop lay_011

# format and lint
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --workspace --all-targets

# parity gate (release: the debug binary overflows its stack at lang/061 on this host)
cargo run --release -p lovelace-corpus
cargo run --release -p lovelace-corpus -- lang/001

# canonical numerical kernel and architecture policy (run before calling a change done;
# the mutation suite has 21 scenarios, and CODEOWNERS declares the policy and kernel owners)
python tools/check-architecture.py
python tools/test-architecture-gates.py
python tools/harness/check-agent-contracts.py        # after building lovelace-run
python tools/harness/test-agent-contracts.py         # proves stale skill checks fail
cargo test -p lovelace-rational
cargo test -p lovelace-lang --test cross_library

# desktop headless modes. The application directory must be writable: set
# APPDATA on Windows, or XDG_CONFIG_HOME/HOME on Linux, to a scratch directory.
# Every mode below runs on both platforms except --platform-parity, which needs
# the Windows build and exits 2 with a named refusal elsewhere.
cargo run -p lovelace-desktop -- --gui-selftest
cargo run -p lovelace-desktop -- --journey J-02 --out .tmp/journey
cargo run -p lovelace-desktop -- --selftest
cargo run -p lovelace-desktop -- --contrast-check
cargo run -p lovelace-desktop -- --legibility-check
cargo run -p lovelace-desktop -- --visual-audit
cargo run -p lovelace-desktop -- --glyph-check
cargo run -p lovelace-desktop -- --ui-audit
cargo run -p lovelace-desktop -- --thread-audit
cargo run -p lovelace-desktop -- --shortcuts
cargo run -p lovelace-desktop -- --platform-claims
cargo run -p lovelace-desktop -- --platform-parity            # Windows build only; exit 2 elsewhere
cargo run -p lovelace-desktop -- --raster-svg in.svg --out out.png --scale 2   # in.svg: any SVG file
cargo run -p lovelace-desktop -- --help
```

The paths above are POSIX, for bash on Linux or Git Bash on Windows. Windows PowerShell spells the
same output directory `.tmp\journey`; a backslash path handed to bash creates a file literally named
`.tmp\journey`, so the two forms are not interchangeable. `--journey` is headless on both platforms
(the modes that need the Windows build are `--legacy-win32`, `--ime-probe` and `--platform-parity`),
and `--journey J-02` writes no artifact: only J-09 and J-15 write into `--out`, so the success line
for J-02 is `JOURNEY summary journeys=1 failed=0`.

Test counts are not duplicated here: [docs/testing.md](docs/testing.md#1-unit-tests-in-crate-test)
carries the measured aggregate with its commit SHA, OS, command and date, keeps the Linux and
Windows desktop observations separate, and records the one ignored test
(`property_assertions::mathir_evaluates_like_symbolic`, annotated in place). Read the count from a
run at the commit you are changing; do not quote a historical total as current.
`--gui-selftest` gives 20 checks, 0 failed on both platforms; `--selftest` gives 15 checks on
Windows and 14 plus a named `no-network-imports` skip on Linux; the corpus gives 231 passed, 0
failed. A run that needs the application directory needs it writable:
set `APPDATA` on Windows, or `XDG_CONFIG_HOME`/`HOME` on Linux, to a scratch directory first.

### Build on Linux

The GUI stack loads X11, GL and xkbcommon through `dlopen`, so the build itself needs no X11
headers; opening the window does. On Debian/Ubuntu:

```console
sudo apt-get update
sudo apt-get install -y libxkbcommon-dev libxkbcommon-x11-0 libx11-dev libxcursor-dev \
    libxrandr-dev libxi-dev libgl1-mesa-dev libegl1-mesa-dev libgl1-mesa-dri \
    libwayland-dev pkg-config fonts-urw-base35 xvfb xauth x11-utils
```

- `fonts-urw-base35` is the strong-face dependency (`NimbusSans-Bold.otf`, 1.20 em); without it
  `--legibility-check` and the strong-role test fail.
- `libgl1-mesa-dri` is mesa's software GL (llvmpipe) for a host with no GPU, such as WSLg or a CI
  runner under Xvfb.
- The renderer is deliberately split: Windows keeps eframe's default wgpu renderer, non-Windows
  compiles and selects `glow` (`lovelace-desktop/Cargo.toml:39`,
  `lovelace-desktop/src/ui/app.rs:9908`). Do not "unify" it: wgpu cannot acquire a device under
  WSLg (no `/dev/dri`), which is why the Linux window uses GLX/llvmpipe.
- Under WSLg, `unset WAYLAND_DISPLAY` and `export DISPLAY=:0`; otherwise winit selects Wayland and
  the process dies with `Io error: Connection reset by peer (os error 104)`.
- Windowless acceptance and tests:

  ```console
  xvfb-run -a -s "-screen 0 1600x1000x24" ./target/release/lovelace-desktop --gui-selftest
  xvfb-run -a -s "-screen 0 1600x1000x24" cargo test -p lovelace-desktop
  ```

### CI jobs

`.github/workflows/ci.yml` has five job definitions and runs on pushes to `main`, pull requests and
manual dispatch:

| Job | Commands |
|---|---|
| Windows (full workspace) | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`, `cargo build --release -p lovelace-desktop`, then upload `target/release/lovelace-desktop.exe` as `lovelace-desktop-windows-x86_64` |
| Linux (engine crates + GUI desktop under Xvfb) | `cargo fmt --all -- --check`, the Linux GUI runtime, `cargo clippy -p lovelace-lang -p lovelace-rational -p lovelace-real -p lovelace-nat -p lovelace-symbolics -p lovelace-systems --all-targets`, `cargo test -p lovelace-lang -p lovelace-real -p lovelace-symbolics -p lovelace-systems`, `cargo build --release -p lovelace-desktop`, the glibc ABI guard over the built artifact, `xvfb-run ... --gui-selftest` (exit 0 and `checks=20 failed=0`), `xvfb-run ... cargo test -p lovelace-desktop`, then upload `target/release/lovelace-desktop` as `lovelace-desktop-linux-gui-x86_64` |
| Architecture policy (`ubuntu-latest` and `windows-latest`) | `python tools/check-architecture.py --report architecture-impact-<os>.md`, `python tools/test-architecture-gates.py` (21 scenarios, including the two-field/two-method `partial-duplicate-kernel`), `python tools/rational-oracle.py --check`, `python tools/systems-oracle.py --check`, then upload the impact report. Ownership is abstraction-specific (`reservedTypeNames`, `permittedRepresentations`), and `.github/CODEOWNERS` routes policy, checker, mutation-fixture, ownership-document and rational-kernel changes to the owner. Independent of the unit-test jobs: a green suite cannot outvote a second rational kernel. |
| Numerical kernel | `cargo test -p lovelace-rational -p lovelace-int -p lovelace-nat -p lovelace-real`, `cargo test -p lovelace-rational --test properties`, `--test oracle`, `cargo test -p lovelace-systems --test oracle`, `cargo test -p lovelace-lang --test cross_library` |
| Documentation examples (`ubuntu-latest` and `windows-latest`) | seventeen steps, one definition, two OS variants: checkout pinning `core.autocrlf=false`, Rust toolchain, cargo cache, `cargo build -p lovelace-run`, `python tools/harness/check-agent-contracts.py`, `python tools/harness/test-agent-contracts.py`, `bash tools/check-crate-examples.sh`, `bash tools/run-tutorials.sh`, `bash docs/language/examples/run-examples.sh` (fenced examples plus the builtin-catalogue check against `--functions`) and `bash docs/extensions/examples/run-examples.sh`, `bash tools/check-doc-examples.sh` (every documented Lovelace snippet, in each page family's documented output mode), `bash tools/check-docs.sh --symbols`, `--claims`, `--readme`, `bash tools/check-reference-pages.sh`, `bash tools/test-docs-gates.sh` (the mutation self-test), `bash tools/docs-report.sh`, and the evidence upload |

The Linux job is the GUI acceptance gate: it compiles the workbench, runs the 20-check selftest and
the full desktop suite (editor, panes, theme, plot rasteriser, journeys) under Xvfb, and uploads the
GUI binary. The Windows job still runs the workspace suite, builds the release executable and uploads
`lovelace-desktop-windows-x86_64`. The documentation job compiles and runs the crate-page Rust
fences and the four tutorials, executes both example runners (the language one also verifies the
builtin catalogue against the built runner's `--functions` registry) and the every-page
`check-doc-examples.sh` backstop, runs the three consistency
modes and the reference-page contract, and proves each gate can fail with the mutation self-test — on
Ubuntu and on Windows, so a path-shape, `.exe`-discovery or line-ending bug
fails on the platform that has it; its checkout pins `core.autocrlf=false` so the bash runners see
LF on Windows. No job captures a window: the Windows frames come from the documenting host and the
one Linux frame is `docs/images/gui-linux.png` (WSLg). A `--gui-selftest` pass is a run-path check,
not a GUI journey pass. The Linux job was also the first to compile the desktop crate there; it found the missing
`std::os::unix::fs::OpenOptionsExt` import in the atomic-save path. Clippy is run without
`-D warnings` because the workspace has pre-existing warnings; do not add new ones silently.

## Layout

One line per crate, and what it owns:

| Crate | Owns |
|---|---|
| `lovelace-json` | JSON reader/writer; numbers kept as their source text |
| `lovelace-nat` | arbitrary-precision naturals over base-2^64 limbs, NTT multiplication, Burnikel-Ziegler division, Newton square root |
| `lovelace-int` | sign + magnitude integers; truncating division; no negative zero |
| `lovelace-rational` | the one canonical exact rational kernel: full arbitrary-precision GCD reduction, checked arithmetic, growth budgets, exactness provenance and conversions |
| `lovelace-real` | reals as `int * 10^exp` with exact periodic decimal metadata; `sqrt`, `pi`, display-precision scoping; the `Rat`↔`Real` projections |
| `lovelace-array` | generic row-major `NdArray<T>`, shape algebra, indexing, slicing, reductions |
| `lovelace-symbolics` | canonical expressions, differentiation, series, polynomial algebra, Groebner bases, the cooperative cancellation token; re-exports `Rat` from the kernel |
| `lovelace-lang` | values, the active lexer/parser (`src/syntax.rs`), interpreter and builtins (`src/lib.rs`, `src/builtins.rs`), registry, MathIR, plots, the `sys_*` systems adapter |
| `lovelace-systems` | exact queueing/operations-research models: M/M/1, M/M/c, M/M/∞, operational laws, open Jackson networks, exact single-class MVA, bottleneck/capacity, sweeps; `Q` is the canonical `Rat` alias; no dependency on the interpreter |
| `lovelace-run` | the CLI, the JSON wire envelope, registry discovery and golden envelopes |
| `lovelace-know` | the observation-driven behavioural graph, JSON persistence and convergence tests; `ExactNumber` is a facade over `Rat` |
| `lovelace-corpus` | the documentation-corpus parity gate over `parity/corpus/*.json` |
| `lovelace-bench` | the micro-benchmark harness shaped like the C# reference harness |
| `lovelace-desktop` | the native workbench (Windows and Linux), execution host, incremental cache, persistence, accessibility and headless acceptance modes |
| `lovelace-desktop-spike` | the measured UI spike that chose eframe/egui; deliberately outside the workspace |

`docs/` is the long-form documentation: start at `docs/README.md`, then `docs/architecture.md`,
`docs/testing.md`, `docs/extending.md`, `docs/release.md`, the per-crate pages under `docs/crates/`,
and the decision records under `docs/adr/`. The screenshots and their capture method are in
`docs/images/`.

### Fixtures, and why C# fixtures are here

Fixtures live where the consuming test resolves them:

- `parity/corpus/lang.json`, `parity/corpus/symbolics.json` and `parity/corpus/systems.json` - the 231-record documentation
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

### Documentation requirements

Documentation changes are reviewed against [docs/editorial.md](docs/editorial.md). Two rules are
blocking: every numerical claim says which of the four kinds it is — exact, precision-limited,
error-bounded or parity-verified — and every example uses a repository-relative path or a
placeholder rather than a path from one machine.

### Where new things go

| You are adding | Put it here |
|---|---|
| a builtin | descriptor in `lovelace-lang/src/registry.rs`, implementation arm in `lovelace-lang/src/builtins.rs`, test beside the existing batch files, corpus record in `parity/corpus/` |
| a statically integrated library family | a workspace crate under `lovelace-systems/` (typed models, exact arithmetic through `lovelace-rational`, no interpreter dependency) plus a thin adapter and static descriptors in `lovelace-lang`; follow `docs/extending.md` §2 and declare the crate in `tools/architecture-policy.json` |
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

One logical change per commit. "All tests pass" is not evidence; "a measured test count, release x2 + debug
green" is.

Conventional prefixes drive the release version, so the prefix is not cosmetic: `feat:` is a minor
bump, `fix:`/`perf:` a patch, and `type!:` or a `BREAKING CHANGE:` footer a minor bump while the
major version is 0 (`0.1.0 -> 0.2.0`, deliberately not `1.0.0`) and a major bump once it is 1 or
later. `docs:`, `test:`, `ci:`, `chore:`, `style:`, `refactor:`, `build:`, unknown prefixes and
freeform subjects do not release on their own. 1.0.0 is reached only by a commit whose subject
contains `release: 1.0.0` (or a `BREAKING CHANGE:` footer carrying `major: 1` on its first line);
nothing else moves the major version. `.github/workflows/release.yml` releases automatically after a
green CI run on `main`, so do not tag by hand. The full table is [docs/release.md](docs/release.md).

### Licence

RustLace is released under the MIT licence ([LICENSE](LICENSE)); contributions are accepted under
the same terms (inbound = outbound), with no separate contributor licence agreement.

## Guardrails

- Before following a task skill, validate its current schema references with
  `python tools/harness/check-agent-contracts.py --static-only`. The builtin registry uses
  the static `module` field, never the retired `plugin` field. Runtime verification is
  mandatory for changes affecting a builtin contract; run the checker without `--static-only`.
- Never weaken a test to make a change pass: do not delete it, `#[ignore]` it, relax an assertion,
  or drop a fixture. If a contract must move, move the fixture with it and say so in the commit.
- Never invent capability. If a code path does not exist, report it as missing rather than
  describing it as implemented; an unsigned release is unsigned and a headless check is not a GUI
  journey pass.
- Never commit build outputs (`target/`, `.tmp/`), `%APPDATA%` state, certificates, `.pfx` files,
  private keys or secrets. Screenshots belong in `docs/images/` only when they document a shipped
  state.
- Never hand-edit generated or captured artifacts without regenerating them: the Windows frames in
  `docs/images/` come from `docs/images/capture.ps1`, `gui-linux.png` is an `xwd` capture (provenance
  in `docs/images/README.md`); the golden wire fixtures are rewritten only with
  `LOVELACE_RECORD_GOLDENS=1`; `docs/goal-desktop-v4/reference/REF-*.json` are captured from a real
  run. Re-run the consuming test after regenerating.
- Never push a feature branch to `main` without a green CI run. Branch, open a pull request, and let
  the Windows, Linux, architecture-policy and documentation jobs pass (both OS variants of the
  architecture and documentation jobs); `main` moves only through that gate.
- One numerical foundation. `lovelace-rational` owns the only general-purpose rational
  implementation; `lovelace-symbolics::Rat` is a re-export, `lovelace-systems::Q` is an alias, and
  `lovelace-know::ExactNumber` is a delegating facade. Do not add numerator/denominator storage,
  GCD normalization or scalar arithmetic traits outside the owners. Improve the owner instead
  (RNL-ARC-001, RNL-AGT-003). Run `python tools/check-architecture.py` and the cross-library tests;
  a local test pass is not completion evidence. The mutation suite is 21 scenarios, including the
  two-field/two-method partial representation; ownership is per reserved type, not per crate; an
  arithmetic impl the scanner cannot classify is reported as `review-required`, not passed. Policy,
  checker, mutation-fixture, ownership-document and rational-kernel changes are declared in
  `.github/CODEOWNERS`. Never weaken the policy, delete a failing test or grant yourself an
  exception (RNL-AGT-006).
- No external crate may enter an engine crate (ADR-001). The desktop's GUI stack is the only
  external dependency, declared in the desktop crate only: eframe's default wgpu on Windows, plus
  glow on Linux.
- No floating point in a value path, and no `unsafe` outside the documented Win32 lane.
- The UI never evaluates, constructs an engine or compiles: `--ui-audit` enforces this. Pane data
  arrives through `WorkspaceSnapshot` / `DocumentView`.
- `LOVELACE_*_FAULT`, `LOVELACE_LOG_FLOOD` and the other acceptance seams must never be reachable
  from a script or on by default.

## If you want to X, read the skill

| Goal | Skill |
|---|---|
| add a language builtin end to end | [add-builtin](.dsh/skills/add-builtin/SKILL.md) |
| add or change a workbench pane | [add-ui-pane](.dsh/skills/add-ui-pane/SKILL.md) |
| change the lexer, parser or interpreter | [add-language-feature](.dsh/skills/add-language-feature/SKILL.md) |
| add a headless acceptance mode or journey | [add-acceptance-mode](.dsh/skills/add-acceptance-mode/SKILL.md) |
| change `lovelace-rational`, `lovelace-real` or `lovelace-nat` arithmetic | [change-numeric-kernel](.dsh/skills/change-numeric-kernel/SKILL.md) |
| reproduce and fix a desktop defect with evidence | [debug-desktop-issue](.dsh/skills/debug-desktop-issue/SKILL.md) |
| build, tag and publish a release | [release-and-publish](.dsh/skills/release-and-publish/SKILL.md) |
| update the dependent documentation after a change | [sync-docs](.dsh/skills/sync-docs/SKILL.md) |

The same index, with one-line descriptions and when-to-use guidance, is
[docs/skills.md](docs/skills.md). For the prose reference, start at
[docs/extending.md](docs/extending.md) and [docs/contributing.md](docs/contributing.md).

# RustLace — workspace context for AI assistants

## What this repository is

RustLace is the Rust-only successor of the LovelaceSharp stack: arbitrary-precision exact
arithmetic, an N-dimensional array library, the Lovelace scripting language, the `lovelace-run` CLI
and the native desktop IDE `lovelace-desktop`, which builds and runs on Windows and on Linux (X11). The C# sources are **not here** — they remain in the
LovelaceSharp repository. Comments that say "the reference's …" cite that implementation on purpose:
they are receipts for behaviour this port reproduces, not leftovers to clean up.

A different `.github/copilot-instructions.md` lives in LovelaceSharp and describes the C# solution
(BCD digit store, xUnit, Native AOT, the Studio HTTP API). It does not apply to this repository.
This file does.

## Build and test

```console
cargo build --workspace
cargo test --workspace
cargo run --release -p lovelace-corpus      # 231 records; run it in release (see below)
cargo test -p lovelace-desktop
cargo run -p lovelace-desktop -- --gui-selftest
python tools/check-architecture.py          # ownership and dependency policy
python tools/test-architecture-gates.py     # mutation self-test for the checker
cargo test -p lovelace-lang --test cross_library
```

Toolchain: Rust 1.85+, edition 2024, MSVC linker on Windows. The engine builds offline; the desktop
needs a crate registry once for `eframe`/`egui` 0.37 and `resvg` 0.50. On Linux the workbench also
needs the X11/GL/font packages and, with no display, `xvfb-run` (see `docs/build-and-run.md#linux`);
non-Windows targets compile eframe's `glow` backend and `ui::native_options` selects it, because wgpu
cannot acquire a device on the certified Linux path. The debug build of the corpus
gate overflows its main-thread stack at `lang/061` on some hosts; the release build passes.

## Architectural rules (do not break these)

1. **No external crates in the engine.** `[dependencies]` may reference workspace members by path
   only. Only `lovelace-desktop` has third-party dependencies (`eframe`/`egui`, `resvg`,
   `raw-window-handle`), and they are declared for every target, not under `cfg(windows)`: non-Windows
   targets add eframe's `glow` backend. The desktop is not Windows-only.
2. **No floating point in a value path.** Reals are `int * 10^exp` plus exact period metadata.
   `1/3` is `0.(3)`, never a rounding.
3. **One numerical foundation.** `lovelace-rational` owns the only general-purpose rational
   implementation. `lovelace-symbolics::Rat` is a re-export and `lovelace-systems::Q` is a
   type-identical alias; neither crate may define numerator/denominator storage, GCD normalization or
   scalar arithmetic traits of its own. The canonical owners and permitted dependency edges are in
   `tools/architecture-policy.json`, explained by `docs/architecture-ownership.md`, and enforced by
   `python tools/check-architecture.py`. Improve the owner; do not start a second kernel. A new crate
   must declare its layer, responsibility, dependencies and reused abstractions in that policy.
4. **Exactness is four claims, not one.** Keep computation exactness, input fidelity, representation
   exactness and render precision separate. A `Real` is a projection; an exact systems value crosses
   the language and wire boundary through the `exact_values` numerator/denominator record. Never
   promote a precision-limited `Real` to an exact rational.
3. **No `unsafe` outside `lovelace-desktop/src/win32`.** The four `#![forbid(unsafe_code)]` crates
   enforce it for themselves.
4. **Rendering is the contract.** The rendered text, the error sentences and the kind names are pinned
   by `parity/corpus/*.json`. Changing one is a contract change, not a formatting tweak.
5. **The engine is the only source of behaviour.** A host that re-implements a rule (formatting,
   evaluation, arity, cancellation) is a defect even when its output happens to match.
6. **Cancellation has exactly two seams**: the top-level statement boundary in
   `lovelace-lang/src/lib.rs` and the polls inside long kernels. Do not add a third.
7. **The desktop's incremental cache is keyed on full canonical strings**, never digests, and a cache
   hit is only reported after the cached typed value was applied. Side-effecting builtins are never
   cached (`lovelace-desktop/src/incremental.rs`).
8. **Every save is atomic** (temp file in the target's directory, flush, sync, rename) and an
   external change is detected by size + mtime + content hash (`document_io.rs`).
9. **UI colours come from `ui/theme.rs`.** That module is the only place in `ui/` allowed to construct
   a `Color32`, and a test enforces it. Text sizes come from `ui/type_scale.rs`.
10. **`LOVELACE_*` fault and flood switches are acceptance seams.** They must stay inert by default
    and unreachable from a script.

## Where to look

| Question | File |
|---|---|
| the engine API | `lovelace-lang/src/lib.rs` (`Engine`) |
| the builtin catalogue (names, arity, categories, docs) | `lovelace-lang/src/registry.rs` |
| builtin implementations | `lovelace-lang/src/builtins.rs` (`pub fn call`) |
| the systems library | `lovelace-systems/src/lib.rs`, `lovelace-lang/src/systems_family.rs` |
| the canonical rational kernel | `lovelace-rational/src/lib.rs`, `docs/crates/lovelace-rational.md` |
| architecture ownership | `docs/architecture-ownership.md`, `tools/architecture-policy.json` |
| cancellation | `lovelace-lang/src/cancellable.rs`, `lovelace-symbolics/src/cancel.rs` |
| the wire envelope | `lovelace-run/src/main.rs` |
| the parity gate | `lovelace-corpus/src/main.rs`, `parity/corpus/` |
| the desktop run path | `lovelace-desktop/src/execution.rs`, `workbench.rs` |
| the incremental cache | `lovelace-desktop/src/incremental.rs` |
| files and recovery | `lovelace-desktop/src/document_io.rs`, `recovery.rs` |
| the theme and contrast gate | `lovelace-desktop/src/ui/theme.rs` |
| the default precision decision | `lovelace-desktop/src/engine.rs` (`DEFAULT_PRECISION`), `precision_default.rs` |

The long-form documentation starts at [`docs/README.md`](../docs/README.md); the decisions are in
[`docs/adr/`](../docs/adr/); the workspace contract is [`CONTRACT.md`](../CONTRACT.md).

## When changing code

- Test at the layer you touched, then run `cargo test --workspace` and the release corpus gate.
- Before adding a primitive or type, search the workspace for the canonical owner and record your
  reuse decision. Run `python tools/check-architecture.py` and the cross-library tests; a local test
  pass is not completion evidence (RNL-AGT-005).
- Format with `cargo fmt --all`; CI checks it. Clippy runs without `-D warnings` because the tree has
  pre-existing warnings — do not add new ones silently.
- Update the docs that describe the touched area in the same change (architecture, crate page,
  testing, or an ADR).
- Commit subjects are imperative and evidence-bearing: name the requirement or behaviour and the test
  count or profile, as the history does (`Desktop v4: RUN-007 certified … (709 tests)`).

# ADR-001: no external crates in the engine

**Status:** accepted (inherited from the port's contract)

## Context

The workspace was ported from LovelaceSharp under a build host that could not reach a crate
registry. The contract in `CONTRACT.md` states the rule directly: "**No external crates.** The
registry is unreachable; `[dependencies]` may only reference workspace crates by path. No unsafe, no
floating point in value paths." Every engine crate's `Cargo.toml` follows it — `lovelace-nat`,
`lovelace-int`, `lovelace-real`, `lovelace-array`, `lovelace-symbolics`, `lovelace-lang`,
`lovelace-json`, `lovelace-run`, `lovelace-corpus`, `lovelace-bench` and `lovelace-know` declare only
path dependencies.

## Decision

Keep the engine dependency-free. JSON, arbitrary-precision arithmetic, the NTT, the SVG writer and
the corpus gate are written in-tree and auditable in one place. The only external crates in the
workspace are the desktop's GUI stack: `eframe`/`egui` 0.37, `resvg` 0.50 and `raw-window-handle`
0.6, declared in `lovelace-desktop/Cargo.toml`. That stack is cross-platform, and so is the
declaration: the base `[dependencies]` table carries it for every target, and non-Windows targets add
eframe's `glow` backend under `[target.'cfg(not(windows))'.dependencies]`. No engine crate depends on
any of it, and nothing else in the workspace names a registry package.

## Consequences

- A clean clone builds offline for the engine and the CLI; only the desktop needs a registry.
- Any production dependency added to an engine crate is an architectural change, not a convenience.
- The desktop's dependency set is platform-neutral rather than Windows-only: adding a Linux or macOS
  build does not change this decision, and it must not be used as a reason to give an engine crate a
  registry dependency.
- `#![forbid(unsafe_code)]` in `lovelace-int`, `lovelace-array` and `lovelace-know`, plus
  `#![cfg_attr(not(windows), forbid(unsafe_code))]` in `lovelace-desktop/src/main.rs`, keeps the
  in-tree code reviewable. The Windows `unsafe` lane is confined to `lovelace-desktop/src/win32`.

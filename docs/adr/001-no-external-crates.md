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
workspace are the desktop's GUI stack, declared under `[target.'cfg(windows)'.dependencies]`:
`eframe`/`egui` 0.37, `resvg` 0.50 and `raw-window-handle` 0.6 (`lovelace-desktop/Cargo.toml`).

## Consequences

- A clean clone builds offline for the engine and the CLI; only the desktop needs a registry.
- Any production dependency added to an engine crate is an architectural change, not a convenience.
- `#![forbid(unsafe_code)]` in `lovelace-int`, `lovelace-array`, `lovelace-know` and the non-Windows
  half of `lovelace-desktop` keeps the in-tree code reviewable.

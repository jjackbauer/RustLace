# `lovelace-desktop-spike`

The standalone UI spike that decided the desktop stack — kept in the repository as measured evidence,
not as a shipped crate.

## Purpose

It is a **separate package, not a workspace member**: `lovelace-desktop-spike/Cargo.toml` declares an
empty `[workspace]` table, with the comment "Standalone spike: NOT a member of rust/Cargo.toml. The
empty [workspace] table is what stops cargo from walking up and claiming this package as a member."
It measured eframe/egui 0.37 and resvg 0.50 with an optimized dev profile
(`[profile.dev] opt-level = 2`, `debug = false`) so the reported numbers described optimized code.

Modules: `app.rs` (the window), `editor.rs` (text editing), `plot.rs` (SVG rendering), `bench.rs`
(measurement), `cpu.rs`, `json.rs`, `sample.rs`, `syntax.rs`, `main.rs`.

## Public surface

None: it is a `publish = false` binary. The crate exists to be read and, if the decision is
contested, re-run.

## Dependencies

`eframe` 0.37, `egui` 0.37, `resvg` 0.50.

## Tests

No `#[test]` functions. The spike's output was a measured report, and the shipped crate reuses its
stack: "The new native workbench (round-5 cutover) reuses the measured eframe/egui 0.37 and resvg
0.50 stack from rust/lovelace-desktop-spike" (`lovelace-desktop/Cargo.toml`).

## Using it

```console
$ cd lovelace-desktop-spike
$ cargo run
```

It is not built by `cargo build --workspace` and it is not part of the release artifact.

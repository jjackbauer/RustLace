# `lovelace-desktop`

The native desktop IDE for the Lovelace scripting language, on Windows and Linux: a windowed
workbench plus its headless acceptance modes.

## Purpose

The binary has a windowed mode (eframe/egui 0.37, resvg 0.50) and headless modes that drive the same
run path the window uses, so acceptance can run on a build host without a human at the keyboard
(`lovelace-desktop/src/main.rs:1`).

| Mode | What it does |
|---|---|
| default `[script.ls]` | the native workbench, optionally opened on a file |
| `--gui-selftest` | headless acceptance of the workbench run path (the same `Workbench` methods) |
| `--journey <ID> [--out <dir>]` | the implemented journeys (J-02, J-03, J-07, J-08, J-09, J-10, J-15) against `docs/goal-desktop-v4/journeys.md` |
| `--selftest` | the pre-cutover headless acceptance of the application core |
| `--raster-svg <in.svg> --out <out.png> [--scale N]` | rasterise one SVG exactly as the plot pane does |
| `--contrast-check` | every dark/light token pair's WCAG ratio; fails below 4.5:1 / 3:1 |
| `--legibility-check` | type scale and legibility report (min 11 pt, 1.2× line ratio, 4.5:1) |
| `--visual-audit` | design-system tables printed from the constants the window paints with |
| `--glyph-check` | no-tofu report for the mathematical glyph set |
| `--ui-audit` | source audit over `src/ui` (no evaluation, no engine construction) |
| `--thread-audit` | source audit over every thread spawn, sleep and sync primitive |
| `--shortcuts` | the keyboard map, generated from `keyboard::BINDINGS` |
| `--platform-parity` / `--platform-claims` | platform behaviour report and the release matrix's claims |
| `--measure-*`, `--perf-*` | startup, typing, scroll, run responsiveness and precision probes |
| `--ime-probe`, `--raster-svg` | IME probe and SVG rasteriser |

## Structure

`execution.rs` (background runs, typed events), `workbench.rs` (headless document state),
`incremental.rs` (the statement cache), `statements.rs` (the slicer), `document_io.rs` + `recovery.rs`
(files), `settings.rs`, `engine/` (`Evaluator` trait, `RealEngine`, deterministic `FakeEngine`,
fault injection), `ui/` (window, editor, plot, theme, type scale, accessibility), `win32/` (FFI and
the shell).

## Public surface

A binary crate: no library target. The `Evaluator` trait (`engine.rs:4`) is the seam a host would
implement to drive the app with another engine; `Workbench` (`workbench.rs`) is the headless state a
test drives.

## Dependencies

`lovelace-json`; `lovelace-lang` and `lovelace-symbolics` behind the default `real-engine` feature;
`eframe`/`egui` 0.37, `resvg` 0.50 and `raw-window-handle` 0.6, declared in this crate only. eframe
keeps its default wgpu renderer on Windows and adds the `glow` feature on other platforms. A
`--no-default-features` build keeps the crate usable offline and switches every document to
`engine::FakeEngine` (`lovelace-desktop/Cargo.toml`).

## Tests

716 `#[test]` functions on Windows, 706 plus the 4 in `tests/linux_cli.rs` on Linux, run by
`cargo test -p lovelace-desktop`. They cover the execution host, the
incremental cache, statements, documents and files, recovery, settings, the theme and contrast
checker, the type scale, the editor model, completion, find, the workspace tables, the inspector,
the plot pane and the UI audits.

## Using it

```console
$ cargo run -p lovelace-desktop                 # the window
$ cargo run -p lovelace-desktop -- notes.lv     # the window, opened on a file
$ cargo run -p lovelace-desktop -- --gui-selftest
$ cargo run -p lovelace-desktop -- --shortcuts
```

The panes the run publishes are the product's whole surface. A symbolic result fills both the
workspace's structured projection and the inspector pane:

![A symbolic run: the structured projection in the workspace and the pretty/canonical forms in the Symbolic Inspector](../images/symbolic-inspector.png)

Editing is served by the same panes — the docked find bar works on the active document and reports
its match count:

![The docked find bar with 11 matches highlighted and 1 of 11 shown](../images/find-and-replace.png)

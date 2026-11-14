# ADR-002: egui/eframe as the native shell

**Status:** accepted

## Context

The desktop had to replace a browser IDE (CodeMirror 6 + an HTTP session API) with a native
Windows application without losing editing, accessibility, SVG rendering or layout behaviour. The
candidate spike lived in `lovelace-desktop-spike`, a standalone package that is deliberately **not**
a workspace member (`lovelace-desktop-spike/Cargo.toml`: "Standalone spike: NOT a member of
rust/Cargo.toml"). It measured eframe/egui 0.37 and resvg 0.50 with an optimized dev profile
(`[profile.dev] opt-level = 2`) so the numbers described optimized code.

The shipped crate reuses exactly that stack: `lovelace-desktop/Cargo.toml` says "The new native
workbench (round-5 cutover) reuses the measured eframe/egui 0.37 and resvg 0.50 stack from
rust/lovelace-desktop-spike." The alternative candidates (iced, slint) were probed as minimal
window/text probes, not full spikes; that limitation is recorded as DEV-007 in
`docs/goal-desktop-v4/deviations.md` (carried into this repository).

## Decision

Ship the workbench on eframe/egui 0.37 with resvg 0.50 for plot rasterisation, and keep the GUI
stack Windows-only while the headless modes and the execution core stay platform-neutral
(`main.rs` declares `#[cfg(windows)]` for `ui`, `win32` and the windowed `main`).

## Consequences

- The UI is an immediate-mode retained-by-us model: the app owns its own editor widget, find bar,
  completion popup and accessibility mapping rather than a browser's.
- Accessibility goes through AccessKit → Windows UI Automation, which is what the `--shortcuts`,
  `--ui-audit` and GUI tests read (`lovelace-desktop/src/keyboard.rs:18`).
- A future Linux/macOS port keeps the engine, the runner and the headless modes and replaces only
  the shell.

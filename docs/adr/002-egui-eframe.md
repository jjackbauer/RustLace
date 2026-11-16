# ADR-002: egui/eframe as the native shell

**Status:** accepted — **platform scope superseded by [ADR-008](008-linux-gui-support.md)**. The shell
choice below still stands; the "Windows-only" clause does not. Read this record as history: it is
preserved unedited in substance so the reasoning stays auditable, and the two sentences that describe
the desktop as Windows-only are marked inline.

## Context

The desktop had to replace a browser IDE (CodeMirror 6 + an HTTP session API) with a native
application without losing editing, accessibility, SVG rendering or layout behaviour. The candidate
spike lived in `lovelace-desktop-spike`, a standalone package that is deliberately **not** a
workspace member (`lovelace-desktop-spike/Cargo.toml`: "Standalone spike: NOT a member of
rust/Cargo.toml"). It measured eframe/egui 0.37 and resvg 0.50 with an optimized dev profile
(`[profile.dev] opt-level = 2`) so the numbers described optimized code.

The shipped crate reuses exactly that stack: `lovelace-desktop/Cargo.toml` says "The new native
workbench (round-5 cutover) reuses the measured eframe/egui 0.37 and resvg 0.50 stack from
rust/lovelace-desktop-spike." The alternative candidates (iced, slint) were probed as minimal
window/text probes, not full spikes; that limitation is recorded as DEV-007 in `deviations.md` in
the LovelaceSharp repository.

## Decision

Ship the workbench on eframe/egui 0.37 with resvg 0.50 for plot rasterisation.

When this decision was taken the GUI stack was scoped to Windows, because the v4 desktop goal
deferred the Linux port (DEC-006). Only the Win32 legacy shell, the HWND-based IME probe and the
Windows platform report were treated as platform-specific, and the headless modes and the execution
core were kept platform-neutral so a port would not have to touch them.

> **Superseded by ADR-008.** The desktop is no longer Windows-only. The same UI code compiles and
> runs on Linux X11 as well, and the Win32-only parts are limited to `src/win32`, `--ime-probe` and
> `--platform-parity`. The original text here read "keep the GUI stack Windows-only …
> (`main.rs` declares `#[cfg(windows)]` for `ui`, `win32` and the windowed `main`)", which has not
> been true since the Linux port: `main.rs` now has `#[cfg(not(windows))]` arms for the windowed
> `main` and every mode that needs them.

## Consequences

- The UI is an immediate-mode retained-by-us model: the app owns its own editor widget, find bar,
  completion popup and accessibility mapping rather than a browser's.
- Accessibility goes through AccessKit. On Windows that surfaces as UI Automation, which is what the
  `--shortcuts`, `--ui-audit` and GUI tests read (`BINDINGS`).
- The prediction that closed this record — "a future Linux/macOS port keeps the engine, the runner
  and the headless modes and replaces only the shell" — held for Linux: the port replaced the
  renderer selection and the platform-specific modes, and left the engine, the runner and the
  headless modes untouched. macOS is still a later port.

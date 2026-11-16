# ADR-008: the native workbench runs on Linux as well as Windows

**Status:** accepted (supersedes the platform scope of [ADR-002](002-egui-eframe.md))

## Context

ADR-002 chose eframe/egui 0.37 and resvg 0.50 as the native shell, and scoped the GUI to Windows:
the v4 desktop goal deferred Linux (DEC-006 in `deviations.md` in the LovelaceSharp repository), so
only the Win32 legacy shell, the HWND-based IME probe and the Windows platform report were
platform-specific from the start.

That scoping no longer described the product. By v0.4.1 the workbench builds and runs on Linux:

- `.github/workflows/ci.yml` has a Linux job that compiles `lovelace-desktop`, runs the 20-check
  `--gui-selftest` under `xvfb-run`, runs the desktop test suite there, and uploads the binary.
- The release publishes `lovelace-desktop-linux-x86_64` beside the Windows executable, each with its
  own platform-qualified checksum file.
- `docs/images/gui-linux.png` is a capture of the shipped workbench under WSLg (provenance in
  `docs/images/README.md`).

Two facts drove the implementation:

1. eframe, egui, resvg and `raw-window-handle` all have Linux backends, so nothing in the GUI stack
   was ever Windows-only — only our use of it was.
2. eframe 0.37's default renderer is `wgpu`. Under WSLg there is no `/dev/dri`, so the
   XWayland/EGL path cannot acquire a wgpu device ("Parent device is lost"). The GLX path
   (mesa `llvmpipe`) does work, so the Linux build compiles eframe's `glow` backend and selects it
   explicitly.

## Decision

Support the workbench on **Windows x86_64 and Linux x86_64 (X11)** from the same binary, the same UI
code and the same `ExecutionHost` path.

- The renderer stays deliberately split: Windows keeps eframe's default `wgpu` renderer; non-Windows
  targets compile `glow` and `ui::native_options` selects it. Do not "unify" this — `wgpu` cannot
  acquire a device on the documented Linux path.
- Only genuinely Win32 code stays behind `cfg(windows)`: `src/win32`, the `--ime-probe` mode, and
  `--platform-parity`. The three modes are refused **by name**, with a non-zero exit, on Linux.
- The Linux build needs the X11/GL/font runtime packages listed in
  [`docs/build-and-run.md`](../build-and-run.md#linux), and `xvfb-run` for the windowless modes.
- What is certified is X11. **Wayland is not tested**, and under WSLg `WAYLAND_DISPLAY` must be
  unset or winit selects a Wayland socket WSLg does not provide.
- macOS remains a later port. No macOS host has been used, and no macOS claim is made.

## Consequences

- No current document may describe the desktop as Windows-only: not the README, not a crate page,
  not the language reference introduction, and not an agent instruction. Historical records that do
  (ADR-002, the v4 platform matrix) are marked as historical instead of silently rewritten.
- `.dsh/skills/release-and-publish/SKILL.md` releases **both** binaries. An instruction that says the
  desktop "must not be built or tested on Linux" is now not merely stale but destructive.
- The Linux artifact carries a glibc floor (built on Ubuntu 24.04) and is verified there with
  software rendering; no GPU driver is claimed for it.
- `--gui-selftest` passing is a run-path check on both platforms, not a GUI journey pass. Journey
  evidence is still captured on Windows only, and the platform matrix must say so.
- Accessibility evidence is Windows-specific: AccessKit surfaces as UI Automation, which is what the
  Windows `--shortcuts`, `--ui-audit` and platform-parity checks read. The Linux build inherits
  AccessKit's ability to run but has no equivalent acceptance evidence, and this ADR does not claim
  one.

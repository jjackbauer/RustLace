# Supported platforms — Lovelace Desktop

> **Release metadata, kept from goal-desktop-v4 / v1 and updated for the Linux promotion.** The table
> began as the v1 platform decision, when DEC-006 deferred the Linux port. [ADR-008](../adr/008-linux-gui-support.md)
> then promoted it, so the rows below record the current support statement: **Windows x86_64 and
> Linux x86_64 (X11) are supported**; Windows 10 x64 and macOS are not. See ADR-008 for the decision
> and the release notes for the released assets.
>
> **This file is machine-read.** `lovelace-desktop --platform-claims` and
> `lovelace-desktop/src/platform_claims.rs`'s tests parse the table below and enforce that every
> supported row carries run evidence and that both supported platforms are present. Changing a
> status or run-evidence cell is therefore a code change, not a prose change: `SUPPORTED_PLATFORMS`,
> `matrix_problems` and `unsupported_notice` in that module encode the same two platforms and must
> be updated in the same commit.

CON-009 requires an explicit published target matrix that distinguishes the initial release from
later ports, and forbids a platform claim without a built, run GUI test on it. This file is that
matrix. It is release metadata, not a promise: a row may say `LATER PORT` or `UNSUPPORTED` and that
is a complete, honest answer.

| Platform | Status for this release | Build evidence | Run GUI evidence | Notes |
|---|---|---|---|---|
| Windows 11 x64 (10.0.22631 or later) | **INITIAL RELEASE** | required: `cargo build -p lovelace-desktop` on the release commit | required: native window launched, screenshot captured, GUI suite executed | First certified platform of the v1 release. DPI: 100/150/200% are exercised by TST-014. |
| Windows 10 x64 | **UNSUPPORTED (untested)** | none | none | No Windows 10 host was available. Not claimed. |
| Linux x86_64 | **SUPPORTED (X11)** | CI: `cargo build --release -p lovelace-desktop` on Ubuntu 24.04 | CI: under `xvfb-run`, `--gui-selftest` runs 20 checks (0 failed) and `cargo test -p lovelace-desktop` runs the desktop suite. One window captured on WSLg: `docs/images/gui-linux.png`. | X11 only; **Wayland is untested** (under WSLg `WAYLAND_DISPLAY` must be unset). `--gui-selftest` is a run-path check, not a journey pass; journey evidence is Windows-only. glibc 2.39 floor; software rendering (llvmpipe). |
| macOS (arm64 and x86_64) | **LATER PORT** (not in v1) | none | none | No macOS host has been used; no macOS claim is made (ADR-008). |

## What the application must do on an unsupported platform

PLF-009 requires an explicit, non-silent error. The binary must:

1. fail to build the native shell on a platform without a UI adapter, and
2. when a headless build is asked to start the UI, print a message that names the platform as
   unsupported and exits non-zero — never launch a headless placeholder or pretend to work.

## Rules

- A row changes to a supported status only when a GUI test artifact from that platform is attached to
  the gate evidence for CON-009/PLF-010.
- Cross-compilation alone never changes a row: CON-009 requires a **run** test.
- This matrix is read by `lovelace-desktop --platform-claims` and by the tests in
  `lovelace-desktop/src/platform_claims.rs`.

---
name: add-acceptance-mode
description: Add a headless acceptance mode or desktop journey that drives the same Workbench path as the window and produces a reproducible artifact.
whenToUse: Use when a desktop behaviour needs an automated acceptance check, a new journey row, or a command that a build host can run without a human at the keyboard.
---

# Add an acceptance mode or journey

Acceptance code is a driver of the product, not a second product. Every new mode must call the
same `Workbench` methods the window calls (`run_active`, `poll`, `wait_until_idle`) and read the
typed `DesktopEvent` stream; it must not evaluate a script inline or reconstruct a value from a
display string. A headless pass is not a GUI journey pass: a journey also needs a captured artifact
bound to the commit.

## Files you will touch

- `lovelace-desktop/src/main.rs` - argument dispatch, both the Windows and non-Windows blocks, and
  the `usage()` text.
- `lovelace-desktop/src/gui_selftest.rs` - the `--gui-selftest` check list and the `GUI_SELFTEST`
  output contract.
- `lovelace-desktop/src/journey.rs` - the `--journey` IDs, `Options`, `needs_output_dir`, the match
  arms and the per-journey functions.
- `lovelace-desktop/src/selftest.rs` - the older `--selftest` core checks, if the mode belongs there.
- `lovelace-desktop/src/workbench.rs` - the headless document state the driver calls.
- `docs/goal-desktop-v4/journeys.md` - the executable journey specification.
- `docs/images/capture.ps1` - the real-window UIA and PrintWindow driver.
- `docs/crates/lovelace-desktop.md`, `docs/testing.md` and `README.md` - the mode table and counts.

## Procedure

1. **Choose the shape.** A `--gui-selftest` check is a small function in
   `lovelace-desktop/src/gui_selftest.rs` added to the `checks` array; it prints one line per check
   and exits non-zero on failure. A `--journey` is a row in `docs/goal-desktop-v4/journeys.md` plus a
   function in `lovelace-desktop/src/journey.rs` that asserts the row's expected observable and
   writes any artifact. A new standalone mode gets a flag, a dispatch branch and a module. Prefer a
   journey when the requirement is a user-visible journey; prefer a check when it is a run-path
   invariant.

2. **Add the dispatch.** In `lovelace-desktop/src/main.rs`, add the flag before the unknown-option
   check and before the windowed launch, and mirror it in the `#[cfg(not(windows))]` block if it can
   run without a window. Print the flag in `usage()` with its arguments. A mode that needs a window
   must be rejected with exit code 2 on the non-Windows build, as `--journey` is.

3. **Drive the workbench.** Construct `Workbench::new()`, set the source with `set_source`, call
   `run_active()` and wait with `wait_until_idle(RUN_TIMEOUT)`. Read the published
   `WorkspaceSnapshot` and the document's log; never call the engine directly. Use the helpers in
   `lovelace-desktop/src/gui_selftest.rs` (`snapshot`, `log_text`) as the model.

4. **Print a parseable contract.** Each check prints one line, and the mode ends with a summary:
   `GUI_SELFTEST summary checks=<n> failed=<n>` or `JOURNEY summary journeys=<n> failed=<n>`. The
   process returns 0 only when every item passes, 1 on a failed assertion and 2 on a usage error.
   Do not turn a mismatch into a warning; a journey expectation is never relaxed to make it pass.

5. **Write the artifact.** A mode that produces evidence takes `--out <dir>` and writes a
   deterministic file, then prints its path, byte count and SHA-256. For a journey, add the ID to
   `needs_output_dir` so the directory is created before evaluation; `J-09` copies its engine SVGs
   and `J-15` writes a PNG raster and the exported SVG this way. Use a filename that names the
   journey and the run (`J-09-run1.svg`, `J-15-plot-2x.png`), never a timestamp.

6. **Document the journey row.** Add a row to `docs/goal-desktop-v4/journeys.md` with the input,
   actions, expected observable, reference capture and status. A row stays `NOT_TESTED` until an
   artifact exists; a headless `JOURNEY ... PASS` line alone does not make it green. If the row needs
   a new reference, capture it into `docs/goal-desktop-v4/reference/` as `REF-<topic>.json` and cite
   it from the row.

7. **Capture the GUI half.** When the mode must be proven in the real window, extend
   `docs/images/capture.ps1` (its `ValidateSet` and `$scenarios`) with a scenario that launches the
   release binary, drives controls by their UIA names, waits for the observable and calls
   `Save-Frame`. The script prints the PNG bytes and SHA-256. A UIA dump and a PrintWindow frame are
   the evidence pair; put them beside the run's evidence, not in the repository unless they document
   a shipped state.

8. **Test the mode itself.** Add an integration or unit test in `lovelace-desktop/src/` that asserts
   the exact output line and exit code, and run the mode from a writable `APPDATA`. If it exercises a
   journey, add the id to the validated list in `journey.rs`; an unknown id is a usage error by
   design.

9. **Update the docs table.** Add the mode to `docs/crates/lovelace-desktop.md` and, if the count of
   checks changed, to `docs/testing.md` and `README.md`. Keep the wording tied to the observed output
   rather than a remembered count.

## Verify

```console
cargo run -p lovelace-desktop -- --journey J-02 --out .tmp/journey
cargo run -p lovelace-desktop -- --gui-selftest
cargo run -p lovelace-desktop -- --help
cargo test -p lovelace-desktop
```

Observable success: the journey prints `JOURNEY J-02 PASS` and
`JOURNEY summary journeys=1 failed=0`, exits 0, and `.tmp/journey` contains the artifact named in
step 5 with the printed SHA-256; `--gui-selftest` ends with
`GUI_SELFTEST summary checks=20 failed=0` (or the new count) and exits 0; `--help` lists the new flag;
the desktop suite passes.

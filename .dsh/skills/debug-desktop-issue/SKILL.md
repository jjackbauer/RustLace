---
name: debug-desktop-issue
description: Reproduce a lovelace-desktop defect, capture UI Automation and PrintWindow evidence, fix it in the owning layer and prove the fix with a failing-first regression test.
whenToUse: Use when a desktop symptom is reported, a pane paints the wrong state, a control is unreachable or clipped, or a headless mode passes while the real window behaves differently.
---

# Debug a desktop issue

A desktop fix needs three things: a reproduction against the real release binary, evidence that is
independent of the reporter's screen (a UIA tree and a PrintWindow frame), and a regression test that
fails on the old code and passes on the new. A fix that only "looks right" in one screenshot is not
evidence.

## Files you will touch

- `lovelace-desktop/src/ui/app.rs` - paint, layout, focus and the `ui::app::tests` module.
- `lovelace-desktop/src/workbench.rs` and `lovelace-desktop/src/execution.rs` - the headless state
  and typed event stream when the defect is not purely paint.
- `lovelace-desktop/src/keyboard.rs` - focus groups and bindings.
- `lovelace-desktop/src/ui/a11y.rs` - accessible roles, names and the focus ring.
- `lovelace-desktop/src/ui/theme.rs` - the only module allowed to construct a colour.
- `docs/images/capture.ps1` - the existing UIA and PrintWindow driver to copy from or extend.
- `docs/crates/lovelace-desktop.md` - the behaviour description when it was wrong.

## Procedure

1. **Reproduce on the release binary, not a debug guess.** Build with
   `cargo build --release -p lovelace-desktop`, point `APPDATA` at a scratch directory outside the
   repository, launch the binary with the script that triggers the defect and record the exact
   window size, scale, theme and steps. A debug build can change timing and layout; reproduce on the
   same profile a user runs.

2. **Dump the UIA tree.** Attach to the live window with UI Automation, enumerate every descendant
   and write one row per node: control type, name, automation id, enabled state and bounding
   rectangle. This is the tree a screen reader consumes; a missing name or an off-window rectangle is
   the defect made visible. The helper functions in `docs/images/capture.ps1` (`Get-All`,
   `Get-Names`, `Find-Element`, `Wait-Status`) are the working reference.

   ```powershell
   Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
   $root = [System.Windows.Automation.AutomationElement]::FromHandle($h)
   $all = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants,
       [System.Windows.Automation.Condition]::TrueCondition)
   foreach ($e in $all) {
       $c = $e.Current
       "{0} | {1} | {2} | enabled={3} | {4}" -f `
           $c.ControlType.ProgrammaticName, $c.Name, $c.AutomationId, $c.IsEnabled, $c.BoundingRectangle
   }
   ```

   Save the output as `uia.txt` beside the frame. If the window has recreated its OS handle, resolve
   it again from the process id, as `Get-LiveHandle` does.

3. **Capture the frame with PrintWindow.** Do not use `CopyFromScreen`; a locked or occluded session
   is exactly when the evidence is needed. Create a bitmap the size of the window, get its HDC, call
   `PrintWindow($h, $dc, 2)` (`PW_RENDERFULLCONTENT`), save the PNG and record its size and SHA-256.
   The `Save-Frame` function in `docs/images/capture.ps1` already implements the sizing, retry and
   `PrintWindow` call; extend its scenario list rather than inventing a second capture path.

4. **Narrow the layer before editing.** If the workbench path is suspected, run
   `cargo run -p lovelace-desktop -- --gui-selftest` and the focused unit tests. If a run event is
   suspect, set `LOVELACE_RUN_TRACE` to a file and read the run marks. If the defect needs a state a
   script cannot reach, use the existing acceptance seams (`LOVELACE_LOG_FLOOD`,
   `LOVELACE_ENGINE_FAULT`, `LOVELACE_PLOT_FAULT`) and remove them from the reproduction once the
   state is captured. `--ui-audit` tells you whether the UI is reaching into the engine, which is a
   design bug even when it "works".

5. **Fix in the owning layer.** A paint or layout defect belongs in `lovelace-desktop/src/ui/`;
   a run-state defect belongs in `workbench.rs` or `execution.rs`; a missing name belongs in the
   widget or `lovelace-desktop/src/ui/a11y.rs`. The UI must not evaluate, construct an engine or
   compile. All colours go through `lovelace-desktop/src/ui/theme.rs` and all text sizes through
   `lovelace-desktop/src/ui/type_scale.rs`.

6. **Write the regression test first.** Add a test in the module that owns the logic - the
   `ui::app::tests` module for paint/layout, `workbench.rs` tests for headless state - with the same
   concrete input as the reproduction and a concrete assertion (a rectangle inside the client, a
   painted string, an accessible name, a state value). Run it against the old code and record that it
   fails for the stated reason; then apply the fix and run it again. A test that passes before the
   fix proves nothing.

7. **Re-capture and compare.** Run the same UIA dump and the same PrintWindow scenario after the fix.
   The evidence pair is the before and after `uia.txt` plus the two PNGs and their hashes. If the
   behaviour is part of the shipped product, update `docs/images/` and `docs/images/README.md`; if it
   is a one-off defect, keep the captures outside the repository and attach them to the issue. Never
   commit `%APPDATA%` state or a build output.

8. **Check for a second symptom.** Search the UIA dump for other controls with the same missing name,
   the same zero or off-window bounds, or the same disabled state. A layout bug usually appears at
   more than one scale; run the `lay_011` sizes (900x600 and 1280x800 at 100/150/200%) before
   declaring it fixed.

## Verify

```console
cargo test -p lovelace-desktop <regression_test_name>
cargo test -p lovelace-desktop
cargo run -p lovelace-desktop -- --gui-selftest
cargo run -p lovelace-desktop -- --journey J-02 --out .tmp\journey
cargo run -p lovelace-desktop -- --ui-audit
```

Observable success: the regression test fails on the parent commit and passes after the fix; the
full desktop suite reports `716 passed; 0 failed` (or more with the new test); `--gui-selftest` ends
with `failed=0`; the journey prints `JOURNEY J-02 PASS`; `--ui-audit` exits 0; and the after UIA dump
names the control with a rectangle inside the client while the after PNG shows the corrected state,
both with the recorded SHA-256.

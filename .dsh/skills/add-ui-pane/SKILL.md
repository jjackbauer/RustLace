---
name: add-ui-pane
description: Add or change a workbench pane across all layout modes with theme tokens, an empty state, keyboard focus, accessibility and captured evidence.
whenToUse: Use when adding a pane or a visible region to lovelace-desktop, or when changing a pane's layout, title, empty state, focus order or colours.
---

# Add a workbench pane

The central workbench has five panes today: Editor, Graph, Workspace, Logs and the Symbolic
Inspector; the File bar, Toolbar, tab strip and status bar are chrome around them. A new pane must
exist in all three layout modes, paint only from theme tokens, explain its empty state, be
reachable by F6, and appear in the accessibility tree. A pane that only paints in the 1600x1000
split is not done.

## Files you will touch

- `lovelace-desktop/src/keyboard.rs` - `Pane`, `Pane::ALL`, labels, F6 bindings and the binding
  tests.
- `lovelace-desktop/src/ui/app.rs` - `LayoutMode`, `PaneLayout`, `CompactPane`, `compute_layout`,
  `compute_compact_layout`, `panes`, `compact_panes`, `with_pane` and the pane paint methods.
- `lovelace-desktop/src/ui/empty.rs` - the empty state and its `catalog`.
- `lovelace-desktop/src/ui/theme.rs` - the token table and contrast pairs.
- `lovelace-desktop/src/ui/type_scale.rs` - the text roles.
- `lovelace-desktop/src/ui/a11y.rs` - the accessibility helpers and the focus ring.
- `docs/images/capture.ps1` - the PrintWindow/UIA capture driver.
- `docs/crates/lovelace-desktop.md` - the pane/mode description.

## Procedure

1. **Decide what kind of pane it is.** Read `LayoutMode` and `PaneLayout` in
   `lovelace-desktop/src/ui/app.rs`. A pane in the split belongs in `PaneLayout` and
   `compute_layout`; an ancillary pane that should collapse into the compact drawer also belongs in
   `CompactPane` and `compute_compact_layout`. The compact mode is a real supported layout, not a
   fallback that may omit the new pane.

2. **Add the focus group.** Add a variant to `Pane` in `lovelace-desktop/src/keyboard.rs`, add it to
   `Pane::ALL` in the intended F6 order, and give it a label. Check the F6 and Ctrl+1..5 bindings
   and their assertions; the tests at the bottom of the file encode the wrap order and the group
   count. If the pane has an interactive control, make that control the group's first focus target.

3. **Lay it out in every mode.** Add the rectangle to `PaneLayout` and compute it in
   `compute_layout` (with a minimum size and a splitter if it is resizable). For the compact mode,
   add the drawer variant to `CompactPane`, give it a tab label, and handle it in
   `compute_compact_layout` and `compact_panes`. Clamp every rectangle inside the client and keep
   the editor at or above the documented share.

4. **Paint through `with_pane`.** Call the `with_pane` helper in `lovelace-desktop/src/ui/app.rs`
   for the frame, header and clip. It registers the pane with `egui::Role::Pane` and an accessible
   label, paints the panel background, border and header from tokens, and gives the body a rect.
   The body reads `DocumentView` / `WorkspaceSnapshot`; it must never construct an engine, evaluate
   or compile.

5. **Use tokens, not colours.** Every colour comes from an accessor in
   `lovelace-desktop/src/ui/theme.rs`. `theme.rs` is the only module in `lovelace-desktop/src/ui/` allowed to construct a
   `Color32`, and a test enforces that. If the pane needs a new meaning, add a field to `Tokens`,
   fill it in `DARK` and `LIGHT`, add the accessor, and add the pair to the contrast `checks` list
   so `--contrast-check` measures it. Text sizes come from a `Role` in
   `lovelace-desktop/src/ui/type_scale.rs`; never pass a numeric font size.

6. **Write the empty state.** Add a constructor to `lovelace-desktop/src/ui/empty.rs` with a stable
   key, headline, purpose, reason, next step and, where one applies, an `EmptyAction`. Register it
   in `catalog()`. The pane must state what it is for, why it is empty and what to do next; the
   existing tests assert that every state carries those facts.

7. **Expose accessibility.** `with_pane` supplies the pane role and name. Interactive controls must
   be real egui widgets so AccessKit publishes role, name, value and state; use the helpers in
   `lovelace-desktop/src/ui/a11y.rs` for the focus ring and localisation. Add the pane's title to
   the assertions that read `accesskit_rows`.

8. **Update the layout tests.** Extend
   `lay_011_adaptive_layout_keeps_every_pane_inside_every_supported_client` for the new rectangle at
   900x600 / 1280x800 at 100/150/200%, and
   `lay_002_the_five_panes_are_titled_and_own_their_empty_and_populated_states` for its title and
   empty state. Update `a11y_002_every_control_exposes_role_name_value_and_state` if the pane adds
   controls. These tests read the frame the window actually painted, not a hand-built rect.

9. **Capture evidence.** Extend the `ValidateSet` of `docs/images/capture.ps1`, add the scenario to
   its `$scenarios` list and write the scenario with `Start-App`, `Invoke-Control`, `Set-EditorText`
   and `Save-Frame`. Build the release binary, pass `-Exe` if the default target directory is not
   `target-release`, redirect `APPDATA` with `-WorkRoot`, and run the scenario. `Save-Frame` uses
   `PrintWindow` with `PW_RENDERFULLCONTENT` and records the PNG size and SHA-256, so the frame is
   real evidence. Commit a new PNG to `docs/images/` only when it documents the shipped state and
   update `docs/images/README.md`.

## Verify

```console
cargo test -p lovelace-desktop lay_011
cargo test -p lovelace-desktop lay_002
cargo test -p lovelace-desktop a11y_002
cargo run -p lovelace-desktop -- --ui-audit
cargo run -p lovelace-desktop -- --contrast-check
cargo run -p lovelace-desktop -- --gui-selftest
powershell -NoProfile -ExecutionPolicy Bypass -File docs/images/capture.ps1 -Scenario my-pane -Exe target/release/lovelace-desktop.exe -WorkRoot .tmp/capture
```

Observable success: `lay_011`, `lay_002` and `a11y_002` pass; `--ui-audit` exits 0 with every file
PASS; `--contrast-check` prints "every required pair passes"; `--gui-selftest` keeps
`failed=0`; the capture prints `IMAGE my-pane <bytes> bytes <w>x<h> sha256=<hash>` with bytes above
the script's minimum, and the PNG shows the pane title and its empty state.

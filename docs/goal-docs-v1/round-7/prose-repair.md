# Round 7 — prose repair: brittle source citations

- Repository: RustLace, `HEAD` `bb9d21a` (`docs: describe the Linux checksum file accurately`); the
  worktree already carried the uncommitted earlier rounds, so `git diff` below includes that work.
- Files repaired: `docs/architecture.md`, `docs/glossary.md`, `docs/extending.md`, `docs/testing.md`,
  `docs/parity-with-studio.md`, `docs/editorial.md`, `docs/adr/002-egui-eframe.md`,
  `docs/adr/008-linux-gui-support.md`.
- Nothing outside those files was edited. No fenced block tagged `lovelace` or `text` was touched in
  any of them.
- Rule applied: a cited line that defines a symbol the prose did not already name becomes that
  symbol; a line that is crate/module documentation (no symbol to name) becomes the bare file path;
  where the prose already names the symbol defined on the line, the now-redundant locator is removed
  rather than duplicated (editorial rule 4: cite the symbol, not the line).
- Before: the checker listed 32 + 19 + 10 + 3 line-suffix citations (`architecture`, `glossary`,
  `extending`, `testing`), 6 line-suffix citations and 1 code reference in `parity-with-studio.md`,
  1 line-suffix citation in `editorial.md`, and the two `deviations.md` code references (one in each
  ADR). The `keyboard.rs` line citation in ADR-002 was checker-permitted and is repaired too.

## Verification

### 1. The required filtered checker command prints nothing

```console
$ bash tools/check-docs.sh --symbols 2>&1 | grep -E 'architecture\.md|glossary\.md|extending\.md|testing\.md|parity-with-studio\.md|editorial\.md|adr/002|adr/008'
$ echo $?
1
```

No output at all. The pipeline status is `1` because `grep` matched no line (verified separately:
`filtered-lines=0`, `filtered-exit=1`); the checker itself exits `0`.

### 2. The whole checker is green

```console
$ bash tools/check-docs.sh --symbols
check-docs: 55 Markdown files, 147 relative links, 199 code references, 20 line-suffix citations, 17 symbol citations
check-docs: OK
$ echo $?
0
```

The 20 remaining line-suffix citations are all under `docs/adr/`, which the checker permits by
policy; the filter in §1 proves none of them is in a file owned by this round.

### 3. `git diff --stat` for the owned files

`docs/editorial.md` and `docs/adr/008-linux-gui-support.md` are new, untracked files from an earlier
round, so `git diff --stat` cannot show them; `git status` is included for those two.

```console
$ git diff --stat -- docs/architecture.md docs/glossary.md docs/extending.md docs/testing.md docs/parity-with-studio.md docs/editorial.md docs/adr/002-egui-eframe.md
 docs/adr/002-egui-eframe.md |  41 +++--
 docs/architecture.md        | 351 ++++++++++++++++++------------------
 docs/extending.md           | 203 +++++++++++-----------
 docs/glossary.md            |  58 +++---
 docs/parity-with-studio.md  | 135 +++++++-------
 docs/testing.md             | 425 +++++++++++++++++++++++---------------------
 6 files changed, 621 insertions(+), 592 deletions(-)

$ git status --short -- docs/architecture.md docs/glossary.md docs/extending.md docs/testing.md docs/parity-with-studio.md docs/editorial.md docs/adr/002-egui-eframe.md docs/adr/008-linux-gui-support.md
 M docs/adr/002-egui-eframe.md
 M docs/architecture.md
 M docs/extending.md
 M docs/glossary.md
 M docs/parity-with-studio.md
 M docs/testing.md
?? docs/adr/008-linux-gui-support.md
?? docs/editorial.md
```

Two artefacts inflate the plain stat and neither is this round's edit: (a) the worktree uses CRLF
while the index uses LF, so every line of `architecture.md`, `glossary.md`, `extending.md`,
`testing.md` and `parity-with-studio.md` differs; (b) the earlier uncommitted rounds changed prose
in the same files. The line-ending-free stat isolates the content changes:

```console
$ git diff --stat --ignore-cr-at-eol -- docs/architecture.md docs/glossary.md docs/extending.md docs/testing.md docs/parity-with-studio.md docs/adr/002-egui-eframe.md
 docs/adr/002-egui-eframe.md | 41 ++++++++++++++++++---------
 docs/architecture.md        | 67 +++++++++++++++++++++++----------------------
 docs/extending.md           | 31 +++++++++++----------
 docs/glossary.md            | 36 ++++++++++++------------
 docs/parity-with-studio.md  | 15 +++++-----
 docs/testing.md             | 35 +++++++++++++++--------
 6 files changed, 127 insertions(+), 98 deletions(-)
```

## Conversions and corrections, one line per file

- `docs/architecture.md` — **32 citations converted**: six value-layer crate locators to bare paths; `Engine`/`Engine::eval`/`eval_text`, `Engine::new` (the 1000 default), `cancelled_error`, `InFlightCancellation`, `ExecutionHost`, `apply_cached`, `ENTRY_LIMIT`, `app_dir_from`, `default_plot_dir`, `document_io::write`, `RecoveryRecord::write_atomic`; redundant locators for already-named `Engine::display_precision`, `Engine::set_cancel_after`/`set_cancel_flag` and `StatementKey` removed. **Two sentences needed a corrected citation, not a prose rewrite**: `lib.rs:190` is `CANCELLED_MESSAGE`, while the `"Cancelled"`/`"BudgetExceeded"` construction the prose describes is `cancelled_error` (lib.rs:195–196); `execution.rs:112` is a blank line, while the two named in-flight outcomes are `InFlightCancellation` (execution.rs:121–129) — both now cite the right symbol.
- `docs/glossary.md` — **19 citations converted**: seven crate/table locators to bare paths
  (`lovelace-nat`, `-int`, `-real`, `modus`, `incremental`, `recovery`, `corpus` main) plus `Value`,
  `Value::kind_name`, `Engine::display_precision`, `Engine::new`, `DEFAULT_PRECISION`, `ModusPlugin`,
  `WorkspaceSnapshot::committed_steps`, `eval_text`, `RunId`/`DocumentId`/`SourceRevision`,
  `WorkspaceSnapshot`, `document_io::write`, and the gate test
  `fresh_defaults_are_thirty_digits_and_inside_the_fixed_width_gate`. One re-point:
  `execution.rs:55` sits inside the `WorkspaceSnapshot` doc, not on a definition, so the
  committed-prefix row now cites its field `WorkspaceSnapshot::committed_steps` (execution.rs:99).
  No prose changed.
- `docs/extending.md` — **10 citations converted**: `BUILTINS`, `Pane`, `ui_root`, `no_paint_module_contains_a_hard_coded_colour`, `ErrorCategory`, `LangError`, `record_all`, and bare path `lovelace-lang/src/registry.rs`; locators for the already-named `pub fn call`, `ModusPlugin` and `load_plugin` removed. **One sentence had a wrong citation**: the message/position contract was attributed to `lib.rs:1` (crate docs), but it is stated on `LangError`'s fields (lib.rs:94–100) — now cites `LangError`; the `app.rs:3864`–`3909` range started at the F8 handler and is now `ui_root`, the function that owns the five panels (they begin at line 3876).
- `docs/testing.md` — **3 citations converted**: `document_io.rs:856,886`, `workbench.rs:6276` and `netcheck.rs:238` all pointed at the `#[cfg(windows)]` attribute line immediately above the test, so each became the test name (`a_failed_rename_keeps_the_read_only_original_byte_identical`, `a_successful_save_preserves_the_targets_file_attributes`, `a_failed_atomic_save_keeps_the_original_and_its_recorded_snapshot`, `this_test_binary_imports_no_networking_dll`). No prose changed.
- `docs/parity-with-studio.md` — **6 line citations + 1 code reference converted**: `Engine`, bare `lovelace-lang/src/modus.rs`, `lovelace-run/src/main.rs` and `lovelace-desktop/src/execution.rs`, `DEFAULT_PRECISION`, `InFlightCancellation`; the external `Lovelace.Suite/docs/Language.md` now reads "the C# suite's `Language.md` in the LovelaceSharp repository" (an artefact of the original suite repository, not a path in this tree). No prose meaning changed; `execution.rs:112` was the same blank-line pointer as in `architecture.md`.
- `docs/editorial.md` — **1 citation converted**: the rule-4 anti-pattern example `builtins.rs:5748` became "a `file:line` citation such as `builtins.rs:NNN`", so it still teaches the rule without looking like a live citation. No other prose changed.
- `docs/adr/002-egui-eframe.md` — **1 code reference + 1 line citation converted**: DEV-007 now lives in "`deviations.md` in the LovelaceSharp repository" (the false "(carried into this repository)" was dropped — the file is not here); `lovelace-desktop/src/keyboard.rs:18` is module documentation naming the table, not a historical line, so it became `BINDINGS`. Contract/decision text unchanged.
- `docs/adr/008-linux-gui-support.md` — **1 code reference converted**: DEC-006 now lives in "`deviations.md` in the LovelaceSharp repository". Contract/decision text unchanged.

## Wrong citations found (all repaired)

1. `docs/architecture.md` — `lovelace-lang/src/lib.rs:190` was `CANCELLED_MESSAGE`, not the coded
   `Cancelled`/`BudgetExceeded` error the sentence describes (`cancelled_error`, 195–196).
2. `docs/architecture.md` and `docs/parity-with-studio.md` — `lovelace-desktop/src/execution.rs:112`
   is blank; the in-flight outcome type is `InFlightCancellation` at 121.
3. `docs/extending.md` — `lovelace-lang/src/lib.rs:1` (crate docs) did not document the
   complete-sentence/UTF-16-position contract; `LangError`'s fields do (94–100).
4. `docs/extending.md` — `ui/app.rs:3864`–`3909` began at the F8 diagnostic-jump handler, not at the
   five-panel shell (`ui_root`, panels from 3876).
5. `docs/glossary.md` — `execution.rs:55` is a doc sentence inside `WorkspaceSnapshot`, not the
   committed-prefix item; the prefix count is the `committed_steps` field.
6. `docs/adr/002-egui-eframe.md` — `docs/goal-desktop-v4/deviations.md` claimed to be "carried into
   this repository"; it is not in this repository (DEV-007 lives in the LovelaceSharp repository).

## Not converted

None. Every failure the checker reported for these eight files is gone; the only remaining line
citations in the repository are the 20 under `docs/adr/` that the checker deliberately permits. The
one checker-permitted line citation in this set (`keyboard.rs:18`) was replaced anyway because that
line is not the historical subject.

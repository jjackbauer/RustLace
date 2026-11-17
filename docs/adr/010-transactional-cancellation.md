# ADR-010: Discard the in-flight statement on cancellation

**Status:** accepted (2026-11-17)

## Context

`COR-016` requires cancellation or failure not to leave a partially committed workspace change.
The desktop has two cancellation observations:

- **statement boundary** — the web Studio's only observation point. A statement that has already
  completed is committed (`InFlightCancellation::Completed`); `REF-cancel.json` captures this case.
- **live kernel flag** — `RUN-008`; an in-flight statement can stop early
  (`InFlightCancellation::Interrupted`). Before this ADR, the desktop committed the early-exit state
  "like the web Studio", including the loop's partial variables and print lines.

Committing the interrupted state contradicts `COR-016`: a cancelled `while` loop published a
partially advanced counter, and a cancelled kernel's partial output could appear in the workspace.

## Decision

1. On `InFlightCancellation::Interrupted`, **do not commit the statement**. Publish only the
   previously committed prefix; drop the run's engine (and therefore its partial variables) and do
   not drain the interrupted statement's output or plot.
2. Keep the statement-boundary `Completed` commit unchanged; `REF-cancel.json` remains a valid
   capture of that distinct case and its parity test still passes.
3. Make the settings/session write atomic through the same temporary-file-and-rename path as
   document saves, so a failed write cannot leave an unreadable settings file.
4. Update the RUN-008 tests and the workbench log text: the terminal reason is "the in-flight
   statement was discarded at the cancellation flag and only the committed prefix was kept".

## Consequences

- `cancel_parity.rs::an_interrupted_in_flight_statement_is_rolled_back...` now expects 2 committed
  statements, `c = 0`, no `StepCommitted(2)` event and no `d`.
- The web Studio parity test is unaffected because its statement completed before cancellation; the
  module documentation now names the two cases explicitly.
- A cancelled run's cache receives no entry for the interrupted statement, and the UI ends in
  `RunState::Cancelled` with the committed prefix only.
- The cost is that work completed inside an interrupted statement is not shown; this is the
  intended transactional trade-off for `COR-016`.

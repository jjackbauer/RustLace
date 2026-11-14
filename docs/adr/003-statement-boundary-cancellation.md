# ADR-003: cancellation stops at a statement boundary

**Status:** accepted

## Context

The web Studio's run loop observes cancellation only between top-level statements: an in-flight
statement always completes and is committed. The desktop port (CON-003) has to preserve behaviour
and data semantics, not the browser implementation, so the question was whether to invent a finer
cancellation granularity.

## Decision

Two seams, with the statement boundary as the contract:

1. Every top-level statement is preceded by a cancellation check
   (`lovelace-lang/src/lib.rs:1595`–`1599`: "Statement granularity, exactly as the reference's
   interpreter checks: a script whose budget expires between two statements stops before the next
   one runs"). The stopped run publishes the position of the statement that did not start and a
   `LangError` with code `Cancelled`, category `BudgetExceeded` (`lib.rs:190`).
2. Long kernels poll the same token between limbs or iterations – `pow`/`factorial`
   (`lovelace-lang/src/cancellable.rs`), `sqrt`/`pi` and the transcendental kernels – so a kernel can
   stop early *in addition* to the boundary rule.

The in-flight statement therefore has two possible outcomes, and the desktop names them instead of
pretending they are one (`lovelace-desktop/src/execution.rs:112`–`120`): completed normally, or
stopped early by the live flag. A kernel with no poll seam (a single `sqrt` at very high precision)
is documented as uninterruptible rather than silently budgeted.

## Consequences

- The committed prefix is always a whole number of statements; the incremental cache and the
  workspace snapshot are consistent with it.
- Cancellation latency is bounded by the kernel, not by the UI: Cancel is available while a run is
  active (`ui/app.rs` paints it with the tooltip "Cancel (Esc): stop the active run at the next
  statement boundary"), and the status strip says so.
- The contract is testable: `cancel_parity` and the workbench tests assert which statements remain
  committed and what the in-flight one reports.

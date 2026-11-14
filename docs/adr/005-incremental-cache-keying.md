# ADR-005: the incremental cache is keyed on full canonical strings

**Status:** accepted

## Context

The web Studio's `IncrementalRunner` avoided recomputing unchanged top-level statements. A naive port
could key on a hash of the statement text, but two hazards follow: a digest collision reuses a
different statement, and a display-form key misses the fact that two values that *print* the same can
be different typed values (for example a `Real` and a truncated rendering of a longer `Real`).

## Decision

One `StatementKey` holds four full canonical strings, never digests
(`lovelace-desktop/src/incremental.rs:9`–`28`):

1. the normalized statement source — comments removed, whitespace runs collapsed, whitespace dropped
   next to punctuation, string literals copied verbatim;
2. the submitted document precision **and** the effective precision the engine reported when the
   statement was planned, so a precision change reuses nothing and a mid-script `setprecision`
   changes the effective half;
3. ordered dependency keys — for each global the statement reads, `name -> canonical typed value
   key`, or `UNDEF`, derived from the typed value rather than its display form;
4. function keys — `builtin` or `rev:<n>` per called function, with the revision bumped only when a
   `func` body's normalized source changes.

Statements that mention a side-effecting builtin (`print`, `plot`, `setprecision`, the `assume*`
family, `assumptions`, `clear`) are never cached (`incremental.rs:75`–`94`), and a cache hit is
reported as reused only after the cached typed value was applied to the fresh run engine
(`incremental.rs:58`).

## Consequences

- Over-approximating dependencies can only cost a reuse, never serve a stale value
  (`incremental.rs:44`).
- The cache is per document and bounded at 4096 entries (`incremental.rs:111`); eviction is
  deterministic and can only cause recomputation.
- The key strings are larger than digests; the trade is deliberate and testable.

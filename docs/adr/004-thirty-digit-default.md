# ADR-004: 30-digit default precision in the desktop

**Status:** accepted (user-directed deviation DEV-009)

## Context

The Rust engine's own default is a 1000-digit computation budget
(`lovelace-lang/src/lib.rs:836`). The C# extension host's untouched knob was **30 computation / 15
display** digits, and 30 sits inside the shared fixed-width gate that is `<= 37` digits
(`lovelace_lang::complex::FAST_PATH_MAX_DIGITS`); at 1000 every precision-selected fast path is
skipped.

## Decision

`lovelace-desktop` defaults a fresh settings document and a new document to **30** digits
(`lovelace-desktop/src/engine.rs:45`–`54`, `pub const DEFAULT_PRECISION: usize = 30`). A persisted
`active_precision` keeps its stored value: loading settings never rewrites it, so an installation
that stored 1000 keeps 1000 (`precision_default.rs:82`–`93`).

The choice is evidenced three ways in `lovelace-desktop/src/precision_default.rs`:
the constant and the fresh-default paths; a **representation witness** — `movingavg([2.5], 3)`
renders the reference's fixed-width `0.(8)` at 30 and 37 and the arbitrary-precision `0.8(3)` at 38
and 1000 — and a timing observation that `sqrt(2)` at 30 digits is materially faster than at 1000.

## Consequences

- The desktop matches the extension host's budget rather than the engine's raw default.
- No settings migration: the schema is unchanged, and only fresh defaults moved.
- Revisit if parity with a host that wants 1000 digits is required; the constant is the single knob.

---
name: change-numeric-kernel
description: Change lovelace-real or lovelace-nat arithmetic safely by preserving the precision contract, periodic-decimal representation and canonical limb invariants, with regression tests, corpus evidence and a digest-stable benchmark.
whenToUse: Use when changing Real or Nat arithmetic, division, rounding, periodic-decimal handling, square root, pi, NTT or division thresholds, or any code that can change a stored numeric value or its rendering.
---

# Change the numeric kernel

Numeric behaviour is a contract twice over: the stored value must stay canonical, and the rendered
text must stay exact. The engine has no floating point in a value path. A change is not proven by a
faster benchmark; it is proven by the same digits and the same period metadata before and after, and
by a digest that does not move.

## Files you will touch

- `lovelace-real/src/lib.rs` - `Real`, parsing, `add`/`sub`/`mul`/`div`, period metadata and the
  display cap.
- `lovelace-real/src/transcendental.rs` - `sqrt` and `pi`.
- `lovelace-real/src/round34_inexact_division.rs`, `lovelace-real/src/real_parity_round7.rs` and the
  other parity files under `lovelace-real/src/` - the regression suites.
- `lovelace-nat/src/lib.rs` - limb representation, `add`/`sub`/`mul`/`div_rem`, `factorial`.
- `lovelace-nat/src/ntt.rs`, `lovelace-nat/src/bz.rs`, `lovelace-nat/src/newton.rs` - the large-value
  engines.
- `lovelace-nat/src/proof_small_domain.rs` - the exhaustive small-domain proof harness.
- `lovelace-bench/src/main.rs` - the benchmark ops and digest.
- `docs/adr/004-thirty-digit-default.md`, `docs/crates/lovelace-real.md`,
  `docs/crates/lovelace-nat.md` - the documented contract.

## The contract you must preserve

- `Real` is `int * 10^exp` plus optional `period_start`/`period_len`. A repeating value stores the
  repeating block once; `1/3` renders `0.(3)` and is never rounded.
- Arithmetic on terminating values works on scaled integers. Arithmetic involving a period converts
  to an exact rational first; `1/3 + 1/3` is `0.(6)`, not `0.6`.
- `lovelace-nat` is a little-endian limb array with no most-significant zero limb; zero is the empty
  array. The decimal view is a derived cache, not the source of truth.
- There is no global precision in `lovelace-real`: every operation takes its digit budget as an
  argument. The display cap is the thread-local `with_display_decimal_places` scope.
- The engine separates computation from display: `Engine::set_precision` moves both,
  `Engine::set_display_precision` pins only the display bound, and the desktop's
  `DEFAULT_PRECISION` in `lovelace-desktop/src/engine.rs` is 30 for a fresh document. A persisted
  precision keeps its stored value.
- The fixed-width complex path is gated at `FAST_PATH_MAX_DIGITS` (37) in
  `lovelace-lang/src/complex.rs`; the `lovelace-real/src/fast_path.rs` module documents reference
  fast-path observables and is not where the language applies that gate.

## Procedure

1. **Read the current representation and tests.** Read the `Real` struct and `from_fraction` in
   `lovelace-real/src/lib.rs`, the `Nat` representation and `div_rem` in `lovelace-nat/src/lib.rs`,
   and the inexact-division tests in `lovelace-real/src/round34_inexact_division.rs`. Run the
   existing suites before you change anything; the baseline is the evidence.

2. **State which budget the change honours.** Write down whether it takes `digits`/`max_digits` as an
   explicit argument, whether it respects the thread-local display cap, and whether it is allowed on
   the <=37 fast path. If a new code path truncates, it must mark the value inexact in the same way
   the existing cap does, and the rendering must say what it did rather than silently rounding.

3. **Keep the representation canonical.** Never introduce a leading zero limb, a negative zero, a
   trailing-zero-only encoding, or a period that overlaps the pre-period. Normalize at the same
   boundaries the current code does, and keep the period metadata consistent through add, subtract,
   multiply and divide. An exact rational publishes its canonical decimal form.

4. **Add the regression test first.** In `lovelace-real/src/round34_inexact_division.rs` or a new
   module under `lovelace-real/src/`, assert the exact string and the period accessors:

   ```rust
   let third = Real::from_fraction(&1u64.into(), &3u64.into(), 100).unwrap();
   assert_eq!(third.to_string(), "0.(3)");
   assert_eq!(third.period_start(), 0);
   assert_eq!(third.period_length(), 1);
   ```

   Cover the boundary you are touching: a period that closes exactly at the cap, a period that
   closes before it, a terminating quotient, a negative value, and a cancellation such as
   `1/3 - 0.333...3`. Run the test on the old code and confirm it fails for the reason you claim.

5. **Add the Nat invariant test.** If the change touches limbs, division, NTT or the square root,
   add a small-domain or boundary case in `lovelace-nat/src/` and keep
   `lovelace-nat/src/proof_small_domain.rs` green. Assert the identity (`q * d + r == n`), the empty
   zero, and the no-leading-zero-limb invariant through the public API.

6. **Pin the parser and renderer.** A change that affects how a literal is parsed or rendered needs a
   corpus record in `parity/corpus/lang.json` (or `parity/corpus/symbolics.json`) whose `text` is
   the exact typed rendering. The corpus is the gate; a missing corpus is exit 2, never a pass.

7. **Run the benchmark without fooling yourself.** `lovelace-bench` prints `RESULT <op> <digits>
   <mean_ms> <median_ms>` and a `DIGEST` line. Run the same op with the default worker budget and
   with `LOVELACE_THREADS=1`; the digests must match. A timing improvement with a changed digest is a
   correctness change, not an optimization.

8. **Update the prose.** If the precision or representation decision changes, update
   `docs/crates/lovelace-real.md` or `docs/crates/lovelace-nat.md`; if it changes the default, update
   the ADR and the desktop constant's test. Do not leave a comment claiming a cap that the code no
   longer applies.

## Verify

```console
cargo test -p lovelace-real
cargo test -p lovelace-nat proof_small_domain
cargo test -p lovelace-lang
cargo run --release -p lovelace-corpus
cargo run --release -p lovelace-bench -- sqrt 10000 3
```

Then the same benchmark with one worker, in the host's own syntax:

```console
# bash (Linux, or Git Bash on Windows)
LOVELACE_THREADS=1 cargo run --release -p lovelace-bench -- sqrt 10000 3
```

```powershell
# Windows PowerShell
$env:LOVELACE_THREADS=1; cargo run --release -p lovelace-bench -- sqrt 10000 3
```

Observable success: the Real and Nat suites each report a nonzero passed count with `0 failed`; the
corpus reports `214 passed, 0 failed`; the two benchmark runs print `RESULT sqrt 10000 <mean_ms>
<median_ms>` and the same `DIGEST` — today `DIGEST sqrt 10002:1.4142135623730950488016` — with only
the timings differing; the regression test asserts the exact decimal text and period metadata (for
example `0.(3)` with start 0 and length 1), not merely that two values are "close".

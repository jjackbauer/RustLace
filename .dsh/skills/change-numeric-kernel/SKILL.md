---
name: change-numeric-kernel
description: Change lovelace-rational, lovelace-real or lovelace-nat arithmetic safely by preserving the canonical rational form, precision contract, periodic-decimal representation and canonical limb invariants, with regression tests, cross-library evidence, corpus evidence and a digest-stable benchmark.
whenToUse: Use when changing Rat, Real or Nat arithmetic, division, rounding, normalization, exponentiation, periodic-decimal handling, square root, pi, NTT or division thresholds, or any code that can change a stored numeric value or its rendering.
---

# Change the numeric kernel

Numeric behaviour is a contract twice over: the stored value must stay canonical, and the rendered
text must stay exact. The engine has no floating point in a value path. A change is not proven by a
faster benchmark; it is proven by the same digits and the same period metadata before and after, and
by a digest that does not move.

## Before you change anything: own the abstraction

1. **Inspect the canonical owner.** `lovelace-rational` owns rational arithmetic; `lovelace-real`
   owns the real representation and the `Rat`↔`Real` projections; `lovelace-nat` owns limbs and
   `lovelace-int` owns signs. Read `docs/architecture-ownership.md` and
   `tools/architecture-policy.json`, then answer the RNL-AGT-002 questions: does the abstraction
   exist, is it suitable, can the shared implementation be improved, would a new type create
   competing semantics, and does an approved ADR authorize it?
2. **Improve the owner, not a consumer.** Symbolics re-exports `Rat`; Systems aliases it as `Q`;
   Know's `ExactNumber` is a facade. None of them may acquire numerator/denominator storage, GCD
   normalization or scalar arithmetic traits. `python tools/check-architecture.py` fails on a second
   implementation even when its unit tests pass (RNL-AGT-006: never weaken the policy to land a
   change).

## Files you will touch

- `lovelace-rational/src/lib.rs` - `Rat`, constructors, normalization, add/sub/mul/div, powers,
  budgets, cancellation and conversions.
- `lovelace-rational/src/provenance.rs` - the four exactness claims and `Conversion`.
- `lovelace-rational/tests/properties.rs`, `lovelace-rational/tests/oracle.rs` - the seeded property
  campaigns and the independent Python `fractions.Fraction` oracle.
- `lovelace-real/src/lib.rs` - `Real`, parsing, `add`/`sub`/`mul`/`div`, period metadata,
  `from_rational`/`to_rat` and the display cap.
- `lovelace-real/src/transcendental.rs` - `sqrt` and `pi`.
- `lovelace-real/src/round34_inexact_division.rs`, `lovelace-real/src/real_parity_round7.rs` and the
  other parity files under `lovelace-real/src/` - the regression suites.
- `lovelace-nat/src/lib.rs` - limb representation, `add`/`sub`/`mul`/`div_rem`, `factorial`.
- `lovelace-nat/src/ntt.rs`, `lovelace-nat/src/bz.rs`, `lovelace-nat/src/newton.rs` - the large-value
  engines.
- `lovelace-nat/src/proof_small_domain.rs` - the exhaustive small-domain proof harness.
- `lovelace-lang/tests/cross_library.rs` - the Symbolics/Systems/language equivalence tests.
- `lovelace-bench/src/main.rs` - the benchmark ops and digest.
- `docs/adr/011-shared-rational-kernel.md`, `docs/crates/lovelace-rational.md`,
  `docs/crates/lovelace-real.md`, `docs/crates/lovelace-nat.md` - the documented contract.

## The contract you must preserve

- `Rat` is canonical: positive non-zero denominator, sign only in the numerator, zero is `0/1`, and
  numerator/denominator are divided by their full arbitrary-precision GCD. Normalization and
  arithmetic never route through decimal strings.
- Failures are explicit: a zero denominator, division by zero and `0` to a negative power return a
  categorized `RationalError`; a budget failure is an error, never a truncated success. Do not add
  `unwrap_or_else(...::zero)` on a fundamental operation.
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

1. **Read the current representation and tests.** Read the `Rat` struct and normalization in
   `lovelace-rational/src/lib.rs`, the `Real` struct and `from_fraction` in
   `lovelace-real/src/lib.rs`, the `Nat` representation and `div_rem` in `lovelace-nat/src/lib.rs`,
   and the inexact-division tests in `lovelace-real/src/round34_inexact_division.rs`. Run the
   existing suites before you change anything; the baseline is the evidence.

2. **State which budget the change honours.** Write down whether it takes `digits`/`max_digits` or a
   `Limits` as an explicit argument, whether it polls cancellation, whether it respects the
   thread-local display cap, and whether it is allowed on the <=37 fast path. If a new code path
   truncates, it must mark the value inexact in the same way the existing cap does, and the
   rendering must say what it did rather than silently rounding.

3. **Keep the representation canonical.** Never introduce a leading zero limb, a negative zero, a
   zero or negative denominator, a trailing-zero-only encoding, or a period that overlaps the
   pre-period. Normalize at the same boundaries the current code does, and keep the period metadata
   consistent through add, subtract, multiply and divide. An exact rational publishes its canonical
   decimal form.

4. **Add the regression test first.** For the rational kernel add a case to
   `lovelace-rational/src/lib.rs` or `lovelace-rational/tests/properties.rs`:

   ```rust
   let result = Rat::from_parts(Int::from_i64(1), Nat::from_u64(2))
       .unwrap()
       .checked_div(&Rat::from_parts(Int::from_i64(-1), Nat::from_u64(4)).unwrap())
       .unwrap();
   assert_eq!(result, Rat::from_i64(-2));
   assert_eq!(result.denominator(), &Nat::from_u64(1));
   ```

   For `Real`, assert the exact string and the period accessors:

   ```rust
   let third = Real::from_rational(&Rat::from_parts(Int::from_i64(1), Nat::from_u64(3)).unwrap(), 100);
   assert_eq!(third.value.to_string(), "0.(3)");
   assert_eq!(third.value.period_start(), 0);
   assert_eq!(third.value.period_length(), 1);
   assert!(third.is_lossless());
   ```

   Cover the boundary you are touching: a period that closes exactly at the cap, a period that
   closes before it, a terminating quotient, a negative and a negative divisor, and a cancellation
   such as `1/3 - 0.333...3`. Run the test on the old code and confirm it fails for the reason you
   claim.

5. **Add the Nat invariant test.** If the change touches limbs, division, NTT or the square root,
   add a small-domain or boundary case in `lovelace-nat/src/` and keep
   `lovelace-nat/src/proof_small_domain.rs` green. Assert the identity (`q * d + r == n`), the empty
   zero, and the no-leading-zero-limb invariant through the public API.

6. **Pin the parser and renderer.** A change that affects how a literal is parsed or rendered needs a
   corpus record in `parity/corpus/lang.json` (or `parity/corpus/symbolics.json` / `systems.json`)
   whose `text` is the exact typed rendering. The corpus is the gate; a missing corpus is exit 2,
   never a pass.

7. **Re-run the independent oracle and the cross-library suite.** The oracle fixture must be
   regenerated with `python tools/rational-oracle.py --write` (and `--check` in CI);
   `cargo test -p lovelace-lang --test cross_library` asserts that the kernel, Symbolics, Systems and
   the language agree on the exact value, sign, precision classification and error category. A local
   test pass is not completion evidence (RNL-AGT-005).

8. **Run the benchmark without fooling yourself.** `lovelace-bench` prints `RESULT <op> <digits>
   <mean_ms> <median_ms>` and a `DIGEST` line. Run the same op with the default worker budget and
   with `LOVELACE_THREADS=1`; the digests must match. A timing improvement with a changed digest is a
   correctness change, not an optimization.

9. **Update the prose.** If the precision or representation decision changes, update
   `docs/crates/lovelace-rational.md`, `docs/crates/lovelace-real.md` or
   `docs/crates/lovelace-nat.md`; if it changes the default, update the ADR and the desktop
   constant's test. Do not leave a comment claiming a cap that the code no longer applies.

## Verify

```console
cargo test -p lovelace-rational
cargo test -p lovelace-rational --test properties
cargo test -p lovelace-rational --test oracle
cargo test -p lovelace-real
cargo test -p lovelace-nat proof_small_domain
cargo test -p lovelace-lang
cargo test -p lovelace-lang --test cross_library
python tools/check-architecture.py
python tools/test-architecture-gates.py
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
corpus reports `231 passed, 0 failed`; the two benchmark runs print `RESULT sqrt 10000 <mean_ms>
<median_ms>` and the same `DIGEST` — today `DIGEST sqrt 10002:1.4142135623730950488016` — with only
the timings differing; the regression test asserts the exact decimal text and period metadata (for
example `0.(3)` with start 0 and length 1), not merely that two values are "close".

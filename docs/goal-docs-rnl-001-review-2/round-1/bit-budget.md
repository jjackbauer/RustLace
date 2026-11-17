# Round 1 — bit-budget enforcement in `Rat::from_parts_with`

**Finding:** PR #9 second review, Finding 1 — `Rat::from_parts_with` silently ignores bit-only budgets.
**Branch / base:** `fix/release-provenance` at `428f3ba` before any edit. No commit was made.
**Date / host:** 2026-11-17, Windows, `cargo 1.100.0`. In this shell `cargo` is not on `PATH`; every
command below was run as `%USERPROFILE%\.cargo\bin\cargo.exe ...`.
**Scope:** `lovelace-rational/src/lib.rs` (`from_parts_with` only) and
`lovelace-rational/tests/properties.rs`; this report. `from_parts` is unchanged.

## 1. Facts verified before changing code

| Claim | Evidence |
|---|---|
| The constructor preflights `check_digits` on the unreduced parts and then returns `Rat::reduce_by(...)` without `limits.check` | HEAD `lib.rs:371-379` (read before editing) |
| The zero branch returns `Ok(Rat::zero())` before the preflight | HEAD `lib.rs:368-370`; pre-fix zero test returned `Ok(0/1)` under `bits(0)` |
| `2/1` has bit length 3 | `assert_eq!(Rat::from_i64(2).bit_length(), 3)` in the new test; post-fix refusal reports `observed: 3` |
| Pre-fix `Limits::bits(1)` on `2/1` returns `Ok` | failing-first output in §2 |

## 2. Failing-first evidence (tests added first, old code)

All four tests were appended to `properties.rs:584-742` before the constructor changed.

Command:

```console
cargo test -p lovelace-rational normalization_enforces -- --nocapture
```

Observed output excerpt (`exit 101`):

```text
running 3 tests

thread 'normalization_enforces_combined_budget' (226012) panicked at lovelace-rational\tests\properties.rs:644:5:
assertion `left == right` failed
  left: Ok(Rat { num: Int("2"), den: Nat { limbs_cache: OnceLock([1]), decimal: OnceLock(<uninit>) } })
 right: Err(BitBudgetExceeded { limit: 2, observed: 3 })

thread 'normalization_enforces_bit_budget' (203400) panicked at lovelace-rational\tests\properties.rs:596:5:
assertion `left == right` failed
  left: Ok(Rat { num: Int("2"), den: Nat { limbs_cache: OnceLock([1]), decimal: OnceLock(<uninit>) } })
 right: Err(BitBudgetExceeded { limit: 1, observed: 3 })

test normalization_enforces_digit_budget ... ok
test normalization_enforces_combined_budget ... FAILED
test normalization_enforces_bit_budget ... FAILED

test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 13 filtered out
```

The digit-only test passed pre-fix **by design**: the unreduced digit preflight already refuses
`2/1` under `digits(1)`. The bit-only and combined tests are the failing-first cases, and both show
`Ok(2/1)` from the old constructor where `BitBudgetExceeded` is required.

The fourth test does not contain `normalization_enforces`, so the filter above did not run it. It was
run separately, before the fix:

```console
cargo test -p lovelace-rational --test properties normalization_checks_zero_and_reduced_inputs -- --nocapture
```

Observed output excerpt (`exit 101`):

```text
thread 'normalization_checks_zero_and_reduced_inputs' (228368) panicked at lovelace-rational\tests\properties.rs:692:5:
assertion `left == right` failed
  left: Ok(Rat { num: Int("0"), den: Nat { limbs_cache: OnceLock([1]), decimal: OnceLock(<uninit>) } })
 right: Err(BitBudgetExceeded { limit: 0, observed: 1 })

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 15 filtered out
```

That `Ok(0/1)` is the observed proof that the zero early return skipped the limits check.

## 3. Change

`lovelace-rational/src/lib.rs`, zero branch (`lib.rs:372-376`):

```rust
if num.is_zero() {
    let value = Rat::zero();
    limits.check(&value)?;
    return Ok(value);
}
```

`lovelace-rational/src/lib.rs`, after normalization (`lib.rs:385-387`):

```rust
let value = Rat::reduce_by(num, den, &divisor)?;
limits.check(&value)?;
Ok(value)
```

`Limits::check` enforces the digit bound and then the bit bound, so the returned **normalized** value
now honours both. The existing digit preflight before the GCD is kept unchanged, and `from_parts`
(the unbounded form) is untouched. The `from_parts_with` doc comment (`lib.rs:341-359`) now states the
final both-bounds check, the zero-input check (`0/1` has bit length one and two upper-bound decimal
digits), and that the preflight over-counts on the unreduced parts.

Diff size: `lib.rs` 19 insertions / 11 deletions; `properties.rs` 160 insertions / 0 deletions.

Tests added (all before the fix):

1. `normalization_enforces_bit_budget` — `2/1` + `bits(1)` must be
   `BitBudgetExceeded { limit: 1, observed: 3 }`; also asserts the stable code
   `RationalBitBudgetExceeded` and category `BudgetExceeded`.
2. `normalization_enforces_digit_budget` — `2/1` + `digits(1)` must be
   `DigitBudgetExceeded { limit: 1, observed: 2 }` (stable code `RationalDigitBudgetExceeded`), and
   `digits(10)` returns `Ok(2/1)`.
3. `normalization_enforces_combined_budget` — `bits(2).with_digits(10)` must be
   `BitBudgetExceeded { limit: 2, observed: 3 }`; `bits(100).with_digits(1)` must be
   `DigitBudgetExceeded { limit: 1, observed: 2 }` from the preflight, with the exact digit code.
4. `normalization_checks_zero_and_reduced_inputs` — zero is `Ok` under `bits(1)`,
   `BitBudgetExceeded { limit: 0, observed: 1 }` under `bits(0)`, and
   `DigitBudgetExceeded { limit: 1, observed: 2 }` under `digits(1)`; `10/5` under `bits(2)` is
   refused after reduction to `2/1` with `observed: 3`; already-reduced `2/1` under `bits(3)` is
   `Ok`.

## 4. Post-fix observed output

Command 1:

```console
cargo test -p lovelace-rational normalization_enforces -- --nocapture
```

```text
running 3 tests
test normalization_enforces_combined_budget ... ok
test normalization_enforces_digit_budget ... ok
test normalization_enforces_bit_budget ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out
```

`exit 0`.

Command 2 (the fourth test, same filter reason as §2):

```console
cargo test -p lovelace-rational --test properties normalization_checks_zero_and_reduced_inputs -- --nocapture
```

```text
running 1 test
test normalization_checks_zero_and_reduced_inputs ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out
```

`exit 0`.

Command 3:

```console
cargo test -p lovelace-rational
```

```text
running 16 tests   (lib unit tests)        test result: ok. 16 passed; 0 failed
running 2 tests    (tests/oracle.rs)       test result: ok. 2 passed; 0 failed
running 16 tests   (tests/properties.rs)   test result: ok. 16 passed; 0 failed
running 0 tests    (doc-tests)             test result: ok. 0 passed; 0 failed
```

Aggregate: **34 passed, 0 failed, 0 ignored** across the four binary targets; `exit 0`. This includes
the pre-existing `normalization_is_cancellable` test, which still passes unchanged.

Command 4:

```console
cargo fmt --all -- --check
```

No output, `exit 0`.

## 5. Not verified / residual

- Deliberately not run, because this round's SCOPE excludes other crates and tools: whole-workspace
  tests, clippy, `tools/check-architecture.py`, `tools/test-architecture-gates.py`, the rational
  oracle regeneration, `lovelace-lang` cross-library tests, the corpus and the benchmark. Findings
  D3-D5 belong to the other round work.
- No caller of `from_parts_with` exists outside `lovelace-rational` (repository grep found only the
  definition and the existing tests), so no cross-crate regression path exists to exercise; the
  crate suite is the observed evidence.
- The working tree also carries modified `tools/architecture-policy.json`,
  `tools/architecture-fixtures/base/tools/architecture-policy.json`, `tools/check-architecture.py`
  and `tools/test-architecture-gates.py`. Those are a concurrent agent's Finding 2 work; I did not
  edit them and they are not part of this round's evidence.
- `Cargo.lock` was transiently rewritten by cargo during the first build: the committed lock had a
  stale `lovelace-rational 0.7.0` while the workspace is `0.7.1`. I reverted that generated change,
  so this round's diff is the two source files plus this report; a later cargo run may regenerate it.
- The zero branch is polled for cancellation before the new check, so cancellation keeps precedence
  over a budget refusal; the existing `normalization_is_cancellable` coverage exercises that order,
  but no new test targets cancellation combined with a zero numerator.
- Nothing else in `from_parts_with` was changed or re-verified: the unreduced digit preflight
  estimate, the zero-denominator precedence and the two cancellation polls are the old behaviour and
  the old tests still pass.

# Round 2 — Finding 5: budgets are enforced before the expensive work

**Finding (verbatim):** "**Finding 5 — Arithmetic budgets are checked after allocation** …
**Required correction:** add conservative preflight growth estimates, checks within expensive
numerical operations, and cancellation coverage for long-running normalization."

**Verdict:** closed for the four operations the review names, with one scope limit stated in
[§5](#5-what-i-could-not-verify). The preflight tests fail on the pre-fix code, and all required
commands pass on the fixed code ([§4](#4-final-pass-outputs)).

## 0. Scope

| File | Change |
|---|---|
| `lovelace-rational/src/lib.rs` | `Limits::check_digits`, preflights in `add_with`/`sub_with`/`mul_with`/`checked_div_with`, preflight before the power loop, `Rat::from_parts_with`, shared reduction helper |
| `lovelace-rational/tests/properties.rs` | three required tests plus a `check_digits` unit test in `src/lib.rs` |
| `docs/goal-docs-rnl-001-review/round-2/budgets.md` | this record |

`Rat::from_parts` is unchanged (`lovelace-rational/src/lib.rs:334`); no other crate is edited.
`cargo test -p lovelace-rational` remains green: 16 lib + 2 oracle + 12 properties, 0 failed.

## 1. Test-first: the observed pre-fix failure

### 1a. The preflight tests fail on behaviour, not on compilation

The two preflight tests use only API that already existed at `HEAD` (`8ec1aaf`), so they compiled and
ran against the pre-fix kernel. Observed:

```console
$ cargo test -p lovelace-rational --test properties
running 11 tests
test cancellation_is_polled_during_a_long_power ... ok
test preflight_refuses_multiplication_before_the_products_are_formed ... FAILED
test huge_power_is_refused_by_preflight ... FAILED
test seeded_zero_handling_is_canonical ... ok
test seeded_conversion_round_trips ... ok
test seeded_signed_division_handles_every_sign_pair ... ok
test seeded_resource_limit_refusals_never_return_a_partial_value ... ok
test the_same_seed_reproduces_the_campaign ... ok
test seeded_sign_combinations_and_canonical_reduction ... ok
test seeded_algebraic_identities_and_ordering ... ok
test seeded_large_operands_normalize_beyond_u64 ... ok

---- preflight_refuses_multiplication_before_the_products_are_formed stdout ----
thread 'preflight_refuses_multiplication_before_the_products_are_formed' (216296) panicked at
lovelace-rational\tests\properties.rs:405:10:
the preflight must refuse before forming the ~200-digit product: Rat { num: Int("1"), den: Nat { limbs_cache: OnceLock([1]), decimal: OnceLock(<uninit>) } }

---- huge_power_is_refused_by_preflight stdout ----
thread 'huge_power_is_refused_by_preflight' (220648) panicked at
lovelace-rational\tests\properties.rs:444:13:
the refusal must come from the preflight estimate, not the first over-budget squaring (observed 79)

test result: FAILED. 9 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
```

What the two failures prove:

* `mul_with` returned **`Ok(1)`** for `a * (1/a)` with `a` a 101-digit-parts rational and
  `Limits::digits(50)`: the two ~200-digit products were formed and only the two-digit result was
  checked. The test's `expect_err` at `properties.rs:405` is therefore the finding, reproduced.
* `checked_pow(1_000_000)` with `Limits::digits(50)` reported `observed 79` — the first intermediate
  the squaring loop happened to overshoot — not a count derived from the exponent, so the refusal
  came after the loop had started.

The same test also asserts `add_with` (`a + -a = 0`) and `checked_div_with` (`a / a = 1`); execution
stops at the first panic, so that run shows only the multiplication failure. Those two methods had
the same post-check-only body at `HEAD` (`add_with` at `HEAD:lib.rs:547-551`, `checked_div_with` at
`HEAD:lib.rs:615-619`), and the fixed code now pins both (see §5).

### 1b. The new API did not exist

After adding the normalization test (it needs `from_parts_with`) and the `check_digits` unit test,
the pre-fix run fails to compile, and that is the only failure:

```console
$ cargo test -p lovelace-rational --test properties
error[E0599]: no associated function or constant named `from_parts_with` found for struct `Rat` in the current scope
... (6 occurrences)
error: could not compile `lovelace-rational` (test "properties") due to 6 previous errors
[exit code: 101]

$ cargo test -p lovelace-rational --lib
error[E0599]: no method named `check_digits` found for struct `Limits` in the current scope
...
[exit code: 101]
```

## 2. Implementation: the preflight formulas

`Limits::check_digits(observed)` (`lovelace-rational/src/lib.rs:245`) is the cheap entry point: it
compares an estimate against `max_digits` and returns the existing
`RationalError::DigitBudgetExceeded`; it is digits-only by design, because a digit count proves
nothing about a bit budget. `Limits::check` (`lib.rs:255`) now starts with `check_digits`, so both
paths share one comparison.

Per-part digits are bounded without rendering decimal text (`lib.rs:968`):
`digits_ub(n) = floor(bit_length(n) * 30103 / 100000) + 1`, and `30103/100000 > log10(2)`, so this
is an upper bound.

| Operation | Preflight estimate | Why it is conservative |
|---|---|---|
| `add_with` / `sub_with` (`lib.rs:630`, `:646`) | For `a/b + c/d`: `max(da+dd, dc+db) + 1 + db + dd` | The unreduced sum is `(a*d + c*b)/(b*d)`; each product's digits are at most the sum of its factors' digits, `+1` bounds the carry, and reduction only shrinks both parts. The denominator GCD is **not** computed, so the estimate over-counts when the denominators share a large factor (the real sum uses their LCM). |
| `mul_with` (`lib.rs:678`) | `da+db+dc+dd` = `self.decimal_digits() + other.decimal_digits()` | Each result part is at most the product of one numerator with one denominator, so at most the sum of the two parts' digit counts. The cross-cancellation that `mul` performs before multiplying is not modelled, so the two cancelled factors are over-counted. |
| `checked_div_with` (`lib.rs:722`) | same bound as `mul_with` | `(a/b)/(c/d) = (a*d)/(b*c)` pairs one numerator with one denominator, so the same bound applies; the cross-cancellation is again not modelled. A zero divisor is not preflighted, so `checked_div` still owns `DivisionByZero`. |
| `checked_pow` / `pow_u64` (`lib.rs:790`, preflight at `:802`) | **Refuse** when the *lower* bound `Σ_parts (floor((B-1)·e·30102/100000)+1)` exceeds the digit limit; report the matching upper bound `Σ_parts (floor(B·e·30103/100000)+1)` | With `B = bit_length(part)`, `part ≥ 2^(B-1)`, so `part^e ≥ 2^((B-1)e)`. `30102/100000 < log10(2)`, so the refusal test is a sound lower bound: it cannot reject a power whose exact result fits (`2^100` under `Limits::digits(50)` still computes, asserted in the test). The same shape is used for a bit budget with lower `(B-1)e+1` and upper `B·e`. |

Zero operands are special-cased to the exact digit count of the result (`0/1` is two digits, adding
zero returns the other operand unchanged), so a zero operand never causes a false refusal. Every
preflight runs **before** the `add`/`mul`/`checked_div`/squaring work and the materialized result is
checked afterwards exactly as before, so the post-check remains the authority for results that fit.

The over-count is documented on each method: `add_with` (`lib.rs:623-629`), `mul_with`
(`lib.rs:669-677`), `checked_div_with` (`lib.rs:715-721`), `from_parts_with` (`lib.rs:348-355`), and
in the crate-level "Resource limits" section (`lib.rs:33-42`). The deliberate trade-off is that a
large-operand operation whose reduced result would fit can be refused; the tests pin exactly that
(`a * (1/a)`, `a + -a`, `a / a` under `Limits::digits(50)`).

The power preflight is applied to every positive exponent rather than behind a size threshold. A
threshold is unnecessary because the trigger is a sound lower bound, not an over-estimate: a small
power whose result fits is provably not refused, so the "avoid rejecting small legitimate powers"
requirement is met without a magic exponent.

## 3. Cancellation points

| Point | Location | Test evidence |
|---|---|---|
| Before the GCD normalization (after the zero-denominator domain check) | `lib.rs:365` | `normalization_is_cancellable`: `AlwaysCancel` returns `Cancelled` |
| Digit preflight of the unreduced parts, before the GCD | `lib.rs:371` | same test, `Limits::digits(50)` returns `DigitBudgetExceeded` |
| After the GCD normalization, before any reduction | `lib.rs:376` | `CancelOnNth::new(1)` returns `Cancelled`: a token that is false once and true on its second poll stops normalization, so cancellation is polled more than once across the GCD |
| The reduction itself runs only after the second poll | `lib.rs:379` (`reduce_by` at `:454`) | returning `Err` there means no partially normalized value can escape: the value is constructed only in the success arm |
| Before the power squaring loop | `lib.rs:802` | `huge_power_is_refused_by_preflight` |
| Between power squaring steps | `lib.rs:806` (pre-existing) | `cancellation_is_polled_during_a_long_power` still green |

`Nat::gcd` (`lovelace-nat/src/lib.rs:319-328`) has no cancellation hook; that crate is out of this
task's scope, so the poll granularity is the normalization as a whole (before and after the GCD),
not one GCD iteration. The rustdoc on `from_parts_with` states this (`lib.rs:345-346`).

## 4. Final pass outputs

Run from the repository root with `cargo ... *> <log>; $LASTEXITCODE` so the exit code is the
command's, not the log consumer's:

```console
$ cargo test -p lovelace-rational                                        # exit 0
     Running unittests src\lib.rs
running 16 tests
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests\oracle.rs
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests\properties.rs
running 12 tests
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
   Doc-tests lovelace_rational
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

$ cargo test -p lovelace-rational --test properties                       # exit 0
running 12 tests
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s

$ cargo test -p lovelace-rational --test oracle                           # exit 0
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

$ cargo fmt --all -- --check                                             # exit 0, no output
$ cargo clippy -p lovelace-rational --all-targets                        # exit 0, no warnings
```

The nine pre-existing property campaigns are unchanged and green; the test count moved from 9 to 12
only by the three tests below.

Tests added:

* `preflight_refuses_multiplication_before_the_products_are_formed` — `tests/properties.rs:373`
  (also covers `add_with` and `checked_div_with`; `hundred_digit_rational` at `:362`).
* `huge_power_is_refused_by_preflight` — `tests/properties.rs:435`.
* `normalization_is_cancellable` — `tests/properties.rs:504` (tokens at `:471` and `:495`).
* `digit_preflight_reports_the_existing_category` — `src/lib.rs:1282`.

## 5. What I could not verify

1. **`add_with`/`checked_div_with` pre-fix behaviour was not observed independently.** The pre-fix
   test run panicked at the `mul_with` assertion first, so the run only demonstrates `mul_with`
   returning `Ok(1)`. The other two were post-check-only at `HEAD` by inspection
   (`HEAD:lib.rs:547-551`, `:615-619`) and are pinned by the fixed-code test; they share the same
   one-line body the failing `mul_with` had.
2. **"Without iterating to a million" is not a wall-clock claim.** Exponentiation by squaring is
   `O(log exponent)`, so the pre-fix loop never iterated a million times either. The observable
   difference is the reported estimate: `observed 602062` after the fix versus `observed 79` from
   the first over-budget squaring before it (test asserts `observed > 100_000`). No timing or
   allocation measurement was made; the preflight itself is two `bit_length` reads and integer
   arithmetic.
3. **Cancellation inside `Nat::gcd` is not covered.** `lovelace-nat` is out of scope and exposes no
   cancellation hook; a single GCD call cannot be interrupted, only the normalization around it.
4. **Bit budgets are preflighted only for powers.** `add_with`/`mul_with`/`checked_div_with` take a
   digit estimate; a `Limits::bits`-only caller is still refused after the operation (as before).
   Extending the preflight to bits would need an upper-bound bit estimate per operation, which the
   review did not require.
5. **Whole-workspace verification is not mine to claim.** `cargo test --workspace`,
   `cargo test -p lovelace-lang --test cross_library`, `python tools/check-architecture.py` and the
   corpus gate were not run here: other agents are editing `lovelace-lang`,
   `tools/architecture-fixtures` and the architecture checker in this same working tree, so those
   runs are the parent's integration step. `cargo test -p lovelace-rational` and fmt/clippy on that
   crate are green (§4).
6. **Pre-existing rustdoc warning.** `cargo doc -p lovelace-rational --no-deps` reports
   `warning: unresolved link to 'lovelace_real::Real'` from `lovelace-rational/src/provenance.rs:15`
   — untouched by this change (the crate does not depend on `lovelace-real`). No new rustdoc
   warnings were introduced.
7. **Line endings.** `lovelace-rational/src/lib.rs` is the only file committed with CRLF; it was
   kept CRLF so the content diff stays 285/18 lines instead of a whole-file rewrite. Consequently
   `git diff --check` reports "trailing whitespace" for every added line because git treats the CR
   as whitespace; the file contains 0 lines with a real trailing space or tab. `cargo fmt --check`
   passes on the CRLF file.

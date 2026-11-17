# RNL-001 release evidence

**Status:** implemented in commit `32ea6cd0f185e4e8f28d53369837085933ec6e65` (`32ea6cd`), on
top of baseline `8b124d1`. This page records the commands, the observed results and the remaining limitations so a
reader does not have to trust a summary.

## 1. What changed

- `lovelace-rational` is the single general-purpose rational implementation: canonical arbitrary-
  precision form, full-GCD normalization, checked division, exponentiation by squaring, `Limits`
  budgets, cancellation and the four-part exactness report with `Conversion`.
- `lovelace-symbolics::Rat` is a re-export; its private `rat_reduce`/`gcd_int` normalizer was
  deleted. `lovelace-systems::Q` is a type-identical alias; the Systems rational module was deleted.
  `lovelace-know::ExactNumber` was found by the new checker as a third implementation and is now a
  delegating facade over `Rat` with no arithmetic of its own.
- `lovelace-real` owns `Real::from_rational(&Rat, max_digits) -> Conversion<Real>` and
  `Real::to_rat() -> Conversion<Rat>`, so a projection and a previously truncated input are
  distinguishable.
- Systems results expose exact numerator/denominator pairs through the additive `exact_values`
  record while retaining the decimal `Real` projection. The wire protocol stays at version 2.
- The architecture policy, checker, mutation suite, CI jobs and contributor/agent instructions are
  in place; the documented gates are unchanged and still run.
- RNL-MIG-006 and RNL-MIG-007 are fixed with regressions: MVA reports `Q_network(n) = n - X(n)*Z`;
  both sweep axes accept an inclusive endpoint only on an exact step grid.

## 2. Commands and observed results

Measured in the committed working tree on Windows 11 x86_64, 2026-11-17, rustc 1.100.0,
release benchmarks with `opt-level = 3`:

| Command | Observed result |
|---|---|
| `cargo test --workspace --no-fail-fast` | 45 targets, **6267 passed, 0 failed, 0 ignored** (exit 0) |
| `cargo run --release -p lovelace-corpus` | **corpus: 231 passed, 0 failed, 231 total** (exit 0) |
| `python tools/check-architecture.py` | **14 crates, 332 files, 0 violations, 0 waived** (exit 0) |
| `python tools/check-architecture.py --json` | the same graph as a machine-readable impact report |
| `python tools/test-architecture-gates.py` | **11 mutation scenarios, 11 passed, 0 failed** |
| `python tools/rational-oracle.py --check` | 730 cases match the Python `Fraction` oracle |
| `python tools/systems-oracle.py --check` | 25 systems metrics match the independent formulas |
| `cargo test -p lovelace-rational --test properties` | 9 seeded campaigns, 0 failed |
| `cargo test -p lovelace-rational --test oracle` | 2 tests, 730 recorded cases recomputed |
| `cargo test -p lovelace-systems --test oracle` | 3 tests, M/M/1, Erlang C, MVA and Jackson |
| `cargo test -p lovelace-lang --test cross_library` | 5 tests, kernel/Symbolics/Systems/language |
| `cargo fmt --all -- --check` | clean (exit 0) |
| `cargo clippy` on the changed crates | no new warnings; the remaining warnings predate this change |
| `bash tools/check-crate-examples.sh` | 15 pages, 10 Rust fences compiled and ran |
| `bash tools/check-docs.sh --symbols` / `--claims` / `--readme` | all OK |
| `bash tools/check-reference-pages.sh` | 30 scoped pages, 19 manifested, 11 non-reference, 133 sections |
| `bash docs/language/examples/run-examples.sh` | **198 of 198 examples passed**; 134 builtins in 12 categories |
| `bash docs/extensions/examples/run-examples.sh` | **31 of 31 examples passed** |
| `bash tools/run-tutorials.sh` | **4 tutorials**, 8 fences, 1 independent oracle |
| `bash tools/test-docs-gates.sh` | **31 mutations, 31 passed** |

The benchmark evidence, including the pre-migration `symbolics::Rat` and `systems::Q` measurements
at `8b124d1`, is in [`benchmarks-numerics.md`](benchmarks-numerics.md). The measured per-crate test
table is in [`testing.md`](testing.md#1-unit-tests-in-crate-test).

## 3. Final definition of done

| Requirement | Evidence |
|---|---|
| Exactly one canonical rational implementation remains | `python tools/check-architecture.py` (0 duplicate declarations); the Systems kernel and the Know implementation are deleted/migrated |
| Symbolics and Systems consume the same implementation | `Q` is an alias and `Rat` a re-export; `cross_library` compares them by type and value |
| Arithmetic edge cases and Systems defects have regression coverage | `(1/2)/(-1/4) = -2`, `i64::MIN`, large-GCD normalization, MVA think-state occupancy, sweep endpoint refusals |
| Exact values and precision-limited projections are distinguishable | `exact_values` in the language/wire; `Conversion`/`ExactnessProvenance` in Rust; the precision-boundary test |
| The checker rejects seeded duplicates and forbidden dependencies | `tools/test-architecture-gates.py`, 11 scenarios including duplicate rational, duplicate trait, forbidden edge, cycle, undeclared numeric, unauthorized exception, zero scan |
| Legitimate domain code remains permitted | the valid fixture carries a domain formula, a typed unit, a permitted delegating specialization and a canonical alias |
| CI enforces architecture rules as required checks | `architecture-policy` and `numerical-kernel` jobs in `.github/workflows/ci.yml`; the policy names the required jobs and the checker fails if one disappears |
| Contributor and agent instructions require reuse | `AGENTS.md`, `.github/copilot-instructions.md`, `docs/contributing.md`, `docs/extending.md` §2 and the three numeric/language skills |
| Documentation reflects the implementation | `CONTRACT.md`, ADR-011, `docs/architecture-ownership.md`, `docs/architecture.md`, the crate pages, the precision guide |
| Workspace tests, corpus and documentation gates pass | the table above |
| Release evidence records commit, commands, results and limitations | this page |

## 4. Acceptance invariant

The invariant is that a second general-purpose rational implementation in a domain library fails CI
before merge, regardless of passing unit tests. It is demonstrated twice:

1. `tools/test-architecture-gates.py` injects a Systems `Rat` with full arithmetic and requires
   `duplicate-numeric-type`; it injects an unapproved `impl Add` and requires
   `duplicate-arithmetic-trait`; it injects a field-pair type with methods and requires
   `duplicate-numeric-representation`.
2. The checker found a real third implementation in `lovelace-know/src/exact.rs` during this
   migration. It is now a facade over `Rat` with no numerator/denominator storage, GCD or
   arithmetic. That is exactly the failure mode the acceptance invariant exists to catch: the
   implementation's own tests were green.

## 5. Remaining limitations

- **CI has not run from this host.** The workflow and policy are committed and the jobs are
  defined; the Windows job, Linux/Xvfb job, architecture matrix and documentation matrix must run
  on GitHub before merge. The local observations above are Windows-only.
- **Branch protection is an administrative setting.** The repository can verify that the required
  jobs are defined (the checker does); it cannot configure the protected-branch list from code.
  A maintainer must require `architecture-policy` (both OS variants), `numerical-kernel`, `windows`
  and `docs` (both OS variants).
- **The checker is syntax-aware, not a proof.** It parses declarations, fields, impl methods and
  trait impls after stripping comments/strings, and it uses `cargo metadata` for the real graph.
  A sufficiently obfuscated duplicate can evade a heuristic; `docs/architecture-ownership.md` defines
  the review process, and the exception procedure requires an ADR.
- **Tests changed with a measured reason.** `wl16_solver_is_cancellable` now solves degree 200
  instead of 50 because the faster canonical kernel finished the old workload inside the 5 ms
  cancellation budget. The assertion is unchanged; the workload is heavier, not weaker.
- **Performance is measured, not gated.** The numbers are one host and one date. CI does not fail
  on elapsed time; a regression needs a reviewed measurement at the commit under test (RNL-CI-011).
- **API corrections are breaking.** `Rat::from_parts` returns `Result`, Systems `Q::to_real` /
  `Q::from_real` are replaced by `Real::from_rational` / `Real::to_rat`, and the Systems rational
  module is gone. ADR-011 records the replacements; the language promotion policy is unchanged.
- **Exactness is provenance, not world-truth.** An exact model solution is exact under its
  assumptions; the systems crate does not test that a real system is Poisson or product-form.

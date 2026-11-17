# RNL-001 release evidence

**Status:** RNL-001 implemented in commit `32ea6cd0f185e4e8f28d53369837085933ec6e65` (`32ea6cd`).
PR #9's review remediation is commit `c26f909cabaeab6d5f664974ae1c06eb8ceeb9de` (`c26f909`),
merged with `main` in `55a8a421f3599eacfd8300948f09183169e56e36` (`55a8a42`). All seven required
PR checks passed in run 44125853308; §6 maps the review findings to the fix. A second review followed;
§7 records its corrections. This page records the commands, the observed results and the remaining
limitations so a reader does not have to trust a summary.

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

Measured in the `fix/release-provenance` working tree (the second-review fixes are uncommitted) on
Windows 11 x86_64, 2026-11-17, rustc 1.100.0, release benchmarks with `opt-level = 3`:

| Command | Observed result |
|---|---|
| `cargo test --workspace --no-fail-fast` | 45 targets, **6280 passed, 0 failed, 0 ignored** (exit 0) |
| `cargo run --release -p lovelace-corpus` | **corpus: 231 passed, 0 failed, 231 total** (exit 0) |
| `python tools/check-architecture.py` | **14 crates, 332 files, 0 violations, 0 waived** (exit 0) |
| `python tools/check-architecture.py --json` | the same graph as a machine-readable impact report |
| `python tools/test-architecture-gates.py` | **21 mutation scenarios, 21 passed, 0 failed**; the valid fixture reports 7 crates, 7 files, 0 violations |
| `python tools/rational-oracle.py --check` | 730 cases match the Python `Fraction` oracle |
| `python tools/systems-oracle.py --check` | 25 systems metrics match the independent formulas |
| `cargo test -p lovelace-rational --test properties` | 16 seeded property campaigns, 0 failed |
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
| Exact values and precision-limited projections are distinguishable | `exact_values` on analysis and model-construction records in the language/wire; `Conversion`/`ExactnessProvenance` in Rust; the precision-boundary test |
| The checker rejects seeded duplicates and forbidden dependencies | `tools/test-architecture-gates.py`, 21 scenarios including duplicate rational, duplicate in an owner crate, split/generic/free-function/renamed shapes, a two-field/two-method partial representation, forbidden edge, cycle, undeclared numeric, broad exception, unresolved arithmetic impl and zero scan |
| Legitimate domain code remains permitted | the valid fixture carries a domain formula, a typed unit, a permitted delegating specialization and a canonical alias |
| CI enforces architecture rules as required checks | all seven PR checks are required on `main` with strict up-to-date branches and `enforce_admins`; negative-probe PR #10 failed the architecture checks and its merge was refused |
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

- **CI has run on PR #9.** At head `55a8a42`, all seven checks passed: architecture-policy
  (Ubuntu and Windows), numerical-kernel, Windows full workspace, Linux/Xvfb desktop and
  documentation examples (Ubuntu and Windows), in run
  [44125853308](https://github.com/jjackbauer/RustLace/actions/runs/44125853308). The local
  observations above are Windows-only; the CI run is the Linux and Windows reproduction.
- **Branch protection is configured.** `main` requires the seven PR checks: both
  `architecture-policy` matrix legs, `numerical-kernel`, `windows`, `linux` and both `docs` matrix
  legs. Protection also requires strict up-to-date branches, applies to administrators
  (`enforce_admins`), and disables force pushes and branch deletion. Negative-probe PR #10 carried
  a deliberately forbidden Systems rational: both architecture checks failed and
  `gh pr merge 10 --merge` was refused with "the base branch policy prohibits the merge." Required
  code-owner review is **not** enabled: `.github/CODEOWNERS` declares ownership of the architecture
  policy, checker, mutation fixtures, ownership document, ADR-011 and the rational kernel, but the
  repository has one collaborator, the PR author, and GitHub does not allow self-approval. This is
  the recorded user-approved limitation DEC-002; the status checks are enforced independently.
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

## 6. PR #9 review remediation

The independent review of PR #9 raised seven findings. This section maps each to the fix in
commit `c26f909`, merged with `main` in `55a8a42`; the detailed command output is in the round-2
records under `docs/goal-docs-rnl-001-review/round-2/`.

| Finding | Correction now in the working tree | Evidence |
|---|---|---|
| 1 — Exact model values are lost during construction (P0) | `Station` and `QueueNetwork` records carry additive `exact_values` for service time, visits and service demand, or arrival rate / think time; `parse_station` and `parse_network` consume the sidecar and use the decimal field only when the whole sidecar is absent. | `long_period_service_time_survives_station_and_open` and `long_period_think_time_survives_closed_and_mva`; `1/97` keeps denominator 97 through `sys_station → sys_open → sys_analyze` and `sys_station → sys_closed → sys_mva`. |
| 2 — Checker detection gaps (P1) | Method counts are summed across split `impl` blocks; generic wrapper targets are resolved; empty or generic arithmetic-trait targets are reported as `review-required`; kernel-named free functions are restricted to their owner; a two-field/two-method partial representation is flagged below the four-method threshold. | Nine new mutation scenarios (`split-impl`, `generic-arithmetic-trait`, `free-function-kernel`, `unresolved-arithmetic-impl` and the owner-crate cases); the suite now has 21 scenarios including the second-review `partial-duplicate-kernel`. |
| 3 — Required CI jobs are not required merge checks (P1) | Seven PR checks are required on `main`, with strict up-to-date branches, `enforce_admins`, and force pushes and deletions disabled. | Protection read-back; negative-probe PR #10 failed the architecture checks and its merge was refused. |
| 4 — Whole-crate exemptions undermine the single owner (P0) | `ownerCrates` is retired and rejected; `reservedTypeNames` and `permittedRepresentations` enforce per-type ownership. | A `Rat` in `lovelace-lang` and in `lovelace-real`, and a second `Real` in `lovelace-real`, are rejected; the real workspace reports 14 crates, 332 files, 0 violations. |
| 5 — Arithmetic budgets are checked after allocation (P1) | `Limits::check_digits` and preflights in `add_with`/`sub_with`/`mul_with`/`checked_div_with` and before the power loop; `from_parts_with` preflights the unreduced digits, polls cancellation before and after normalization, and checks the normalized value against both bounds. | Preflight and cancellation tests in `lovelace-rational`; `Nat::gcd` has no internal cancel hook, so normalization is the cancellation granularity. |
| 6 — Architecture exceptions are too easy to broaden (P1) | Exception scopes must be exact paths or crate names; `*`, `?`, `[` and `]` are rejected. CODEOWNERS declares review ownership. | The `broad-exception-scope` scenario; DEC-002 records why required code-owner review is not enabled. |
| 7 — Benchmark wording and repository hygiene (P2) | The prose now matches the measured table: multiplication about 2× faster, division essentially unchanged (0.116 vs 0.109 ms), addition slower (0.192 vs 0.239 ms), `pow` about 6×, normalize about 2×. The tracked Python bytecode is removed and `.gitignore` covers `__pycache__/` and `*.py[cod]`. | `docs/benchmarks-numerics.md`; `git ls-files` no longer lists a `.pyc`; both ignore rules match. |

## 7. Second review (2026-11-17)

The second review of PR #9 raised two further gaps. Both are fixed in the uncommitted working tree
on top of `428f3ba`; the source records are
`docs/goal-docs-rnl-001-review-2/round-1/bit-budget.md` and
`docs/goal-docs-rnl-001-review-2/round-1/partial-duplicate.md`.

- **`from_parts_with` honours bit-only budgets.** The constructor keeps its unreduced digit
  preflight, then calls `Limits::check` on the normalized value before returning it. The check
  covers the zero value — `0/1` under `bits(0)` is `BitBudgetExceeded { limit: 0, observed: 1 }` —
  and already-reduced input such as `2/1` under `bits(1)`. `from_parts` remains the unbounded,
  uncancellable form.
- **Partial representations are detected below the four-method threshold.** The policy adds
  `minimumRepresentationMethods: 2`; a struct or union with at least two numeric-typed fields and
  at least two arithmetic methods is reported as `duplicate-numeric-representation`. The new
  `partial-duplicate-kernel` scenario fails pre-fix and passes with that diagnostic, and the suite
  is now 21 scenarios. The valid fixture and the real workspace (14 crates, 332 files) remain at 0
  violations, and no new `permittedRepresentations` entry was added.

The final local workspace suite is **45 targets, 6280 passed, 0 failed, 0 ignored**; the rational
crate is 34 tests (16 unit + 16 property + 2 oracle).

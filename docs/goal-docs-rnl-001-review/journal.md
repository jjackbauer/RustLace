# Goal Journal — rnl-001-review

> Append-only. See the stretch-goal-harness evidence protocol for entry format.

---

### DEC-001: Accept the review remediation scope without dropping a mechanism

- **Decision**: All ten mechanisms named in the review are KEPT and are the goal; no mechanism is
  replaced or removed. No user scope question exists.
- **Owner**: user
- **Status**: approved
- **Rationale**: The review's "merge conditions" and findings name each mechanism; the user invoked
  the harness with that review as the task. `goal.md` records the verbatim excerpts and the
  KEPT table.
- **Alternatives considered**: re-scoping to M1/M4 only (rejected: drops the P1 review conditions);
  treating branch protection as out of scope (rejected: M6 is explicitly required).
- **Related**: goal.md

### TODO-001: Round 1 — independently reproduce/refute findings 1, 2, 4, 6, 7

- **Task**: Probe the current head `8ec1aaf` with an Intent Auditor, a Probe for exact model
  transport, and a Reviewer for the checker/policy gaps. Do not fix anything in this round.
- **Priority**: P0
- **Depends on**: DEC-001
- **Status**: In progress
- **Related**: goal.md

### RISK-001: Review claim may be false because language `1/97` is already exact

- **Risk**: The review's `1/97` example may not reproduce if the language value is exact at 1000
  digits; the real break may require a period longer than 1000 or a different projection.
- **Likelihood**: Medium
- **Impact**: High (the P0 finding and regression design depend on it)
- **Evidence**: `lovelace-lang/src/systems_family.rs` `real()` uses `DEFAULT_PRECISION=30`;
  `Ctx::number()` rejects `!is_lossless`.
- **Mitigation**: Probe must run the exact end-to-end script and record the observed error or
  success; the fix must be tested against a value that actually exceeds the 30-digit projection.
- **Gate**: G1

### OBS-001: The 1/97 open-network model fails on re-parse

- **Source**: `lovelace-lang/src/systems_family.rs:486-528` (`station_record`/`network_record` omit `exact_values`); `lovelace-lang/src/systems_family.rs:327-328` and `:362-369` (parser calls `ctx.number` on the projection); observed `target/debug/lovelace-run.exe --file .tmp/probe-open.ls --text`
- **Fact**: `sys_station("cpu", 1/97, 1)` succeeds and prints `service_time: 0.010309278350515463917525773195`, but `sys_open(1, [s])` fails with `InvalidArgument/TypeMismatch`: `sys_open(): argument 2 expects an exact number ...; got a precision-limited Real ...`.
- **Implications**: Review Finding 1 is real; the model-construction record does not carry an authoritative exact value.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: HYP-001

### HYP-001: Exact model records must carry and consume `exact_values`

- **Claim**: Adding an additive `exact_values` record to `Station` and `QueueNetwork`, and making `parse_station`/`parse_network` prefer it, makes the 1/97 open and closed models succeed without changing the decimal display fields.
- **Supporting OBS**: OBS-001
- **Why it matters**: P0 review condition; without it valid long-period models cannot be constructed.
- **Falsification strategy**: Add the regression test first and observe the current failure; after the change, run both end-to-end scripts and the full language suite; assert the exact denominator is 97 and the solver succeeds.
- **Status**: Under review
- **Confidence**: High

### VAL-001: Finding 1 reproduced before any fix

- **Target**: HYP-001
- **Method**: Ran the two end-to-end scripts through `target/debug/lovelace-run.exe --file`.
- **Evidence examined**: open: `sys_open(): argument 2 expects an exact number ... got a precision-limited Real 0.010309278350515463917525773195` (exit 1); closed: `sys_closed(): argument 3 ...` same (exit 1); station alone: `Station(id: cpu, discipline: fcfs, service_time: 0.0103..., visits: 1, service_demand: 0.0103..., servers: 1)` with no `exact_values`.
- **Result**: Supported (the failure is real)
- **Conclusion**: The fix must add exact model fields and consume them; decimal projections alone cannot be the transport.
- **Related**: OBS-001, HYP-001

### OBS-002: Whole-crate owner exemption is real

- **Source**: `tools/check-architecture.py:749,764` (`owners` set and `if ... crate in owners: continue`); `tools/architecture-policy.json` `duplicateDetection.ownerCrates` includes `lovelace-lang`
- **Fact**: A fixture whose policy adds `lovelace-lang` to `ownerCrates` and whose `lovelace-lang/src/lib.rs` declares `Rat { num, den }` with four arithmetic methods exits 0 (`violations=0`). The same fixture without `lovelace-lang` in owners exits 1 with `duplicate-numeric-type` and `duplicate-arithmetic-impl`.
- **Implications**: Review Finding 4 is real: the whole-crate skip exempts a canonical rational name in the language crate.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: HYP-002

### OBS-003: Split impl blocks and generic trait impls evade detection

- **Source**: `tools/check-architecture.py:770-775` (`method_counts` takes the max per impl); `:543-575` (`type_name_of` returns the last identifier, so `Wrapper<T>` resolves to `T`); observed fixture runs
- **Fact**: A `ScalarPair { top: Int, bottom: Nat }` with `add/sub` in one impl and `mul/div` in another exits 0. A `Wrapper<T>` with `impl<T> std::ops::Add for Wrapper<T>` exits 0.
- **Implications**: Review Finding 2 is real; both mutations must fail the checker.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: HYP-002

### OBS-004: Broad wildcard exception scope is accepted

- **Source**: `tools/check-architecture.py:861-863` (`scope` rejects only the literal `"*"`); observed fixture run
- **Fact**: An exception with `scope: ["lovelace-*"]`, a real ADR file, reviewer, rationale and removal plan waives a `forbidden-dependency` violation (the checker prints `ARCH waived ... by EX-1`).
- **Implications**: Review Finding 6 is real; any wildcard scope must be rejected.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: HYP-002

### OBS-005: Benchmark wording and tracked bytecode are real

- **Source**: `docs/benchmarks-numerics.md` comparison table; `git ls-files`; `.gitignore`
- **Fact**: The table records add 0.192 -> 0.239 and div 0.116 -> 0.109, but the prose says multiplication and division are about 2x faster. `tools/__pycache__/check-architecture.cpython-312.pyc` is tracked; `.gitignore` has no Python cache rule.
- **Implications**: Review Finding 7 is real; it is a documentation/hygiene fix.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: HYP-002

### HYP-002: Findings 1, 2, 4, 6 and 7 all reproduce at 8ec1aaf

- **Claim**: Each blocking/P1 review finding is a real gap in the current tree, not a misinterpretation.
- **Supporting OBS**: OBS-001..OBS-005
- **Why it matters**: The round selects fixes only for reproduced gaps.
- **Falsification strategy**: Independent agents re-run the scripts/fixtures; any not reproduced becomes an OQ, not a fix.
- **Status**: Supported
- **Confidence**: High

### VAL-002: Independent probe batch

- **Target**: HYP-002
- **Method**: Orchestrator reproduced OBS-001..OBS-005; independent agents were dispatched to re-derive the same facts in `round-1/`.
- **Evidence examined**: commands above; agent reports to be merged.
- **Result**: Supported
- **Conclusion**: Round 1 gate can pass once the agent reports are checked; fixes proceed in round 2.
- **Related**: OBS-001..OBS-005

### DEC-002: Code-owner review is declared, not self-blocking

- **Decision**: Add `.github/CODEOWNERS` for the architecture policy/checker/ownership files. Do not
  enable GitHub's required code-owner review on `main` in this change: the only collaborator is
  `jjackbauer`, the author of PR #9, and GitHub does not allow self-approval, so enabling it now
  would make the PR unmergeable. Required status checks remain enforced for admins.
- **Owner**: user
- **Status**: approved
- **Rationale**: The user selected "Declare ownership only" in the round-2 question. The declaration
  is in the repository; the enforcement limitation is recorded in the evidence.
- **Alternatives considered**: enforce now and add a second reviewer (rejected: no second account);
  enforce after this PR (deferred, not selected).
- **Related**: intent-audit.md, goal.md

### OBS-006: Model-construction exact transport is implemented and verified

- **Source**: `lovelace-lang/src/systems_family.rs:329-468` (`exact_value`/`model_number`), `:570-636` (`station_record`/`network_record`), `:1013-1050` (two regressions); observed `cargo test -p lovelace-lang systems_family` and the original `.tmp/probe-*.ls` scripts
- **Fact**: `sys_station("cpu", 1/97, 1)` now emits `exact_values.service_time` denominator 97; the open and closed end-to-end scripts both reach an `Analysis` and the 1/97 values keep denominator 97. The decimal field is unchanged (`0.0103...`).
- **Implications**: Review Finding 1 is corrected; D1/D2 pass.
- **Confidence**: High
- **Agent**: implementer + orchestrator
- **Related**: HYP-001, VAL-001

### OBS-007: Abstraction-specific ownership and new checker shapes pass

- **Source**: `tools/check-architecture.py:932-1122`; `tools/architecture-policy.json`; `tools/test-architecture-gates.py`; observed commands
- **Fact**: The real workspace is `crates=14 files=332 violations=0`; the mutation suite is **20/20** with the nine new scenarios (owner-crate Rat, owner-real Rat/Real, split impl, generic trait, free function, renamed representation, broad exception, unresolved arithmetic impl). `ownerCrates` is retired and rejected.
- **Implications**: Review Findings 2, 4 and 6 are corrected; D3/D4/D6 pass.
- **Confidence**: High
- **Agent**: implementer + orchestrator
- **Related**: HYP-002

### OBS-008: Budget preflight and cancellable normalization pass

- **Source**: `lovelace-rational/src/lib.rs` (`check_digits`, `_with` preflights, `from_parts_with`); `lovelace-rational/tests/properties.rs`; observed `cargo test -p lovelace-rational`
- **Fact**: `cargo test -p lovelace-rational` reports 16 unit + 2 oracle + 12 property tests, 0 failed; the preflight tests include `mul_with(left, 1/left)` refused under a 50-digit budget and `2^1_000_000` refused with a preflight-sized observation, and `from_parts_with` cancellation before/after the GCD.
- **Implications**: Review Finding 5 is corrected; D5 passes. `Nat::gcd` has no internal cancellation hook (documented limitation).
- **Confidence**: High
- **Agent**: implementer + orchestrator
- **Related**: HYP-002

### OBS-009: Benchmark prose and repository hygiene are corrected

- **Source**: `docs/benchmarks-numerics.md:66-74`; `.gitignore`; observed `git ls-files` and `git check-ignore`
- **Fact**: The prose now says multiplication ~2x, division essentially unchanged, addition slower, pow ~6x, normalize ~2x. The tracked `.pyc` is removed; `.gitignore` covers `__pycache__/` and `*.py[cod]`.
- **Implications**: Review Finding 7 is corrected; D7 passes.
- **Confidence**: High
- **Agent**: implementer + orchestrator
- **Related**: HYP-002

### OBS-010: Branch protection blocks a failing architecture check

- **Source**: negative-probe PR #10 (branch `arch-policy-negative-probe`, later closed/deleted); `gh pr checks 10`; `gh pr merge 10 --merge`
- **Fact**: On PR #10, both architecture-policy checks failed while Numerical kernel, Documentation (both OS), Linux and Windows passed; `gh pr merge 10 --merge` exited 1 with "the base branch policy prohibits the merge." Read-back of `main` protection shows strict=true, enforce_admins=true and the seven required contexts.
- **Implications**: Review Finding 3 / merge condition 4 is verified; D8 passes.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: EVD-011

### OBS-011: Final local workspace suite is green

- **Source**: `cargo test --workspace --no-fail-fast`; per-crate counts
- **Fact**: 45 targets, **6276 passed, 0 failed, 0 ignored** (Windows 11, 2026-11-17). The per-crate counts are rational 30, lang 1747, real 2493, run 454, nat 271, symbolics 197, int 135, array 61, systems 55, know 56, corpus 28, json 20, bench 0, desktop 729.
- **Implications**: D1–D7 pass in the final tree; the PR CI must re-run after the commit.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: VAL-003

### DEC-002: Code-owner review is declared, not self-blocking

- **Decision**: Add `.github/CODEOWNERS` for the architecture policy/checker/ownership/rational files; do not enable required code-owner review on `main` for this PR.
- **Owner**: user
- **Status**: approved
- **Rationale**: The user selected "Declare ownership only"; the sole collaborator is the PR author, and GitHub forbids self-approval. The status-check protection is enforced independently.
- **Alternatives considered**: enforce now and add a second reviewer (not available); enforce after this PR (deferred, not selected).
- **Related**: intent-audit.md, EVD-011

### VAL-003: Round-2 implementation batch

- **Target**: HYP-001, HYP-002
- **Method**: Re-ran the model tests and the two original scripts; ran the updated checker and 20-scenario mutation suite; ran the rational suite; ran the full workspace suite; ran the negative-probe PR merge attempt; closed/deleted the probe.
- **Evidence examined**: commands and outputs above; `round-2/*.md`.
- **Result**: Supported
- **Conclusion**: Findings 1, 2, 4, 5, 6, 7 and merge condition 4 are fixed/verified; Finding 3 status-check protection is verified, code-owner review is a recorded user-approved limitation.
- **Related**: OBS-006..OBS-011, DEC-002

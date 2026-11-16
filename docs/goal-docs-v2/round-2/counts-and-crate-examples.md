# RQ-011 and RQ-010 — reconciled test counts and compiled crate-page examples

**Branch:** `docs/documentation-quality-remediation` · **Reviewed revision:**
`fa87ecc6abe41d3e174d62b211dd6b6fac2e29b7` (`v0.4.1-4-gfa87ecc`)
**Date:** 2026-11-16 · **Host:** WSL2, Ubuntu 24.04.4 LTS, x86_64; cargo 1.100.0 (3acc79c0c);
`CARGO_TARGET_DIR=/tmp/rl-target`

**Changed:** `docs/testing.md`, `AGENTS.md`, `tools/check-crate-examples.sh` (new).
**Deliberately not changed:** `.github/workflows/ci.yml` (wiring is round 3), `README.md` (out of
scope; reported below), `.dsh/skills/release-and-publish/SKILL.md` (out of scope; already clean),
`tools/check-docs.sh`, `tools/check-reference-pages.sh`, both `run-examples.sh`,
`docs/editorial.md`, and `docs/goal-docs-v2/**` other than this report.

At measurement time the working tree contained documentation and harness edits only:
`git diff --ignore-cr-at-eol --stat` lists no `lovelace-*/src` or `lovelace-*/tests` file, so the
counts below are counts of the `fa87ecc` test sources.

---

## RQ-011 — reconcile the test-count record

### 1. Current aggregate, measured on this host

```console
$ xvfb-run -a -s "-screen 0 1600x1000x24" cargo test --workspace --no-fail-fast
```

This was run at `fa87ecc6abe41d3e174d62b211dd6b6fac2e29b7` on 2026-11-16
(workspace run finished `2026-11-16T12:14:31Z`), with `CARGO_TARGET_DIR=/tmp/rl-target`.
It reports **35 test targets: 6147 passed, 0 failed, 1 ignored**. The per-crate rows are
`cargo test -p <crate>` on the same tree, date and target directory (the desktop under
`xvfb-run`); every command exited 0.

| Crate | Command | Exit | Targets | Passed | Failed | Ignored |
|---|---|---:|---:|---:|---:|---:|
| `lovelace-real` | `cargo test -p lovelace-real` | 0 | 2 | 2487 | 0 | 0 |
| `lovelace-lang` | `cargo test -p lovelace-lang` | 0 | 2 | 1750 | 0 | 0 |
| `lovelace-desktop` (Linux) | `xvfb-run ... cargo test -p lovelace-desktop` | 0 | 2 | 715 | 0 | 0 |
| `lovelace-run` | `cargo test -p lovelace-run` | 0 | 3 | 450 | 0 | 0 |
| `lovelace-nat` | `cargo test -p lovelace-nat` | 0 | 2 | 271 | 0 | 0 |
| `lovelace-symbolics` | `cargo test -p lovelace-symbolics` | 0 | 2 | 197 | 0 | 0 |
| `lovelace-int` | `cargo test -p lovelace-int` | 0 | 2 | 135 | 0 | 0 |
| `lovelace-array` | `cargo test -p lovelace-array` | 0 | 2 | 61 | 0 | 0 |
| `lovelace-corpus` | `cargo test -p lovelace-corpus` | 0 | 3 | 18 | 0 | 1 |
| `lovelace-json` | `cargo test -p lovelace-json` | 0 | 2 | 7 | 0 | 0 |
| `lovelace-bench` | `cargo test -p lovelace-bench` | 0 | 1 | 0 | 0 | 0 |
| `lovelace-know` | `cargo test -p lovelace-know` | 0 | 12 | 56 | 0 | 0 |
| **Total** | | | **35** | **6147** | **0** | **1** |

The sums were checked mechanically (`awk` over the table extracts): targets 35, passed 6147,
failed 0, ignored 1 — identical to the workspace run's 35 `test result:` lines below.

The one ignored test is `property_assertions::mathir_evaluates_like_symbolic`, annotated in
place: "blocked: numeric::evaluate and IrEvaluator differ by 1 ulp at 50 places
(lovelace-lang)".

`lovelace-know`'s CLI-stdio tests spawn the real `lovelace-run` binary; a standalone
`cargo test -p lovelace-know` in a *fresh* target directory needs `cargo build -p lovelace-run`
first (the helper at `lovelace-know/tests/common/mod.rs:43` prints that instruction).
`cargo test --workspace` builds it as part of `lovelace-run`'s own test targets.

### 2. Every `test result:` line from the measured workspace run

```text
test result: ok. 61 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 18 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 9.65s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 711 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.20s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 135 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.28s
test result: ok. 1750 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.21s
test result: ok. 271 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.44s
test result: ok. 2487 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.29s
test result: ok. 438 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.61s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
test result: ok. 197 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.03s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Sum from those lines: **35 targets, 6147 passed, 0 failed, 1 ignored**.

### 3. The 6,067 / 6,135 arithmetic — and the corrected split of the 68

The table carried at `fa87ecc` listed eleven rows that sum to **6,067** and displayed **6,135**.
The 68-test difference is genuine, but it is **not one crate**. Row-level measurement splits it
into the two test targets the table omitted:

| Crate / target | Tests | Evidence |
|---|---:|---|
| `lovelace-know` | 56 | 56 tests listed at `7ceeeba` (`cargo test -p lovelace-know -- --list`); 56 passed at v0.4.1 with `LOVELACE_RUN` set and 56 passed at `fa87ecc` |
| `lovelace-run` `tests/recursion_depth.rs` + `tests/repl.rs` | 12 (1 + 11) | the `7ceeeba` list has 450 `lovelace-run` tests, 438 of them in `src/main.rs`, leaving these 12; 12 passed at v0.4.1 and at `fa87ecc` |
| **Omitted total** | **68** | |

```text
rows as originally written    2487+1737+716+438+271+197+135+61+18+7+0 = 6067
omitted: lovelace-know                                                   56
omitted: lovelace-run tests/                                             12
                                                                        ---
reconciled total                                                       6135
```

The old prose attributed the whole 68 to `lovelace-know` "identified by subtraction". The
subtraction was right; the attribution is falsified by measurement. The 438 row is the
`lovelace-run` `src/main.rs` unit-test target only; that crate also has two integration targets
(438 + 1 + 11 = **450** at both v0.4.1 and `fa87ecc`). No 68-test `lovelace-know` target exists
at `7ceeeba`, v0.4.1 or `fa87ecc`; adding a fabricated `lovelace-know | 68` row would break the
"no invented count" constraint. `docs/testing.md` now lists both missing targets and shows the
68 as 56 + 12.

The **6,135 aggregate itself is the page's recorded Windows observation**: its desktop row is the
Windows 716, carried without a date or commit of its own. It is reproduced here as arithmetic on
measured row counts plus that recorded row, not as a new Windows measurement. It is labelled that
way in `docs/testing.md`.

The fixture story also survives: before `e87ca2a` (2026-11-14, ancestor of `7ceeeba`) carried
`knowledge-graph-demo.json`, the workspace run reported 6,130 passed / 5 failed (the five were
`lovelace-know`'s `convergence_tests` 2, `graph_tests` 2 and `seed_reproduction` 1). With the
fixture present the same composition is 6,135 passed / 1 ignored.

### 4. 1,737 versus 1,750 — by commit, not mixed

| Count | Revision | Measurement | Date |
|---:|---|---|---|
| **1737** | `7ceeeba32d800317205a22b923f5954c6ce67389` (2026-11-14, "docs: document the whole RustLace workspace") | `cargo test -p lovelace-lang` in a `git worktree` at that revision, Linux WSL Ubuntu 24.04.4, exit 0: `test result: ok. 1737 passed; 0 failed; 0 ignored; ...` | 2026-11-16 |
| **1749** | `c7d9840cd87d9b488c29e52535516bebf4c53f1f` (v0.4.1, 2026-11-15) | same command in a worktree at the tag: `test result: ok. 1749 passed; 0 failed; 0 ignored; ...` | 2026-11-16 |
| **1750** | `fa87ecc6abe41d3e174d62b211dd6b6fac2e29b7` (2026-11-15) | `cargo test -p lovelace-lang` in this tree: `test result: ok. 1750 passed; 0 failed; 0 ignored; ...` | 2026-11-16 |

The +13 delta is accounted for by two commits:

- **+12** `eigen_jacobi_round` tests added by
  `5b5fe1087ee17ceb3ffb85ab7cf72085479433a0` ("feat: add eig_sym Jacobi symmetric eigensolver",
  2026-11-14, between `7ceeeba` and v0.4.1);
- **+3 − 2 = +1** movingavg tests in
  `1f114c9e5b76d5f8dd9e9ea50a96253543466f09` ("fix(movingavg): return the exact quotient instead
  of a defective reference rendering", 2026-11-15): three exact-quotient tests replace two
  fixed-width-rendering tests.

Test-name lists from `cargo test -p lovelace-lang --lib -- --list` at the three revisions differ
by exactly:

```text
7ceeeba -> v0.4.1 (+12, all in eigen_jacobi_round):
  complex_entries_are_refused_as_real_only
  diagonal_matrix_is_sorted_with_permuted_identity_vectors_and_no_sweeps
  empty_matrix_is_refused
  five_by_five_householder_matrix_has_spectrum_one_to_five
  non_square_matrix_is_refused
  non_symmetric_matrix_is_refused
  one_by_one_matrix_is_the_trivial_case
  registry_descriptor_pins_the_declared_surface
  repeated_runs_are_bit_identical
  residual_tracks_the_ambient_precision
  three_by_three_block_returns_two_three_four
  two_by_two_returns_one_and_three_with_orthonormal_vectors

v0.4.1 -> HEAD (+3, -2, all in dsp_round20):
  + movingavg_given_fractional_samples_divides_exactly
  + movingavg_given_periodic_samples_is_exact_at_every_precision
  + movingavg_matches_exact_division_at_every_precision
  - movingavg_given_fractional_samples_keeps_the_fixed_width_period_rendering
  - movingavg_given_periodic_samples_marks_the_fixed_width_result_inexact
```

### 5. Linux 715 and Windows 716 stay separate

- **Linux desktop, measured at `fa87ecc` on 2026-11-16:**
  `xvfb-run -a -s "-screen 0 1600x1000x24" cargo test -p lovelace-desktop` →
  `test result: ok. 711 passed; ...` in `src/main.rs` plus
  `test result: ok. 4 passed; ...` in `tests/linux_cli.rs` = **715 passed, 0 failed, 0 ignored**.
  The earlier v0.4.1 record reports the same 711 + 4 split.
- **Windows desktop, recorded but NOT measured here: 716 passed, 0 failed, 0 ignored.** There is
  no Windows host in this environment. The 716 is a carried observation; it is not inferred from
  715, and 715 is not inferred from 716.

The ten tests that do not compile on Linux (explaining the difference) are unchanged and listed
in `docs/testing.md §1`: `platform_parity` 3, `win32/shell.rs` 3, two `document_io.rs` rename
tests, one `workbench.rs` atomic-save test, one `netcheck.rs` PE test.

### 6. What could not be measured (not inferred)

| Item | Status | Reason |
|---|---|---|
| Windows desktop count | **Not measured** | No Windows host available (WSL Ubuntu only). 716 remains a labelled recorded observation. |
| The 6,135 Windows workspace aggregate | **Recorded, not re-measured** | Reconstructed from measured rows plus the recorded Windows 716; no Windows host. |
| Hosted GitHub Actions runs | **Not run** | This round is local only; RQ-013 owns hosted verification. |
| `7ceeeba`/v0.4.1 CI test runs | **Not run** | The counts above come from local worktrees of those revisions on Linux, not from their CI. |

### 7. Files changed for RQ-011

- `docs/testing.md` §1 rewritten: a pinned current table (35 targets / 6147 passed / 1 ignored
  with commit, OS, command, date), the reconciled `6,067 + 56 + 12 = 6,135` table, the
  `1737 / 1749 / 1750` delta with commit SHAs, and Linux 715 vs Windows 716 kept apart.
  §6's inline desktop comment now points at the measured record.
- `AGENTS.md`: the fixed `6135`/desktop-at-v0.4.1 sentence is gone; the contributor guide now
  points at `docs/testing.md` for counts and says not to quote a historical total as current.
- **README.md (out of scope, needs its owner):**
  - `README.md:105` — `test result: ok. 716 passed; ...` in the command transcript.
  - `README.md:160` — `$ cargo test --workspace   # 6135 tests (1 ignored) on v0.4.1, across the twelve crates`.
  Both duplicate stale fixed counts. Recommended: replace with a pointer to
  `docs/testing.md#1-unit-tests-in-crate-test`, or state the commit/OS with the number.
- **`.dsh/skills/release-and-publish/SKILL.md` (out of scope, no change needed):** already says
  "Read the counts from the run; never adopt a number from an earlier release" (`:135-138`) and
  "A count from an earlier release is not evidence about this one" (`:262-266`); a search for
  `6135|716|1737|1750` finds nothing. It points at `docs/testing.md` as the record.
- **`docs/release.md:238`** already says "Expect the counts in [testing.md](testing.md)"; no
  fixed count there.

---

## RQ-010 — the crate-page Rust examples compile and run

### What `tools/check-crate-examples.sh` does

- Reads every `docs/crates/*.md` page (13 today), finds every fenced ` ```rust ` block, and
  writes each body to its own generated binary.
- Builds one throwaway Cargo package in `$(mktemp -d)` with path dependencies on the seven
  engine crates the fences may use (`lovelace-array`, `lovelace-int`, `lovelace-json`,
  `lovelace-lang`, `lovelace-nat`, `lovelace-real`, `lovelace-symbolics`), compiles all binaries,
  then executes each one.
- Maps generated-file diagnostics back to the Markdown page: a compile error or panic is printed
  as `docs/crates/page.md:LINE`, where `LINE` is the page line the generated line came from.
  Build failures are exit 1 and are independent of `check-docs.sh`/`check-reference-pages.sh`.
- Supports one explicit exclusion annotation on the line immediately above the opening fence:

  ````markdown
  <!-- not-compiled: <reason> -->
  ```rust
  <body>
  ```
  ````

  Every exclusion is printed as `EXCLUDED page:line (reason: ...)`; the final line reports
  `<N> compiled and ran, <N> excluded`; `EXCLUDED (none)` is printed when the list is empty. An
  annotation that is not immediately above a rust fence, or has an empty reason, is a failure —
  an exemption cannot rot silently.
- Exit codes: 0 all included fences compiled and ran; 1 a compile/assertion/annotation failure;
  2 bad usage, missing tree, or zero rust fences (zero work is not a pass).
- Uses only bash, awk, sed, grep, mktemp and a `cargo` on `PATH`; no network, no external
  crates. The temporary crate is removed by an `EXIT` trap unless `--keep` is passed.

### Clean-tree run (exit 0, full exclusion report)

```console
$ CARGO_TARGET_DIR=/tmp/rl-target bash tools/check-crate-examples.sh
PASS docs/crates/lovelace-array.md:83
PASS docs/crates/lovelace-int.md:71
PASS docs/crates/lovelace-json.md:69
PASS docs/crates/lovelace-lang.md:95
PASS docs/crates/lovelace-nat.md:87
PASS docs/crates/lovelace-real.md:94
PASS docs/crates/lovelace-symbolics.md:84
EXCLUDED (none)
check-crate-examples: 13 page(s), 7 rust fence(s): 7 compiled and ran, 0 excluded
check-crate-examples: OK
[exit code: 0]
```

All seven maintained Rust fences compile and execute their assertions, including the corrected
`lovelace-int` `div_rem` / `div_rem_signed` example (the task's validation target). No annotation
is needed on any current page.

An independent run with `CARGO_TARGET_DIR=/tmp/rl-crate-examples-fresh` (a fresh target directory,
not the populated `/tmp/rl-target`) also exited 0 with the same 7 compiled / 0 excluded result, so
the gate does not depend on reused build artifacts.

### Exclusion audit: an annotated fence is reported, not silently skipped

Probe on a **copy** of `docs/crates/lovelace-json.md` with
`<!-- not-compiled: probe only: illustrative JSON fragment -->` inserted immediately above its
fence (copy removed afterwards):

```console
$ bash tools/check-crate-examples.sh
EXCLUDED docs/crates/zz-probe-excluded.md:70 (reason: probe only: illustrative JSON fragment)
PASS docs/crates/lovelace-array.md:83
PASS docs/crates/lovelace-int.md:71
PASS docs/crates/lovelace-json.md:69
PASS docs/crates/lovelace-lang.md:95
PASS docs/crates/lovelace-nat.md:87
PASS docs/crates/lovelace-real.md:94
PASS docs/crates/lovelace-symbolics.md:84
check-crate-examples: 14 page(s), 8 rust fence(s): 7 compiled and ran, 1 excluded
check-crate-examples: OK
[exit code: 0]
```

A stale annotation (blank line before the fence) is rejected, which proves the audit is not a
free-form skip:

```console
FAIL docs/crates/zz-probe-stale.md:69: not-compiled annotation is not immediately above a ```rust fence (reason: probe only)
check-crate-examples: annotation audit failed
[exit code: 1]
```

### Assertion corruption, restore, and a passing rerun

Procedure: copied `docs/crates/lovelace-int.md` to `docs/crates/zz-probe-int.md`, changed the
first expected quotient in the copy from `"-3"` to `"999"` (the real page was never touched),
ran the gate, removed the copy, then reran clean.

```console
$ bash tools/check-crate-examples.sh        # with the corrupted copy in docs/crates/
PASS docs/crates/lovelace-array.md:83
PASS docs/crates/lovelace-int.md:71
PASS docs/crates/lovelace-json.md:69
PASS docs/crates/lovelace-lang.md:95
PASS docs/crates/lovelace-nat.md:87
PASS docs/crates/lovelace-real.md:94
PASS docs/crates/lovelace-symbolics.md:84

thread 'main' (305244) panicked at docs/crates/zz-probe-int.md:77:1:
assertion `left == right` failed
  left: ("-3", "-2")
 right: ("999", "-2")
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
FAIL docs/crates/zz-probe-int.md:77: fence did not run to completion (exit 101)
check-crate-examples: 1 of 8 included fence(s) failed
[exit code: 1]
```

Restore evidence: the real pages were hashed before and after all probes and are byte-identical
(the copy was deleted, not the page restored):

```text
3663b723caf88c5b38cd4aab44dff3746410c6604993d6478f8ba79ded8a8724  docs/crates/lovelace-int.md
bdd8a0eac614fc4df61f6a88e954a1fe6aaa9cb6caf9ed29ccb8ea30f5177c46  docs/crates/lovelace-json.md
--- diff of before/after hash files ---
RESTORE=byte-identical
```

Clean rerun after deleting the copy:

```console
$ bash tools/check-crate-examples.sh
PASS docs/crates/lovelace-array.md:83
PASS docs/crates/lovelace-int.md:71
PASS docs/crates/lovelace-json.md:69
PASS docs/crates/lovelace-lang.md:95
PASS docs/crates/lovelace-nat.md:87
PASS docs/crates/lovelace-real.md:94
PASS docs/crates/lovelace-symbolics.md:84
EXCLUDED (none)
check-crate-examples: 13 page(s), 7 rust fence(s): 7 compiled and ran, 0 excluded
check-crate-examples: OK
[exit code: 0]
```

### Syntax/API drift is a build failure with `file:line`

A second copied page renamed `Nat::from_u64` to `Nat::from_u64_typo` in the `lovelace-int` fence:

```console
error[E0599]: no associated function or constant named `from_u64_typo` found for struct `Nat` in the current scope
   --> docs/crates/zz-probe-api.md:75:20
    |
  6 | ...t::from_u64_typo(5);
    |       ^^^^^^^^^^^^^ associated function or constant not found in `Nat`
...
error: could not compile `rustlace-crate-examples-check` (bin "fence_08") due to 1 previous error
check-crate-examples: the crate-page examples do not compile; a syntax/API change is a build failure
[exit code: 1]
```

The `-->` location is the page line (`zz-probe-api.md:75`). The rustc code-frame row still shows
the generated line number (`6`); the mapped `-->` and the `FAIL` line are the page locations.

### RQ-010 limits and follow-ups

- The gate builds seven engine crates because those are the crates the current pages use. A
  future fence that needs `lovelace-run`, `lovelace-know`, `lovelace-corpus` or `lovelace-bench`
  needs that path dependency added to the generated `Cargo.toml` (the script fails loudly with an
  unresolved-crate error until then).
- The script is **not wired into CI**: `.github/workflows/ci.yml` is owned by another agent, and
  the round-2 plan defers the docs-job step to round 3 so CI never references a file still being
  written.
- No crate page currently needs a `not-compiled` annotation; the annotation path was proved on a
  copy only.
- `docs/crates/lovelace-int.md` already carried the corrected fence from the previous round; this
  gate is what now compiles and executes it on every run, and its assertions (quotient/remainder
  for both division forms, `None` on both zero divisors) pass.

---

## Gate status and residual actions for the orchestrator

| Gate | Status | Evidence |
|---|---|---|
| C10 (RQ-010) | **Met locally** | 7/7 fences compile+run, exit 0; assertion and API-drift falsifications fail with page:line; exclusion report auditable |
| C11 (RQ-011) | **Met locally** | current 6147/0/1 pinned to `fa87ecc`/Linux/2026-11-16; 6067 + 56 + 12 = 6135; 1737/1749/1750 by commit; 715/716 separated |

Actions outside my file ownership:

1. **README owner:** replace the stale fixed counts at `README.md:105` and `README.md:160` with a
   pointer to the measured record.
2. **CI owner (round 3):** add `bash tools/check-crate-examples.sh` to the documentation job on
   both OS variants; no conditional skip (RQ-006.5).
3. **Goal ledger:** the C11 text says "the missing 68 is `lovelace-know`". The measured split is
   `lovelace-know` 56 + `lovelace-run` integration 12 = 68. The page records the measured split;
   the acceptance text should note the 56/12 composition rather than ask for a 68-test
   `lovelace-know` row, which no revision supports.

## Reproduction index

```console
# current counts (this host)
CARGO_TARGET_DIR=/tmp/rl-target xvfb-run -a -s "-screen 0 1600x1000x24" cargo test --workspace --no-fail-fast
CARGO_TARGET_DIR=/tmp/rl-target cargo test -p <crate>

# historical revisions (worktrees; removed after measurement)
git worktree add --detach /tmp/rl-7ceeeba 7ceeeba
git worktree add --detach /tmp/rl-v041 v0.4.1

# crate-page examples
bash tools/check-crate-examples.sh          # add --keep to retain the generated crate
bash tools/check-docs.sh
bash tools/check-reference-pages.sh
```

Raw local logs (not committed) were under `/tmp/rl-counts/`: `workspace.log`, `measure.log`,
`7ceeeba-lang.log`, `7ceeeba-lang-list.txt`, `v041-lang.log`, `v041-know-with-run.log`,
`probe-assert.log`, `probe-api.log`, `probe-excluded.log`, `probe-stale.log`, `probe-clean.log`.

# Round 1 — DOC-001 code half: two supported platforms

Implementer report. Date: 2026-11-15. Environment: Windows host, no Windows Rust; all Rust
commands below run in WSL Ubuntu against `/mnt/c/Users/ricar/dev/RustLace` with
`CARGO_TARGET_DIR=/tmp/rl-target`.

## Scope

| File | Change |
|---|---|
| `lovelace-desktop/src/platform_claims.rs` | supported-platform set is now Windows + Linux; `matrix_problems`, `unsupported_notice`, `probe_report`/`run` and the tests moved to the two-platform invariant |
| `docs/goal-desktop-v4/platform-matrix.md` | Linux row promoted to supported with factual CI/WSLg evidence; false "still reads LATER PORT" banner and "only platform this run can certify" note replaced |
| `.github/workflows/release.yml` | **untouched**: grep found no platform-claim manifest, only artifact-name comments |

`release.yml` grep evidence (no hit for a claim/manifest/matrix/support statement):

```console
$ Select-String -Path .github/workflows/release.yml -Pattern 'platform.?claim|platform_claims|supported platform|Windows 11|matrix'
# no matches
```

## Baseline (reproduced before changes)

`cargo test -p lovelace-desktop platform_claims` → `5 passed; 0 failed`, including
`exactly_one_platform_is_the_certified_windows_release`.

Linux binary `--platform-claims` (before):

```text
this build: target_os=linux arch=x86_64 advertised=false
refusal[linux/x86_64]: lovelace-desktop: the native workbench is unsupported on linux/x86_64: this build has no UI adapter for that platform (the supported platform for this release is Windows 11 x64). No headless placeholder is started; exiting.
result PASS: 4 platform row(s), 0 problem(s)
# exit 0 — the Linux build refused itself and certified itself
```

## Test-first

Tests were changed first, before the production code and before the matrix row. The probe-output
test was added together with the `probe_report` extraction (it references that new function, so it
could not compile against the old tree); the five failures below were observed on the old code.
Command as run:

```console
wsl.exe -d Ubuntu -- bash -c 'cd /mnt/c/Users/ricar/dev/RustLace && export CARGO_TARGET_DIR=/tmp/rl-target && /home/ricardo/.cargo/bin/cargo test -p lovelace-desktop platform_claims 2>&1 | tee /tmp/test-first.log | tail -140'
```

Observed failing output (cargo's pre-existing warning block elided; test-harness output verbatim):

```text
running 9 tests
test platform_claims::tests::an_empty_matrix_is_reported_rather_than_accepted ... ok
test platform_claims::tests::a_silent_row_is_reported_rather_than_accepted ... ok
test platform_claims::tests::only_platforms_without_a_gui_adapter_are_refused ... FAILED
test platform_claims::tests::a_supported_row_without_run_evidence_is_still_rejected ... FAILED
test platform_claims::tests::more_than_one_supported_row_is_accepted ... FAILED
test platform_claims::tests::the_release_matrix_marks_every_unshipped_platform_explicitly ... ok
test platform_claims::tests::the_linux_row_is_advertised_by_the_release_matrix ... FAILED
test platform_claims::tests::windows_and_linux_are_both_the_certified_release ... FAILED
test platform_claims::tests::the_probe_reads_the_shipped_matrix_and_exits_zero ... ok

failures:

---- platform_claims::tests::only_platforms_without_a_gui_adapter_are_refused stdout ----

thread 'platform_claims::tests::only_platforms_without_a_gui_adapter_are_refused' (52548) panicked at lovelace-desktop/src/platform_claims.rs:273:13:
the refusal must name Linux as one of the supported platforms: lovelace-desktop: the native workbench is unsupported on macos/x86_64: this build has no UI adapter for that platform (the supported platform for this release is Windows 11 x64). No headless placeholder is started; exiting.

---- platform_claims::tests::a_supported_row_without_run_evidence_is_still_rejected stdout ----

thread 'platform_claims::tests::a_supported_row_without_run_evidence_is_still_rejected' (52545) panicked at lovelace-desktop/src/platform_claims.rs:345:9:
a supported row with no run evidence must be reported: ["Linux x86_64 is neither supported with run evidence nor explicitly unsupported: status=\"**SUPPORTED (X11)**\""]

---- platform_claims::tests::more_than_one_supported_row_is_accepted stdout ----

thread 'platform_claims::tests::more_than_one_supported_row_is_accepted' (52547) panicked at lovelace-desktop/src/platform_claims.rs:323:9:
assertion `left == right` failed: two supported rows are both accepted when each carries run evidence
  left: ["Linux x86_64 is neither supported with run evidence nor explicitly unsupported: status=\"**SUPPORTED (X11)**\""]
 right: []
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- platform_claims::tests::the_linux_row_is_advertised_by_the_release_matrix stdout ----

thread 'platform_claims::tests::the_linux_row_is_advertised_by_the_release_matrix' (52549) panicked at lovelace-desktop/src/platform_claims.rs:368:9:
the Linux row must be advertised by this release: Claim { platform: "Linux x86_64", status: "**LATER PORT** (not in v1)", run: "none" }

---- platform_claims::tests::windows_and_linux_are_both_the_certified_release stdout ----

thread 'platform_claims::tests::windows_and_linux_are_both_the_certified_release' (52552) panicked at lovelace-desktop/src/platform_claims.rs:234:9:
assertion `left == right` failed: Windows x86_64 and Linux x86_64 (X11) both claim a built-and-run GUI, got [Claim { platform: "Windows 11 x64 (10.0.22631 or later)", status: "**INITIAL RELEASE**", run: "required: native window launched, screenshot captured, GUI suite executed" }]
  left: 1
 right: 2


failures:
    platform_claims::tests::a_supported_row_without_run_evidence_is_still_rejected
    platform_claims::tests::more_than_one_supported_row_is_accepted
    platform_claims::tests::only_platforms_without_a_gui_adapter_are_refused
    platform_claims::tests::the_linux_row_is_advertised_by_the_release_matrix
    platform_claims::tests::windows_and_linux_are_both_the_certified_release

test result: FAILED. 4 passed; 5 failed; 0 ignored; 0 measured; 701 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p lovelace-desktop --bin lovelace-desktop`
```

## After the change

### `cargo test -p lovelace-desktop platform_claims`

Command as run:

```console
wsl.exe -d Ubuntu -- bash -c 'cd /mnt/c/Users/ricar/dev/RustLace && export CARGO_TARGET_DIR=/tmp/rl-target && /home/ricardo/.cargo/bin/cargo test -p lovelace-desktop platform_claims > /tmp/final-platform-test.log 2>&1'
# observed exit status: 0
```

Observed passing output (from `/tmp/final-platform-test.log`, warnings elided):

```text
running 10 tests
test platform_claims::tests::more_than_one_supported_row_is_accepted ... ok
test platform_claims::tests::a_silent_row_is_reported_rather_than_accepted ... ok
test platform_claims::tests::a_supported_row_without_run_evidence_is_still_rejected ... ok
test platform_claims::tests::an_empty_matrix_is_reported_rather_than_accepted ... ok
test platform_claims::tests::only_platforms_without_a_gui_adapter_are_refused ... ok
test platform_claims::tests::windows_and_linux_are_both_the_certified_release ... ok
test platform_claims::tests::the_release_matrix_marks_every_unshipped_platform_explicitly ... ok
test platform_claims::tests::the_probe_reads_the_shipped_matrix_and_exits_zero ... ok
test platform_claims::tests::the_linux_probe_advertises_linux_and_never_refuses_it ... ok
test platform_claims::tests::the_linux_row_is_advertised_by_the_release_matrix ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 701 filtered out; finished in 0.00s

     Running tests/linux_cli.rs (/tmp/rl-target/debug/build/lovelace-desktop/a8a5344be0841e3b/out/linux_cli-a8a5344be0841e3b)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s
```

### `--platform-claims` on the Linux build

Command as run:

```console
wsl.exe -d Ubuntu -- bash -c '/tmp/rl-target/debug/lovelace-desktop --platform-claims'
```

Observed output (all lines; exit status 0):

```text
lovelace-desktop platform claims (PLF-009)
release metadata: /mnt/c/Users/ricar/dev/RustLace/lovelace-desktop/../docs/goal-desktop-v4/platform-matrix.md
platform | status | run evidence | verdict
Windows 11 x64 (10.0.22631 or later) | **INITIAL RELEASE** | required: native window launched, screenshot captured, GUI suite executed | SUPPORTED (built + run GUI declared)
Windows 10 x64 | **UNSUPPORTED (untested)** | none | EXPLICITLY NOT SHIPPED
Linux x86_64 | **SUPPORTED (X11)** | CI: under `xvfb-run`, `--gui-selftest` runs 20 checks (0 failed) and `cargo test -p lovelace-desktop` runs the desktop suite. One window captured on WSLg: `docs/images/gui-linux.png`. | SUPPORTED (built + run GUI declared)
macOS (arm64 and x86_64) | **LATER PORT** (not in v1) | none | EXPLICITLY NOT SHIPPED
this build: target_os=linux arch=x86_64 advertised=true
supported[linux/x86_64]: this release has a UI adapter for it
refusal[macos/aarch64]: lovelace-desktop: the native workbench is unsupported on macos/aarch64: this build has no UI adapter for that platform (the supported platforms for this release are Windows 11 x64 and Linux x86_64 (X11)). No headless placeholder is started; exiting.
result PASS: 4 platform row(s), 0 problem(s)
```

`refusal[linux/x86_64]` no longer appears; `advertised=true`; the macOS refusal names both supported
platforms and still refuses a headless placeholder.

### Full desktop suite (regression, Linux under Xvfb)

```console
wsl.exe -d Ubuntu -- bash -c "cd /mnt/c/Users/ricar/dev/RustLace && export CARGO_TARGET_DIR=/tmp/rl-target && xvfb-run -a -s '-screen 0 1600x1000x24' /home/ricardo/.cargo/bin/cargo test -p lovelace-desktop 2>&1 | tail -25"
```

Observed: `test result: ok. 711 passed; 0 failed; 0 ignored` for the binary suite and
`test result: ok. 4 passed; 0 failed` for `tests/linux_cli.rs` (715 total; the 711 is the previous
706 plus the 5 new `platform_claims` tests).

`cargo fmt --all -- --check` → `FMT_OK` (exit 0).

## Diff summary

```console
$ git diff --stat -- lovelace-desktop/src/platform_claims.rs docs/goal-desktop-v4/platform-matrix.md .github/workflows/release.yml
 docs/goal-desktop-v4/platform-matrix.md |  23 ++-
 lovelace-desktop/src/platform_claims.rs | 342 ++++++++++++++++++++++++--------
 2 files changed, 281 insertions(+), 84 deletions(-)
```

`platform-matrix.md` already carried uncommitted Phase-0 banner edits before this round, so this
stat is cumulative against `HEAD`; the Linux row, the Windows note, the banner and the "Rules"
readers bullet are this round's edits. `release.yml` is unchanged.

## Test rename/change ledger (no weakening)

- `exactly_one_platform_is_the_certified_windows_release` **renamed** to
  `windows_and_linux_are_both_the_certified_release`: the old `supported.len() == 1` / "must be the
  Windows release" assertions encoded the one-platform invariant; the new test asserts `== 2` and
  that both the Windows and Linux rows are in the supported set — stricter, nothing dropped.
- `every_other_platform_gets_a_refusal_that_names_it` **renamed** to
  `only_platforms_without_a_gui_adapter_are_refused`: the old name/assertion required Linux to be
  refused, which is now false; the new test keeps the refusal assertions for macOS/FreeBSD (name,
  "unsupported", "No headless placeholder") and adds that the notice names Windows and Linux and
  that Windows/Linux return `None` — assertions added, none removed.
- `more_than_one_supported_row_is_accepted` **added**: two supported rows with run evidence must
  yield no problems; it failed on the old code (single-platform rule), so it is new coverage, not a
  relaxation.
- `a_supported_row_without_run_evidence_is_still_rejected` **added**: pins the "supported row with
  `none` evidence" rejection explicitly; it failed on the old code's generic wording, and it keeps
  the required rejection stronger, not weaker.
- `an_empty_matrix_is_reported_rather_than_accepted` **added**: pins the empty-parse rejection
  (`the platform matrix could not be parsed`), which the old code also had; it is a regression
  guard, not a change of verdict.
- `the_linux_row_is_advertised_by_the_release_matrix` **added**: parses the shipped matrix and
  requires the Linux row to be advertised and to claim a built-and-run GUI; it failed on the old
  row (`LATER PORT`).
- `the_linux_probe_advertises_linux_and_never_refuses_it` **added** alongside the `probe_report`
  extraction: pins the exact printed verdict (`advertised=true`, `supported[linux/x86_64]`, no
  `refusal[linux/x86_64]`, `result PASS`); it could not compile before the extraction because the
  helper did not exist.
- Unchanged tests: `the_release_matrix_marks_every_unshipped_platform_explicitly`,
  `a_silent_row_is_reported_rather_than_accepted`, `the_probe_reads_the_shipped_matrix_and_exits_zero`
  — same assertions, still green.

No test was deleted, no `#[ignore]` was added, no fixture or assertion was dropped.

## Matrix honesty notes

- Linux run evidence states what exists: the CI job builds the release GUI on Ubuntu 24.04, runs
  `--gui-selftest` (20 checks, 0 failed) and `cargo test -p lovelace-desktop` under `xvfb-run`, and
  one window was captured on WSLg at `docs/images/gui-linux.png`. The Windows wording ("native
  window launched, screenshot captured, GUI suite executed") is **not** copied into the Linux row.
- Wayland is stated as untested in the Linux Notes cell; journey evidence is stated as
  Windows-only; `--gui-selftest` is stated as a run-path check, not a journey pass.
- The banner no longer says the table reads `LATER PORT` and no longer says the table is "not the
  current support statement"; the Windows note no longer says "the only platform this run can
  certify"; the last "Rules" bullet now names the actual readers (`--platform-claims` and the
  `platform_claims` tests) instead of the absent `gate/register.json`/release-manifest reference.

## Not passing / not done

- Nothing in the scoped verification is failing.
- A Windows binary run was not possible here (no Windows Rust); the same tests and matrix are
  platform-independent, and only Linux was executed. This is a verification-environment gap, not a
  claimed pass.
- Wayland remains untested by design and is documented as such; no Wayland claim was made.

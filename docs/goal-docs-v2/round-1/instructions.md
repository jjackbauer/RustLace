# Round 1 — correctness of current instructions (RQ-004, RQ-007 part 1, RQ-012)

**Branch/revision:** `docs/documentation-quality-remediation`; base commit `fa87ecc`, plus this round's
working-tree edits to the five owned files.
**Host:** WSL Ubuntu; `CARGO_TARGET_DIR=/tmp/rl-target`; cargo `/home/ricardo/.cargo/bin/cargo`.
**Owned files:** `docs/editorial.md`, `.dsh/skills/sync-docs/SKILL.md`,
`.dsh/skills/release-and-publish/SKILL.md`, `AGENTS.md`, `.github/copilot-instructions.md`.

## RQ-004 — single-supported-platform instruction regression

### What was stale before this round

- `docs/editorial.md:76-78` (pre-edit) told contributors that `platform-matrix.md` is parsed "by tests
  that assert a single supported row". That invariant was removed when Linux was promoted: the
  current code supports two platforms.
- `.dsh/skills/sync-docs/SKILL.md:21` (pre-edit) repeated the same invariant as an instruction — the
  matrix "keeps its single-supported-row shape". An agent following that line would try to delete
  a supported row.

### What the code actually requires

- lovelace-desktop/src/platform_claims.rs:28 — `SUPPORTED_PLATFORMS: [&str; 2] = ["windows",
  "linux"]`.
- lovelace-desktop/src/platform_claims.rs:142-185 — `matrix_problems`: a supported claim with
  `none` run evidence is a problem (`150-162`); every entry of `SUPPORTED_PLATFORMS` needs a
  built-and-run GUI row (`163-172`); every other row must be explicitly unsupported (`173-183`).
- lovelace-desktop/src/platform_claims.rs:189-199 — `release_problems`: a supported platform must
  never be refused by the same module that advertises it.
- Test names that pin the two-platform behaviour: `more_than_one_supported_row_is_accepted`
  (lovelace-desktop/src/platform_claims.rs:371),
  `the_linux_probe_advertises_linux_and_never_refuses_it` (`:441`),
  `the_probe_reads_the_shipped_matrix_and_exits_zero` (`:468`).

### Changes made

- `docs/editorial.md:76-80` — editorial rule 3 now requires a matrix row with built-and-run GUI
  evidence for **each** platform in `SUPPORTED_PLATFORMS`, says a support claim without run evidence
  is rejected, and states that adding, removing or restating a supported platform is a code change.
  The single-row assertion is gone.
- `.dsh/skills/sync-docs/SKILL.md:21` — the platform map row now requires the 10-test
  `platform_claims` suite, "every supported platform has a matrix row with built-and-run GUI
  evidence", and "no supported platform is refused by its own build".
- `.dsh/skills/sync-docs/SKILL.md:24-28` — new coverage paragraph: current matrix coverage, not a
  single supported row; names `SUPPORTED_PLATFORMS` and `matrix_problems` in
  `lovelace-desktop/src/platform_claims.rs`, requires evidence for every supported platform, and
  calls an instruction that expects one supported row a regression.

### No instruction tells an agent to remove, skip or refuse Linux support

- `.github/copilot-instructions.md:7` — the desktop "builds and runs on Windows and on Linux (X11)".
- `.github/copilot-instructions.md:36-37` — "The desktop is not Windows-only."
- `AGENTS.md:21-23` — the workbench "builds and runs on Windows and on Linux (X11)".
- `AGENTS.md:109-111` — both platform jobs are documented, and the documentation job is added (RQ-012).
- `.dsh/skills/release-and-publish/SKILL.md:21-26` — the Linux executable is "**not** optional and is
  **not** a placeholder".
- `.dsh/skills/release-and-publish/SKILL.md:253-255` — "do not omit, skip or weaken the Linux job".
- The only "Windows-only" phrase in the five files is `.dsh/skills/release-and-publish/SKILL.md:256`
  ("the journey captures are Windows-only"), which describes capture provenance, not support.

### Test evidence — 10 tests, 0 failed

```console
$ xvfb-run -a -s "-screen 0 1600x1000x24" cargo test -p lovelace-desktop platform_claims
     Running unittests src/main.rs (/tmp/rl-target/debug/build/lovelace-desktop/18922ec53b1c167f/out/lovelace_desktop-18922ec53b1c167f)

running 10 tests
test platform_claims::tests::an_empty_matrix_is_reported_rather_than_accepted ... ok
test platform_claims::tests::a_supported_row_without_run_evidence_is_still_rejected ... ok
test platform_claims::tests::a_silent_row_is_reported_rather_than_accepted ... ok
test platform_claims::tests::only_platforms_without_a_gui_adapter_are_refused ... ok
test platform_claims::tests::more_than_one_supported_row_is_accepted ... ok
test platform_claims::tests::windows_and_linux_are_both_the_certified_release ... ok
test platform_claims::tests::the_probe_reads_the_shipped_matrix_and_exits_zero ... ok
test platform_claims::tests::the_linux_probe_advertises_linux_and_never_refuses_it ... ok
test platform_claims::tests::the_release_matrix_marks_every_unshipped_platform_explicitly ... ok
test platform_claims::tests::the_linux_row_is_advertised_by_the_release_matrix ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 701 filtered out; finished in 0.00s
```

## RQ-007 part 1 — the docs-sync filter, proved nonzero

### Where the docs-sync tests actually live

- lovelace-lang/src/docs_sync.rs:126-147 — the `#[cfg(test)] mod tests` of the `docs_sync` module;
  its one test is `docs_sync::tests::parser_reads_examples_and_snippets_like_the_reference`
  (function at `:131`).
- lovelace-lang/src/round93_eval_print.rs:506-537 — the two `DocsSyncTests` that read the C#
  oracle under `docs/csharp-oracle/`:
  `round93_eval_print::docssynctests_every_c_sharp_snippet_appears_in_usage_examples` (`:509`) and
  `round93_eval_print::docssynctests_every_lovelace_example_has_an_expected_result` (`:522`).

Cargo's test filter is a substring match over the module path. `docs_sync` selects the module test
(1 test); `docssync` selects the two `docssynctests_*` oracle tests (2 tests). A single filter that
selects all three does not exist, which is why the skill now runs both.

### What was wrong and what changed

- Pre-edit `.dsh/skills/sync-docs/SKILL.md:88` ran `cargo test -p lovelace-lang docssync`, while the
  same page's success line (pre-edit `:93`) required `docs_sync` to pass. The filter never selected
  the `docs_sync` module test the page demanded, and the page contained no rule that proved any
  test had actually run — a filter that matched nothing would have exited 0 and been recorded as a
  pass.
- `.dsh/skills/sync-docs/SKILL.md:93-97` — the Verify block now runs
  `cargo test -p lovelace-lang docs_sync`, `cargo test -p lovelace-lang docssync`, and
  `cargo test -p lovelace-run golden_batch`, with comments naming what each filter selects.
- `.dsh/skills/sync-docs/SKILL.md:99-111` — new section "Prove the filter selected a test": read the
  `test result:` line, require a nonzero passed count (`1 passed` for `docs_sync`, `2 passed` for
  `docssync`) and `0 failed`; reject `running 0 tests` or `0 passed`; run
  `cargo test -p lovelace-lang <filter> -- --list` before adopting or renaming a filter.
- `.dsh/skills/sync-docs/SKILL.md:113-117` — observable success now requires every filtered run to
  report its nonzero passed count with `0 failed`.

### Observed output

```console
$ cargo test -p lovelace-lang docs_sync
running 1 test
test docs_sync::tests::parser_reads_examples_and_snippets_like_the_reference ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1749 filtered out; finished in 0.01s

$ cargo test -p lovelace-lang docssync
running 2 tests
test round93_eval_print::docssynctests_every_lovelace_example_has_an_expected_result ... ok
test round93_eval_print::docssynctests_every_c_sharp_snippet_appears_in_usage_examples ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1748 filtered out; finished in 0.00s
```

## RQ-012 — release instructions aligned with actual CI

### What the workflows define (read before rewriting the skill)

- `.github/workflows/ci.yml:29-64` — job `Windows (full workspace)` on `windows-latest`.
- `.github/workflows/ci.yml:66-171` — job `Linux (engine crates + GUI desktop under Xvfb)` on
  `ubuntu-24.04` (pinned, `:74`); glibc ABI guard `:125-147`; `--gui-selftest` `:153-157`; desktop
  suite `:161-162`.
- `.github/workflows/ci.yml:178-229` — job `Documentation examples (${{ matrix.os }})`; matrix
  `[ubuntu-latest, windows-latest]` `:183-184`; LF-pinned checkout `:190-195`; both example runners
  and `check-docs.sh` `:209-223`; `check-reference-pages.sh` `:228-229`.
- `.github/workflows/release.yml:26-45` — the four assets and the "built at the tagged commit" rule;
  `:295-311` writes `SHA256SUMS.windows`; `:344-357` creates the release; `:369-538` the Linux job;
  `:461-479` renames to `lovelace-desktop-linux-x86_64` and writes `SHA256SUMS.linux`; `:481-490`
  uploads; `:494-515` downloads and re-verifies; `:520-538` checks all four assets.

### Acceptance-criterion mapping

| Criterion | Satisfied at |
|---|---|
| 1. Windows, Linux and documentation job definitions, including the two OS variants of the docs job | `.dsh/skills/release-and-publish/SKILL.md:38-54` |
| 2. Release checks include the mandatory documentation job, not only "both jobs green" | `:40-42`, `:238-251`, `:292-299` |
| 3. Platform-specific build/run/checksum instructions separated; no Windows command presented as Linux | gate `:91-141`; artifact identity `:143-162`; signing `:164-187` (Windows only); publish `:202-236`; Verify `:268-290` |
| 4. Manual procedure handles both binaries and both platform-qualified checksums | asset table `:16-19`; publish `:202-236`; "all four assets" `:28-33` and `:230-236` |
| 5. Version examples are placeholders/variables or labelled illustrative; no instruction to tag a fixed version | `:189-200` (`<version>`, with `0.2.0` labelled as an example) |
| 6. Historical test counts are not immutable acceptance criteria | `:135-141`, `:262-266` ("Read the counts from the run; never adopt a number from an earlier release") |

Checks run against the new text: `grep -nE 'v0\.1\.0|6135|716|710|214'` over the skill printed
nothing (exit 1), so the fixed tag and the old counts are gone.

### AGENTS.md alignment (dependent contributor instruction)

- `AGENTS.md:104-111` — "three job definitions", and a new `Documentation examples` row naming both
  OS variants.
- `AGENTS.md:113-119` — the documentation job's two-OS purpose is stated, the glibc guard is added to
  the Linux row, and "No job captures a window" replaces the two-job phrasing.
- `AGENTS.md:259-261` — the gate the guardrail names is now "the Windows, Linux and documentation
  jobs", both documentation variants included.
- `.dsh/skills/release-and-publish/SKILL.md:56-65` — "Files you will touch" now lists
  `.github/workflows/release.yml` and the three-job `ci.yml`.

## VERIFY — observed output

The script that produced this block is `/tmp/rl-rq-verify.sh` (outside the repository, so the stale
patterns used by the grep are not re-introduced into `docs/`). Test order inside a suite varies
between runs; the counts and the summary line do not. The `check-docs` link/reference counts exclude
`docs/goal-docs-*` as harness working memory — a concurrent round widened that exclusion from
`goal-docs-v1` to the wildcard while this round ran, so the file/reference counts are a point-in-time
observation; the claims page count still scans every Markdown page, including this report.

```console
$ cargo test -p lovelace-lang docs_sync
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.34s
running 1 test
test docs_sync::tests::parser_reads_examples_and_snippets_like_the_reference ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1749 filtered out; finished in 0.00s
[exit code: 0]

$ cargo test -p lovelace-lang docssync
running 2 tests
test round93_eval_print::docssynctests_every_lovelace_example_has_an_expected_result ... ok
test round93_eval_print::docssynctests_every_c_sharp_snippet_appears_in_usage_examples ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1748 filtered out; finished in 0.00s
[exit code: 0]

$ xvfb-run -a -s "-screen 0 1600x1000x24" cargo test -p lovelace-desktop platform_claims
running 10 tests
test platform_claims::tests::an_empty_matrix_is_reported_rather_than_accepted ... ok
test platform_claims::tests::a_supported_row_without_run_evidence_is_still_rejected ... ok
test platform_claims::tests::a_silent_row_is_reported_rather_than_accepted ... ok
test platform_claims::tests::only_platforms_without_a_gui_adapter_are_refused ... ok
test platform_claims::tests::more_than_one_supported_row_is_accepted ... ok
test platform_claims::tests::windows_and_linux_are_both_the_certified_release ... ok
test platform_claims::tests::the_probe_reads_the_shipped_matrix_and_exits_zero ... ok
test platform_claims::tests::the_linux_probe_advertises_linux_and_never_refuses_it ... ok
test platform_claims::tests::the_release_matrix_marks_every_unshipped_platform_explicitly ... ok
test platform_claims::tests::the_linux_row_is_advertised_by_the_release_matrix ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 701 filtered out; finished in 0.00s
[exit code: 0]

$ bash tools/check-docs.sh --claims
check-docs: 57 Markdown files, 158 relative links, 234 code references
check-docs: claims audit: 83 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: OK
[exit code: 0]

$ bash tools/check-docs.sh --readme
check-docs: 57 Markdown files, 158 relative links, 234 code references
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
[exit code: 0]

$ bash tools/check-docs.sh --symbols      # the mode the docs CI job runs; extra check
check-docs: 57 Markdown files, 158 relative links, 234 code references, 20 line-suffix citations, 19 symbol citations
check-docs: OK
[exit code: 0]

$ grep -rn -E "$C4_PATTERN" docs/editorial.md .dsh/skills/sync-docs/SKILL.md \
    .dsh/skills/release-and-publish/SKILL.md AGENTS.md .github/copilot-instructions.md
[grep exit code: 1]            # no match in any of the five owned files

$ grep -rn -E "$C4_PATTERN" docs .dsh .github AGENTS.md
docs/goal-docs-v1/round-5/implementation.md:194: (historical round report quoting the old row)
docs/goal-docs-v2/evidence.md:7: (EVD-001, records the old editorial defect)
docs/goal-docs-v2/evidence.md:8: (EVD-002, records the old skill defect)
docs/goal-docs-v2/goal.md:72: (the C4 check itself, quoting both patterns)
docs/goal-docs-v2/intent-audit.md:8: (the RQ-004 audit row, quoting both patterns)
[grep exit code: 0]

$ git diff --stat -- docs/editorial.md .dsh/skills/sync-docs/SKILL.md \
    .dsh/skills/release-and-publish/SKILL.md AGENTS.md .github/copilot-instructions.md
 .dsh/skills/release-and-publish/SKILL.md | 251 ++++++++++++++++++++++---------
 .dsh/skills/sync-docs/SKILL.md           |  36 ++++-
 AGENTS.md                                |  22 +--
 docs/editorial.md                        |   6 +-
 4 files changed, 229 insertions(+), 86 deletions(-)
```

The five required-scope grep hits are not current instructions: `docs/goal-docs-v1/` is preserved
harness working memory, and `docs/goal-docs-v2/` quotes the defect and the C4 pattern in its own
goal/evidence files. All five are outside this round's owned files, so they are left for the
orchestrator (see Residuals). The five lines are summarized by `path:line` rather than pasted
verbatim because restating the matched text here would add new hits to the same gate; the raw output
is in `/tmp/rq-verify-final.log`, and `C4_PATTERN` is defined in the external script as the two
alternatives named in `docs/goal-docs-v2/goal.md:72`.

## Per-file changelog

| File | Change |
|---|---|
| `docs/editorial.md` | Rule 3 rewritten at `:76-80`: per-platform matrix coverage in `SUPPORTED_PLATFORMS`, built-and-run evidence for each supported platform, support claims without run evidence rejected, platform-status edits are code changes. |
| `.dsh/skills/sync-docs/SKILL.md` | Platform row rewritten at `:21`; new coverage paragraph at `:24-28`; Verify commands corrected to `docs_sync` + `docssync` + `golden_batch` at `:93-97`; new nonzero-test guard at `:99-111`; observable success at `:113-117`. |
| `.dsh/skills/release-and-publish/SKILL.md` | Rewritten (299 lines): three-job CI table including both docs OS variants (`:38-54`); platform-separated gate, identity, signing and publish blocks; both binaries and both checksums; placeholder versions; historical counts rejected as acceptance criteria; all-three-jobs check before announcing. |
| `AGENTS.md` | `:104-111` now says three job definitions and adds the documentation job row; `:113-119` adds the docs-job description and the glibc guard; `:259-261` gates on the Windows, Linux and documentation jobs. Line endings normalised to LF so the diff is content-only. |
| `.github/copilot-instructions.md` | Audited, no content change required: it already says the desktop builds and runs on Windows and Linux (`:7`), that non-Windows targets add glow (`:36`) and "The desktop is not Windows-only" (`:37`). Line endings normalised to LF, so `git diff` for this file is empty. |

## Residuals and limits

1. The required-scope grep cannot be empty while the goal's own working logs quote the defect:
   `docs/goal-docs-v1/round-5/implementation.md:194` and `docs/goal-docs-v2/evidence.md:7-8`,
   `docs/goal-docs-v2/goal.md:72`, `docs/goal-docs-v2/intent-audit.md:8` contain the two patterns.
   None is an instruction, and none is in this round's file list. Recommendation: scope C4's grep to
   the five instruction files (all zero, shown above), or annotate/move those historical hits.
2. `docs/release.md:230-252` (the release checklist, not owned by this round) still omits the
   documentation job from its manual gate. The release skill now requires it; the dependent page
   should be updated in the same change if RQ-012 is judged to cover it.
3. `AGENTS.md:63-71` still carries observed test counts (workspace 6135, corpus 214) without a
   commit pin. That is RQ-011's file to reconcile; `AGENTS.md` is in this round's exclusive write
   set, so route the count change through this file's owner rather than editing it in parallel.
4. The `ci.yml` documentation job currently invokes `tools/check-docs.sh` with `--symbols`; RQ-006
   will add the other modes. The release skill deliberately says "every mode the step invokes" and
   lists all three in the manual gate, so it stays correct when RQ-006 lands.

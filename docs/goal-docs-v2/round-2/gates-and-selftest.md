# RQ-006 and RQ-012 — the documentation gates run in CI and each one is falsifiable

**Branch:** `docs/documentation-quality-remediation` · **Revision:** `fa87ecc` (working tree) ·
**Date:** 2026-11-16 · **Host:** WSL Ubuntu, `CARGO_TARGET_DIR=/tmp/rl-target`.
Windows/Git Bash execution was not available on this host; `tools/test-docs-gates.sh` is bash and
the CI docs job runs it on both `ubuntu-latest` and `windows-latest`.

## Result

| Requirement | Status | Where |
|---|---|---|
| RQ-006 — every implemented documentation-consistency mode runs in CI | met | `.github/workflows/ci.yml`, docs job: `--symbols`, `--claims`, `--readme`, the reference-page gate, both example runners, and the gate self-test |
| RQ-006 — a negative test per mode | met | `tools/test-docs-gates.sh`: 17 mutations, each requiring a non-zero exit **and** a diagnostic naming the file and the problem |
| RQ-012 — release instructions match the actual CI | met | `docs/release.md`: the Windows, Linux and **both** documentation job variants are named as release gates, and the manual checklist gains the documentation gate |

No requirement was satisfied by weakening a check: the existing example step, matrix, reference-page
step and `--symbols` step are all intact; the change only adds modes and the self-test.

## Files changed (all inside this task's ownership)

```text
 .github/workflows/ci.yml        | 40 changed (34 insertions, 6 deletions)
 docs/release.md                 | 52 changed (47 insertions, 5 deletions)
 tools/test-docs-gates.sh        | new file, 709 lines (17 mutations + CI-wiring assertions)
 docs/goal-docs-v2/round-2/gates-and-selftest.md   new file (this report)
 2 tracked files changed, 81 insertions(+), 11 deletions(-); 2 new files
```

`tools/check-docs.sh`, `tools/check-reference-pages.sh`, both `run-examples.sh` scripts and every
other documentation page were **not** modified.

---

## RQ-006.1 — all three checker modes run

`check-docs.sh` accepts one or several flags at a time. The job uses three separate steps (the
existing `--symbols` step is extended, not replaced), so a failure is attributable to one mode.
Every invocation also runs the base relative-link / path-shaped-code-reference pass, which is why
cases 1–2 below are covered by the same steps.

```yaml
      - name: Check documentation links and symbol citations
        shell: bash
        run: bash tools/check-docs.sh --symbols

      - name: Check documentation claims and tutorial precision
        shell: bash
        run: bash tools/check-docs.sh --claims

      - name: Check the README structure
        shell: bash
        run: bash tools/check-docs.sh --readme
```

## RQ-006.2 — the reference-page gate is retained

```yaml
      - name: Check reference pages
        shell: bash
        run: bash tools/check-reference-pages.sh
```

## RQ-006.3 — both example checks, on Ubuntu and Windows

The step and the matrix are unchanged:

```yaml
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest]
    ...
      - name: Run the language and extension examples
        shell: bash
        run: |
          set -uo pipefail
          status=0
          bash docs/language/examples/run-examples.sh || status=1
          bash docs/extensions/examples/run-examples.sh || status=1
          exit "$status"
```

`|| status=1` is not `|| true`: both runners always execute and either failure is carried into the
step's exit status.

## RQ-006.4 — `tools/test-docs-gates.sh`: a mutation per gate

The self-test builds each defect in a `mktemp` tree, never in the working tree. Each scratch checker
is a byte-for-byte copy of the real one, verified with `cmp`, so the test cannot pass by exercising a
stub. Every case first runs the clean fixture (`CTRL … clean fixture passed`) and then the same
fixture with one defect added. The exit is non-zero and the log must contain both the defective path
and the expected problem string; otherwise the case is a `FAIL`.

### Clean-tree gate baselines (before any mutation)

```text
$ bash tools/check-docs.sh
check-docs: 57 Markdown files, 159 relative links, 234 code references
check-docs: OK
EXIT=0
$ bash tools/check-docs.sh --symbols
check-docs: 57 Markdown files, 159 relative links, 234 code references, 20 line-suffix citations, 19 symbol citations
check-docs: OK
EXIT=0
$ bash tools/check-docs.sh --claims
check-docs: 57 Markdown files, 159 relative links, 234 code references
check-docs: claims audit: 84 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: OK
EXIT=0
$ bash tools/check-docs.sh --readme
check-docs: 57 Markdown files, 159 relative links, 234 code references
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
EXIT=0
$ bash tools/check-reference-pages.sh
check-reference-pages: 16 page(s), 112 required sections in order
check-reference-pages: OK
EXIT=0
```

The page/link/reference totals in these transcripts are what the runs observed at capture time.
Another round-2 agent is editing other documentation pages concurrently, so a later run may report a
different total while every gate stays green; the invariant under test is the exit status and the
mutation verdicts, not the totals.

### Full self-test transcript

```text
$ bash tools/test-docs-gates.sh
SETUP lovelace-run: /tmp/rl-target/debug/lovelace-run
SETUP scratch: /tmp/test-docs-gates.ixPuE8
CTRL check-docs base: broken-link fixture: clean fixture passed
PASS check-docs: broken relative link (exit 1)
CTRL check-docs base: missing-code-reference fixture: clean fixture passed
PASS check-docs: path-shaped code reference to a nonexistent file (exit 1)
CTRL check-docs --symbols: .rs:NNN fixture: clean fixture passed
PASS check-docs --symbols: .rs:NNN citation outside docs/adr/ (exit 1)
CTRL check-docs --symbols: live-symbol fixture: clean fixture passed
PASS check-docs --symbols: dead symbol citation (exit 1)
CTRL check-docs --claims: phrase fixture: clean fixture passed
PASS check-docs --claims: prohibited absolutist phrase (exit 1)
CTRL check-docs --claims: tutorial fixture: clean fixture passed
PASS check-docs --claims: tutorial missing '## Precision' (exit 1)
CTRL check-docs --readme: README fixture: clean fixture passed
PASS check-docs --readme: README missing '## Current limitations' (exit 1)
CTRL check-reference-pages: conformant fixture: clean fixture passed
PASS check-reference-pages: missing required section (exit 1)
CTRL check-reference-pages: ordered fixture: clean fixture passed
PASS check-reference-pages: out-of-order section (exit 1)
CTRL language runner: declared-set fixture: clean fixture passed
PASS language runner: undeclared page in the manifest (exit 1)
CTRL language runner: complete-page-set fixture: clean fixture passed
PASS language runner: declared page missing on disk (exit 1)
CTRL language runner: one-example fixture: clean fixture passed
PASS language runner: zero example pairs (exit 1)
CTRL language runner: paired fixture: clean fixture passed
PASS language runner: unpaired lovelace block (exit 1)
CTRL extensions runner: declared-set fixture: clean fixture passed
PASS extensions runner: undeclared page in the manifest (exit 1)
CTRL extensions runner: complete-page-set fixture: clean fixture passed
PASS extensions runner: declared page missing on disk (exit 1)
CTRL extensions runner: one-example fixture: clean fixture passed
PASS extensions runner: zero example pairs (exit 2)
CTRL extensions runner: paired fixture: clean fixture passed
PASS extensions runner: unpaired lovelace block (exit 1)
WIRE ci.yml: three check-docs modes, reference pages, both example runners, the gate self-test, and the Ubuntu+Windows matrix are present; the docs job has no conditional skip

test-docs-gates: 17 mutation(s), 17 passed, 0 failed
test-docs-gates: OK (17 mutations, every gate failed for its intended reason)
SELFTEST_EXIT=0
```

### Mutation → expected diagnostic

| # | Gate | Defect built in the scratch tree | Asserted diagnostic (file + problem) | Exit |
|---|---|---|---|---|
| 1 | `check-docs.sh` | `docs/index.md` gains `[link](missing-page.md)` | `docs/index.md` / `broken link 'missing-page.md'` | 1 |
| 2 | `check-docs.sh` | `docs/index.md` gains `` `lovelace-lang/src/absent.rs` `` | `docs/index.md` / `does not name an existing file` | 1 |
| 3 | `check-docs.sh --symbols` | `docs/index.md` gains `` `lovelace-lang/src/lib.rs:12` `` | `docs/index.md` / `is not allowed outside docs/adr/` | 1 |
| 4 | `check-docs.sh --symbols` | `docs/index.md` gains `` `absent_function()` `` | `docs/index.md` / `is not defined in lovelace-*/src` | 1 |
| 5 | `check-docs.sh --claims` | `docs/index.md` gains `The engine reports exact 30-place results.` | `docs/index.md` / `prohibited numerical claim 'exact 30-place'` | 1 |
| 6 | `check-docs.sh --claims` | `docs/tutorials/first-steps.md` has no `## Precision` | `docs/tutorials/first-steps.md` / `missing '## Precision' section` | 1 |
| 7 | `check-docs.sh --readme` | `## Current limitations` removed from `README.md` | `README.md` / `missing '## Current limitations' section` | 1 |
| 8 | `check-reference-pages.sh` | `## Limitations` removed from the listed page | `docs/reference/thing.md` / `: Limitations` | 1 |
| 9 | `check-reference-pages.sh` | `## Outputs` placed before `## Inputs` | `docs/reference/thing.md` / `out of order; it must come after Outputs` | 1 |
| 10 | language runner | `docs/language/new-page.md` added (with a valid example pair) | `docs/language/new-page.md` / `is not in the page manifest` | 1 |
| 11 | language runner | `docs/language/plots.md` deleted | `docs/language/plots.md` / `declared page is missing or unreadable` | 1 |
| 12 | language runner | the only `lovelace`/`text` pair removed | `README.md` / `refusing to pass on zero work` | 1 |
| 13 | language runner | a `lovelace` fence with no following `text` fence appended | `docs/language/README.md` / `lovelace block has no following text block` | 1 |
| 14 | extensions runner | `docs/extensions/new-page.md` added (with a valid example pair) | `docs/extensions/new-page.md` / `is not in the page manifest` | 1 |
| 15 | extensions runner | `docs/extensions/symbolics.md` deleted | `docs/extensions/symbolics.md` / `declared page is missing or unreadable` | 1 |
| 16 | extensions runner | the only `lovelace`/`text` pair removed | `4 page(s)` / `No examples were found in 4 page(s); nothing was verified.` | 2 |
| 17 | extensions runner | a `lovelace` fence with no following `text` fence appended | `README.md` / `no immediately following text block` | 1 |

Case 16 is tree-level: no single page is at fault, so the runner's diagnostic names the page set
(`4 page(s)`) instead of one file. Every other case is asserted against a file path.

### The self-test cannot pass when a mutation stops failing

The verdict logic is part of the script, not a convention. A mutation that exits 0 (or whose
diagnostic is wrong) is counted as a failed mutation, and the final guard proves every mutation got
exactly one verdict:

```bash
    if [ "$rc" -eq 0 ]; then
        fail_mutation "$label" "the mutated fixture PASSED (exit 0); a mutation that does not fail is not a gate"
    ...
    if [ "$((passed + mutation_failed))" -ne "$mutations" ]; then
        printf 'test-docs-gates: FAIL (%d of %d mutations produced no verdict)\n' ...
        exit 1
    fi
    if [ "$failed" -gt 0 ]; then
        printf 'test-docs-gates: FAIL (%d problem(s))\n' "$failed" >&2
        exit 1
    fi
```

A clean fixture that does not pass is also a `FAIL`, so a case cannot fail for an unrelated reason
and still count as a mutation verdict.

**Demonstration that this is not a claim.** In a throwaway fake repository under `/tmp`, the two
checker files were replaced with `exit 0` stubs (the real checkers were only read, never written; the
combined hash is unchanged). The self-test then failed all nine `check-docs` and reference-page
mutations instead of passing them, and exited 1:

```text
REAL_CHECKERS_BEFORE=299231d3b6dac29922cc633027a50dae8c5e76961425eee1792dfdd28d7002a7
fake checker bodies:
#!/usr/bin/env bash
exit 0
#!/usr/bin/env bash
exit 0
FAKE_SELFTEST_EXIT=1
----- FAIL lines (the self-test catching non-failing gates) -----
FAIL check-docs: broken relative link: the mutated fixture PASSED (exit 0); a mutation that does not fail is not a gate
FAIL check-docs: path-shaped code reference to a nonexistent file: the mutated fixture PASSED (exit 0); a mutation that does not fail is not a gate
FAIL check-docs --symbols: .rs:NNN citation outside docs/adr/: the mutated fixture PASSED (exit 0); a mutation that does not fail is not a gate
FAIL check-docs --symbols: dead symbol citation: the mutated fixture PASSED (exit 0); a mutation that does not fail is not a gate
FAIL check-docs --claims: prohibited absolutist phrase: the mutated fixture PASSED (exit 0); a mutation that does not fail is not a gate
FAIL check-docs --claims: tutorial missing '## Precision': the mutated fixture PASSED (exit 0); a mutation that does not fail is not a gate
FAIL check-docs --readme: README missing '## Current limitations': the mutated fixture PASSED (exit 0); a mutation that does not fail is not a gate
FAIL check-reference-pages: missing required section: the mutated fixture PASSED (exit 0); a mutation that does not fail is not a gate
FAIL check-reference-pages: out-of-order section: the mutated fixture PASSED (exit 0); a mutation that does not fail is not a gate
----- summary -----

test-docs-gates: 17 mutation(s), 8 passed, 9 failed
test-docs-gates: FAIL (9 problem(s))
REAL_CHECKERS_AFTER=299231d3b6dac29922cc633027a50dae8c5e76961425eee1792dfdd28d7002a7
```

The eight runner mutations still passed because those fixtures run the real runner scripts; only the
gates whose checkers had become no-ops were reported as broken gates. In the real tree all 17 pass,
so each gate demonstrably fails for its intended reason.

## RQ-006.5 — mandatory steps, no conditional skip

- Three `check-docs` steps, one reference-page step, one example step, one self-test step: all are
  unconditional steps of the docs job.
- No `|| true`, no `continue-on-error`, no `if:` in the docs job.
- The self-test re-checks this on every run: it greps `ci.yml` for all three modes, the
  reference-page command, both runner commands, its own command and the
  `os: [ubuntu-latest, windows-latest]` matrix, then asserts the docs job block has no `|| true`,
  no `continue-on-error` and no `if:`. A mode dropped from CI turns the self-test red:
  `FAIL ci wiring: ci.yml does not contain: bash tools/check-docs.sh --claims`.

## Real working-tree mutation, caught and restored byte-identically

Two temporary mutations were made against the working tree and restored; `sha256sum` before and
after proves byte identity. Nothing outside the two files was touched.

### Mutation 1 — a broken relative link in `docs/release.md` (caught by `check-docs.sh`)

```text
TARGET=docs/release.md
SHA256_BEFORE=d6f1f1a37bfb99e3bc3ac9763bf10840b816005d056360b34a1a1b81430d332c
docs/release.md:296: broken link 'definitely-absent-from-the-tree.md' (no such file or directory: docs/definitely-absent-from-the-tree.md)
check-docs: 57 Markdown files, 160 relative links, 234 code references
check-docs: FAIL (1 failure(s))
MUTATED_EXIT=1
SHA256_AFTER=d6f1f1a37bfb99e3bc3ac9763bf10840b816005d056360b34a1a1b81430d332c
RESTORE=byte-identical
check-docs: 57 Markdown files, 159 relative links, 234 code references
check-docs: OK
RESTORED_EXIT=0
```

### Mutation 2 — the `--claims` step removed from `ci.yml` (caught by the self-test's wiring guard)

```text
TARGET=.github/workflows/ci.yml
SHA256_BEFORE=6ba1c48772fa82fe18f2a5fbb6ab463307203e2ea83d370fa3074caf02f5361c
--- self-test against the mutated workflow ---
MUTATED_EXIT=1
FAIL ci wiring: ci.yml does not contain: bash tools/check-docs.sh --claims
test-docs-gates: 17 mutation(s), 17 passed, 1 failed
test-docs-gates: FAIL (1 problem(s))
SHA256_AFTER=6ba1c48772fa82fe18f2a5fbb6ab463307203e2ea83d370fa3074caf02f5361c
RESTORE=byte-identical
--- self-test after the restore ---
RESTORED_EXIT=0
test-docs-gates: 17 mutation(s), 17 passed, 0 failed
test-docs-gates: OK (17 mutations, every gate failed for its intended reason)
```

## Final `ci.yml` docs job (verbatim)

```yaml
  docs:
    name: Documentation examples (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest]
    steps:
      # windows-latest checks out with the runner's system core.autocrlf=true,
      # which would give the bash runners CRLF and break them before they run.
      # Pin LF for this job's checkout only; the windows and linux jobs above
      # keep their existing checkout behaviour.
      - name: Checkout
        uses: actions/checkout@v4
        env:
          GIT_CONFIG_COUNT: '1'
          GIT_CONFIG_KEY_0: core.autocrlf
          GIT_CONFIG_VALUE_0: 'false'

      - name: Install stable Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo registry and target
        uses: Swatinem/rust-cache@v2

      - name: Build lovelace-run
        run: cargo build -p lovelace-run

      # `shell: bash` is the system bash on ubuntu-latest and Git Bash on
      # windows-latest. Both runners always execute; the step fails if either
      # exits non-zero, so a wrong recorded output fails the workflow.
      - name: Run the language and extension examples
        shell: bash
        run: |
          set -uo pipefail
          status=0
          bash docs/language/examples/run-examples.sh || status=1
          bash docs/extensions/examples/run-examples.sh || status=1
          exit "$status"

      # The documentation consistency gate has three implemented modes, and all
      # three run. check-docs.sh always performs the relative-link and
      # path-shaped-code-reference pass; each flag adds one audit on top of it:
      #   --symbols  no .rs:NNN citations outside docs/adr/ and no dead symbols
      #   --claims   no prohibited absolutist claim, every tutorial has ## Precision
      #   --readme   README value proposition, limitations, command and order
      # Separate steps keep a failure attributable to one mode. No step is
      # conditional and none is allowed to fail without failing the job.
      - name: Check documentation links and symbol citations
        shell: bash
        run: bash tools/check-docs.sh --symbols

      - name: Check documentation claims and tutorial precision
        shell: bash
        run: bash tools/check-docs.sh --claims

      - name: Check the README structure
        shell: bash
        run: bash tools/check-docs.sh --readme

      # The reference-page contract (DOC-014): every page listed in
      # docs/reference-pages.md carries the seven sections of
      # docs/reference-template.md, in order, with no other level-2 heading.
      - name: Check reference pages
        shell: bash
        run: bash tools/check-reference-pages.sh

      # Each gate above has a recorded mutation test: tools/test-docs-gates.sh
      # builds the defect in a scratch tree, runs the real checker against it,
      # and requires a non-zero exit with a diagnostic naming the file and the
      # problem. A mutation that stops failing fails this step, so a gate
      # cannot quietly become a no-op. It also asserts that this workflow still
      # invokes every mode, so the self-test goes red when a mode is removed
      # from the job instead of passing on a smaller set.
      - name: Test the documentation gates (mutation)
        shell: bash
        run: bash tools/test-docs-gates.sh
```

The `windows` and `linux` job definitions were not changed; only the top-of-file comment that
describes the docs job was extended.

## RQ-012 — `docs/release.md` diff summary

```text
 docs/release.md | 52 +++++++++++++++++++++++++++++++++++++++++++-----
 1 file changed, 47 insertions(+), 5 deletions(-)
```

1. **New `### What must be green before a tag`** section under `## Automatic releases`. It states
   that the release workflow's success condition is the whole `CI` workflow, then names all three
   job definitions and both documentation variants in a table: `Windows (full workspace)`,
   `Linux (engine crates + GUI desktop under Xvfb)`, `Documentation examples (ubuntu-latest)` and
   `Documentation examples (windows-latest)`. It calls the documentation job "one definition that
   runs twice" and a release gate, lists the documentation defects that block a tag, and says one
   green variant does not vouch for the other.
2. **The manual release checklist gains the documentation gate** as item 10: both example runners,
   `check-docs.sh --symbols`, `--claims`, `--readme`, `check-reference-pages.sh`, and
   `test-docs-gates.sh`, with the observable `test-docs-gates: OK`. Items 10–11 were renumbered
   11–12.
3. **Test counts removed from the checklist** (the corpus's `214` and the selftest's `20` checks):
   both now state the shape the run must print (`corpus: <N> passed, 0 failed, <N> total`,
   `GUI_SELFTEST summary checks=<N> failed=0` with N nonzero), and the new section says explicitly
   that the gate is an exit status and that counts are read from this commit's run and compared with
   the dated table in [testing.md](testing.md). No undated count was added.

## Residuals and limits

1. **No hosted CI run.** All evidence above is local WSL Ubuntu. RQ-013/C13 (green hosted runs at
   the final commit) is outside this task and is not claimed here.
2. **Windows/Git Bash not executed on this host.** The self-test uses only bash, `awk`, `grep`,
   `find`, `cp`, `cmp`, `mktemp` and `sed`, and resolves the Windows `.exe` suffix the same way the
   runners do; the docs job executes it on `windows-latest`, but that execution has not happened
   yet.
3. **The self-test needs a built `lovelace-run`.** It exits 2 with a "build it first" message
   otherwise; the docs job builds it before the gate steps, so the precondition is satisfied in CI.
4. **`AGENTS.md` still describes the docs job without the self-test.** `AGENTS.md` is out of scope
   here; its CI table should gain `bash tools/test-docs-gates.sh` and the three modes in the change
   that owns that file.

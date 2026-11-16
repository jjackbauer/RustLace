# Round 5 — implementation report

**Date:** 2026-11-15
**Scope:** DOC-008 / D9 (archival boundary), D17 (documentation-sync skill), D19 (epistemic limit),
D12 (editorial enforcement path).
**Environment:** WSL Ubuntu, `/home/ricardo/.cargo/bin/cargo` 1.100.0, `CARGO_TARGET_DIR=/tmp/rl-target`.
No native Windows Rust is installed.

## How the WSL commands were run

PowerShell strips the inner quotes from `xvfb-run -s "-screen 0 1600x1000x24"` when the line crosses
into `wsl.exe`; the first attempt failed with `bash: -c: line 1: syntax error near unexpected token '('`.
Every WSL command in this report was therefore sent as a newline script, base64-decoded inside WSL:

```powershell
$lines = @(
  'cd /mnt/c/Users/ricar/dev/RustLace',
  'export CARGO_TARGET_DIR=/tmp/rl-target',
  'export PATH=/home/ricardo/.cargo/bin:$PATH',
  '<the exact inner command shown below>'
)
$b = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(($lines -join "`n")))
wsl.exe -d Ubuntu -- bash -c "echo $b | base64 -d | bash"
```

The `bash` scripts below are the exact inner commands; the outputs are verbatim.

---

## 1. DOC-008 / D9 — archival boundary in `journeys.md`

**Change.** A six-line blockquote at the top of `docs/goal-desktop-v4/journeys.md` (after the H1,
before the intro paragraph and the table). It states that the file is a historical carried
specification, that the `round-33/`, `round-44/`, `round-47/`, `round-61/` and `deviations.md`
capture artifacts live in the LovelaceSharp repository and are deliberately not carried, and that the
`--journey` tests read the file for its expected observables. The table was not touched.

### 1.1 Journey tests — BEFORE the edit

Command (inside WSL):

```bash
cd /mnt/c/Users/ricar/dev/RustLace
export CARGO_TARGET_DIR=/tmp/rl-target
export PATH=/home/ricardo/.cargo/bin:$PATH
xvfb-run -a -s "-screen 0 1600x1000x24" cargo test -p lovelace-desktop journey
```

Verbatim output (the compile output above this excerpt was pre-existing crate warnings; the test
execution follows):

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.13s
     Running unittests src/main.rs (/tmp/rl-target/debug/build/lovelace-desktop/18922ec53b1c167f/out/lovelace_desktop-18922ec53b1c167f)

running 7 tests
test journey::tests::fractional_digits_counts_only_digits_after_the_point ... ok
test journey::tests::only_the_artifact_writing_journeys_need_the_output_directory ... ok
test journey::tests::a_mismatch_is_a_failure_and_a_match_is_not ... ok
test journey::tests::options_require_a_journey_and_reject_unknown_flags ... ok
test journey::tests::sha256_matches_the_nist_vectors ... ok
test journey::tests::con_007_the_recorded_references_are_deterministic_and_the_desktop_scenarios_execute_against_them ... ok
test ui::app::tests::the_keyboard_journey_opens_edits_runs_inspects_switches_panes_and_exports ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 704 filtered out; finished in 1.30s

     Running tests/linux_cli.rs (/tmp/rl-target/debug/build/lovelace-desktop/a8a5344be0841e3b/out/linux_cli-a8a5344be0841e3b)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s

EXIT_CODE=0
```

### 1.2 Journey tests — AFTER the edit

Command (identical):

```bash
cd /mnt/c/Users/ricar/dev/RustLace
export CARGO_TARGET_DIR=/tmp/rl-target
export PATH=/home/ricardo/.cargo/bin:$PATH
xvfb-run -a -s "-screen 0 1600x1000x24" cargo test -p lovelace-desktop journey
```

Verbatim output (compile warnings omitted; the test execution follows):

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 9.11s
     Running unittests src/main.rs (/tmp/rl-target/debug/build/lovelace-desktop/18922ec53b1c167f/out/lovelace_desktop-18922ec53b1c167f)

running 7 tests
test journey::tests::a_mismatch_is_a_failure_and_a_match_is_not ... ok
test journey::tests::fractional_digits_counts_only_digits_after_the_point ... ok
test journey::tests::only_the_artifact_writing_journeys_need_the_output_directory ... ok
test journey::tests::options_require_a_journey_and_reject_unknown_flags ... ok
test journey::tests::sha256_matches_the_nist_vectors ... ok
test journey::tests::con_007_the_recorded_references_are_deterministic_and_the_desktop_scenarios_execute_against_them ... ok
test ui::app::tests::the_keyboard_journey_opens_edits_runs_inspects_switches_panes_and_exports ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 704 filtered out; finished in 1.26s

     Running tests/linux_cli.rs (/tmp/rl-target/debug/build/lovelace-desktop/a8a5344be0841e3b/out/linux_cli-a8a5344be0841e3b)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s

EXIT_CODE=0
```

Both runs: **7 passed, 0 failed, 0 ignored, 704 filtered out** in the unit target and **0 passed,
4 filtered out** in `tests/linux_cli.rs`; exit 0 both times. The journey test that parses
`journeys.md` (`con_007_...`) passed in both runs.

### 1.3 The table is untouched

Windows `git` (the authoritative status for this checkout):

```console
$ git diff --stat -- docs/goal-desktop-v4/journeys.md
 docs/goal-desktop-v4/journeys.md | 7 +++++++
 1 file changed, 7 insertions(+)

$ git diff --numstat -- docs/goal-desktop-v4/journeys.md
7       0       docs/goal-desktop-v4/journeys.md
```

There are no deletions: the banner is a pure insertion after line 2. A content comparison of the
table from `| # |` to EOF, with CR normalised, is identical:

```bash
echo '=== table comparison with CR normalised ==='
git show HEAD:docs/goal-desktop-v4/journeys.md | sed -n '/^| # |/,$p' | tr -d '\r' > /tmp/rl-table-before.txt
sed -n '/^| # |/,$p' docs/goal-desktop-v4/journeys.md | tr -d '\r' > /tmp/rl-table-after.txt
cmp /tmp/rl-table-before.txt /tmp/rl-table-after.txt && echo "TABLE CONTENT IDENTICAL (CR-normalised)"
wc -l /tmp/rl-table-before.txt /tmp/rl-table-after.txt
```

```text
=== table comparison with CR normalised ===
TABLE CONTENT IDENTICAL (CR-normalised)
   33 /tmp/rl-table-before.txt
   33 /tmp/rl-table-after.txt
   66 total
```

The raw-byte note: this Windows checkout stores CRLF in the worktree (`file` reports "with CRLF line
terminators") while the Git blob is LF, and WSL's git has `core.autocrlf` unset while Windows git has
it `true`. A raw-byte `cmp` and WSL `git diff` therefore show every line as changed
(`50 43`), which is the CRLF/LF artifact, not a table edit. Normalising CR and comparing the table
from `| # |` to EOF gives 33 identical lines, which is the actual table and running rules.

---

## 2. D17 — the `sync-docs` documentation-sync skill

**Change.** New `.dsh/skills/sync-docs/SKILL.md` with the same frontmatter shape and
numbered-procedure style as the other skills. It contains the dependent-document map (registry ->
`docs/language/` builtin reference; behaviour/error text -> ADR and `parity/corpus/`; platform
assumption -> `docs/goal-desktop-v4/platform-matrix.md`, `docs/adr/` and the release skill; wire
envelope -> `docs/crates/lovelace-run.md` and the golden fixtures), an explicit "check semantic drift,
not that a Markdown file was edited" procedure, and a `## Verify` section. It is indexed from
`docs/skills.md` (row appended; the table now has nine data rows, so this is the tenth table row
counting the header) and from the "If you want to X, read the skill" table in `AGENTS.md`.

Verification command (inside WSL):

```bash
cd /mnt/c/Users/ricar/dev/RustLace
grep -n "sync-docs" docs/skills.md AGENTS.md
sed -n '1,4p' .dsh/skills/sync-docs/SKILL.md
sed -n '15,22p' .dsh/skills/sync-docs/SKILL.md
grep -n "semantic drift" .dsh/skills/sync-docs/SKILL.md
grep -n "## Verify" .dsh/skills/sync-docs/SKILL.md
```

Verbatim output:

```text
docs/skills.md:18:| [sync-docs](../.dsh/skills/sync-docs/SKILL.md) | Keeps the dependent documents in step with a change: registry to the builtin reference, behaviour and error text to the ADR and corpus, platform assumptions to the matrix and release skill, wire envelope to the CLI page and golden fixtures. | A code change alters what a document says, or a documentation edit must be proved by re-running its example rather than by the diff. |
AGENTS.md:279:| update the dependent documentation after a change | [sync-docs](.dsh/skills/sync-docs/SKILL.md) |
---
name: sync-docs
description: Keep the dependent documentation in step with a builtin, behaviour, platform or wire-envelope change by following the dependent-document map, re-running the affected documented example for semantic drift, and verifying the links, corpus records and golden fixtures.
whenToUse: Use when a code change alters what a document says, when a documentation edit must be proved by re-running its example rather than by the diff, or when you are asked to update docs after an API, error-text, platform or wire-envelope change.
## Dependent-document map

| When this changes | Update these documents | The check that proves it |
|---|---|---|
| The builtin registry: a descriptor, name, arity, help text or category | the builtin reference under `docs/language/` (`docs/language/builtins.md`), and any help, completion or functions list that renders the entry | run the documented example with `lovelace-run`; the builtin's tests and the corpus record still pass |
| A behaviour, its output, or an error sentence or code | the ADR under `docs/adr/` that records the decision, and the record in `parity/corpus/` | `cargo run --release -p lovelace-corpus`; the record quotes the new sentence exactly |
| A platform assumption, support claim, or build/release step | `docs/goal-desktop-v4/platform-matrix.md`, the relevant `docs/adr/` record, and `.dsh/skills/release-and-publish/SKILL.md` | `cargo test -p lovelace-desktop platform_claims`; the matrix keeps its one-supported-row shape |
| The wire envelope: a field, its order, its type or its error shape | `docs/crates/lovelace-run.md` and the golden fixtures under `docs/csharp-oracle/Lovelace.Run.Tests/fixtures/` | `cargo test -p lovelace-run golden_batch` |
3:description: Keep the dependent documentation in step with a builtin, behaviour, platform or wire-envelope change by following the dependent-document map, re-running the affected documented example for semantic drift, and verifying the links, corpus records and golden fixtures.
40:3. **Check semantic drift, not that a Markdown file was edited.** Re-run the affected documented
74:## Verify
```

The two test filters used by the new `## Verify` section were checked to exist and match:

```console
$ cargo test -p lovelace-lang docssync -- --list
     Running unittests src/lib.rs (/tmp/rl-target/debug/build/lovelace-lang/de7ac7dec5c21661/out/lovelace_lang-de7ac7dec5c21661)
round93_eval_print::docssynctests_every_c_sharp_snippet_appears_in_usage_examples: test
round93_eval_print::docssynctests_every_lovelace_example_has_an_expected_result: test

2 tests, 0 benchmarks

$ cargo test -p lovelace-run golden_batch -- --list
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.38s
     Running unittests src/main.rs (/tmp/rl-target/debug/build/lovelace-run/f92f2e05c62d1bc2/out/lovelace_run-f92f2e05c62d1bc2)
golden_batch::a_recorded_envelope_reproduces_every_golden: test
golden_batch::error_envelope_carries_a_structural_code_and_category: test
golden_batch::every_fixture_matches_its_golden_envelope: test
golden_batch::every_fixture_script_has_a_corpus_row: test
golden_batch::rich_result_diagnostics_cross_the_wire_as_an_array: test
golden_batch::solve_record_carries_the_structural_solve_contract: test
golden_batch::the_recorder_is_inert_unless_enabled: test
golden_batch::wire_diagnostic_carries_the_six_frozen_fields_in_order: test

8 tests, 0 benchmarks
EXIT_CODE=0
```

Those are list runs, not pass runs; see "Not verified" below.

---

## 3. D19 — the epistemic limit in `editorial.md` and `testing.md`

**Change.** One statement in each file:

- `docs/editorial.md`, in "The three kinds of claim": "Automated wording and link checks can verify
  consistency, but they cannot establish mathematical correctness. A numerical claim needs an
  independent oracle, a reference calculation, or a stated error bound; a checker that confirms a
  sentence was written or a link resolves is not evidence that the value it states is right."
- `docs/testing.md`, immediately after the five-layer introduction: the same limit, with "a suite
  that asserts nothing about the value is a consistency check, not a proof."

Verification command (inside WSL):

```bash
grep -n "cannot establish mathematical correctness" docs/editorial.md docs/testing.md
```

Verbatim output:

```text
docs/editorial.md:47:cannot establish mathematical correctness. A numerical claim needs an independent oracle, a
docs/testing.md:7:consistency, but they cannot establish mathematical correctness: a numerical claim needs an
```

`docs/editorial.md` line 46 carries the first half of the sentence ("Automated wording and link checks
can verify consistency, but they"); line 47 carries the exact phrase the D19 check greps for.

---

## 4. D12 — the editorial enforcement path

**Change.** A `### Documentation requirements` subsection in `AGENTS.md` under `## Conventions`, and a
`## Documentation requirements` section in `docs/contributing.md` after the review expectations. Both
name `docs/editorial.md` as a review requirement for documentation changes and state the two blocking
rules: the exact / precision-bounded / parity label on every numerical claim, and the
repository-relative-or-placeholder path rule.

Verification command (inside WSL):

```bash
grep -n "Documentation requirements" AGENTS.md docs/contributing.md
grep -n "docs/editorial.md" AGENTS.md docs/contributing.md
grep -n -E "exact, precision-bounded or parity|repository-relative path" AGENTS.md docs/contributing.md
```

Verbatim output:

```text
AGENTS.md:189:### Documentation requirements
docs/contributing.md:75:## Documentation requirements
AGENTS.md:191:Documentation changes are reviewed against [docs/editorial.md](docs/editorial.md). Two rules are
docs/contributing.md:78:[docs/editorial.md](editorial.md). Two rules are blocking:
AGENTS.md:192:blocking: every numerical claim is labelled exact, precision-bounded or parity, and every example
AGENTS.md:193:uses a repository-relative path or a placeholder rather than a path from one machine.
docs/contributing.md:79:every numerical claim says whether it is exact, precision-bounded or parity, and every example uses a
docs/contributing.md:80:repository-relative path or a placeholder instead of a path from one machine. A link check does not
```

---

## 5. Scope check

`git status --short` scoped to the files in this round:

```text
 M AGENTS.md
 M docs/contributing.md
 M docs/goal-desktop-v4/journeys.md
 M docs/skills.md
 M docs/testing.md
?? .dsh/skills/sync-docs/SKILL.md
?? docs/editorial.md
?? docs/goal-docs-v1/round-5/implementation.md
```

`docs/editorial.md` is pre-existing but untracked in this worktree, so it shows `??`; the round's
edits are inside it. `AGENTS.md`, `docs/skills.md` and `docs/testing.md` already carried working-tree
edits from earlier rounds, so their `M` status is not exclusive to this round; the hunks added here
are the sections quoted above. Nothing outside the declared scope was edited. The report itself is
the only new file under `docs/goal-docs-v1/`.

## 6. Not verified / limits

- I ran the journey-filtered test target, not the full desktop suite or `cargo test --workspace`; the
  round asked specifically for the journey tests, and both runs are pasted above.
- I ran `--list` on the two test filters named in the new skill, not the tests themselves, so the
  skill's `## Verify` commands are checked to resolve to real tests but their pass status is not
  claimed here.
- I did not run `bash docs/language/examples/run-examples.sh` or the extension runner this round, and
  do not claim their counts.
- The `--journey` mode itself (`cargo run -p lovelace-desktop -- --journey ...`) was not run; the
  unit tests that read `journeys.md` for the expected observables were run before and after.
- The raw-byte WSL comparison of `journeys.md` reports CRLF/LF churn because the Windows checkout is
  CRLF and WSL git does not normalise; the CR-normalised table comparison above is the table proof.
- Out-of-scope observation: `docs/goal-desktop-v4/platform-matrix.md` is not in this round's scope and
  currently carries no `Historical snapshot` marker, so the D9 grep over
  `docs/goal-desktop-v4/*.md` is only satisfied for `journeys.md` by this round. That file's half
  needs the agent or round that owns it.

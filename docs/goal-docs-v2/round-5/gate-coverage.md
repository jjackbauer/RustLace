# Round 5 — RQ-015, RQ-016 and TODO-005: closing the gate-coverage blind spots

**Branch:** `docs/documentation-quality-remediation` · **Revision:** `d946316` (working tree) ·
**Date:** 2026-11-16 · **Host:** WSL Ubuntu, `CARGO_TARGET_DIR=/tmp/rl-target`.

## Result

| Item | Status | Where the observable is |
|---|---|---|
| RQ-015 / C15 — maintained Markdown links, heading anchors, root pages and agent instructions | **met** | `tools/check-docs.sh` scans 70 pages in three classes and resolves 48 `#fragment` links against GitHub-style heading slugs |
| RQ-016 / C16 — reference manifest is two-way and rejects empty sections | **met** | `tools/check-reference-pages.sh` + `docs/reference-pages.md`; 27 scoped pages, 16 manifested, 11 declared non-reference, 112 sections |
| TODO-005 — wiring guard covers every mandatory docs step and proves a deletion fails | **met** | `tools/test-docs-gates.sh`: 26 mutations including a deleted-step mutation; the guard asserts a subset, not a step count |
| No regression in the existing gates | **met** | all six verify commands exit 0 and every old mutation still passes |

No check was weakened. `ci.yml` was read only; the deleted-step mutation runs against a byte copy
and the workflow's hash is unchanged before and after.

## Files changed

```text
 tools/check-docs.sh          | scanner classes, heading-fragment resolution, token-search caveat
 tools/check-reference-pages.sh | two-way manifest, manifest:line diagnostics, empty-section check
 docs/reference-pages.md      | declares the two-way contract and both required tables
 tools/test-docs-gates.sh     | 9 new mutations (26 total) and a docs-job wiring guard on every step
 docs/language/lexical-structure.md | 2 dead anchors the new check exposed, retargeted
 docs/goal-docs-v2/round-5/gate-coverage.md | this report
```

`docs/language/lexical-structure.md` is outside this task's four owned files, but it is not on the
out-of-scope list and it is owned by nobody in round 5. The new gate exposed two pre-existing dead
anchors in it (`#separators` and `statements-and-control-flow.md#conditions`); those two link
targets were retargeted to the real headings, and nothing else on the page was changed by this
round. This was an intended fix and the orchestrator approved keeping it: fixing dead anchors is
the point of RQ-015.

---

## RQ-015 — what the scanner covers

### Three scanned classes

| Class | Pages | Why |
|---|---|---|
| `docs/*` (recursive) | 57 | the current documentation tree |
| root reader pages | 3 | `README.md`, `AGENTS.md`, `CONTRACT.md` — what a reader opens first |
| agent instructions | 10 | `.github/copilot-instructions.md` and the nine `.dsh/skills/*/SKILL.md` |

Two `docs/` trees stay excluded, with the reasons in the script header:

- `docs/goal-desktop-v4/` — archived specification; its capture links are deliberately not
  resolvable in this repository.
- `docs/goal-docs-*/` — harness working memory for any goal; the wildcard is deliberate so a new
  goal directory cannot turn the gate red, and a journal is not a reader-facing page.

Every scanned page now gets the same three base checks: relative link targets resolve, heading
fragments resolve, and path-shaped inline code references name an existing file. The `--claims`
audit keeps its documented scope (all Markdown under `docs/`, including the excluded trees, because
a claim must not escape measurement); the `--readme` audit is unchanged.

### Heading fragments

A target of the form `page.md#anchor` or `#anchor` is resolved against the destination page's ATX
headings. The slug rule is GitHub's, implemented in awk:

1. strip a closing `###` sequence, inline HTML and the URL half of an inline link, then lowercase;
2. keep ASCII letters, digits, `_`, `-` and spaces; drop every other punctuation character;
3. turn spaces into hyphens;
4. suffix a repeated heading with `-1`, `-2`, … the way GitHub does.

Fenced blocks are skipped, so a heading-shaped line inside an example is not an anchor. This is an
ASCII approximation of GitHub's algorithm: a non-ASCII letter is dropped where GitHub keeps it. The
check verifies that the *anchor exists*, not that the sentence around the link is true; that is
stated in the script header.

### `--symbols` is a token search, not semantic resolution

The help text and the header now say this explicitly. The mode rejects `.rs:NNN` line-suffix
citations on `docs/` pages outside `docs/adr/`, and requires each cited `snake_case()` name to occur
as an identifier somewhere under `lovelace-*/src`. It does **not** parse Rust, resolve types or
prove that the cited call site or signature exists. Root pages and agent instructions may still
cite a source line as an operational anchor (`AGENTS.md` does); that class is audited by RQ-022,
and the old citation rule was and remains a `docs/` rule.

### Acceptance mutations

| Check | Mutation | Diagnostic (verbatim) |
|---|---|---|
| root page + same-page fragment | append `[missing section](#a-heading-that-does-not-exist)` to `README.md` | `README.md:227: broken heading anchor '#a-heading-that-does-not-exist' in README.md (no heading has that GitHub anchor)` |
| cross-file fragment | append `[absent heading](template.md#absent-heading)` to `docs/index.md` | `docs/index.md:9: broken heading anchor '#absent-heading' in docs/template.md (no heading has that GitHub anchor)` |
| root-page nonexistent path | append `[the missing guide](docs/absent-guide.md)` to `README.md` | `README.md:227: broken link 'docs/absent-guide.md' (no such file or directory: docs/absent-guide.md)` |
| agent instruction link | append `[the missing page](../../../docs/absent.md)` to `.dsh/skills/scratch/SKILL.md` | `.dsh/skills/scratch/SKILL.md:7: broken link '../../../docs/absent.md' …` |

The self-test baselines carry **resolving** anchors in all three classes, so the anchor pass is
demonstrated to run, not only to fail.

---

## RQ-016 — the manifest is now two-way

`docs/reference-pages.md` declares the contract, and `tools/check-reference-pages.sh` enforces it
on the three reference families (`docs/language/`, `docs/extensions/`, `docs/crates/`):

1. **Unlisted new page** — every Markdown page under a scoped tree must appear in `## Manifest` or
   in `## Not in scope`. A page in neither table fails with
   `page:1: page is not classified in docs/reference-pages.md …`.
2. **Listed-but-missing page** — a Manifest row whose file does not exist fails at the manifest row:
   `docs/reference-pages.md:51: listed reference page '…' does not exist`.
3. **Empty mandatory section** — a required heading with no content before the next level-2 heading
   fails at the heading: `docs/extensions/dsp.md:233: required section 'Limitations' is empty …`.
   A level-3 subsection under the heading counts as content, because it belongs to that section.
4. **Unjustified or stale exemption** — a `## Not in scope` row without a reason fails, and a row
   whose file no longer exists fails; a page in both tables fails too.

The 16 currently manifested pages keep the seven-section contract: 16 manifested, 11 declared
non-reference, 27 scoped pages, 112 sections in order and non-empty. The failure summary now names
the unclassified count separately from the manifested failures.

### Acceptance mutations

| Check | Mutation | Diagnostic (verbatim) |
|---|---|---|
| unlisted reference page | add a conformant `docs/language/gate-coverage-mutation.md` | `docs/language/gate-coverage-mutation.md:1: page is not classified in docs/reference-pages.md (add a '## Manifest' row, or a '## Not in scope' row with a reason)` |
| listed-but-missing page | add a Manifest row for `docs/language/gate-coverage-absent.md` | `docs/reference-pages.md:51: listed reference page 'docs/language/gate-coverage-absent.md' does not exist` |
| empty mandatory section | remove the body of `## Limitations` in `docs/extensions/dsp.md` | `docs/extensions/dsp.md:233: required section 'Limitations' is empty (no content before the next level-2 heading)` |
| exemption without reason | blank the reason cell of a `## Not in scope` row | `docs/reference-pages.md:11: non-reference page 'docs/language/guide.md' has no reason (say why it is not a reference page)` |

---

## TODO-005 — the wiring guard covers every mandatory docs step

`check_ci_wiring` reads the `docs` job block (not the whole workflow, so a command mentioned in
another job or a comment cannot satisfy it) and requires **each** of these commands, as a **subset**
assertion — adding a step is allowed and no exact step count is required:

```text
bash tools/check-crate-examples.sh
bash tools/run-tutorials.sh
bash docs/language/examples/run-examples.sh
bash docs/extensions/examples/run-examples.sh
bash tools/check-docs.sh --symbols
bash tools/check-docs.sh --claims
bash tools/check-docs.sh --readme
bash tools/check-reference-pages.sh
bash tools/test-docs-gates.sh
```

It also requires the `os: [ubuntu-latest, windows-latest]` matrix and rejects `|| true`,
`continue-on-error` and any `if:` in the docs job. A failure names the workflow and the docs job's
line. The guard is also invocable alone as
`bash tools/test-docs-gates.sh --check-ci FILE`, which is what makes the deleted-step mutation
possible without editing `ci.yml`.

Mutation 26 deletes the `Compile and run the crate-page examples` step from a copy of `ci.yml`,
runs the real guard against the copy, and requires a diagnostic naming the missing command. The
self-test's clean baseline runs the same guard against the real workflow first, so the failure is
attributable to the deletion.

---

## Verification — clean tree

### `bash tools/check-docs.sh --symbols`

```console
$ bash tools/check-docs.sh --symbols
check-docs: 70 Markdown files (57 docs/, 3 root, 10 agent instructions), 236 relative links, 48 heading anchors, 442 code references, 21 line-suffix citations, 22 symbol citations
check-docs: OK
EXIT=0
```

### `bash tools/check-docs.sh --claims`

```console
$ bash tools/check-docs.sh --claims
check-docs: 70 Markdown files (57 docs/, 3 root, 10 agent instructions), 236 relative links, 48 heading anchors, 442 code references
check-docs: claims audit: 90 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: OK
EXIT=0
```

### `bash tools/check-docs.sh --readme`

```console
$ bash tools/check-docs.sh --readme
check-docs: 70 Markdown files (57 docs/, 3 root, 10 agent instructions), 236 relative links, 48 heading anchors, 442 code references
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
EXIT=0
```

### default mode

```console
$ bash tools/check-docs.sh
check-docs: 70 Markdown files (57 docs/, 3 root, 10 agent instructions), 236 relative links, 48 heading anchors, 442 code references
check-docs: OK
EXIT=0
```

### `bash tools/check-reference-pages.sh`

```console
$ bash tools/check-reference-pages.sh
check-reference-pages: 27 scoped page(s): 16 manifested, 11 declared non-reference, every page classified
check-reference-pages: 112 required sections in order and non-empty
check-reference-pages: OK
EXIT=0
```

### `bash tools/test-docs-gates.sh`

```console
$ bash tools/test-docs-gates.sh
SETUP lovelace-run: /tmp/rl-target/debug/lovelace-run
SETUP scratch: /tmp/test-docs-gates.SoEL6W
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
CTRL check-docs: README anchor fixture: clean fixture passed
PASS check-docs: broken heading anchor in README.md (exit 1)
CTRL check-docs: cross-file anchor fixture: clean fixture passed
PASS check-docs: heading anchor missing in the destination page (exit 1)
CTRL check-docs: root-page path fixture: clean fixture passed
PASS check-docs: nonexistent path linked from a root page (exit 1)
CTRL check-docs: agent-instruction fixture: clean fixture passed
PASS check-docs: broken link in an agent instruction (exit 1)
CTRL check-reference-pages: conformant fixture: clean fixture passed
PASS check-reference-pages: missing required section (exit 1)
CTRL check-reference-pages: ordered fixture: clean fixture passed
PASS check-reference-pages: out-of-order section (exit 1)
CTRL check-reference-pages: classified-tree fixture: clean fixture passed
PASS check-reference-pages: unlisted new scoped-tree page (exit 1)
CTRL check-reference-pages: listed-present fixture: clean fixture passed
PASS check-reference-pages: manifested page missing on disk (exit 1)
CTRL check-reference-pages: filled-section fixture: clean fixture passed
PASS check-reference-pages: empty required section (exit 1)
CTRL check-reference-pages: reasoned-exemption fixture: clean fixture passed
PASS check-reference-pages: declared non-reference page without a reason (exit 1)
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
CTRL ci wiring: current workflow: clean fixture passed
PASS ci wiring: mandatory docs step deleted (exit 1)
WIRE ci.yml: the docs job runs the crate-example and tutorial gates, both example runners, the three check-docs modes, the reference-page gate and this mutation self-test on the Ubuntu+Windows matrix; no conditional skip

test-docs-gates: 26 mutation(s), 26 passed, 0 failed
test-docs-gates: OK (26 mutations, every gate failed for its intended reason)
SELFTEST_EXIT=0
```

`bash -n` passes for all three scripts. The self-test grew from 17 to **26** mutations: four for
the scanner classes and fragments, four for the manifest, one for the deleted CI step.

---

## Real-tree mutations, restored byte-identically

Each mutation was applied to the working tree, the gate was run, and the file was restored from a
byte copy. Every restore was verified with `sha256sum` before and after; `ci.yml` was never written.

### A — broken anchor in `README.md`

```console
TARGET=README.md
SHA256_BEFORE=cbd5a00735c4bcc0dd1942402dcd2c6349432d0408c78e9452583c6600045085
README.md:227: broken heading anchor '#a-heading-that-does-not-exist' in README.md (no heading has that GitHub anchor)
check-docs: 70 Markdown files (57 docs/, 3 root, 10 agent instructions), 236 relative links, 49 heading anchors, 442 code references
check-docs: FAIL (1 failure(s))
MUTATED_EXIT=1
SHA256_AFTER=cbd5a00735c4bcc0dd1942402dcd2c6349432d0408c78e9452583c6600045085
RESTORE=byte-identical
RESTORED_EXIT=0
check-docs: OK
```

### B — unlisted reference page

```console
TARGET=docs/language/gate-coverage-mutation.md
docs/language/gate-coverage-mutation.md:1: page is not classified in docs/reference-pages.md (add a '## Manifest' row, or a '## Not in scope' row with a reason)
check-reference-pages: 28 scoped page(s) (16 manifested, 11 declared non-reference, 1 unclassified), 1 defect(s)
check-reference-pages: FAIL
MUTATED_EXIT=1
RESTORED=file absent again
RESTORED_EXIT=0
check-reference-pages: OK
```

### C — manifested page missing from disk

```console
TARGET=docs/reference-pages.md
SHA256_BEFORE=f327b0b6685e9f70ecccafe3cc019a1b047350946cee99cc68ecf218ecfbb017
docs/reference-pages.md:51: listed reference page 'docs/language/gate-coverage-absent.md' does not exist
check-reference-pages: 27 scoped page(s) (17 manifested, 11 declared non-reference, 0 unclassified), 1 defect(s)
check-reference-pages: FAIL
MUTATED_EXIT=1
SHA256_AFTER=f327b0b6685e9f70ecccafe3cc019a1b047350946cee99cc68ecf218ecfbb017
RESTORE=byte-identical
RESTORED_EXIT=0
check-reference-pages: OK
```

### D — empty mandatory section

```console
TARGET=docs/extensions/dsp.md
SHA256_BEFORE=3d0ceea57b31b5be0731e88315306a5104ec07ca461dbd9f14b6d35416f06f62
docs/extensions/dsp.md:233: required section 'Limitations' is empty (no content before the next level-2 heading)
check-reference-pages: 27 scoped page(s) (16 manifested, 11 declared non-reference, 0 unclassified), 1 defect(s)
check-reference-pages: FAIL
MUTATED_EXIT=1
SHA256_AFTER=3d0ceea57b31b5be0731e88315306a5104ec07ca461dbd9f14b6d35416f06f62
RESTORE=byte-identical
RESTORED_EXIT=0
check-reference-pages: OK
```

### E — deleted CI step (copy only; workflow untouched)

```console
TARGET=.github/workflows/ci.yml
SHA256_BEFORE=3cf9a0db0a52e59e89d7f9e661f9b3d3cae4210438ec4ddd2d19b55d5d53f1e0
BASELINE_EXIT=0
WIRE ci.yml: the docs job runs the crate-example and tutorial gates, both example runners, the three check-docs modes, the reference-page gate and this mutation self-test on the Ubuntu+Windows matrix; no conditional skip
MUTATED_EXIT=1
FAIL ci wiring: /tmp/rq015-mutation-backup/ci-step-deleted.yml:182: the docs job does not run: bash tools/run-tutorials.sh
SHA256_AFTER=3cf9a0db0a52e59e89d7f9e661f9b3d3cae4210438ec4ddd2d19b55d5d53f1e0
CI_YML_UNCHANGED=byte-identical
```

### Final state after every restore

```console
check-docs_EXIT=0
check-docs--symbols_EXIT=0
check-reference-pages_EXIT=0
check-docs: 70 Markdown files (57 docs/, 3 root, 10 agent instructions), 236 relative links, 48 heading anchors, 442 code references
check-docs: OK
check-reference-pages: 112 required sections in order and non-empty
check-reference-pages: OK
```

---

## New check → mutation → diagnostic

| # | New check | Mutation | Diagnostic asserted |
|---|---|---|---|
| 1 | root-page same-file heading anchor | `README.md` gains `[missing section](#a-heading-that-does-not-exist)` | `README.md:227: broken heading anchor '#a-heading-that-does-not-exist' in README.md` |
| 2 | cross-file heading anchor | `docs/index.md` gains `[absent heading](template.md#absent-heading)` | `docs/index.md:N: broken heading anchor '#absent-heading' in docs/template.md` |
| 3 | root-page relative link | `README.md` gains `[the missing guide](docs/absent-guide.md)` | `README.md:N: broken link 'docs/absent-guide.md'` |
| 4 | agent-instruction link | `.dsh/skills/scratch/SKILL.md` gains a link to `../../../docs/absent.md` | `.dsh/skills/scratch/SKILL.md:N: broken link '../../../docs/absent.md'` |
| 5 | unlisted scoped page | add `docs/extensions/new-surface.md` (conformant, unlisted) | `docs/extensions/new-surface.md:1: page is not classified in docs/reference-pages.md` |
| 6 | listed-but-missing page | remove `docs/reference/thing.md` while it is manifested | `docs/reference-pages.md:N: listed reference page 'docs/reference/thing.md' does not exist` |
| 7 | empty mandatory section | `## Limitations` immediately followed by `## Examples` | `docs/reference/thing.md:17: required section 'Limitations' is empty` |
| 8 | exemption without a reason | blank the reason cell of a non-reference row | `docs/reference-pages.md:N: non-reference page '…' has no reason` |
| 9 | CI wiring step | delete `Compile and run the crate-page examples` from a copy of `ci.yml` | `<copy>:182: the docs job does not run: bash tools/check-crate-examples.sh` |

Rows 1–4 and 5–8 are the new self-test cases; row 9 is mutation 26. Rows 2–4, 5, 6 and 8 quote the
exact format the script emits (the self-test asserts the file path plus the message substring,
which is stronger than a count). Row 9 is the exact output of the deleted-step run in this report.

---

## Final state at stop

At the moment this round stopped, all six gates exited 0 from the repository root:

```console
check-docs --symbols exit=0   check-docs: OK
check-docs --claims  exit=0   check-docs: OK
check-docs --readme  exit=0   check-docs: OK
check-docs default   exit=0   check-docs: OK
reference            exit=0   check-reference-pages: OK
selftest             exit=0   test-docs-gates: 26 mutation(s), 26 passed, 0 failed
```

For honesty about concurrency: during round 5 the scanner went red once on a file this task does
not own — `.dsh/skills/debug-desktop-issue/SKILL.md:107` linked `../../docs/testing.md`, one `../`
short, introduced by the round-5 instruction agent. `check-docs` reported the broken link
(`--symbols` and `--claims` both exited 1) while `--readme`, `check-reference-pages.sh` and the
self-test stayed green. The owner fixed it to `../../../docs/testing.md` before this round stopped,
and the six-gate run above was taken afterwards. The failure was real, in another agent's file, and
the scanner behaved correctly.

---

## Limits and residuals

1. **Counts are capture-time totals.** Another round-5 agent was editing documentation while these
   runs were captured, so a later run may report a different page/link total while every gate stays
   green; the invariant under test is the exit status and the mutation verdicts, not the totals.
2. **No Windows/Git Bash execution on this host.** The checkers use only bash, awk, sed, grep, find,
   sort, mktemp and coreutils constructs already used by the previous scripts; the `docs` job runs
   them on `windows-latest`, but that hosted run is RQ-013's evidence and is not claimed here.
3. **ASCII approximation of GitHub slugs.** Non-ASCII heading letters are dropped rather than kept;
   every anchor currently linked in the scanned classes is ASCII, so nothing is skipped today. The
   limitation is stated in the script header.
4. **Symbols remain a token search.** `--symbols` does not resolve Rust semantics; the help text
   says so. `.rs:NNN` citations are still a `docs/` rule, because `AGENTS.md` deliberately cites a
   source line as an operational anchor and is audited by RQ-022.
5. **The language-runner fixture now generates its catalogue.** The round-5 C14 work extended
   `docs/language/examples/run-examples.sh` to compare `docs/language/builtins.md` with
   `lovelace-run --functions`. A stub page would make every language mutation fail for an unrelated
   reason, so the self-test generates a minimal catalogue from the binary under test. The trade-off
   is a dependency on that runner's page format; if the format moves, the fixture moves with it.
   The four language-runner mutations still test the manifest, missing-page, zero-example and
   unpaired-fence behaviours.
6. **`docs/goal-docs-v2/` is excluded from the scanner by design**, so this report's own links are
   not checked; it is harness working memory, not reader documentation.
7. **No hosted CI run was made.** All evidence above is local WSL Ubuntu at `d946316` plus the
   working tree; the docs job's Ubuntu and Windows variants remain RQ-013's evidence.

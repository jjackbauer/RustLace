# Round 7 — the last two documentation-checker gaps

**Branch:** `docs/documentation-quality-remediation`
**HEAD:** `89ea5c7` (`docs: close the documentation-review follow-up (RQ-001 to RQ-024)`)
**Recorded:** 2026-11-16T13:50Z, WSL Ubuntu, commands run from the repository root with
`PATH=/home/ricardo/.cargo/bin:…` and `CARGO_TARGET_DIR=/tmp/rl-target`
**Objective:** close RQ-015's remaining checker gap (`docs/goal-docs-v2/evidence.md` EVD-056) and
TODO-007 (`docs/goal-docs-v2/journal.md`: the retired claim vocabulary), with reproduced evidence.

**Scope guard:** only `tools/check-docs.sh`, `tools/test-docs-gates.sh`,
`docs/reference-pages.md` and this report were edited. `.github/**`, `docs/editorial.md`,
`docs/internal/**`, the other `docs/goal-docs-v2/**` records and every source file were not
touched.

**Result:** both rules exist, both were shown to fail on a real defect with `file:line`, every
existing check still passes on the clean tree, and the mutation self-test grew from 27 to 30
mutations, all PASS. `tools/docs-report.sh` still reports 4/4 gates with the `DOCREPORT` lines
intact.

## 1. RQ-015 / EVD-056 — a stale directory path the checker missed

### 1.1 The defect, reproduced

The committed page names the moved archive directory by its old location. The path-shaped code
reference rule only accepted paths ending in `.rs`, `.toml`, `.yml`, `.yaml`, `.json` or `.md`, so
a directory path with a trailing slash and no extension was invisible.

```console
$ git show HEAD:docs/reference-pages.md | sed -n 77p
`docs/symbolics/` are fixtures; and `docs/goal-desktop-v4/` and `docs/goal-docs-v1/` are archived

$ bash tools/check-docs.sh
check-docs: 71 Markdown files (58 docs/, 3 root, 10 agent instructions), 251 relative links, 49 heading anchors, 442 code references
check-docs: OK
```

Exit status 0 with the stale path in place is the gap: `docs/goal-docs-v1/` had moved to
`docs/internal/archive/goal-docs-v1/` (RQ-024) and no rule noticed.

### 1.2 The prose fix

`docs/reference-pages.md` now names the current location:

```markdown
`docs/symbolics/` are fixtures; and `docs/goal-desktop-v4/` and
`docs/internal/archive/goal-docs-v1/` are archived specification and harness working memory. None of
them documents a public callable or programmatic surface.
```

### 1.3 The new directory-path rule

`tools/check-docs.sh` rule 3 now has a directory half. After the existing trimming, command/glob
skip and line-suffix strip, a backticked reference that ends in `/` is a directory reference when

- it is **not** an `http(s)://` URL, a leading-slash absolute path or a Windows-separator path;
- it has **at least two segments** (`docs/internal/archive/`); a single-segment name (`rust/`,
  `lang/`, `round-33/`) is not treated as a repository path, because it is as often a historical
  proper name, a corpus-id prefix or a capture round named in the LovelaceSharp repository;
- the citing line does not itself declare the path absent (`exist yet`, `does not exist`,
  `do not exist`, `not part of this repository`, `no longer`, `not yet`). `docs/editorial.md:72`
  names `docs/how-to/` as a planned page shape and says in the same line that no how-to pages exist
  yet; that sentence is not a claim that the path exists.

A qualifying reference is resolved through the same `resolve_ref` roots as a file reference
(repository root, `docs/`, `docs/goal-desktop-v4/`, and the crate/src/tests roots) and fails with
`code reference '…' does not name an existing directory`. The existing file rule, link rule,
anchor rule, `--symbols` rules and the three documented tree exclusions are unchanged; the new
class only adds failures.

On the clean tree the rule checks 56 directory references: the code-reference count moves from
**442 to 498**, and the DOCREPORT field names, order and meaning are unchanged.

### 1.4 Real mutation — the committed page restored

The mutation restores the pre-fix committed page exactly (`git show HEAD:docs/reference-pages.md`),
which carries the stale path at line 77, runs the real checker, then restores the fixed file:

```console
===== MUT-DIR: the pre-fix HEAD page names the moved directory by its old path =====
mutated lines 76-79 (the committed page):
  rest are explanation or procedure; `docs/adr/` is decision history; `docs/csharp-oracle/` and
  `docs/symbolics/` are fixtures; and `docs/goal-desktop-v4/` and `docs/goal-docs-v1/` are archived
  specification and harness working memory. None of them documents a public callable or programmatic
  surface.
exit=1
  docs/reference-pages.md:77: code reference 'docs/goal-docs-v1/' does not name an existing directory
summary: check-docs: FAIL (1 failure(s))
restored: sha256=2bcc5448a07084bb99599aa5c5b46b9f1ebf823b8ecec848ec2cdd0a14f27941 identical=yes
```

The diagnostic is at exactly `docs/reference-pages.md:77`, the line EVD-056 cites, and the restore
is byte-identical.

## 2. TODO-007 — a gate for the retired claim vocabulary

### 2.1 The rule

`tools/check-docs.sh --claims` now measures the retired term on the raw prose line of every
**current page, root page and agent instruction** (the same file set as the main scan, so
`README.md`, `AGENTS.md`, `CONTRACT.md`, `.github/copilot-instructions.md` and
`.dsh/skills/**/SKILL.md` are covered). Inline code spans are included on purpose: a backticked
`precision-bounded` still teaches the term. Fenced blocks remain examples and are not scanned.

A hit reports `file:line` and the replacement:

```text
docs/reference-pages.md:102: retired term 'precision-bounded' (use 'precision-limited'; the four claim kinds are exact, precision-limited, error-bounded and parity-verified)
```

The exclusions are the documented historical ones, in code:

- `docs/internal/archive/**` and `docs/goal-docs-*/**` never reach the scan, because
  `is_current_doc` already excludes them; a fixture proves an archived page may keep the term.
- `docs/adr/` is decision history and may quote the retired wording **only as an explicit
  historical quotation**: the term must be in quotation marks (`"precision-bounded"` or
  `` `precision-bounded` ``) on a line that also marks it as history (`previously`, `historically`,
  `used to`, `was called`, `retired`, `former`, `superseded`, `originally`). An unquoted use on an
  ADR page is current prose and fails, so `docs/adr/` is not a blanket exemption.
- `docs/editorial.md` is not exempt: it must not teach the term either.

The `--claims` counter line gains `retired-term use(s)` as a human-readable count. The
`DOCREPORT` line format is unchanged.

### 2.2 The checker's own messages no longer teach the term

Before (the two messages a reader could see and copy):

```text
prohibited numerical claim '…' (label it exact, precision-bounded or parity; see docs/editorial.md)
missing '## Precision' section (every tutorial labels its values exact, precision-bounded or parity)
```

After:

```text
prohibited numerical claim '…' (label it exact, precision-limited, error-bounded or parity-verified; see docs/editorial.md)
missing '## Precision' section (every tutorial labels its values exact, precision-limited, error-bounded or parity-verified)
```

Apart from comments and the ADR quotation patterns that implement the historical exception, the
tool uses the retired string only in the matcher and in the diagnostic that names the defect and
gives the replacement. The `--help` text says "the retired claim term" without repeating it.

### 2.3 Real mutation — a current page uses the term

```console
===== MUT-TERM: a current page uses the retired claim term =====
exit=1
  docs/reference-pages.md:102: retired term 'precision-bounded' (use 'precision-limited'; the four claim kinds are exact, precision-limited, error-bounded and parity-verified)
summary: check-docs: FAIL (1 failure(s))
restored: sha256=2bcc5448a07084bb99599aa5c5b46b9f1ebf823b8ecec848ec2cdd0a14f27941 identical=yes
```

## 3. Clean-tree regression evidence

`tools/check-docs.sh` (no mode):

```console
check-docs: 71 Markdown files (58 docs/, 3 root, 10 agent instructions), 251 relative links, 49 heading anchors, 498 code references
check-docs: OK
DOCREPORT gate=check-docs status=ok pages=71 links=251 anchors=49 refs=498 citations=0 symbols=0 claims=0 tutorials=0 sections=0 readme=0 modes=none failures=0
```

`--symbols`:

```console
check-docs: 71 Markdown files (58 docs/, 3 root, 10 agent instructions), 251 relative links, 49 heading anchors, 498 code references, 21 line-suffix citations, 22 symbol citations
check-docs: OK
DOCREPORT gate=check-docs status=ok pages=71 links=251 anchors=49 refs=498 citations=21 symbols=22 claims=0 tutorials=0 sections=0 readme=0 modes=symbols failures=0
```

`--claims`:

```console
check-docs: 71 Markdown files (58 docs/, 3 root, 10 agent instructions), 251 relative links, 49 heading anchors, 498 code references
check-docs: claims audit: 95 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s), 0 retired-term use(s)
check-docs: OK
DOCREPORT gate=check-docs status=ok pages=71 links=251 anchors=49 refs=498 citations=0 symbols=0 claims=0 tutorials=5 sections=5 readme=0 modes=claims failures=0
```

`--readme`:

```console
check-docs: 71 Markdown files (58 docs/, 3 root, 10 agent instructions), 251 relative links, 49 heading anchors, 498 code references
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
DOCREPORT gate=check-docs status=ok pages=71 links=251 anchors=49 refs=498 citations=0 symbols=0 claims=0 tutorials=0 sections=0 readme=4 modes=readme failures=0
```

`bash tools/test-docs-gates.sh` — 30 mutations, 30 PASS, including the three new cases
(`backticked directory path that does not exist`, `retired claim term on a current page`,
`retired claim term unquoted in an ADR`) and the two new controls (archived records, ADR quotation):

```console
SETUP lovelace-run: /tmp/rl-target/debug/lovelace-run
SETUP scratch: /tmp/test-docs-gates.rq8LkP
CTRL check-docs base: broken-link fixture: clean fixture passed
PASS check-docs: broken relative link (exit 1)
CTRL check-docs base: missing-code-reference fixture: clean fixture passed
PASS check-docs: path-shaped code reference to a nonexistent file (exit 1)
CTRL check-docs base: missing-directory-reference fixture: clean fixture passed
PASS check-docs: backticked directory path that does not exist (exit 1)
CTRL check-docs --symbols: .rs:NNN fixture: clean fixture passed
PASS check-docs --symbols: .rs:NNN citation outside docs/adr/ (exit 1)
CTRL check-docs --symbols: live-symbol fixture: clean fixture passed
PASS check-docs --symbols: dead symbol citation (exit 1)
CTRL check-docs --claims: phrase fixture: clean fixture passed
PASS check-docs --claims: prohibited absolutist phrase (exit 1)
CTRL check-docs --claims: tutorial fixture: clean fixture passed
PASS check-docs --claims: tutorial missing '## Precision' (exit 1)
CTRL check-docs --claims: retired-term fixture: clean fixture passed
PASS check-docs --claims: retired claim term on a current page (exit 1)
CTRL check-docs --claims: archived harness records keep the retired term: clean fixture passed
CTRL check-docs --claims: ADR historical quotation: clean fixture passed
PASS check-docs --claims: retired claim term unquoted in an ADR (exit 1)
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
CTRL docs-report: complete summaries: clean fixture passed
PASS docs-report: gate exits 0 with no DOCREPORT line (exit 1)
WIRE ci.yml: the docs job runs the crate-example and tutorial gates, both example runners, the three check-docs modes, the reference-page gate, the evidence report and this mutation self-test on the Ubuntu+Windows matrix; no conditional skip

test-docs-gates: 30 mutation(s), 30 passed, 0 failed
test-docs-gates: OK (30 mutations, every gate failed for its intended reason)
```

`bash tools/docs-report.sh` — 4/4 gates, `DOCREPORT` lines unchanged in name, order and meaning:

```console
===== gate 1: bash tools/check-crate-examples.sh =====
check-crate-examples: 13 page(s), 7 rust fence(s): 7 compiled and ran, 0 excluded
check-crate-examples: OK
DOCREPORT gate=check-crate-examples status=ok pages=13 fences=7 compiled=7 excluded=0 failures=0
===== gate 2: bash tools/run-tutorials.sh =====
PASS linear-algebra: docs/tutorials/linear-algebra.md Source -> Output match
ORACLE linear-algebra: det(A) = 5; A * sol = b exactly at scale 10^1; gap = [0, 0]
PASS symbolic: docs/tutorials/symbolic.md Source -> Output match
PASS signal-processing: docs/tutorials/signal-processing.md Source -> Output match
PASS reproducible: docs/tutorials/reproducible.md fields display/kind/exact/plot match; plot artifact exists outside the repository
run-tutorials: 4 tutorials checked (linear-algebra symbolic signal-processing reproducible)
run-tutorials: 1 independent oracle(s)
run-tutorials: OK
DOCREPORT gate=run-tutorials status=ok tutorials=4 fences=8 oracles=1 failures=0
===== gate 3: bash tools/check-docs.sh --symbols --claims --readme =====
check-docs: 71 Markdown files (58 docs/, 3 root, 10 agent instructions), 251 relative links, 49 heading anchors, 498 code references, 21 line-suffix citations, 22 symbol citations
check-docs: claims audit: 95 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s), 0 retired-term use(s)
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
DOCREPORT gate=check-docs status=ok pages=71 links=251 anchors=49 refs=498 citations=21 symbols=22 claims=0 tutorials=5 sections=5 readme=4 modes=symbols,claims,readme failures=0
===== gate 4: bash tools/check-reference-pages.sh =====
check-reference-pages: 27 scoped page(s): 16 manifested, 11 declared non-reference, every page classified
check-reference-pages: 112 required sections in order and non-empty
check-reference-pages: OK
DOCREPORT gate=check-reference-pages status=ok pages=27 manifested=16 non_reference=11 unclassified=0 sections=112 failed_pages=0 failures=0
DOCREPORT total gates=4 ok=4 failed=0 commit=89ea5c7 os=Linux
```

## 4. New rule → mutation → diagnostic

| New rule | Mutation | Diagnostic (verbatim) |
|---|---|---|
| Directory path rule (`check-docs.sh` rule 3) | `git show HEAD:docs/reference-pages.md` restores the committed page with `docs/goal-docs-v1/` at line 77 | `docs/reference-pages.md:77: code reference 'docs/goal-docs-v1/' does not name an existing directory` |
| Retired-term rule (`--claims`), current page | append `The manifest is precision-bounded at that count.` | `docs/reference-pages.md:102: retired term 'precision-bounded' (use 'precision-limited'; the four claim kinds are exact, precision-limited, error-bounded and parity-verified)` |
| Retired-term rule, ADR is not a blanket exemption | self-test: append `A precision-bounded result is acceptable here.` to `docs/adr/999-scratch.md` | `PASS check-docs --claims: retired claim term unquoted in an ADR (exit 1)` |
| Directory path rule, scratch fixture | self-test: append `` `docs/goal-docs-v1/` `` to the fixture's `docs/index.md` | `PASS check-docs: backticked directory path that does not exist (exit 1)` |

The new fixture controls prove the exemptions rather than assume them:

| Control | Fixture | Result |
|---|---|---|
| Archived harness records keep the term | `docs/internal/archive/goal-docs-v1/journal.md` and `docs/goal-docs-v2/journal.md` containing `precision-bounded` | `CTRL check-docs --claims: archived harness records keep the retired term: clean fixture passed` |
| ADR historical quotation | `The earlier draft previously said "precision-bounded" for this kind of value.` | `CTRL check-docs --claims: ADR historical quotation: clean fixture passed` |
| Directory rule has a positive case | fixture's `docs/index.md` says the harness lives under `docs/internal/archive/` | `CTRL check-docs base: missing-directory-reference fixture: clean fixture passed` |

Mutation count: **27 → 30** (three new mutations: directory path, retired term on a current page,
retired term unquoted in an ADR). Three `CTRL` baselines were added (the directory mutation's clean
fixture, the archived-record fixture and the ADR-quotation fixture); none is counted as a mutation.

The transcripts above were recorded before this report existed. The report is itself a Markdown
page under `docs/`, so the next `--claims` run measures **96** pages instead of 95; the counts that
matter stay 0. The final `tools/docs-report.sh` run after this file was written reports
`DOCREPORT gate=check-docs … claims=0 … failures=0` (96 pages) and
`DOCREPORT total gates=4 ok=4 failed=0`.

## 5. Integrity checks

**Byte-identical restore.** The two real mutations were applied to the working tree and restored
from a `cp` backup; `cmp`-equivalent SHA-256 equality was asserted by the mutation script:

```text
final: tools/check-docs.sh      983d51f44e58f6cc56424da8e90c7705983718170c2871211e8d9174c063b113
final: tools/test-docs-gates.sh 68a458092e574e2a63a69333aecba61113c8770514e2f9786a13f1b19f1a70e1
final: docs/reference-pages.md  2bcc5448a07084bb99599aa5c5b46b9f1ebf823b8ecec848ec2cdd0a14f27941
after: docs/reference-pages.md  2bcc5448a07084bb99599aa5c5b46b9f1ebf823b8ecec848ec2cdd0a14f27941  identical=yes
```

(The checker and self-test are not touched by the mutation script; the hashes above are the final
working-tree hashes for all three files. The mutated page was restored and compared to the hash
taken before the mutation.)

**Exit codes and DOCREPORT.** `check-docs.sh` keeps its three statuses: 0 clean, 1 with at least one
failure, 2 bad usage or a missing tree. The final `DOCREPORT gate=check-docs …` field list and order
are byte-for-byte the same as before the change; only the `refs=` value grew from 442 to 498
because directory references are now counted, and the human `--claims` summary gained the
`retired-term use(s)` count. No `DOCREPORT` consumer needs a change.

**No weakening of existing checks.** The file-reference rule, link rule, anchor rule, `--symbols`
rules, prohibited-phrase rule, tutorial `## Precision` rule and README rule are unchanged; the
`--claims` messages were corrected from the retired three-way vocabulary to the four current kinds.
The three documented tree exclusions are untouched. `.github/**` is untouched and the self-test's
CI-wiring guard still passes.

## 6. Residual and known limits

- The directory rule covers two-segment-or-more trailing-slash references. A single-segment name
  (`rust/`, `lang/`, `round-33/`, `tests/`) is deliberately out of scope: the same shape is used
  for historical proper names, corpus-id prefixes and crate-relative roots, so checking it would
  produce false failures on true historical prose. `docs/` and `src/` are therefore not gated by
  this rule.
- A directory path is exempt only when its own line declares it absent. This is what keeps
  `docs/editorial.md:72`'s planned `docs/how-to/` green without editing that out-of-scope page; if
  that sentence stops saying "no how-to pages exist yet", the gate fails until the directory
  exists or the prose changes. That is the intended trade: a forward-looking path is not a stale
  path, but it has to say so on the line.
- The ADR exemption is narrow by design: quotation marks **and** a historical marker on the same
  line. A future ADR that needs to quote the term without a marker will have to add one; that is
  the price of not exempting the whole `docs/adr/` tree.
- The retired-term rule scans raw lines, so a backticked mention counts as a use on a current page.
  Historical quotations belong in `docs/adr/` or an archived/goal record, not in current prose.

# Round 9 — claim-level audit (D3) and README structure assertion (D6)

Date: 2026-11-15
Environment: WSL Ubuntu, `/mnt/c/Users/ricar/dev/RustLace`
Owner slice: `tools/check-docs.sh`, `README.md`, `docs/tutorials/README.md`
Evidence: measured checker runs, six falsifications, byte-identical restores

## Outcome

| Dimension | Command | Before | After |
|---|---|---|---|
| D3 claim audit | `bash tools/check-docs.sh --claims` | one real defect: `docs/tutorials/README.md` had no `## Precision` section; zero prohibited prose claims | exit 0; `57 Markdown files, 158 relative links, 233 code references`; `claims audit: 75 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)` |
| D6 README | `bash tools/check-docs.sh --readme` | one real defect: `README.md` used `### Current limitations` | exit 0; `README.md structural audit: 4 requirements checked` |
| existing gate | `bash tools/check-docs.sh --symbols` | exit 0 | exit 0; 20 line-suffix citations, 19 symbol citations |

No `.rs` file, `.github/` file, `docs/reference-*` page, `docs/crates/` page, or
`docs/extensions/` page was edited. The only documentation page changed to make the claim audit
pass is `docs/tutorials/README.md`.

## What `--claims` measures

The phrase half scans every Markdown page under `docs/`, including `docs/goal-docs-v1/` and
`docs/goal-desktop-v4/`, not only the current-documentation subset. Fenced blocks and inline code
spans are removed before matching: an example, a quoted command, or a phrase inside backticks is a
mention, not a claim. The audit then fails on these three prose phrases:

| Prohibited phrase | Allowed prose context |
|---|---|
| `exact 30-place` | `docs/editorial.md`, `docs/adr/004-thirty-digit-default.md`, `docs/extensions/dsp.md` |
| `exactly 30 places` | same three pages |
| `nothing is floating point` | same three pages |

Those three pages are the only file-level exceptions: the editorial guide forbids the wording, the
ADR cites it as the wording to replace, and the DSP page quotes it only to correct it. A phrase used
as prose in any other page is a failure, reported as `file:line: message`, and exits 1.

The structure half requires every `*.md` under `docs/tutorials/` to contain a real level-2
`## Precision` heading outside a fenced block. A heading in a code sample does not satisfy it.

## What `--readme` asserts

1. **Value proposition.** A prose line among the first ten must contain a capability term
   (`arithmetic`, `mathemat`, `array`, `symbolic`, `precision`, `scripting`, `computing`,
   `differentiat`, `polynomial` or `signal`), must not be a heading or table row, must have at least
   five words, and must not contain a lineage term (`LovelaceSharp`, `successor`, `descends`,
   `descended`, `port of` or `C#`). This is a measurable proxy for "a sentence naming what the
   software does, not its lineage"; it cannot prove the sentence is good prose.
2. **Limitations.** A real level-2 `## Current limitations` heading outside a fenced block.
3. **Runnable command.** At least one fenced block containing `lovelace-run`.
4. **Order.** The value-proposition line, the limitations heading and the command line must all
   precede the first real level-2 `## The crates` heading.

Failures are printed as `README.md: missing <thing>` and exit 1.

## Before the fixes

The first run after adding both flags, before editing `README.md` or
`docs/tutorials/README.md`. stdout:

```text
check-docs: 55 Markdown files, 147 relative links, 199 code references
check-docs: claims audit: 55 pages scanned, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: README.md structural audit: 4 requirements checked
check-docs: FAIL (2 failure(s))
```

stderr:

```text
docs/tutorials/README.md:1: missing '## Precision' section (every tutorial labels its values exact, precision-bounded or parity)
README.md: missing '## Current limitations' section
```

Exit status: 1.

These are the two real defects the new checks found. The counts are from the first version of the
claims scan, which counted only the current-documentation subset (55). The delivered scan covers
every page under `docs/`; the six-mode capture below reports 75 pages, and a concurrent round-8
report added later raises the live count to 76 without changing the result. A
concurrent agent added `docs/reference-pages.md`, `docs/reference-template.md` and
`tools/check-reference-pages.sh` during the round, which is why the current-documentation file count
rose from 55 to 57. The failure set did not change when the phrase scan was widened.

## The fixes

1. `README.md:33`: `### Current limitations` became `## Current limitations`. One `#` was removed;
   the file is 13,490 bytes (was 13,491) and the CRLF line endings were preserved.
2. `docs/tutorials/README.md`: a `## Precision` section was inserted before the paragraph that
   describes the seven tutorial sections and the **exact** / **precision-bounded** / **parity**
   labels. The file is 25 lines (was 23).

No other page needed a prose correction. The only prose occurrences of the three prohibited
phrases are in the three allowed pages; the occurrences in `docs/goal-docs-v1/` are inside quoted
grep commands and inline code, so they are mentions, not claims, and the audit does not flag them.
No exemption was added to hide a real violation.

## Passing runs after the fixes

Captured on the green tree with this report present:

```text
===================================================================
$ bash tools/check-docs.sh 
check-docs: 57 Markdown files, 158 relative links, 233 code references
check-docs: OK
exit=0

===================================================================
$ bash tools/check-docs.sh --symbols
check-docs: 57 Markdown files, 158 relative links, 233 code references, 20 line-suffix citations, 19 symbol citations
check-docs: OK
exit=0

===================================================================
$ bash tools/check-docs.sh --claims
check-docs: 57 Markdown files, 158 relative links, 233 code references
check-docs: claims audit: 75 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: OK
exit=0

===================================================================
$ bash tools/check-docs.sh --readme
check-docs: 57 Markdown files, 158 relative links, 233 code references
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
exit=0

===================================================================
$ bash tools/check-docs.sh --claims --readme
check-docs: 57 Markdown files, 158 relative links, 233 code references
check-docs: claims audit: 75 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
exit=0

===================================================================
$ bash tools/check-docs.sh --symbols --claims --readme
check-docs: 57 Markdown files, 158 relative links, 233 code references, 20 line-suffix citations, 19 symbol citations
check-docs: claims audit: 75 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
exit=0

```

After that capture, a concurrent agent added `docs/goal-docs-v1/round-8/reference-pages.md`. The
maximal gate on the now-current tree still passes:

```text
$ bash tools/check-docs.sh --symbols --claims --readme
check-docs: 57 Markdown files, 158 relative links, 233 code references, 20 line-suffix citations, 19 symbol citations
check-docs: claims audit: 76 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
exit=0
```

## Falsification

An ephemeral harness under `.tmp/` (ignored, deleted after capture) applied each mutation, ran the
checker, restored the file from a `cp` byte copy, compared SHA-256, and ran the passing check again.
Six mutations cover every condition:

- claims: all three prohibited phrases used as prose in `docs/glossary.md`;
- claims: a tutorial page loses its `## Precision` heading;
- README: the value proposition is replaced by a lineage sentence;
- README: `## Current limitations` becomes `## Current limitation`;
- README: no fenced block contains `lovelace-run`;
- README: `## The crates` is moved above the required material.

Verbatim harness output:

```text

===================================================================
FALSIFICATION: claims: prohibited phrase in a page that makes the claim
===================================================================
baseline docs/glossary.md: sha256 debcf91c817d37913d3cc7c9d71f61f8e3af5edba2a5d632e6b33ced53327b7a
$ bash tools/check-docs.sh --claims
docs/glossary.md:31: prohibited numerical claim 'exact 30-place' (label it exact, precision-bounded or parity; see docs/editorial.md)
docs/glossary.md:32: prohibited numerical claim 'exactly 30 places' (label it exact, precision-bounded or parity; see docs/editorial.md)
docs/glossary.md:33: prohibited numerical claim 'nothing is floating point' (label it exact, precision-bounded or parity; see docs/editorial.md)
check-docs: 57 Markdown files, 158 relative links, 233 code references
check-docs: claims audit: 74 pages under docs/, 5 tutorial pages, 3 prohibited phrase(s)
check-docs: FAIL (3 failure(s))
exit=1
restored docs/glossary.md: sha256 debcf91c817d37913d3cc7c9d71f61f8e3af5edba2a5d632e6b33ced53327b7a (byte-identical: yes)
$ bash tools/check-docs.sh --claims --readme
check-docs: 57 Markdown files, 158 relative links, 233 code references
check-docs: claims audit: 74 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
exit=0

===================================================================
FALSIFICATION: claims: tutorial page without a ## Precision section
===================================================================
baseline docs/tutorials/symbolic.md: sha256 5e06903f6257667753941e9c9f52e0f8763800926462fee54642e197c1a6b95f
$ bash tools/check-docs.sh --claims
docs/tutorials/symbolic.md:1: missing '## Precision' section (every tutorial labels its values exact, precision-bounded or parity)
check-docs: 57 Markdown files, 158 relative links, 233 code references
check-docs: claims audit: 74 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: FAIL (1 failure(s))
exit=1
restored docs/tutorials/symbolic.md: sha256 5e06903f6257667753941e9c9f52e0f8763800926462fee54642e197c1a6b95f (byte-identical: yes)
$ bash tools/check-docs.sh --claims --readme
check-docs: 57 Markdown files, 158 relative links, 233 code references
check-docs: claims audit: 74 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
exit=0

===================================================================
FALSIFICATION: readme: value proposition replaced by lineage
===================================================================
baseline README.md: sha256 2100598e653907b3456935fc338de1097b32f1133819c774f6f2f9ccb5a28817
$ bash tools/check-docs.sh --readme
README.md: missing value proposition in the first 10 lines (a sentence naming what the software does, not its lineage)
check-docs: 57 Markdown files, 158 relative links, 233 code references
check-docs: README.md structural audit: 4 requirements checked
check-docs: FAIL (1 failure(s))
exit=1
restored README.md: sha256 2100598e653907b3456935fc338de1097b32f1133819c774f6f2f9ccb5a28817 (byte-identical: yes)
$ bash tools/check-docs.sh --claims --readme
check-docs: 57 Markdown files, 158 relative links, 233 code references
check-docs: claims audit: 74 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
exit=0

===================================================================
FALSIFICATION: readme: missing ## Current limitations heading
===================================================================
baseline README.md: sha256 2100598e653907b3456935fc338de1097b32f1133819c774f6f2f9ccb5a28817
$ bash tools/check-docs.sh --readme
README.md: missing '## Current limitations' section
check-docs: 57 Markdown files, 158 relative links, 233 code references
check-docs: README.md structural audit: 4 requirements checked
check-docs: FAIL (1 failure(s))
exit=1
restored README.md: sha256 2100598e653907b3456935fc338de1097b32f1133819c774f6f2f9ccb5a28817 (byte-identical: yes)
$ bash tools/check-docs.sh --claims --readme
check-docs: 57 Markdown files, 158 relative links, 233 code references
check-docs: claims audit: 74 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
exit=0

===================================================================
FALSIFICATION: readme: no fenced block containing lovelace-run
===================================================================
baseline README.md: sha256 2100598e653907b3456935fc338de1097b32f1133819c774f6f2f9ccb5a28817
$ bash tools/check-docs.sh --readme
README.md: missing a fenced block containing lovelace-run
check-docs: 57 Markdown files, 158 relative links, 233 code references
check-docs: README.md structural audit: 4 requirements checked
check-docs: FAIL (1 failure(s))
exit=1
restored README.md: sha256 2100598e653907b3456935fc338de1097b32f1133819c774f6f2f9ccb5a28817 (byte-identical: yes)
$ bash tools/check-docs.sh --claims --readme
check-docs: 57 Markdown files, 158 relative links, 233 code references
check-docs: claims audit: 74 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
exit=0

===================================================================
FALSIFICATION: readme: crates section before the value proposition, limitations and command
===================================================================
baseline README.md: sha256 2100598e653907b3456935fc338de1097b32f1133819c774f6f2f9ccb5a28817
$ bash tools/check-docs.sh --readme
README.md: missing required order: '## Current limitations' must appear before '## The crates'
README.md: missing required order: the lovelace-run command must appear before '## The crates'
check-docs: 57 Markdown files, 158 relative links, 233 code references
check-docs: README.md structural audit: 4 requirements checked
check-docs: FAIL (2 failure(s))
exit=1
restored README.md: sha256 2100598e653907b3456935fc338de1097b32f1133819c774f6f2f9ccb5a28817 (byte-identical: yes)
$ bash tools/check-docs.sh --claims --readme
check-docs: 57 Markdown files, 158 relative links, 233 code references
check-docs: claims audit: 74 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
exit=0

FALSIFICATION HARNESS COMPLETE: failed=0
```

Every restore line reports `byte-identical: yes`, and the harness exits 0. The checker is therefore
falsifiable in both directions: it fails on each broken condition and passes again after the exact
bytes are restored. The three mutated files (`README.md`, `docs/glossary.md` and
`docs/tutorials/symbolic.md`) match their baseline SHA-256 values after restoration, so the
falsification left no content residue. `docs/glossary.md` was already modified before this round
under the repository's CRLF condition; this round did not change its bytes.

## Change set and `git diff --stat`

```text
$ git status --porcelain -- README.md tools/check-docs.sh docs/tutorials/README.md docs/goal-docs-v1/round-9
 M README.md
?? docs/goal-docs-v1/round-9/
?? docs/tutorials/README.md
?? tools/check-docs.sh
```

`tools/check-docs.sh` and `docs/tutorials/` are untracked in this baseline, so `git diff` cannot show
their contents; `git status` does. The script is 512 lines (300 before this round); the tutorials
index is 25 lines (23 before).

```text
$ git diff --stat -- README.md
 README.md | 399 ++++++++++++++++++++++++++++++++++----------------------------
 1 file changed, 216 insertions(+), 183 deletions(-)

$ git diff --stat --ignore-cr-at-eol -- README.md
 README.md | 51 ++++++++++++++++++++++++++++++++++++++++++---------
 1 file changed, 42 insertions(+), 9 deletions(-)
```

The raw README diff is line-ending noise: the working tree is CRLF-converted relative to the LF
index (pre-existing, recorded in round 7), so every line reads as changed. The `--ignore-cr-at-eol`
stat is still a diff against `HEAD` and therefore includes the earlier README rewrite; this round's
actual README edit is the single-character heading promotion described above.

The unscoped `git diff --stat` reports `466 files changed, 295959 insertions(+), 294607 deletions(-)`
at capture. That is the whole dirty tree under the same CRLF condition plus concurrent agents'
edits, not this round's change set; the scoped status and stat above are the reviewable diff.

## Residual and limits

- The value-proposition rule is a vocabulary-and-structure check. It fails a lineage-only opening and
  requires a capability term, but it cannot judge whether the resulting sentence is accurate or
  well written. That limit is the same one `docs/editorial.md` records: a checker can prove a shape,
  not a claim.
- The claims rule treats inline code as a mention. A claim deliberately written inside backticks
  would not be flagged. This is the documented claim-versus-mention boundary in the script header;
  the three allowed pages remain the only prose exceptions.
- The green transcripts were captured after the concurrent reference-page work settled. A transient
  broken link in `docs/language/builtins.md:428` (another agent's file) made intermediate runs red;
  it was fixed by its owner before the final capture, and this round did not edit that file.
- The scratch harness and its backups were under `.tmp/` (ignored) and were removed after capture.

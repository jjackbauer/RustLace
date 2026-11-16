---
name: sync-docs
description: Keep the dependent documentation in step with a builtin, behaviour, platform or wire-envelope change by following the dependent-document map, re-running the affected documented example for semantic drift, and verifying the links, corpus records and golden fixtures.
whenToUse: Use when a code change alters what a document says, when a documentation edit must be proved by re-running its example rather than by the diff, or when you are asked to update docs after an API, error-text, platform or wire-envelope change.
---

# Synchronise the documentation

A documentation change is not done when one Markdown file says the new thing. It is done when every
dependent document says it, the example those documents show actually produces that output on this
tree, and the checks that pin the contract pass. Editing the sentence is the easy half; the failure
this skill exists for is a page that was edited but now documents a value the product no longer
produces.

## Dependent-document map

| When this changes | Update these documents | The check that proves it |
|---|---|---|
| The builtin registry: a descriptor, name, arity, help text or category | the builtin reference under `docs/language/` (`docs/language/builtins.md`), and any help, completion or functions list that renders the entry | run the documented example with `lovelace-run`; the builtin's tests and the corpus record still pass |
| A behaviour, its output, or an error sentence or code | the ADR under `docs/adr/` that records the decision, and the record in `parity/corpus/` | `cargo run --release -p lovelace-corpus`; the record quotes the new sentence exactly |
| A platform assumption, support claim, or build/release step | `docs/goal-desktop-v4/platform-matrix.md`, the relevant `docs/adr/` record, and `.dsh/skills/release-and-publish/SKILL.md` | `cargo test -p lovelace-desktop platform_claims`; the matrix keeps its one-supported-row shape |
| The wire envelope: a field, its order, its type or its error shape | `docs/crates/lovelace-run.md` and the golden fixtures under `docs/csharp-oracle/Lovelace.Run.Tests/fixtures/` | `cargo test -p lovelace-run golden_batch` |

A document can appear in more than one row. When in doubt, search the current pages for the old name
or sentence (`Select-String -Path docs -Pattern '<old text>' -Recurse`) and classify every hit before
editing: a deliberate historical mention is not drift.

## Procedure

1. **Name the change and read its row.** Decide whether the change is a builtin, a behaviour or
   error text, a platform assumption, or the wire envelope. That row is the minimum set of documents
   to update; it is not the maximum. `docs/editorial.md` rule 3 already says a machine-read document
   such as `platform-matrix.md` is a code change, not prose.

2. **Update the owning page and every dependent page in the same change.** The builtin reference
   without the corpus record, or the ADR without the release skill, is a half-change. If behaviour
   changed, the agent instructions that describe it change too: `AGENTS.md`, the Copilot instructions
   and `.dsh/skills/**` are operational inputs, not commentary.

3. **Check semantic drift, not that a Markdown file was edited.** Re-run the affected documented
   example with the real binary and compare the actual typed line, error sentence or envelope field
   with the text on the page. A page that appears in `git diff` is not a page that is true, and a
   link check that passes only proves the link resolves. If the run output differs from the
   documented output, the page is stale even when its prose was just rewritten.

   ```console
   cargo run -p lovelace-run -- --eval "<the exact source the page shows>" --text
   ```

   Compare the output character for character with the page's block; then run the page's own
   examples through `bash docs/language/examples/run-examples.sh` (or the extensions runner) so every
   example on the page, not only the one you remembered, is exercised.

4. **Re-pin the fixture when the contract moved.** A moved error sentence needs its corpus record
   updated, and a moved wire field needs its golden envelope regenerated. Regenerate only through
   the documented switch, never by hand-editing the fixture:

   ```console
   LOVELACE_RECORD_GOLDENS=1 cargo test -p lovelace-run golden_batch
   ```

   Then re-run the same test without the variable and confirm it passes against the new fixture.

5. **Apply the editorial rules while the page is open.** Every numerical claim says whether it is
   **exact**, **precision-bounded** or **parity**, and every example uses a repository-relative path
   or a placeholder instead of a path from one machine (`docs/editorial.md`). Automated wording and
   link checks can verify consistency; they cannot establish mathematical correctness, so a number
   certified only by a consistency check is not certified.

6. **Record the evidence in the change.** State the command, the observed output and the documents
   updated. "Docs updated" is not evidence; "the page's `sqrt(2)` example prints `1.414...` at 30
   places and `cargo run --release -p lovelace-corpus` prints 214 passed" is.

## Verify

```console
# the documented example, executed rather than read
cargo run -p lovelace-run -- --eval "<the documented source>" --text

# every documented language and extension example
bash docs/language/examples/run-examples.sh
bash docs/extensions/examples/run-examples.sh

# the behaviour/error-text records the page names
cargo run --release -p lovelace-corpus

# the C# documentation oracle and the wire golden envelopes
cargo test -p lovelace-lang docssync
cargo test -p lovelace-run golden_batch
```

Observable success: every command exits 0; the runners report their full example counts with no
drift; the corpus reports `214 passed, 0 failed, 214 total`; `docs_sync` and `golden_batch` pass; and
the output of the documented example, pasted beside the page, is identical to the block the page
shows. A page that was edited while its example still prints the old output has not been synced.

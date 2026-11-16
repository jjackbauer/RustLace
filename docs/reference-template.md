# Reference page template

This is the shape contract for every page listed in [the manifest](reference-pages.md). It applies
to a page that documents a callable or programmatic surface: a family of builtins, a library crate,
or a command-line binary. A page conforms when its top-level sections are these seven, at heading
level 2, with these exact names, in this order. Everything else on the page is a level-3 (or deeper)
subsection of the section it belongs to. The contract is enforced by
[`tools/check-reference-pages.sh`](../tools/check-reference-pages.sh), which the `docs` job in
[`.github/workflows/ci.yml`](../.github/workflows/ci.yml) runs, so a page that drifts is a failing
CI run rather than a review comment.

## The seven sections

| Order | Section | What belongs there |
|---:|---|---|
| 1 | **Purpose** | What the surface is for and who calls it: the problem it solves, the place it holds in the workspace, and the shape of the thing a reader gets — one paragraph a reader can act on. |
| 2 | **Inputs** | What the caller supplies: the call shape, argument counts and kinds in order, the state or precondition a call needs, and — for a crate — the public surface a caller programs against and the dependencies it is built from. |
| 3 | **Outputs** | What a call gives back: the value kind and shape, record fields, the display and typed form, and any side effect, file, stream or exit status the surface produces. |
| 4 | **Precision behaviour** | For every numeric result, whether it is exact, precision-limited, error-bounded or parity-verified, where the budget comes from, its default and its cap, and what happens when a request exceeds the budget. |
| 5 | **Errors** | Every refusal with the code and category it carries (or the Rust error type it returns), the condition that raises it, and the outcomes that look like errors but are documented results instead. |
| 6 | **Limitations** | What the surface refuses or does not do: unsupported inputs and domains, missing families, documented defects and parity-verified artifacts, and the work a caller must do instead. Every entry is checked against the code, its tests or `docs/rustlace-notes.md`; an invented limitation is worse than none. |
| 7 | **Examples** | At least one call a reader can run, with the exact result the implementation produced, in the repository's example format; a page whose examples are executed by a runner keeps each fenced pair intact. |

## Rules

1. **All seven, always.** The seven headings are present in the order above. A section is never
   omitted because the answer is short: a page with nothing to report says so explicitly, with
   `None known.` under Limitations or `None.` under a section whose answer is genuinely empty. An
   empty heading, or a missing one, fails the gate; "None" is a documented answer, not an omission.
2. **One top-level layer.** No other level-2 heading appears on the page. An existing heading that
   carries content is nested under the required section it belongs to (its level goes up by one),
   never deleted, and its anchor text is kept.
3. **Move, do not rewrite.** A page brought into this shape keeps every existing sentence, table
   and example: content is moved under the right heading, and only the heading level changes.
   New prose is added only where a required section has no source material, and then it is derived
   from the implementation, its tests or `docs/rustlace-notes.md`.
4. **Example pairs are a unit.** A fenced `lovelace` block and the `text` block that follows it are
   one example. They stay adjacent and byte-identical, because
   `docs/language/examples/run-examples.sh` and `docs/extensions/examples/run-examples.sh` execute
   them and compare the observed output.
5. **Claims are labelled.** Every numeric statement says whether it is exact, precision-limited,
   error-bounded or parity-verified, per
   [the editorial guide](editorial.md#the-four-kinds-of-claim). A documented defect is
   presented as a defect, not as the expected result.
6. **Limits are evidence-backed.** A limitation is written only after checking what the code
   refuses or does not do. `None known` is a claim that the check was made, not a placeholder.
7. **The manifest is the gate's input.** Being listed in [the manifest](reference-pages.md) is what
   puts a page under this contract; the checker reads that table and nothing else. A new page in
   `docs/language/`, `docs/extensions/` or `docs/crates/` that documents a callable surface is
   added to the manifest in the same change that adds the page.

## Skeleton

```markdown
# `surface-name`

<existing one-line description>

## Purpose

<what it is for, who calls it>  (`### <existing sub-section>`, if the page has one)

## Inputs

<call shape, arguments, preconditions>  (`### Public surface`, `### Dependencies`, ...)

## Outputs

<value kinds, record fields, display form, side effects>

## Precision behaviour

<exact / precision-limited / error-bounded / parity-verified, the budget and its cap>

## Errors

<refusal, code, category, and the non-errors>

## Limitations

<what it refuses or does not do; `None known.` only after checking>

## Examples

<runnable calls with their observed results>
```

The heading names are literal: the gate compares the text after `## `, so `Precision behaviour`
with a lowercase `b` is the spelling, and `&` or a trailing colon is not.

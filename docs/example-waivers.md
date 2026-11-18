# Documentation example waiver policy

Executable Lovelace snippets are part of the public contract. Adding
`<!-- not-run: ... -->` without authorization can hide a broken example while
other examples continue passing. The documentation job rejects unregistered
exemptions using `python tools/check-doc-metadata.py`.

When an example genuinely cannot execute in the available test environment,
a contributor must add a record to [example-waivers.json](example-waivers.json)
with all of: `id` (stable `WAIVER-...` identifier), `path` (repository-relative
Markdown file), `sha256` (SHA-256 of the exact fenced Lovelace source with
normalized LF line endings), `justification`, `approved_by`, and `expires`
(ISO calendar date). The corresponding annotation must read:

```markdown
<!-- not-run: WAIVER-123 a concise explanation -->
```

The annotation must be on the line immediately before its `lovelace` fence.
The reason may include comparison symbols such as `<` or `>` and text may
follow the closing `-->`; the authorization checker and executable-example
runner both recognize these forms. The required `WAIVER-...` identifier
must still name an approved, unexpired inventory entry pinned to the exact
script hash.

The checker rejects missing, expired, unreferenced, duplicate, or mismatched
waivers. Changing the example invalidates its hash. The waiver inventory, this policy page and
the checker are routed through CODEOWNERS, but an additional independent reviewer
and branch-protection configuration are required to enforce separation of
duties; the checker alone cannot prove that `approved_by` is an authorized person.

Waivers must never be used to bless a mathematically wrong output, silence a
regression that a test can reproduce, or evade an updated API contract. Prefer
an executable minimal example or a plain non-executable pseudocode fence.

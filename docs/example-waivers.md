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

The checker rejects missing, expired, unreferenced, duplicate, or mismatched
waivers. Changing the example invalidates its hash. The waiver inventory and
the checker are routed through CODEOWNERS, but an additional independent reviewer
and branch-protection configuration are required to enforce separation of
duties; the checker alone cannot prove that `approved_by` is an authorized person.

Waivers must never be used to bless a mathematically wrong output, silence a
regression that a test can reproduce, or evade an updated API contract. Prefer
an executable minimal example or a plain non-executable pseudocode fence.

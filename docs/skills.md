# Skills

Eight task skills for extending RustLace without guessing. Each skill is a numbered procedure with
the real files to touch and a `## Verify` section whose commands produce an observable result. The
machine-readable skills live under `.dsh/skills/`; the portable contributor guide is
[AGENTS.md](../AGENTS.md). Start with [docs/extending.md](extending.md) for the prose walkthroughs.

| Skill | What it does | Load it when |
|---|---|---|
| [add-builtin](../.dsh/skills/add-builtin/SKILL.md) | Adds a language builtin end to end: registry descriptor, implementation, arity, docs, test and parity fixture. | You are adding or changing a callable builtin, or a function must appear in the registry, help, completion or wire functions array. |
| [add-modus-plugin](../.dsh/skills/add-modus-plugin/SKILL.md) | Adds a Modus plugin with registration, capability descriptor, tests and a clear statement of CLI and desktop visibility. | You are adding an opt-in extension package, or a plugin builtin is missing from `--functions` or the desktop. |
| [add-ui-pane](../.dsh/skills/add-ui-pane/SKILL.md) | Adds or changes a workbench pane across all layout modes with theme tokens, an empty state, keyboard focus, accessibility and captured evidence. | You are adding a pane or visible region to the desktop, or changing its layout, title, focus order or colours. |
| [add-language-feature](../.dsh/skills/add-language-feature/SKILL.md) | Changes the active lexer, parser or interpreter with focused tests and the corpus and golden fixtures that pin the contract. | You are adding syntax, a token, an expression or statement form, or an evaluation rule. |
| [add-acceptance-mode](../.dsh/skills/add-acceptance-mode/SKILL.md) | Adds a headless acceptance mode or journey that drives the Workbench path and produces a reproducible artifact. | A desktop behaviour needs an automated check, a new journey row, or a build-host command. |
| [change-numeric-kernel](../.dsh/skills/change-numeric-kernel/SKILL.md) | Changes Real or Nat arithmetic while preserving the precision contract, periodic decimals and canonical limb invariants, with regression, corpus and benchmark evidence. | You are changing arithmetic, division, rounding, periodic handling, square root, pi or the NTT/division thresholds. |
| [debug-desktop-issue](../.dsh/skills/debug-desktop-issue/SKILL.md) | Reproduces a desktop defect, captures UIA and PrintWindow evidence, fixes the owning layer and proves it with a failing-first regression test. | A desktop symptom is reported, a pane paints the wrong state, or a control is unreachable or clipped. |
| [release-and-publish](../.dsh/skills/release-and-publish/SKILL.md) | Builds, tests, optionally signs with the self-signed caveat, tags and publishes the Windows artifact, and checks the CI run behind it. | You are cutting a release, bumping the workspace version, publishing the executable or checking CI before a tag. |

## Format

Every skill file begins with the DeepSeek frontmatter shape:

```yaml
---
name: <kebab-case name>
description: <one sentence>
whenToUse: <when a contributor or agent should load this skill>
---
```

The `name` must equal the directory name. The body is a numbered procedure; the final section is
`## Verify` with exact commands and the observable that proves success. The cited paths are checked
against the repository, and the links above are checked relative to this page.

Adding a skill: create `.dsh/skills/<name>/SKILL.md`, add the row here and in the pointer table in
[AGENTS.md](../AGENTS.md), and keep the frontmatter `name` equal to `<name>`.

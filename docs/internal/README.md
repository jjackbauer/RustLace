# Internal working material

This directory holds material that is kept in the repository as evidence but is
not part of the published documentation. It is deliberately excluded from the
link checker's current-page class: nothing here is written for a reader who
arrived from the README, and what look like links were historical pointers, not
navigation.

## What the archive holds

`docs/internal/archive/goal-docs-v1/` is the complete working record of the
first documentation-goal cycle: the goal statement, the intent audit, the
evidence ledger, the journal, the per-round reports under `round-*/`, and the
closing state file. It is 16 files and 287,349 bytes, moved here from the public
`docs/` tree with `git mv` (RQ-024), so both the file contents and their git
history are unchanged. Do not edit an archived file: it records what was
believed at the time, including claims that were later corrected. The
corrections belong in the reader-facing pages, not in the archive.

## Why two goal trees existed

`docs/goal-docs-v2/` is where the second goal cycle keeps its working memory
while that goal is still running, which is why it stays beside the current
documentation instead of in this archive. When it closes it can be moved here by
the same rule: the live journal stays next to the work, the finished one becomes
evidence. Two such trees existed because v1 had finished while v2 was still in
flight; only the finished one is archived.

## Rules

- The archive is not documentation. Do not cite it as the source of a
  behavioural claim; cite the code, a test, or a reader-facing page.
- Never link to this directory from the public documentation index or from any
  reader-facing page. The index is for navigation; the archive is for audit.
- Do not delete or rewrite archived evidence, and do not "fix" an archived
  discrepancy. Record the correction in the current page and leave the record
  intact.
- `tools/check-docs.sh` excludes `docs/internal/archive/` from its current-page
  scan, in addition to `docs/goal-desktop-v4/` and `docs/goal-docs-*`. That
  exclusion is policy: an archived link target may no longer exist, and that is
  not a defect in today's documentation.

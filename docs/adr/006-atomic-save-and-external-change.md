# ADR-006: atomic saves and recorded file identity

**Status:** accepted

## Context

DOC-002 requires a file lifecycle that never rewrites bytes the user did not change; DOC-006 forbids
silently overwriting a file that changed on disk while the tab was open; PLF-004 requires a save to
be atomic. Three separate failure modes had to be closed at once: a crash mid-write, an external
edit, and an edit that preserves size and modification time.

## Decision

**Atomic write.** The bytes go to a temporary file in the *target's own directory* (so the final
rename is a same-volume replacement, atomic on NTFS), the file is flushed and `sync_all`ed, and only
then renamed over the target. A failed write, sync or rename leaves the target byte-identical and the
temporary file is removed (`lovelace-desktop/src/document_io.rs:438`–`537`). The target's
attributes are carried over, but the read-only bit is deliberately not, because a read-only target
makes the rename fail.

**Recorded identity.** A document remembers size, modification time and an FNV-1a content hash for
the file it opened and for every file it wrote. Before a write, all three are read again; any
difference withholds the write and opens the conflict prompt (`document_io.rs:20`–`28`). The hash is
what catches a same-length rewrite restored to its old mtime, which size/mtime alone would miss.
`FileSnapshot` also drives the "deleted or renamed" state.

**Recovery journal.** Unsaved buffers are journaled with the same write-then-rename discipline to
`LOVELACE-RECOVERY` records under the application directory, debounced to one second after the first
dirty revision (`lovelace-desktop/src/recovery.rs:32`).

## Consequences

- A save either replaces the file completely or leaves it untouched; a crash cannot truncate it.
- The conflict prompt is a decision, not an error: Reload / Compare / Keep are the recorded choices.
- Every save costs a temporary file and a sync; that is the price of the guarantee.

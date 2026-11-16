# RQ-005 and RQ-007 parts 3–5 — crate pages and example runners

**Branch:** `docs/documentation-quality-remediation` · **Reviewed revision:** `fa87ecc` ·
**Date:** 2026-11-16 · **Host:** WSL Ubuntu (Linux), `CARGO_TARGET_DIR=/tmp/rl-target`.
Windows/Git Bash execution was not available in this environment and is listed under residuals.

## Files changed (all inside my ownership)

```text
 docs/crates/lovelace-int.md              |  24 ++-
 docs/crates/lovelace-json.md             |   8 +-
 docs/crates/lovelace-real.md             |  20 +--
 docs/extensions/examples/run-examples.sh |  99 +++++++++---
 docs/language/examples/run-examples.sh   | 250 ++++++++++++++++++++-----------
 5 files changed, 274 insertions(+), 127 deletions(-)
```

No fenced `lovelace` or `text` content in `docs/language/*.md` or `docs/extensions/*.md` was
changed by these edits; the only fenced block I edited is the `rust` example on the
`lovelace-int` page, which RQ-005 requires to become compilable.

---

## RQ-005.1 — `docs/crates/lovelace-int.md`

The page called the signed division form with the magnitude-division signature. The real API is:

```rust
// lovelace-int/src/lib.rs
pub fn div_rem(&self, divisor: &Nat) -> Option<(Self, Self)>
pub fn div_rem_signed(&self, divisor: &Self) -> Option<(Self, Self)>
pub fn from_nat(magnitude: Nat) -> Self
pub fn sign(&self) -> i8
```

`div_rem` documents "Divides self by a non-negative Nat"; `div_rem_signed` documents "Divides self
by a signed Int using C# truncating semantics." The old fence passed an `&Int` to `div_rem`:

```text
let q = Int::from_i64(-17).div_rem(&Int::from_i64(5)).unwrap();
```

The replacement uses both valid forms and asserts quotient and remainder for each, plus the zero
divisor refusals:

```rust
use lovelace_int::{Int, Nat};

// The divisor of `div_rem` is the magnitude type, `Nat`.
let divisor = Nat::from_u64(5);
let (q, r) = Int::from_i64(-17).div_rem(&divisor).expect("nonzero divisor");
assert_eq!((q.to_decimal(), r.to_decimal()), ("-3".to_string(), "-2".to_string()));

// `div_rem_signed` takes a signed `Int`, so the sign of the divisor is part of the request.
let (q, r) = Int::from_i64(-17)
    .div_rem_signed(&Int::from_i64(5))
    .expect("nonzero divisor");
assert_eq!((q.to_decimal(), r.to_decimal()), ("-3".to_string(), "-2".to_string()));

assert!(Int::from_i64(7).div_rem(&Nat::zero()).is_none());
assert!(Int::from_i64(7).div_rem_signed(&Int::zero()).is_none());
```

The `Public surface` list was also corrected to include `from_nat`, `sign`, and the two division
signatures rather than the bare name `div_rem`.

**Compiled and executed evidence.** The exact fenced block was extracted from the page, wrapped only
in `fn main()`, and built and run in a scratch Cargo package with a path dependency on
`lovelace-int`:

```text
exact page fence compiled and ran its assertions
INT_PAGE_FENCE_EXIT=0
```

---

## RQ-005.2 — `docs/crates/lovelace-json.md`

Both contradictory assertions were fixed. The type is:

```rust
// lovelace-json/src/lib.rs
pub enum Json {
    ...
    Obj(BTreeMap<String, Json>),
}
pub fn obj<I, K>(pairs: I) -> Json      // inserts every pair into BTreeMap::new()
```

**(a) Public surface.** `objects preserve key order` became `objects are a BTreeMap, so their keys
iterate and render in sorted key order`.

**(b) Using it.** `the published field order is the order the code writes` became `its published
field order is sorted key order, not the order the code writes and not a hash order`.

**Observed envelope evidence.** `lovelace-run --eval "1+1" --json` emits keys in this order even
though the code pushes `protocolVersion` first:

```text
top-level keys: ['artifacts', 'elapsed', 'elapsedTime', 'functions', 'mathIrVersion', 'ok',
'output', 'protocolVersion', 'result', 'revision', 'symbolicFormatVersion', 'timings',
'variables']
```

The page's Limitations section already stated the `BTreeMap` behaviour; the two conflicting
sentences no longer contradict it.

---

## RQ-005.3 — `docs/crates/lovelace-real.md`

The cap is thread-local and scoped, not process-wide:

```rust
// lovelace-real/src/lib.rs
thread_local! {
    static DISPLAY_DECIMAL_PLACES: Cell<usize> = const { Cell::new(usize::MAX) };
}
pub fn display_decimal_places() -> usize
pub fn with_display_decimal_places<T>(places: usize, f: impl FnOnce() -> T) -> T
```

`with_display_decimal_places` replaces the value on the calling thread, installs a `Restore` guard
whose `Drop` writes back the previous value, and then runs the closure. The `Public surface` bullet
now says thread-local and scoped; the Precision behaviour paragraph says "the scope is active on the
calling thread"; the Limitations bullet calls it "thread-local scoped state", notes restoration and
that another thread keeps its own cap.

**Runtime evidence.** A scratch package with path dependencies on `lovelace-int` and
`lovelace-real` asserted, from the real implementation:

```text
real display cap is thread-local and scope-restored OK
```

That program checks `display_decimal_places()` is `usize::MAX` before the scope, renders
`sqrt(2)` at 50 places as `1.41421` under a 5-place scope, restores the full 50-place rendering
afterwards, and spawns a thread *inside* the scope whose `display_decimal_places()` is still
`usize::MAX` and whose rendering is still the full 50 places.

---

## RQ-005.4 — scan of the remaining crate pages

I checked every page under `docs/crates/` against the implementation for the same classes of
contradiction: object/iteration order versus the container type, process/global versus thread-local
or scoped state, and Rust fences versus the named function signatures. No further contradiction in
those classes was found.

- `lovelace-nat.md` says `Nat::precision`/`display_digits` hold the reference's global configuration
  "but no arithmetic or rendering path in this crate reads them", which matches the source.
- `lovelace-symbolics.md` says cancellation state is thread-local and scope-restored; that matches
  the `thread_local!` slot and `Scope` guard in the cancellation module.
- The remaining Rust fences (`lovelace-array.md`, `lovelace-lang.md`, `lovelace-nat.md`,
  `lovelace-real.md`, `lovelace-symbolics.md`, `lovelace-json.md`) were read against the named
  method signatures and argument types. Only the corrected `lovelace-int` fence (and the
  `lovelace-real` scope program) was compiled; compiling every crate-page fence is RQ-010/C10, not
  this requirement.

Out-of-scope residuals found during that scan, not touched because they are not crate pages or not
in my file ownership:

1. `CONTRACT.md` still lists `div_rem(&Int)->Option<(Int,Int)>`; the implementation takes `&Nat` and
   the signed form is `div_rem_signed`.
2. `lovelace-json/src/lib.rs` has doc comments that still say "preserving object key order",
   "insertion ordered" and "preserving the given order" above a `BTreeMap`; the page now describes
   the actual sorted order.
3. `lovelace-int/src/lib.rs` module docs say "base-10^9 limbs", while `lovelace-nat` stores
   little-endian base-2^64 limbs.

---

## RQ-007 parts 3–5 — both runners

### Inventory approach: validated manifest (not a glob-only inventory)

For each runner, the page list is now a **manifest validated in both directions** against its
directory:

- every manifest entry must exist and be readable (a deleted page fails);
- every `*.md` file in the directory must be in the manifest (a new page fails instead of being
  silently skipped);
- therefore a stale or empty manifest cannot narrow the checked set silently.

No page is excluded. `docs/extensions/README.md` is scanned even though it has no `lovelace` block;
a page-level zero is allowed and the run-level zero is not. That is why no "stated reason"
exclusion list exists to get stale: there are no exclusions.

Why a manifest rather than a plain directory inventory: a pure glob cannot satisfy requirement
3(b). If `builtins.md` is deleted, a glob simply produces nine pages and the run still has examples,
so the gate passes on a smaller set. The manifest makes deletion a hard failure while the reverse
check makes a stale manifest (a new page) a hard failure too.

### Guards added to both runners

- **Missing declared page:** fatal with `declared page is missing or unreadable: docs/…/<page>`.
- **Unlisted page:** fatal with `docs/…/<page> is not in the page manifest`, including the
  instruction to add it or record an exclusion with a reason.
- **Zero examples:** language runner exits 1 with
  `no ```lovelace/```text example pairs found under docs/language; refusing to pass on zero work`;
  extensions runner exits 2 with `No examples were found in 4 page(s); nothing was verified`.
- **Unpaired `lovelace` block:** no following text block, or the next block is another language, or
  prose separates script from output — fatal.
- **Unpaired `text` block:** a `text` block directly following a fence of a different language is
  fatal; it has lost the script it is supposed to document. Standalone `text` blocks that follow
  prose or start a page (the existing plots/error pages have those) remain valid.
- The example comparison itself was **not** weakened: the language runner still compares normalised
  stdout/stderr with the next text block, and the extensions runner still compares the combined
  stdout/stderr in one-space normal form. Pairing is now adjacency-of-fences, which is what the
  existing pages already use.

The extensions runner's unpaired list now records both directions and its summary heading is
`unpaired example blocks:` rather than a lovelace-only label.

### Clean-tree verification (after the edits)

```text
$ bash docs/language/examples/run-examples.sh
README.md: 4 example(s) passed
lexical-structure.md: 19 example(s) passed
values-and-types.md: 23 example(s) passed
expressions-and-operators.md: 44 example(s) passed
statements-and-control-flow.md: 26 example(s) passed
functions-and-scope.md: 22 example(s) passed
builtins.md: 25 example(s) passed
errors-and-diagnostics.md: 5 example(s) passed
precision-and-formatting.md: 18 example(s) passed
plots.md: 7 example(s) passed

All 193 examples passed.
LANG_EXIT=0

$ bash docs/extensions/examples/run-examples.sh      # tail
PASS  symbolics.md:306 (exit 0)
PASS  symbolics.md:318 (exit 0)
PASS  symbolics.md:330 (exit 0)
PASS  symbolics.md:342 (exit 1)
PASS  symbolics.md:353 (exit 1)

pages: 4   examples checked: 27   passed: 27   failed: 0   unpaired: 0
OK: 27 examples match the documented output.
EXT_EXIT=0
```

Both counts are unchanged from the pre-edit baseline (193 and 27), so the new guards did not skip or
drop a page that previously ran.

### Negative tests

Each case copies the real runner byte-for-byte into a scratch tree and constructs the defect there,
so the page content under test is never mutated. In addition, one real-tree mutation removes a
declared page, observes the failure, restores the page byte-for-byte and reruns the clean tree. The
scratch copies' hashes were recorded:

```text
runner hashes (scratch copies must match):
ca88792dde0140bbc63fe7d29cd3e1a2d3abb88c9a29581ffdb02c29d8f215b7  docs/language/examples/run-examples.sh
4f4beb1342c64a59fe50f7ff39e2342f4f50c391250538e2887207945161024a  docs/extensions/examples/run-examples.sh
```

````text
=== language: declared page missing ===
exit=1 (expected 1)
error: declared page is missing or unreadable: docs/language/plots.md

=== language: zero examples ===
exit=1 (expected 1)
README.md: 0 example(s) passed
lexical-structure.md: 0 example(s) passed
values-and-types.md: 0 example(s) passed
expressions-and-operators.md: 0 example(s) passed
statements-and-control-flow.md: 0 example(s) passed
functions-and-scope.md: 0 example(s) passed
builtins.md: 0 example(s) passed
errors-and-diagnostics.md: 0 example(s) passed
precision-and-formatting.md: 0 example(s) passed
plots.md: 0 example(s) passed
error: no ```lovelace/```text example pairs found under docs/language; refusing to pass on zero work

=== language: lovelace block with no text block ===
exit=1 (expected 1)
error: docs/language/README.md:3: lovelace block has no following text block

=== language: text block with no preceding lovelace block ===
exit=1 (expected 1)
error: docs/language/README.md:7: text block is not the output of a preceding lovelace block (preceded by 'json')

=== language: runnable page absent from the manifest ===
exit=1 (expected 1)
error: docs/language/new-page.md is not in the page manifest; add it to 'pages' (or exclude it with a stated reason) so it cannot be silently skipped

=== extensions: declared page missing ===
exit=1 (expected 1)
error: declared page is missing or unreadable: docs/extensions/dsp.md

=== extensions: zero examples ===
exit=2 (expected 2)

pages: 4   examples checked: 0   passed: 0   failed: 0   unpaired: 0
No examples were found in 4 page(s); nothing was verified.

=== extensions: lovelace block with no text block ===
exit=1 (expected 1)

unpaired example blocks:
  README.md:4 (lovelace block has no immediately following text block)

pages: 4   examples checked: 0   passed: 0   failed: 0   unpaired: 1

=== extensions: text block with no preceding lovelace block ===
exit=1 (expected 1)

unpaired example blocks:
  README.md:7 (text block preceded by 'json', not lovelace)

pages: 4   examples checked: 0   passed: 0   failed: 0   unpaired: 1

=== extensions: runnable page absent from the manifest ===
exit=1 (expected 1)
error: docs/extensions/new-page.md is not in the page manifest; add it to 'pages' (or exclude it with a stated reason) so it cannot be silently skipped

NEGATIVE TESTS: OK (10 cases, every guard fired with a diagnostic)
````

The "runnable page absent from the manifest" cases contain a complete, executable `lovelace`/`text`
pair; they still fail before execution, so a page with real examples cannot be dropped silently.

Real-tree mutation and restore (the missing-page case against the actual documentation directory):

```text
--- real tree: docs/language/plots.md temporarily removed ---
error: declared page is missing or unreadable: docs/language/plots.md
MUTATED_EXIT=1
RESTORED_SHA_BEFORE=75d33b9e806671f3f96c84208fa6309924b61476c60145b518201b97e76c4793
RESTORED_SHA_AFTER =75d33b9e806671f3f96c84208fa6309924b61476c60145b518201b97e76c4793
RESTORE=byte-identical
plots.md: 7 example(s) passed

All 193 examples passed.
RESTORED_CLEAN_EXIT=0
```

---

## Documentation gate verification

```text
$ bash tools/check-docs.sh --symbols
check-docs: 57 Markdown files, 158 relative links, 234 code references, 20 line-suffix citations, 19 symbol citations
check-docs: OK
SYMBOLS_EXIT=0

$ bash tools/check-docs.sh
check-docs: 57 Markdown files, 158 relative links, 234 code references
check-docs: OK
DOCS_EXIT=0

$ bash tools/check-reference-pages.sh
check-reference-pages: 16 page(s), 112 required sections in order
check-reference-pages: OK
REFS_EXIT=0
```

The earlier failure of `--symbols` came from the harness tree `docs/goal-docs-v2/`, which was then
excluded from the checker by its owner; the three crate pages and both runners were never the
cause. This report is inside that excluded harness tree.

---

## Residuals and what was not verified

1. **PowerShell runners are out of scope and were not updated by me.** As read during this work,
   the language `.ps1` still has a fixed page array and no zero-example guard; the extensions `.ps1`
   still discovers pages by glob and has no declared-page check. Reading them showed the same
   false-positive paths RQ-007 names; this work covers only the two `.sh` runners in my ownership.
   No PowerShell was available in WSL to execute them, so this is a source observation, not a test
   result.
2. **Windows/Git Bash execution was not run.** The scripts retain the existing `.exe` discovery and
   `cygpath` handling, and the new code uses only bash features already used by the scripts
   (`[[ =~ ]]`, arrays, `shopt -s nullglob`), but the Windows variant is unexercised here.
3. **The source doc-comment residuals listed under RQ-005.4 remain.** They are implementation
   comments, not crate pages.
4. **No hosted CI was run for this work.** All evidence above is local WSL Ubuntu.

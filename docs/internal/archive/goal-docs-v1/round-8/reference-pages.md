# Round 8 — DOC-014 reference-page contract and gate

Date: 2026-11-15
Owner slice: `docs/reference-template.md`, `docs/reference-pages.md`,
`tools/check-reference-pages.sh`, one step in the `docs` job of `.github/workflows/ci.yml`, and the
16 reference pages listed in the manifest. No `.rs` file and no page outside the manifest was
edited.

## What was delivered

1. **`docs/reference-template.md`** — the seven-section contract (Purpose · Inputs · Outputs ·
   Precision behaviour · Errors · Limitations · Examples), one sentence per section on what belongs
   there, the "None is a documented answer, not an omission" rule, and the rules that a listed page
   has no other level-2 heading, that content is moved rather than rewritten, and that a fenced
   `lovelace`/`text` pair is one unit.
2. **`docs/reference-pages.md`** — the manifest: 16 pages, each with its surface and a one-line
   justification, plus an explicit "Not in scope" table naming the ten narrative language guides,
   `docs/extensions/README.md` and `docs/crates/lovelace-desktop-spike.md` with the reason for each,
   and a paragraph covering every other `docs/` tree.
3. **`tools/check-reference-pages.sh`** — reads the `## Manifest` table of the manifest, and for each
   listed page fails with `path: section` for a missing required heading, an out-of-order heading, a
   duplicate heading, or any other level-2 heading. Exit 0 only when every listed page conforms;
   exit 2 for bad usage/missing manifest/no pages. Bash + awk + mktemp only, CRLF-tolerant, so it
   runs in the `ubuntu-latest` and `windows-latest` docs jobs. Wired as the `Check reference pages`
   step after `tools/check-docs.sh --symbols` in the `docs` job.
4. **Restructured the 16 listed pages.** Every pre-existing sentence, table, code block and example
   is preserved: the check below compares every non-heading body line of the four pages with a
   pre-edit copy (`0` missing) and of the twelve crate pages with `HEAD` (only the pre-existing
   Round-7 citation edits and the earlier `.lv`→`.ls` edit are absent, listed under "Not changed").

## Per page: sections added and where the content came from

For every page, `Purpose` already existed and is retained. "Moved" means the existing text and its
heading were placed under the required section, heading level only changed.

| Page | Required sections | Where the added/moved content came from |
|---|---|---|
| `docs/language/builtins.md` | Inputs (`Reading the catalogue` moved; `Full catalogue` moved up), Outputs (the 11 family sections moved), Precision behaviour (new), Errors (new), Limitations (new), Examples (`Numerics`…`Optimization` example pairs moved, with the Symbolics lead-in) | Precision/Errors/Limitations were derived from the page's own per-family statements (caps, `eig_sym` real-symmetric only, solve domains, `and`/`or`/`not`, no complex literal, `new_bare`), from `lovelace-lang/src/diagnostic.rs` (the frozen seven categories) and `lovelace-lang/src/lib.rs` (`LangError` fields), and from `docs/editorial.md` (claim kinds). No sentence was rewritten. |
| `docs/extensions/dsp.md` | Purpose (`What it is`, `Where the code lives`), Inputs (`How it is enabled`), Outputs (`Every function it contributes`), Precision behaviour (`Precision and limits`), Errors (new), Limitations (`What is NOT in this extension` + `Known defect: movingavg…` moved out of Purpose), Examples (`Worked examples`, `Verified calls`) | Existing text moved. Errors table derived from observed page messages plus `lovelace-lang/src/builtins.rs` (`FFT length must be a power of two`, lines 6644-6649: `InvalidArgument`/`TypeMismatch`), `LangError::plain` = `InvalidOperation`/`DomainError` (`lib.rs`), `modus.rs` (`Plugin … already loaded`, `invalid_operation`) and `unknown_function_among`. |
| `docs/extensions/mathir.md` | Same shape; `Precision, limits and the version` under Precision behaviour | Existing text moved. Errors table derived from `lovelace-lang/src/mathir.rs` (version gate and malformed line: `InvalidInput`/`ParseError`; IR validation: `plain` → `InvalidOperation`/`DomainError`), `builtins.rs` (kernel arity message), and the page's `Compilation requires at least one parameter` (`InvalidArgument`/`TypeMismatch`). |
| `docs/extensions/symbolics.md` | Same shape; `Precision, assumptions and limits` under Precision behaviour | Existing text moved. Errors table derived from the page's own quoted messages and `lovelace-lang/src/builtins.rs:546` (`UnsupportedOperation`/`UnsupportedOperation` for non-integer exponents); undefined-variable code from `LangError::plain`. |
| `docs/crates/lovelace-array.md` | Inputs (`Public surface` + `Dependencies`), Outputs, Precision behaviour, Errors, Limitations, Examples (`Using it` + `Tests`) | `lovelace-array/src/lib.rs`: the `NdArray`/`Field` signatures, the `ArrayError` variants and their doc comments, `Field for i64` (`/` truncation), `from_vec`/`from_rows` error contracts, the owned (non-view) slice/reshape results. |
| `docs/crates/lovelace-bench.md` | Inputs (`Command line` + `Dependencies`), Outputs, Precision behaviour, Errors, Limitations, Examples (`Using it` + `Tests`) | `lovelace-bench/src/main.rs`: usage/exit 2, `expect("digits")`, default `reps = 20`, unknown `op` → exit 2; the digest claim is the page's own. |
| `docs/crates/lovelace-corpus.md` | Inputs (`Public surface` + `Dependencies`; corpus provenance moved under Purpose), Outputs, Precision behaviour, Errors, Limitations, Examples | `lovelace-corpus/src/main.rs`: exit 0/1/2, summary line format, `corpus is empty: refusing a vacuous pass`, filter-as-prefix and the `0 passed` vacuous selection; library surface from the page and `src/lib.rs`. |
| `docs/crates/lovelace-desktop.md` | Inputs (`Public surface` + `Dependencies`; `Structure` under Purpose), Outputs, Precision behaviour, Errors, Limitations, Examples (`Using it` + `Tests`) | The page's mode table and dependency paragraph, `lovelace-desktop/src/main.rs` (mode dispatch, `process::exit(2)` for bad arguments), and `docs/editorial.md`/`AGENTS.md` (UI never evaluates; a headless selftest is not a GUI journey pass). |
| `docs/crates/lovelace-int.md` | Inputs (`Public surface` + `Dependencies`), Outputs, Precision behaviour, Errors, Limitations, Examples | `lovelace-int/src/lib.rs`: `div_rem(&Nat)` vs `div_rem_signed(&Int)`, truncating division, `from_decimal` doc, `i64::MIN`, `Display`. |
| `docs/crates/lovelace-json.md` | Inputs (`Public surface` + `Dependencies`), Outputs, Precision behaviour, Errors, Limitations, Examples | `lovelace-json/src/lib.rs`: the `Json` enum (including `Obj(BTreeMap<…>)`), `JsonError { offset, message }`, `parse`/`to_pretty` docs, `num_array` taking `i64`, the literal-preserving `Num(String)`. |
| `docs/crates/lovelace-know.md` | Inputs (`Public surface` + `Runner selection` + `Dependencies`, plus the CLI/request fields), Outputs, Precision behaviour, Errors, Limitations, Examples | `lovelace-know/src/main.rs` (the `--eval`/`--file`/`--stdin`/`--help` options, usage errors exit 2, error response + exit 1) and `src/cli.rs` module doc (0/1/2 contract, the six commands, the `CliResponse` field set); determinism from the page's proposal/seed text. |
| `docs/crates/lovelace-lang.md` | Inputs (`Public surface` + `Dependencies`), Outputs, Precision behaviour, Errors, Limitations, Examples (`How to use it` + `Tests`) | `lovelace-lang/src/lib.rs`: the default computation precision 1000, display precision 100, `set_precision`/`set_display_precision` doc comments, `LangError` fields, `§` categories in `src/diagnostic.rs`; plugin/`new_bare` behaviour from the page. |
| `docs/crates/lovelace-nat.md` | Inputs (`Public surface` + `Dependencies`), Outputs, Precision behaviour, Errors, Limitations, Examples | `lovelace-nat/src/lib.rs`: `sub`/`div_rem`/`from_decimal`/`to_u64` docs, the verified Burnikel–Ziegler fallback, `render_decimal` not consulting the display statics, the "configuration only on the Rust side" comment; `digit_store.rs`'s non-canonical-mutator caveat. |
| `docs/crates/lovelace-real.md` | Inputs (`Public surface` + `Dependencies`), Outputs, Precision behaviour, Errors, Limitations, Examples | `lovelace-real/src/lib.rs`: `from_fraction` doc (terminating exact at any budget, period at the cap never discovered, `DEFAULT_COMPUTATION_DECIMAL_PLACES = 1000`), `div`/`pow_int`/`to_int_exact` refusals, `sqrt` of a negative returning zero, the `with_display_decimal_places` scope. |
| `docs/crates/lovelace-run.md` | Inputs (`Public surface` + `Command line` + `Dependencies`), Outputs, Precision behaviour, Errors, Limitations, Examples | `lovelace-run/src/main.rs` (`usage`, `parse_args`, exit 2 on usage, exit 1 on a failed run in both modes) and the observed envelope (`target/debug/lovelace-run --eval "1/3"`); UTF-16 positions are the page's own contract. |
| `docs/crates/lovelace-symbolics.md` | Inputs (`Public surface` + `Dependencies`), Outputs, Precision behaviour, Errors, Limitations, Examples | `lovelace-symbolics/src/lib.rs` (the `Expr`/`Rat` signatures, `diff`, the predicates), `src/cancel.rs` (`check`/`poll` meanings, `POLL_INTERVAL`, thread-local deadline, latched observation); exact-rational character from the page. |

## Verification output

### 1. The gate passes on all 16 listed pages

```text
$ bash tools/check-reference-pages.sh
check-reference-pages: 16 page(s), 112 required sections in order
check-reference-pages: OK
```

### 2. Falsification: one section removed, then restored byte-identically

`Errors` was removed from `docs/crates/lovelace-int.md` (the working copy was copied to
`/tmp/doc014-falsify-int.md`, then restored with `cp`):

```text
$ cp docs/crates/lovelace-int.md /tmp/doc014-falsify-int.md
$ sha256sum docs/crates/lovelace-int.md /tmp/doc014-falsify-int.md
053940e76257e968aee4967fa874aab427fd4ddd6e719685f6ce790061134d65  docs/crates/lovelace-int.md
053940e76257e968aee4967fa874aab427fd4ddd6e719685f6ce790061134d65  /tmp/doc014-falsify-int.md
$ python3 .tmp/remove_section.py docs/crates/lovelace-int.md Errors
removed ## Errors from docs/crates/lovelace-int.md
$ bash tools/check-reference-pages.sh
check-reference-pages: 16 page(s) checked, 15 conform, 1 failed (1 defect(s))
check-reference-pages: FAIL
docs/crates/lovelace-int.md: Errors
[exit code: 1]

$ cp /tmp/doc014-falsify-int.md docs/crates/lovelace-int.md
$ sha256sum docs/crates/lovelace-int.md /tmp/doc014-falsify-int.md
053940e76257e968aee4967fa874aab427fd4ddd6e719685f6ce790061134d65  docs/crates/lovelace-int.md
053940e76257e968aee4967fa874aab427fd4ddd6e719685f6ce790061134d65  /tmp/doc014-falsify-int.md
$ bash tools/check-reference-pages.sh
check-reference-pages: 16 page(s), 112 required sections in order
check-reference-pages: OK
```

The order arm was also exercised: swapping `Errors` and `Limitations` in the same page produced
`docs/crates/lovelace-int.md: Errors (out of order; it must come after Limitations)`, and restoring
the backup returned the same sha256 and `check-reference-pages: OK`. The removal and swap helpers
were scratch scripts in an untracked `.tmp/` directory; that directory was deleted before the final
`git status` below.

### 3. Executable extension examples — unchanged, still 27/27

```text
$ bash docs/extensions/examples/run-examples.sh
...
pages: 4   examples checked: 27   passed: 27   failed: 0   unpaired: 0
OK: 27 examples match the documented output.
```

The language runner was also run because `docs/language/builtins.md` is executed by it:
`All 193 examples passed.` (25 of them in `builtins.md`).

### 4. Fenced blocks are unchanged

A block-by-block comparison (language + content, as a multiset, against a pre-edit copy for the
four language/extension pages and against `HEAD` for the crate pages) reports every block identical:
`builtins.md` 51, `dsp.md` 22, `mathir.md` 18, `symbolics.md` 20, and 1–2 per crate page. The one
difference against `HEAD` is `docs/crates/lovelace-desktop.md`'s console block, which already said
`notes.ls` in the working tree (`HEAD` said `notes.lv`); this task did not edit it.

### 5. `check-docs.sh --symbols` still OK

```text
$ bash tools/check-docs.sh --symbols
check-docs: 57 Markdown files, 158 relative links, 233 code references, 20 line-suffix citations, 19 symbol citations
check-docs: OK
```

The first run of this task caught one broken link introduced in `builtins.md`
(`extensions/dsp.md` from `docs/language/`); it was corrected to `../extensions/dsp.md`, and the
summary above is the post-fix run. The 20 line-suffix citations are all in `docs/adr/`, which the
checker exempts as historical.

### 6. Scoped `git status --short`

```text
 M .github/workflows/ci.yml
 M docs/crates/lovelace-array.md
 M docs/crates/lovelace-bench.md
 M docs/crates/lovelace-corpus.md
 M docs/crates/lovelace-desktop.md
 M docs/crates/lovelace-int.md
 M docs/crates/lovelace-json.md
 M docs/crates/lovelace-know.md
 M docs/crates/lovelace-lang.md
 M docs/crates/lovelace-nat.md
 M docs/crates/lovelace-real.md
 M docs/crates/lovelace-run.md
 M docs/crates/lovelace-symbolics.md
 M docs/extensions/dsp.md
 M docs/extensions/mathir.md
 M docs/extensions/symbolics.md
 M docs/language/builtins.md
?? docs/goal-docs-v1/round-8/
?? docs/reference-pages.md
?? docs/reference-template.md
?? tools/check-reference-pages.sh
```

`docs/crates/lovelace-desktop-spike.md` is deliberately absent: it is excluded in the manifest (see
below) and was not edited.

## Manifest decisions

- **16 pages in scope:** `docs/language/builtins.md`, the three extension family pages, and the
  twelve `docs/crates/*.md` pages that document a public API.
- **Not in scope:** the ten narrative language guides (`README.md`, `lexical-structure.md`,
  `expressions-and-operators.md`, `statements-and-control-flow.md`, `functions-and-scope.md`,
  `values-and-types.md`, `precision-and-formatting.md`, `errors-and-diagnostics.md`, `plots.md`) and
  `docs/extensions/README.md` are language guides and indexes, not API reference;
  `docs/crates/lovelace-desktop-spike.md` documents no API (its own "Public surface" is `None`; it is
  a `publish = false` binary outside the workspace, kept as measured evidence and not built by
  `cargo build --workspace`). All are named with this reason in the manifest's "Not in scope" table.

## Not changed (out of scope, recorded rather than silently fixed)

These are pre-existing documentation defects found while reading the source; the task forbids
rewriting an existing sentence, so they are reported here instead:

1. `docs/crates/lovelace-int.md` — the `Using it` example calls
   `Int::from_i64(-17).div_rem(&Int::from_i64(5))`, but `Int::div_rem` takes `&Nat`
   (`lovelace-int/src/lib.rs`); the example as written does not compile (the sibling
   `div_rem_signed(&Int)` is the signed form). The example was preserved byte-for-byte.
2. `docs/crates/lovelace-json.md` — "objects preserve key order" and the `Using it` sentence "the
   published field order is the order the code writes" do not match `Obj(BTreeMap<String, Json>)`,
   which renders keys in sorted order; the observed `lovelace-run` envelope is alphabetical. The
   new Limitations section states the `BTreeMap` behaviour, and both existing sentences are
   preserved.
3. `docs/crates/lovelace-real.md` — the `Public surface` bullet calls the display cap
   "process-wide"; it is thread-local (`thread_local!` in `lovelace-real/src/lib.rs`). The new
   Precision behaviour section describes the cap as scoped and restored, without repeating
   "process-wide".

## Scope statement

No `.rs` file, corpus fixture, golden envelope, image, or page outside the manifest was touched. The
only CI change is the one `Check reference pages` step in the existing `docs` job. The three
acceptance commands requested all pass in WSL: `check-reference-pages.sh` (0), the extension example
runner (`27 passed, failed: 0`), `check-docs.sh --symbols` (`OK`).

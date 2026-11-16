# Reference pages

This is the manifest of public reference pages and the contract they follow. A page is in scope
when it documents a **callable or programmatic surface**: a family of language builtins, a library
crate's public API, or a command-line binary someone runs. The shape those pages must have is
[the reference template](reference-template.md); the gate that enforces it is
[`tools/check-reference-pages.sh`](../tools/check-reference-pages.sh), run by the `docs` job in
[`.github/workflows/ci.yml`](../.github/workflows/ci.yml).

The gate is **two-way**, and it is the manifest, not a convention, that decides scope. The three
reference families — `docs/language/`, `docs/extensions/` and `docs/crates/` — are the **scoped
trees**. Every Markdown page under a scoped tree must appear in exactly one of the two tables
below:

- in **Manifest**, as a reference page that carries the seven sections; or
- in **Not in scope**, as a declared non-reference page with a reason.

A page that appears in neither table is unclassified and fails the gate. That is the point: before
this rule, a new callable-surface page passed by simply not being listed. A page that appears in
both tables is a contradiction and fails. A manifested page whose file is missing fails too, and a
required section whose heading introduces nothing before the next level-2 heading fails.

## Manifest

The table is the gate's input. Each row lists one page, the surface it documents, and why that
surface is callable or programmatic. The checker reads the two tables only: every row's first
backticked token is a page path, and every page path listed here must carry the seven sections
(`Purpose`, `Inputs`, `Outputs`, `Precision behaviour`, `Errors`, `Limitations`, `Examples`) in
that order, with no other level-2 heading on the page and no empty section. Adding a reference page
means adding a row here in the same change.

| Page | Surface | Why it is in scope |
|---|---|---|
| `docs/language/builtins.md` | the 124 engine builtins | The callable language catalogue: every builtin's name, arity, parameters and result, dispatched by `Engine::call`. |
| `docs/extensions/dsp.md` | 16 DSP plugin builtins | Callable from every script: `fft`, `dft`, `filter`, `noise` and the rest of `Lovelace.Dsp`. |
| `docs/extensions/mathir.md` | 5 Compilation plugin builtins | Callable from every script: `lower`, `compile`, `compile_full`, `evalir`, `evalir_batch` lower and evaluate kernels. |
| `docs/extensions/symbolics.md` | 59 Symbolics plugin builtins | Callable from every script: the `diff`/`solve`/`simplify` family, the largest plugin surface in the engine. |
| `docs/crates/lovelace-array.md` | `NdArray<T>`, `Field`, `Slice`, `ArrayError` | The library API every array operation in the language is implemented over. |
| `docs/crates/lovelace-bench.md` | the `lovelace-bench` binary | A programmatic tool: `lovelace-bench <op> <digits> [reps]` prints machine-readable measurements and a digest. |
| `docs/crates/lovelace-corpus.md` | the `lovelace-corpus` binary and library | The parity gate is run by CI (`[filter] [--verbose]`) and the library exposes `dotnet_random` and `property_corpus`. |
| `docs/crates/lovelace-desktop.md` | the `lovelace-desktop` binary | A programmatic surface: the window plus the headless acceptance modes (`--gui-selftest`, `--journey`, `--raster-svg`, audits) a build host runs. |
| `docs/crates/lovelace-int.md` | `Int` | The signed arbitrary-precision integer API every numeric layer above `lovelace-nat` builds on. |
| `docs/crates/lovelace-json.md` | `Json` | The workspace's only JSON reader/writer; every wire envelope and settings document goes through it. |
| `docs/crates/lovelace-know.md` | the `lovelace-know` binary and modules | A JSON-over-stdio command protocol plus the library modules a host embeds. |
| `docs/crates/lovelace-lang.md` | `Engine`, `Value`, `LangError` | The engine API every host (runner, corpus, knowledge graph, desktop) calls instead of re-implementing the language. |
| `docs/crates/lovelace-nat.md` | `Nat`, `Number`, `DigitStore` | The arbitrary-precision natural-number API at the base of the numeric stack. |
| `docs/crates/lovelace-real.md` | `Real` | The arbitrary-precision real API, including the exact periodic-decimal representation. |
| `docs/crates/lovelace-run.md` | the `lovelace-run` binary | The script runner and the stable JSON wire envelope an external harness depends on. |
| `docs/crates/lovelace-symbolics.md` | `Expr`, `Rat`, `cancel` | The symbolic-algebra library API: canonical expressions, `Expr` construction and the cancellation token. |

## Not in scope

These pages live beside the reference pages but are narrative guides, indexes or evidence, not
reference for a callable surface. They are the **declared non-reference pages**. Every entry needs
a reason: an exemption without one is a defect in this manifest, and so is an entry whose file no
longer exists, because a stale exemption would silently cover a future reference page at the same
path. When a page is added under a scoped tree, classify it here or in the Manifest table in the
same change.

| Page | Why it is not in scope |
|---|---|
| `docs/language/README.md` | The index of the language reference: it links the other pages and orients a reader; it documents no surface of its own. |
| `docs/language/lexical-structure.md` | A language guide: it describes tokens, literals and source encoding (syntax), not a callable or programmatic interface. |
| `docs/language/expressions-and-operators.md` | A language guide: operators and broadcasting are syntax the parser implements, not an API a caller invokes. |
| `docs/language/statements-and-control-flow.md` | A language guide: statements, blocks and control flow are grammar, not a callable surface. |
| `docs/language/functions-and-scope.md` | A language guide: how user functions and scoping work, not a public API. |
| `docs/language/values-and-types.md` | A language guide to the value kinds and record shapes a script manipulates; the programmatic surface that produces them is `Engine`/`Value` in `docs/crates/lovelace-lang.md`. |
| `docs/language/precision-and-formatting.md` | A cross-cutting guide to how precision and display behave across the language; it owns no callable surface of its own. |
| `docs/language/errors-and-diagnostics.md` | A language guide to the error taxonomy and diagnostics a script sees, not a reference for an API entry point. |
| `docs/language/plots.md` | A guide to the plot contract; the only callable entry point, `plot`, is catalogued in `docs/language/builtins.md`, and the guide itself defines no programmatic surface. |
| `docs/extensions/README.md` | The index of the plugin families: it explains the plugin model and links the three family pages; it contributes no builtins. |
| `docs/crates/lovelace-desktop-spike.md` | A crate page that documents no API: its own "Public surface" is `None`, it is a `publish = false` binary kept as measured evidence, it is not a workspace member, and `cargo build --workspace` does not build it. The `cargo run` command on the page reproduces a historical measurement; it is not a maintained callable surface. |

Everything else under `docs/` is out of scope for the same family of reasons: `architecture.md`,
`testing.md`, `editorial.md`, `contributing.md`, `extending.md`, `release.md`, `skills.md` and the
rest are explanation or procedure; `docs/adr/` is decision history; `docs/csharp-oracle/` and
`docs/symbolics/` are fixtures; and `docs/goal-desktop-v4/` and `docs/goal-docs-v1/` are archived
specification and harness working memory. None of them documents a public callable or programmatic
surface.

## Conformance

A page is conformant when the checker exits 0. On failure the checker prints one line per defect,
`file:line: message`, where the file and line are the defective heading, the unclassified page
(line 1), or the manifest row that lists or exempts a missing page. The defects are:

- `missing required section '…'` — one of the seven headings is absent;
- `'…' is out of order; it must come after …` — a heading appears before its predecessor;
- `unexpected level-2 heading '…'` — any other level-2 heading, which must be nested;
- `required section '…' is empty` — the heading has no content before the next level-2 heading;
- `page is not classified in docs/reference-pages.md` — a scoped-tree page in neither table;
- `listed reference page '…' does not exist` — a Manifest row for a missing file;
- `declared non-reference page '…' does not exist` or `has no reason` — a stale or unjustified
  exemption.

Run it from the repository root with:

```console
$ bash tools/check-reference-pages.sh
```

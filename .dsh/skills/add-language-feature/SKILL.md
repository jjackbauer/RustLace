---
name: add-language-feature
description: Change the active lexer, parser or interpreter with focused tests and the corpus and golden fixtures that pin the contract.
whenToUse: Use when adding syntax, a statement or expression form, a token or an evaluation rule to the Lovelace language, including a change that shifts source positions or rendered values.
---

# Add a language feature

The active language pipeline is in two files: `lovelace-lang/src/syntax.rs` owns the tokens, AST
and recursive-descent parser that `Engine` actually uses, and `lovelace-lang/src/lib.rs` owns the
tree-walking evaluation (`eval_text`, `exec`, `eval_expr_node`). Several older files with similar
names are tracked but are not declared as modules and do not compile - read the `mod` list at the
top of `lovelace-lang/src/lib.rs` before editing anything.

## Files you will touch

- `lovelace-lang/src/syntax.rs` - `Tok`, `Token`, `tokenize`, `Expr`, `Stmt` and `Parser`.
- `lovelace-lang/src/lib.rs` - `Engine::eval_text`, `exec_statement`, `eval_expr_node` and module
  declarations.
- `lovelace-lang/src/builtins.rs` - only when the feature is a callable builtin (see `add-builtin`).
- `lovelace-lang/src/lexer.rs` - the published reference lexer surface; update it only when the
  change also belongs to that additive vocabulary.
- `lovelace-lang/src/suite_lex_parse_batch.rs` and `lovelace-lang/src/suite_parity_batch.rs` - lexer,
  parser and evaluation tests.
- `parity/corpus/lang.json` - the documented-language fixture.
- `lovelace-run/src/golden_batch.rs` and `docs/csharp-oracle/Lovelace.Run.Tests/fixtures/` - the wire
  envelopes when the feature changes the envelope or source positions.
- `docs/crates/lovelace-lang.md`, `docs/glossary.md`, `docs/parity-with-studio.md` - the prose when
  the public language surface or a parity decision changes.

## Procedure

1. **Confirm which pipeline is live.** `lovelace-lang/src/lib.rs` declares `pub mod syntax;` and
   imports `use syntax::{Expr, IndexArg, InterpPart, Parser, Stmt};`. `Engine::eval_text` calls
   `syntax::tokenize` and `Parser::new(toks).program()`. The files `parser.rs`, `token.rs`, `ast.rs`,
   `interp.rs` and `value.rs` are not declared as modules; editing them compiles nothing and changes
   no behaviour. If you are about to edit one of those, stop and re-read this step.

2. **Add the token.** In `lovelace-lang/src/syntax.rs`, add the variant to `Tok`, handle it in
   `tokenize`, and preserve UTF-16 code-unit positions (`positions` in `tokenize` already counts
   `len_utf16`). Token positions are part of the wire contract. If the token is also part of the
   published reference vocabulary in `lovelace-lang/src/lexer.rs`, add it there too and append it
   after the existing `TokenKind` members; the published ordinals are contract and are not
   renumbered.

3. **Add the AST and the parse rule.** Add the `Expr` or `Stmt` variant in `syntax.rs` and parse it
   in the method that owns the precedence level. The module comment states the precedence, lowest to
   highest: assignment, comparison, additive, multiplicative, power, range, unary prefix, postfix,
   primary. A new infix operator must slot into that chain, not bypass it. On a malformed form,
   return `LangError::at(message, self.current().position)` with a complete sentence.

4. **Evaluate it.** Add the match arm in `lovelace-lang/src/lib.rs` at `eval_expr_node` (expressions)
   or `exec_statement` (statements). Keep the per-statement cancellation check and the position
   publication intact. If the feature is a builtin, implement it in `builtins.rs` and register it as
   `add-builtin` describes; do not add a second evaluation path.

5. **Test the three layers.** In `lovelace-lang/src/suite_lex_parse_batch.rs`, assert the token
   payload/position, the parser shape (the existing `sexpr` helper renders a structural string) and
   the evaluation result through `Engine::eval` plus `format_typed`. Use concrete input and a
   concrete expected string:

   ```rust
   let mut engine = Engine::new();
   assert_eq!(engine.eval("1 != 2").unwrap().typed(), "True (Boolean)");
   ```

   Also test the refusal path with the exact message and position. If you create a new test file,
   declare it as a `mod` in `lovelace-lang/src/lib.rs`.

6. **Add the corpus record.** Append the documented behaviour to `parity/corpus/lang.json` with the
   next sequential ID and the exact `text` rendering. The corpus is the language gate, so the record
   is the parity fixture, not a duplicate test. Run it in release because the debug corpus binary
   overflows its stack at `lang/061` on this host.

7. **Check architecture and cross-library semantics.** A feature that touches numeric promotion,
   arithmetic or systems values must go through the canonical kernel. Read
   `docs/architecture-ownership.md`, run `python tools/check-architecture.py` and
   `cargo test -p lovelace-lang --test cross_library`. Do not introduce a Rational value kind or a
   domain-local arithmetic type as a side effect: those are separately specified decisions
   (RNL-MIG-005) and a second implementation fails the `architecture-policy` job.

8. **Update the wire fixtures only if the envelope moves.** A new statement kind, a new error code or
   a change in token positions changes the runner's positions and envelope. Edit the `.ls` input
   under `docs/csharp-oracle/Lovelace.Run.Tests/fixtures/`, then regenerate with
   `LOVELACE_RECORD_GOLDENS=1 cargo test -p lovelace-run`, inspect the `.json` diff, and run the test
   again **without** the variable. The recorder is inert unless the variable is set; never hand-edit
   a golden to make a comparison pass.

9. **Update the prose.** `docs/crates/lovelace-lang.md` for the public surface, `docs/glossary.md` for
   a new term, `docs/parity-with-studio.md` when the feature deliberately differs from the C#
   reference, and `docs/testing.md` if a new test layer is added.

## Verify

`<the test you added>` is the name of your new test; substitute it before running. Read each
`test result:` line — a filter that matches nothing prints `running 0 tests` and still exits 0, and a
corpus filter that selects no record prints `0 passed` and also exits 0.

```console
cargo test -p lovelace-lang suite_lex_parse_batch
cargo test -p lovelace-lang <the test you added>
cargo test -p lovelace-lang --test cross_library
python tools/check-architecture.py
cargo test -p lovelace-run golden_batch
cargo run --release -p lovelace-corpus -- lang/113
cargo run -p lovelace-run -- --eval "1 != 2" --text
```

Observable success: `suite_lex_parse_batch` and the focused feature test each report a nonzero passed
count with `0 failed`; `golden_batch` reports its nonzero count (8 tests today) with the environment
variable unset; the corpus filter reports `1 passed, 0 failed, 232 total (filter lang/113)`; the CLI
prints the exact typed value (`True (Boolean)` in the example) or the exact coded error with its
UTF-16 position.

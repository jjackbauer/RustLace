# Contributing

## Workflow

1. **Clone and build** (see [build-and-run.md](build-and-run.md)):

   ```console
   $ cargo build --workspace
   ```

2. **Branch** from `main`. A change lands through a branch and a pull request; `main` is what CI
   runs on (`push: branches: [main]` plus `pull_request` in `.github/workflows/ci.yml`).
3. **Make the change in the layer that owns it.** Behaviour belongs in `lovelace-lang` (or below);
   a host that re-implements a rule is a bug even when its output matches. The desktop drives the
   engine through `Evaluator`, never around it.
4. **Test at the layer you touched** and then run the gate:

   ```console
   $ cargo test -p lovelace-lang      # the crate you touched; a desktop change runs bare here too
   $ cargo test --workspace
   $ cargo run --release -p lovelace-corpus
   ```

   Read the `test result:` line, not only the exit status: a `cargo test -p <crate> <filter>` whose
   filter matches nothing prints `running 0 tests` and still exits 0, and a `lovelace-corpus` filter
   that selects no record prints `0 passed ... (filter ...)` and exits 0. A run with zero selected
   items is a failed check; require a nonzero passed count and `0 failed`.

5. **Format and lint** before opening the pull request:

   ```console
   $ cargo fmt --all
   $ cargo clippy --workspace --all-targets
   ```

   CI checks formatting with `--check` and runs clippy without `-D warnings`, because the workspace
   has pre-existing warnings; do not add new ones silently.

## Commit style observed in the history

The history mixes two consistent shapes, both imperative and evidence-bearing:

```text
Carry the Rust tests' out-of-tree fixtures and retarget their paths
Desktop v4: RUN-007 certified - cancellation parity with the web Studio reference (709 tests)
Desktop v4: DEF-011 fixed - cache settled before the terminal event (705 tests, release x2 + debug green)
style: apply cargo fmt across the workspace
test: carry the frozen knowledge graph reference and retarget its path
ci: add Windows full-suite and Linux engine-only GitHub Actions
```

What to copy from them:

- A subject in the imperative mood, under ~72 characters, naming **what changes**, not "fixes".
- An area prefix when it helps (`Desktop v4:`, `style:`, `test:`, `ci:`); a bare subject is also in
  the history.
- Evidence in the subject or body: the requirement id, the number of tests, the profiles exercised.
  "All tests pass" is not evidence; "705 tests, release ×2 + debug green" is.
- One logical change per commit. The desktop rounds committed the requirement, its evidence and the
  regenerated fixtures together.

## Review expectations

A reviewer should be able to answer these from the diff alone:

1. **Which layer changed, and which layer should have?** A UI change that formats a value is a
   finding, not a style preference.
2. **Is the contract still true?** Rendering, error sentences, the wire envelope, the statement
   boundary and the cache key are contracts. If one moves, the change says so and updates the
   fixture that pins it.
3. **Is the evidence attached?** Test names, counts, and the command that produced them. A
   screen-state change needs a captured frame or a headless check; a numeric change needs a corpus
   record or a parity test.
4. **Are the seams still inert by default?** `LOVELACE_*_FAULT`, `LOVELACE_LOG_FLOOD` and friends must
   never be reachable from a script or on by default.
5. **Is anything weaker?** Deleting a test, skipping one, relaxing an assertion, or dropping a
   fixture is a change to the contract and needs its own justification (the repository has one
   recorded residual of this kind, in `docs/rustlace-notes.md`).

## Documentation requirements

A documentation change is reviewed against the editorial guide,
[docs/editorial.md](editorial.md). Two rules are blocking:
every numerical claim says whether it is exact, precision-limited, error-bounded or parity-verified, and every example uses a
repository-relative path or a placeholder instead of a path from one machine. A link check does not
make a page ready to merge; the [editorial review checklist](editorial.md#review-checklist) does.

## Definition of done

A change is done when all of the following hold:

- It builds with `cargo build --workspace` and the desktop still builds with
  `cargo build --release -p lovelace-desktop`.
- The tests for its layer pass, and `cargo test --workspace` has the same pass/fail shape as before
  (the single known-ignored test stays the only ignore).
- The documentation corpus gate passes in release: 231 of 231.
- Any new behaviour is reachable from the real host surface — `lovelace-run` for the engine, the
  workbench or a headless mode for the desktop — and that path was exercised.
- The docs that describe the touched area were updated in the same change: `docs/architecture.md`
  for a layer or threading change, the crate page for a public surface, `docs/testing.md` for a test
  layer, an ADR for a decision a reader would otherwise have to reverse-engineer.
- No stale reference to the C# project survives in code comments or docs unless it explains a
  behaviour the port preserves (the comments are full of deliberate "the reference's …" citations;
  those are receipts, not leftovers).

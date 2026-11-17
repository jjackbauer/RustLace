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
   engine through `Evaluator`, never around it. Before adding a data type, numerical primitive,
   serializer or execution subsystem, read [`docs/architecture-ownership.md`](architecture-ownership.md)
   and identify the canonical owner. If the primitive exists, improve it there; a domain-local
   replacement needs an ADR and a documented exception, not a new type.
4. **Test at the layer you touched** and then run the gate:

   ```console
   $ cargo test -p lovelace-lang      # the crate you touched; a desktop change runs bare here too
   $ cargo test --workspace
   $ cargo run --release -p lovelace-corpus
   $ python tools/check-architecture.py
   $ python tools/test-architecture-gates.py
   $ cargo test -p lovelace-lang --test cross_library
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
6. **Is ownership preserved?** A new numerical type, arithmetic trait implementation or dependency
   edge needs a policy decision, not only a passing test. The `architecture-policy` job is a
   required check; a green unit-test suite cannot outvote a second rational implementation. Changes
   to the architecture policy, checker, mutation fixtures, `docs/architecture-ownership.md` or the
   rational kernel are declared in `.github/CODEOWNERS`, so the owner is requested even though
   GitHub's required code-owner review cannot be enabled while the repository has one collaborator,
   the PR author (DEC-002). An arithmetic impl whose target is empty or one of its generic
   parameters is reported as `review-required`; that is a review gate, not a pass, and the author
   classifies or narrows the impl instead of dismissing it. The reviewer asks the five questions in
   RNL-AGT-002: does it exist, is it suitable, can the shared
   implementation be improved, would a new one create competing semantics, and does an approved ADR
   authorize it?
7. **Do the surfaces still agree?** A change to exact arithmetic, promotion, conversion or systems
   values is checked through the kernel, Symbolics, Systems and the language with
   `cargo test -p lovelace-lang --test cross_library`, not only through the layer that changed.

## Required status checks

`main` moves only through the CI jobs named in `tools/architecture-policy.json`
(`requiredChecks.ciJobs`): `architecture-policy` (both OS variants), `numerical-kernel`, `windows`
(full workspace tests and the release corpus), `linux` and `docs` (both OS variants). `main`
protection requires the seven PR checks, strict up-to-date branches, `enforce_admins`, and it
disables force pushes and branch deletion. A negative-probe PR carrying a deliberately forbidden
Systems rational failed both architecture checks and its merge was refused. A job removed from
`.github/workflows/ci.yml` fails the architecture checker (`required-ci-job-missing`), so the claim
that a check is required is itself checked. `.github/CODEOWNERS` routes architecture-policy,
checker, mutation-fixture, ownership-document and rational-kernel changes to the owner; required
code-owner review is not enabled because the only collaborator is the PR author and GitHub does not
allow self-approval (DEC-002).

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
- `python tools/check-architecture.py` exits 0 and `python tools/test-architecture-gates.py`
  passes its 20 scenarios; a new crate is declared in `tools/architecture-policy.json` with its
  layer, responsibility, dependencies and reused abstractions, and a policy, checker or ownership
  change is declared in `.github/CODEOWNERS`.
- The documentation corpus gate passes in release: 231 of 231.
- Numerical changes also pass the `numerical-kernel` gate: the canonical-kernel tests, the seeded
  property campaigns, both independent Python oracles and the cross-library consistency tests.
- Any new behaviour is reachable from the real host surface — `lovelace-run` for the engine, the
  workbench or a headless mode for the desktop — and that path was exercised.
- The docs that describe the touched area were updated in the same change: `docs/architecture.md`
  for a layer or threading change, the crate page for a public surface, `docs/testing.md` for a test
  layer, an ADR for a decision a reader would otherwise have to reverse-engineer.
- No stale reference to the C# project survives in code comments or docs unless it explains a
  behaviour the port preserves (the comments are full of deliberate "the reference's …" citations;
  those are receipts, not leftovers).

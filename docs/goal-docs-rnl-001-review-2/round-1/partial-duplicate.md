# Partial duplicate kernel — second-review Finding 2 fix report

Repository: `F:\dev\RustLace`, branch `fix/release-provenance`, HEAD `428f3ba`
(`docs: finalize RNL-001 review harness state`). Host: Windows, Python 3.12.10.
Architecture policy schema: `schemaVersion` 1.

**Result:** pre-fix, the `partial-duplicate-kernel` fixture exited 0; after the change it exits 1
with `duplicate-numeric-representation`. Real workspace: 14 crates / 332 files / 0 violations.
Mutation suite: 21/21. Valid fixture: exit 0. No `permittedRepresentations` entry was added.

Files edited for this work:

- `tools/check-architecture.py`
- `tools/architecture-policy.json`
- `tools/architecture-fixtures/base/tools/architecture-policy.json`
- `tools/test-architecture-gates.py`
- this report

The working tree already contained `Cargo.lock` and `lovelace-rational/` changes from Finding 1
before this task started; they are not part of this change set. No Rust source or existing
hand-written documentation was edited for this finding, and nothing was committed.

All commands below were run from the repository root. Quoted blocks are the observed console
output, not a reconstruction.

## 1. Pre-fix: the new scenario passed when it should fail

The `partial-duplicate-kernel` scenario was added to `tools/test-architecture-gates.py` first
(`:282-296`) and the executed-scenario floor was raised to 21 (`:398`), without touching the checker
or either policy. The scenario appends this to the fixture's `lovelace-systems/src/lib.rs`:

```rust
use lovelace_int::Int;
use lovelace_nat::Nat;

pub struct RatioValue { pub n: Int, pub d: Nat }

impl RatioValue {
    pub fn add(&self, other: &RatioValue) -> RatioValue { other.clone() }
    pub fn div(&self, other: &RatioValue) -> RatioValue { other.clone() }
}
```

`n`/`d` are not in `numericFieldPairs`, and two arithmetic methods are below
`minimumArithmeticMethods` (4), so the pre-change checker reported nothing. Full pre-fix mutation
run:

```console
$ python tools/test-architecture-gates.py
PASS valid: exit 0, violations=0
PASS duplicate-rational: exit 1, duplicate-numeric-type
PASS duplicate-trait: exit 1, duplicate-arithmetic-trait
PASS undeclared-numeric: exit 1, duplicate-numeric-representation
PASS forbidden-dependency: exit 1, forbidden-dependency
PASS dependency-cycle: exit 1, dependency-cycle
PASS unauthorized-exception: exit 1, unauthorized-exception
PASS invalid-policy: exit 2, CONFIG ERROR
PASS empty-workspace: exit 2, CONFIG ERROR
PASS zero-scan: exit 2, CONFIG ERROR
PASS non-delegating-specialization: exit 1, specialization-not-delegating
PASS duplicate-in-owner-crate: exit 1, duplicate-numeric-type
PASS duplicate-rat-in-owner-crate: exit 1, duplicate-numeric-type
PASS duplicate-real-in-owner-crate: exit 1, duplicate-numeric-type
PASS split-impl: exit 1, duplicate-arithmetic-impl
PASS generic-arithmetic-trait: exit 1, duplicate-arithmetic-trait
PASS free-function-kernel: exit 1, free-function-kernel
PASS renamed-representation: exit 1, duplicate-numeric-representation
FAIL partial-duplicate-kernel: expected exit 1 and 'duplicate-numeric-representation', got 0
ARCH summary: crates=7 files=7 violations=0 waived=0
ARCH owners: natural->lovelace-nat, numeric-promotion->lovelace-lang, rational->lovelace-rational, real->lovelace-real, signed-integer->lovelace-int, symbolic-expression->lovelace-symbolics, systems-models->lovelace-systems
---
PASS broad-exception-scope: exit 1, unauthorized-exception
PASS unresolved-arithmetic-impl: exit 1, review-required
ARCH GATES FAILED: 1 scenario(s): partial-duplicate-kernel
```

`ARCH GATES FAILED` is printed on stderr; it is shown last for readability. The suite exited 1 only
because the new expectation failed; the fixture itself was accepted.

The isolated fixture command shows the wrong exit 0 directly:

```console
$ python tools/check-architecture.py --root .tmp/arch-gates/partial-duplicate-kernel --metadata .tmp/arch-gates/partial-duplicate-kernel/metadata.json
ARCH summary: crates=7 files=7 violations=0 waived=0
ARCH owners: natural->lovelace-nat, numeric-promotion->lovelace-lang, rational->lovelace-rational, real->lovelace-real, signed-integer->lovelace-int, symbolic-expression->lovelace-symbolics, systems-models->lovelace-systems
```
Exit code: **0** (observed).

At that point `numeric_field_match` required `arithmetic_method_count >= minimumArithmeticMethods`
(4) for the arbitrary-field-name branch, so a two-method duplicate could not reach it.

## 2. Policy and checker change

- `tools/architecture-policy.json:314` and
  `tools/architecture-fixtures/base/tools/architecture-policy.json:170` add
  `"minimumRepresentationMethods": 2` alongside the unchanged `"minimumArithmeticMethods": 4`.
- `tools/check-architecture.py:893-915`: `numeric_field_match` now flags a struct/union with at
  least two fields whose type is in `numericFieldTypes` when the type carries at least
  `minimumRepresentationMethods` (default 2) arithmetic methods summed over its `impl` blocks. The
  named `numericFieldPairs` branch is unchanged and remains conclusive: it returns true before the
  method count is consulted.
- The arithmetic-impl rule is untouched: `check_duplicates` still reads
  `minimumArithmeticMethods` (4) at `tools/check-architecture.py:945` and still requires the
  summed method total to reach it (`:1083`). The new key therefore does not lower the four-method
  threshold; the two rules now have separate thresholds for separate signals.
- Diagnostic wording is unchanged, so the new scenario fails with the existing named rule
  `duplicate-numeric-representation`.
- The suite's executed-scenario floor moved from 20 to 21, so the new scenario cannot disappear
  silently.

## 3. Final evidence

Real workspace:

```console
$ python tools/check-architecture.py
ARCH summary: crates=14 files=332 violations=0 waived=0
ARCH owners: array->lovelace-array, natural->lovelace-nat, numeric-promotion->lovelace-lang, rational->lovelace-rational, real->lovelace-real, signed-integer->lovelace-int, symbolic-expression->lovelace-symbolics, systems-models->lovelace-systems
```
Exit code: **0**.

Mutation suite, 21 scenarios:

```console
$ python tools/test-architecture-gates.py
PASS valid: exit 0, violations=0
PASS duplicate-rational: exit 1, duplicate-numeric-type
PASS duplicate-trait: exit 1, duplicate-arithmetic-trait
PASS undeclared-numeric: exit 1, duplicate-numeric-representation
PASS forbidden-dependency: exit 1, forbidden-dependency
PASS dependency-cycle: exit 1, dependency-cycle
PASS unauthorized-exception: exit 1, unauthorized-exception
PASS invalid-policy: exit 2, CONFIG ERROR
PASS empty-workspace: exit 2, CONFIG ERROR
PASS zero-scan: exit 2, CONFIG ERROR
PASS non-delegating-specialization: exit 1, specialization-not-delegating
PASS duplicate-in-owner-crate: exit 1, duplicate-numeric-type
PASS duplicate-rat-in-owner-crate: exit 1, duplicate-numeric-type
PASS duplicate-real-in-owner-crate: exit 1, duplicate-numeric-type
PASS split-impl: exit 1, duplicate-arithmetic-impl
PASS generic-arithmetic-trait: exit 1, duplicate-arithmetic-trait
PASS free-function-kernel: exit 1, free-function-kernel
PASS renamed-representation: exit 1, duplicate-numeric-representation
PASS partial-duplicate-kernel: exit 1, duplicate-numeric-representation
PASS broad-exception-scope: exit 1, unauthorized-exception
PASS unresolved-arithmetic-impl: exit 1, review-required
ARCH GATES PASSED: 21 scenarios
```
Exit code: **0**. That is the 20 pre-existing scenarios plus `partial-duplicate-kernel`; no existing
scenario was deleted, renamed, skipped or relaxed.

Valid fixture:

```console
$ python tools/check-architecture.py --root .tmp/arch-gates/valid --metadata .tmp/arch-gates/valid/metadata.json
ARCH summary: crates=7 files=7 violations=0 waived=0
ARCH owners: natural->lovelace-nat, numeric-promotion->lovelace-lang, rational->lovelace-rational, real->lovelace-real, signed-integer->lovelace-int, symbolic-expression->lovelace-symbolics, systems-models->lovelace-systems
```
Exit code: **0**.

`partial-duplicate-kernel` fixture:

```console
$ python tools/check-architecture.py --root .tmp/arch-gates/partial-duplicate-kernel --metadata .tmp/arch-gates/partial-duplicate-kernel/metadata.json
ARCH summary: crates=7 files=7 violations=1 waived=0
ARCH owners: natural->lovelace-nat, numeric-promotion->lovelace-lang, rational->lovelace-rational, real->lovelace-real, signed-integer->lovelace-int, symbolic-expression->lovelace-symbolics, systems-models->lovelace-systems
VIOLATION duplicate-numeric-representation lovelace-systems/src/lib.rs:22 crate 'lovelace-systems' declares 'RatioValue' with a numerator/denominator-like numeric representation
```
Exit code: **1**. The struct starts at line 22 because the fixture's `lovelace-systems/src/lib.rs`
is 17 lines before the append.

## 4. Real-workspace permits added

**None.** The stronger rule reports 0 violations on the real workspace (14 crates, 332 files), so no
legitimate existing type needed a `permittedRepresentations` entry. The allowlist is unchanged:
`Complex` for `lovelace-lang`, `Fixed` for `lovelace-real` in the real policy, and `Complex` in the
base fixture policy. No whole-crate exemption was added or used.

## 5. Not verified / out of scope

- CI was not run and nothing was committed (both outside/forbidden for this task); the evidence above
  is local only.
- Mutations were run against the fixture tree at HEAD `428f3ba` plus this diff, not against a clean
  checkout of another commit.
- Several documents still state that the mutation suite has 20 scenarios: `AGENTS.md`,
  `CONTRACT.md`, `docs/architecture-ownership.md`, `docs/adr/011-shared-rational-kernel.md`,
  `docs/testing.md`, `docs/contributing.md` and `docs/rnl-001-evidence.md`. Updating them is outside
  this task's scope; they need a follow-up documentation sync to say 21.
- No focused unit tests were added for `numeric_field_match` boundary cases (one numeric field,
  one arithmetic method, named pair with zero methods). The behaviour is covered indirectly by
  `duplicate-rational`, `split-impl`, `undeclared-numeric`, `renamed-representation` and the real
  workspace/valid-fixture runs.
- No standalone struct inventory was produced for the real workspace; the "no legitimate type is
  newly flagged" conclusion rests on the checker's 0-violation run over all 332 files.

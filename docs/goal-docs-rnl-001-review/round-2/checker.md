# Architecture checker — round 2 fix report

Repository: `F:\dev\RustLace`, branch `fix/release-provenance`, base commit `8ec1aaf`
(`ci: let the architecture checker resolve metadata on a cold runner`). Host: Windows, Python 3.12.10.
Checker policy schema: `schemaVersion` 1.

Scope of this change set (no file outside it was edited for this work):

- `tools/check-architecture.py`
- `tools/architecture-policy.json`
- `tools/architecture-fixtures/base/**` (policy, metadata, `lovelace-lang`, new `lovelace-real`)
- `tools/test-architecture-gates.py`
- this report

All commands below were run from the repository root. Every quoted block is the observed console
output, not a reconstruction.

## 1. Test-first: the new scenarios passed before the checker was changed

The nine new mutation scenarios were added to `tools/test-architecture-gates.py` first
(`:205-317`). At that moment `tools/check-architecture.py` was still the base-commit file, and the
base fixture policy still used `duplicateDetection.ownerCrates`; it was aligned to the shipped
policy's whole-crate list (`lovelace-nat`, `lovelace-int`, `lovelace-rational`, `lovelace-real`,
`lovelace-lang`) so the fixture reproduced the shipped exemption under test. Every one of the eleven
existing scenarios still passed. Command and observed output:

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
FAIL duplicate-in-owner-crate: expected exit 1 and 'duplicate-numeric-type', got 0
ARCH summary: crates=7 files=7 violations=0 waived=0
---
FAIL duplicate-rat-in-owner-crate: expected exit 1 and 'duplicate-numeric-type', got 0
ARCH summary: crates=7 files=7 violations=0 waived=0
---
FAIL duplicate-real-in-owner-crate: expected exit 1 and 'duplicate-numeric-type', got 0
ARCH summary: crates=7 files=7 violations=0 waived=0
---
FAIL split-impl: expected exit 1 and 'duplicate-arithmetic-impl', got 1
ARCH summary: crates=7 files=7 violations=1 waived=0
VIOLATION duplicate-numeric-representation lovelace-systems/src/lib.rs:22 crate 'lovelace-systems' declares 'SplitPair' with a numerator/denominator-like numeric representation
---
FAIL generic-arithmetic-trait: expected exit 1 and 'duplicate-arithmetic-trait', got 0
ARCH summary: crates=7 files=7 violations=0 waived=0
---
FAIL free-function-kernel: expected exit 1 and 'free-function-kernel', got 0
ARCH summary: crates=7 files=7 violations=0 waived=0
---
FAIL renamed-representation: expected exit 1 and 'duplicate-numeric-representation', got 0
ARCH summary: crates=7 files=7 violations=0 waived=0
---
FAIL broad-exception-scope: expected exit 1 and 'unauthorized-exception', got 0
ARCH summary: crates=7 files=7 violations=0 waived=1
ARCH exception: EX-BROAD-1
ARCH waived duplicate-numeric-type lovelace-systems/src/lib.rs by EX-BROAD-1 (docs/adr/999-exception-scope-probe.md)
---
FAIL unresolved-arithmetic-impl: expected exit 1 and 'review-required', got 0
ARCH summary: crates=7 files=7 violations=0 waived=0
---
ARCH GATES FAILED: 9 scenario(s): duplicate-in-owner-crate, duplicate-rat-in-owner-crate, duplicate-real-in-owner-crate, split-impl, generic-arithmetic-trait, free-function-kernel, renamed-representation, broad-exception-scope, unresolved-arithmetic-impl
```

Eight scenarios exited 0 (the wrong pass); `split-impl` exited 1 only for
`duplicate-numeric-representation`, i.e. the named `duplicate-arithmetic-impl` rule still did not
exist. After the fix the same nine scenarios all exit 1 with the named rule (section 4). (The
`ARCH GATES FAILED` summary is printed on stderr; the block above keeps the stdout lines in program
order, with that summary last for readability.)

The pre-fix capture is reproducible against base-commit `8ec1aaf` plus the fixture state described
above; the final tree retires `ownerCrates`, and the new checker rejects that key rather than
honouring it:

```console
$ python tools/check-architecture.py --root .tmp/round2/legacy-owner-probe --metadata .tmp/round2/legacy-owner-probe/metadata.json
ARCH CONFIG ERROR: duplicateDetection.ownerCrates is retired; a whole-crate exemption is not allowed. Declare per-type ownership in 'reservedTypeNames' and explicit local exceptions in 'permittedRepresentations'.
exit 2
```

## 2. Policy: exact keys

`tools/architecture-policy.json:218-256` now carries:

| Key | Shape | Value in the shipped policy |
|---|---|---|
| `reservedTypeNames` | name → owning crate | `Nat`→`lovelace-nat`, `Int`→`lovelace-int`, `Rat`/`Rational`/`Fraction`/`BigRational`/`BigFraction`/`Frac`/`Q`→`lovelace-rational`, `Real`→`lovelace-real`, `NdArray`→`lovelace-array` (`:219-230`) |
| `permittedRepresentations` | crate → allowed local type names | `lovelace-lang`: `Complex`; `lovelace-real`: `Fixed` (`:232-239`) |
| `freeFunctionPatterns` | regex list | `^(?:rat\|rational\|fraction\|frac\|q)_(?:add\|sub\|mul\|div\|rem\|neg\|abs\|gcd\|lcm\|reduce\|normalize\|reciprocal)$` (`:249-251`) |
| `freeFunctionOwners` | crate list | `lovelace-rational` (`:252-254`) |

`duplicateDetection.ownerCrates` is removed. It is also rejected by
`check_policy_shape` (`tools/check-architecture.py:249-255`), so a policy cannot silently keep a
whole-crate exemption. `forbiddenTypeNames`, `numericFieldPairs`, `numericFieldTypes`,
`arithmeticMethods`, `arithmeticTraits`, `minimumArithmeticMethods`, `permittedSpecializations`
and `permittedAliases` are retained; `check_policy_shape` now requires every `forbiddenTypeNames`
entry to have an owner in `reservedTypeNames` so the two lists cannot drift
(`tools/check-architecture.py:216-300`).

The base fixture policy uses the same schema at
`tools/architecture-fixtures/base/tools/architecture-policy.json:99-135` (reserved names
`:100-112`, permitted representations `:113-118`, free-function keys `:127-133`);
`minimumWorkspaceCrates` is 7 (`:189`) because the fixture now also carries the
`lovelace-real` crate (`tools/architecture-fixtures/base/metadata.json:5,10`).

### Why each permitted representation exists (discovered by running the real checker)

`permittedRepresentations` is a per-type allowlist; it is not a crate exemption. Two entries are
needed by the real workspace, and both were found by running the checker, not assumed:

1. `Fixed` in `lovelace-real`. Without the permit the checker exits 1 with eight violations, all
   from the real kernel's internal fixed-point carrier:

   ```console
   $ python tools/check-architecture.py          # before adding the lovelace-real permit
   ARCH summary: crates=14 files=332 violations=8 waived=0
   VIOLATION duplicate-numeric-representation lovelace-real/src/lreal.rs:98 crate 'lovelace-real' declares 'Fixed' with a numerator/denominator-like numeric representation
   VIOLATION duplicate-arithmetic-impl lovelace-real/src/lreal.rs:114 crate 'lovelace-real' implements 7 scalar arithmetic methods on 'Fixed' outside the canonical owners
   ... (six more duplicate-arithmetic-impl lines at lreal.rs:120,717,723,725,731,737)
   ```

   `lovelace-real` owns the real abstraction and `Fixed` is its private working representation
   (`lovelace-real/src/lreal.rs:98`), so a type-specific permit is the abstraction-specific answer.
2. `Complex` in `lovelace-lang`. With only the `Fixed` permit in place the checker exits 1 with
   seven violations from the language's complex value (`lovelace-lang/src/complex.rs:66`,
   `lovelace-lang/src/value.rs:15`, plus five `duplicate-arithmetic-impl` lines):

   ```console
   ARCH summary: crates=14 files=332 violations=7 waived=0
   VIOLATION duplicate-numeric-representation lovelace-lang/src/complex.rs:66 ...
   VIOLATION duplicate-arithmetic-impl lovelace-lang/src/complex.rs:71 crate 'lovelace-lang' implements 8 scalar arithmetic methods on 'Complex' outside the canonical owners
   ... (the remaining arithmetic-impl lines are complex.rs:292,298,300 and value.rs:22;
        value.rs:15 is the second duplicate-numeric-representation)
   ```

   `Complex` is built on the canonical `Real` and is the language's value type; the permit is
   scoped to the name, so `Rat` declared in `lovelace-lang` is still rejected (section 4).

## 3. Checker functions changed

| Function | State | Purpose |
|---|---|---|
| `check_policy_shape` `:216` | extended | requires the new keys, rejects `ownerCrates` (`:249`), validates regexes and the `forbiddenTypeNames` ↔ `reservedTypeNames` relation |
| `split_top_level` `:597` | new | split generic-parameter lists outside `<>()[]{}` |
| `split_impl_generics` `:616` | new | strip a leading `impl<...>` parameter list and return its parameter names (skips `->` inside bounds) |
| `split_impl_header` `:641` | fixed | splits trait/type only after the generics are stripped; a `for<'a>` binder is not a separator |
| `type_name_of` `:670` | fixed | resolves `Wrapper<T>`, `&Wrapper<T>`, `&'a mut Wrapper<T>` and `crate::Wrapper<T>` to `Wrapper` instead of `T` |
| `trait_name_of` `:681` | fixed | strips a leading higher-ranked `for<...>` binder and generic arguments; `std::ops::Add` → `Add` |
| `free_functions` `:711` | new | collects `fn` items that are not inside an `impl`/`trait` body (module and function nesting still counts as free) |
| `analyze_file` `:758` | extended | calls `split_impl_generics` (`:833`), stores `trait_text`/`generic_parameters`, returns free functions (`:858`) |
| `scan_crate_sources` `:861` | extended | returns `(files, declarations, impls, free_functions)` |
| `unresolved_arithmetic_impl` `:915` | new | review gate for arithmetic-trait headers whose target is empty or a generic parameter |
| `check_duplicates` `:932` | rewritten | no crate skip; reserved-name ownership (`:956-1012`), canonical-owner allowance, summed method counts (`:1025-1029`), permitted representations, specialization delegation |
| `check_free_functions` `:1096` | new | flags a matching free function outside `freeFunctionOwners` |
| `validate_exceptions` `:1130` | fixed | rejects every scope entry containing `*`, `?`, `[` or `]` (`:1153`) |
| `run` `:1216` | extended | passes free functions to both checks (`:1329-1330`) |

`check_duplicates` no longer contains any `continue` keyed on a crate being an owner; the only
skips are per item: the one canonical declaration of a reserved name in its owning crate, a
`(crate, type)` named in `permittedRepresentations`, and an impl whose target type is not declared
in that crate. A second declaration of a reserved name inside its owner is also rejected
(`:970-995`): the owner may declare the canonical type once.

## 4. New mutation scenarios: command and observed diagnostic

`python tools/test-architecture-gates.py` generates each fixture as `.tmp/arch-gates/<name>` and
leaves it in place. Each command below is the real checker over that generated fixture, with the
recorded metadata. The gate run itself reports `ARCH GATES PASSED: 20 scenarios`.

| Scenario | Mutation | Command (from the repository root) | Observed |
|---|---|---|---|
| `duplicate-in-owner-crate` | `Rat { num: Int, den: Nat }` + four methods appended to `lovelace-lang`, a numeric owner for promotion | `python tools/check-architecture.py --root .tmp/arch-gates/duplicate-in-owner-crate --metadata .tmp/arch-gates/duplicate-in-owner-crate/metadata.json` | exit 1, `duplicate-numeric-type lovelace-lang/src/lib.rs:23 ... belongs to lovelace-rational` (plus representation and arithmetic-impl) |
| `duplicate-rat-in-owner-crate` | same `Rat` appended to `lovelace-real`, owner of `Real` | `python tools/check-architecture.py --root .tmp/arch-gates/duplicate-rat-in-owner-crate --metadata .tmp/arch-gates/duplicate-rat-in-owner-crate/metadata.json` | exit 1, `duplicate-numeric-type lovelace-real/src/lib.rs:10 ... belongs to lovelace-rational` |
| `duplicate-real-in-owner-crate` | a second `pub struct Real` appended to `lovelace-real` | `python tools/check-architecture.py --root .tmp/arch-gates/duplicate-real-in-owner-crate --metadata .tmp/arch-gates/duplicate-real-in-owner-crate/metadata.json` | exit 1, `duplicate-numeric-type lovelace-real/src/lib.rs:7 crate 'lovelace-real' declares reserved numeric type 'Real' more than once; abstraction-specific ownership allows the canonical declaration once` |
| `split-impl` | `SplitPair { num: Int, den: Nat }` with `add`/`sub` in one `impl` and `mul`/`div` in a second | `python tools/check-architecture.py --root .tmp/arch-gates/split-impl --metadata .tmp/arch-gates/split-impl/metadata.json` | exit 1, `duplicate-arithmetic-impl ...:24 ... implements 4 scalar arithmetic methods` and `...:29` (summed count, not per-block 2) |
| `generic-arithmetic-trait` | `impl<T> std::ops::Add for Wrapper<T>` and `impl<T> std::ops::Add for &Wrapper<T>` | `python tools/check-architecture.py --root .tmp/arch-gates/generic-arithmetic-trait --metadata .tmp/arch-gates/generic-arithmetic-trait/metadata.json` | exit 1, `duplicate-arithmetic-trait ...:21 ... for 'Wrapper'` and `...:26 ... for 'Wrapper'` |
| `free-function-kernel` | free `rat_add`, `fraction_mul`, `rational_div`, `q_gcd` in `lovelace-systems` | `python tools/check-architecture.py --root .tmp/arch-gates/free-function-kernel --metadata .tmp/arch-gates/free-function-kernel/metadata.json` | exit 1, four `free-function-kernel` lines at `lovelace-systems/src/lib.rs:19,20,21,22` |
| `renamed-representation` | `Quantity { left: Int, right: Nat }` with two arithmetic methods per `impl` block | `python tools/check-architecture.py --root .tmp/arch-gates/renamed-representation --metadata .tmp/arch-gates/renamed-representation/metadata.json` | exit 1, `duplicate-numeric-representation lovelace-systems/src/lib.rs:22` (plus two arithmetic-impl lines) |
| `broad-exception-scope` | complete, otherwise-valid exception with `scope: ["lovelace-*"]` and an ADR, waiving a `Rat` violation | `python tools/check-architecture.py --root .tmp/arch-gates/broad-exception-scope --metadata .tmp/arch-gates/broad-exception-scope/metadata.json` | exit 1, `unauthorized-exception ... scope entries must be exact paths or crate names, not wildcard patterns ('*', '?', '[' or ']'): lovelace-*`; the `Rat` violation is not waived |
| `unresolved-arithmetic-impl` | `impl<T> Add for T` in `lovelace-systems` | `python tools/check-architecture.py --root .tmp/arch-gates/unresolved-arithmetic-impl --metadata .tmp/arch-gates/unresolved-arithmetic-impl/metadata.json` | exit 1, `review-required ...:21 ... the impl target 'T' is one of the impl's generic parameters` |

The wildcard rejection is not limited to `*`; a scratch fixture with
`scope: ["lovelace-?", "lovelace-[ab]", "lovelace-]"]` produced:

```console
exit 1
VIOLATION unauthorized-exception tools/architecture-policy.json exceptions[0] is not a valid architecture exception: scope entries must be exact paths or crate names, not wildcard patterns ('*', '?', '[' or ']'): lovelace-?, lovelace-[ab], lovelace-]
```

### The valid fixture still passes and keeps its legal forms

```console
$ python tools/check-architecture.py --root tools/architecture-fixtures/base --metadata tools/architecture-fixtures/base/metadata.json
ARCH summary: crates=7 files=7 violations=0 waived=0
```

It contains the four required legal forms plus the new permitted representation:

- domain formula: `utilization` (`tools/architecture-fixtures/base/lovelace-systems/src/lib.rs:17`)
  and `demand` (`.../lovelace-lang/src/lib.rs:7`);
- typed unit: `ServiceTime` (`.../lovelace-systems/src/lib.rs:5`);
- canonical alias: `pub use lovelace_rational::Rat as Q;` (`.../lovelace-systems/src/lib.rs:1`);
- permitted specialization: `impl Add for ServiceTime` delegating through `self.0.add(...)`
  (`.../lovelace-systems/src/lib.rs:12-15`), declared at
  `tools/architecture-fixtures/base/tools/architecture-policy.json:170-176`;
- permitted representation: `Complex` (`.../lovelace-lang/src/lib.rs:11-18`), declared at
  `tools/architecture-fixtures/base/tools/architecture-policy.json:113-118`.

The eleven pre-existing scenarios are unchanged in name, mutation and expected rule
(`tools/test-architecture-gates.py:319-334`); the suite minimum was raised from 11 to 20
(`:377`), so deleting a scenario fails the gate.

## 5. Real workspace and free-function false-positive proof

```console
$ python tools/check-architecture.py
ARCH summary: crates=14 files=332 violations=0 waived=0
ARCH owners: array->lovelace-array, natural->lovelace-nat, numeric-promotion->lovelace-lang, rational->lovelace-rational, real->lovelace-real, signed-integer->lovelace-int, symbolic-expression->lovelace-symbolics, systems-models->lovelace-systems
```

The free-function detector was run over every real-workspace source file with the shipped policy:

```console
patterns: ['^(?:rat|rational|fraction|frac|q)_(?:add|sub|mul|div|rem|neg|abs|gcd|lcm|reduce|normalize|reciprocal)$']
owners: ['lovelace-rational']
control rat_add -> True
control fraction_mul -> True
control rational_div -> True
control q_gcd -> True
control rational_pow -> False
free functions detected in real workspace: 9475
free functions matching kernel patterns: 0
```

The `rational_pow` control matters: the real workspace has one free helper named
`lovelace-symbolics/src/lib.rs:1323 fn rational_pow`, which delegates to `Rat::pow_i64`. An earlier
candidate pattern that included `pow` flagged it, a false positive; the shipped pattern therefore
covers the kernel operations `add/sub/mul/div/rem/neg/abs/gcd/lcm/reduce/normalize/reciprocal`
and excludes `pow`. A competing `rat_pow` free kernel is consequently not caught by name — see
residual risks.

Final mutation run, all 20 scenarios (11 pre-existing + 9 new):

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
PASS broad-exception-scope: exit 1, unauthorized-exception
PASS unresolved-arithmetic-impl: exit 1, review-required
ARCH GATES PASSED: 20 scenarios
```

Final mutation count: **20 scenarios** (11 pre-existing + 9 new), all passing.

## 6. What I could NOT verify

- No Rust was compiled or tested: the checker is a text scanner, the fixture trees are not compiled,
  and Rust/build/CI execution is outside this change set. The real-workspace claim is a scan result,
  not a `cargo test` result.
- Macro-generated `impl` blocks and free functions, `include!`-included files, and impls the
  text scanner cannot parse are outside the scan. The new `review-required` rule covers only the
  arithmetic-trait headers the parser can see (empty target or generic-parameter target).
- The free-function pattern is name-based and intentionally narrow (see the `rational_pow` note);
  `rat_pow`-style kernels and non-rational free kernels are not detected.
- `undeclared-numeric-alias` still checks `forbiddenTypeNames` (the rational names), not every
  `reservedTypeNames` entry: a `type Real = ...` alias outside `lovelace-real` is not flagged by
  that rule. Declarations and arithmetic impls of `Real` are flagged.
- `permittedRepresentations` approves a type name wholly for a crate; it cannot express "this type
  may implement only delegating arithmetic". Both shipped entries were justified by their real
  declarations, but a future permitted type could hide duplication.
- The `?`/`[`/`]` scope rejection was verified with a scratch fixture, not a committed scenario;
  only `*` has a committed mutation scenario (`broad-exception-scope`), per the brief.
- Only the Windows/Python 3.12.10 host was exercised. The Linux CI job was not run here.

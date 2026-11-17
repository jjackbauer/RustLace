# ADR-011: one shared rational kernel

**Status:** accepted; normative under RNL-001.

## Context

Before this decision the workspace carried two independent rational implementations:

- `lovelace-symbolics::Rat` — a numerator/denominator pair reduced only when both parts fit `u64`,
  and whose arithmetic built `Int` denominators through decimal strings;
- `lovelace-systems::Q` — a second numerator/denominator pair that reduced with `Nat::gcd` and was
  introduced because the Symbolics type's cost profile was wrong for the MVA recurrence.

The duplication was not a style question. It produced two canonical forms, two division sign paths
(the Symbolics `div` multiplied by the denominator and then negated the result, which is easy to get
wrong for a negative divisor), two answers to "is this rational reduced?", and no single place to add
a resource limit, a conversion contract or an exactness report. A fix in one implementation did not
reach the other. The former Systems rational module (deleted by RNL-MIG-002) contained the fallbacks
`unwrap_or_else(Q::zero)` after a constructor a caller could not make fail, which turned an internal
invariant violation into a plausible-looking zero.

## Decision

Introduce `lovelace-rational` as the **only** general-purpose rational implementation in the
workspace, depending only on `lovelace-nat` and `lovelace-int`.

- `lovelace-symbolics` re-exports the canonical `Rat` (`pub use lovelace_rational::Rat;`) and owns
  symbolic semantics, not arithmetic.
- `lovelace-systems` publishes `Q` as a documented alias of the same type (`pub use
  lovelace_rational::Rat as Q;`) and deletes the former Systems rational module. The alias is type-identical; it is
  not a wrapper and has no methods of its own.
- `lovelace-real` depends on `lovelace-rational` and owns the `Rat`↔`Real` conversions, because it
  is the lowest crate that can see both representations.
- `lovelace-lang` continues to own numeric promotion and dispatch, but obtains every rational value
  from the canonical kernel.

The directed graph is enforced in `tools/architecture-policy.json` and verified with `cargo
metadata` by `tools/check-architecture.py`:

```text
lovelace-nat
    ↓
lovelace-int
    ↓
lovelace-rational
    ├── lovelace-real
    ├── lovelace-symbolics
    └── lovelace-systems
             ↓
        lovelace-lang
             ↓
       execution hosts
```

`lovelace-int` still depends on `lovelace-nat`; consumers do not have to depend on every
intermediate crate. The arrow expresses permitted direction and ownership, not a required edge.

## Canonical representation and API

A `Rat` always has a strictly positive denominator, the sign solely in the numerator, zero as
`0/1`, and numerator/denominator divided by their full arbitrary-precision `Nat::gcd`. Equal
fractions have identical representations.

The public surface is:

- constructors `zero`, `one`, `from_i64`, `from_u64`, `from_nat`, `from_int`, and the fallible
  `from_parts(Int, Nat) -> Result<Rat, RationalError>`; no public constructor can produce an
  invalid rational;
- `add`, `sub`, `mul`, `neg`, `abs`, `add_with`/`sub_with`/`mul_with` (growth-budgeted), `cmp`;
- `div -> Option<Rat>` for source compatibility plus `checked_div -> Result<Rat, RationalError>`,
  and `checked_div_with`;
- `pow_i64 -> Option<Rat>` plus `checked_pow(i64, &Limits, &dyn Cancel)`, exponentiation by
  squaring, with explicit handling of `0`, negative exponents and `i64::MIN`;
- `numerator`, `denominator`, `is_zero`, `is_one`, `is_negative`, `is_positive`, `is_integer`,
  `to_i64`, `to_u64`, `to_int_exact`, `decimal_digits` (a binary-length upper bound, never a
  decimal-string round trip), `bit_length`;
- `provenance::{ComputationExactness, InputFidelity, RepresentationExactness,
  ExactnessProvenance, Conversion}`.

Arithmetic uses `Int`/`Nat` only. Multiplication and division cross-cancel common factors before
forming the products. Addition uses the least common multiple, not the product, of the
denominators. A caller can pass `Limits::digits`/`Limits::bits` and a `Cancel` token; a budget
failure returns `RationalError::DigitBudgetExceeded`/`BitBudgetExceeded` and never a partially
computed success.

## Alternatives considered

1. **Keep `lovelace-systems::Q` and migrate Symbolics to it.** Rejected because `lovelace-symbolics`
   would then depend on a domain library, reversing the required dependency direction.
2. **Keep `lovelace-symbolics::Rat` and fix its normalization.** Rejected because the real
   constraints (resource limits, conversions to `Real`, exactness provenance, checked division) are
   numeric-kernel concerns, not symbolic-algebra concerns. Fixing it in place would leave the
   systems crate depending on symbolic expressions for arithmetic.
3. **A third crate that feature-gates both existing types.** Rejected: it preserves two canonical
   forms indefinitely and makes `Rat` equality depend on a build feature.
4. **Newtype wrappers (`Q(Rat)`, `Rat(Nat, Int)`) in the consumers.** Rejected for `Q`: a wrapper
   is a place for a second set of arithmetic methods to appear. `Q` is an alias, and the checker
   rejects a wrapper that implements arithmetic traits without an approved specialization.
5. **Introduce a `Rational` language value kind.** Not part of this decision. RNL-MIG-005 requires
   mixed-type promotion rules to be specified separately; this ADR keeps the existing
   Natural → Integer → Real policy and exposes exact systems values with a structured sidecar
   instead.

## Exactness and precision implications

The four claims that "exact" used to collapse are now separate:

1. **Computation exactness** — whether the algorithm approximated.
2. **Input fidelity** — whether the inputs had already been truncated.
3. **Representation exactness** — whether the returned representation carries the value exactly.
4. **Render precision** — how many digits were used to print it.

`Real::from_rational(&Rat, max_digits)` returns a `Conversion<Real>` whose `lossless` flag is false
when the expansion exceeded the budget; the `Real` is marked inexact. `Real::to_rat()` returns a
`Conversion<Rat>` that is representation-exact but reports `input: truncated` for a previously
truncated value. Converting a truncated `Real` back to a rational therefore does not pretend to
restore the lost precision. The `sys_*` adapter refuses a precision-limited `Real` as an exact model
input with `InvalidArgument`/`TypeMismatch` instead of promoting its stored digits.

Systems results retain exact values across the language and wire boundaries through an additive
`exact_values` record on each numeric record (see `docs/extensions/systems.md`). The decimal fields
remain `Real` projections; `exact_values.<metric>` is a record with `numerator` (Integer) and
`denominator` (Natural). This is the structured numerator/denominator strategy from RNL-NUM-011. It
is additive: existing field reads and the wire envelope keys are unchanged, so `protocolVersion`
stays at 2; record consumers already ignore unknown fields. The compatibility test is
`lovelace-run`'s `sys_exact_values_are_additive_and_protocol_compatible`, which asserts the
envelope version and both the projected and exact fields on the wire.

## Performance rationale

The old Symbolics type avoided full reduction for large operands; that is exactly the case an exact
MVA recurrence reaches. Full arbitrary-precision GCD is more expensive per operation but bounds
intermediate digit growth, and the kernel adds cross-cancellation in multiplication/division and LCM
addition to reduce how often that GCD is large. The benchmark harness (`lovelace-bench`) records
workload sizes, host, compiler, profile, repetitions and exact-output digests, so a performance claim
is measured rather than inferred from implementation structure. This decision does not claim the new
kernel is faster in every workload; it claims there is one implementation whose performance can be
measured and improved in one place.

## Migration strategy

- **M0** — policy, ownership document, ADR, checker and mutation suite.
- **M1** — `lovelace-rational`, Symbolics re-export, deletion of the Systems rational module, conversion
  APIs in `lovelace-real`, regression tests.
- **M2** — signed division regression, MVA `n - X*Z` occupancy, sweep grid endpoint semantics,
  removal of silent zero fallbacks, exactness sidecar.
- **M3** — CI jobs, agent/contributor instructions, documentation, negative tests.
- **M4** — cross-cancellation, benchmark comparison and regression analysis.

The release is atomic: no release retains two rational kernels. The APIs that moved are recorded as
intentional contract corrections: `Rat::from_parts` now returns a `Result`, `Real::from_rational`
replaces `Q::to_real`, and precision-limited `Real` inputs to `sys_*` are refused rather than
silently rounded into exact rationals.

## Consequences and limitations

- The canonical kernel is a foundational dependency; a defect in it reaches every consumer. That is
  the point: one place to test, limit and fix.
- Exact rationals can still grow; `Limits` and the systems solver budgets bound growth, and a budget
  failure is an error, not a truncated value.
- Systems and Symbolics now share one type, so a change to `Rat`'s display or ordering is visible on
  both surfaces; the cross-library tests compare them.
- The `Q` alias remains in the systems API for source compatibility. Removing it would be a separate
  versioned API change; retaining it does not retain a second implementation.
- The architecture checker is heuristic at the declaration level. It is deliberately paired with
  `cargo metadata` for the dependency graph and with mutation fixtures that prove each rule can
  fail; it is not a proof assistant. `docs/architecture-ownership.md` defines the review procedure
  for a case the syntax scan cannot decide.

## Post-review correction (2026-11-17)

The independent review of PR #9 confirmed the decision above and required three corrections that do
not change it. They are recorded here rather than by rewriting the original decision.

- **Model transport is exact, not projected.** `sys_station` returns a `Station`, and `sys_open` and
  `sys_closed` return a `QueueNetwork`; both now carry an additive `exact_values` record for their
  numerical parameters (service time, visits and service demand; or arrival rate and think time).
  The language adapter's `parse_station` and `parse_network` read `exact_values` first and fall back
  to the decimal field only when the whole sidecar is absent. A present but malformed sidecar is
  refused instead of falling back. `1/97` therefore survives `sys_station → sys_open → sys_analyze`
  and `sys_station → sys_closed → sys_mva` with denominator 97; both end-to-end regressions are in
  `lovelace-lang`.
- **Ownership is abstraction-specific.** `duplicateDetection.ownerCrates` is retired and rejected
  by the checker. `reservedTypeNames` names the owner of each foundational type, and
  `permittedRepresentations` allows only `Complex` in `lovelace-lang` and `Fixed` in
  `lovelace-real`. An owner crate may declare its canonical type once; a second `Rat` in
  `lovelace-lang` or `lovelace-real`, or a second `Real` in `lovelace-real`, is a violation. The
  mutation suite has 20 scenarios (11 original + 9 new), including split `impl`, generic arithmetic
  trait, free-function kernel, renamed representation and broad exception scope; an arithmetic impl
  whose target is empty or a generic parameter is reported as `review-required`.
- **Code-owner review is declared, not enforced.** `.github/CODEOWNERS` declares ownership of the
  policy, checker, mutation fixtures, this ADR, the ownership document and the rational kernel.
  Required code-owner review is not enabled because the repository has one collaborator, the author
  of PR #9, and GitHub does not allow self-approval; that is the user-approved limitation DEC-002.
  Required status checks are enforced independently: all seven PR checks are required on `main`,
  with strict up-to-date branches, `enforce_admins`, and force pushes and deletions disabled.

The original decision, its alternatives and its consequences stand unchanged.

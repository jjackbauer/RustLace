# Architecture ownership

This document is normative. It defines which crate owns each foundational numerical abstraction, who
may consume it, which dependency directions are legal, and how an exception is granted. The
machine-readable form is [`tools/architecture-policy.json`](../tools/architecture-policy.json); the
checker [`tools/check-architecture.py`](../tools/check-architecture.py) interprets that policy and
runs in CI as the `architecture-policy` job. Ownership is abstraction-specific: the policy names each
reserved type and the crate that owns it, not a list of crates exempt from the rule. A statement here
that the checker cannot decide is a review criterion, not a suggestion.

## 1. Ownership matrix

| Abstraction | Owner | Authorized consumers |
|---|---|---|
| Natural integers | `lovelace-nat` | every crate that needs limb arithmetic |
| Signed integers | `lovelace-int` | numeric, domain and language crates |
| Exact rationals | `lovelace-rational` | `lovelace-real`, `lovelace-symbolics`, `lovelace-systems`, `lovelace-know`, `lovelace-lang`, `lovelace-bench` |
| Arbitrary-precision real representation | `lovelace-real` | `lovelace-lang`, execution hosts, `lovelace-systems` via the language only |
| Symbolic expressions | `lovelace-symbolics` | `lovelace-lang`, `lovelace-corpus`, `lovelace-desktop` |
| Queueing and systems models | `lovelace-systems` | `lovelace-lang`, `lovelace-bench` |
| Language numeric promotion and dispatch | `lovelace-lang` | execution hosts |
| Generic arrays | `lovelace-array` | `lovelace-lang` |

Exactly one crate owns each abstraction, and the policy expresses that per type, not per crate.
`reservedTypeNames` maps every foundational type name to its owner — `Nat`, `Int`, the rational names
(`Rat`, `Rational`, `Fraction`, `BigRational`, `BigFraction`, `Frac`, `Q`), `Real` and `NdArray`.
`permittedRepresentations` names the only local representations a crate may declare: `Complex` in
`lovelace-lang` and `Fixed` in `lovelace-real`. The retired `duplicateDetection.ownerCrates` key is
rejected, so no crate is exempt as a whole: `lovelace-rational` may declare the canonical `Rat` once
and still fails on a second `Rat`, and a numeric owner such as `lovelace-real` fails on a `Rat` or a
duplicate `Real`. A domain crate may define a formula, a model, a unit or an algorithm; it may not
define a second general-purpose arithmetic type.

## 2. Permitted dependency direction

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

The arrows are permissions, not requirements: a consumer does not have to depend on every
intermediate crate. The rules the checker enforces are:

1. No dependency cycle, including dev-dependencies.
2. No edge that the policy's `allowedEdges` does not list.
3. No dependency from a lower layer to a higher layer (`layers` in the policy).
4. No numerical primitive depends on a scientific domain library.
5. No domain library depends on `lovelace-lang` to perform arithmetic.
6. No third-party package enters an engine crate. The desktop's GUI stack is the only exemption and
   is listed per package in `thirdPartyAllowed`.
7. Every workspace crate is declared with its layer, responsibility, permitted dependencies and the
   existing abstractions it reuses (`crates` in the policy). A new crate without a declaration is a
   failure, not a default.

`cargo metadata --no-deps` is the source of truth for the graph. The diagrams in this file and
`docs/architecture.md` are explanatory; CI verifies the real graph rather than trusting them.

## 3. Forbidden duplication

The following are violations even when every test passes and the arithmetic is correct:

- a second numerator/denominator type for an abstraction whose reserved name is owned elsewhere,
  including a second copy inside the owning crate;
- a `type` alias that renames a competing numeric type;
- a general-purpose `add`/`sub`/`mul`/`div`/`neg` implementation on a numeric representation
  outside the owning crate;
- an implementation of `Add`, `Sub`, `Mul`, `Div`, `Rem`, `Neg` or an assign variant for a domain
  type without an approved specialization;
- arithmetic on decimal strings where native `Nat`/`Int` operations exist;
- a silent fallback (`unwrap_or_else(...::zero)`, an empty result, a fabricated success) after a
  failed fundamental arithmetic operation.

Three declaration shapes need care. A numeric implementation split across two `impl` blocks is
counted as one implementation: method counts are summed, not measured per block. An arithmetic trait
implemented for a generic wrapper is resolved to the wrapper type, and an impl whose target is empty
or one of its own generic parameters cannot be classified; that header is reported as
`review-required` rather than silently passing. A free function that implements a kernel operation
(`rat_add`, `fraction_mul`, `rational_div`, `q_gcd`-style names) is checked against
`freeFunctionPatterns` and may only appear in a crate listed in `freeFunctionOwners`
(`lovelace-rational`).

The checker inspects declarations, field shapes, inherent methods, free functions and trait
implementations. It does not reject arithmetic expressions, formulas or typed units.

## 4. Legitimate domain types

A domain-specific wrapper is allowed when it delegates numerical operations to a canonical
primitive. Examples: `ArrivalRate`, `ServiceTime`, `Population`, `Probability`, `Rate`, `Time`,
`Demand`. A wrapper:

- stores one canonical numeric value (or a documented, validated pair of integers for a
  domain-specific identity);
- implements only domain conversion and validation itself;
- calls the canonical type for arithmetic, or is listed in `permittedSpecializations` with a
  delegating implementation.

Formulas, recurrences, conservation checks, service-demand calculations and model validation remain
in the domain crate. The checker must not reject `utilization = arrival * service_time / servers`
merely because it uses multiplication and division.

A specialization entry in the policy has the form:

```json
{ "crate": "lovelace-systems", "type": "ServiceTime", "reason": "typed unit wrapper; arithmetic delegates to Rat" }
```

The checker verifies that the implementing body delegates through `self.0`, `self.value` or
`self.inner`. A permitted type whose implementation does not delegate is a violation
(`specialization-not-delegating`).

## 5. Exception procedure

An architecture exception is an architectural change, not a way to make a check green. It requires:

1. an ADR under `docs/adr/` recording the conflict, the alternatives and why the shared abstraction
   cannot be improved instead;
2. a `rationale` of at least reviewable length;
3. a named `reviewer`;
4. a `scope` that is a narrow list of exact paths or crate names; the checker rejects every entry
   containing `*`, `?`, `[` or `]`, so a wildcard waiver cannot be granted;
5. a `removalPlan` describing when and how the temporary duplication disappears;
6. a corresponding entry in `exceptions` in `tools/architecture-policy.json`.

The checker rejects an exception missing any of those (`unauthorized-exception`), and it rejects a
wildcard scope even when the rest of the entry is valid. Suppressing a failing rule, deleting a test,
weakening an assertion, or editing an owner list without an ADR is not an exception; it is the failure
the policy exists to prevent.

## 6. New-crate declaration

Every new workspace crate adds an entry to `crates` with:

- `layer` — one of the policy's layers;
- `responsibility` — what the crate owns;
- `reuses` — the existing abstractions it consumes;
- the permitted edges in `allowedEdges`.

A new scientific crate must explicitly identify its numerical-kernel dependency
(`lovelace-rational`, `lovelace-real` or both). A new crate that adds a numerical representation
must instead improve the owner and cite the ADR that authorizes any exception.

## 7. Enforcement and evidence

```console
python tools/check-architecture.py
python tools/check-architecture.py --json
python tools/check-architecture.py --report target/architecture-report.md
python tools/test-architecture-gates.py
```

The checker exits `2` when it cannot perform the scan (missing policy, unparsable policy, no
workspace, no source files, missing required crate or file). It exits `1` on a violation. It never
reports an empty scan as success. On the real workspace it reports `crates=14 files=332 violations=0`,
and the valid fixture reports `crates=7 files=7 violations=0`.

The mutation suite (`tools/test-architecture-gates.py`) has **20 scenarios**: the 11 original rules
plus nine added for the review remediation — a duplicate `Rat` in a numeric-owner crate
(`lovelace-lang` and `lovelace-real`), a second `Real` in `lovelace-real`, a split `impl`, a generic
arithmetic trait, a free-function kernel, a renamed representation, a broad exception scope, and an
unclassifiable arithmetic impl. Each scenario injects the forbidden change into an isolated fixture
and requires the checker to fail with the named rule; the valid fixture contains domain formulas,
typed units, a canonical alias, a delegating specialization and the permitted `Complex`
representation. A scenario that stops failing fails the suite, and the suite minimum is 20 so
deleting one is itself a failure.

`main` protection requires the seven PR checks: both `architecture-policy` matrix legs,
`numerical-kernel`, `windows` (workspace tests and corpus), `linux`, and both `docs` matrix legs.
Protection also requires branches to be up to date before merging, applies to administrators
(`enforce_admins`), and disables force pushes and branch deletion. A negative-probe PR #10 carrying a
deliberately forbidden Systems rational failed both architecture checks, and `gh pr merge` was refused
by the base-branch policy; a failing architecture check therefore blocks the merge even when every
application test passes.

`.github/CODEOWNERS` declares the owner for `tools/architecture-policy.json`,
`tools/check-architecture.py`, `tools/test-architecture-gates.py`, the architecture fixtures, this
document, ADR-011, `lovelace-rational/`, the rational and systems parity oracles, and
`.github/workflows/ci.yml`. Required code-owner review is **not** enabled: the repository has one
collaborator, who is the PR author, and GitHub does not allow self-approval. That is the user-approved
limitation DEC-002; the status checks above are enforced independently, so the ownership declaration
does not make the author's PR unmergeable.

## 8. Agent and contributor obligations

Before implementing a primitive, serializer or execution subsystem, an agent must inspect the
workspace for the existing abstraction and record its reuse decision (RNL-AGT-001/002). A
performance concern alone does not justify duplication: improve the canonical primitive or present
an ADR (RNL-AGT-003). The agent skills `change-numeric-kernel`, `add-builtin`,
`add-language-feature` and the static-family guidance in `docs/extending.md` require the architecture
checker and the cross-library numerical tests before completion.

## 9. Periodic architecture audit

Local pull-request enforcement sees one diff; architectural drift can emerge across several. At each
major capability milestone, run the audit and attach the generated report to the release evidence:

```console
python tools/check-architecture.py --report architecture-impact.md
python tools/check-architecture.py --json
cargo test --workspace
cargo test -p lovelace-lang --test cross_library
```

The reviewer looks for duplicated representations that the syntax scan cannot classify (a
hand-rolled GCD in a domain algorithm, a parallel execution path, a public contract that exists in
two places), cross-layer dependencies introduced through a helper crate, and policy exceptions that
were never removed. A finding becomes an ADR and a follow-up change; deleting the check or granting
an exception in the same change is not an audit remediation.

## 10. Architecture impact reporting

`python tools/check-architecture.py --report <path>` writes the summary required for a pull request
that adds or modifies a crate, a numeric type, a foundational API or a dependency edge: the scanned
crate/file counts, the canonical owners, the allowed edges and every violation or waiver. The
`architecture-policy` CI job uploads it per OS. The machine-readable form (`--json`) is the same
data for a tool.

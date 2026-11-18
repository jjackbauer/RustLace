# Agent harness: contracts and verification

This page separates **implemented checks** from the intended agent-harness
architecture. It is a requirements document, not a claim that an agent runtime,
task scheduler, session store or confidence estimator already exists in RustLace.

## Why an agent needs more than a green snapshot

An executable documentation example demonstrates that the *current implementation*
produces the output recorded next to it. It does not prove that the recorded
output is mathematically correct. A registry descriptor may also be stale while
the function and its example agree, and an agent may follow an obsolete skill.

A contract therefore has several independent sources of evidence:

1. **Public API:** live names, arity, parameter names and possible return kinds.
2. **Runtime:** typed values, structured errors, limits and wire behavior.
3. **Mathematics:** externally derived answers, algebraic invariants, properties,
   domain restrictions and, where relevant, validated error bounds.
4. **Architecture:** canonical ownership and permitted dependency edges.
5. **Operations:** current, executable agent skills and CI verification commands.

Two observations derived from the same implementation are not independent
mathematical evidence. C# parity is behavioral evidence, not a mathematical
oracle, especially when the historical behavior was defective.

## Implemented in this change

`python tools/harness/check-agent-contracts.py`:

- Reads `BuiltinInfo`'s current fields from `lovelace-lang/src/registry.rs`.
- Checks that every Core descriptor exists; no plugin loader is introduced.
- Checks actual `.dsh/skills/*/SKILL.md` names, required frontmatter and
  Verify sections, and the `AGENTS.md` / `docs/skills.md` inventories.
- Refuses the retired `plugin: None` / `plugin: Some(...)` instructions
  and unknown registry fields described by the `add-builtin` skill.
- Executes representative core calls through the built `lovelace-run`, asserting
  that their *actual* return kinds are within the advertised registry union.
- Independently calculates the rank of small exact integer matrices using
  Python `fractions.Fraction`, then compares it with the live public
  `matrix_rank` result. Square, rectangular, singular and multiply dependent
  rows are included.

`python tools/harness/test-agent-contracts.py` mutates skill references,
the skill inventory and registry ownership metadata, and requires every mutant
to produce a named rejection. A successful tool exit with zero selected cases
is not sufficient evidence. Both checks run in the documentation CI matrix on
Linux and Windows.

**Current limit:** these representative calls are *not* a complete behavioral
matrix for all core functions or all registered builtins. Tests for language syntax,
source positions, cancellation, the numerical kernel and the existing
documentation corpus remain independently necessary.

## Commands

From the workspace root, after compiling the runner:

```console
cargo build -p lovelace-run
python tools/harness/check-agent-contracts.py
python tools/harness/test-agent-contracts.py
```

The first check reports `AGENTCONTRACT skills=... core=<live-count> executed=... failed=0`.
The mutation check reports `AGENTCONTRACTMUTATION total=... passed=... failed=0`.
The counts are observed data, not hardcoded evidence of mathematical correctness.
`--static-only` is available for quick validation of skill and registry
structure, but it **does not certify runtime behavior**.

## Proposed next phases (not yet implemented)

### Phase A: machine-readable contracts

Define stable contract IDs for every builtin and language construct.
Existing registry descriptors remain the authority for names, arity and
discovery. Separate versioned fixtures describe accepted input kinds, outputs
conditional on argument kinds, stable refusals, precision classification,
algebraic identities and which independent oracle is authoritative.

Generate a coverage report listing **registered**, **contracted**, **executed**,
**independently checked**, **skipped** and **failed**, without collapsing them
into one misleading percentage. Require every new/changed public return branch
to be represented by a live typed execution.

### Phase B: impact-directed test selection

Build a change-impact graph from Cargo metadata, the canonical ownership policy,
the registry, syntax nodes, test references, documentation and agent skills.
Changed symbols select focused checks first, but risk policy still requires
cross-library and full CI checks before merge. Missing ownership is a
`review-required` event, not an empty test selection.

### Phase C: independently generated evidence receipts

The harness—not the implementation agent—records the base/head commit, exact
command and executable, fixture digest, OS, toolchain, exit code, selected and
executed case counts, skipped cases, invariant and oracle results, and
remaining unverified claims. Receipts are invalid if the head SHA changes.
An agent cannot certify its own output by editing expected snapshots.

### Phase D: risk and policy governance

- Documentation-only: links, snippet execution and semantic claim checks.
- Public API / language feature: typed contracts, parser properties, source
  positions, existing parity corpus, and downstream compatibility.
- Exact arithmetic / symbolic solvers / optimization: independently checked
  identities, boundary and adversarial cases, canonical ownership, and
  cross-platform behavior.
- Verifier, policy or oracle changes: mutation tests **and independent review**;
  do not allow the implementing agent to waive its own gate.

Preserve `lovelace-rational` as the only general-purpose rational kernel.
The contract runner and its oracle must not become a second production numeric
implementation or dispatch layer.

### Phase E: long-lived agent quality improvement

Record task attempts, objective outcomes, failing-first regression evidence,
post-merge defects, downstream metrics, token/cost use and elapsed work.
On a schedule or on a measured decline in productivity or increase in failure
rate, an agent may review its own sessions and **propose** improved skills or
methods. Validate proposals on frozen historical tasks and independent gates
before adoption. No automated self-improvement cycle may remove coverage,
weaken a numeric oracle or alter merge policy without human review.

A nominal confidence target (for example 95%) is not an evidence certificate.
Calibrate any probability against measured defect outcomes; otherwise publish
explicit verified/unverified claims and the evidence that supports each one.

## Known compatibility boundary: symbolic matrix rank

This change replaces the numeric `matrix_rank` determinant shortcut with exact
elimination through the canonical `Rat` kernel. Matrices with free symbols
still take the *legacy generic estimate*, not a pointwise conditional rank
certificate. For example, the rank of `[[x]]` depends on whether `x = 0`.
A future condition-aware API must define assumptions and result shape before
the legacy symbolic branch can be retired. Until then, do not describe the
symbolic estimate as an unconditional exact rank.

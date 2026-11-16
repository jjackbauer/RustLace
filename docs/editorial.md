# Editorial guide

This is the voice and structure contract for RustLace documentation. It applies to every human
contributor and every coding agent that writes a page, a comment, a skill or a release note. It is
enforced in review, not by a linter: a checker can prove a link resolves, and it cannot prove a claim
is true.

## Voice

RustLace should read like it was written by an engineer who understands the mathematics, respects the
reader's intelligence, and is comfortable admitting a limitation.

- **Lead with the reader's problem**, not with the file that implements it. A page about eigenvalues
  opens with the computation, not with `builtins.rs`.
- **Prefer plain statements to defensive ones.** "The Linux build uses eframe's `glow` backend" is
  better than three paragraphs explaining why that is not a regression.
- **Give evidence once, where it belongs.** A test count belongs in the release note or
  `docs/testing.md`, not repeated in the README, the crate page and the ADR.
- **Let confidence come from evidence.** No superlatives, no "fully", "completely", "nothing is" —
  unless the sentence would still be true for every input.
- **Historical context is local.** Mention the C# implementation when it explains a design decision or
  a contract. Never require the reader to know LovelaceSharp to use RustLace.

## The three kinds of claim

Every numerical statement must say which of these it is. Conflating them is the most common defect in
this repository, and it has already produced one wrong result presented as evidence.

| Kind | Meaning | Example |
|---|---|---|
| **Exact** | The value is the mathematically correct result, exactly representable. | `fft([0,1,0,0])` → `[1, -1i, -1, 1i]`; `1/3` → `0.(3)` |
| **Precision-bounded** | The value is correct to the stated budget; the error is bounded by it. | `sqrt(2)` at 30 places; `pi` |
| **Parity** | The value reproduces a reference implementation, including its defects. | `movingavg([2.5], 3)` → `0.(8)` |

Rules:

- "Exact 30-place decimal" is a contradiction and must not appear. Say "computed to 30 places" or
  "exact" — not both.
- "Nothing is floating point" is a statement about the representation. It is not a correctness
  guarantee, and it may not be used as one.
- When a documented behaviour is a reproduced defect, say so in the same paragraph, and link the ADR
  that records it. Do not present the observed output as the expected output.
- An `inexact` marker is provenance, not an error bound. Do not use it to excuse a wrong leading
  digit.

Automated wording and link checks can verify consistency, but they
cannot establish mathematical correctness. A numerical claim needs an independent oracle, a
reference calculation, or a stated error bound; a checker that confirms a sentence was written or a
link resolves is not evidence that the value it states is right.

## Structure

New pages follow one of four shapes, after Diátaxis:

| Shape | Reader question | Where it goes |
|---|---|---|
| **Tutorial** | "Teach me by doing." | a new `docs/tutorials/` page (target shape; none exist yet) |
| **How-to** | "How do I do this one thing?" | a new `docs/how-to/` page (target shape; none exist yet) |
| **Reference** | "What exactly does this do?" | `docs/language/`, `docs/crates/`, `docs/extensions/` |
| **Explanation** | "Why is it built this way?" | `docs/architecture.md`, `docs/adr/` |

A tutorial contains, in this order: the problem, the runnable source, the expected output, an
interpretation of that output, the assumptions, the precision considerations, and one command that
reproduces it. A reference page for a builtin contains: purpose, inputs and their accepted kinds,
output, precision behaviour, errors, limitations, and at least one example with its observed typed
result.

## Rules for claims and evidence

1. **A measurement names its version and host.** Test counts, timings and platform claims are
   observations of a commit, not properties of the project. Write "6135 tests on v0.4.1"; do not write
   "6135 tests".
2. **A claim of support names its evidence.** "Supported on Linux" requires a built-and-run artifact.
   A headless selftest is a run-path check, not a GUI journey pass, and the two must not be described
   interchangeably.
3. **Do not hand-edit a machine-read document as if it were prose.** `docs/goal-desktop-v4/platform-matrix.md`
   is parsed by `--platform-claims` and by tests that assert exactly one supported row. Changing a
   status cell is a code change.
4. **Prefer symbols to line numbers.** Cite `dsp_movingavg` or the function's doc comment, not a
   `file:line` citation such as `builtins.rs:NNN`, which rots on the next refactor. A line number is
   acceptable only when the line itself is the subject, and then it is commit-pinned.
5. **Portable paths in examples.** No `C:\Users\<someone>\...`. Use a repository-relative path or a
   placeholder that reads as one.
6. **`.ls` is the script extension.** Use it in every example; the compatibility note for other extensions lives in the language reference.

## Agent instructions are code

`AGENTS.md`, `.github/copilot-instructions.md` and `.dsh/skills/**` are executable operational inputs.
An agent will follow them literally. A stale instruction is not a documentation wart; it is a defect
that changes what the project does. In particular:

- An instruction that tells an agent to skip, omit or refuse a supported platform is a P0 defect.
- An instruction that describes a build or release step must match `.github/workflows/` exactly.
- When behaviour changes, the agent instructions that describe it change in the same commit.

## Review checklist

Before opening a documentation change:

- [ ] Does the opening sentence describe what the reader can do, not how the code is organised?
- [ ] Is every numerical claim labelled exact, precision-bounded or parity?
- [ ] Is each measurement tied to a version or commit?
- [ ] Do all relative links resolve, and do they point at a current page rather than a historical one?
- [ ] If behaviour changed, did the platform matrix, the agent instructions and the README change too?
- [ ] Is any historical material explicitly marked as historical?
- [ ] Would an agent following the changed instructions do the right thing?

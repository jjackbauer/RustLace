# Goal — RustLace documentation and editorial remediation

**Slug:** `goal-docs-v1`
**Opened:** 2026-11-15
**Repository:** `C:\Users\ricar\dev\RustLace` (git `bb9d21a`, version 0.4.1)
**Toolchain:** WSL Ubuntu, `/home/ricardo/.cargo/bin/cargo` 1.100.0; repo at `/mnt/c/Users/ricar/dev/RustLace`.
Native Windows `cargo` is absent — every build/test claim must be produced in WSL.

## Goal (one sentence)

RustLace's current documentation and agent instructions satisfy DOC-001 through DOC-015: every
named contradiction between documents resolves to one provable source of truth, every numerical
claim is labelled exact / precision-bounded / parity, and four named tutorials execute with
reproduced output.

## Verbatim anchor — the user's request

The user submitted a documentation review and invoked `/stretch-goal-harness`. The mechanisms the
review names, quoted verbatim from its requirements table:

> | DOC-001 | P0 | **Resolve active/historical platform conflicts.** Every current installation, build, release, and agent-facing document agrees with the supported-platform matrix. |
> | DOC-002 | P0 | **Audit exactness claims.** Numerical docs distinguish mathematical exactness, precision-bounded approximations, and implementation representations. |
> | DOC-003 | P0 | **Investigate the `movingavg` discrepancy.** A reference mismatch may not be presented as mathematical correctness. Add a mathematical oracle independent of C# parity. |
> | DOC-004 | P0 | **Synchronize agent instructions.** `AGENTS.md`, Copilot instructions, and `.dsh/skills` may not contradict current build and release behavior. |
> | DOC-005 | P1 | **Rewrite the README around user value.** A newcomer can identify capabilities, limitations, installation instructions, and a runnable example without reading internal design material. |
> | DOC-006 | P1 | **Provide complete tutorials.** Publish at least four independently executable mathematical workflows with interpreted results. |
> | DOC-007 | P1 | **Gate documentation examples in CI.** Execute language and extension examples on relevant platforms; changes to code or examples must fail when output drifts unexpectedly. |
> | DOC-008 | P1 | **Create an explicit archival boundary.** Old parity specifications and their missing capture assets are visibly historical and not presented as current platform evidence. |
> | DOC-009 | P1 | **Establish a canonical script convention.** Document `.ls` and `.lv` behavior and consistently prefer one extension across introductory material. |
> | DOC-010 | P1 | **Version technical evidence.** Tests, benchmarks, platform claims, and capability counts refer to a release or commit and can be regenerated. |
> | DOC-011 | P2 | **Write and enforce the editorial guide.** Documentation PRs follow common rules for tone, terminology, structure, example output, and claim strength. |
> | DOC-012 | P2 | **Validate live links and code references.** Broken links fail CI in current docs; archived evidence uses a separate, declared policy. |
> | DOC-013 | P2 | **Remove personal environment details from examples.** Use portable paths and isolate provenance details in archival metadata. |
> | DOC-014 | P2 | **Standardize reference pages.** Public APIs document purpose, inputs, outputs, precision behavior, errors, limitations, and examples in a consistent order. |
> | DOC-015 | P2 | **Avoid brittle source line references.** Prefer stable symbols and source links, or commit-pinned links when a historical line is important. |

And the review's governing sentence:

> **The implementation history explains how the software came to exist. The documentation should explain how to use it correctly, what it guarantees, and where those guarantees end.**

## Mechanism ledger — every named mechanism is KEPT

No mechanism is replaced, narrowed, or removed. Each row names the artifact that will carry it.

| Mechanism | Verdict | Carrying artifact |
|---|---|---|
| DOC-001 platform conflicts | KEPT | `docs/adr/008-linux-gui-support.md`, the six named documents, and `platform_claims` code + tests |
| DOC-002 exactness claims | KEPT | `docs/extensions/dsp.md`, `docs/adr/004`, `README.md`, `docs/editorial.md` |
| DOC-003 `movingavg` oracle | KEPT | `lovelace-lang/src/dsp_round20.rs` oracle test + `docs/adr/004` correction |
| DOC-004 agent instructions | KEPT | `AGENTS.md`, `.github/copilot-instructions.md`, `.dsh/skills/**`, `docs/skills.md` |
| DOC-005 README | KEPT | `README.md` |
| DOC-006 four tutorials | KEPT | `docs/tutorials/` — linear algebra, symbolic, signal processing, reproducible computing |
| DOC-007 CI example gating | KEPT | `.github/workflows/ci.yml` + the two example runners |
| DOC-008 archival boundary | KEPT | `docs/goal-desktop-v4/platform-matrix.md`, `docs/goal-desktop-v4/journeys.md`, `docs/README.md` |
| DOC-009 `.ls` convention | KEPT | `docs/language/README.md`, `docs/build-and-run.md`, `docs/extending.md`, `docs/crates/**` |
| DOC-010 versioned evidence | KEPT | `README.md`, `docs/testing.md`, release notes |
| DOC-011 editorial guide | KEPT | `docs/editorial.md` (written) + enforcement points |
| DOC-012 link validation | KEPT | a link-check script + CI step |
| DOC-013 personal paths | KEPT | `docs/language/plots.md` and every other example |
| DOC-014 reference pages | KEPT | a documented template + conformance check over the public pages |
| DOC-015 line references | KEPT | symbol-based citations across `docs/**` |
| Documentation-sync agent skill ("identify dependent docs when modifying an API or behavior … check for semantic drift, not merely confirm that a Markdown file was edited") | KEPT | `.dsh/skills/sync-docs/SKILL.md` + `docs/skills.md` index + `docs/editorial.md` rule 3 |
| Automated documentation CI pipeline (structural validation → example execution → contract verification → consistency checks → published report) | KEPT | one `docs` job in `.github/workflows/ci.yml` running `tools/check-docs.ps1` plus both example runners, in the review's order |
| Epistemic limit: consistency checks cannot establish mathematical correctness | KEPT | `docs/editorial.md` §"The three kinds of claim" + `docs/testing.md` + the D4 oracle, which is the only thing allowed to certify a number |

## Definition of done — falsifiable checks

Every dimension has a command and an expected observable. `met` requires the observable to be
reproduced by the orchestrator, not reported by an agent.

| # | Dimension | Check command | Expected observable |
|---|---|---|---|
| D1 | DOC-001 | `grep -rn "Windows-only\|must not be built\|native Windows IDE" docs README.md CONTRACT.md .dsh .github` | zero hits in current-support context (historical ADRs exempt and labelled) |
| D2 | DOC-001 | `cargo test -p lovelace-desktop platform_claims` (WSL) | exit 0 with the matrix declaring both Windows and Linux supported |
| D3 | DOC-002 | `grep -rn "exact 30-place\|Nothing is floating point" docs README.md` | zero hits |
| D4 | DOC-003 | `cargo test -p lovelace-lang movingavg` | exit 0; the oracle test is present and passing |
| D5 | DOC-004 | `grep -rn "must not be built or tested on Linux\|only under \`cfg(windows)\`" .dsh .github AGENTS.md docs` excluding `docs/goal-docs-v1/` (harness memory) and `docs/adr/008-linux-gui-support.md` (which quotes the removed sentence to condemn it) | zero hits |
| D6 | DOC-005 | run the README's first command from a clean shell | prints `0.(3)`; README states capability, limitation, install, example above the crate table |
| D7 | DOC-006 | each tutorial's own verification command, in WSL | exit 0 and output equals the recorded block |
| D8 | DOC-007 | `grep -n "run-examples" .github/workflows/ci.yml` | both runners invoked; a drifted example fails the job |
| D9 | DOC-008 | `grep -n "Historical snapshot" docs/goal-desktop-v4/journeys.md`; `grep -n "current support statement" docs/goal-desktop-v4/platform-matrix.md` | `journeys.md` is marked historical; `platform-matrix.md` is marked as the current support statement. The matrix is **not** historical: Round 1 promoted it, so demanding a historical marker there would be a false claim |
| D10 | DOC-009 | `grep -rn "\.lv\b" docs README.md` classified into *usage* vs *documentation* | `.lv` documented exactly once (canonical `.ls`, `.lv` accepted for compatibility) and used in **zero** example command lines |
| D11 | DOC-010 | `grep -n "v0\.4\.1" README.md docs/testing.md` | every count is version-qualified |
| D12 | DOC-011 | `test -f docs/editorial.md`; `grep -n "editorial" AGENTS.md docs/contributing.md`; the docs job must fail on a missing taxonomy | the file exists, is a stated review requirement in `AGENTS.md` and the contributing guide, and the enforcement path (checklist + docs job) is named and runnable |
| D13 | DOC-012 | `bash tools/check-docs.sh` | exit 0: every relative link resolves **and** every path-shaped code reference names a file that exists; `docs/goal-desktop-v4/` exempt under the declared archival policy |
| D18 | review §6 automated gates | `grep -n "name: docs" .github/workflows/ci.yml` | a single `docs` job runs structural validation, then example execution, then contract/consistency checks, then publishes a summary; a drift fails the workflow |
| D19 | review §6 epistemic limit | `grep -n "cannot establish mathematical correctness" docs/editorial.md docs/testing.md` | the limit is stated in both, and no tutorial certifies a number with a wording or link check — only D4's oracle may |
| D14 | DOC-013 | `grep -rn "C:\\\\Users" docs README.md` excluding `docs/goal-docs-v1/` (harness memory records the path it is repairing) | zero hits in documentation |
| D15 | DOC-014 | `bash tools/check-reference-pages.sh` | every page listed in `docs/reference-pages.md` carries the seven required sections in the required order |
| D16 | DOC-015 | `bash tools/check-docs.sh --symbols` | no `.rs:NNN` in current guidance, and every symbol named in a doc resolves in the source tree (`grep -w`) |
| D17 | review §6 "documentation-specific agent skill" | `test -f .dsh/skills/sync-docs/SKILL.md && grep -qi "dependent" .dsh/skills/sync-docs/SKILL.md` | the skill exists, names the dependent-doc map, and asks for semantic drift rather than file modification |

## Hard constraints (gates)

| ID | Constraint | Provenance |
|---|---|---|
| C1 | Never weaken, delete, `#[ignore]`, or relax a test or fixture to reach green | repo (`AGENTS.md` guardrails) |
| C2 | Never invent capability; a headless selftest is not a GUI journey pass | repo |
| C3 | Engine crates keep zero external dependencies (ADR-001) | repo |
| C4 | Changing `movingavg` arithmetic is a **product decision**; DOC-003 asks for the oracle and the claim audit, not a silent behaviour change | user (DOC-003 wording: "may not be presented as", "add a mathematical oracle") |
| C5 | Build/test evidence must be produced in WSL; no unverified "tests pass" claim | agent (environment) |
| C6 | No `.gitignore` exists; build outputs must never be committed | repo + OBS |

## Stop criteria

1. All 19 checks D1–D19 reproduce with their expected observable, **and**
2. `cargo test -p lovelace-lang -p lovelace-real -p lovelace-symbolics` exits 0 in WSL, **and**
3. no gate G1–G5 is failing at close.

Round cap: 8. A cap reached before the criteria is a *reported shortfall*, never convergence.

## Deliberate non-goals

- Repositioning RustLace as a MATLAB-compatible product. The review explicitly rejects this.
- Deleting migration provenance. DOC-008 asks for a boundary, not a purge.
- Rewriting the 124-entry builtin catalogue. DOC-014 asks for a consistent reference shape; the
  interpretation of "standardize" is a scope question for the user (see `intent-audit.md`).

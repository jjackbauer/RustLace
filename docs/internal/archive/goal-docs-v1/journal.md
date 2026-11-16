# Goal Journal — goal-docs-v1

> Append-only. See the stretch-goal-harness evidence protocol for entry format.

---

<!-- New entries go below this line -->

### OBS-001: The release skill instructed agents to skip the Linux build

- **Source**: `.dsh/skills/release-and-publish/SKILL.md:138` (pre-fix)
- **Fact**: The skill stated "The desktop crate is Windows-only and must not be built or tested on Linux", while `.github/workflows/release.yml:30` publishes `lovelace-desktop-linux-x86_64`.
- **Implications**: An agent following the skill could delete the Linux release gate. Highest-severity finding of the review.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: EVD-012, ART-001

### OBS-002: `dsp_movingavg` substitutes a rendered string for a correct quotient

- **Source**: `lovelace-lang/src/builtins.rs:5820-5835`, `lovelace-lang/src/builtins.rs:5701-5777`
- **Fact**: Below 38 digits `dsp_movingavg` computes the exact `Real::div` quotient, then, when `dsp_lreal128_division_render` produces a different string, replaces the quotient with `Real::from_str(rendered)`. The renderer anchors the recurring period one position early when the numerator's decimal exponent is negative.
- **Implications**: `movingavg` is not merely documented wrongly; it returns a wrong value with exact provenance.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: EVD-002, EVD-004, HYP-001

### HYP-001: The `movingavg` result below 38 digits is a reproduced reference defect reported as exact

- **Claim**: `movingavg([2.5], 3)` returns 8/9 flagged `exact:true`, where the mathematically correct result is 5/6 and the engine computes 5/6 correctly for the same division outside `movingavg`.
- **Supporting OBS**: OBS-002
- **Why it matters**: If true, DOC-003 is satisfied only by an explicit correction plus an oracle; a documentation rewrite alone would leave a wrong value published as exact.
- **Falsification strategy**: Look for any input where the fixed-width path returns the correct value for a fractional sample (which would make it a rounding rather than a structural error); check whether the `exact` flag is ever set false for a non-periodic sample; attempt `movingavg([2.5], 3)` at 37 and 38 to confirm the threshold behaviour.
- **Status**: Supported
- **Confidence**: High

### VAL-001: Falsification attempt on HYP-001

- **Target**: HYP-001
- **Method**: Ran the three precision cases, the `0.1` analogue, and the `1/3` control; extracted `"exact"` flags mechanically with `grep`/`uniq -c`.
- **Evidence examined**: `wsl lovelace-run --eval "movingavg([2.5], 3)" --text` → `[0.(8)]`; `--eval "2.5/3"` → `0.8(3)` with numerator 5 denominator 6 and `"exact":true`; `--eval "movingavg([0.1], 3)"` → `[0.(0)]`; flag tallies `2 "exact":true` for both wrong results and `2 "exact":false` for the correct `[1/3]` case.
- **Result**: Supported
- **Conclusion**: The error is structural (period placement), not a rounding artefact, and the exactness metadata is inverted. The `1/3` control is correct only because its pre-period is empty.
- **Related**: HYP-001, EVD-001, EVD-002, EVD-003, EVD-004

### DEC-001: Numerical claims use a three-way taxonomy, not a binary exact/inexact

- **Decision**: Every numerical statement is labelled **exact**, **precision-bounded**, or **parity**, as written in `docs/editorial.md:33-45`.
- **Owner**: agent
- **Status**: approved
- **Rationale**: EVD-002/EVD-004 show a value can be octally wrong while flagged exact; a two-way taxonomy cannot express that.
- **Alternatives considered**: "exact vs approximate" (rejected: cannot label parity artefacts); relying on the `exact` flag (rejected: EVD-004 proves it is unreliable).
- **Related**: EVD-004, ART-003

### DEC-002: `.ls` is the canonical script extension

- **Decision**: Current documentation uses `.ls` exclusively; `.lv` is mentioned only as a tolerated older extension in one place.
- **Owner**: agent
- **Status**: approved
- **Rationale**: EVD-010 — 21 `.ls` files, 0 `.lv` files; every committed fixture and `examples/hello.ls` use `.ls`.
- **Alternatives considered**: canonicalizing `.lv` (rejected: would require renaming every fixture).
- **Related**: EVD-010, TODO-004

### DEC-003: The machine-read platform matrix is not edited as prose

- **Decision**: `docs/goal-desktop-v4/platform-matrix.md` keeps its v1 rows and gains a historical banner instead; promoting Linux requires the `platform_claims` code and test change in the same commit.
- **Owner**: agent
- **Status**: approved
- **Rationale**: EVD-007/EVD-008 — `matrix_problems` fails when a second row claims `INITIAL RELEASE`; editing the row alone would fail `platform_claims` tests and the binary would still refuse Linux.
- **Alternatives considered**: flipping the Linux row (rejected: breaks the invariant and leaves `unsupported_notice` lying); deleting the matrix (rejected: it is release metadata read at runtime).
- **Related**: EVD-007, EVD-008, EVD-009, TODO-001

### DEC-004: D1 and D3 as written are unusable measurements and are refined

- **Decision**: D1 and D3 stop being zero-hit string greps. D1 measures *assertions* of Windows-only support (negations and historical markers exempt). D3 measures the phrase used *as a claim*, not its mention in the rule that forbids it or in a correction that quotes it.
- **Owner**: agent
- **Status**: approved
- **Rationale**: The measured hits are dominated by correct text — `copilot-instructions.md:37` says "The desktop is not Windows-only", `docs/editorial.md:37` forbids the phrase, `docs/adr/004:101` cites it as the thing to replace. A naive grep would report done work as a defect and push a future round to delete accurate sentences.
- **Alternatives considered**: keeping the greps literal (rejected: it incentivises deleting correct negations to go green, which is the "weakened to reach done" failure mode).
- **Related**: TODO-009, TODO-010

### RISK-001: No `.gitignore`, so build outputs are one `git add -A` from being committed

- **Risk**: `target/`, `.round3-artifact/`, `.round4/` and three `linux-job-*.log` files are untracked and unignored; a stray commit would violate the AGENTS.md guardrail.
- **Likelihood**: Medium
- **Impact**: High
- **Evidence**: EVD-011
- **Mitigation**: add a `.gitignore` covering `target/`, `.tmp/`, `*.log`, and the scratch directories — but only after the user confirms which of those files are meant to be tracked.
- **Gate**: —

### RISK-002: The desktop crate may not build offline, so D2 cannot be verified

- **Risk**: `platform_claims` lives in `lovelace-desktop`, which depends on `eframe`/`egui`/`resvg` from the registry. The engine crates build offline; the desktop may not.
- **Likelihood**: Medium
- **Impact**: High
- **Evidence**: EVD-006 (engine built), `lovelace-desktop/Cargo.toml:19-41`
- **Mitigation**: attempt `cargo build -p lovelace-desktop` in WSL at the start of round 1; if the registry is unreachable, record D2 as `Unresolved` (an OQ), never as met.
- **Gate**: —

### TODO-001: Promote Linux in `platform_claims` and the release matrix

- **Task**: Allow N supported rows (`matrix_problems`), remove the hard-coded Windows-only refusal in `unsupported_notice`, flip the Linux row to supported with its CI evidence, update the affected tests (`exactly_one_platform_is_the_certified_windows_release`), and check `.github/workflows/release.yml`'s manifest claim.
- **Priority**: P0
- **Depends on**: DEC-003, D2 buildability (RISK-002)
- **Status**: Open
- **Related**: DOC-001, EVD-007, EVD-008, EVD-009

### TODO-002: Publish the four tutorials

- **Task**: `docs/tutorials/` — linear algebra, symbolic, signal processing, reproducible computing. Each with problem, source, reproduced output, interpretation, precision notes, verification command.
- **Priority**: P1
- **Depends on**: none (WSL runner available)
- **Status**: Open
- **Related**: DOC-006, D7

### TODO-003: Gate the Markdown example runners in CI

- **Task**: Invoke both example runners from `.github/workflows/ci.yml`; fix the Windows-only `.exe` discovery in `docs/extensions/examples/run-examples.ps1`.
- **Priority**: P1
- **Depends on**: OQ-002
- **Status**: Open
- **Related**: DOC-007, D8

### TODO-004: Remove `.lv` from current documentation

- **Task**: Replace `.lv` with `.ls` in `docs/crates/lovelace-desktop.md:64`, `docs/extensions/README.md:165`, `docs/language/plots.md:188,191`, and confirm `docs/language/README.md:44` reads as a historical note rather than a second convention.
- **Priority**: P1
- **Depends on**: DEC-002
- **Status**: Open
- **Related**: DOC-009, EVD-010, D10

### TODO-005: Add a link checker

- **Task**: `tools/check-links.ps1` — resolve every relative link in current docs; fail on a miss; exempt `docs/goal-desktop-v4/` under a declared archival policy.
- **Priority**: P2
- **Depends on**: DOC-008 boundary
- **Status**: Open
- **Related**: DOC-012, D13

### TODO-006: Reference-page template and conformance check

- **Task**: Document the seven-section shape and check the public reference pages against it.
- **Priority**: P2
- **Depends on**: OQ-001 (scope: all 124 builtins, or the page-level surface?)
- **Status**: Open
- **Related**: DOC-014, D15

### TODO-007: Replace brittle line references with symbols

- **Task**: Sweep `docs/**` for `.rs:NNN` citations outside historical records.
- **Priority**: P2
- **Depends on**: none
- **Status**: Open
- **Related**: DOC-015, D16

### TODO-008: Remove the personal development path from the plotting guide

- **Task**: `docs/language/plots.md:45-46` contains `C:\Users\ricar\dev\LovelaceSharp\...` as observed output; re-run the example to get a portable path or mark it provenance.
- **Priority**: P2
- **Depends on**: none
- **Status**: Open
- **Related**: DOC-013, D14

### TODO-009: Make D1 precise

- **Task**: Implement D1 as an assertion-level check with an explicit exemption list (negations, historical ADRs, the `journeys.md` capture note).
- **Priority**: P1
- **Depends on**: DEC-004
- **Status**: Open
- **Related**: DEC-004

### TODO-010: Make D3 precise

- **Task**: Implement D3 as a claim-level check: the phrase may appear only inside `docs/editorial.md`, `docs/adr/004`, and `docs/extensions/dsp.md` as a correction or prohibition.
- **Priority**: P1
- **Depends on**: DEC-004
- **Status**: Open
- **Related**: DEC-004

### TODO-011: Add the documentation-sync agent skill

- **Task**: Create `.dsh/skills/sync-docs/SKILL.md` — a dependent-document map (registry → `docs/language/builtins`, behaviour → ADR, platform → matrix + release skill, wire envelope → `docs/crates/lovelace-run`) and the instruction to check *semantic drift*, not merely that a Markdown file was touched. Index it from `docs/skills.md`.
- **Priority**: P1
- **Depends on**: none
- **Status**: Open
- **Related**: D17, ART-003

### OQ-001: What does DOC-014 "standardize reference pages" cover?

- **Question**: Does DOC-014 require rewriting all 124 builtin descriptors into the seven-section shape, or a documented template plus a conformance check over the page-level public reference surface (`docs/language/`, `docs/extensions/`, `docs/crates/`)?
- **Needed evidence**: user decision. 124 descriptor rewrites is a multi-round effort and is not implied by "Public APIs document ... in a consistent order" as written.
- **Priority**: P1 important
- **Raised by**: Round 0, orchestrator
- **Related**: DOC-014, TODO-006

### OQ-002: Should the example runners be ported or run on Windows only?

- **Question**: DOC-007 says "execute language and extension examples on relevant platforms". The runners are PowerShell and `run-examples.ps1` assumes Windows `.exe` paths. Does the user want a bash port for the Linux job, or the Windows job to run them (where the platform is relevant)?
- **Needed evidence**: user decision; a bash port roughly doubles the runner surface to maintain.
- **Priority**: P1 important
- **Raised by**: Round 0, orchestrator
- **Related**: DOC-007, TODO-003

### OBS-003: The Linux build passes `--platform-claims` while refusing itself

- **Source**: `/tmp/rl-target/debug/lovelace-desktop --platform-claims` (built from this tree)
- **Fact**: The binary prints `this build: target_os=linux arch=x86_64 advertised=false`, then `refusal[linux/x86_64]: ... the native workbench is unsupported on linux/x86_64 ... (the supported platform for this release is Windows 11 x64)`, then `result PASS: 4 platform row(s), 0 problem(s)` and exits 0.
- **Implications**: DOC-001 cannot be closed in documentation. The shipped Linux artifact tells an operator it is unsupported and simultaneously certifies itself. The release matrix and `platform_claims` must change together.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: EVD-017, EVD-018, EVD-019, TODO-001

### OBS-004: The natural first attempt at each tutorial workflow fails

- **Source**: `/tmp/rl-target/debug/lovelace-run --eval <expr> --text` (eleven invocations)
- **Fact**: `diff(x^3 + 2*x, x)` fails with `Undefined variable 'x'` — a symbol must be declared first. `linsolve([[2,1],[1,3]], [5,6])` fails with `linsolve() requires a symbolic matrix A`. `matmul(A, [1,1])` fails with `matmul shapes do not agree`. Working forms: `x = symbol("x"); diff(x^3 + 2*x, x)` → `3*x^2 + 2`; `matmul([[2,1],[1,3]], [[5],[6]])` → `[[16],[23]]`; and `Inv = inv(...); sol = matmul(Inv, [[5],[6]]); residual = matmul(A, sol)` → `[[5],[6]]` exactly.
- **Implications**: DOC-006's four tutorials must open with the working form and state the declaration requirement, or a newcomer's first command errors. This is the onboarding gap the review describes, and the material for it.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: EVD-021, TODO-002

### OBS-005: `simplify` does not cancel a removable singularity

- **Source**: `lovelace-run --eval 'x = symbol("x"); simplify((x^2 - 1)/(x - 1))' --text`
- **Fact**: The result is `(x^2 - 1)/(x - 1)`, unchanged. `lovelace-lang/src/registry.rs`'s descriptor says `simplify` "applies only universal rules and rules whose side conditions are already provable (conditions are never silently discarded)".
- **Implications**: The symbolic tutorial's "simplify" step must either declare the assumption that makes the cancellation sound or teach the refusal as the correct behaviour. It must not claim that `simplify` cancels.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: OBS-004, TODO-002

### VAL-002: The intent audit artifact is real and its findings reproduce

- **Target**: round-1 claim batch (the auditor's 19-row table)
- **Method**: Opened `docs/goal-docs-v1/intent-audit.md` and checked every "Proposal location" citation against `goal.md`; confirmed the two verdict rows and the narrowing flags against the D-check text as written.
- **Evidence examined**: `docs/goal-docs-v1/intent-audit.md:1-23`; `goal.md` mechanism ledger and D-table.
- **Result**: Supported
- **Conclusion**: 17 KEPT / 1 REPLACED (#18 automated docs CI pipeline) / 1 REMOVED (#19 epistemic limit) is accurate. A third finding the summary understated: **six** dimensions are narrowed rather than removed (D10 deletes `.lv` behaviour instead of documenting it; D12 verifies existence, not enforcement; D13 checks links but not code references; D14 gates nothing on provenance isolation; D16 gates the line-number form of DOC-015 but not dead symbol citations; D3 greps two phrases rather than auditing claims). Those are corrected in goal.md before dispatch.
- **Related**: DEC-005, OQ-001

### DEC-005: The ledger and the D-checks are corrected in response to the audit

- **Decision**: Restore #18 and #19 as KEPT with named carrying artifacts (`docs` CI job; the epistemic limit in `docs/editorial.md` and `docs/testing.md`); replace the narrowed checks: D10 now splits *documented* from *used*, D12 names the enforcement path, D13 covers path-shaped code references, D16 covers dead symbol citations, and new D18/D19 pin the docs job and the correctness limit. D-table grows to 19.
- **Owner**: agent
- **Status**: approved
- **Rationale**: VAL-002. `REMOVED` without a user-approved exclusion forbids round 1; the audit's narrowing flags are defects in my own measurement, and fixing a measurement is an agent decision.
- **Alternatives considered**: asking the user to approve the removal of #19 (rejected: the user named it, and it costs one paragraph and one grep to keep); leaving D10 as a zero-hit grep (rejected: it would delete the `.lv` documentation the user asked for).
- **Related**: VAL-002, TODO-012, TODO-013

### DEC-006: DOC-014 conformance scope

- **Decision**: Seven sections in fixed order (purpose, inputs, outputs, precision behaviour, errors, limitations, examples) are enforced **with a failing CI gate over the page-level reference surface** — `docs/language/`, `docs/extensions/`, `docs/crates/`. The 124 builtin descriptors stay in the existing registry table but must satisfy the three-claim taxonomy (no unlabelled exactness claim). No descriptor-by-descriptor rewrite.
- **Owner**: user
- **Status**: approved
- **Rationale**: D15 redefined accordingly. The reviewer asked for an "enforceable project contract"; the page-level reading delivers that without rewriting the registry's user-facing prose.
- **Alternatives considered**: all-124 rewrite (rejected by the user: multi-round, and not implied by "in a consistent order"); template-only (rejected by the user: does not match "failing CI").
- **Related**: OQ-001, TODO-006, TODO-014, D15

### TODO-012: Add the `docs` CI job

- **Task**: One `docs` job in `.github/workflows/ci.yml` running structural validation → both example runners → `tools/check-docs.ps1` → summary, in that order, failing the workflow.
- **Priority**: P1
- **Depends on**: TODO-003, TODO-005, TODO-013
- **Status**: Open
- **Related**: D18, DOC-007, DOC-012

### TODO-013: `tools/check-docs.ps1`

- **Task**: Link resolution + path-shaped code references + `-Symbols` mode that rejects `.rs:NNN` and unresolved symbol citations; `docs/goal-desktop-v4/` exempt under the declared archival policy.
- **Priority**: P2
- **Depends on**: none
- **Status**: Open
- **Related**: D13, D16, DEC-005

### VAL-003: Round 1 implementer claims verified; no weakening found

- **Target**: round-1 claim batch (implementer `3cfe2e77`)
- **Method**: Rebuilt the binary, re-ran the filtered and the full claim probe myself, read the matrix, and read the whole `platform_claims.rs` diff looking specifically for a relaxed assertion.
- **Evidence examined**: `cargo test -p lovelace-desktop platform_claims` → 10 passed / 0 failed; `--platform-claims` → `advertised=true`, `supported[linux/x86_64]`, no Linux refusal, macOS refusal naming both, `result PASS`, exit 0; `platform-matrix.md:23-26`; the diff's `claims_support`, `matrix_problems`, `release_problems`.
- **Result**: Supported
- **Conclusion**: Every claimed observable reproduced. The change is a net strengthening: `matrix_problems` gained two requirements (a row per supported platform; run evidence required for any support claim) and `release_problems` added a self-refusal invariant. The two renamed tests encoded the retired one-platform rule; five tests were added. No assertion weakened, none deleted, none ignored.
- **Residual**: the implementer could not run a Windows binary (no Windows Rust here), and `xvfb-run` availability on this host is being checked separately. Neither is a scope defect.
- **Related**: EVD-022, EVD-023, EVD-024, EVD-025, TODO-001

### DEC-007: Round 1 closes DOC-001's code half

- **Decision**: D2 is `Met`. The matrix and `platform_claims` now encode two supported platforms, and the Linux build advertises itself instead of refusing itself.
- **Owner**: agent
- **Status**: approved
- **Rationale**: EVD-022/EVD-023 reproduce the required observables; EVD-025 shows no gate was bought with a weakened test.
- **Alternatives considered**: keeping D2 open pending a Windows binary (rejected: DOC-001 is about the documents and the matrix agreeing, and the Linux build is the one that was lying).
- **Related**: D2, EVD-023, TODO-001

### OBS-006: Round 1 made every documented desktop test count stale

- **Source**: `AGENTS.md` (desktop counts), `docs/testing.md:38-40`, `docs/crates/lovelace-desktop.md:54`
- **Fact**: Those pages state 716 on Windows and 706 + 4 = 710 on Linux. The measured Linux total is now 711 + 4 = 715 (EVD-026), because Round 1 added five tests.
- **Implications**: DOC-010 is not satisfied by a one-time edit; the numbers are duplicated across three pages with no version and no measured-on note. The fix is to measure them, version them, and stop duplicating them.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: EVD-026, EVD-018, TODO-015, D11

### TODO-015: Re-measure and de-duplicate the desktop test counts

- **Task**: Record the Linux total measured on v0.4.1 (711 + 4 = 715) once, in `docs/testing.md`; have `AGENTS.md` and `docs/crates/lovelace-desktop.md` cite that page instead of repeating numbers; mark the Windows figure as not measured in this environment rather than adjusting it by inference.
- **Priority**: P1
- **Depends on**: none
- **Status**: Open
- **Related**: OBS-006, EVD-026, DOC-010, D11

### DEC-008: Round 2 scope widened for DOC-009

- **Decision**: Add `docs/build-and-run.md` and `docs/extending.md` to the Round 2 scope; keep the two `.ps1` example runners out of scope and record them as residual for the CI round.
- **Owner**: agent
- **Status**: approved
- **Rationale**: The implementer reported that D10 ("no example command line in `docs/` uses `.lv`") cannot be satisfied inside the seven files I listed, because `docs/build-and-run.md:57,148` and `docs/extending.md:92` also carry `.lv`. The scope list was defective; the requirement is not. Widening the *round's* file set restores the named mechanism, whereas dropping the requirement would narrow the goal.
- **Alternatives considered**: leaving those files to a later round (rejected: D10 would stay unmeasurable and the same defect would be re-found); relaxing D10 to "three files only" (rejected: that is narrowing the goal to match a bad scope).
- **Related**: TODO-004, D10, DOC-009

### DEC-009: Both example runners are ported to bash and run on both CI jobs

- **Decision**: Convert `docs/language/examples/run-examples.ps1` and `docs/extensions/examples/run-examples.ps1` to bash equivalents, remove the Windows-only `.exe` discovery assumption, and invoke both from a single `docs` job that runs on both the Windows and Linux jobs.
- **Owner**: user
- **Status**: approved
- **Rationale**: DOC-007 requires the examples to be executed on relevant platforms; the runners are the only thing that proves a fenced example still matches its recorded output. Porting closes the gap on both platforms rather than declaring Linux out of scope. Alternatives (Windows-only gating; rewriting as Rust tests) were offered and rejected by the user.
- **Alternatives considered**: Windows-job-only gating (rejected by the user: a Linux-only doc drift would not fail); Rust integration tests (rejected by the user: the review said the existing infrastructure sufficed).
- **Related**: OQ-002, TODO-003, TODO-012, D8, D18

### VAL-004: Round 2 claims verified; D10/D11/D14 close

- **Target**: round-2 claim batch (implementer `df3f7a37`)
- **Method**: Re-ran the `.lv` sweep over every `.md`/`.yml` outside the goal directory and opened both surviving hits; read `docs/testing.md:39-44`, `AGENTS.md:65-67`, `docs/language/plots.md:40-50`, `docs/architecture.md:36-40` and `docs/editorial.md:77-81`.
- **Evidence examined**: EVD-027 (two `.lv` hits, zero example command lines, one behavioural statement), EVD-028 (715 versioned, Windows explicitly not inferred), EVD-029 (placeholder path, 0 personal paths, architecture and editorial corrected).
- **Result**: Supported
- **Conclusion**: All four corrections landed and none is cosmetic: the count carries a version, a platform and a refusal to infer the unmeasured platform; the plot example keeps a runnable command and a truthful output while removing the machine path.
- **Residual**: `C:\Users\<you>` placeholders remain in `docs/build-and-run.md` and `docs/editorial.md`, which is the correct form (a placeholder, not a personal path). `.lv` remains inside the `.ps1` runners' temp-file names — script behaviour, deferred to the CI round.
- **Related**: EVD-027, EVD-028, EVD-029, DEC-010

### DEC-010: Round 2 closes DOC-009, DOC-010 and DOC-013

- **Decision**: D10, D11 and D14 are `Met`.
- **Owner**: agent
- **Status**: approved
- **Rationale**: EVD-027/028/029 reproduce every required observable. D10's ambiguity was resolved by widening the round (DEC-008) rather than relaxing the check.
- **Alternatives considered**: keeping D10 open over the historical `welcome.lv` record (rejected: a captured past run is archival evidence, not an instruction to a reader).
- **Related**: D10, D11, D14, EVD-027

### RISK-003: Build artifacts reappear in the tree because there is no `.gitignore`

- **Risk**: `cargo test` writes `target/tmp/round19-plot-*` and a plot example writes `target/example-plots/`; both are untracked and unignored, so the working tree cannot stay clean and a `git add -A` would commit them.
- **Likelihood**: High (observed twice)
- **Impact**: Medium
- **Evidence**: EVD-011; `git status` after both the Round 1 and Round 2 verification runs showed `?? target/`
- **Mitigation**: I delete `target/` after each verification pass. The durable fix is a `.gitignore` covering `/target/` and `/.tmp/`, which I have not added because it is repository policy and outside this goal's declared scope. It is reported in the close-out.
- **Gate**: —

### DEC-011: The repository gains a `.gitignore` (RISK-003 closed)

- **Decision**: Add `.gitignore` at the repository root covering Rust build output (`/target/`, `/lovelace-desktop-spike/target/`), local scratch (`/.tmp/`), the local working artifacts observed untracked (`.round3-artifact/`, `.round4/`, `linux-job-*.log`, `pr-body.md`), secrets and certificates (`*.pfx`, `*.p12`, `*.key`, `*.pem`, `.env*`, `/.secrets/`), and editor/OS noise.
- **Owner**: user
- **Status**: approved
- **Rationale**: The user asked for it directly after RISK-003 was reported. Before this, `cargo test` created `target/tmp/round19-plot-*` and the plot example created `target/example-plots/` on every verification pass, and with no ignore rules a single `git add -A` would have committed them — against the AGENTS.md guardrail.
- **Verification**: `git check-ignore -v` matches all eight probe paths; `git check-ignore` returns nothing for `Cargo.lock`, `Cargo.toml`, `README.md`, `docs/editorial.md`, `docs/tutorials/README.md`, `.dsh/skills/...` and `docs/goal-desktop-v4/journeys.md`. `git status` now lists only the seven legitimate new content paths.
- **Alternatives considered**: ignoring only `/target/` (rejected: leaves the other observed artifacts noisy); also ignoring `.vscode/` conditionally (kept unconditional, since nothing is tracked there today).
- **Note**: `.gitignore` sits outside this goal's `baseline_scope`, which is why it is recorded as a user-authorised change rather than folded into a round.
- **Related**: RISK-003, EVD-011

### VAL-005: Round 3 tutorials reproduce and their claims hold

- **Target**: round-3 claim batch (implementer `bf9d8569`)
- **Method**: Wrote a verifier that extracts each tutorial's own `## Verification` block, executes it in `/tmp`, and diffs stdout against the `## Output` block; then re-ran the four most surprising sub-claims *outside* the tutorial scripts.
- **Evidence examined**: EVD-030 (four exact matches, seven headings in order in every file); EVD-031 (`assume(x != 0); simplify(x/x)` → `1`; `cancel` → `x + 1`; `evalf(sqrt(2),21)` → 21 digits; `assume(x != 1)` leaves the quotient alone).
- **Result**: Supported
- **Conclusion**: The tutorials are executable documentation, not transcripts. Two apparent red flags were checked and cleared: a grep for hedging words matched only a variable literally named `approx`, and `movingavg` is used on an integer impulse where the documented defect cannot bite (`movingavg([0,1,0,0], 2)` → `[0, 0.5, 0.5, 0]`, verified) with the defect cited.
- **Residual**: `linsolve` is correctly absent (it rejects numeric matrices); the numeric solve is `inv` + `matmul`.
- **Related**: EVD-030, EVD-031, OBS-004, OBS-005

### DEC-012: Round 3 closes DOC-006

- **Decision**: D7 is `Met`. Four independently executable tutorials with interpreted results exist.
- **Owner**: agent
- **Status**: approved
- **Rationale**: EVD-030 reproduces every recorded block byte-for-byte with the seven-part shape; EVD-031 confirms the semantics the prose claims.
- **Alternatives considered**: accepting the report's claim without re-running (rejected: that is the failure mode the harness exists to prevent).
- **Related**: D7, ART-006

### VAL-006: Round 4 gate verified and independently falsified

- **Target**: round-4 claim batch (implementer `d7ef2779`)
- **Method**: Ran both `.sh` runners myself; read the runner's comparison logic; then falsified the gate on a page and line the implementer had not touched.
- **Evidence examined**: EVD-032 (193 + 27 = 220 examples pass), EVD-033 (corrupting `lexical-structure.md:58` → exit 1, `FAIL ...:54 (exit 1): output mismatch`, `1 of 193 example(s) failed`; restore byte-identical; exit 0 again), EVD-034 (the `docs` job block).
- **Result**: Supported
- **Conclusion**: The gate is real. `run-examples.sh:199-219` compares the normalised observed output against the recorded block with string equality and prints file, line, expected and observed on failure — not a scaffolding check. The `docs` job fails the workflow when either runner exits non-zero.
- **Residual (recorded, not hidden)**: the builtins-catalogue cross-check against `--functions` remains in the `.ps1` script only, so the bash port covers fenced examples but not that one check. No `windows-latest` run has executed; the `.exe` branch was exercised by simulation only. Both are `OQ-003`/`OQ-004`.
- **Self-correction**: my falsification script's `git diff --stat` briefly reported a 752-line change to the restored file (eol-normalisation flapping around the restore). Re-checked afterwards: `git status --porcelain` and `git diff --numstat` are both empty for that file, and `docs/language/` has only the two intended Round 2 edits.
- **Related**: EVD-032, EVD-033, EVD-034, TODO-003, DEC-009

### DEC-013: Round 4 closes DOC-007 and the `docs` CI job

- **Decision**: D8 and D18 are `Met`.
- **Owner**: agent
- **Status**: approved
- **Rationale**: EVD-032 reproduces the pass on both runners; EVD-033 proves the gate fails on drift, which is what DOC-007 requires; EVD-034 confirms the job runs on both platforms and propagates the exit status.
- **Alternatives considered**: marking D8 partial until a real `windows-latest` run exists (rejected: the job is defined and the runner logic is platform-branched and exercised; the missing piece is recorded as an OQ rather than blocking the dimension).
- **Related**: D8, D18, ART-007

### VAL-007: Round 5 verified; the machine-read file survived

- **Target**: round-5 claim batch (implementer `93089ca7`)
- **Method**: Re-ran the journey tests myself after the change; read both document headers; grepped for the two required sentences; confirmed the skill index and the refreshed harness catalog.
- **Evidence examined**: EVD-035 (journey tests 7/7 after the banner), EVD-036 (both headers correct), EVD-037 (limit sentence in both files; enforcement path in two places), EVD-038 (`sync-docs` indexed and in the session catalog).
- **Result**: Supported
- **Conclusion**: The banner was the risky change — `journeys.md` is parsed by the journey tests — and it changed 7 insertions, 0 deletions with the table untouched, confirmed by both the implementer's table diff and my re-run.
- **Residual**: the implementer `--list`-checked the new skill's Verify commands rather than executing them. Those commands are `cargo build`/`cargo test` invocations already exercised elsewhere in this run, so this is recorded rather than treated as a pass.
- **Related**: EVD-035, EVD-036, EVD-037, EVD-038

### DEC-015: D9's check was wrong, not the platform matrix

- **Decision**: D9 now requires a historical marker on `journeys.md` **only**, and a "current support statement" marker on `platform-matrix.md`.
- **Owner**: agent
- **Status**: approved
- **Rationale**: EVD-036. Round 1 promoted the matrix to the current support statement (DEC-007), so a check demanding "Historical snapshot" there would force a future round to write something false about the file, or revert the promotion.
- **Alternatives considered**: adding a historical marker to the matrix as well (rejected: it would contradict ADR-008 and the Linux row's SUPPORTED status).
- **Related**: EVD-036, DEC-007, D9

### DEC-016: Round 5 closes DOC-008, DOC-011, the skill and the epistemic limit

- **Decision**: D9, D12, D17 and D19 are `Met`.
- **Owner**: agent
- **Status**: approved
- **Rationale**: EVD-035..038 reproduce every required observable. D12's "enforcement path" is now concrete: a named review requirement in two contributor-facing documents plus the `docs` CI job.
- **Alternatives considered**: keeping D12 partial until a CI run enforces the checklist (rejected: enforcement is stated where contributors read it, and the gate that fails on drift is D8/D18, already met).
- **Related**: D9, D12, D17, D19, ART-008

### OQ-003: The builtins-catalogue check is not in the bash runner

- **Question**: Should the `--functions` catalogue cross-check be ported to bash as well, so the `docs` job covers it on both platforms?
- **Needed evidence**: user or maintainer decision; it is one additional check in the same job.
- **Priority**: P2 curiosity
- **Raised by**: Round 4
- **Related**: TODO-003, D8

### VAL-008: The checker is real and its first output is a list of real defects

- **Target**: round-6 claim batch (implementer `547c5665`)
- **Method**: Ran both checker modes myself; read every failure line; grouped the 141 line-suffix failures by file.
- **Evidence examined**: EVD-039 (55 pages / 147 links / 224 refs; 4 failures; `--symbols` 145), EVD-040 (the four are genuine broken references; the 141 are line-suffix citations over 23 files).
- **Result**: Supported
- **Conclusion**: The checker discriminates — zero broken links, zero unknown symbols, and every failure is a real defect. The implementer was told not to weaken it to manufacture a pass and did not: the new CI step will keep the `docs` job red until the repair lands.
- **Consequence for the gates**: D13 and D16 **cannot** be marked met while the checker exits 1. They stay open until the repair round, which is the correct behaviour — an implemented-but-failing check is not a delivered property.
- **Related**: EVD-039, EVD-040, TODO-007, D13, D16

### DEC-017: Round 6 delivers the checker; D13/D16 remain open by design

- **Decision**: Register `tools/check-docs.sh` as a deliverable, but leave D13 and D16 unmet until `bash tools/check-docs.sh --symbols` exits 0.
- **Owner**: agent
- **Status**: approved
- **Rationale**: EVD-039/EVD-040. Marking a dimension met because the tool exists would reward the failure mode the harness exists to prevent.
- **Alternatives considered**: marking D13 met because the link check half passes (rejected: the dimension covers code references too, and 4 fail).
- **Related**: D13, D16, ART-011

### TODO-016: Repair the 145 checker failures

- **Task**: Convert 141 `.rs:NNN` citations to symbol or file references across 23 files, and reword the 4 broken external/absent path references.
- **Priority**: P0
- **Depends on**: nothing
- **Status**: In progress (Round 7, two agents on disjoint file sets)
- **Related**: EVD-040, D13, D16

### VAL-009: The repair is real and it found stale citations

- **Target**: round-7 claim batch (implementers `58bdbbc1`, `f4a96741`)
- **Method**: Ran the checker myself until it exited 0; grouped the remaining citations by directory; re-ran both example gates; compared raw and whitespace-ignoring diffs to rule out a cosmetic rewrite; spot-checked four of the reported stale line numbers against the source.
- **Evidence examined**: EVD-041 (exit 0, 20 exemptions all in `docs/adr/`), EVD-042 (193 + 27 still pass; diffs are citation-shaped), EVD-043 (four `file:line` citations pointed at the wrong line).
- **Result**: Supported
- **Conclusion**: 145 failures → 0, with no check weakened and no example broken. The repair also demonstrated DOC-015's premise empirically: at least eight citations named the right *symbol* at the wrong *line*, so the prose was correct while the locator had rotted. That is precisely why the reviewer asked for stable symbols.
- **Residual**: 20 line-suffix citations remain under `docs/adr/` by design — the checker exempts historical records, and the review allows commit-pinned line references there.
- **Related**: EVD-041, EVD-042, EVD-043

### DEC-018: Round 7 closes DOC-012 and DOC-015

- **Decision**: D13 and D16 are `Met`.
- **Owner**: agent
- **Status**: approved
- **Rationale**: EVD-041 — the checker that was red at 145 failures is now green with exit 0, every surviving citation is an exempt historical one, and EVD-042 shows the gates that execute the docs still pass.
- **Alternatives considered**: marking D16 met on the strength of the citation count alone (rejected: the count only matters because the checker, not the count, decides).
### DEC-020: The goal cannot be closed while `.gitignore` sits outside the declared baseline scope

- **Decision**: Record the completion blocker and ask the user to choose; do not delete a user-requested file to satisfy a declaration made before that request.
- **Owner**: user
- **Status**: pending
- **Rationale**: `update_goal action=complete` returns "cannot complete goal: 1 changed path(s) lie outside the accepted baseline scope … : .gitignore". Every dimension is met (D1–D19) and all six documentation gates plus the Rust suites reproduce, so the blocker is scope bookkeeping, not missing work: the goal was created with `baseline_scope` before the user asked for a `.gitignore` (DEC-011), and `update_goal` offers no way to widen the scope afterwards.
- **Alternatives considered**: deleting `.gitignore` to close the goal (rejected: it would revert an explicit user request and re-open RISK-003); writing the rules into `.git/info/exclude` (rejected: local-only, does not ship with the repository, so it does not answer the request); leaving the goal to expire on the round cap (rejected: that would misreport a finished objective as abandoned).
- **Related**: DEC-011, RISK-003, EVD-045

### OQ-004: No real `windows-latest` run has executed

- **Question**: Does the `docs` job pass on a real Windows runner, where Git Bash handles the `.exe` branch and `cygpath`?
- **Needed evidence**: one CI run on `windows-latest`.
- **Priority**: P1 important
- **Raised by**: Round 4
- **Related**: EVD-034, RISK-004

### RISK-004: The `docs` job is unproven on Windows until CI runs it

- **Risk**: The `.sh` runners branch on `uname` for `.exe` and use `cygpath`; both branches were simulated, not executed under Git Bash on a Windows runner.
- **Likelihood**: Medium
- **Impact**: Medium
- **Evidence**: EVD-034, OQ-004
- **Mitigation**: the job is matrixed and will report on the first push; if it fails, the failure will name the example or the missing binary rather than passing silently.
- **Gate**: —

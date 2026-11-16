# Goal Journal — goal-docs-v2

> Append-only. See the stretch-goal-harness evidence protocol for entry format.

---

<!-- New entries go below this line -->

### OBS-002: The doc checker treated harness memory as current documentation

- **Source**: `tools/check-docs.sh:103` (before the fix); `docs/goal-docs-v2/round-1/instructions.md`
- **Fact**: `is_current_doc` excluded `docs/goal-desktop-v4/*` and `docs/goal-docs-v1/*` only, so the new goal directory was scanned as reader-facing documentation. Round 1's own evidence file introduced 6 line-suffix citations and turned `check-docs.sh --symbols` red. The round-1 implementer reported it and correctly refused to edit a file outside its scope.
- **Implications**: A harness that pollutes the gate it is trying to satisfy will fail its own acceptance criteria, and the failure would have been attributed to the deliverable rather than to the harness.
- **Confidence**: High
- **Agent**: round-1 implementer (reported) + orchestrator (verified)
- **Related**: DEC-002, RQ-024, C24

### DEC-002: `check-docs.sh` excludes every `docs/goal-docs-*` directory

- **Decision**: Replace the literal `docs/goal-docs-v1/*` exclusion with `docs/goal-docs-*`, and document in the script header that the wildcard is deliberate.
- **Owner**: agent
- **Status**: approved
- **Rationale**: OBS-002. Harness journals are working memory, not reader-facing documentation (RQ-024 says exactly that). A per-version exclusion guarantees the same failure recurs at `goal-docs-v3`, and the cost of the wildcard is that no goal directory can ever be validated as documentation — which is the intended policy.
- **Alternatives considered**: stripping line-suffix citations from harness files (rejected: the journals must be able to cite evidence precisely, and every future round would re-introduce them); excluding only v2 (rejected: recurs at v3).
- **Related**: OBS-002, RQ-024, C24, EVD-008

### OBS-003: The negative tests for the new guards were scratch-only

- **Source**: `docs/goal-docs-v2/round-1/crate-pages-and-runners.md:249-254`; `tools/`
- **Fact**: The round-1 implementer ran ten negative cases and one real-tree mutation, each by copying the runner into a scratch tree. `tools/` contains no gate self-test, so nothing in the repository can reproduce those cases after the scratch directories are gone.
- **Implications**: RQ-007.5 says "Add negative tests", and RQ-006.4 requires "a negative/mutation test for each mode". A transient run satisfies neither durably: the next change to a guard could silently remove it and no check would notice.
- **Confidence**: High
- **Agent**: orchestrator (against the implementer's claim)
- **Related**: EVD-015, TODO-001, RQ-007, RQ-006

### VAL-001: Round-1 instruction claims reproduce

- **Target**: round-1 claim batch (implementers `116422bd`, `37efbc59`)
- **Method**: Ran both test filters, the platform_claims suite, all three checker modes, both example runners, and two falsifications of the new manifest guard with sha256-verified restores; read every rewritten prose section and every corrected crate-page claim against the source signatures.
- **Evidence examined**: EVD-009…EVD-014.
- **Result**: Supported
- **Conclusion**: RQ-004, RQ-005 and the skill half of RQ-007 are genuinely done. The editorial rule now requires per-platform evidence, the sync-docs skill carries a "prove the filter selected a test" rule with explicit counts, the release skill covers three jobs and both binaries, and both guard directions fail with precise diagnostics.
- **Related**: DEC-003, EVD-009, EVD-012, EVD-014

### VAL-002: Round-1 negative-test claim is partially falsified

- **Target**: the implementer's "ten scratch negative cases plus one real-tree mutation all fail" claim, read against RQ-007.5's requirement to *add* negative tests
- **Method**: Listed `tools/`, searched the runners for a self-test mode, and read the report's negative-test section.
- **Evidence examined**: EVD-015.
- **Result**: Falsified as a durable **delivery** (the behaviour was true; the artifact was not delivered)
- **Conclusion**: The guards work — I reproduced two of them myself — but the tests that demonstrate them exist only in a scratch tree. C7 stays **Partial** until a committed, repeatable gate self-test exists and runs in the docs job.
- **Related**: EVD-015, OBS-003, DEC-003, TODO-001

### DEC-003: Round 1 closes RQ-004, RQ-005 and RQ-012; RQ-007 stays partial

- **Decision**: C4, C5 and C12 are `Met`. C7 is `Partial`: sub-criteria 1–4 are met and reproduced, sub-criterion 5 (added negative tests) is not durable.
- **Owner**: agent
- **Status**: approved
- **Rationale**: VAL-001 and VAL-002. Marking C7 met would accept a scratch run as a delivered test, which is the "accepting a summary in place of the artifact" failure the harness forbids.
- **Alternatives considered**: accepting the scratch cases because they demonstrably ran (rejected: nothing in the repo covers the guard, so a later edit removes it silently); re-dispatching round 1 (rejected: the missing piece is a small committed harness, and it belongs with RQ-006's CI mutation tests in round 2).
- **Related**: VAL-001, VAL-002, TODO-001, C4, C5, C7, C12

### TODO-001: Commit a durable gate self-test

- **Task**: Add `tools/test-docs-gates.sh` that constructs each defect in a scratch copy (undeclared page, missing declared page, zero examples, both unpaired shapes, each `check-docs` mode, each reference-page violation), asserts a non-zero exit and the expected diagnostic, and restores nothing because it never touches the tree. Wire it into the `docs` CI job.
- **Priority**: P1
- **Depends on**: none
- **Status**: Open
- **Related**: OBS-003, RQ-006.4, RQ-007.5, C6, C7

### VAL-003: The gate self-test is real and guards its own wiring

- **Target**: round-2 claim batch (implementer `711fb606`)
- **Method**: Ran the self-test on a clean tree; ran every gate mode and both runners; grepped the whole workflow for silent-skip constructs; then mutated `ci.yml` myself by replacing the `--claims` step and re-ran the self-test.
- **Evidence examined**: EVD-017 (17/17, exit 0), EVD-018 (the docs job block; no `|| true` inside it), EVD-019 (removing a step produces `FAIL ci wiring`, exit 1, and an identical restore returns 17/17).
- **Result**: Supported
- **Conclusion**: RQ-006.4 and RQ-006.5 are satisfied in substance, not just in form: each gate has a mutation that proves it can fail, and the self-test fails when the CI wiring that runs those gates is removed. This is what closed VAL-002's scratch-only verdict.
- **Residual**: the self-test has not run on Windows/Git Bash, and no hosted run exists yet (RQ-013, last round).
- **Related**: EVD-017, EVD-018, EVD-019, VAL-002, DEC-004

### DEC-004: Round 2 closes RQ-006, RQ-007 and RQ-012

- **Decision**: C6 and C7 are `Met`; C12 was already met and now also covers `docs/release.md`.
- **Owner**: agent
- **Status**: approved
- **Rationale**: EVD-017–EVD-019 reproduce the required observables, including the mutation-per-mode requirement and the "cannot be silently skipped" requirement. VAL-002's falsified delivery is remedied by a committed artifact, not a scratch run.
- **Alternatives considered**: keeping C7 partial until a hosted run exercises the mutation step (rejected: the requirement is that the tests exist and run in CI, both now true; hosted execution is C13's separate obligation).
- **Related**: VAL-003, C6, C7, C12, TODO-001

### VAL-004: My own EVD-005 is falsified — the 68-test gap was an inference, not a measurement

- **Target**: EVD-005, and the round-2 count claim batch (implementer `90fa296b`)
- **Method**: Re-measured `lovelace-know` and `lovelace-run` with full, untruncated output; summed the new `docs/testing.md` rows by hand; ran the workspace aggregate myself.
- **Evidence examined**: EVD-020 (know = 56 across 12 binaries; run = 450 = 438+1+11), EVD-021 (rows sum to 6147, matching my measured aggregate of 6147/0/1 across 56 result lines).
- **Result**: Falsified (EVD-005) / Supported (the agent's reconciliation)
- **Conclusion**: EVD-005 asserted that the 68-test gap "is exactly `lovelace-know`" on the strength of 6135 − 6067 = 68 and the crate being named in prose. That was an inference presented as a verified fact — the exact defect the evidence protocol calls out. The agent's measurement splits it 56 + 12 and is correct. EVD-020 supersedes EVD-005.
- **Self-correction**: my first attempt to check the claim reported `lovelace-know` = 1 test, because I piped `cargo test` through `tail -4` and truncated 10 of its 12 binaries. I caught it before recording a false accusation. Truncating a command's output is a way of fabricating a measurement.
- **Related**: EVD-005, EVD-020, EVD-021, RQ-011, C11

### DEC-005: Round 2 closes RQ-010 and RQ-011

- **Decision**: C10 and C11 are `Met`.
- **Owner**: agent
- **Status**: approved
- **Rationale**: EVD-020–EVD-022. The counts now sum to a measured aggregate at a named commit, OS and date; the 1,737/1,749/1,750 delta is explained by two commits; the Linux and Windows desktop figures are kept separate and Windows is labelled unmeasured; the crate fences are compiled and the checker can fail.
- **Alternatives considered**: keeping C11 partial because `README.md:105` and `:160` still carry stale fixed counts (rejected as a *C11* failure: RQ-011.5 names README, so it stays open as TODO-002 rather than silently passing).
- **Related**: VAL-004, EVD-020, EVD-022, C10, C11, TODO-002

### TODO-002: Stale fixed counts remain in the README

- **Task**: `README.md:105` carries a 716 transcript and `:160` a 6135 total. Replace both with output shapes and a pointer to `docs/testing.md`, which is now the measured record. `README.md` was outside the round-2 agent's scope.
- **Priority**: P1
- **Depends on**: none
- **Status**: Open
- **Related**: RQ-011.5, C11, OBS-004

### OQ-002: `check-crate-examples.sh` does not locate cargo itself

- **Question**: Should the script fall back to `$HOME/.cargo/bin/cargo` (and `%USERPROFILE%\.cargo\bin\cargo.exe` on Windows) instead of requiring `PATH` or an explicit `CARGO`?
- **Needed evidence**: none outstanding for CI (cargo is on `PATH` there), but a contributor on a shell without cargo on `PATH` gets exit 2. Decide when the script is wired into CI in round 3.
- **Priority**: P2 curiosity
- **Raised by**: Round 2, orchestrator
- **Related**: EVD-023, RQ-010.2

### OBS-004: Hosted CI caught a formatting failure that no local gate could

- **Source**: `gh run view 43866853549 --json jobs`; `cargo fmt --all -- --check`
- **Fact**: On `fa87ecc`, both the Linux and Windows jobs failed at **Check formatting** before any test ran. Locally, `cargo fmt --all -- --check` reported diffs in `precision_default.rs`, `builtins.rs` and `dsp_round20.rs` — the `movingavg` fix `1f114c9` was committed without formatting.
- **Implications**: Every local doc gate was green while the product did not build clean. This is exactly why RQ-013 forbids closing on local evidence: the documentation job does not run `rustfmt`, so it passed while both build jobs died.
- **Confidence**: High
- **Agent**: orchestrator (from hosted evidence)
- **Related**: EVD-028, EVD-029, RQ-013, DEC-007

### DEC-006: Formatting fixed as `style:` on the branch

- **Decision**: Run `cargo fmt --all`, verify `--check` is clean, re-run the affected suites, and commit as `style: apply cargo fmt …` (`d946316`).
- **Owner**: agent
- **Status**: approved
- **Rationale**: EVD-029. Formatting only — 3 files, +9/−12 — and the re-run confirms behaviour is unchanged (movingavg 9 passed, platform_claims 10 passed). The `style:` prefix deliberately does not trigger a release.
- **Alternatives considered**: amending `1f114c9` (rejected: the branch is pushed and the history should show the fix as its own evidence-bearing change).
- **Related**: OBS-004, DEC-007

### VAL-005: Round 3 claims reproduce

- **Target**: round-3 claim batch (`35eab53f`, `1a265e86`)
- **Method**: Ran the four-category grep, both RQ-002 target greps, four budget probes of `movingavg([1],7)`, the tutorial runner clean and corrupted, the stale-path and stale-count greps, and all documentation gates.
- **Evidence examined**: EVD-026, EVD-027, EVD-030, EVD-031.
- **Result**: Supported
- **Conclusion**: The numerical vocabulary is now defensible and the tutorials are reproducible and gated. The budget probe is the strongest artefact: `1/7` is truncated and marked inexact at budget 6 and exact at 7, so "precision-limited" is demonstrated rather than asserted.
- **Related**: DEC-007, DEC-008, EVD-026, EVD-030

### DEC-007: Round 3 closes RQ-002, RQ-008, RQ-009 and RQ-018; RQ-003 stays partial

- **Decision**: C2, C8, C9 and C18 are `Met`. C3 is `Partial`: the four categories exist and the reference pages use them, but RQ-003.4's tutorial half is deferred to round 4 because another agent owned `docs/tutorials/**` this round.
- **Owner**: agent
- **Status**: approved
- **Rationale**: VAL-005. Marking C3 met would accept a half-applied vocabulary across the very tutorials the requirement names.
- **Alternatives considered**: letting one agent own both tutorial portability and tutorial terminology (rejected at dispatch time: the tutorial runner was itself being rewritten, so two writers on those pages would have collided).
- **Related**: VAL-005, C2, C3, C8, C9, C18, TODO-003

### OBS-005: The new tutorial examples contradict the documented `.ls` convention

- **Source**: `docs/language/README.md:43-45`; `docs/tutorials/*.md`; `docs/tutorials/examples/`
- **Fact**: The language reference says scripts use `.ls` by convention and "new material should use `.ls`", but the four new tutorials carry 14 `.lv` references and ship four `.lv` example files.
- **Implications**: The newest material in the repository violates the convention the reference states. It is the same class as RQ-017's obsolete-assertion problem, and the earlier goal explicitly settled this convention (`.ls` canonical).
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: EVD-032, TODO-003, RQ-017, C17

### TODO-003: Rename the tutorial examples to `.ls` and apply the four-category vocabulary

- **Task**: Rename `docs/tutorials/examples/*.lv` to `.ls`, update every reference in the four tutorials and `tools/run-tutorials.sh`, and rewrite each tutorial's `## Precision` section to use exact / precision-limited / error-bounded / parity-verified. Also bring `docs/reference-template.md` and `docs/contributing.md` onto the four-category wording.
- **Priority**: P1
- **Depends on**: DEC-007 (tutorials now free)
- **Status**: Open
- **Related**: EVD-032, OBS-005, RQ-003.4, RQ-017, C3, C17

### OBS-006: Two current pages still carry counts and states that changed

- **Source**: `README.md:193`; `docs/skills.md:3`; `docs/editorial.md:71`
- **Fact**: The README calls `docs/skills.md` "the eight task skills" while that page says "Nine task skills" and `.dsh/skills/` holds nine directories. `docs/editorial.md:71` still says tutorials are a target shape with "none exist yet", although four exist.
- **Implications**: RQ-017 asks for exactly this class: current-documentation assertions that a count or an inventory disproves. Neither is caught by any existing gate, which is why RQ-022's cross-file inventory checks matter.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: EVD-033, TODO-004, RQ-017, RQ-022, C17, C22

### TODO-004: Correct the skills count and the tutorial status

- **Task**: `README.md:193` → "nine task skills"; `docs/editorial.md:71` → tutorials exist, point at `docs/tutorials/`. Both are owned by nobody in round 4, so they are queued.
- **Priority**: P2
- **Depends on**: none
- **Status**: Open
- **Related**: EVD-033, OBS-006, RQ-017, C17

### OBS-007: The wiring guard does not cover the two steps it should protect

- **Source**: `tools/test-docs-gates.sh`; `.github/workflows/ci.yml`
- **Fact**: The `docs` job now has 12 steps including `Compile and run the crate-page examples` and `Run the tutorials`, but `grep` finds neither command in the mutation self-test. The self-test asserts the older `--claims`/`--readme`/`--symbols` wiring only.
- **Implications**: RQ-006.5 says no check may be silently skipped while the job reports success. The two newest mandatory steps can be deleted from the workflow without any local signal — the exact failure the guard was built to prevent, now with a blind spot.
- **Confidence**: High
- **Agent**: orchestrator (the round-4 agent declared this residual itself)
- **Related**: EVD-036, TODO-005, RQ-006.5, C6

### TODO-005: Extend the wiring guard to every mandatory docs step

- **Task**: Make `tools/test-docs-gates.sh` assert the presence of **all** mandatory `ci.yml` steps by name and command — including `check-crate-examples.sh` and `run-tutorials.sh` — and prove it by deleting each one in turn.
- **Priority**: P1
- **Depends on**: none
- **Status**: Open
- **Related**: EVD-036, OBS-007, RQ-006.5, C6

### TODO-006: Bring the AGENTS.md CI table up to the new docs job

- **Task**: The CI table in `AGENTS.md` describes the docs job without the two new runners. Update the row so an agent reading it knows all twelve steps exist.
- **Priority**: P2
- **Depends on**: EVD-035
- **Status**: Open
- **Related**: RQ-022, RQ-012, C22

### VAL-006: Round 4 claims reproduce; C3 closes

- **Target**: round-4 claim batch (`d77f3f6c`, `444daebd`)
- **Method**: Grepped for `.lv`, listed the example directory, counted every category per tutorial, read a `## Precision` section in full, ran the tutorial runner, ran all six gates, and proved cargo self-location with `PATH` stripped and `CARGO` unset.
- **Evidence examined**: EVD-034 (self-location), EVD-035 (12 unconditional steps), EVD-037 (rename + vocabulary), EVD-038 (template/guide + all gates).
- **Result**: Supported
- **Conclusion**: RQ-003.4 is satisfied across reference pages, template, guide and tutorials. The tutorials now justify each label per value rather than sprinkling adjectives, which is what makes the taxonomy checkable.
- **Residual**: README.md:193 still says "eight task skills" (TODO-004); the wiring guard still misses the two new steps (TODO-005); the AGENTS.md CI table is stale (TODO-006); no Windows engine run or hosted run for the tutorials yet.
- **Related**: DEC-008, EVD-034, EVD-035, EVD-037, EVD-038

### DEC-008: Round 4 closes RQ-003 and OQ-002

- **Decision**: C3 is `Met`. OQ-002 (cargo self-location) is closed by EVD-034. C6 and C9 are extended to cover the two new runners.
- **Owner**: agent
- **Status**: approved
- **Rationale**: VAL-006. The four-category vocabulary is now consistent across `editorial.md`, the reference template, the contributor guide, the DSP and Real pages and all four tutorials.
- **Alternatives considered**: closing C3 on the reference pages alone (rejected in round 3; the tutorials were the named half and are now done).
- **Related**: VAL-006, C3, C6, C9, OQ-002

### OBS-008: The widened scanner caught a broken link introduced in the same round

- **Source**: `bash tools/check-docs.sh` during round-5 verification
- **Fact**: The scanner now covers `.dsh/skills/**`, and its first run on the widened scope failed with `.dsh/skills/debug-desktop-issue/SKILL.md:107: broken link '../../docs/testing.md'`. The file is modified in the worktree by the round-5 instruction-audit agent, and the link does not exist at HEAD, so the defect was introduced this round: from `.dsh/skills/debug-desktop-issue/`, `../..` lands in `.dsh/`, not the repository root.
- **Implications**: The requirement works — a link broken minutes earlier was caught by the check RQ-015 asked for. It also shows why RQ-022's advice is not enough on its own: an agent auditing links can still add one.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: EVD-041, RQ-015, RQ-022, C15, C22

### OBS-009: `AGENTS.md` still carries the retired three-category vocabulary

- **Source**: `AGENTS.md:210`
- **Fact**: The contributor guide says "every numerical claim is labelled exact, precision-bounded or parity", while `docs/editorial.md:27` now defines four categories and names the second one "precision-**limited**". The round-4 conversion covered `docs/`, but not this file.
- **Implications**: An agent reading AGENTS.md applies a different rule from the one the `--claims` checker enforces. This is exactly the "agent instructions are executable" failure the editorial guide warns about.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: EVD-045, RQ-022, RQ-003, C22

### VAL-007: Round 5a verified; C15 and C16 close

- **Target**: round-5 claim batch (`879ecc5d`)
- **Method**: Ran the widened scanner and read its class counts; mutated a README anchor and an unlisted reference page; ran the reference manifest; ran the 26-mutation self-test; confirmed the live broken link was a regression and then that it was fixed.
- **Evidence examined**: EVD-041 (70 pages, 48 anchors, anchor mutation caught), EVD-042 (27 scoped pages, unlisted page caught), EVD-043 (live regression caught and fixed), EVD-044 (26 mutations, subset wiring guard).
- **Result**: Supported
- **Conclusion**: RQ-015 and RQ-016 are satisfied by checks that demonstrably fail — and the scanner paid for itself on its first run by catching a link broken minutes earlier in another agent's file.
- **Residual**: the slug rule is an ASCII approximation of GitHub's algorithm, and `--symbols` remains a token search (the help text now says so). Both are recorded limitations, not claims.
- **Related**: DEC-009, EVD-041, EVD-042, EVD-043, EVD-044

### DEC-009: Round 5 closes RQ-015 and RQ-016; TODO-005 done

- **Decision**: C15 and C16 are `Met`. TODO-005 (wiring guard blind spot) is closed by EVD-044.
- **Owner**: agent
- **Status**: approved
- **Rationale**: VAL-007. Both new capabilities were falsified with real mutations and restored byte-identically, and the guard now asserts every mandatory docs step.
- **Alternatives considered**: marking C15 met while the tree was red on the regression link (rejected: the gate would have been green only because the failing file was in another agent's scope; the fix landed and all six gates now pass).
- **Related**: VAL-007, C15, C16, TODO-005

### OBS-010: The `AGENTS.md` vocabulary is being corrected in-round

- **Source**: `AGENTS.md:210`
- **Fact**: The instruction-audit agent was told to replace the three-category sentence with the four-category names and to grep `AGENTS.md`, the Copilot instructions and `.dsh/skills/**` for surviving `precision-bounded` wording.
- **Implications**: A cross-file vocabulary check does not yet exist; this was found by reading, not by a gate. That gap belongs to RQ-022's inventory half and is recorded as TODO-007.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: OBS-009, TODO-007, C22

### TODO-007: Add a cross-file vocabulary check

- **Task**: Extend `tools/check-docs.sh --claims` (or a new mode) to fail when a retired claim-vocabulary term appears in current documentation, agent instructions or skills — for example `precision-bounded` outside a historical record, or a three-category list where four are defined.
- **Priority**: P2
- **Depends on**: OBS-009
- **Status**: Open
- **Related**: OBS-009, OBS-010, RQ-022, C22

### VAL-008: Round 5 is verified end to end

- **Target**: round-5 claim batches (`879ecc5d`, `26c429fb`)
- **Method**: Ran the language runner clean; renamed a documented builtin and read both diagnostics; re-ran all six gates; read the two corrected claims in `README.md` and `AGENTS.md`.
- **Evidence examined**: EVD-041…EVD-048.
- **Result**: Supported
- **Conclusion**: RQ-014, RQ-015, RQ-016 and RQ-022 are satisfied by checks that fail for their intended reasons. The catalogue check is the strongest new artefact: it compares name, category, arity and parameters in both directions against the running binary, and a single renamed row produced two independent diagnostics.
- **Residual**: RQ-017 and RQ-022 still have survivors — the tutorials/how-to "none exist yet" note and three `precision-bounded` occurrences (EVD-048). No gate catches the retired vocabulary yet (TODO-007).
- **Related**: DEC-010, EVD-045, EVD-047, EVD-048

### DEC-010: Round 5 closes RQ-014, RQ-015, RQ-016 and RQ-022; RQ-017 stays partial

- **Decision**: C14, C15, C16 and C22 are `Met`. C17 is `Partial` on EVD-048.
- **Owner**: agent
- **Status**: approved
- **Rationale**: VAL-008. Closing C17 while two current pages describe the tutorials as non-existent, and three carry the retired term, would be exactly the "obsolete current assertion" the requirement names.
- **Alternatives considered**: treating `docs/adr/004:100` as exempt because it is an ADR (rejected: it is the *correction* section describing current practice, not a historical quotation; the word there should be the current one).
- **Related**: VAL-008, C14, C15, C16, C17, C22

### TODO-008: Close the remaining C17 items

- **Task**: `docs/editorial.md:71-72` — tutorials and how-to pages are described as "target shape; none exist yet"; the four tutorials exist, so point at them. Replace `precision-bounded` with `precision-limited` at `docs/adr/004-thirty-digit-default.md:100`, `docs/crates/lovelace-bench.md:45` and `docs/crates/lovelace-lang.md:58`.
- **Priority**: P1
- **Depends on**: none
- **Status**: Open
- **Related**: EVD-048, RQ-017, C17

### VAL-009: Round 6a verified; RQ-017, RQ-019, RQ-020 and RQ-021 close

- **Target**: round-6a claim batch (`c442138f`)
- **Method**: Grepped for the two C17 remnants, read the provenance paragraph, read the two navigation entry points, and ran all seven gates.
- **Evidence examined**: EVD-049 (both C17 remnants gone), EVD-050 (provenance split), EVD-051 (navigation), EVD-052 (seven gates green, archive present).
- **Result**: Supported
- **Conclusion**: The last obsolete-assertion items are gone and the reference now distinguishes "authored against" from "checked on this branch" — the exact distinction RQ-021 asks for.
- **Residual**: `tools/check-docs.sh`'s own diagnostics still use the retired term in a message string (not documentation), and no gate yet rejects the retired vocabulary (TODO-007).
- **Related**: DEC-011, EVD-049, EVD-050, EVD-051

### DEC-011: Round 6a closes RQ-017, RQ-019, RQ-020 and RQ-021

- **Decision**: C17, C19, C20 and C21 are `Met`.
- **Owner**: agent
- **Status**: approved
- **Rationale**: VAL-009. The ADR-004 judgement call is accepted: its item 3 prescribes *current* reference-doc practice, so the current term belongs there, while the quoted `"exact 30-place decimal"` is genuine history and stays.
- **Alternatives considered**: treating ADR-004:100 as an exempt historical quotation (rejected by the implementer with a reason, and I agree — the sentence prescribes what to do now).
- **Related**: VAL-009, C17, C19, C20, C21

### OBS-012: Hosted Windows found a path-translation bug no local run could

- **Source**: GitHub run 43878298949, job `Documentation examples (windows-latest)`
- **Fact**: The step `Compile and run the crate-page examples` failed with `failed to read C:\d\a\RustLace\RustLace\lovelace-array\Cargo.toml`. The generated temporary `Cargo.toml` carried POSIX paths (`/d/a/RustLace/...`); Git Bash translates argv for native binaries but not file contents, so native Windows cargo read `/d/...` as `C:\d\...`. The other three jobs — Windows workspace, Linux, and Ubuntu docs — were green.
- **Implications**: This is precisely the class RQ-013.4 names ("checkout/CRLF, Git Bash, binary lookup, path, or shell discrepancies"). It is invisible on Linux by construction, and it is the second failure hosted CI has caught that local gates called green.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: EVD-057, RQ-013.4, TODO-010, C13

### TODO-010: Make generated Cargo manifests platform-native

- **Task**: In `tools/check-crate-examples.sh` (and any sibling that writes a path into a file), convert the path with `cygpath -w` when available so the generated manifest is `D:\a\RustLace\...` on Git Bash, leaving Linux untouched.
- **Priority**: P0 (blocks C13)
- **Depends on**: none
- **Status**: Done in round 8, reproduced against a real Git Bash host in round 9, and confirmed by a
  hosted Windows run in the same round. The generated manifest now records
  `F:\\dev\\RustLace\\lovelace-array`, which decodes to `F:\dev\RustLace\lovelace-array` and resolves
  through the native Windows API; the pre-fix `/f/dev/RustLace/...` shape does not resolve there.
  Hosted run 43902955424 at `b0ed8f5` then passed the step that had failed.
- **Related**: OBS-012, OBS-013, EVD-057, EVD-059, RQ-013.4, C13

### DEC-001: All 24 requirements gate completion

- **Decision**: Every requirement RQ-001…RQ-024 must be met with evidence. No P2/P3 item may close as an "accepted residual".
- **Owner**: user
- **Status**: approved
- **Rationale**: The intent audit's single question. The user's §7 permits P2/P3 residuals to be "explicitly documented, prioritized and accepted", but the user chose the stricter reading when asked directly. The stop criteria in `goal.md` were rewritten accordingly, and the objective sentence now says "no requirement is closed as a residual".
- **Alternatives considered**: P1-only gating (the reading §7 appears to allow); RQ-001–016 gating with RQ-017–024 residual.
- **Related**: OQ-001, C1–C24

### OBS-001: The Intent Auditor confirmed 27/27 mechanisms KEPT and found four ledger defects

- **Source**: `docs/goal-docs-v2/intent-audit.md`
- **Fact**: All 24 requirements and the three structural claims (branch/revision, no arithmetic regression, no-regressions list) are KEPT. It also found: the ledger mislabelled RQ-019 as verbatim while dropping "turning documentation into"; C20 omitted RQ-020's reference-navigation half; C18 and C23 under-covered DSP wording and machine-readable output; and the header revision is the *reviewed* revision while C13 demands evidence at the *final* commit.
- **Implications**: The first three were fixed in `goal.md` before round 1. The fourth is resolved by reading C13 as "the final commit of this goal", recorded here.
- **Confidence**: High
- **Agent**: Intent Auditor + orchestrator
- **Related**: DEC-001, C13, C18, C20, C23

### OQ-001: Which commit is "the final commit" for RQ-013?

- **Question**: RQ-013 requires hosted runs at the final reviewed commit, but the branch will gain commits during this goal. Is the final commit the last commit of this goal's work (i.e. CI is run after the code is frozen)?
- **Needed evidence**: none outstanding — resolved by DEC-001's sequencing: all code and documentation work lands first, then one commit is pushed and a PR opened, and C13 is judged at that commit.
- **Priority**: P1 important
- **Raised by**: Round 0, Intent Auditor
- **Related**: RQ-013, C13

### OBS-013: Hosted run 43902955424 at `b0ed8f5` is green 4/4 and closes C13

- **Source**: GitHub Actions run 43902955424, ref `docs/documentation-quality-remediation`, head
  SHA `b0ed8f5e27f6223c105e1ceed5d5dbcbcfddb522`
- **Fact**: All four jobs succeeded — `Windows (full workspace)` 8m8s (job 133424152063), `Linux
  (engine crates + GUI desktop under Xvfb)` 5m18s (job 133424152489), `Documentation examples
  (ubuntu-latest)` 1m10s (job 133424152456), `Documentation examples (windows-latest)` 6m13s (job
  133424153047). The Windows documentation job passed `Compile and run the crate-page examples`,
  the step that failed at `89ea5c7` (EVD-057), and both documentation transcripts end with
  `DOCREPORT total gates=4 ok=4 failed=0 commit=b0ed8f5` (`os=Linux` and
  `os=MINGW64_NT-10.0-26100`).
- **Implications**: C13 is met and the definition of done reaches 24/24. The failure that hosted
  CI found at `89ea5c7` is now a passing step at the next commit, on the same host spelling
  (`/d/a/RustLace/RustLace`) that produced it.
- **Confidence**: High
- **Agent**: orchestrator
- **Related**: EVD-059, EVD-060, EVD-061, RQ-013, OBS-012, C13

### OQ-002: Does C13 require a pull request, or is a dispatched run enough?

- **Question**: OQ-001's resolution text says "one commit is pushed and a PR opened", but C13's
  expected observable is "`gh run list` / `gh pr checks` at the final commit". The user asked for
  a branch to inspect, not for a merge.
- **Needed evidence**: none outstanding — the check names both routes. Run 43902955424 was produced
  by `workflow_dispatch` on the branch, so `gh run list` answers it; no pull request was opened,
  because C13 does not require one and the user asked to inspect the branch rather than merge it.
  Opening a PR remains available to the user and would trigger the same `pull_request` jobs.
- **Priority**: P3 minor
- **Raised by**: Round 9, on reading OQ-001 against C13
- **Related**: RQ-013, C13, OQ-001

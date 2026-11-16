# Round 4 — CI wiring and cargo self-location (RQ-006, RQ-009, RQ-010, OQ-002)

- **Date:** 2026-11-16 (America/Sao_Paulo)
- **Branch head at the run:** `d9463162fd27cf8a6c485ef14f3867ee6219262a` (`d946316`, working tree
  dirty by design; this report is an uncommitted deliverable)
- **Owner files:** `.github/workflows/ci.yml`, `tools/check-crate-examples.sh`, `docs/release.md`,
  this report. Nothing under `docs/tutorials/**`, `tools/run-tutorials.sh`, `tools/test-docs-gates.sh`,
  `tools/check-docs.sh`, `tools/check-reference-pages.sh`, `docs/testing.md`, `AGENTS.md` or
  `docs/goal-docs-v2/**` (other than this file) was edited.

## 1. What changed

### 1.1 `tools/check-crate-examples.sh` locates cargo itself (OQ-002)

The single `cargo_bin=${CARGO:-cargo}` check that exited 2 when cargo was not on `PATH` is replaced
by a fixed-resolution resolver, printed on every run so CI logs show which compiler was used:

1. `$CARGO`, when it is set and executable (a path that passes `[ -x ]`, or a name `command -v`
   resolves);
2. `command -v cargo` — cargo on `PATH`;
3. `$HOME/.cargo/bin/cargo` — the rustup default on Unix;
4. `$USERPROFILE/.cargo/bin/cargo.exe` — the rustup default in Git Bash; `USERPROFILE` is run
   through `cygpath -u` when Git Bash provides it, so `[ -x ]` and `exec` see a POSIX path.

`$CARGO` still wins when it is valid; if it is set but unusable the script says so on stderr and
falls through instead of using a broken compiler. When all four fail, the script exits 2 and names
every place it tried plus the rustup install hint. It never proceeds without a compiler and never
turns a missing compiler into a pass. The header comment now documents this order.

### 1.2 Both runners are unconditional steps in the `docs` job

Inserted after `Build lovelace-run` and before `Run the language and extension examples`; no
existing step was removed or reordered. The five existing steps (the three `check-docs.sh` modes,
the reference-page check and the mutation self-test) and their `--symbols` / `--claims` / `--readme`
flags are untouched.

- `Compile and run the crate-page examples` → `bash tools/check-crate-examples.sh`
- `Run the tutorials` → `bash tools/run-tutorials.sh`

Both run in the one `docs` definition, whose matrix is `[ubuntu-latest, windows-latest]`, so both
run twice. `shell: bash` is set explicitly because the default shell on `windows-latest` is
PowerShell: GitHub's `bash` is Git Bash there and the system bash on Ubuntu, and these two scripts
are bash only. Neither step has `if:`, `continue-on-error`, or `|| true`. The workflow's header
comment now names the crate-page and tutorial runners.

### 1.3 `docs/release.md` stays truthful

- The `What must be green before a tag` table row for `Documentation examples (ubuntu-latest)` now
  names **Compile and run the crate-page examples** (`bash tools/check-crate-examples.sh`) and
  **Run the tutorials** (`bash tools/run-tutorials.sh`) — the exact step names added in `ci.yml` —
  and the Windows row still says "the same documentation gate on Windows".
- The paragraph below the table now lists a crate-page Rust fence that no longer compiles and a
  tutorial whose execution no longer matches its recorded output among the defects that block a tag.
- Release checklist item 10 and its command block now include `bash tools/check-crate-examples.sh`
  and `bash tools/run-tutorials.sh`, and state the expected end states (`every crate-page Rust fence
  compiles and runs`, `run-tutorials: OK`).

## 2. Full final `docs` job block from `.github/workflows/ci.yml`

```yaml
  # The documentation examples are executable documentation: each runner parses
  # the fenced blocks out of the Markdown and compares every observed typed
  # output with the recorded one. The matrix is the point - a runner that only
  # works on one platform (path shapes, .exe discovery, line endings) must fail
  # on the platform that has the bug, not pass because it was never run there.
  # The consistency gates and their self-test run in both variants for the same
  # reason: the checkers are bash, so a line-ending or path-shape defect must
  # fail on the platform that has it.
  docs:
    name: Documentation examples (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest]
    steps:
      # windows-latest checks out with the runner's system core.autocrlf=true,
      # which would give the bash runners CRLF and break them before they run.
      # Pin LF for this job's checkout only; the windows and linux jobs above
      # keep their existing checkout behaviour.
      - name: Checkout
        uses: actions/checkout@v4
        env:
          GIT_CONFIG_COUNT: '1'
          GIT_CONFIG_KEY_0: core.autocrlf
          GIT_CONFIG_VALUE_0: 'false'

      - name: Install stable Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo registry and target
        uses: Swatinem/rust-cache@v2

      - name: Build lovelace-run
        run: cargo build -p lovelace-run

      # Compiles and runs every maintained ```rust fence under docs/crates/
      # against the workspace crates. The script locates cargo itself ($CARGO,
      # then PATH, then $HOME/.cargo/bin/cargo, then
      # %USERPROFILE%/.cargo/bin/cargo.exe in Git Bash), so the gate cannot
      # pass by not finding a compiler: it exits 2 instead. `shell: bash` is
      # the system bash on ubuntu-latest and Git Bash on windows-latest; both
      # variants run this step unconditionally.
      - name: Compile and run the crate-page examples
        shell: bash
        run: bash tools/check-crate-examples.sh

      # Executes the four tutorials as contracts: each page's ## Source block
      # is run through the lovelace-run built above and its output is compared
      # with the recorded ## Output block. Unconditional, like every other
      # gate here; the runner is found under target/{debug,release}.
      - name: Run the tutorials
        shell: bash
        run: bash tools/run-tutorials.sh

      # `shell: bash` is the system bash on ubuntu-latest and Git Bash on
      # windows-latest. Both runners always execute; the step fails if either
      # exits non-zero, so a wrong recorded output fails the workflow.
      - name: Run the language and extension examples
        shell: bash
        run: |
          set -uo pipefail
          status=0
          bash docs/language/examples/run-examples.sh || status=1
          bash docs/extensions/examples/run-examples.sh || status=1
          exit "$status"

      # The documentation consistency gate has three implemented modes, and all
      # three run. check-docs.sh always performs the relative-link and
      # path-shaped-code-reference pass; each flag adds one audit on top of it:
      #   --symbols  no .rs:NNN citations outside docs/adr/ and no dead symbols
      #   --claims   no prohibited absolutist claim, every tutorial has ## Precision
      #   --readme   README value proposition, limitations, command and order
      # Separate steps keep a failure attributable to one mode. No step is
      # conditional and none is allowed to fail without failing the job.
      - name: Check documentation links and symbol citations
        shell: bash
        run: bash tools/check-docs.sh --symbols

      - name: Check documentation claims and tutorial precision
        shell: bash
        run: bash tools/check-docs.sh --claims

      - name: Check the README structure
        shell: bash
        run: bash tools/check-docs.sh --readme

      # The reference-page contract (DOC-014): every page listed in
      # docs/reference-pages.md carries the seven sections of
      # docs/reference-template.md, in order, with no other level-2 heading.
      - name: Check reference pages
        shell: bash
        run: bash tools/check-reference-pages.sh

      # Each gate above has a recorded mutation test: tools/test-docs-gates.sh
      # builds the defect in a scratch tree, runs the real checker against it,
      # and requires a non-zero exit with a diagnostic naming the file and the
      # problem. A mutation that stops failing fails this step, so a gate
      # cannot quietly become a no-op. It also asserts that this workflow still
      # invokes every mode, so the self-test goes red when a mode is removed
      # from the job instead of passing on a smaller set.
      - name: Test the documentation gates (mutation)
        shell: bash
        run: bash tools/test-docs-gates.sh
```

Structural check with PyYAML (workflow parses, step order, no conditional skip):

```text
$ python3 <scratch PyYAML check of .github/workflows/ci.yml>
ci.yml parses; jobs: windows, linux, docs
docs matrix os: ['ubuntu-latest', 'windows-latest']
 1. Checkout  [shell=-]
 2. Install stable Rust  [shell=-]
 3. Cache cargo registry and target  [shell=-]
 4. Build lovelace-run  [shell=-]
 5. Compile and run the crate-page examples  [shell=bash]
 6. Run the tutorials  [shell=bash]
 7. Run the language and extension examples  [shell=bash]
 8. Check documentation links and symbol citations  [shell=bash]
 9. Check documentation claims and tutorial precision  [shell=bash]
10. Check the README structure  [shell=bash]
11. Check reference pages  [shell=bash]
12. Test the documentation gates (mutation)  [shell=bash]
docs job: no if/continue-on-error/|| true
```

## 3. Self-location transcript (cargo NOT on `PATH`, no `CARGO`)

Run through WSL Ubuntu, from a shell whose `PATH` was reduced to the system directories and whose
`CARGO` was unset. `CARGO_TARGET_DIR=/tmp/rl-target` only reuses the workspace build cache; it has no
effect on cargo discovery.

```text
--- shell probe ---
PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
CARGO=<unset>
cargo on PATH: no
--- gate ---
check-crate-examples: cargo: /home/ricardo/.cargo/bin/cargo (from $HOME/.cargo/bin/cargo)
PASS docs/crates/lovelace-array.md:83
PASS docs/crates/lovelace-int.md:71
PASS docs/crates/lovelace-json.md:69
PASS docs/crates/lovelace-lang.md:95
PASS docs/crates/lovelace-nat.md:87
PASS docs/crates/lovelace-real.md:100
PASS docs/crates/lovelace-symbolics.md:84
EXCLUDED (none)
check-crate-examples: 13 page(s), 7 rust fence(s): 7 compiled and ran, 0 excluded
check-crate-examples: OK
exit=0
```

The first line is the proof: the shell reported `cargo on PATH: no`, and the script still found
`/home/ricardo/.cargo/bin/cargo` and ran all 7 fences. The exact command form was the equivalent of

```console
$ env -u CARGO PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
    CARGO_TARGET_DIR=/tmp/rl-target bash tools/check-crate-examples.sh
```

### 3.1 `$CARGO` override still wins, from the same stripped `PATH`

```text
=== case 1: CARGO override, cargo absent from PATH ===
check-crate-examples: cargo: /home/ricardo/.cargo/bin/cargo (from $CARGO)
PASS docs/crates/lovelace-array.md:83
PASS docs/crates/lovelace-int.md:71
PASS docs/crates/lovelace-json.md:69
PASS docs/crates/lovelace-lang.md:95
PASS docs/crates/lovelace-nat.md:87
PASS docs/crates/lovelace-real.md:100
PASS docs/crates/lovelace-symbolics.md:84
EXCLUDED (none)
check-crate-examples: 13 page(s), 7 rust fence(s): 7 compiled and ran, 0 excluded
check-crate-examples: OK
exit=0
```

### 3.2 Nothing found: exit 2, every place named

With `HOME=/nonexistent-home`, `USERPROFILE` unset, no `CARGO`, and the stripped `PATH`:

```text
check-crate-examples: cargo not found; tried, in order:
  1. CARGO=(unset)
  2. cargo on PATH
  3. /nonexistent-home/.cargo/bin/cargo
  4. <USERPROFILE unset>/.cargo/bin/cargo.exe (Git Bash)
check-crate-examples: set CARGO to the cargo executable, put cargo on PATH,
check-crate-examples: or install Rust from https://rustup.rs
exit=2
```

### 3.3 Git Bash on Windows: the script parses and refuses to pass without a compiler

This Windows host has no Rust toolchain in Git Bash (`cargo on PATH: no`, no `cargo.exe` under
`HOME` or `USERPROFILE`), so a real Windows build run was **not** possible here. Git Bash does
execute the edited script through to the resolver and reports the Git Bash fallback location:

```text
shell: bash 5.2.37(1)-release on MINGW64_NT-10.0-26100
cargo on PATH: no
HOME=/c/Users/ricar
USERPROFILE=C:\Users\ricar
no cargo.exe under HOME or USERPROFILE
--- gate ---
exit=2
[stderr]
check-crate-examples: cargo not found; tried, in order:
  1. CARGO=(unset)
  2. cargo on PATH
  3. /c/Users/ricar/.cargo/bin/cargo
  4. C:\Users\ricar/.cargo/bin/cargo.exe (Git Bash)
check-crate-examples: set CARGO to the cargo executable, put cargo on PATH,
check-crate-examples: or install Rust from https://rustup.rs
```

This proves the MSYS dialect parses and that a missing compiler is exit 2 on Windows too. It does
**not** prove a Windows build of the 7 fences; that evidence must come from the hosted
`windows-latest` run (RQ-013), and is not claimed here.

## 4. Existing gates after the edit

### 4.1 `bash tools/check-reference-pages.sh`

```text
check-reference-pages: 16 page(s), 112 required sections in order
check-reference-pages: OK
```

### 4.2 `bash tools/check-docs.sh` base and all three modes (extra, because `docs/release.md` changed)

```text
check-docs: 57 Markdown files, 171 relative links, 241 code references
check-docs: OK
check-docs: 57 Markdown files, 171 relative links, 241 code references, 20 line-suffix citations, 20 symbol citations
check-docs: OK
check-docs: 57 Markdown files, 171 relative links, 241 code references
check-docs: claims audit: 89 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: OK
check-docs: 57 Markdown files, 171 relative links, 241 code references
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
```

The counts are from the final re-run; another agent was editing `docs/tutorials/**` concurrently, so
the page and link totals move independently of this change. Every mode exited 0.

### 4.3 `bash tools/test-docs-gates.sh` (mutation self-test)

Run with `CARGO_TARGET_DIR=/tmp/rl-target` so it used the already-built runner.

```text
SETUP lovelace-run: /tmp/rl-target/debug/lovelace-run
SETUP scratch: /tmp/test-docs-gates.kTCTGd
CTRL check-docs base: broken-link fixture: clean fixture passed
PASS check-docs: broken relative link (exit 1)
CTRL check-docs base: missing-code-reference fixture: clean fixture passed
PASS check-docs: path-shaped code reference to a nonexistent file (exit 1)
CTRL check-docs --symbols: .rs:NNN fixture: clean fixture passed
PASS check-docs --symbols: .rs:NNN citation outside docs/adr/ (exit 1)
CTRL check-docs --symbols: live-symbol fixture: clean fixture passed
PASS check-docs --symbols: dead symbol citation (exit 1)
CTRL check-docs --claims: phrase fixture: clean fixture passed
PASS check-docs --claims: prohibited absolutist phrase (exit 1)
CTRL check-docs --claims: tutorial fixture: clean fixture passed
PASS check-docs --claims: tutorial missing '## Precision' (exit 1)
CTRL check-docs --readme: README fixture: clean fixture passed
PASS check-docs --readme: README missing '## Current limitations' (exit 1)
CTRL check-reference-pages: conformant fixture: clean fixture passed
PASS check-reference-pages: missing required section (exit 1)
CTRL check-reference-pages: ordered fixture: clean fixture passed
PASS check-reference-pages: out-of-order section (exit 1)
CTRL language runner: declared-set fixture: clean fixture passed
PASS language runner: undeclared page in the manifest (exit 1)
CTRL language runner: complete-page-set fixture: clean fixture passed
PASS language runner: declared page missing on disk (exit 1)
CTRL language runner: one-example fixture: clean fixture passed
PASS language runner: zero example pairs (exit 1)
CTRL language runner: paired fixture: clean fixture passed
PASS language runner: unpaired lovelace block (exit 1)
CTRL extensions runner: declared-set fixture: clean fixture passed
PASS extensions runner: undeclared page in the manifest (exit 1)
CTRL extensions runner: complete-page-set fixture: clean fixture passed
PASS extensions runner: declared page missing on disk (exit 1)
CTRL extensions runner: one-example fixture: clean fixture passed
PASS extensions runner: zero example pairs (exit 2)
CTRL extensions runner: paired fixture: clean fixture passed
PASS extensions runner: unpaired lovelace block (exit 1)
WIRE ci.yml: three check-docs modes, reference pages, both example runners, the gate self-test, and the Ubuntu+Windows matrix are present; the docs job has no conditional skip

test-docs-gates: 17 mutation(s), 17 passed, 0 failed
test-docs-gates: OK (17 mutations, every gate failed for its intended reason)
```

## 5. `tools/run-tutorials.sh` existed when this work finished

It **did** exist — 30,582 bytes, 815 lines, mtime `2026-11-16 09:39`, sha256
`5288bdf47f6c2bc2a1776d051c8faed1061f626cd032db614944f3d5c16c4eba`. The step was added regardless,
unconditionally, as required. It was run on Ubuntu at this snapshot and passed:

```text
run-tutorials: expecting 4 tutorials: linear-algebra symbolic signal-processing reproducible
run-tutorials: runner: /tmp/rl-target/debug/lovelace-run
PASS linear-algebra: docs/tutorials/linear-algebra.md Source -> Output match
ORACLE linear-algebra: det(A) = 5; A * sol = b exactly at scale 10^1; gap = [0, 0]
PASS symbolic: docs/tutorials/symbolic.md Source -> Output match
PASS signal-processing: docs/tutorials/signal-processing.md Source -> Output match
PASS reproducible: docs/tutorials/reproducible.md fields display/kind/exact/plot match; plot artifact exists outside the repository

run-tutorials: 4 tutorials checked (linear-algebra symbolic signal-processing reproducible)
run-tutorials: 1 independent oracle(s)
run-tutorials: OK
```

Caveat: `tools/run-tutorials.sh` and `docs/tutorials/**` were owned by another agent this round and
were still being changed (the `.lv` → `.ls` rename was not yet in the tree when this run was made),
so this transcript is a snapshot, not a claim about the final content of those files. The CI step
only fixes the command (`bash tools/run-tutorials.sh`); whatever that script asserts at the merge
commit is what both OS variants will run. It was not guarded with `if` or `continue-on-error`.

## 6. Limits and residuals

1. **Windows build of the new steps not locally reproduced.** Git Bash is present, but this host has
   no Windows Rust toolchain, so `windows-latest` execution is evidenced only by the workflow
   declaration, the PyYAML step listing, and the Git Bash resolver/error-path run. Hosted
   `windows-latest` and `ubuntu-latest` runs at a named commit are RQ-013's evidence and remain
   outstanding.
2. **`tools/test-docs-gates.sh` does not yet assert the two new steps.** Its `check_ci_wiring`
   function (out of scope this round) verifies the three `check-docs` modes, the reference-page
   check, both language/extension example runners, the matrix, and the absence of `if:`,
   `continue-on-error` and `|| true`. It does not check for `bash tools/check-crate-examples.sh` or
   `bash tools/run-tutorials.sh`, and its success line still says "both example runners". A future
   removal of either new CI step would therefore not be caught by the self-test. Owner of
   `tools/test-docs-gates.sh` should extend that literal list.
3. **`AGENTS.md` CI jobs table** (out of scope) still lists only the old documentation-job commands;
   it should be updated by its owner to name the crate-page and tutorial steps.
4. The `docs` job's top-level comment now describes the full set of executable documentation, but
   `docs/release.md` is the only release-facing document updated here; the release skill
   (`.dsh/skills/release-and-publish/SKILL.md`) is owned elsewhere.

## 7. Requirement mapping for this report

| Requirement | Evidence here |
|---|---|
| RQ-006 (C6) — every implemented documentation-consistency mode in CI | The docs job block shows all three `check-docs.sh` modes, `check-reference-pages.sh`, the mutation self-test, and now the crate-page and tutorial runners, on both matrix variants, with no conditional step |
| RQ-009 (C9) — all four tutorials as executable CI contracts | `Run the tutorials` step plus the clean `run-tutorials` transcript (4/4 PASS, 1 oracle) |
| RQ-010 (C10) — compile and execute public Rust reference examples | `Compile and run the crate-page examples` step plus the 7/7 fence transcript; the gate is self-locating (OQ-002) and still fails on drift |
| OQ-002 — cargo self-location | Resolution order, `$CARGO` override transcript, all-places error transcript, and the stripped-`PATH` pass |
| RQ-012 (C12) — release instructions match actual CI | `docs/release.md` names the two new steps in the tag gate table, paragraph and checklist; step names match `ci.yml` exactly |

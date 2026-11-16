# Round 4 implementation: cross-platform documentation example runners

RustLace 0.4.1, documented on 2026-11-15. Host: Windows; every command below was executed through
WSL Ubuntu, with the repository mounted at `/mnt/c/Users/ricar/dev/RustLace`. The runner was the
already-built debug binary `/tmp/rl-target/debug/lovelace-run`, selected by the scripts through
`CARGO_TARGET_DIR=/tmp/rl-target`, which also exercises the discovery rule this task added.
`pwsh` is not installed in WSL, so the two existing `.ps1` runners were read for semantics and not
executed there; see [What could not be verified](#what-could-not-be-verified).

**Examples executed: the language runner executed 193 examples across 10 pages; the extension
runner executed 27 examples across 4 pages (220 total).**

## What changed

| Path | Change |
|---|---|
| `docs/language/examples/run-examples.sh` | New bash runner for the 10 `docs/language` pages (239 lines). |
| `docs/extensions/examples/run-examples.sh` | New bash runner for all `docs/extensions/*.md` pages (227 lines). |
| `.github/workflows/ci.yml` | New `docs` job, matrix `ubuntu-latest` + `windows-latest`; existing jobs untouched. |
| `docs/goal-docs-v1/round-4/implementation.md` | This report. |

The two `.ps1` runners were not edited (their CRLF-normalised diff is empty, exit 0). No
documentation Markdown page other than this report, and no `.rs` file, was touched.

## Runner semantics

Both scripts are direct ports of their PowerShell counterparts:

- **Language**: fixed page list, in the same order, as `run-examples.ps1`. A ```` ```lovelace ````
  block's expected output must be the next non-blank fenced block and must be ```` ```text ````;
  anything else (missing block, wrong language, unterminated fence, missing page) is a fatal
  `error: <file>:<line>: ...` and exit 1 rather than a skipped page. Each snippet is written to a
  temporary `.lv` file and run as
  `lovelace-run --file <tmp> --text --plot-dir <tmp>/plots --plot-file case-N.svg`, with the same
  normalisation as `Normalise-Text` (CR stripped, runs of whitespace inside a line collapsed to one
  space, each line trimmed, leading/trailing blank lines dropped). stdout is compared on exit 0,
  stderr on a non-zero exit. Mismatches print the file, the 1-based fence line, the normalised
  expected text and the observed text, then exit 1.
- **Extensions**: every `docs/extensions/*.md` page, split into fenced blocks; a `lovelace` block
  is paired with the `text` block immediately after it. Each snippet runs as
  `lovelace-run --file <tmp> --text --omit-variables`; stdout and stderr are combined and compared
  after `ConvertTo-Normalized` (all whitespace runs collapse to one space, then trim). Exit codes
  are reported but not compared, so intentional error examples pass. An unpaired `lovelace` block
  is listed and exits 1; a run that finds no examples exits 2; an unterminated fence is fatal.
- **Discovery** (both): `${CARGO_TARGET_DIR:-<repo>/target}` then `debug`, then `release`; the
  executable is `lovelace-run`, plus `.exe` only when `uname -s` is MINGW/MSYS/CYGWIN. Neither
  script hard-codes a repository or executable path (the only absolute path is the `/tmp` fallback
  for `mktemp` when `TMPDIR` is unset), and neither has a `LOVELACE_RUN`/`-Exe` override.
- **Scratch space** (both): `mktemp -d` outside the repository, `trap`-removed on every exit,
  with `TMPDIR`/`TEMP`/`TMP`/`XDG_CONFIG_HOME`/`APPDATA` redirected under it. The runners only
  read the repository.

Scope boundary: the bash language runner reproduces the fenced-example gate. The second half of
`run-examples.ps1` (the builtins catalogue cross-check against `--functions --function-limit 500`)
is not ported; it remains in the PowerShell script. It is not a fenced example, so it is outside
this task, but it is stated here rather than left implicit.

## 1. Runner passes

```console
$ wsl.exe -d Ubuntu -- env CARGO_TARGET_DIR=/tmp/rl-target bash docs/language/examples/run-examples.sh; Write-Output "runner-exit=$LASTEXITCODE"
```

Verbatim output:

```text
README.md: 4 example(s) passed
lexical-structure.md: 19 example(s) passed
values-and-types.md: 23 example(s) passed
expressions-and-operators.md: 44 example(s) passed
statements-and-control-flow.md: 26 example(s) passed
functions-and-scope.md: 22 example(s) passed
builtins.md: 25 example(s) passed
errors-and-diagnostics.md: 5 example(s) passed
precision-and-formatting.md: 18 example(s) passed
plots.md: 7 example(s) passed

All 193 examples passed.
runner-exit=0
```

## 2. Runner fails on a corrupted expectation

One recorded output line in `docs/language/errors-and-diagnostics.md` was changed with a
line-addressed `sed` (line 150, `DivisionByZero` -> `DivisionByZer0`). The page was backed up byte
for byte first. The commands and their verbatim output:

```console
$ wsl.exe -d Ubuntu -- cp docs/language/errors-and-diagnostics.md /tmp/rl-falsify-backup.md
$ wsl.exe -d Ubuntu -- sha256sum docs/language/errors-and-diagnostics.md /tmp/rl-falsify-backup.md
050b9608da7b78bd9d892c0202d83e0ef5a79324aeae3eb05a7a0f2f30f3241e  docs/language/errors-and-diagnostics.md
050b9608da7b78bd9d892c0202d83e0ef5a79324aeae3eb05a7a0f2f30f3241e  /tmp/rl-falsify-backup.md
$ wsl.exe -d Ubuntu -- sed -i 150s/DivisionByZero/DivisionByZer0/ docs/language/errors-and-diagnostics.md
$ wsl.exe -d Ubuntu -- sed -n 150p docs/language/errors-and-diagnostics.md
Error [DivisionByZer0/DomainError]: Cannot divide by zero.
$ wsl.exe -d Ubuntu -- env CARGO_TARGET_DIR=/tmp/rl-target bash docs/language/examples/run-examples.sh; Write-Output "runner-exit=$LASTEXITCODE"
README.md: 4 example(s) passed
lexical-structure.md: 19 example(s) passed
values-and-types.md: 23 example(s) passed
expressions-and-operators.md: 44 example(s) passed
statements-and-control-flow.md: 26 example(s) passed
functions-and-scope.md: 22 example(s) passed
builtins.md: 25 example(s) passed

FAIL docs/language/errors-and-diagnostics.md:144 (exit 1): output mismatch
  expected:
    Error [DivisionByZer0/DomainError]: Cannot divide by zero.
  observed:
    Error [DivisionByZero/DomainError]: Cannot divide by zero.
errors-and-diagnostics.md: 4 example(s) passed
precision-and-formatting.md: 18 example(s) passed
plots.md: 7 example(s) passed

1 of 193 example(s) failed
runner-exit=1
```

The gate fails: exit 1, with the file (`docs/language/errors-and-diagnostics.md`), the 1-based
line of the example's ```` ```lovelace ```` fence (144), the expected text and the observed text.

## 3. Runner passes again after restore

```console
$ wsl.exe -d Ubuntu -- cp /tmp/rl-falsify-backup.md docs/language/errors-and-diagnostics.md
$ wsl.exe -d Ubuntu -- sha256sum docs/language/errors-and-diagnostics.md /tmp/rl-falsify-backup.md
050b9608da7b78bd9d892c0202d83e0ef5a79324aeae3eb05a7a0f2f30f3241e  docs/language/errors-and-diagnostics.md
050b9608da7b78bd9d892c0202d83e0ef5a79324aeae3eb05a7a0f2f30f3241e  /tmp/rl-falsify-backup.md
$ wsl.exe -d Ubuntu -- cmp /tmp/rl-falsify-backup.md docs/language/errors-and-diagnostics.md; Write-Output "cmp-exit=$LASTEXITCODE"
cmp-exit=0
$ wsl.exe -d Ubuntu -- sed -n 150p docs/language/errors-and-diagnostics.md
Error [DivisionByZero/DomainError]: Cannot divide by zero.
$ wsl.exe -d Ubuntu -- env CARGO_TARGET_DIR=/tmp/rl-target bash docs/language/examples/run-examples.sh; Write-Output "runner-exit=$LASTEXITCODE"
README.md: 4 example(s) passed
lexical-structure.md: 19 example(s) passed
values-and-types.md: 23 example(s) passed
expressions-and-operators.md: 44 example(s) passed
statements-and-control-flow.md: 26 example(s) passed
functions-and-scope.md: 22 example(s) passed
builtins.md: 25 example(s) passed
errors-and-diagnostics.md: 5 example(s) passed
precision-and-formatting.md: 18 example(s) passed
plots.md: 7 example(s) passed

All 193 examples passed.
runner-exit=0
$ wsl.exe -d Ubuntu -- git -C /mnt/c/Users/ricar/dev/RustLace -c core.autocrlf=true diff --exit-code -- docs/language/errors-and-diagnostics.md; Write-Output "git-residue-diff-exit=$LASTEXITCODE"
git-residue-diff-exit=0
```

No residue: `cmp` is 0, both SHA-256 digests match, and the CRLF-normalised git diff for the page
is empty with exit 0.

## 4. The extension runner's gate also fails when it should

The mandatory falsification above is the language runner. The extension runner was falsified the
same way (line 97 of `docs/extensions/mathir.md`, `#!mathir 2` -> `#!mathir 9`), because a gate
that was never observed failing is not a gate. Excerpt of the failing output (the preceding
`PASS dsp.md:*` lines are unchanged from the passing run):

```text
FAIL  mathir.md:92 (exit 0)
      expected: = #!mathir 9 param x const (rat 1 1) const (rat 2 1) Constant 0 0 Parameter 0 0 Constant 0 1 PowInt 0 1,2 Add 0 0,3
      observed: = #!mathir 2 param x const (rat 1 1) const (rat 2 1) Constant 0 0 Parameter 0 0 Constant 0 1 PowInt 0 1,2 Add 0 0,3

pages: 4   examples checked: 27   passed: 26   failed: 1   unpaired: 0
runner-exit=1
```

After `cp` restore, the SHA-256 of the page matched the backup, `cmp` exited 0, the final pass
ended:

```text
pages: 4   examples checked: 27   passed: 27   failed: 0   unpaired: 0
OK: 27 examples match the documented output.
runner-exit=0
git-residue-diff-exit=0
```

## 5. The `docs` CI job

Quoted from `.github/workflows/ci.yml`:

```yaml
  # The documentation examples are executable documentation: each runner parses
  # the fenced blocks out of the Markdown and compares every observed typed
  # output with the recorded one. The matrix is the point - a runner that only
  # works on one platform (path shapes, .exe discovery, line endings) must fail
  # on the platform that has the bug, not pass because it was never run there.
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
```

Both runners are always attempted; `exit "$status"` makes the step, and therefore the workflow,
fail if either exits non-zero. A failing runner's own output includes the file/line/expected/
observed line.

The checkout EOL pin is not cosmetic: `actions/checkout` on `windows-latest` honours the runner's
system `core.autocrlf=true`, which would check the `.sh` files out as CRLF and break them under Git
Bash before either runner starts. The same failure and fix are documented upstream in
[NVIDIA/NemoClaw@557cc43](https://github.com/NVIDIA/NemoClaw/commit/557cc43ca0cda42473949efec46ccaf2cb9c308d)
and [microsoft/apm@5033ba4](https://github.com/microsoft/apm/commit/5033ba473d1a9661c366f6d6526656bd0cedb0f9).
The pin is scoped to the `docs` job's checkout step; the `windows` and `linux` jobs keep their
existing steps byte for byte (the only other edit in `ci.yml` is the top-of-file comment changing
"Two jobs:" to "Three jobs:" and describing the new job).

Validation performed locally: PyYAML parses the file and reports `jobs: windows, linux, docs`; the
`docs` checkout step has exactly `GIT_CONFIG_COUNT`, `GIT_CONFIG_KEY_0`, `GIT_CONFIG_VALUE_0` as
its only env; `git diff` over `ci.yml` shows additions only (plus the one comment line). A live
GitHub Actions run is not available on this host, so the workflow itself was not executed.

## 6. Portability probes

- **`$CARGO_TARGET_DIR` set** is the evidence above: `/tmp/rl-target/debug/lovelace-run`.
- **Fallback `<repo>/target`**, with no `CARGO_TARGET_DIR`: a symlinked repository outside the
  tree (`/tmp/rl-fakerepo/target/debug/lovelace-run` -> the real binary, pages symlinked) ran both
  scripts. Language: `All 193 examples passed.`; extensions:
  `OK: 27 examples match the documented output.`; both exit 0.
- **Windows `.exe` discovery**: with a `uname` shim reporting `MINGW64_NT-10.0` and a target
  directory containing only `lovelace-run.exe` (no extensionless binary), both scripts ran all
  193 + 27 examples and exited 0. This exercises the suffix branch on Linux; it is not a
  substitute for the real `windows-latest` run.
- **No repository writes**: a full run of both runners was bracketed by a `git status --porcelain`
  hash and an mtime marker. Verbatim output:

```text
language-exit=0 extensions-exit=0
status-before: 6c9f32ea9c6a1d83f392e01c3abbd77681af2fffce8851495096be607ead235a  -
status-after:  6c9f32ea9c6a1d83f392e01c3abbd77681af2fffce8851495096be607ead235a  -
--- repository files written during the runs (expect none) ---
--- scratch directories left behind (expect none) ---
none
```

## 7. Worktree

`git diff --stat` of the three scope files (the two new scripts are marked intent-to-add only for
this measurement; the index was restored immediately afterwards, so they remain untracked):

```text
 .github/workflows/ci.yml                 |  50 ++++++-
 docs/extensions/examples/run-examples.sh | 227 +++++++++++++++++++++++++++++
 docs/language/examples/run-examples.sh   | 239 +++++++++++++++++++++++++++++++
 3 files changed, 515 insertions(+), 1 deletion(-)
```

The single deletion is the `# Two jobs:` -> `# Three jobs:` comment line.

Scoped `git status --short --untracked-files=all` (only the expected paths):

```text
 M .github/workflows/ci.yml
?? docs/extensions/examples/run-examples.sh
?? docs/goal-docs-v1/round-4/implementation.md
?? docs/language/examples/run-examples.sh
```

Nothing else in `docs/language` or `docs/extensions` changed; CRLF-normalised status shows only
the four pages that were already modified before this task started:

```text
 M docs/extensions/README.md
 M docs/extensions/dsp.md
 M docs/language/README.md
 M docs/language/plots.md
?? docs/extensions/examples/run-examples.sh
?? docs/language/examples/run-examples.sh
```

The clone was already dirty at session start: its working tree is CRLF while the index is LF, so
plain `git status` reports every tracked file as modified. `git -c core.autocrlf=true` removes that
noise; every diff quoted above uses it. No corruption remains: both falsified pages have identical
SHA-256 to their backups, `cmp` exits 0, and the normalised diffs are empty.

## What could not be verified

- **`pwsh` is not installed in WSL**, and this host has no Windows Rust toolchain or
  `target/debug/lovelace-run.exe`. The existing `.ps1` runners were therefore read, not executed;
  the compensation is that the bash ports reproduce their parsing, normalisation and exit rules
  line by line, pass on all 220 real examples, and were each shown failing on a corrupted
  expectation and passing again after a byte-exact restore.
- **A real `windows-latest` run** was not possible here. The Windows suffix branch was simulated
  and the checkout's CRLF hazard was handled from documented upstream behaviour, but the workflow
  itself has not run; the first CI run is the remaining check.
- **GitHub Actions** cannot be executed locally; only the YAML was validated.
- The **builtins catalogue cross-check** remains PowerShell-only, as stated under
  [Runner semantics](#runner-semantics).

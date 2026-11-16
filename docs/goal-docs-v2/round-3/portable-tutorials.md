# RQ-008 and RQ-009 — portable tutorials and an executable tutorial contract

**Branch:** `docs/documentation-quality-remediation` · **Measured revision:**
`d9463162fd27cf8a6c485ef14f3867ee6219262a` (`d946316`, "style: apply cargo fmt to the movingavg
and platform-claims changes")
**Date:** 2026-11-16 · **Host:** WSL2, Ubuntu 24.04.4 LTS, x86_64; cargo 1.100.0 (3acc79c0c),
`CARGO_TARGET_DIR=/tmp/rl-target`. Windows host: PowerShell 5.1.26100.4202 and Git Bash
5.2.37(1)-release (x86_64-pc-msys), used only for the shell-plumbing check in §2.6.

**Changed (this task):** `docs/tutorials/README.md`, the four tutorial pages,
`docs/tutorials/examples/{linear-algebra,symbolic,signal-processing,reproducible}.lv` (new),
`tools/run-tutorials.sh` (new), `README.md`.
**Deliberately not changed:** `.github/**` (wiring is round 4, per the task), `docs/editorial.md`,
`docs/extensions/**`, `docs/crates/**`, `docs/language/**`, `docs/testing.md`, `tools/check-docs.sh`,
`tools/check-reference-pages.sh`, `tools/check-crate-examples.sh`, both `run-examples.sh`,
`AGENTS.md`, and `docs/goal-docs-v2/**` other than this report. No `lovelace-*/src` or
`lovelace-*/tests` file was touched: `git diff --name-only -- 'lovelace-*/src' 'lovelace-*/tests'`
is empty.

No fenced `lovelace` block's mathematics was changed. The three text transcripts already matched
the engine; the reproducibility page's transcript keeps the same four fields, with the
host-specific temporary-directory prefix shown as `<temp-dir>` and compared as host information.

---

## 1. RQ-008 — remove host-specific paths from the four tutorials

### 1.1 The portable command form (criterion 2)

Every tutorial now ends with a project-root Cargo command rather than a path into a build
directory:

| Page | Documented verification command |
|---|---|
| `linear-algebra.md` | `cargo run -p lovelace-run -- --file docs/tutorials/examples/linear-algebra.lv --text` |
| `symbolic.md` | `cargo run -p lovelace-run -- --file docs/tutorials/examples/symbolic.lv --text` |
| `signal-processing.md` | `cargo run -p lovelace-run -- --file docs/tutorials/examples/signal-processing.lv --text` |
| `reproducible.md` | `cargo run -p lovelace-run -- --file docs/tutorials/examples/reproducible.lv --omit-functions --omit-variables` plus a field selector (bash + `python3`, or Windows PowerShell + `ConvertFrom-Json`) |

Each page also names `bash tools/run-tutorials.sh`, the portable repository script from RQ-009.
`docs/tutorials/README.md` states the prerequisites once (Rust 1.85+ with `cargo` on `PATH`; Python 3
only for the reproducibility page's bash selector) and no longer mentions a target-directory path.

Every script is a checked-in file under `docs/tutorials/examples/`; there is no here-document to
copy, so the same text runs in bash, Windows PowerShell and Command Prompt. The pages say so, and
the runner enforces it statically (§2.2).

### 1.2 No `/tmp/rl-target` remains (criterion 1)

```console
$ grep -rnF -- '/tmp/rl-target' docs/tutorials README.md
$ echo "[exit=$?]"
[exit=1]
$ grep -rnF -- '/tmp' docs/tutorials
$ echo "[exit=$?]"
[exit=1]
```

Both greps exit 1 (no match). The old Assumptions bullets that named the debug CLI are replaced on
all four pages; the old `/tmp/plots` output path is gone from the reproducibility page.

### 1.3 Temporary output is OS-appropriate, configurable and created on demand (criterion 3)

The reproducibility page documents the engine's actual default, read from
`lovelace-lang/src/lib.rs:831`: a `lovelace-plots` directory under the operating system's temporary
directory (`%TEMP%` on Windows, the system temp root on Linux), created if missing. `--plot-dir
<path>` overrides it, and `lovelace-run/src/main.rs:377` creates that directory with
`create_dir_all` before evaluation.

The runner proves the configurable path and the creation as a side effect of its normal run: it
passes a **non-existent** directory under its `mktemp` scratch root as `--plot-dir`, asserts the
envelope names a file there, and asserts the directory exists afterwards. The clean-run transcript
in §2.1 is that proof; no repository path is used.

### 1.4 No Python, here-document or Unix path is required on Windows (criterion 4)

- The three text tutorials need only `cargo run` and a repository-relative path, identical on both
  platforms.
- The reproducibility page offers two selectors that print the same compact JSON object:
  - bash + `python3` (the page states Python 3 as an explicit prerequisite for this route only);
  - Windows PowerShell 5.1+ + `ConvertFrom-Json`, which needs no Python. The page labels it
    "Windows PowerShell 5.1+ (no Python)".
- No page contains a shell here-document (the runner rejects `<<`), and no page contains a Unix
  temporary path (the runner rejects `/tmp`).
- The signal-processing page's extra length-3 refusal command notes that bash and PowerShell both
  accept single quotes and gives the Command Prompt form.

The PowerShell selector was executed on the Windows host against a real envelope captured by the
Linux engine (the Windows host has no Rust toolchain, so it cannot run `cargo run` itself):

```console
PS> Get-Content -Raw -Encoding UTF8 .tmp\round3\envelope.json | ConvertFrom-Json | ForEach-Object { [pscustomobject]@{ display = $_.result.display; kind = $_.result.kind; exact = $_.result.structured.exact; plot = $_.artifacts[0].path } } | ConvertTo-Json -Compress
{"display":"5","kind":"Real","exact":true,"plot":"/tmp/lovelace-plots/plot.svg"}
```

The object is byte-identical to the Python selector's output for the same envelope; only the
host-specific prefix differs from `<temp-dir>` in the page. The selector is the page's pipeline with
`Get-Content` substituted for the `cargo run` stage; see the limitation in §4.2.

### 1.5 A new reader can run the workflow on both platforms (criterion 5)

The commands are shell-neutral; the plot path is the only host-specific value and the page explains
it. Evidence on the two shells:

- **Ubuntu (WSL):** all four documented commands were executed from the repository root; transcripts
  in §1.6.
- **Windows:** the full runner was executed under Git Bash with a fixture `lovelace-run.exe` and
  real engine-captured outputs, exercising MINGW `.exe` discovery, `cygpath` argument conversion,
  Windows JSON path escaping, Windows path normalisation and the repository-containment check; see
  §2.6. The fixture is plumbing evidence, not an engine run; no Windows Rust toolchain exists on
  this host.

### 1.6 The four tutorials' own Verification commands, from the repository root

Cargo's `Finished`/`Running` lines are written to stderr and are omitted here; stdout is verbatim.
Each tutorial's main command exited 0; the length-3 refusal below exits 1 by design.

**`linear-algebra.md`**

```console
$ cargo run -p lovelace-run -- --file docs/tutorials/examples/linear-algebra.lv --text
= [[5], [6]] (Array)
  A = [[2, 1], [1, 3]]
  Inv = [[0.6, -0.2], [-0.2, 0.4]]
  _ = [[5], [6]]
  b = [[5], [6]]
  d = 5
  gap = [[0], [0]]
  residual = [[5], [6]]
  sol = [[1.8], [1.4]]
```

This is exactly the page's `## Output` block.

**`symbolic.md`**

```console
$ cargo run -p lovelace-run -- --file docs/tutorials/examples/symbolic.lv --text
= 1.4142135623730950488 (Real)
  _ = 1.4142135623730950488
  approx = 1.4142135623730950488
  at_2 = 14
  cancelled = x + 1
  df = 3*x^2 + 2
  f = x^3 + 2*x
  fired = 1
  refused = (x^2 - 1)/(x - 1)
  still_refused = (x^2 - 1)/(x - 1)
  x = x
```

The page's 21-place comparison is also still true:

```console
$ cargo run -p lovelace-run -- --eval 'evalf(sqrt(2), 21)' --text
= 1.414213562373095048801 (Real)
  _ = 1.414213562373095048801
```

**`signal-processing.md`**

```console
$ cargo run -p lovelace-run -- --file docs/tutorials/examples/signal-processing.lv --text
= [36, -4 + 9.65685424949238019520675489683879231427868750150776i, -4 + 4i, -4 + 1.65685424949238019520675489683879231427868750150776i, -4, -4 - 1.65685424949238019520675489683879231427868750150776i, -4 - 4i, -4 - 9.65685424949238019520675489683879231427868750150776i] (Vector)
  _ = [36, -4 + 9.65685424949238019520675489683879231427868750150776i, -4 + 4i, -4 + 1.65685424949238019520675489683879231427868750150776i, -4, -4 - 1.65685424949238019520675489683879231427868750150776i, -4 - 4i, -4 - 9.65685424949238019520675489683879231427868750150776i]
  default_spectrum = [36, -4 + 9.656854249492380195206754896836i, -4 + 4i, -4 + 1.656854249492380195206754896836i, -4, -4 - 1.656854249492380195206754896836i, -4 - 4i, -4 - 9.656854249492380195206754896836i]
  padded_spectrum = [6, -2 - 2i, 2, -2 + 2i]
  signal = [0, 1, 0, 0]
  smoothed = [0, 0.5, 0.5, 0]
  spectrum = [1, -1i, -1, 1i]
  wave = [1, 2, 3, 4, 5, 6, 7, 8]
  wide_spectrum = [36, -4 + 9.65685424949238019520675489683879231427868750150776i, -4 + 4i, -4 + 1.65685424949238019520675489683879231427868750150776i, -4, -4 - 1.65685424949238019520675489683879231427868750150776i, -4 - 4i, -4 - 9.65685424949238019520675489683879231427868750150776i]
```

The length-3 refusal is exactly the page's quote:

```console
$ cargo run -p lovelace-run -- --eval 'fft([1, 2, 3])' --text
Error [InvalidArgument/TypeMismatch]: FFT length must be a power of two, but got 3. (Parameter 'x')
[exit=1]
```

**`reproducible.md`** (bash + `python3` selector)

```console
$ cargo run -p lovelace-run -- --file docs/tutorials/examples/reproducible.lv --omit-functions --omit-variables | python3 -c 'import json, sys; e = json.load(sys.stdin); r = e["result"]; print(json.dumps({"display": r["display"], "kind": r["kind"], "exact": r["structured"]["exact"], "plot": e["artifacts"][0]["path"]}, separators=(",", ":")))'
{"display":"5","kind":"Real","exact":true,"plot":"/tmp/lovelace-plots/plot.svg"}
```

`/tmp/lovelace-plots` is the documenting host's OS temp root; the page records that prefix as
`<temp-dir>` and the runner normalises exactly that prefix (and nothing else about the path except
the separator shape). The plot file exists and begins with an `<svg` element; the runner checks it.

---

## 2. RQ-009 — the four tutorials as an executable contract

`tools/run-tutorials.sh` (815 lines, bash) executes every tutorial and fails when a page, an
example, a recorded value or an expected tutorial is missing or wrong. The expected set is an array
of four names, asserted in both directions.

### 2.1 Clean-tree run (exit 0, all four named)

```console
$ CARGO_TARGET_DIR=/tmp/rl-target bash tools/run-tutorials.sh
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
[exit=0]
```

The runner looked the binary up at `${CARGO_TARGET_DIR}/{debug,release}/lovelace-run`, as the docs
CI job (which builds `-p lovelace-run` first) will. The `cargo run -p lovelace-run` fallback was
also exercised by moving the binary aside; the run then reported
`runner: cargo run -p lovelace-run` and still exited 0 (§2.5, last row).

### 2.2 What is compared, and what is normalised (criterion 2)

- **Text tutorials:** the entire typed transcript, line for line. The normalisation is: drop a CR
  from a Windows checkout, strip trailing spaces/tabs, trim leading/trailing blank lines. Internal
  spacing and every digit are preserved; there is no numeric parsing or rounding on this path.
- **Reproducibility:** the JSON envelope is parsed by a small POSIX-awk walker and these fields are
  compared with the recorded `## Output` object: `result.display`, `result.kind`,
  `result.structured.exact`, and the plot artifact's path suffix. Additional integrity checks:
  `ok` is `true`, `result.typed` is `5 (Real)`, `artifacts[0].kind` is `plot`,
  `artifacts[0].title` is `squares`, the named file exists, is non-empty, starts with `<svg`, and is
  outside the repository. The only normalised path information is the absolute temporary-directory
  prefix (`<temp-dir>`) and `\` versus `/`; the file name and the artifact identity are checked.
- **Never normalised away:** a wrong mathematical value. The tail differences between the 30-place
  and 50-place spectra are compared literally, and the linear-algebra values are additionally checked
  by the oracle below.
- **Page shape:** each page's `## Source` fence must be byte-identical (after CR removal) to its
  `docs/tutorials/examples/*.lv`; the `## Verification` section must contain the project-root
  `cargo run -p lovelace-run --` command and the example path. A missing section, an empty fence, a
  wrong fence language, a missing page, an unlisted page or an example that has drifted is a
  failure, never a skip. A page-wide static pass rejects `/tmp`, `<<` and (outside the
  reproducibility page and its README note) `python3`.

### 2.3 The independent oracle (criterion 4)

The linear-algebra page claims `det(A) = 5` and `A * sol = b` with `gap = [[0], [0]]`. The runner
recomputes both from the **observed** transcript with bash 64-bit integer arithmetic, not the
engine and not the page's expected values:

- parse `A`, `b`, `sol`, `gap`, `d` from the observed lines;
- `det = a11*a22 - a12*a21` must equal the observed `d`;
- scale `sol` by its decimal places (here one), then require
  `a11*sx + a12*sy == b1 * 10^scale` and `a21*sx + a22*sy == b2 * 10^scale`;
- require both `gap` entries to be exactly zero.

Clean run (`scale 10^1`, i.e. `sol = [18, 14]/10`):

```text
ORACLE linear-algebra: det(A) = 5; A * sol = b exactly at scale 10^1; gap = [0, 0]
```

The oracle's value is that it fails when the engine and the page agree on the **same wrong value**
(see §2.5, case 4), which a transcript comparison cannot detect.

### 2.4 Deliberate corruption of a recorded expected output (criterion 5)

The real page was corrupted once, then restored from a byte copy. SHA-256 before and after:
`49981b7c0d061d106b56b36f90e162753ee2eb950716098e21ffcb25c6d93f33`.

```console
$ sed -i 's/^  sol = \[\[1\.8\], \[1\.4\]\]$/  sol = [[1.9], [1.4]]/' docs/tutorials/linear-algebra.md
corrupted line: 1 match; sha256 before=49981b7c0d061d106b56b36f90e162753ee2eb950716098e21ffcb25c6d93f33
$ bash tools/run-tutorials.sh
run-tutorials: expecting 4 tutorials: linear-algebra symbolic signal-processing reproducible
run-tutorials: runner: /tmp/rl-target/debug/lovelace-run

FAIL linear-algebra: output does not match the recorded transcript
  expected:
    = [[5], [6]] (Array)
      A = [[2, 1], [1, 3]]
      Inv = [[0.6, -0.2], [-0.2, 0.4]]
      _ = [[5], [6]]
      b = [[5], [6]]
      d = 5
      gap = [[0], [0]]
      residual = [[5], [6]]
      sol = [[1.9], [1.4]]
  observed:
    (the remaining observed lines are identical to the clean transcript in §1.6)
      sol = [[1.8], [1.4]]
PASS symbolic: docs/tutorials/symbolic.md Source -> Output match
PASS signal-processing: docs/tutorials/signal-processing.md Source -> Output match
PASS reproducible: docs/tutorials/reproducible.md fields display/kind/exact/plot match; plot artifact exists outside the repository

run-tutorials: 4 tutorials checked (linear-algebra symbolic signal-processing reproducible)
run-tutorials: 0 independent oracle(s)
run-tutorials: FAIL (1 failure(s))
[corrupted exit=1]
restored sha256=49981b7c0d061d106b56b36f90e162753ee2eb950716098e21ffcb25c6d93f33
RESTORE=byte-identical
run-tutorials: 4 tutorials checked (linear-algebra symbolic signal-processing reproducible)
run-tutorials: 1 independent oracle(s)
run-tutorials: OK
[restored exit=0]
```

The corruption (`sol` 1.8 → 1.9 in the page's `## Output`) is shown in `expected`; the engine's
`observed` value is unchanged. The failure names the tutorial and the differing line; the restore
is byte-identical and the rerun is green. The oracle counter is 0 on the corrupted run because the
oracle runs only after the transcript matches.

### 2.5 Falsification matrix (each mutation applied to a scratch copy; the real tree untouched)

| # | Mutation | Observable | Exit |
|---|---|---|---|
| 1 | delete `docs/tutorials/symbolic.md` from the scratch copy | `FAIL inventory: expected tutorial 'symbolic' has no page at docs/tutorials/symbolic.md`; other three still pass | 1 |
| 2 | add an unlisted `docs/tutorials/extra.md` | `FAIL inventory: docs/tutorials/extra.md is not in the expected set of four; add it to the manifest or remove the page, so no tutorial is silently skipped` | 1 |
| 3 | append a line to the scratch `examples/symbolic.lv` | `FAIL symbolic: the ## Source fence and docs/tutorials/examples/symbolic.lv differ; a reader copying the page would run a different script` | 1 |
| 4 | replace the page + example with a self-consistent **wrong** system (`sol = [[1.9], [1.4]]`, `residual = [[5.2], [6.1]]`, `gap = [[0.2], [0.1]]`) and record its real engine output | transcript comparison **PASSES** (page and engine agree) but `FAIL linear-algebra: oracle: row 1 of A * sol is not b at scale 10^1 (sol = [19, 14] / 10^1)` | 1 |
| 5 | change `symbolic.md`'s Verification command from `cargo run -p lovelace-run -- ...` to a bare `lovelace-run ...` | `FAIL symbolic: the ## Verification section does not use the project-root cargo run command` | 1 |
| 6 | convert every scratch page and `.lv` to CRLF | all four still PASS, exit 0 (§2.6 has the same check against the real pages in a scratch copy) | 0 |
| 7 | move the built binary aside | runner reports `runner: cargo run -p lovelace-run`; all four PASS | 0 |

Case 4 is the oracle's reason to exist: the page and the engine agreed, so only an independent
recomputation could reject the claim. Case 3 and case 5 are the two ways the reader's copy could
stop being the runner's copy.

### 2.6 Windows shell check (Git Bash, fixture runner — plumbing only)

The Windows host has no Rust toolchain, so a fixture `lovelace-run.exe` (a bash script carrying the
real engine's captured stdout and a synthetic envelope) was placed in a scratch target directory
and the **real** `tools/run-tutorials.sh` was executed by Git Bash:

```console
$ uname -s
MINGW64_NT-10.0-26100
$ CARGO_TARGET_DIR=.../win-target TUTORIAL_FIXTURE=.../win-fixture bash tools/run-tutorials.sh
run-tutorials: expecting 4 tutorials: linear-algebra symbolic signal-processing reproducible
run-tutorials: runner: .../win-target/debug/lovelace-run.exe
PASS linear-algebra: docs/tutorials/linear-algebra.md Source -> Output match
ORACLE linear-algebra: det(A) = 5; A * sol = b exactly at scale 10^1; gap = [0, 0]
PASS symbolic: docs/tutorials/symbolic.md Source -> Output match
PASS signal-processing: docs/tutorials/signal-processing.md Source -> Output match
PASS reproducible: docs/tutorials/reproducible.md fields display/kind/exact/plot match; plot artifact exists outside the repository
run-tutorials: 4 tutorials checked (linear-algebra symbolic signal-processing reproducible)
run-tutorials: 1 independent oracle(s)
run-tutorials: OK
```

This exercises the MINGW branch (`.exe` suffix), `cygpath -w` argument conversion, a
`C:\...` artifact path in the JSON (escaped `\\`), `cygpath -u` file existence, the
repository-containment comparison and the awk JSON walker on Windows. It does **not** run the Rust
engine on Windows, and the `.exe` is not a PE binary; §4.2 records the boundary. Absolute paths are
shortened to `...` in this transcript; the run used the full `C:/Users/...` values.

CRLF tolerance was checked separately on the real pages and examples copied to a scratch tree and
converted with CRLF: all four passed. The runner strips CR when reading pages and examples and
executes an LF scratch copy, so a Windows checkout that ignores the `autocrlf=false` pin still
works.

---

## 3. README stale counts (TODO-002 / RQ-011.5)

`README.md` previously carried a fixed `716` desktop transcript and a fixed `6135` workspace total.
Both are replaced with output shapes plus a pointer to the measured record; no new fixed number is
introduced.

Before (`README.md:104-105`):

```text
$ cargo test -p lovelace-desktop
test result: ok. 716 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.79s
```

After:

```text
$ cargo test -p lovelace-desktop
test result: ok. <passed> passed; 0 failed; <ignored> ignored; <measured> measured; <filtered> filtered out; finished in <time>
```

Before (`README.md:160`):

```text
$ cargo test --workspace          # 6135 tests (1 ignored) on v0.4.1, across the twelve crates
```

After:

```text
$ cargo test --workspace
test result: ok. <passed> passed; 0 failed; <ignored> ignored; ...   # one such line per test target
```

Both areas now say that counts and timings depend on the commit, host and features, and point at
[docs/testing.md](../testing.md#1-unit-tests-in-crate-test) as the measured record (commit, OS,
command, counts, date; Linux and Windows desktop rows separate; the one ignored test named). The
pointer text sits before `## The crates`, so `check-docs.sh --readme`'s ordering rules still hold.
The file was normalised to LF, matching its stored form; the pre-existing checkout copy was CRLF, so
the visible diff is 14 insertions and 2 deletions rather than a whole-file line-ending rewrite.

---

## 4. What was not verified

### 4.1 Hosted GitHub Actions

Not run in this round. RQ-013 owns hosted verification; `.github/workflows/ci.yml` is out of scope
and the wiring of `tools/run-tutorials.sh` into the docs job is explicitly deferred to round 4.

### 4.2 Windows engine execution

No Windows Rust toolchain exists on this host (`cargo` is not on the Windows `PATH` and
`%USERPROFILE%\.cargo` is absent), so `lovelace-run.exe` could not be built or run. The Windows
evidence is therefore the Git Bash plumbing test with a fixture (§2.6) plus the PowerShell selector
against a real Linux-captured envelope (§1.4), not an end-to-end Windows engine run. The `.exe`
discovery branch is exercised with a non-PE fixture named `lovelace-run.exe`; a real PE binary is
the CI job's evidence, not this report's.

### 4.3 PowerShell route inside a real `cargo run`

The PowerShell pipeline was run against a real envelope file, not as a stage of a Windows
`cargo run`. The stage it replaces (`cargo run ... |`) is the same command the Ubuntu run executes;
the JSON selection and output shape are verified.

### 4.4 Tutorial Precision terminology

The four pages' `## Precision` sections are unchanged. The state file records the reason: the shared
vocabulary is owned by the round-3 editorial change and the tutorials' terminology move is TODO-003
for round 4. This task deliberately did not pre-empt it, and no numerical claim was weakened.

### 4.5 Runner executable bit

The repository's `core.fileMode=false` and the Windows mount report every file as `rwxrwxrwx`, so no
mode change is recorded; like `tools/check-docs.sh` (index mode `100644`), the runner is invoked as
`bash tools/run-tutorials.sh`. The docs job's `shell: bash` step does the same.

---

## 5. Per-file changelog

| File | Change |
|---|---|
| `tools/run-tutorials.sh` (new, 815 lines) | The runner: fixed four-name manifest asserted in both directions; `## Source`/`## Output` fence extraction with failure on missing/empty/wrong-language fences; checked-in example equality; typed-transcript comparison; JSON envelope walker and field comparison; plot artifact/file checks; independent integer oracle; RQ-008 static checks; runner discovery (`LOVELACE_RUN`, target dir debug/release with `.exe`, `cargo run` fallback); `cygpath` native paths; CRLF tolerance; scratch-only writes; exit 0/1/2 contract. |
| `docs/tutorials/examples/linear-algebra.lv` (new, 8 lines) | The page's Source fence as an executable file. |
| `docs/tutorials/examples/symbolic.lv` (new, 11 lines) | The page's Source fence as an executable file. |
| `docs/tutorials/examples/signal-processing.lv` (new, 8 lines) | The page's Source fence as an executable file. |
| `docs/tutorials/examples/reproducible.lv` (new, 6 lines) | The page's Source fence as an executable file; the plot goes to the OS temp dir by default. |
| `docs/tutorials/linear-algebra.md` | Assumption names the project-root `cargo run` command; Source links its `.lv`; Verification replaces the debug-binary here-document with the portable command and names the runner. Diff: 14 insertions / 13 deletions. |
| `docs/tutorials/symbolic.md` | Same shape: Assumption, Source link, Verification. Diff: 11 insertions / 15 deletions. |
| `docs/tutorials/signal-processing.md` | Same shape, plus the length-3 refusal command with a Command Prompt note. Diff: 22 insertions / 13 deletions. |
| `docs/tutorials/reproducible.md` | Rewritten around `--file`, the OS temp default and `--plot-dir`; Output uses `<temp-dir>` for the host-specific prefix; Reading explains the three pipeline stages and the field meanings; Assumptions make the Python prerequisite explicit and give the PowerShell no-Python route; Verification has the bash + `python3` and PowerShell + `ConvertFrom-Json` blocks and names the runner. The Precision table is untouched. Diff: 70 insertions / 37 deletions. |
| `docs/tutorials/README.md` | Removes the debug-binary paragraph; states prerequisites, the portable command form, the checked-in example location and the OS-temp/`--plot-dir` rule; documents `bash tools/run-tutorials.sh` and what it asserts; keeps the tutorial table and the `## Precision` section (required by `check-docs.sh --claims`). Diff: 29 insertions / 4 deletions. |
| `README.md` | Two stale fixed counts replaced with output shapes; both areas point at `docs/testing.md` as the measured record; EOL normalised to LF. 14 insertions / 2 deletions versus the stored file. |
| `docs/goal-docs-v2/round-3/portable-tutorials.md` (this file) | The report. |

Scratch evidence scripts and captured logs live under `.tmp/round3/` (ignored by
`.gitignore`); nothing there is committed.

---

## 6. Reproduction index

```console
# the four tutorials, and the contract runner (the target directory is host state)
cargo run -p lovelace-run -- --file docs/tutorials/examples/linear-algebra.lv --text
cargo run -p lovelace-run -- --file docs/tutorials/examples/symbolic.lv --text
cargo run -p lovelace-run -- --file docs/tutorials/examples/signal-processing.lv --text
cargo run -p lovelace-run -- --file docs/tutorials/examples/reproducible.lv --omit-functions --omit-variables
CARGO_TARGET_DIR=/tmp/rl-target bash tools/run-tutorials.sh

# the documentation gates, in the modes the docs job runs
bash tools/check-docs.sh
bash tools/check-docs.sh --claims
bash tools/check-docs.sh --readme
bash tools/check-docs.sh --symbols
bash tools/check-crate-examples.sh
bash tools/check-reference-pages.sh
```

### Gate results at the measured revision

```console
$ bash tools/check-docs.sh
check-docs: 57 Markdown files, 170 relative links, 241 code references
check-docs: OK
$ bash tools/check-docs.sh --claims
check-docs: claims audit: 88 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: OK
$ bash tools/check-docs.sh --readme
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
$ bash tools/check-docs.sh --symbols
check-docs: 57 Markdown files, 170 relative links, 241 code references, 20 line-suffix citations, 20 symbol citations
check-docs: OK
$ bash tools/check-crate-examples.sh
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
$ bash tools/check-reference-pages.sh
check-reference-pages: 16 page(s), 112 required sections in order
check-reference-pages: OK
```

---

## 7. Acceptance mapping

| Requirement | Evidence |
|---|---|
| RQ-008.1 no `/tmp/rl-target` in tutorial commands or `docs/tutorials/README.md` | §1.2 grep (no match); the runner's static check rejects it page-wide (§2.2). |
| RQ-008.2 project-root `cargo run`, release binary on `PATH`, or portable script | §1.1 and §1.6; the runner enforces the `cargo run` shape in every `## Verification` (§2.5 case 5) and itself accepts `LOVELACE_RUN`, a built target binary, or `cargo run` fallback (§2.1, §2.5 case 7). |
| RQ-008.3 OS-appropriate, configurable, created temp output | §1.3; default is the OS temp dir (`lovelace-lang/src/lib.rs:831`), `--plot-dir` override is created (`lovelace-run/src/main.rs:377`), and the runner passes a non-existent scratch directory and asserts creation. |
| RQ-008.4 no required Python, here-document or Unix path on Windows | §1.4; PowerShell no-Python route executed on the Windows host against a real envelope; runner rejects `<<`, `/tmp` and stray `python3` page-wide. |
| RQ-008.5 copyable on both platforms | §1.5, §1.6, §2.6; Windows end-to-end engine run is the stated limitation (§4.2). |
| RQ-009.1 runner covering all four | §2.1, §2.5 cases 1–2; fixed four-name manifest. |
| RQ-009.2 stable typed values and fields, only host info normalised | §2.2, §2.4 expected/observed diff, §2.5 case 4. |
| RQ-009.3 Ubuntu and Windows CI (bash), fails on omission | §2.1 (Ubuntu), §2.6 (Git Bash), §2.5 cases 1–2, CRLF check. |
| RQ-009.4 independent numerical oracle | §2.3, §2.5 case 4. |
| RQ-009.5 corruption and restore | §2.4 (SHA-256 before/after identical, exit 1 then 0). |
| README stale counts | §3. |

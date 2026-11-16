# Round 5 — builtin-registry sync (RQ-014) and agent-instruction audit (RQ-022)

| Field | Value |
|---|---|
| Requirements | RQ-014 (C14), RQ-022 (C22), TODO-004, TODO-006 |
| Branch | `docs/documentation-quality-remediation` |
| Baseline | `d946316` + the uncommitted round-1…5 documentation working tree |
| Host | WSL Ubuntu, kernel 6.x (`uname` Linux), repo at `/mnt/c/Users/ricar/dev/RustLace` |
| Toolchain | `cargo 1.100.0 (3acc79c0c 2026-11-05)`, `rustc 1.100.0 (0ef1bc386 2026-11-09)`, `GNU bash 5.2.21` |
| Target dir | `CARGO_TARGET_DIR=/tmp/rl-target` |
| Windows host | Windows PowerShell 5.1.26100.4202; **no** `cargo`, **no** `pwsh` on PATH |

Files owned and changed: `docs/language/examples/run-examples.sh`, `AGENTS.md`,
`.github/copilot-instructions.md`, `.dsh/skills/**`, `docs/contributing.md`, `README.md`,
`.github/workflows/ci.yml`, and `docs/release.md` (the release guide named by RQ-022).

---

## 0. Summary

1. **RQ-014.** `docs/language/examples/run-examples.sh` now extracts the documented builtin names,
   their declared category, arity and parameter names from `docs/language/builtins.md`, reads the
   runtime registry with `lovelace-run --functions --function-limit 500`, and fails the run on any
   disagreement. It is pure bash (no `jq`), so the documentation job executes the same check on
   `ubuntu-latest` and on `windows-latest` through its existing
   `bash docs/language/examples/run-examples.sh` step. The fenced-example check is unchanged:
   the clean run reports the same 193 examples as before plus the new catalogue line.
2. **RQ-022.** Every command in the eight audited files was executed here where the host supports it,
   or verified against the file/flag/crate it names where it is Windows-only. Ten commands were
   wrong, stale or zero-work hazards; all are fixed (section 2.3). The gates the parent asked for are
   green on the final tree (section 4).
3. **TODO-004 / TODO-006.** README now says nine task skills; the `AGENTS.md` CI row now describes the
   twelve steps the `docs` job actually runs (section 3).

---

## 1. RQ-014 — portable builtin-catalogue verification

### 1.1 How the sync works

`--functions` publishes one compact JSON object. Its shape was read from the running binary before
the parser was written:

```json
{"functions":[{"category":"Numerics","minArity":1,"name":"abs","parameterCount":1,
"parameters":["x"],"variadic":false}, …],
 "mathIrVersion":2,"ok":true,"pattern":"","protocolVersion":1,
 "returned":124,"symbolicFormatVersion":"#!lovelace-sym 1","total":124,"truncated":false}
```

The runner parses that shape with bash parameter expansion only (`json_string`, `json_number`,
`json_boolean`, `parameters_text`, `arity_text`), because neither `ubuntu-latest` nor Git Bash ships
`jq` and the gate must not depend on a tool one of them lacks. It then checks, in both directions:

| # | Check | Diagnostic when it fails |
|---|---|---|
| 1 | every runtime builtin appears in the full catalogue | `builtins.md:<section line>: the full catalogue omits runtime builtin '<name>' (category '<category>')` |
| 2 | its category matches the section heading | `builtins.md:<row>: '<name>' is listed under '<A>'; the runtime registry says '<B>'` |
| 3 | its arity text matches (`minArity`/`parameterCount`/`variadic`) | `…: '<name>' lists arity '<A>'; the registry declares '<B>'` |
| 4 | its parameter names match, in order | `…: '<name>' lists parameters '<A>'; the registry declares '<B>'` |
| 5 | every documented row exists at runtime | `…: documented builtin '<name>' is not published by lovelace-run --functions (runtime total <N>)` |
| 6 | the "Registry at a glance" total and per-category counts agree | `…: the glance table says '<C>' has <n>; the registry has <m>` / `…: the glance categories sum to <a> but --functions reports total <b>` |
| 7 | each `#### Category (N builtins)` heading and its row count agree | `…: the '<C>' section declares <n> rows but lists <m>` |
| 8 | no duplicate row, no row under no heading, no section for an unpublished category | `…: catalogue row '<name>' appears twice (first at line <n>)` and siblings |
| 9 | the listing was complete and well formed | `lovelace-run --functions: … truncated …; raise the limit in this runner`, plus fatal errors for malformed JSON or a non-zero exit |

Zero-work guards: a runtime listing with no builtins, a page with no table rows, a missing glance
table and a mutated `### Full catalogue` heading each **fail** the run with a named diagnostic
instead of passing vacuously (mutation 6 below).

### 1.2 Clean-tree transcript

`sha256sum docs/language/builtins.md` before and after:
`830e16ee8809989e726ab6034b1570a3e0996fca90013fb8f8f3b0d245ee751f` (unchanged).

```console
$ bash docs/language/examples/run-examples.sh
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
catalogue: 124 builtins in 11 categories match lovelace-run --functions --function-limit 500

All 193 examples passed.
$ echo $?
0
```

Per-page counts (4+19+23+44+26+22+25+5+18+7 = 193) are identical to the pre-change runner; the new
line is the `catalogue:` one.

### 1.3 Mutation round-trip

Each mutation was made to `docs/language/builtins.md`, the runner was executed, and the original was
restored from a copy. After every restore `sha256sum` equaled
`830e16ee…ee751f`, i.e. byte-identical, and the final clean run passed again. Row numbers are the
real lines of the page (`abs` 77, `is_even` 81, `sqrt` 86, `apart` 95, Numerics heading 73,
Symbolics heading 87, glance Numerics row 23, glance heading 19, `### Full catalogue` 71).

| # | Mutation | Exit | Verbatim diagnostic (stderr) |
|---|---|---:|---|
| 1 | rename documented `is_even` → `is_even_renamed` | 1 | `builtins.md:73: the full catalogue omits runtime builtin 'is_even' (category 'Numerics')`<br>`builtins.md:81: documented builtin 'is_even_renamed' is not published by lovelace-run --functions (runtime total 124)` |
| 2 | move the `abs` row under `#### Symbolics` | 1 | `builtins.md:95: 'abs' is listed under 'Symbolics'; the runtime registry says 'Numerics'`<br>`builtins.md:73: the 'Numerics' section declares 10 rows but lists 9`<br>`builtins.md:87: the 'Symbolics' section declares 40 rows but lists 41` |
| 3 | delete the `sqrt` row (the runtime-only-addition direction) | 1 | `builtins.md:73: the full catalogue omits runtime builtin 'sqrt' (category 'Numerics')`<br>`builtins.md:73: the 'Numerics' section declares 10 rows but lists 9` |
| 4 | change `abs` arity `1` → `2` | 1 | `builtins.md:77: 'abs' lists arity '2'; the registry declares '1'` |
| 5 | change the glance Numerics count `10` → `11` | 1 | `builtins.md:23: the glance table says 'Numerics' has 11; the registry has 10`<br>`builtins.md:19: the glance categories sum to 125 but --functions reports total 124` |
| 6 | rename `### Full catalogue` → `### Full builtin catalogue` (structure drift) | 1 | `error: the builtin catalogue lists no Name/Arity/Parameters rows under "### Full catalogue"; refusing to pass on zero work` |

Mutation 1 is the rename the acceptance names; mutation 3 is exactly the observation a runtime-only
addition produces (a runtime name with no documented row); mutation 2 is the misclassification.
The restore was verified for all six:

```console
RESTORED byte-identically: sha256=830e16ee8809989e726ab6034b1570a3e0996fca90013fb8f8f3b0d245ee751f   (×6)
=== final integrity and clean run ===
830e16ee8809989e726ab6034b1570a3e0996fca90013fb8f8f3b0d245ee751f  docs/language/builtins.md
CLEAN_EXIT=0
catalogue: 124 builtins in 11 categories match lovelace-run --functions --function-limit 500
All 193 examples passed.
```

One defect was found in my own first draft by mutation 6: under `set -u` a merely declared
associative array reports "unbound variable" for `${#array[@]}`, so the zero-work guard printed a
bash error instead of its diagnostic. All associative arrays are now initialised with `=()` and
mutation 6 produces the named error above.

### 1.4 What the check does not claim

It compares names, categories, arity and parameter names — not summaries, examples or return kinds,
and not the semantic truth of a row. It also does not replace the fenced examples: the catalogue
check can pass while a documented output is wrong, which is what the 193 example pairs cover.

---

## 2. RQ-022 — command audit

### 2.1 Method

For every fenced block and every inline command in the eight files, the command was executed on the
host that supports it, or — for Windows-only commands — the named file, flag, crate or cmdlet was
checked to exist. `cargo test -p <crate> <filter>` forms were run and their `test result:` line read,
because a filter that matches nothing exits 0. A Windows host with no Rust toolchain and no `pwsh`
cannot run the Linux/CI commands, and this report says so rather than implying otherwise.

### 2.2 Audit table

Host keys: **L** = WSL Ubuntu bash; **W** = Windows PowerShell 5.1; **S** = source/flag inspection
(not executed); **X** = could not be executed here.

#### `AGENTS.md`

| Command | Host | Result |
|---|---|---|
| `cargo build --workspace` | L | exit 0 |
| `cargo build --release -p lovelace-desktop` | L | exit 0, 1m25s; `target/release/lovelace-desktop` 32,746,184 bytes, `lovelace-desktop 0.4.1` |
| `cargo test --workspace` | L | exit 0: 35 targets, **6147 passed, 0 failed, 1 ignored** |
| `cargo test -p lovelace-lang` | L | 1750 passed, 0 failed |
| `cargo test -p lovelace-lang round67` | L | 22 passed, 0 failed |
| `cargo test -p lovelace-desktop` | L (Xvfb) | 711 + 4 integration = 715 passed, 0 failed |
| `cargo test -p lovelace-desktop lay_011` | L (Xvfb) | 2 passed, 0 failed |
| `cargo fmt --all` | L | exit 0, no reformat; `--check` also exit 0 |
| `cargo fmt --all -- --check` | L | exit 0 |
| `cargo clippy --workspace --all-targets` | L | exit 0 (pre-existing warnings only) |
| `cargo run --release -p lovelace-corpus` | L | `corpus: 214 passed, 0 failed, 214 total` |
| `cargo run --release -p lovelace-corpus -- lang/001` | L | `corpus: 1 passed, 0 failed, 214 total (filter lang/001)` |
| `cargo run -p lovelace-desktop -- --gui-selftest` | L | `GUI_SELFTEST summary checks=20 failed=0` |
| `cargo run -p lovelace-desktop -- --journey J-02 --out .tmp/journey` | L | `JOURNEY J-02 PASS`, `JOURNEY summary journeys=1 failed=0`; **no artifact and no directory is written for J-02** (only J-09/J-15 write) |
| `--selftest` | L | `SELFTEST summary checks=14 skipped=1 failed=0`; skip line names `no-network-imports` |
| `--contrast-check` | L | `contrast: every required pair passes (normal text 4.5:1, large text/non-text 3:1)` |
| `--legibility-check` | L | `RESULT PASS` |
| `--visual-audit` | L | PASS; `counts: 6 type roles, 6 control kinds x 7 states, 7 semantic roles, 8 empty states` |
| `--glyph-check` | L | `result PASS: every glyph is drawable or mapped` |
| `--ui-audit` | L | `summary files=15 passed=15 failed=0 errors=0`, `result PASS` |
| `--thread-audit` | L | `result PASS` |
| `--shortcuts` | L | `45 registered bindings, generated from src/keyboard.rs` |
| `--platform-parity` | L | **exit 2**, stderr `--platform-parity requires the Windows build` |
| `--platform-claims` | L | `result PASS: 4 platform row(s), 0 problem(s)` — the mode existed in `--help` but was missing from this list (now added) |
| `--raster-svg in.svg --out out.png --scale 2` | L | exit 0, `raster … px=40x20 scale=2.000`; `in.svg` is an input placeholder, now labelled |
| `--help` | L | exit 0, lists every mode above |
| `sudo apt-get install …` (Build on Linux) | L | not run (would mutate the host); package names checked against the CI job, which installs the same set |
| `xvfb-run … ./target/release/lovelace-desktop --gui-selftest` | L | `GUI_SELFTEST summary checks=20 failed=0` |
| `xvfb-run … cargo test -p lovelace-desktop` | L | 715 passed, 0 failed |

Fixes: the block used a Windows path (`.tmp\journey`) and an `APPDATA`-only note; `.tmp/journey` is
now the POSIX form with the PowerShell form stated in prose, `--platform-parity` is marked
Windows-only, `--raster-svg` input is labelled, `--platform-claims` was added, and the J-02 artifact
claim was removed. The "built with 1.99.1" provenance now names the toolchain actually used here
(1.100.0) and points at `rust-version = "1.85"` in `Cargo.toml`.

#### `.github/copilot-instructions.md`

| Command | Host | Result |
|---|---|---|
| `cargo build --workspace` | L | exit 0 (also via `cargo test --workspace`) |
| `cargo test --workspace` | L | 6147 passed, 1 ignored, exit 0 |
| `cargo run --release -p lovelace-corpus` | L | 214 passed, 0 failed |
| `cargo test -p lovelace-desktop` | L (Xvfb) | 715 passed, 0 failed |
| `cargo run -p lovelace-desktop -- --gui-selftest` | L | 20 checks, 0 failed |
| Toolchain claim (`Rust 1.85+`, `edition 2024`) | S | `Cargo.toml`: `edition = "2024"`, `rust-version = "1.85"` — accurate; no change needed |

#### `docs/contributing.md`

| Command | Host | Result |
|---|---|---|
| `cargo test -p <crate>` | L | was a literal placeholder; replaced with `cargo test -p lovelace-lang` (verified 1750 passed) plus the rule that a zero-selected filter is a failure |
| `cargo test --workspace` | L | 6147 passed |
| `cargo run --release -p lovelace-corpus` | L | 214 passed |
| `cargo fmt --all` | L | exit 0 |
| `cargo clippy --workspace --all-targets` | L | exit 0 |

#### `docs/release.md`

| Command | Host | Result |
|---|---|---|
| `./tools/next-version.ps1 -CurrentVersion 0.1.0 -Commits 'feat: publish the RustLace workspace'` | W | exact documented output reproduced (`previous=0.1.0 next=0.2.0 bump=minor … deciding commits: synthetic-1 …`); script has no shebang, so the `./` form is PowerShell-only — now labelled, with the `pwsh -File` form named |
| `cargo build --release -p lovelace-desktop` | L | exit 0; artifact path now shows both `target/release/lovelace-desktop.exe` and `target/release/lovelace-desktop` |
| `cargo test -p lovelace-desktop` (checklist step 5) | L | 715 passed under Xvfb; the Xvfb form is now stated in the step |
| `cargo run -p lovelace-desktop -- --gui-selftest` (step 6) | L | 20/0, with `XDG_CONFIG_HOME` as the Linux application directory (now stated alongside `APPDATA`) |
| `--contrast-check` (step 7) | L | every required pair passes |
| `--journey J-02 J-03 J-07 J-08 J-09 J-10 J-15 --out <dir>` (step 8) | L | `JOURNEY summary journeys=7 failed=0`; the placeholder "the documented set" now names the seven IDs |
| 9 documentation-gate commands (step 10) | L | all exit 0 (section 4) |
| `stat -c '%s %n'`, `sha256sum`, `sha256sum -c SHA256SUMS.linux` | L | `32746184 …`, digest `86afa2e6d01d0721…`, `-c` reports `OK` |
| `Get-Item … \| Select-Object Length`, `Get-FileHash … -Algorithm SHA256` | W | cmdlets present and functional (run against `LICENSE`: Length 1098, SHA256 `1D98FC79…262D`) |
| `git ls-remote --tags origin` | L | exit 0; `refs/tags/v0.4.1`, `refs/tags/v0.4.0` listed |
| `signtool sign …`, `Get-AuthenticodeSignature` | X | `signtool` is not installed on this host; `Get-AuthenticodeSignature` exists. Not executed — the page already says the default artifact is unsigned |
| `git tag -a` / `git push origin` / `gh release create` / `gh release upload` | X | mutating and remote; flags verified instead: `gh release create --verify-tag/--title/--notes-file`, `gh release upload --clobber`, `gh release download`, `gh release view`, `gh run rerun` all exist in `gh 2.104.0` |

#### `.dsh/skills/add-builtin/SKILL.md`

| Command | Host | Result |
|---|---|---|
| `cargo test -p lovelace-lang <name>` | L | placeholder; was the literal `my_builtin`, which selects 0 tests and exits 0 — now marked as placeholder with the zero-selection rule |
| `cargo test -p lovelace-lang every_builtin_refuses_every_count_outside_its_declared_arity` | L | 1 passed, 0 failed |
| `cargo run -p lovelace-run -- --function <name>` | L | `--function sqrt` prints the descriptor JSON with `parameters`, `minArity`, `parameterCount`, `summary` |
| `cargo run -p lovelace-run -- --eval "<name>(X, Y)" --text` | L | `--eval "1/3" --text` → `= 0.(3) (Real)` |
| `cargo run --release -p lovelace-corpus -- lang/112` | L | corpus has 111 lang records, so `lang/112` is the next free id (verified); `lang/001` form returns `1 passed`; an unmatched filter returns `0 passed` and exit 0 — now stated |

#### `.dsh/skills/add-modus-plugin/SKILL.md`

| Command | Host | Result |
|---|---|---|
| `cargo test -p lovelace-lang round67_modus` | L | 22 passed, 0 failed |
| `cargo test -p lovelace-lang <the test you added>` | L | placeholder; the literal `my_plugin_registers_and_answers` selects 0 tests — marked |
| `cargo test -p lovelace-desktop` | L (Xvfb) | 715 passed; Xvfb form added |
| `cargo run -p lovelace-run -- --functions` | L | **truncated by default: `returned:50`, `total:124`, `truncated:true`** — an absent name there was not proof of invisibility. The skill now uses `--function <name>` plus `--functions --function-limit 500` and says why |

#### `.dsh/skills/add-ui-pane/SKILL.md`

| Command | Host | Result |
|---|---|---|
| `cargo test -p lovelace-desktop lay_011` | L (Xvfb) | 2 passed |
| `cargo test -p lovelace-desktop lay_002` | L (Xvfb) | 1 passed |
| `cargo test -p lovelace-desktop a11y_002` | L (Xvfb) | 1 passed |
| `--ui-audit`, `--contrast-check`, `--gui-selftest` | L | PASS, PASS, 20/0 |
| `powershell -NoProfile -ExecutionPolicy Bypass -File docs/images/capture.ps1 -Scenario my-pane -Exe … -WorkRoot .tmp/capture` | S/X | parameters `-Scenario/-Exe/-OutDir/-WorkRoot` and the `IMAGE <name> <bytes> bytes <w>x<h> sha256=<hash>` line all exist in the script; default `-Exe` is `target-release\release\lovelace-desktop.exe` as the prose says. **Not executed**: needs the Windows release binary and a real desktop session for UIA/PrintWindow. The command now sits in its own ```powershell``` block, with `my-pane` marked as the scenario you add to `ValidateSet`/`$scenarios` |

#### `.dsh/skills/add-language-feature/SKILL.md`

| Command | Host | Result |
|---|---|---|
| `cargo test -p lovelace-lang suite_lex_parse_batch` | L | 49 passed |
| `cargo test -p lovelace-lang <the test you added>` | L | placeholder `my_feature_name` removed |
| `cargo test -p lovelace-run golden_batch` | L | 8 passed, 0 failed |
| `cargo run --release -p lovelace-corpus -- lang/112` | L | after adding the record; the zero-match exit-0 hazard is stated |
| `cargo run -p lovelace-run -- --eval "1 != 2" --text` | L | `= True (Boolean)` |
| `LOVELACE_RECORD_GOLDENS=1 cargo test -p lovelace-run` (procedure step 7) | S | env var exists in `lovelace-run/src/golden_batch.rs` (`RECORD_ENV`) and is inert unless set |

#### `.dsh/skills/add-acceptance-mode/SKILL.md`

| Command | Host | Result |
|---|---|---|
| `cargo run -p lovelace-desktop -- --journey J-02 --out .tmp/journey` | L | PASS, exit 0; no artifact (skill's observable corrected) |
| `--gui-selftest` | L | 20/0 |
| `--help` | L | exit 0, lists the modes |
| `cargo test -p lovelace-desktop` | L (Xvfb) | 715 passed; Xvfb form added |
| "`--journey` is rejected on non-Windows" (step 2) | L | **false**: `--journey` has no platform gate and passes on Linux; the Windows-only refusals are `--legacy-win32`, `--ime-probe`, `--platform-parity`. Sentence corrected |

#### `.dsh/skills/change-numeric-kernel/SKILL.md`

| Command | Host | Result |
|---|---|---|
| `cargo test -p lovelace-real` | L | 2487 passed, 0 failed |
| `cargo test -p lovelace-nat proof_small_domain` | L | 7 passed |
| `cargo test -p lovelace-lang` | L | 1750 passed |
| `cargo run --release -p lovelace-corpus` | L | 214 passed |
| `cargo run --release -p lovelace-bench -- sqrt 10000 3` | L | `RESULT sqrt 10000 20.303 20.592`, `DIGEST sqrt 10002:1.4142135623730950488016` |
| `$env:LOVELACE_THREADS=1; cargo run …` | L/W | the documented line was PowerShell syntax in a mixed block; it is now two blocks — `LOVELACE_THREADS=1 …` for bash and `$env:LOVELACE_THREADS=1; …` for Windows PowerShell. Both runs print the **same** digest: `DIGEST sqrt 10002:1.4142135623730950488016` |
| `cargo test -p lovelace-nat` (implicit) | L | covered by the proof filter; whole crate not separately run here |

#### `.dsh/skills/debug-desktop-issue/SKILL.md`

| Command | Host | Result |
|---|---|---|
| `cargo test -p lovelace-desktop <regression_test_name>` | L | placeholder kept, zero-selection rule added |
| `cargo test -p lovelace-desktop` | L (Xvfb) | 715 passed |
| `--gui-selftest`, `--ui-audit` | L | 20/0, PASS |
| `--journey J-02 --out .tmp\journey` | L | the Windows path was in a cross-platform block; now `.tmp/journey` with the PowerShell form stated. PASS |
| "the full desktop suite reports `716 passed; 0 failed`" | L | **stale** (Linux is 711 + 4 = 715 today). Replaced with "read this commit's measured rows in `docs/testing.md`", keeping Linux and Windows separate |
| UIA/PrintWindow `powershell` snippet | S/X | the snippet is already in a ```powershell``` block and names helpers that exist in `docs/images/capture.ps1` (`Get-All`, `Get-Names`, `Find-Element`, `Wait-Status`, `Save-Frame`); not executable without a Windows desktop |

#### `.dsh/skills/release-and-publish/SKILL.md`

| Command | Host | Result |
|---|---|---|
| `cargo fmt --all -- --check`, `cargo test --workspace`, `cargo run --release -p lovelace-corpus`, `cargo build --release -p lovelace-desktop` | L | all exit 0 (counts above) |
| `cargo run -p lovelace-desktop -- --version` (preflight) | L | `lovelace-desktop 0.4.1`, equal to `[workspace.package] version` |
| Windows block: `--gui-selftest`, `--contrast-check`, `--journey … --out .tmp\release-journeys` | L | mode outputs as above; the seven-ID journey run passes with `journeys=7 failed=0`; the backslash path is correct *in PowerShell* and the block is labelled as such |
| Linux block: `unset WAYLAND_DISPLAY`, `XDG_CONFIG_HOME="$(mktemp -d)"`, `xvfb-run … cargo test/… --contrast-check/… --journey` | L | all exit 0 (715 desktop tests; contrast; 7 journeys) |
| Docs gate block (6 commands as written before) | L | exit 0, **but incomplete**: the skill said "the same way the Documentation examples job does" while omitting `check-crate-examples.sh`, `run-tutorials.sh` and `test-docs-gates.sh`. All nine are now listed and run |
| `sha256sum -c SHA256SUMS.windows` | S | shape verified against `release.yml` which writes `"<sha256>  lovelace-desktop.exe"`; not run (no Windows asset on this host) |
| `gh release …`, `git tag/push`, `signtool` | X | see `docs/release.md` row above |
| CI gate table | S | now matches the twelve `ci.yml` steps (was missing crate examples, tutorials and the mutation self-test) |

#### `.dsh/skills/sync-docs/SKILL.md`

| Command | Host | Result |
|---|---|---|
| `cargo run -p lovelace-run -- --eval "<the documented source>" --text` | L | placeholder; executed with real sources |
| `bash docs/language/examples/run-examples.sh` | L | 193 examples + catalogue 124/11 |
| `bash docs/extensions/examples/run-examples.sh` | L | `pages: 4   examples checked: 27   passed: 27   failed: 0   unpaired: 0` |
| `cargo run --release -p lovelace-corpus` | L | 214 passed |
| `cargo test -p lovelace-lang docs_sync` | L | **1 passed** (claim of 1 is correct) |
| `cargo test -p lovelace-lang docssync` | L | **2 passed** (claim of 2 is correct) |
| `cargo test -p lovelace-run golden_batch` | L | 8 passed |
| `cargo test -p lovelace-desktop platform_claims` | L (Xvfb) | **10 passed** (claim of 10 is correct) |
| `LOVELACE_RECORD_GOLDENS=1 cargo test -p lovelace-run golden_batch` | S | env var honoured by `golden_batch.rs`, inert otherwise |
| `Select-String -Path docs -Pattern '<old text>' -Recurse` | W | works; a bash `grep -rn '<old text>' docs` form is now given beside it |
| "every numerical claim says **exact**, **precision-bounded** or **parity**" | S | retired three-way vocabulary; replaced with the editorial guide's four kinds: exact, precision-limited, error-bounded, parity-verified |

### 2.3 Fixes applied (ten command defects)

1. `AGENTS.md`: `--out .tmp\journey` was a Windows path in a POSIX block — now `.tmp/journey` with the
   PowerShell form stated; the J-02 artifact claim removed.
2. `AGENTS.md`: `--platform-parity` is Windows-only (verified exit 2 here) — marked at the line.
3. `AGENTS.md`: `--raster-svg in.svg` names a file that does not exist — labelled as an input
   placeholder.
4. `AGENTS.md`: `--platform-claims` (a shipped, cross-platform mode) was missing — added.
5. `AGENTS.md`: "the tree was built with 1.99.1" → 1.100.0, with `rust-version = "1.85"` named; README
   the same.
6. `AGENTS.md`: "exact, precision-bounded or parity" → the four editorial kinds.
7. `README.md`: "the eight task skills" → nine (TODO-004); "precision-bounded" → precision-limited.
8. `docs/contributing.md`: literal `cargo test -p <crate>` → a real crate, and the zero-selected-test
   rule added.
9. `docs/release.md`: `./tools/next-version.ps1` presented as host-neutral although the script has no
   shebang — labelled PowerShell, `pwsh -File` named; the release artifact path now has both OS
   forms; `APPDATA`-only → `APPDATA`/`XDG_CONFIG_HOME`; the desktop test step now states `xvfb-run`;
   "the documented set" of journeys now names the seven IDs.
10. Skills: Windows path in `debug-desktop-issue`; PowerShell `$env:` line in
    `change-numeric-kernel`; capture.ps1 mixed into a cross-platform block in `add-ui-pane`;
    `--journey`-is-Windows-only falsehood and the J-02 artifact claim in `add-acceptance-mode`; the
    bare `--functions` truncation in `add-modus-plugin`; zero-test placeholder filters in
    `add-builtin`, `add-language-feature` and `add-modus-plugin`; the stale `716 passed` in
    `debug-desktop-issue`; the incomplete docs gate and CI row in `release-and-publish`; the
    PowerShell-only search line and three-way vocabulary in `sync-docs`; the stale link
    `../../docs/testing.md` → `../../../docs/testing.md`.

### 2.4 What I could not verify here

| Item | Why |
|---|---|
| `bash docs/language/examples/run-examples.sh` under Windows Git Bash | The Windows host has no `cargo`, no built `lovelace-run.exe`, and no `pwsh`. The runner's only Windows-specific branches (`.exe` suffix, `cygpath`) are unchanged from the version CI already ran; the new catalogue code is host-neutral bash and was exercised on Linux. |
| Both `Documentation examples` CI variants (Ubuntu and Windows) | Hosted runs are RQ-013, not executable from this session. |
| `docs/images/capture.ps1` execution | Needs the Windows release binary, an interactive desktop session, UIA and PrintWindow; its parameters and `IMAGE` output contract were verified from source. |
| `signtool` signing | Not installed on this host. The page already documents the unsigned default and the self-signed caveat. |
| `gh release create/upload/download/view`, `git tag -a`, `git push` | Mutating and remote. Flags verified with `--help`; `git ls-remote --tags origin` was executed. |
| `--legacy-win32`, `--ime-probe` | Windows-only; the refusal is visible in `lovelace-desktop/src/main.rs`, and the equivalent `--platform-parity` refusal was executed. |
| Windows desktop test count (716 rows in `docs/testing.md`) | No Windows toolchain here; the Linux count measured today is 711 + 4. |

---

## 3. TODO-004 and TODO-006

- **TODO-004.** `README.md:194` now reads "the nine task skills", matching `docs/skills.md` ("Nine
  task skills") and the nine directories under `.dsh/skills/`.
- **TODO-006.** The `AGENTS.md` CI table's documentation row now describes the **twelve** steps in
  `.github/workflows/ci.yml`, in order: checkout (`core.autocrlf=false`), Rust toolchain, cargo cache,
  `cargo build -p lovelace-run`, `tools/check-crate-examples.sh`, `tools/run-tutorials.sh`, both
  example runners (the language one carrying the catalogue check), `check-docs.sh --symbols`,
  `--claims`, `--readme`, `check-reference-pages.sh`, `test-docs-gates.sh`. The paragraph below the
  table was updated to the same set.

---

## 4. Gate evidence after the edits

Final tree, `CARGO_TARGET_DIR=/tmp/rl-target`, run from the repository root.

```console
$ bash docs/language/examples/run-examples.sh
… (per-page counts as in 1.2) …
catalogue: 124 builtins in 11 categories match lovelace-run --functions --function-limit 500

All 193 examples passed.
EXIT=0

$ bash tools/check-docs.sh
check-docs: 70 Markdown files (57 docs/, 3 root, 10 agent instructions), 237 relative links, 48 heading anchors, 442 code references
check-docs: OK
EXIT=0

$ bash tools/check-docs.sh --symbols
check-docs: 70 Markdown files (57 docs/, 3 root, 10 agent instructions), 237 relative links, 48 heading anchors, 442 code references, 21 line-suffix citations, 22 symbol citations
check-docs: OK
EXIT=0

$ bash tools/check-docs.sh --claims
check-docs: 70 Markdown files (57 docs/, 3 root, 10 agent instructions), 237 relative links, 48 heading anchors, 442 code references
check-docs: claims audit: 92 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s)
check-docs: OK
EXIT=0

$ bash tools/check-docs.sh --readme
check-docs: 70 Markdown files (57 docs/, 3 root, 10 agent instructions), 237 relative links, 48 heading anchors, 442 code references
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
EXIT=0

$ bash tools/check-reference-pages.sh
check-reference-pages: 27 scoped page(s): 16 manifested, 11 declared non-reference, every page classified
check-reference-pages: 112 required sections in order and non-empty
check-reference-pages: OK
EXIT=0

$ bash tools/test-docs-gates.sh
… 26 PASS lines …
WIRE ci.yml: the docs job runs the crate-example and tutorial gates, both example runners, the three check-docs modes, the reference-page gate and this mutation self-test on the Ubuntu+Windows matrix; no conditional skip
test-docs-gates: 26 mutation(s), 26 passed, 0 failed
test-docs-gates: OK (26 mutations, every gate failed for its intended reason)
EXIT=0
```

The claims audit reports 92 pages because this report is the 92nd page under `docs/`; the link and
symbol scans report 70 files because they exclude the historical `docs/goal-docs-*` trees.

The other gates the documentation job runs, executed for completeness:

```console
$ bash docs/extensions/examples/run-examples.sh
pages: 4   examples checked: 27   passed: 27   failed: 0   unpaired: 0
OK: 27 examples match the documented output.                       # EXIT=0

$ bash tools/check-crate-examples.sh
check-crate-examples: 13 page(s), 7 rust fence(s): 7 compiled and ran, 0 excluded
check-crate-examples: OK                                          # EXIT=0

$ bash tools/run-tutorials.sh
run-tutorials: 4 tutorials checked (linear-algebra symbolic signal-processing reproducible)
run-tutorials: 1 independent oracle(s)
run-tutorials: OK                                                 # EXIT=0
```

`ci.yml` was not weakened: no `continue-on-error`, no conditional `if:`, no `|| true` was added to a
step (the two `|| true` hits in the file are inside the Linux glibc guard's command substitution,
pre-existing), and every `docs` step is unconditional on both matrix variants.

---

## 5. Final `docs` job definition in `.github/workflows/ci.yml` (twelve steps)

```yaml
  docs:
    name: Documentation examples (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest]
    steps:
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

      - name: Compile and run the crate-page examples
        shell: bash
        run: bash tools/check-crate-examples.sh

      - name: Run the tutorials
        shell: bash
        run: bash tools/run-tutorials.sh

      - name: Run the language and extension examples
        shell: bash
        run: |
          set -uo pipefail
          status=0
          bash docs/language/examples/run-examples.sh || status=1
          bash docs/extensions/examples/run-examples.sh || status=1
          exit "$status"

      - name: Check documentation links and symbol citations
        shell: bash
        run: bash tools/check-docs.sh --symbols

      - name: Check documentation claims and tutorial precision
        shell: bash
        run: bash tools/check-docs.sh --claims

      - name: Check the README structure
        shell: bash
        run: bash tools/check-docs.sh --readme

      - name: Check reference pages
        shell: bash
        run: bash tools/check-reference-pages.sh

      - name: Test the documentation gates (mutation)
        shell: bash
        run: bash tools/test-docs-gates.sh
```

(Comments omitted here; they remain in the file. The step count is twelve, and the `docs` job header
comment now names the catalogue check.)

---

## 6. Findings outside my write scope

1. **The PowerShell twin is stale.** `docs/language/examples/run-examples.ps1` still looks for
   `^## Registry at a glance`, `^## Full catalogue` and `^### <Category> (N builtins)`, but the page
   has used `### Registry at a glance`, `### Full catalogue` and `#### <Category> (N builtins)` since
   at least `d946316` (`git show HEAD:docs/language/builtins.md`). Run today it would fail with
   "the Registry at a glance table has no Total row"; it is not executed by CI, but
   `docs/language/README.md:62` still documents `pwsh -File docs/language/examples/run-examples.ps1`.
   Recommended: repair or retire it, and update that README passage. Not changed here because it is
   outside the assigned files.
2. **`lovelace-corpus` accepts a zero-match filter.** `lovelace-corpus lang/999` prints
   `corpus: 0 passed, 0 failed, 214 total (filter lang/999)` and **exits 0**. The instructions now
   require the count (a run with `0 passed` is a failed check), but the binary itself is unchanged;
   only a code change in `lovelace-corpus/src/main.rs` can make it exit non-zero.
3. **Retired claim vocabulary elsewhere.** `docs/adr/004-thirty-digit-default.md:100`,
   `docs/crates/lovelace-bench.md:45`, `docs/crates/lovelace-lang.md:58`, and the diagnostics in
   `tools/check-docs.sh:486,529` still say "precision-bounded" / a three-way split. They are in other
   owners' files; the four-way vocabulary is now consistent in every file I own.
4. **`--functions` default listing is truncated** (`returned:50`, `total:124`,
   `truncated:true`). `builtins.md` already uses `--function-limit 500`; the instruction that
   misread the default listing is fixed, but the default itself is a usability trap for any future
   caller.

---

## 7. Reproduction

```console
# from the repository root, WSL Ubuntu
export PATH=/home/ricardo/.cargo/bin:$PATH
export CARGO_TARGET_DIR=/tmp/rl-target
cargo build -p lovelace-run

bash docs/language/examples/run-examples.sh           # 193 examples + "catalogue: 124 builtins in 11 categories"
cp docs/language/builtins.md /tmp/builtins.orig
sed -i 's/^| `is_even` | 1 | x |$/| `is_even_renamed` | 1 | x |/' docs/language/builtins.md
bash docs/language/examples/run-examples.sh; echo "exit=$?"   # exit=1, two builtins.md:<line> diagnostics
cp /tmp/builtins.orig docs/language/builtins.md
sha256sum docs/language/builtins.md                  # 830e16ee8809989e726ab6034b1570a3e0996fca90013fb8f8f3b0d245ee751f

bash tools/check-docs.sh && bash tools/check-docs.sh --symbols && \
bash tools/check-docs.sh --claims && bash tools/check-docs.sh --readme && \
bash tools/check-reference-pages.sh && bash tools/test-docs-gates.sh
```

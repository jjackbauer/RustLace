# Round 8 — the Windows crate-example manifest was POSIX

**Branch:** `docs/documentation-quality-remediation`
**HEAD:** `89ea5c7` (`docs: close the documentation-review follow-up (RQ-001 to RQ-024)`)
**Recorded:** 2026-11-16T14:01Z, WSL Ubuntu (cargo 1.100.0) and Git Bash
`MINGW64_NT-10.0-26100` on the same checkout; repository root mounted as
`/mnt/c/Users/ricar/dev/RustLace` in WSL and reported as `/c/Users/ricar/dev/RustLace` by
Git Bash.
**Objective:** fix the fed-back Windows failure of `check-crate-examples` (GitHub run
43878298949, job `Documentation examples (windows-latest)`, step
`Compile and run the crate-page examples`) by giving every path this script writes into a
generated file the native spelling of the host, and make the failure mode explain itself.

**Scope guard:** only `tools/check-crate-examples.sh`, `tools/test-docs-gates.sh` and this report
were edited. `tools/run-tutorials.sh` and `tools/docs-report.sh` are untouched (`git status`
confirms). `.github/**`, every other Markdown file, `tools/check-docs.sh` and
`tools/check-reference-pages.sh` were not changed by this round. (`tools/check-docs.sh` and
`tools/test-docs-gates.sh` already had round-7 modifications in the working tree before this round;
only the additions described here are new.)

**Result:** the generated `Cargo.toml` now records `D:\...` on a Git Bash/MSYS/Cygwin host and
`/mnt/c/...` on Linux; the fix was reproduced by running the pre-fix checker and the fixed checker
against the same pages on Git Bash; the new diagnostic names the path defect when it recurs; the
mutation self-test grew from 30 to 31 mutations, all PASS; and every Linux gate is still green.

## 1. The failure

The hosted step reported:

```console
check-crate-examples: cargo: /c/Users/runneradmin/.cargo/bin/cargo (from PATH)
error: failed to load manifest for dependency `lovelace-array`
  failed to read `C:\d\a\RustLace\RustLace\lovelace-array\Cargo.toml`
  The system cannot find the path specified. (os error 3)
check-crate-examples: the crate-page examples do not compile; a syntax/API change is a build failure
DOCREPORT gate=check-crate-examples status=fail ... failures=1
```

The cause is the one already diagnosed: the generated temporary `Cargo.toml` carried
`/d/a/RustLace/RustLace/lovelace-array`. Git Bash translates **argv** for a native binary, and it
translates path-like **environment variables** too, but it does not touch the **contents** of a
file the native binary reads. Native Windows cargo opened the POSIX spelling as
`C:\d\a\RustLace\RustLace\lovelace-array`.

That environment-variable half was measured on this host, so the report does not over-claim the
cause and does not "fix" what MSYS already converts:

```console
$ export CARGO_TARGET_DIR=/tmp/rl-target-test
$ cmd.exe //c "set CARGO_TARGET_DIR"
CARGO_TARGET_DIR=C:/Users/ricar/AppData/Local/Temp/rl-target-test
```

The old `toml_path` only invoked `cygpath` when the argument already looked like a Windows path, so
a Git Bash `/d/...` root never matched and was written unchanged:

```bash
toml_path() {
    p=$1
    case "$p" in
        [A-Za-z]:[\\/]* | *\\*)          # /d/a/RustLace/... does not match either arm
            if command -v cygpath >/dev/null 2>&1; then
                p=$(cygpath -m -- "$p") || return 1
            fi
            ;;
    esac
    printf '%s' "$p" | sed 's/\\/\\\\/g'
}
```

## 2. The fix — convert every manifest path, not the observed one

`tools/check-crate-examples.sh` now detects `cygpath` once and converts every path it writes into
the manifest when it exists; elsewhere the absolute path is already native. The whole code path is:

```bash
# tools/check-crate-examples.sh:325-343
cygpath_bin=
if command -v cygpath >/dev/null 2>&1; then
    cygpath_bin=$(command -v cygpath)
fi

# toml_path ABSOLUTE_PATH - the value to put in a TOML basic string. On a
# host with cygpath the path is converted to its native spelling first, then
# escaped: the value is a TOML basic string, where a backslash introduces an
# escape, so the backslashes `cygpath -w` produces must be doubled. On every
# other host the absolute path is already native and passes through.
toml_path() {
    p=$1
    if [ -n "$cygpath_bin" ]; then
        p=$("$cygpath_bin" -w -- "$p") || {
            printf 'check-crate-examples: cygpath -w failed for %s\n' "$1" >&2
            return 1
        }
    fi
    printf '%s' "$p" | sed 's/\\/\\\\/g; s/"/\\"/g'
}
```

The single line that decides the host's answer is `if [ -n "$cygpath_bin" ]; then`. On WSL Ubuntu
`command -v cygpath` fails, `cygpath_bin` stays empty, and the branch is skipped; in Git Bash it is
`/usr/bin/cygpath`, the branch runs `cygpath -w`, and the `sed` doubles each backslash for TOML.
The generator itself is otherwise unchanged, and every dependency is converted rather than only the
one that failed:

```bash
# tools/check-crate-examples.sh:345-359
manifest_file=$tmp/Cargo.toml
{
    ...
    # No member paths: this table only stops the generated package from
    # joining the enclosing workspace.
    printf '[workspace]\n\n'
    printf '[dependencies]\n'
    for dep in $deps; do
        dep_path=$(toml_path "$repo_root/$dep") || exit 2
        printf '%s = { path = "%s" }\n' "$dep" "$dep_path"
    done
} > "$manifest_file"
```

What was audited rather than changed:

- **`[workspace]` members:** the table is empty. There are no member paths to convert; the only
  `path` keys in the file are the seven dependency entries, and all seven go through `toml_path`.
- **Every other file the script writes:** the generated `src/bin/fence_NN.rs` files (a
  repository-relative `docs/crates/x.md:NN` comment plus the fence body) and the TSV maps. Native
  cargo and rustc read only `src/bin/*.rs` (from the source, not from a path field) and the
  `Cargo.toml`; no native program resolves a temp path out of the TSV or the page name.
- **The build directory:** `CARGO_TARGET_DIR` is a path-like environment variable, which MSYS
  converts on its own (measured above), and the run phase resolves the executables through the
  POSIX spelling that this shell understands. No change was needed there, and none was made.
- **Failure handling:** a failing `cygpath -w` is now a setup failure (`exit 2`, message) instead
  of silently writing an empty `path = ""`.

## 3. What the manifest contains on each host

Both files below are the real `$tmp/Cargo.toml` written by the modified script, not a re-creation.
On WSL Ubuntu (the run that also compiled and executed all seven fences):

```console
$ bash tools/check-crate-examples.sh --keep
check-crate-examples: 13 page(s), 7 rust fence(s): 7 compiled and ran, 0 excluded
check-crate-examples: OK
DOCREPORT gate=check-crate-examples status=ok pages=13 fences=7 compiled=7 excluded=0 failures=0
check-crate-examples: kept temporary crate at /tmp/rl-crate-examples.OsDRc9

$ cat /tmp/rl-crate-examples.OsDRc9/Cargo.toml
[package]
name = "rustlace-crate-examples-check"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
lovelace-array = { path = "/mnt/c/Users/ricar/dev/RustLace/lovelace-array" }
lovelace-int = { path = "/mnt/c/Users/ricar/dev/RustLace/lovelace-int" }
lovelace-json = { path = "/mnt/c/Users/ricar/dev/RustLace/lovelace-json" }
lovelace-lang = { path = "/mnt/c/Users/ricar/dev/RustLace/lovelace-lang" }
lovelace-nat = { path = "/mnt/c/Users/ricar/dev/RustLace/lovelace-nat" }
lovelace-real = { path = "/mnt/c/Users/ricar/dev/RustLace/lovelace-real" }
lovelace-symbolics = { path = "/mnt/c/Users/ricar/dev/RustLace/lovelace-symbolics" }
```

On this host's Git Bash, running the same script with the same pages (the repository's `/c/...`
spelling standing in for the runner's `/d/...`; the shape is identical):

```console
$ bash tools/check-crate-examples.sh          # with the stand-in cargo of section 4
[dependencies]
lovelace-array = { path = "C:\\Users\\ricar\\dev\\RustLace\\lovelace-array" }
lovelace-int = { path = "C:\\Users\\ricar\\dev\\RustLace\\lovelace-int" }
lovelace-json = { path = "C:\\Users\\ricar\\dev\\RustLace\\lovelace-json" }
lovelace-lang = { path = "C:\\Users\\ricar\\dev\\RustLace\\lovelace-lang" }
lovelace-nat = { path = "C:\\Users\\ricar\\dev\\RustLace\\lovelace-nat" }
lovelace-real = { path = "C:\\Users\\ricar\\dev\\RustLace\\lovelace-real" }
lovelace-symbolics = { path = "C:\\Users\\ricar\\dev\\RustLace\\lovelace-symbolics" }
```

`C:\\Users\\...` is one backslash per letter after TOML unescaping, which is what Windows cargo
opens; `cygpath -w -- /c/Users/ricar/dev/RustLace` printed `C:\Users\ricar\dev\RustLace` in the same
shell, and the conversion plus `sed` is the only difference between the two dumps above.

## 4. Reproduction of the defect on a cygpath host

This host has no Windows Rust toolchain (`C:\Users\ricar\.cargo\bin\cargo.exe` is absent and
`command -v cargo` is empty in Git Bash), so the reproduction uses a stand-in `cargo` that never
compiles. It copies `Cargo.toml` to `$DUMP` and then judges the first recorded path the way Windows
cargo did: a leading `/` produces exactly the CI message shape, any other path is reported as
native. This is evidence for the manifest text and the diagnostic, not for compilation.

The pre-fix checker from `HEAD` and the fixed checker were run against the same thirteen pages in
the same Git Bash, with the same stand-in cargo:

```console
$ git show HEAD:tools/check-crate-examples.sh > tools/.check-crate-examples-head.sh
$ CARGO=<stand-in> bash tools/.check-crate-examples-head.sh     # pre-fix
$ CARGO=<stand-in> bash tools/check-crate-examples.sh           # fixed
```

Pre-fix manifest:

```console
lovelace-array = { path = "/c/Users/ricar/dev/RustLace/lovelace-array" }
...
```

Pre-fix log — cargo's message and nothing else, the same shape as the hosted failure:

```console
error: failed to load manifest for dependency `lovelace-array`
  failed to read `C:\c\Users\ricar\dev\RustLace\lovelace-array`:
  The system cannot find the path specified. (os error 3)
check-crate-examples: the crate-page examples do not compile; a syntax/API change is a build failure
DOCREPORT gate=check-crate-examples status=fail pages=13 fences=7 compiled=7 excluded=0 failures=1
```

Fixed manifest: the `C:\\...` block quoted in section 3. The fixed checker was also run with a
`cygpath` that exists but returns its argument unchanged — the exact manifest shape the old code
produced — to exercise the new diagnostic; section 5 quotes it.

## 5. TASK 3 — the failure mode now explains itself

The build-failure branch adds one explanation, only when **all** of these hold: `cygpath` exists,
cargo said `failed to load manifest`, and the log carries a path-not-found marker (`The system
cannot find the path specified`, `os error 3`, or `No such file or directory`). The existing
messages, the `DOCREPORT` line and the exit code are untouched; the block is purely additive:

```bash
# tools/check-crate-examples.sh:451-474
    if [ -n "$cygpath_bin" ] &&
        grep -qF 'failed to load manifest' "$build_log" &&
        grep -qE 'The system cannot find the path specified|os error 3|No such file or directory' "$build_log"; then
        printf '%s\n' \
            'check-crate-examples: the generated manifest records a path that the native cargo cannot open.' \
            'check-crate-examples: this host has cygpath, so the manifest must hold the native spelling that' \
            'check-crate-examples: `cygpath -w` returns. Git Bash translates argv for a native program, never' \
            'check-crate-examples: the contents of a file it reads, so a POSIX path here is opened as' \
            'check-crate-examples: C:\d\a\... and does not exist.' \
            "check-crate-examples: the manifest is $manifest_file and recorded:" >&2
        grep 'path = ' "$manifest_file" | sed 's/^/  /' >&2
        printf 'check-crate-examples: check the native spelling with: cygpath -w -- %s\n' "$repo_root" >&2
    fi
    printf 'check-crate-examples: the crate-page examples do not compile; a syntax/API change is a build failure\n' >&2
    printf 'DOCREPORT gate=check-crate-examples status=fail pages=%d fences=%d compiled=%d excluded=%d failures=1\n' \
        "${#pages[@]}" "$fences" "$compiled" "$excluded"
    exit 1
```

Executed on Git Bash with the non-converting cygpath, against the real repository pages:

```console
error: failed to load manifest for dependency `lovelace-array`
  failed to read `C:\c\Users\ricar\dev\RustLace\lovelace-array`:
  The system cannot find the path specified. (os error 3)
check-crate-examples: the generated manifest records a path that the native cargo cannot open.
check-crate-examples: this host has cygpath, so the manifest must hold the native spelling that
check-crate-examples: `cygpath -w` returns. Git Bash translates argv for a native program, never
check-crate-examples: the contents of a file it reads, so a POSIX path here is opened as
check-crate-examples: C:\d\a\... and does not exist.
check-crate-examples: the manifest is /tmp/rl-crate-examples.0VBrg3/Cargo.toml and recorded:
  lovelace-array = { path = "/c/Users/ricar/dev/RustLace/lovelace-array" }
  ... seven lines ...
check-crate-examples: check the native spelling with: cygpath -w -- /c/Users/ricar/dev/RustLace
check-crate-examples: the crate-page examples do not compile; a syntax/API change is a build failure
DOCREPORT gate=check-crate-examples status=fail pages=13 fences=7 compiled=7 excluded=0 failures=1
```

The same log without the hint (section 4) is the pre-fix behaviour, and the two differ only by the
added lines: the exit status is still 1 and the `DOCREPORT` line is byte-for-byte the same.

## 6. TASK 2 — `run-tutorials.sh` has no instance of this defect

Audited, not changed. It already has the conversion helper and it uses it for every path it hands
to the native runner:

```bash
# tools/run-tutorials.sh:102-119
# MSYS translates POSIX paths for native executables inconsistently, so
# every path handed to lovelace-run goes through cygpath when it exists.
...
native_path() {
    if command -v cygpath >/dev/null 2>&1; then
        cygpath -w -- "$1"
    else
        printf '%s' "$1"
    fi
}
```

and the only two places that pass a path to `lovelace-run` are argv, already wrapped
(`tools/run-tutorials.sh:583` `--file "$(native_path "$scratch/$name.example")"`,
`tools/run-tutorials.sh:660-662` the same `--file` plus `--plot-dir "$(native_path "$scratch/plots")"`).

For the second half of the question — does it write a path into a file? The files it creates are
`$scratch/<name>.source` and `.example` (page text and the checked-in script), `.expected` (the
recorded block), `.out`/`.err` (runner output), `.norm` (normalised copies) and `.fields` (JSON
extracts). None of them is read by a native program as a *path source*: the runner reads only the
`--file` argument and creates `--plot-dir` itself, and the comparison side reads the output with
`sed`/`cmp`/`awk`. The example script itself is checked-in tutorial source with no host path in it
(the static rule rejects `/tmp` in any tutorial page). **No defect of this class; no change made.**

## 7. Linux verification (WSL Ubuntu, this checkout)

`bash tools/check-crate-examples.sh` — exit 0:

```console
check-crate-examples: cargo: /home/ricardo/.cargo/bin/cargo (from PATH)
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
DOCREPORT gate=check-crate-examples status=ok pages=13 fences=7 compiled=7 excluded=0 failures=0
```

`bash tools/run-tutorials.sh` — exit 0, four PASS plus the independent oracle:

```console
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
DOCREPORT gate=run-tutorials status=ok tutorials=4 fences=8 oracles=1 failures=0
```

`bash tools/docs-report.sh` — exit 0, 4/4 gates:

```console
DOCREPORT gate=check-crate-examples status=ok pages=13 fences=7 compiled=7 excluded=0 failures=0
DOCREPORT gate=run-tutorials status=ok tutorials=4 fences=8 oracles=1 failures=0
check-docs: 71 Markdown files (58 docs/, 3 root, 10 agent instructions), 251 relative links, 49 heading anchors, 498 code references, 21 line-suffix citations, 22 symbol citations
check-docs: claims audit: 97 pages under docs/, 5 tutorial pages, 0 prohibited phrase(s), 0 retired-term use(s)
check-docs: README.md structural audit: 4 requirements checked
check-docs: OK
DOCREPORT gate=check-docs status=ok pages=71 links=251 anchors=49 refs=498 citations=21 symbols=22 claims=0 tutorials=5 sections=5 readme=4 modes=symbols,claims,readme failures=0
check-reference-pages: 27 scoped page(s): 16 manifested, 11 declared non-reference, every page classified
check-reference-pages: 112 required sections in order and non-empty
check-reference-pages: OK
DOCREPORT gate=check-reference-pages status=ok pages=27 manifested=16 non_reference=11 unclassified=0 sections=112 failed_pages=0 failures=0
DOCREPORT total gates=4 ok=4 failed=0 commit=89ea5c7 os=Linux
```

That `docs-report.sh` run was recorded with this report already on disk: the report is itself a page
under `docs/`, so the claims audit measures 97 pages rather than the 96 it measured before the file
existed. The four gate statuses and the other counts are unchanged.

`bash tools/test-docs-gates.sh` — exit 0, including the new case:

```console
CTRL crate-examples: cygpath host: clean fixture passed
PASS crate-examples: POSIX path in the generated manifest (exit 1)
WIRE ci.yml: the docs job runs the crate-example and tutorial gates, both example runners, the three check-docs modes, the reference-page gate, the evidence report and this mutation self-test on the Ubuntu+Windows matrix; no conditional skip

test-docs-gates: 31 mutation(s), 31 passed, 0 failed
test-docs-gates: OK (31 mutations, every gate failed for its intended reason)
```

## 8. The new self-test case (mutation 31)

`tools/test-docs-gates.sh` now builds a scratch repository the real checker can run in: one
`docs/crates/*.md` page with one Rust fence, the seven workspace-crate `Cargo.toml` files, a fake
`cygpath` and a fake `cargo`.

- **Baseline (counted as `CTRL`, not as a mutation):** the fake `cygpath -w` converts the way Git
  Bash does and the fake cargo *requires* every manifest path to be drive-letter native
  (`^[A-Za-z]:[\\/]`). The clean fixture therefore proves conversion happened on a cygpath host,
  and it must also print `compiled=1 excluded=0 failures=0`.
- **Mutation:** `cygpath` is replaced by a script that exists but returns the POSIX path unchanged —
  the manifest shape the pre-fix `toml_path` produced on Git Bash — and the fake cargo emits the
  Windows `failed to load manifest` / `os error 3` message. The real checker must exit 1, print the
  new explanation (`the generated manifest records a path`), and still print
  `DOCREPORT ... status=fail`.

Because both fakes are shell scripts in the fixture, the case runs identically on Ubuntu and on
Git Bash, so the Windows documentation job exercises the conversion path even though its own cargo
is real. Mutation count: **30 → 31**; one `CTRL` baseline was added; no existing mutation changed.

## 9. What could not be verified here

- **Windows execution.** This host is WSL Ubuntu plus Git Bash on a Windows filesystem; it has no
  Windows Rust toolchain (`C:\Users\ricar\.cargo\bin\cargo.exe` is absent and Git Bash has no
  `cargo`). No real Windows cargo ever loaded the new manifest, so the `\\` → `\` TOML round trip
  and the `D:`-drive shape on the hosted runner (`D:\a\RustLace\RustLace`) are reasoned from the
  TOML and `cygpath` specifications and from this host's Git Bash, not executed by Windows cargo.
  The Git Bash dumps in section 3 are the closest observation available here; they use a stand-in
  cargo that judges the text and never compiles.
- **Hosted CI (RQ-013).** The branch has not been pushed and no GitHub Actions run exists for this
  change, so the required hosted Ubuntu **and** Windows green runs are still outstanding. This
  round removes the diagnosed cause of the Windows failure; it does not supply the hosted record.

## 10. Residual risk and scope

- If `cygpath` were absent on a Git Bash-style host, the manifest would be POSIX again and Windows
  cargo would fail — but now with the section-5 explanation instead of only `os error 3`. Git Bash,
  MSYS and Cygwin always ship `cygpath`, so this is a safety net, not the normal path.
- A failing `cygpath -w` now exits 2 (setup) with a named message rather than writing an empty
  `path = ""` that cargo would report as a confusing parse error. As with the other setup failures
  (missing crate, missing cargo), no `DOCREPORT` line is printed, and `docs-report.sh` records the
  gate as `summary=missing`; that is the pre-existing contract for exit 2.
- `toml_path` escapes backslashes and double quotes only. A repository path is not expected to
  contain other TOML control characters; the seven dependency paths are `<repo>/lovelace-*`.
- RQ mapping: this is the Windows leg of RQ-006 (every implemented consistency mode runs in CI) and
  RQ-010 (crate reference examples compile and execute on both hosts), and it unblocks the Windows
  half of RQ-013. RQ-013 itself still requires the hosted run.
- `git diff --check` is clean; `tools/run-tutorials.sh`, `tools/docs-report.sh`, `.github/**` and
  every Markdown page other than this report are untouched. The pre-fix checker was never committed
  in modified form: the HEAD copy was materialised under `tools/` for one run and removed in the
  same script, and `git status --short -- tools` after the evidence run showed only the intended
  modified files.

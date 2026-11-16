# Round 9 — the hosted run that closes C13

## 1. What this round had to settle

C13 was the only open check. It reads:

> `gh run list` / `gh pr checks` at the final commit — Ubuntu **and** Windows docs job green, plus
> the platform build/test jobs; run IDs recorded.

At `89ea5c7` the answer was no: run **43878298949** was 3 of 4, with
`Documentation examples (windows-latest)` failing at `Compile and run the crate-page examples`
(EVD-057). Round 8 diagnosed and fixed the cause — the generated temporary `Cargo.toml` carried
POSIX paths, which Git Bash does not translate for a native child's file reads — but a fix is not
a record. This round produces the record.

## 2. The commit

Rounds 7 and 8 were uncommitted when the working tree moved from `C:\Users\ricar\dev\RustLace` to
`F:\dev\RustLace` (the move that ended the previous run with `ENOSPC: no space left on device`).
They are committed as:

```
b0ed8f5 fix(docs-gates): native manifests on Git Bash, plus two checker rules
```

Nine files: `tools/check-crate-examples.sh`, `tools/check-docs.sh`, `tools/test-docs-gates.sh`,
`docs/reference-pages.md`, `docs/goal-docs-v2/{evidence,journal,state}.md`, and the two round
reports. `tools/run-tutorials.sh` and `.github/**` are untouched, because neither needed a change:
the tutorial runner already routes every path it hands to `lovelace-run` through `cygpath`.

A stray zero-byte file named `exit=1`, created by a mis-quoted shell command during the round, was
staged by `git add -A` and removed in the same commit before it was pushed.

## 3. Local gates on the committed tree

All eleven gates pass at `F:\dev\RustLace` under WSL Ubuntu:

```
check-docs (default)      ok  pages=71 links=251 anchors=49 refs=498 failures=0
check-docs --symbols      ok  citations=21 symbols=22 failures=0
check-docs --claims       ok  claims=0 tutorials=5 sections=5 failures=0
check-docs --readme       ok  readme=4 failures=0
check-reference-pages     ok  27 pages: 16 manifested, 11 non-reference, 112 sections
check-crate-examples      ok  13 pages, 7 fences, 7 compiled and ran
run-tutorials             ok  4 tutorials, 8 fences, 1 oracle
test-docs-gates           ok  31 mutations, 31 passed, 0 failed
docs-report               ok  total gates=4 ok=4 failed=0
language examples         ok  All 193 examples passed
extension examples        ok  27 passed, 0 failed
```

`cargo test -p lovelace-lang movingavg` is 9 passed, 0 failed, and the four value contracts hold
(EVD-062). This is a re-reproduction, not a re-statement.

## 4. The manifest on a real Git Bash host

The hosted Windows job runs these steps with `shell: bash`, which on `windows-latest` is Git Bash.
That host has `cygpath`, and it is the one environment where the round-8 fix can be observed
directly rather than reasoned about. With a stand-in cargo that records the manifest and then
fails exactly as the hosted job did:

```
lovelace-array   = { path = "F:\\dev\\RustLace\\lovelace-array" }
lovelace-int     = { path = "F:\\dev\\RustLace\\lovelace-int" }
lovelace-json    = { path = "F:\\dev\\RustLace\\lovelace-json" }
lovelace-lang    = { path = "F:\\dev\\RustLace\\lovelace-lang" }
lovelace-nat     = { path = "F:\\dev\\RustLace\\lovelace-nat" }
lovelace-real    = { path = "F:\\dev\\RustLace\\lovelace-real" }
lovelace-symbolics = { path = "F:\\dev\\RustLace\\lovelace-symbolics" }
```

Decoding each TOML basic string and resolving it with the native Windows API gives `True` for all
seven. The pre-fix spellings do not resolve:

```
/f/dev/RustLace/lovelace-array          -> False
/d/a/RustLace/RustLace/lovelace-array   -> False
```

The second line is the shape the hosted runner actually has. `cygpath -w --` on it returns
`D:\a\RustLace\RustLace\lovelace-array`, and the script's `sed` doubles the backslashes to
`D:\\a\\RustLace\\RustLace\\lovelace-array`, which decodes back to the native path. This is
EVD-061.

## 5. The hosted record

`CI` run **43902955424**, `workflow_dispatch`, ref `docs/documentation-quality-remediation`,
head SHA `b0ed8f5e27f6223c105e1ceed5d5dbcbcfddb522`, completed **success**:

| Job | Result | Duration | Job id |
|---|---|---|---|
| Windows (full workspace) | success | 8m8s | 133424152063 |
| Linux (engine crates + GUI desktop under Xvfb) | success | 5m18s | 133424152489 |
| Documentation examples (ubuntu-latest) | success | 1m10s | 133424152456 |
| Documentation examples (windows-latest) | success | 6m13s | 133424153047 |

Both documentation jobs uploaded their transcript. The Windows one is the decisive artifact:

```
===== gate 1: bash tools/check-crate-examples.sh =====
check-crate-examples: cargo: /c/Users/runneradmin/.cargo/bin/cargo (from PATH)
PASS docs/crates/lovelace-array.md:83
...
check-crate-examples: 13 page(s), 7 rust fence(s): 7 compiled and ran, 0 excluded
...
===== gate 2: bash tools/run-tutorials.sh =====
run-tutorials: runner: /d/a/RustLace/RustLace/target/debug/lovelace-run.exe
...
DOCREPORT total gates=4 ok=4 failed=0 commit=b0ed8f5 os=MINGW64_NT-10.0-26100
```

The Ubuntu transcript ends with the same total and `os=Linux`. Both are EVD-060; the run itself is
EVD-059.

Two details matter for honesty:

- The tutorial gate is still resolving its runner at `/d/a/RustLace/RustLace/...`, so this is the
  same host and the same POSIX spelling that broke gate 1 at `89ea5c7`. The difference is that the
  generated manifest no longer carries that spelling to native cargo.
- The step that passes is the exact step that failed. It is not a re-ordered job, a skipped step, or
  a retried flake.

## 6. What this closes, and what it does not

C13 is met: Ubuntu and Windows documentation jobs green, plus both platform build/test jobs, at a
named commit, with run IDs recorded. Every other check was already met in rounds 1–8, so the
definition of done is **24/24**.

It does not:

- merge anything to `main` — the branch is for the user to inspect, and `main` moves only through
  the CI gate on a reviewed change;
- open a pull request — C13 accepts `gh run list` or `gh pr checks`, and the dispatch route leaves
  no review artifact the user did not ask for;
- claim a hosted run at *this* report's commit. This report and the ledger rows it cites are a
  documentation-only commit whose own run is the follow-up dispatch; run 43902955424 is the green
  run at the commit that carries the fix.

## 7. Local environment note

The checkout moved to `F:\dev\RustLace` after the `ENOSPC` that ended the previous run. The drive
has ~555 GB free, `gh` moved with the tooling to `F:\dev\.tools\gh\bin\gh.exe` and is still
authenticated as `jjackbauer`, and the WSL toolchain still builds at `/mnt/f/dev/RustLace`. No
repository file records either path; the paths appear only in this harness-memory directory, which
`tools/check-docs.sh` excludes from the current-documentation set.

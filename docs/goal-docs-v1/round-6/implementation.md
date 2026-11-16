# Round 6 - documentation consistency checker

- Repository: RustLace v0.4.1, `HEAD` `bb9d21a75fa732f089fb0c1e177eaf502baf6539`
  (`docs: describe the Linux checksum file accurately`), worktree from earlier rounds.
- Environment: no Windows Rust and no PowerShell in WSL. The checker is bash and was run
  under Ubuntu WSL; one portability run under Windows Git Bash produced the same counts,
  the same findings and exit status 1.
- Deliverables: `tools/check-docs.sh` (new) and one appended step in the existing `docs`
  job of `.github/workflows/ci.yml`.

## 1. What the checker does

`bash tools/check-docs.sh` checks current documentation (every `docs/**/*.md` page except
`docs/goal-desktop-v4/` and `docs/goal-docs-v1/`):

1. **Links.** Every inline Markdown link target that is relative (not `http`, `https`,
   `mailto`, another scheme, `//`, or a pure `#anchor`) must resolve, relative to the
   citing file, to an existing file or directory. Image links are links too. Fenced code
   blocks and inline code spans are not parsed as links.
2. **Code references.** Every inline code span that looks like a repository path
   (contains `/`, ends in `.rs`, `.toml`, `.yml`, `.yaml`, `.json` or `.md`, optionally
   with a `:NNN` or `:NNN-MMM` line suffix) must name a file that exists. The suffix is
   stripped before resolving. Glob and placeholder shapes (`*`, `{}`, `<>`, whitespace)
   are not literal paths and are skipped.
3. **`--symbols`**, in addition to 1 and 2:
   - any `*.rs:NNN` line-suffix citation outside `docs/adr/` is rejected (those records
     are the historical exception);
   - every cited snake_case function name (`lowercase_with_underscores()` in backticks)
     must occur under `lovelace-*/src`; the citing `file:line` is reported when it does
     not.

Code references are resolved against the repository root, the citing file's directory,
`docs/`, `docs/goal-desktop-v4/` (the carried fixture tree), and each crate's root,
`src/` and `tests/` directories. That covers the documentation's two shorter conventions
(`src/lib.rs` in a crate page, `ui/app.rs` in a workbench walkthrough) without pretending
those paths are repository-root paths; a renamed or deleted full path still fails.

Failures print as `file:line: message` on stderr; the summary prints on stdout; exit
status is 0 only when every enabled check passes, 1 on any failure, 2 on bad usage or a
missing tree. The script uses only bash, find, awk, grep, sort and mktemp, so the same
file runs on `ubuntu-latest` bash and on Git Bash on `windows-latest`.

## 2. Falsification

### (a) Run the checker on the current tree

```console
$ bash tools/check-docs.sh
```

Verbatim output (stderr and stdout combined; exit status 1):

```text
docs/adr/002-egui-eframe.md:21: code reference 'docs/goal-desktop-v4/deviations.md' does not name an existing file
docs/adr/008-linux-gui-support.md:8: code reference 'docs/goal-desktop-v4/deviations.md' does not name an existing file
docs/crates/lovelace-corpus.md:29: code reference 'Lovelace.Suite/docs/Language.md' does not name an existing file
docs/parity-with-studio.md:37: code reference 'Lovelace.Suite/docs/Language.md' does not name an existing file
check-docs: 55 Markdown files, 147 relative links, 224 code references
check-docs: FAIL (4 failure(s))
```

The link check itself is clean: 147 relative links checked, 0 broken. The four failures
are pre-existing code-reference defects, classified in section 3. The checker therefore
does **not** pass on the clean tree; weakening it to make this section green was
explicitly not an option. This is the honest result of the requested check.

### (b) Break one relative link in a current page, then restore it

Edited line (`docs/README.md`, line 7) before:

```text
| [architecture.md](architecture.md) | you need the layering, the threading model, cancellation, the incremental cache, or the data flow of one run |
```

Corruption command:

```console
$ perl -pi -e 'if ($. == 7) { s{\(architecture\.md\)}{\(architecture-renamed.md\)} }' docs/README.md
```

The line after corruption:

```text
| [architecture.md](architecture-renamed.md) | you need the layering, the threading model, cancellation, the incremental cache, or the data flow of one run |
```

Checker output after the break (exit status 1; the injected failure is the first line):

```text
docs/README.md:7: broken link 'architecture-renamed.md' (no such file or directory: docs/architecture-renamed.md)
docs/adr/002-egui-eframe.md:21: code reference 'docs/goal-desktop-v4/deviations.md' does not name an existing file
docs/adr/008-linux-gui-support.md:8: code reference 'docs/goal-desktop-v4/deviations.md' does not name an existing file
docs/crates/lovelace-corpus.md:29: code reference 'Lovelace.Suite/docs/Language.md' does not name an existing file
docs/parity-with-studio.md:37: code reference 'Lovelace.Suite/docs/Language.md' does not name an existing file
check-docs: 55 Markdown files, 147 relative links, 224 code references
check-docs: FAIL (5 failure(s))
```

Restore and byte-identity proof:

```console
$ cp -p .tmp/falsify/backup/README.md docs/README.md
$ cat .tmp/falsify/link-restore-hashes.txt
```

```text
before 9e760e73f221aeda99c44e66da1b0ebb4a4fd761
after  9e760e73f221aeda99c44e66da1b0ebb4a4fd761
restored byte-identical
```

Checker output after the restore (exit status 1; byte-identical to the baseline run):

```text
docs/adr/002-egui-eframe.md:21: code reference 'docs/goal-desktop-v4/deviations.md' does not name an existing file
docs/adr/008-linux-gui-support.md:8: code reference 'docs/goal-desktop-v4/deviations.md' does not name an existing file
docs/crates/lovelace-corpus.md:29: code reference 'Lovelace.Suite/docs/Language.md' does not name an existing file
docs/parity-with-studio.md:37: code reference 'Lovelace.Suite/docs/Language.md' does not name an existing file
check-docs: 55 Markdown files, 147 relative links, 224 code references
check-docs: FAIL (4 failure(s))
```

Result: `docs/README.md:7: broken link ...` appears only while the link is broken. The
overall exit status stays 1 after the restore because the four pre-existing findings
remain; the link check has 0 failures before and after.

### (c) Break one path-shaped code reference, then restore it

Edited line (`docs/extending.md`, line 9) before:

```text
1. **Declare the descriptor** in `lovelace-lang/src/registry.rs`. Add a `BuiltinInfo` entry to
```

Corruption command:

```console
$ perl -pi -e 'if ($. == 9) { s{lovelace-lang/src/registry\.rs}{lovelace-lang/src/registry-renamed.rs} }' docs/extending.md
```

The line after corruption:

```text
1. **Declare the descriptor** in `lovelace-lang/src/registry-renamed.rs`. Add a `BuiltinInfo` entry to
```

Checker output after the break (exit status 1; the injected failure is line 4):

```text
docs/adr/002-egui-eframe.md:21: code reference 'docs/goal-desktop-v4/deviations.md' does not name an existing file
docs/adr/008-linux-gui-support.md:8: code reference 'docs/goal-desktop-v4/deviations.md' does not name an existing file
docs/crates/lovelace-corpus.md:29: code reference 'Lovelace.Suite/docs/Language.md' does not name an existing file
docs/extending.md:9: code reference 'lovelace-lang/src/registry-renamed.rs' does not name an existing file
docs/parity-with-studio.md:37: code reference 'Lovelace.Suite/docs/Language.md' does not name an existing file
check-docs: 55 Markdown files, 147 relative links, 224 code references
check-docs: FAIL (5 failure(s))
```

Restore and byte-identity proof:

```console
$ cp -p .tmp/falsify/backup/extending.md docs/extending.md
$ cat .tmp/falsify/ref-restore-hashes.txt
```

```text
before 9e24a6edd7ce6cf76b0070640d4c173ba773754b
after  9e24a6edd7ce6cf76b0070640d4c173ba773754b
restored byte-identical
```

Checker output after the restore (exit status 1; byte-identical to the baseline run):

```text
docs/adr/002-egui-eframe.md:21: code reference 'docs/goal-desktop-v4/deviations.md' does not name an existing file
docs/adr/008-linux-gui-support.md:8: code reference 'docs/goal-desktop-v4/deviations.md' does not name an existing file
docs/crates/lovelace-corpus.md:29: code reference 'Lovelace.Suite/docs/Language.md' does not name an existing file
docs/parity-with-studio.md:37: code reference 'Lovelace.Suite/docs/Language.md' does not name an existing file
check-docs: 55 Markdown files, 147 relative links, 224 code references
check-docs: FAIL (4 failure(s))
```

Result: `docs/extending.md:9: code reference 'lovelace-lang/src/registry-renamed.rs' ...`
appears only while the path is broken. The checker catches a renamed/deleted `.rs`
reference, which is the failure mode the check exists for.

### (d) Run `--symbols` on the current tree

```console
$ bash tools/check-docs.sh --symbols
```

Verbatim output (147 lines; exit status 1):

```text
docs/adr/002-egui-eframe.md:21: code reference 'docs/goal-desktop-v4/deviations.md' does not name an existing file
docs/adr/008-linux-gui-support.md:8: code reference 'docs/goal-desktop-v4/deviations.md' does not name an existing file
docs/architecture.md:25: line-suffix citation 'lovelace-nat/src/lib.rs:1' is not allowed outside docs/adr/
docs/architecture.md:31: line-suffix citation 'lovelace-int/src/lib.rs:1' is not allowed outside docs/adr/
docs/architecture.md:35: line-suffix citation 'lovelace-json/src/lib.rs:1' is not allowed outside docs/adr/
docs/architecture.md:46: line-suffix citation 'lovelace-real/src/lib.rs:1' is not allowed outside docs/adr/
docs/architecture.md:53: line-suffix citation 'lovelace-array/src/lib.rs:1' is not allowed outside docs/adr/
docs/architecture.md:57: line-suffix citation 'lovelace-symbolics/src/lib.rs:1' is not allowed outside docs/adr/
docs/architecture.md:63: line-suffix citation 'lovelace-lang/src/lib.rs:737' is not allowed outside docs/adr/
docs/architecture.md:64: line-suffix citation 'lib.rs:1568' is not allowed outside docs/adr/
docs/architecture.md:68: line-suffix citation 'lib.rs:836' is not allowed outside docs/adr/
docs/architecture.md:70: line-suffix citation 'lib.rs:1031' is not allowed outside docs/adr/
docs/architecture.md:72: line-suffix citation 'lib.rs:1059' is not allowed outside docs/adr/
docs/architecture.md:72: line-suffix citation 'lib.rs:1075' is not allowed outside docs/adr/
docs/architecture.md:73: line-suffix citation 'lib.rs:1369' is not allowed outside docs/adr/
docs/architecture.md:79: line-suffix citation 'lovelace-run/src/main.rs:1' is not allowed outside docs/adr/
docs/architecture.md:84: line-suffix citation 'lovelace-know/src/lib.rs:1' is not allowed outside docs/adr/
docs/architecture.md:88: line-suffix citation 'lovelace-corpus/src/main.rs:1' is not allowed outside docs/adr/
docs/architecture.md:89: line-suffix citation 'lovelace-bench/src/main.rs:1' is not allowed outside docs/adr/
docs/architecture.md:94: line-suffix citation 'lovelace-desktop/src/main.rs:1' is not allowed outside docs/adr/
docs/architecture.md:98: line-suffix citation 'lovelace-desktop/src/workbench.rs:1' is not allowed outside docs/adr/
docs/architecture.md:112: line-suffix citation 'lovelace-desktop/src/execution.rs:10' is not allowed outside docs/adr/
docs/architecture.md:114: line-suffix citation 'execution.rs:19' is not allowed outside docs/adr/
docs/architecture.md:115: line-suffix citation 'execution.rs:11' is not allowed outside docs/adr/
docs/architecture.md:123: line-suffix citation 'lovelace-lang/src/lib.rs:1597' is not allowed outside docs/adr/
docs/architecture.md:130: line-suffix citation 'lovelace-lang/src/lib.rs:190' is not allowed outside docs/adr/
docs/architecture.md:132: line-suffix citation 'lovelace-desktop/src/execution.rs:112' is not allowed outside docs/adr/
docs/architecture.md:140: line-suffix citation 'lovelace-desktop/src/incremental.rs:9' is not allowed outside docs/adr/
docs/architecture.md:144: line-suffix citation 'incremental.rs:58' is not allowed outside docs/adr/
docs/architecture.md:144: line-suffix citation 'incremental.rs:111' is not allowed outside docs/adr/
docs/architecture.md:174: line-suffix citation 'lovelace-desktop/src/settings.rs:1201' is not allowed outside docs/adr/
docs/architecture.md:174: line-suffix citation 'engine.rs:65' is not allowed outside docs/adr/
docs/architecture.md:176: line-suffix citation 'lovelace-desktop/src/document_io.rs:438' is not allowed outside docs/adr/
docs/architecture.md:177: line-suffix citation 'recovery.rs:32' is not allowed outside docs/adr/
docs/build-and-run.md:50: line-suffix citation 'lovelace-run/src/main.rs:70' is not allowed outside docs/adr/
docs/build-and-run.md:122: line-suffix citation 'ui/type_scale.rs:99' is not allowed outside docs/adr/
docs/build-and-run.md:134: line-suffix citation 'lovelace-desktop/src/ui/app.rs:9908' is not allowed outside docs/adr/
docs/build-and-run.md:186: line-suffix citation 'lovelace-desktop/src/main.rs:148' is not allowed outside docs/adr/
docs/build-and-run.md:200: line-suffix citation 'lovelace-desktop/src/settings.rs:1201' is not allowed outside docs/adr/
docs/build-and-run.md:200: line-suffix citation 'engine.rs:65' is not allowed outside docs/adr/
docs/build-and-run.md:201: line-suffix citation 'lovelace-nat/src/lib.rs:85' is not allowed outside docs/adr/
docs/build-and-run.md:202: line-suffix citation 'lovelace-nat/src/lib.rs:105' is not allowed outside docs/adr/
docs/build-and-run.md:203: line-suffix citation 'lovelace-nat/src/lib.rs:811' is not allowed outside docs/adr/
docs/build-and-run.md:204: line-suffix citation 'lovelace-know/src/runner.rs:210' is not allowed outside docs/adr/
docs/build-and-run.md:205: line-suffix citation 'lovelace-run/src/golden_batch.rs:17' is not allowed outside docs/adr/
docs/build-and-run.md:206: line-suffix citation 'lovelace-desktop/src/run_trace.rs:15' is not allowed outside docs/adr/
docs/build-and-run.md:207: line-suffix citation 'lovelace-desktop/src/ui/measure.rs:125' is not allowed outside docs/adr/
docs/build-and-run.md:208: line-suffix citation 'lovelace-desktop/src/engine/fault.rs:18' is not allowed outside docs/adr/
docs/build-and-run.md:209: line-suffix citation 'lovelace-desktop/src/ui/app.rs:999' is not allowed outside docs/adr/
docs/build-and-run.md:210: line-suffix citation 'lovelace-desktop/src/ui/app.rs:996' is not allowed outside docs/adr/
docs/build-and-run.md:211: line-suffix citation 'lovelace-desktop/src/ui/app.rs:801' is not allowed outside docs/adr/
docs/build-and-run.md:212: line-suffix citation 'lovelace-desktop/src/ime_probe.rs:462' is not allowed outside docs/adr/
docs/build-and-run.md:213: line-suffix citation 'lovelace-desktop/src/frame_probe.rs:10' is not allowed outside docs/adr/
docs/build-and-run.md:247: line-suffix citation 'engine.rs:90' is not allowed outside docs/adr/
docs/build-and-run.md:255: line-suffix citation 'settings.rs:1' is not allowed outside docs/adr/
docs/crates/lovelace-array.md:13: line-suffix citation 'lovelace-array/src/lib.rs:1' is not allowed outside docs/adr/
docs/crates/lovelace-bench.md:10: line-suffix citation 'lovelace-bench/src/main.rs:1' is not allowed outside docs/adr/
docs/crates/lovelace-corpus.md:10: line-suffix citation 'lovelace-corpus/src/main.rs:1' is not allowed outside docs/adr/
docs/crates/lovelace-corpus.md:16: line-suffix citation 'lovelace-corpus/src/lib.rs:1' is not allowed outside docs/adr/
docs/crates/lovelace-corpus.md:29: code reference 'Lovelace.Suite/docs/Language.md' does not name an existing file
docs/crates/lovelace-desktop.md:10: line-suffix citation 'lovelace-desktop/src/main.rs:1' is not allowed outside docs/adr/
docs/crates/lovelace-desktop.md:40: line-suffix citation 'engine.rs:4' is not allowed outside docs/adr/
docs/crates/lovelace-int.md:8: line-suffix citation 'lovelace-int/src/lib.rs:1' is not allowed outside docs/adr/
docs/crates/lovelace-json.md:11: line-suffix citation 'lovelace-json/src/lib.rs:1' is not allowed outside docs/adr/
docs/crates/lovelace-know.md:7: line-suffix citation 'lovelace-know/src/lib.rs:1' is not allowed outside docs/adr/
docs/crates/lovelace-know.md:33: line-suffix citation 'lovelace-know/src/runner.rs:201' is not allowed outside docs/adr/
docs/crates/lovelace-lang.md:10: line-suffix citation 'lovelace-lang/src/lib.rs:1' is not allowed outside docs/adr/
docs/crates/lovelace-lang.md:54: line-suffix citation 'lib.rs:871' is not allowed outside docs/adr/
docs/crates/lovelace-nat.md:8: line-suffix citation 'lovelace-nat/src/lib.rs:1' is not allowed outside docs/adr/
docs/crates/lovelace-nat.md:25: line-suffix citation 'lovelace-nat/src/lib.rs:85' is not allowed outside docs/adr/
docs/crates/lovelace-real.md:9: line-suffix citation 'lovelace-real/src/lib.rs:1' is not allowed outside docs/adr/
docs/crates/lovelace-run.md:11: line-suffix citation 'lovelace-run/src/main.rs:1' is not allowed outside docs/adr/
docs/crates/lovelace-run.md:37: line-suffix citation 'main.rs:70' is not allowed outside docs/adr/
docs/crates/lovelace-symbolics.md:10: line-suffix citation 'lovelace-symbolics/src/lib.rs:1' is not allowed outside docs/adr/
docs/crates/lovelace-symbolics.md:21: line-suffix citation 'lovelace-lang/src/lib.rs:47' is not allowed outside docs/adr/
docs/editorial.md:80: line-suffix citation 'builtins.rs:5748' is not allowed outside docs/adr/
docs/extending.md:14: line-suffix citation 'registry.rs:1' is not allowed outside docs/adr/
docs/extending.md:16: line-suffix citation 'builtins.rs:1300' is not allowed outside docs/adr/
docs/extending.md:33: line-suffix citation 'modus.rs:927' is not allowed outside docs/adr/
docs/extending.md:40: line-suffix citation 'lib.rs:883' is not allowed outside docs/adr/
docs/extending.md:48: line-suffix citation 'lovelace-desktop/src/keyboard.rs:279' is not allowed outside docs/adr/
docs/extending.md:52: line-suffix citation 'app.rs:3864' is not allowed outside docs/adr/
docs/extending.md:55: line-suffix citation 'theme.rs:9' is not allowed outside docs/adr/
docs/extending.md:75: line-suffix citation 'lovelace-lang/src/diagnostic.rs:12' is not allowed outside docs/adr/
docs/extending.md:81: line-suffix citation 'lib.rs:1' is not allowed outside docs/adr/
docs/extending.md:95: line-suffix citation 'golden_batch.rs:94' is not allowed outside docs/adr/
docs/extensions/README.md:17: line-suffix citation 'lovelace-lang/src/registry.rs:33-37' is not allowed outside docs/adr/
docs/extensions/README.md:69: line-suffix citation 'lib.rs:830' is not allowed outside docs/adr/
docs/extensions/README.md:70: line-suffix citation 'lib.rs:875' is not allowed outside docs/adr/
docs/extensions/README.md:71: line-suffix citation 'lib.rs:883' is not allowed outside docs/adr/
docs/extensions/README.md:77: line-suffix citation 'lovelace-lang/src/modus.rs:927-933' is not allowed outside docs/adr/
docs/extensions/README.md:88: line-suffix citation 'lib.rs:883-889' is not allowed outside docs/adr/
docs/extensions/README.md:127: line-suffix citation 'builtins.rs:4279' is not allowed outside docs/adr/
docs/extensions/README.md:131: line-suffix citation 'lib.rs:2796-2797' is not allowed outside docs/adr/
docs/extensions/README.md:132: line-suffix citation 'lib.rs:2800-2801' is not allowed outside docs/adr/
docs/extensions/README.md:180: line-suffix citation 'round67_modus.rs:65' is not allowed outside docs/adr/
docs/extensions/dsp.md:91: line-suffix citation 'builtins.rs:4279' is not allowed outside docs/adr/
docs/extensions/dsp.md:91: line-suffix citation 'lib.rs:2146-2150' is not allowed outside docs/adr/
docs/extensions/dsp.md:112: line-suffix citation 'lib.rs:883' is not allowed outside docs/adr/
docs/extensions/dsp.md:114: line-suffix citation 'lib.rs:892' is not allowed outside docs/adr/
docs/extensions/dsp.md:115: line-suffix citation 'lib.rs:2141' is not allowed outside docs/adr/
docs/extensions/dsp.md:255: line-suffix citation 'lib.rs:2168-2177' is not allowed outside docs/adr/
docs/extensions/dsp.md:258: line-suffix citation 'builtins.rs:5686' is not allowed outside docs/adr/
docs/extensions/dsp.md:262: line-suffix citation 'fixed_dsp.rs:17-20' is not allowed outside docs/adr/
docs/extensions/dsp.md:310: line-suffix citation 'fixed_dsp.rs:36-41' is not allowed outside docs/adr/
docs/extensions/mathir.md:38: line-suffix citation 'lib.rs:875' is not allowed outside docs/adr/
docs/extensions/mathir.md:48: line-suffix citation 'builtins.rs:4279' is not allowed outside docs/adr/
docs/extensions/mathir.md:49: line-suffix citation 'lib.rs:2146-2150' is not allowed outside docs/adr/
docs/extensions/mathir.md:206: line-suffix citation 'mathir.rs:35' is not allowed outside docs/adr/
docs/extensions/mathir.md:207: line-suffix citation 'mathir.rs:335-340' is not allowed outside docs/adr/
docs/extensions/mathir.md:208: line-suffix citation 'mathir.rs:375-379' is not allowed outside docs/adr/
docs/extensions/symbolics.md:38: line-suffix citation 'lib.rs:875' is not allowed outside docs/adr/
docs/extensions/symbolics.md:48: line-suffix citation 'builtins.rs:4279' is not allowed outside docs/adr/
docs/extensions/symbolics.md:49: line-suffix citation 'lib.rs:2146-2150' is not allowed outside docs/adr/
docs/extensions/symbolics.md:273: line-suffix citation 'builtins.rs:12013' is not allowed outside docs/adr/
docs/extensions/symbolics.md:275: line-suffix citation 'builtins.rs:7989' is not allowed outside docs/adr/
docs/glossary.md:5: line-suffix citation 'lovelace-nat/src/lib.rs:1' is not allowed outside docs/adr/
docs/glossary.md:6: line-suffix citation 'lovelace-int/src/lib.rs:1' is not allowed outside docs/adr/
docs/glossary.md:7: line-suffix citation 'lovelace-real/src/lib.rs:1' is not allowed outside docs/adr/
docs/glossary.md:9: line-suffix citation 'lovelace-desktop/src/precision_default.rs:56' is not allowed outside docs/adr/
docs/glossary.md:11: line-suffix citation 'lovelace-lang/src/lib.rs:201' is not allowed outside docs/adr/
docs/glossary.md:12: line-suffix citation 'lib.rs:240' is not allowed outside docs/adr/
docs/glossary.md:13: line-suffix citation 'lib.rs:1031' is not allowed outside docs/adr/
docs/glossary.md:14: line-suffix citation 'lib.rs:836' is not allowed outside docs/adr/
docs/glossary.md:14: line-suffix citation 'engine.rs:54' is not allowed outside docs/adr/
docs/glossary.md:15: line-suffix citation 'lovelace-lang/src/modus.rs:1' is not allowed outside docs/adr/
docs/glossary.md:16: line-suffix citation 'modus.rs:927' is not allowed outside docs/adr/
docs/glossary.md:18: line-suffix citation 'lovelace-desktop/src/incremental.rs:1' is not allowed outside docs/adr/
docs/glossary.md:19: line-suffix citation 'execution.rs:55' is not allowed outside docs/adr/
docs/glossary.md:20: line-suffix citation 'lib.rs:1595' is not allowed outside docs/adr/
docs/glossary.md:21: line-suffix citation 'execution.rs:41' is not allowed outside docs/adr/
docs/glossary.md:22: line-suffix citation 'execution.rs:61' is not allowed outside docs/adr/
docs/glossary.md:23: line-suffix citation 'lovelace-desktop/src/recovery.rs:1' is not allowed outside docs/adr/
docs/glossary.md:24: line-suffix citation 'document_io.rs:438' is not allowed outside docs/adr/
docs/glossary.md:25: line-suffix citation 'lovelace-corpus/src/main.rs:1' is not allowed outside docs/adr/
docs/parity-with-studio.md:13: line-suffix citation 'lovelace-lang/src/lib.rs:828' is not allowed outside docs/adr/
docs/parity-with-studio.md:14: line-suffix citation 'lovelace-lang/src/modus.rs:1' is not allowed outside docs/adr/
docs/parity-with-studio.md:18: line-suffix citation 'lovelace-run/src/main.rs:1' is not allowed outside docs/adr/
docs/parity-with-studio.md:26: line-suffix citation 'lovelace-desktop/src/engine.rs:45' is not allowed outside docs/adr/
docs/parity-with-studio.md:27: line-suffix citation 'execution.rs:1' is not allowed outside docs/adr/
docs/parity-with-studio.md:28: line-suffix citation 'execution.rs:112' is not allowed outside docs/adr/
docs/parity-with-studio.md:37: code reference 'Lovelace.Suite/docs/Language.md' does not name an existing file
docs/testing.md:54: line-suffix citation 'document_io.rs:856,886' is not allowed outside docs/adr/
docs/testing.md:57: line-suffix citation 'workbench.rs:6276' is not allowed outside docs/adr/
docs/testing.md:58: line-suffix citation 'netcheck.rs:238' is not allowed outside docs/adr/
check-docs: 55 Markdown files, 147 relative links, 224 code references, 162 line-suffix citations, 17 symbol citations
check-docs: FAIL (145 failure(s))
```

The symbol-lookup branch itself is clean: 17 symbol citations, 7 distinct names
(`new_bare` x8, `assume_clear` x3, `dsp_precision` x2, `to_pretty`, `display_decimal_places`,
`as_str`, `as_arr` once each), all found under `lovelace-*/src`. The failures are 141
line-suffix citations outside `docs/adr/` plus the four code references from (a), i.e.
145 pre-existing findings. The check does **not** pass on the clean tree.

### (e) Residue after the restores

The two touched pages are already modified against `HEAD` on arrival (the checkout is
CRLF against an LF index, and earlier rounds left content edits), so `git status` cannot
show them clean. The evidence that this round left no residue is that their worktree
hashes, their `git status` entries and the checker output are identical before and after:

```console
$ git status --porcelain -- docs/README.md docs/extending.md
```

before the corruptions:

```text
 M docs/README.md
 M docs/extending.md
```

after the restores:

```text
 M docs/README.md
 M docs/extending.md
```

```console
$ cat .tmp/falsify/hashes-before.txt .tmp/falsify/hashes-after.txt
```

```text
docs/README.md   9e760e73f221aeda99c44e66da1b0ebb4a4fd761
docs/extending.md 9e24a6edd7ce6cf76b0070640d4c173ba773754b
after both restores:
docs/README.md    9e760e73f221aeda99c44e66da1b0ebb4a4fd761
docs/extending.md 9e24a6edd7ce6cf76b0070640d4c173ba773754b
```

Byte-identity of the checker runs against the restored tree:

```text
link-restored.log identical to baseline-default.log
ref-restored.log identical to baseline-default.log
```

The two `M` entries are the pre-existing state of this dirty worktree, not an edit from
this round; both hashes match the pre-corruption values exactly. (The remaining round
artifacts are listed in section 5.)

### Extra: the checker can pass, and the symbol branch can fail

The repository tree has pre-existing findings, so the exit-0 path cannot be shown on it.
A synthetic tree outside the repository (two pages, one valid link, one valid code
reference, one symbol) exercises it. It also appends an unknown symbol to show that the
`--symbols` lookup branch fails rather than silently passing:

```console
$ bash tools/check-docs.sh          # synthetic tree
$ bash tools/check-docs.sh --symbols
```

```text
== clean fixture: default ==
check-docs: 2 Markdown files, 1 relative links, 1 code references
check-docs: OK
exit=0

== clean fixture: --symbols ==
check-docs: 2 Markdown files, 1 relative links, 1 code references, 0 line-suffix citations, 1 symbol citations
check-docs: OK
exit=0

== fixture with one unknown symbol: --symbols ==
docs/README.md:7: cited Rust function 'missing_symbol()' is not defined in lovelace-*/src
check-docs: 2 Markdown files, 1 relative links, 1 code references, 0 line-suffix citations, 2 symbol citations
check-docs: FAIL (1 failure(s))
exit=1
```

## 3. Pre-existing failures - the real findings

These are reported verbatim above and must not be hidden. Fixing them is a later round.
The short version: 0 broken relative links, 0 unknown Rust symbols, 4 broken code
references, 141 line-suffix citations outside `docs/adr/`.

### 3.1 Broken code references (4 sites, 2 distinct targets)

| Citing site | Reference | Why it fails |
|---|---|---|
| `docs/adr/002-egui-eframe.md:21` | `docs/goal-desktop-v4/deviations.md` | the carried archived tree has `journeys.md`, `platform-matrix.md`, `reference/`, `round-64/` - there is no `deviations.md` |
| `docs/adr/008-linux-gui-support.md:8` | `docs/goal-desktop-v4/deviations.md` | same missing file |
| `docs/crates/lovelace-corpus.md:29` | `Lovelace.Suite/docs/Language.md` | a C#-repository source path; no such file is carried in this repository |
| `docs/parity-with-studio.md:37` | `Lovelace.Suite/docs/Language.md` | same external source path |

`docs/goal-desktop-v4/journeys.md` also links `deviations.md`, but that is inside the
archived tree and is deliberately not checked. The ADR citations are current docs, so
they are findings. The two `Lovelace.Suite` citations are path-shaped and name no file
here; whether to reword them as external provenance or retarget them to the carried
oracle fixture is a documentation decision for the repair round.

### 3.2 Line-suffix citations outside `docs/adr/` (141 findings, 23 pages)

| Page | Findings |
|---|---:|
| docs/architecture.md | 32 |
| docs/build-and-run.md | 21 |
| docs/glossary.md | 19 |
| docs/extensions/README.md | 10 |
| docs/extending.md | 10 |
| docs/extensions/dsp.md | 9 |
| docs/parity-with-studio.md | 6 |
| docs/extensions/mathir.md | 6 |
| docs/extensions/symbolics.md | 5 |
| docs/testing.md | 3 |
| docs/crates/lovelace-symbolics.md | 2 |
| docs/crates/lovelace-run.md | 2 |
| docs/crates/lovelace-nat.md | 2 |
| docs/crates/lovelace-lang.md | 2 |
| docs/crates/lovelace-know.md | 2 |
| docs/crates/lovelace-desktop.md | 2 |
| docs/crates/lovelace-corpus.md | 2 |
| docs/editorial.md | 1 |
| docs/crates/lovelace-real.md | 1 |
| docs/crates/lovelace-json.md | 1 |
| docs/crates/lovelace-int.md | 1 |
| docs/crates/lovelace-bench.md | 1 |
| docs/crates/lovelace-array.md | 1 |

The 21 citations in `docs/adr/` are accepted as historical. The 141 above are current
documentation and become failures under `--symbols` until the repair round rewrites or
removes them.

## 4. CI change

The `docs` job already existed in the working tree (it is one of the uncommitted
changes from an earlier round; `HEAD` still has only the `windows` and `linux` jobs). This
round appends one step to it - no second job:

```yaml
      # The documentation consistency gate: relative Markdown links, path-shaped code
      # references and Rust symbol citations must resolve. It runs after the example
      # gate so a documentation defect and an example drift are both reported.
      - name: Check documentation consistency
        shell: bash
        run: bash tools/check-docs.sh --symbols
```

Evidence (the worktree is CRLF against an LF index, so `--ignore-cr-at-eol` is used to
show content changes only):

```console
$ git diff --ignore-cr-at-eol --stat -- .github/workflows/ci.yml
```

```text
 .github/workflows/ci.yml | 57 +++++++++++++++++++++++++++++++++++++++++++++++-
 1 file changed, 56 insertions(+), 1 deletion(-)
```

The 56 insertions are the earlier round's uncommitted `docs` job plus this round's
7-line step; the tail of the diff below isolates the step itself.

```console
$ git diff --ignore-cr-at-eol -- .github/workflows/ci.yml | tail -10
```

```text
+          bash docs/language/examples/run-examples.sh || status=1
+          bash docs/extensions/examples/run-examples.sh || status=1
+          exit "$status"
+
+      # The documentation consistency gate: relative Markdown links, path-shaped code
+      # references and Rust symbol citations must resolve. It runs after the example
+      # gate so a documentation defect and an example drift are both reported.
+      - name: Check documentation consistency
+        shell: bash
+        run: bash tools/check-docs.sh --symbols
```

The step runs on both matrix legs (`ubuntu-latest` and `windows-latest` Git Bash).
**Consequence, stated plainly:** because the current tree has 145 `--symbols` findings,
this new step makes the `docs` job red until the repair round clears them. That is the
intended gate for a later round; the alternative - omitting the pre-existing failures or
downgrading the step - would make the checker decorative.

## 5. `git diff --stat` and scope

The worktree was dirty on arrival: 466 tracked files show as modified because the Windows
checkout is CRLF against the LF index, and earlier rounds left content edits and untracked
files. The plain `git diff --stat` is therefore dominated by pre-existing state; it is
reproduced verbatim in Appendix A. The meaningful scoped views:

```console
$ git diff --stat -- .github/workflows/ci.yml
```

```text
 .github/workflows/ci.yml | 391 +++++++++++++++++++++++++++--------------------
 1 file changed, 223 insertions(+), 168 deletions(-)
```

```console
$ git diff --ignore-cr-at-eol --stat
```

```text
 .dsh/skills/release-and-publish/SKILL.md  |  51 +++--
 .github/copilot-instructions.md           |  11 +-
 .github/workflows/ci.yml                  |  57 ++++-
 AGENTS.md                                 |  19 +-
 CONTRACT.md                               |  10 +-
 README.md                                 |  51 ++++-
 docs/README.md                            |   3 +
 docs/adr/001-no-external-crates.md        |  15 +-
 docs/adr/002-egui-eframe.md               |  37 +++-
 docs/adr/004-thirty-digit-default.md      |  73 +++++++
 docs/architecture.md                      |   4 +-
 docs/build-and-run.md                     |   4 +-
 docs/contributing.md                      |   8 +
 docs/crates/lovelace-desktop.md           |   7 +-
 docs/extending.md                         |   2 +-
 docs/extensions/README.md                 |   2 +-
 docs/extensions/dsp.md                    |  44 +++-
 docs/goal-desktop-v4/journeys.md          |   7 +
 docs/goal-desktop-v4/platform-matrix.md   |  23 +-
 docs/language/README.md                   |  17 +-
 docs/language/plots.md                    |  23 +-
 docs/skills.md                            |   5 +-
 docs/testing.md                           |  22 +-
 lovelace-desktop/src/platform_claims.rs   | 342 +++++++++++++++++++++++-------
 lovelace-desktop/src/precision_default.rs |   9 +-
 lovelace-lang/src/builtins.rs             |  23 +-
 lovelace-lang/src/dsp_round20.rs          |  27 +++
 27 files changed, 717 insertions(+), 179 deletions(-)
```

```console
$ git status --porcelain -- tools/check-docs.sh .github/workflows/ci.yml docs/README.md docs/extending.md
```

```text
 M .github/workflows/ci.yml
 M docs/README.md
 M docs/extending.md
?? tools/check-docs.sh
```

`tools/check-docs.sh` is a new untracked file, hence absent from `git diff --stat`.
`docs/README.md` and `docs/extending.md` are the pre-existing `M` entries discussed in
(e); their hashes are unchanged by this round.

## 6. Notes and limitations

- Inline links only, which is what the task specifies. This tree has no reference-style
  link definitions and no link targets split across lines (checked).
- Fenced code blocks are not scanned: a broken link or path citation printed inside an
  example is example output, not documentation navigation. The same rule keeps the three
  `.rs:NNN` strings inside `docs/extensions/README.md` and `docs/testing.md` fenced
  blocks out of `--symbols`, where they are compiler/comment text rather than citations.
- Link anchors are not validated; only the path is. Pure `#anchor` targets are skipped.
- The archive/exclude policy is applied to links by path. Code references are still
  required to name a file when they point into the archived tree, which is why the
  missing `deviations.md` is a finding rather than an exemption.
- Glob, brace, angle-bracket and whitespace-bearing code spans are treated as
  non-paths. Without that, command snippets such as
  `Select-String ... docs/language/plots.md` and placeholders such as
  `.dsh/skills/<name>/SKILL.md` would be false positives.
- The worktree files are CRLF; the AWK line reader strips the trailing `\r` before
  matching, and the script itself is LF so Git Bash and Ubuntu bash see identical bytes.
- Tested under: Ubuntu WSL bash (`55 files, 147 links, 224 code references`; 4 findings,
  exit 1) and Windows Git Bash (same counts, same findings, exit 1; both modes produced
  output byte-identical to the WSL logs); the synthetic clean tree exits 0 and the
  synthetic unknown symbol exits 1.

## Appendix A - full `git diff --stat` (verbatim)

```text
 .dsh/skills/add-acceptance-mode/SKILL.md           |   190 +-
 .dsh/skills/add-builtin/SKILL.md                   |   192 +-
 .dsh/skills/add-language-feature/SKILL.md          |   198 +-
 .dsh/skills/add-modus-plugin/SKILL.md              |   202 +-
 .dsh/skills/add-ui-pane/SKILL.md                   |   200 +-
 .dsh/skills/change-numeric-kernel/SKILL.md         |   222 +-
 .dsh/skills/debug-desktop-issue/SKILL.md           |   212 +-
 .dsh/skills/release-and-publish/SKILL.md           |   351 +-
 .github/copilot-instructions.md                    |   167 +-
 .github/workflows/ci.yml                           |   391 +-
 .github/workflows/release.yml                      |  1076 +-
 AGENTS.md                                          |   559 +-
 CONTRACT.md                                        |   286 +-
 Cargo.lock                                         |  9614 ++---
 Cargo.toml                                         |    62 +-
 LICENSE                                            |    42 +-
 README.md                                          |   399 +-
 docs/README.md                                     |   137 +-
 docs/adr/001-no-external-crates.md                 |    61 +-
 docs/adr/002-egui-eframe.md                        |    37 +-
 docs/adr/003-statement-boundary-cancellation.md    |    76 +-
 docs/adr/004-thirty-digit-default.md               |   131 +-
 docs/adr/005-incremental-cache-keying.md           |    76 +-
 docs/adr/006-atomic-save-and-external-change.md    |    70 +-
 docs/adr/007-theme-tokens-and-contrast.md          |    68 +-
 docs/architecture.md                               |   352 +-
 docs/build-and-run.md                              |   510 +-
 docs/contributing.md                               |   190 +-
 docs/crates/lovelace-array.md                      |    98 +-
 docs/crates/lovelace-bench.md                      |    84 +-
 docs/crates/lovelace-corpus.md                     |   114 +-
 docs/crates/lovelace-desktop-spike.md              |    78 +-
 docs/crates/lovelace-desktop.md                    |   155 +-
 docs/crates/lovelace-int.md                        |    88 +-
 docs/crates/lovelace-json.md                       |    80 +-
 docs/crates/lovelace-know.md                       |   106 +-
 docs/crates/lovelace-lang.md                       |   108 +-
 docs/crates/lovelace-nat.md                        |    92 +-
 docs/crates/lovelace-real.md                       |   100 +-
 docs/crates/lovelace-run.md                        |   116 +-
 docs/crates/lovelace-symbolics.md                  |    86 +-
 .../Lovelace.Run.Tests/fixtures/absent_field.json  |   486 +-
 .../Lovelace.Run.Tests/fixtures/absent_field.ls    |     2 +-
 .../Lovelace.Run.Tests/fixtures/array.json         |   330 +-
 .../Lovelace.Run.Tests/fixtures/array.ls           |     2 +-
 .../Lovelace.Run.Tests/fixtures/capabilities.json  |  3970 +-
 .../Lovelace.Run.Tests/fixtures/capabilities.ls    |     2 +-
 .../Lovelace.Run.Tests/fixtures/compilation.json   |   506 +-
 .../Lovelace.Run.Tests/fixtures/compilation.ls     |     2 +-
 .../Lovelace.Run.Tests/fixtures/complex.json       |   158 +-
 .../Lovelace.Run.Tests/fixtures/complex.ls         |     2 +-
 .../fixtures/error_envelope.json                   |    82 +-
 .../Lovelace.Run.Tests/fixtures/error_envelope.ls  |     2 +-
 .../fixtures/integration_result.json               |   398 +-
 .../Lovelace.Run.Tests/fixtures/limit_result.json  |   470 +-
 .../Lovelace.Run.Tests/fixtures/limit_result.ls    |     2 +-
 .../fixtures/matrix_inverse_result.json            |   542 +-
 .../fixtures/matrix_solve_result.json              |   542 +-
 .../Lovelace.Run.Tests/fixtures/nested_record.json |   842 +-
 .../Lovelace.Run.Tests/fixtures/nested_record.ls   |     2 +-
 .../fixtures/optimization_result.json              |   526 +-
 .../Lovelace.Run.Tests/fixtures/print_purity.json  |    74 +-
 .../Lovelace.Run.Tests/fixtures/print_purity.ls    |     2 +-
 .../Lovelace.Run.Tests/fixtures/record.json        |   914 +-
 .../Lovelace.Run.Tests/fixtures/record.ls          |     2 +-
 .../fixtures/solve_nosolutions.json                |   762 +-
 .../Lovelace.Run.Tests/fixtures/solve_result.json  |  1126 +-
 .../Lovelace.Run.Tests/fixtures/solve_result.ls    |     2 +-
 .../Lovelace.Run.Tests/fixtures/symbolic.json      |   126 +-
 .../Lovelace.Run.Tests/fixtures/symbolic.ls        |     2 +-
 .../Lovelace.Run.Tests/fixtures/system_solve.json  |   754 +-
 .../Lovelace.Run.Tests/fixtures/system_solve.ls    |     2 +-
 .../fixtures/transform_result.json                 |   770 +-
 .../fixtures/transform_result.ls                   |     2 +-
 .../Lovelace.Run.Tests/fixtures/vector.json        |   250 +-
 .../Lovelace.Run.Tests/fixtures/vector.ls          |     2 +-
 .../Lovelace.Symbolics.Tests/UsageExamples.cs.txt  |   206 +-
 docs/csharp-oracle/Lovelace.Symbolics/README.md    |  2368 +-
 docs/extending.md                                  |   202 +-
 docs/extensions/README.md                          |   380 +-
 docs/extensions/dsp.md                             |   650 +-
 docs/extensions/examples/run-examples.ps1          |   396 +-
 docs/extensions/mathir.md                          |   566 +-
 docs/extensions/symbolics.md                       |   792 +-
 docs/glossary.md                                   |    58 +-
 docs/goal-desktop-v4/journeys.md                   |    93 +-
 docs/goal-desktop-v4/platform-matrix.md            |    71 +-
 docs/goal-desktop-v4/reference/REF-basic.json      |  2844 +-
 docs/goal-desktop-v4/reference/REF-clear.json      |  5198 +--
 docs/goal-desktop-v4/reference/REF-digits.json     |  2638 +-
 docs/goal-desktop-v4/reference/REF-error.json      |  2702 +-
 .../goal-desktop-v4/reference/REF-incremental.json |  5894 +--
 docs/goal-desktop-v4/reference/REF-matir.json      |  2740 +-
 docs/goal-desktop-v4/reference/REF-matrix.json     |  5530 +--
 docs/goal-desktop-v4/reference/REF-plot.json       |  2626 +-
 docs/goal-desktop-v4/reference/REF-precision.json  |  2714 +-
 .../reference/REF-side-effects.json                |  2918 +-
 docs/goal-desktop-v4/reference/REF-solve.json      |  6064 +--
 docs/goal-desktop-v4/reference/REF-stream.json     |  2770 +-
 docs/goal-desktop-v4/reference/REF-symbolic.json   |  2872 +-
 docs/goal-desktop-v4/reference/completions.json    |  1328 +-
 docs/goal-desktop-v4/reference/oracle-defects.md   |    90 +-
 .../reference/oracle-syntax-hang.json              |   348 +-
 docs/goal-desktop-v4/round-64/REF-cancel.json      |  5724 +--
 docs/images/README.md                              |   126 +-
 docs/images/capture.ps1                            |  1330 +-
 docs/language/README.md                            |   315 +-
 docs/language/builtins.md                          |  1230 +-
 docs/language/errors-and-diagnostics.md            |   480 +-
 docs/language/examples/run-examples.ps1            |   706 +-
 docs/language/expressions-and-operators.md         |  1226 +-
 docs/language/functions-and-scope.md               |   656 +-
 docs/language/lexical-structure.md                 |   752 +-
 docs/language/plots.md                             |   393 +-
 docs/language/precision-and-formatting.md          |   612 +-
 docs/language/statements-and-control-flow.md       |   808 +-
 docs/language/values-and-types.md                  |   794 +-
 docs/parity-with-studio.md                         |   134 +-
 docs/release.md                                    |   504 +-
 docs/rustlace-notes.md                             |    76 +-
 docs/skills.md                                     |    73 +-
 docs/symbolics/dsh-protocol.md                     |   824 +-
 docs/testing.md                                    |   422 +-
 examples/hello.ls                                  |    28 +-
 examples/roundtrip.rs                              |    38 +-
 knowledge-graph-demo.json                          | 18596 ++++-----
 lovelace-array/Cargo.toml                          |    18 +-
 lovelace-array/src/dense.rs                        |   524 +-
 lovelace-array/src/lib.rs                          |  3152 +-
 lovelace-bench/Cargo.toml                          |    38 +-
 lovelace-bench/src/main.rs                         |   528 +-
 lovelace-corpus/Cargo.toml                         |    28 +-
 lovelace-corpus/src/dotnet_random.rs               |   616 +-
 lovelace-corpus/src/lib.rs                         |    58 +-
 lovelace-corpus/src/main.rs                        |   294 +-
 lovelace-corpus/src/property_assertions.rs         |   900 +-
 lovelace-corpus/src/property_corpus.rs             |   824 +-
 lovelace-desktop-spike/Cargo.lock                  |  9432 ++---
 lovelace-desktop-spike/Cargo.toml                  |    44 +-
 lovelace-desktop-spike/src/app.rs                  |  1460 +-
 lovelace-desktop-spike/src/bench.rs                |  1984 +-
 lovelace-desktop-spike/src/cpu.rs                  |   212 +-
 lovelace-desktop-spike/src/editor.rs               |   792 +-
 lovelace-desktop-spike/src/json.rs                 |   236 +-
 lovelace-desktop-spike/src/main.rs                 |   276 +-
 lovelace-desktop-spike/src/plot.rs                 |   696 +-
 lovelace-desktop-spike/src/sample.rs               |   176 +-
 lovelace-desktop-spike/src/syntax.rs               |   334 +-
 lovelace-desktop/Cargo.toml                        |    96 +-
 lovelace-desktop/src/app.rs                        |  3352 +-
 lovelace-desktop/src/cancel_parity.rs              |  1286 +-
 lovelace-desktop/src/completion.rs                 |  1366 +-
 lovelace-desktop/src/diagnostic_map.rs             |   902 +-
 lovelace-desktop/src/document_io.rs                |  1844 +-
 lovelace-desktop/src/engine.rs                     |  1900 +-
 lovelace-desktop/src/engine/fake.rs                |  1686 +-
 lovelace-desktop/src/engine/fault.rs               |   238 +-
 lovelace-desktop/src/engine/inspection.rs          |  1966 +-
 lovelace-desktop/src/engine/real.rs                |   598 +-
 lovelace-desktop/src/execution.rs                  |  6870 ++--
 lovelace-desktop/src/find.rs                       |  1152 +-
 lovelace-desktop/src/frame_probe.rs                |   326 +-
 lovelace-desktop/src/glyph_audit.rs                |   246 +-
 lovelace-desktop/src/glyphs.rs                     |   922 +-
 lovelace-desktop/src/gui_selftest.rs               |  3544 +-
 lovelace-desktop/src/history.rs                    |   698 +-
 lovelace-desktop/src/ime_probe.rs                  |  1512 +-
 lovelace-desktop/src/incremental.rs                |  4084 +-
 lovelace-desktop/src/inspector.rs                  |  2494 +-
 lovelace-desktop/src/journey.rs                    |  3688 +-
 lovelace-desktop/src/keyboard.rs                   |  2664 +-
 lovelace-desktop/src/language.rs                   |   152 +-
 lovelace-desktop/src/log_kind.rs                   |   440 +-
 lovelace-desktop/src/main.rs                       |  1076 +-
 lovelace-desktop/src/netcheck.rs                   |   616 +-
 lovelace-desktop/src/perf_probe.rs                 |   998 +-
 lovelace-desktop/src/platform_claims.rs            |   342 +-
 lovelace-desktop/src/platform_dialogs.rs           |   494 +-
 lovelace-desktop/src/platform_parity.rs            |   756 +-
 lovelace-desktop/src/precision_budget.rs           |  1060 +-
 lovelace-desktop/src/precision_default.rs          |   423 +-
 lovelace-desktop/src/quick_eval.rs                 |   446 +-
 lovelace-desktop/src/recovery.rs                   |  2074 +-
 lovelace-desktop/src/regex_lite.rs                 |  1600 +-
 lovelace-desktop/src/results_tree.rs               |  3290 +-
 lovelace-desktop/src/run_state.rs                  |  1184 +-
 lovelace-desktop/src/run_trace.rs                  |   366 +-
 lovelace-desktop/src/selftest.rs                   |  1256 +-
 lovelace-desktop/src/settings.rs                   |  4076 +-
 lovelace-desktop/src/statements.rs                 |   660 +-
 lovelace-desktop/src/svg.rs                        |  1328 +-
 lovelace-desktop/src/symbolic_status.rs            |   904 +-
 lovelace-desktop/src/thread_audit.rs               |   806 +-
 lovelace-desktop/src/ui/a11y.rs                    |  1570 +-
 lovelace-desktop/src/ui/app.rs                     | 40304 +++++++++----------
 lovelace-desktop/src/ui/control.rs                 |  1650 +-
 lovelace-desktop/src/ui/editor.rs                  |  8440 ++--
 lovelace-desktop/src/ui/empty.rs                   |  1034 +-
 lovelace-desktop/src/ui/file_dialogs.rs            |   244 +-
 lovelace-desktop/src/ui/measure.rs                 |  1526 +-
 lovelace-desktop/src/ui/measure_run.rs             |   794 +-
 lovelace-desktop/src/ui/mod.rs                     |   242 +-
 lovelace-desktop/src/ui/plot.rs                    |  5090 +--
 lovelace-desktop/src/ui/semantic.rs                |   620 +-
 lovelace-desktop/src/ui/syntax.rs                  |  2034 +-
 lovelace-desktop/src/ui/theme.rs                   |  3640 +-
 lovelace-desktop/src/ui/type_scale.rs              |  1414 +-
 lovelace-desktop/src/ui/typography.rs              |  1314 +-
 lovelace-desktop/src/ui_audit.rs                   |  2008 +-
 lovelace-desktop/src/win32/ffi.rs                  |  3262 +-
 lovelace-desktop/src/win32/mod.rs                  |    16 +-
 lovelace-desktop/src/win32/shell.rs                |  2030 +-
 lovelace-desktop/src/workbench.rs                  | 18290 ++++-----
 lovelace-desktop/src/workspace_table.rs            |  2344 +-
 lovelace-desktop/tests/linux_cli.rs                |   226 +-
 lovelace-int/Cargo.toml                            |    24 +-
 lovelace-int/src/lib.rs                            |  2534 +-
 lovelace-int/src/proof_small_domain.rs             |   622 +-
 lovelace-int/src/round95_singletons.rs             |    94 +-
 lovelace-json/Cargo.toml                           |    18 +-
 lovelace-json/src/lib.rs                           |  1026 +-
 lovelace-know/Cargo.toml                           |    46 +-
 lovelace-know/src/cli.rs                           |   868 +-
 lovelace-know/src/converge.rs                      |   164 +-
 lovelace-know/src/convergence.rs                   |   476 +-
 lovelace-know/src/exact.rs                         |   648 +-
 lovelace-know/src/graph.rs                         |    64 +-
 lovelace-know/src/json.rs                          |   480 +-
 lovelace-know/src/lib.rs                           |    72 +-
 lovelace-know/src/main.rs                          |   222 +-
 lovelace-know/src/model.rs                         |  2324 +-
 lovelace-know/src/observation.rs                   |   268 +-
 lovelace-know/src/proposal.rs                      |   848 +-
 lovelace-know/src/query.rs                         |   166 +-
 lovelace-know/src/random.rs                        |    78 +-
 lovelace-know/src/reducer.rs                       |   848 +-
 lovelace-know/src/runner.rs                        |   532 +-
 lovelace-know/tests/cli_stdio_tests.rs             |   574 +-
 lovelace-know/tests/common/mod.rs                  |   300 +-
 lovelace-know/tests/convergence_tests.rs           |   654 +-
 lovelace-know/tests/exact_tests.rs                 |   178 +-
 lovelace-know/tests/graph_tests.rs                 |    98 +-
 lovelace-know/tests/json_format_tests.rs           |   154 +-
 lovelace-know/tests/observation_tests.rs           |   196 +-
 lovelace-know/tests/proposal_tests.rs              |   470 +-
 lovelace-know/tests/reducer_tests.rs               |   564 +-
 lovelace-know/tests/seed_reproduction.rs           |   490 +-
 lovelace-lang/Cargo.toml                           |    34 +-
 lovelace-lang/examples/bare_engine.rs              |   158 +-
 lovelace-lang/src/argument_contract.rs             |   196 +-
 lovelace-lang/src/arity_taxonomy_round73.rs        |   902 +-
 lovelace-lang/src/assumptions_round35.rs           |   896 +-
 lovelace-lang/src/ast.rs                           |   388 +-
 lovelace-lang/src/builtins.rs                      | 24737 ++++++------
 lovelace-lang/src/cancellable.rs                   |   346 +-
 lovelace-lang/src/cancellation_round18.rs          |   568 +-
 lovelace-lang/src/circular_family_round31.rs       |  1060 +-
 lovelace-lang/src/complex.rs                       |  1826 +-
 lovelace-lang/src/complex_math.rs                  |   306 +-
 lovelace-lang/src/complex_round24.rs               |   892 +-
 lovelace-lang/src/context.rs                       |   872 +-
 lovelace-lang/src/context_round56.rs               |   886 +-
 .../src/corpus_and_system_solve_round36.rs         |   382 +-
 lovelace-lang/src/diagnostic.rs                    |   172 +-
 lovelace-lang/src/dispatcher_guard_round51.rs      |   296 +-
 lovelace-lang/src/display_format.rs                |   112 +-
 lovelace-lang/src/docs_sync.rs                     |   294 +-
 lovelace-lang/src/dsp_family_round61.rs            |   302 +-
 lovelace-lang/src/dsp_round20.rs                   |   859 +-
 lovelace-lang/src/dsp_round21.rs                   |  1396 +-
 lovelace-lang/src/eigen_jacobi_round.rs            |   528 +-
 lovelace-lang/src/evaluation_boundary.rs           |  1402 +-
 .../src/falsification_round46/boundary.rs          |   406 +-
 lovelace-lang/src/falsification_round46/gate.rs    |  1414 +-
 lovelace-lang/src/falsification_round46/looping.rs |   612 +-
 lovelace-lang/src/falsification_round46/mod.rs     |    40 +-
 .../src/falsification_round46/observation.rs       |  1066 +-
 lovelace-lang/src/falsification_round46/probe.rs   |   624 +-
 .../src/falsification_round46/probeset.rs          |   854 +-
 .../src/falsification_round46/tests/gate_api.rs    |   680 +-
 .../src/falsification_round46/tests/loop_api.rs    |  1566 +-
 .../src/falsification_round46/tests/mod.rs         |   168 +-
 .../src/falsification_round46/tests/probe_api.rs   |   204 +-
 lovelace-lang/src/fixed_dsp.rs                     |   776 +-
 lovelace-lang/src/fixed_dsp_tests.rs               |   694 +-
 lovelace-lang/src/format.rs                        |   184 +-
 lovelace-lang/src/help.rs                          |   698 +-
 lovelace-lang/src/interp.rs                        |  1330 +-
 lovelace-lang/src/inverse_branch_round33.rs        |   766 +-
 lovelace-lang/src/kernel_cancel_round9.rs          |   190 +-
 lovelace-lang/src/latex_and_sqrt_parity.rs         |   540 +-
 lovelace-lang/src/latex_printer_parity.rs          |  1818 +-
 lovelace-lang/src/lexer.rs                         |   960 +-
 lovelace-lang/src/lib.rs                           |  6030 +--
 lovelace-lang/src/mathir.rs                        |  3588 +-
 lovelace-lang/src/mathir_round54.rs                |   886 +-
 lovelace-lang/src/mathir_typing_round55.rs         |   528 +-
 lovelace-lang/src/modus.rs                         |  2184 +-
 lovelace-lang/src/name_suggestions.rs              |    58 +-
 lovelace-lang/src/numeric.rs                       |  2066 +-
 lovelace-lang/src/panic_sweep_round22.rs           |   532 +-
 lovelace-lang/src/parser.rs                        |  1056 +-
 lovelace-lang/src/plot_file.rs                     |   388 +-
 lovelace-lang/src/plot_structure_round87.rs        |   434 +-
 lovelace-lang/src/plot_surface.rs                  |  1722 +-
 lovelace-lang/src/print_budget.rs                  |   346 +-
 lovelace-lang/src/printing_round16.rs              |  1138 +-
 lovelace-lang/src/printing_round17.rs              |  1416 +-
 lovelace-lang/src/progress.rs                      |   458 +-
 lovelace-lang/src/real_constant_round35.rs         |   224 +-
 lovelace-lang/src/real_literal_route.rs            |   122 +-
 lovelace-lang/src/registry.rs                      |  3896 +-
 lovelace-lang/src/relations_families_round33.rs    |   304 +-
 lovelace-lang/src/robustness_round13.rs            |   548 +-
 lovelace-lang/src/round19_batch.rs                 |  1832 +-
 lovelace-lang/src/round26_complex_math_parity.rs   |  1324 +-
 lovelace-lang/src/round26_complex_value_parity.rs  |   128 +-
 lovelace-lang/src/round26_evalf_parity.rs          |   564 +-
 lovelace-lang/src/round27_batch.rs                 |  1298 +-
 lovelace-lang/src/round28_batch.rs                 |   794 +-
 lovelace-lang/src/round30_batch.rs                 |    72 +-
 lovelace-lang/src/round35_extra_tail.rs            |   604 +-
 lovelace-lang/src/round36_remaining.rs             |   624 +-
 lovelace-lang/src/round38_remaining.rs             |   748 +-
 lovelace-lang/src/round38_series_kink_hardening.rs |  1044 +-
 lovelace-lang/src/round39_residue.rs               |   366 +-
 lovelace-lang/src/round42_differential_classes.rs  |   350 +-
 lovelace-lang/src/round43_display_classes.rs       |   362 +-
 lovelace-lang/src/round44_parity_classes.rs        |   244 +-
 lovelace-lang/src/round46_token_kind.rs            |   948 +-
 lovelace-lang/src/round66_engine_data.rs           |  1072 +-
 lovelace-lang/src/round66_resweep.rs               |    94 +-
 lovelace-lang/src/round67_modus.rs                 |  1352 +-
 lovelace-lang/src/round69_parameter_domain.rs      |   212 +-
 lovelace-lang/src/round76_capture_events.rs        |   402 +-
 lovelace-lang/src/round78_scoped_eval.rs           |   292 +-
 lovelace-lang/src/round78_structural_equality.rs   |   658 +-
 lovelace-lang/src/round80_abs_rewrite.rs           |   302 +-
 lovelace-lang/src/round80_throwing_evaluator.rs    |   590 +-
 lovelace-lang/src/round82_evalf_precision.rs       |   420 +-
 lovelace-lang/src/round82_series_laurent.rs        |   268 +-
 lovelace-lang/src/round84_evalir_multi.rs          |   624 +-
 lovelace-lang/src/round90_runner_families.rs       |   586 +-
 lovelace-lang/src/round90_symbolic_families.rs     |   924 +-
 lovelace-lang/src/round92_runner_families.rs       |   916 +-
 lovelace-lang/src/round92_symbolic_families.rs     |   526 +-
 lovelace-lang/src/round93_eval_print.rs            |  1074 +-
 lovelace-lang/src/round95_engine_singletons.rs     |  1000 +-
 lovelace-lang/src/round95_singletons.rs            |    76 +-
 lovelace-lang/src/round98_differential_oracle.rs   |  2142 +-
 lovelace-lang/src/signals.rs                       |   392 +-
 lovelace-lang/src/structural_equality.rs           |   408 +-
 lovelace-lang/src/structured.rs                    |   638 +-
 lovelace-lang/src/suite_engine_batch.rs            |  1462 +-
 lovelace-lang/src/suite_lex_parse_batch.rs         |  1268 +-
 lovelace-lang/src/suite_parity_batch.rs            | 12896 +++---
 lovelace-lang/src/suite_surface_round14.rs         |  1206 +-
 lovelace-lang/src/suite_tail_round35.rs            |  1880 +-
 lovelace-lang/src/symbolics_round23.rs             |   284 +-
 lovelace-lang/src/symbolics_tail_round34.rs        |  2318 +-
 lovelace-lang/src/syntax.rs                        |  2140 +-
 lovelace-lang/src/token.rs                         |   584 +-
 lovelace-lang/src/transcendental_round25.rs        |   870 +-
 lovelace-lang/src/unicode.rs                       |   188 +-
 lovelace-lang/src/value.rs                         |  1384 +-
 lovelace-lang/src/value_widen.rs                   |   774 +-
 lovelace-nat/.newton-tests-backup.rs               |   596 +-
 lovelace-nat/.tests-backup.rs                      |   614 +-
 lovelace-nat/Cargo.toml                            |    18 +-
 lovelace-nat/examples/bz_probe.rs                  |    84 +-
 lovelace-nat/examples/verify_mul.rs                |    80 +-
 lovelace-nat/src/bz.rs                             |   724 +-
 lovelace-nat/src/digit_store.rs                    |  3244 +-
 lovelace-nat/src/lib.rs                            |  4418 +-
 lovelace-nat/src/newton.rs                         |   596 +-
 lovelace-nat/src/ntt.rs                            |  1464 +-
 lovelace-nat/src/proof_small_domain.rs             |   866 +-
 lovelace-nat/src/round39_natural_api.rs            |  1380 +-
 lovelace-real/Cargo.toml                           |    26 +-
 lovelace-real/src/fast_path.rs                     |   898 +-
 lovelace-real/src/fast_path_tests.rs               |  1244 +-
 lovelace-real/src/lib.rs                           | 15582 +++----
 lovelace-real/src/lreal.rs                         |  1598 +-
 lovelace-real/src/lreal_tests.rs                   |   692 +-
 lovelace-real/src/real_parity_round10.rs           |   884 +-
 lovelace-real/src/real_parity_round7.rs            |  9362 ++---
 lovelace-real/src/round26_trig_parity.rs           |   962 +-
 lovelace-real/src/round29_real_batch.rs            |   354 +-
 lovelace-real/src/round30_real_batch.rs            |   130 +-
 lovelace-real/src/round31_trig.rs                  |   656 +-
 lovelace-real/src/round34_inexact_division.rs      |   612 +-
 lovelace-real/src/round36_ambient.rs               |    78 +-
 lovelace-real/src/round36_precbench.rs             |   396 +-
 lovelace-real/src/round39_api.rs                   |   772 +-
 lovelace-real/src/round39_real_tests.rs            |  1080 +-
 lovelace-real/src/round66_resweep.rs               |    62 +-
 lovelace-real/src/round95_singletons.rs            |    90 +-
 lovelace-real/src/transcendental.rs                |  1988 +-
 lovelace-real/src/transcendental_round25.rs        |   452 +-
 lovelace-run/Cargo.toml                            |    44 +-
 lovelace-run/src/cancellation_batch.rs             |   740 +-
 lovelace-run/src/dsp_round21.rs                    |   856 +-
 lovelace-run/src/dx_batch.rs                       |  1082 +-
 lovelace-run/src/enum_batch.rs                     |   256 +-
 lovelace-run/src/error_taxonomy_batch.rs           |   792 +-
 lovelace-run/src/golden_batch.rs                   |   606 +-
 lovelace-run/src/main.rs                           |  2528 +-
 lovelace-run/src/parity_batch.rs                   |  1720 +-
 lovelace-run/src/plot_surface.rs                   |   354 +-
 lovelace-run/src/plot_surface_wire.rs              |   516 +-
 lovelace-run/src/pretty_batch.rs                   |   640 +-
 lovelace-run/src/record_schema_batch.rs            |  1102 +-
 lovelace-run/src/repl.rs                           |  2012 +-
 lovelace-run/src/round19_depth_batch.rs            |   928 +-
 lovelace-run/src/round19_run_batch.rs              |  2978 +-
 lovelace-run/src/round27_run_batch.rs              |   830 +-
 lovelace-run/src/round28_run_batch.rs              |  1654 +-
 lovelace-run/src/round29_run_batch.rs              |   742 +-
 lovelace-run/src/round30_run_batch.rs              |  1004 +-
 lovelace-run/src/round35_run_batch.rs              |   588 +-
 lovelace-run/src/round45_final_sweep.rs            |   840 +-
 lovelace-run/src/round50_resweep.rs                |   392 +-
 lovelace-run/src/round66_resweep.rs                |   130 +-
 lovelace-run/src/round75_probe.rs                  |   284 +-
 lovelace-run/src/round82_evalf_precision.rs        |   464 +-
 lovelace-run/src/round90_runner_families.rs        |   222 +-
 lovelace-run/src/round95_singletons.rs             |   454 +-
 lovelace-run/src/schemas.rs                        |   778 +-
 lovelace-run/src/script_source.rs                  |  1158 +-
 lovelace-run/src/suite_batch.rs                    |   406 +-
 lovelace-run/src/wire_batch.rs                     |  2578 +-
 lovelace-run/tests/recursion_depth.rs              |    88 +-
 lovelace-run/tests/repl.rs                         |   572 +-
 lovelace-symbolics/Cargo.toml                      |    26 +-
 lovelace-symbolics/src/assumptions.rs              |  3400 +-
 lovelace-symbolics/src/cancel.rs                   |   492 +-
 .../src/complex_closed_forms_round30.rs            |   542 +-
 lovelace-symbolics/src/groebner.rs                 |  1372 +-
 .../src/indeterminate_forms_round31.rs             |   318 +-
 lovelace-symbolics/src/lib.rs                      | 12318 +++---
 lovelace-symbolics/src/lib.rs.bak35                |  8724 ++--
 lovelace-symbolics/src/panic_sweep_round23.rs      |   120 +-
 lovelace-symbolics/src/parameter_domain.rs         |   272 +-
 lovelace-symbolics/src/polynomials.rs              |   754 +-
 lovelace-symbolics/src/printing_round16.rs         |   970 +-
 lovelace-symbolics/src/proof_bridge.rs             |   188 +-
 lovelace-symbolics/src/rational_functions.rs       |  1992 +-
 lovelace-symbolics/src/real_literal.rs             |   184 +-
 lovelace-symbolics/src/rewrite_registry.rs         |  3454 +-
 lovelace-symbolics/src/round36_exactness.rs        |    34 +-
 lovelace-symbolics/src/round39_residue.rs          |   550 +-
 lovelace-symbolics/src/round66_resweep.rs          |   508 +-
 .../src/round67_polynomial_algebra.rs              |   844 +-
 lovelace-symbolics/src/round69_parameter_domain.rs |   230 +-
 lovelace-symbolics/src/round75_probe.rs            |   126 +-
 lovelace-symbolics/src/round76_matrix.rs           |   620 +-
 .../src/round90_symbolic_families.rs               |   492 +-
 .../src/round92_symbolic_families.rs               |   346 +-
 lovelace-symbolics/src/series_kink.rs              |  1080 +-
 lovelace-symbolics/src/solution_set.rs             |   494 +-
 lovelace-symbolics/src/symbolic_matrix.rs          |  1850 +-
 lovelace-symbolics/src/tail_round34.rs             |    90 +-
 parity/corpus/lang.json                            |  1790 +-
 parity/corpus/symbolics.json                       |  1662 +-
 tools/next-version.ps1                             |   674 +-
 tools/release-notes.ps1                            |   560 +-
 466 files changed, 296668 insertions(+), 296130 deletions(-)
```

## Appendix B - design note on the archived tree

`docs/goal-desktop-v4/journeys.md` contains links to capture rounds that exist only in the
LovelaceSharp repository. The link pass skips that whole tree by policy, so those links
cannot fail the check. The `deviations.md` findings above come from current pages (ADRs)
citing a file the archived tree does not carry; they are not the deliberate archived
capture links.

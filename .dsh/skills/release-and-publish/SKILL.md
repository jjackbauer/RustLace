---
name: release-and-publish
description: Build, test, optionally sign with the documented self-signed caveat, tag and publish the Windows and Linux artifacts, and verify the CI run that backs them.
whenToUse: Use when cutting a RustLace release, bumping the workspace version, publishing the desktop executables, or checking what the CI workflow must prove before a tag is announced.
---

# Release and publish

A release carries **two** executables — Windows x86_64 and Linux x86_64 (X11) — each with its own
platform-qualified checksum file, built from the tagged release commit. There is no installer, no
MSIX, no side-by-side DLL set and no packaging script. The version lives in one place
(`[workspace.package] version` in `Cargo.toml`), the release workflow builds each file at the tag and
checks its `--version`, and the honest release note is an unsigned build with a published hash unless
someone signs it.

| Asset | Platform | Built at the tag by | Checksum file |
|---|---|---|---|
| `lovelace-desktop.exe` | Windows x86_64 | the `release` job (`windows-latest`) | `SHA256SUMS.windows` |
| `lovelace-desktop-linux-x86_64` | Linux x86_64 (X11, glibc >= 2.39) | the `linux` job (`ubuntu-24.04`) | `SHA256SUMS.linux` |

The Linux executable is **not** optional and is **not** a placeholder: CI builds it, runs the
20-check `--gui-selftest` and the desktop suite under `xvfb-run`, and uploads it; the release
workflow then rebuilds it at the tag, checks its `--version` and publishes it. Do not omit it, and do
not drop the Linux job because a local Windows host cannot run it — the Linux job is the gate that
compiles and exercises the GUI there. `lovelace-desktop-linux-x86_64` is the released name; the build
output `target/release/lovelace-desktop` is not the asset.

**The normal path is automatic.** `.github/workflows/release.yml` runs after a green CI run on
`main`, derives the version from conventional commits with `tools/next-version.ps1`, tags it, builds
both executables at that tag, verifies each `--version`, and publishes all four assets:
`lovelace-desktop.exe`, `SHA256SUMS.windows`, `lovelace-desktop-linux-x86_64` and
`SHA256SUMS.linux`. The `release` job creates the tag and the GitHub Release and attaches the Windows
pair; the `linux` job checks the same tag out, attaches the Linux pair and never creates the release.
When the tag already exists without a release the same workflow recovers by building from the tag;
when both exist it stops without creating anything. The manual procedure below is the fallback for
when that workflow cannot run; do not hand-tag while the workflow is available.

## What CI must prove before a release

`.github/workflows/ci.yml` defines three jobs, and a release needs **all three** green on the CI run
for the validated commit. Checking "both jobs" is not enough: the documentation job is mandatory, and
it runs as two OS variants of one definition.

| Job | Runner | Gate it must pass |
|---|---|---|
| `Windows (full workspace)` | `windows-latest` | `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets` (without `-D warnings`); `cargo test --workspace`; `cargo build --release -p lovelace-desktop`; upload `lovelace-desktop-windows-x86_64` with `if-no-files-found: error` |
| `Linux (engine crates + GUI desktop under Xvfb)` | `ubuntu-24.04`, pinned deliberately rather than `ubuntu-latest` | the format check; the engine clippy selection (`lovelace-lang`, `lovelace-real`, `lovelace-nat`, `lovelace-symbolics`); the engine tests (`lovelace-lang`, `lovelace-real`, `lovelace-symbolics`); `cargo build --release -p lovelace-desktop`; the glibc guard (the artifact's highest `GLIBC_2.x` symbol must not exceed the runner's `ldd` version); `xvfb-run … --gui-selftest` (exit 0 and `checks=20 failed=0`); `xvfb-run … cargo test -p lovelace-desktop`; upload `lovelace-desktop-linux-gui-x86_64` with `if-no-files-found: error` |
| `Documentation examples (${{ matrix.os }})` | `ubuntu-latest` **and** `windows-latest` — two variants of one job definition | twelve steps, one definition: checkout pinning `core.autocrlf=false`, Rust toolchain, cargo cache, `cargo build -p lovelace-run`, `bash tools/check-crate-examples.sh`, `bash tools/run-tutorials.sh`, `bash docs/language/examples/run-examples.sh` (fenced examples plus the builtin-catalogue check against `--functions`) and `bash docs/extensions/examples/run-examples.sh`, `bash tools/check-docs.sh --symbols`, `--claims`, `--readme`, `bash tools/check-reference-pages.sh`, and `bash tools/test-docs-gates.sh` |

Read the job definitions in `.github/workflows/ci.yml` at the release commit rather than trusting
this table: if a job gains or loses a step or a `check-docs.sh` mode, this page changes in the same
commit. A documentation job failure blocks a release exactly like a build failure — the examples are
executable documentation, and running both variants is what catches a path shape, `.exe` lookup or
line-ending defect on the platform that has it. One passing variant is not evidence for the other.

## Files you will touch

- `Cargo.toml` - the single workspace version.
- `Cargo.lock` - updated in the same commit as the version bump.
- `.github/workflows/ci.yml` - the Windows, Linux and documentation jobs, their runners and artifact
  names.
- `.github/workflows/release.yml` - the two release jobs, the guard branches and the four assets.
- `docs/release.md` - the release checklist and signing caveat.
- `docs/images/README.md` - only when a shipped screenshot changes.
- `lovelace-desktop/src/main.rs` - the `--version` output comes from the package metadata.

## Procedure

1. **Preflight the tree.** Be on a branch with a clean working tree (`git status --porcelain` is
   empty), confirm the version in `Cargo.toml`, and confirm `cargo run -p lovelace-desktop --
   --version` prints the same version. A tag is never created on a dirty tree or on an unmerged
   feature branch.

   Also confirm the licence file and the package declaration agree. Run the platform's command:

   ```console
   # Windows (PowerShell)
   Select-String -Path Cargo.toml -Pattern '^license'
   ```

   ```console
   # Linux (bash)
   grep -n '^license' Cargo.toml
   ```

   It must print `license = "MIT"` from `[workspace.package]`; the root `LICENSE` must exist and
   carry the MIT permission text; and every member `Cargo.toml` must either inherit it
   (`license.workspace = true`) or declare the same `MIT`. A missing `LICENSE`, a different
   identifier, or a member overriding it with something else stops the release.

2. **Run the release gate on the release commit.** The blocks below are not interchangeable: the
   Windows block does not run on Linux, and the Linux GUI checks need Xvfb. Run the shared block
   first, then the block for each host you have.

   ```console
   # every host
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets
   cargo test --workspace
   cargo run --release -p lovelace-corpus
   cargo build --release -p lovelace-desktop
   ```

   ```console
   # Windows host (PowerShell; the GUI modes need a writable APPDATA)
   $env:APPDATA = Join-Path $env:TEMP 'rustlace-release'
   cargo test -p lovelace-desktop
   cargo run -p lovelace-desktop -- --gui-selftest
   cargo run -p lovelace-desktop -- --contrast-check
   cargo run -p lovelace-desktop -- --journey J-02 J-03 J-07 J-08 J-09 J-10 J-15 --out .tmp\release-journeys
   ```

   ```console
   # Linux host (bash; X11 package set installed, no display, so use Xvfb)
   unset WAYLAND_DISPLAY
   export XDG_CONFIG_HOME="$(mktemp -d)"
   xvfb-run -a -s "-screen 0 1600x1000x24" cargo test -p lovelace-desktop
   xvfb-run -a -s "-screen 0 1600x1000x24" cargo run -p lovelace-desktop -- --gui-selftest
   xvfb-run -a -s "-screen 0 1600x1000x24" cargo run -p lovelace-desktop -- --contrast-check
   xvfb-run -a -s "-screen 0 1600x1000x24" cargo run -p lovelace-desktop -- --journey J-02 J-03 J-07 J-08 J-09 J-10 J-15 --out .tmp/release-journeys
   ```

   Then run the documentation gate the same way the `Documentation examples` job does — the crate-page
   examples, the tutorials, both example runners, every `tools/check-docs.sh` mode the job invokes,
   `tools/check-reference-pages.sh`, and the gate self-test:

   ```console
   bash tools/check-crate-examples.sh
   bash tools/run-tutorials.sh
   bash docs/language/examples/run-examples.sh
   bash docs/extensions/examples/run-examples.sh
   bash tools/check-docs.sh --symbols
   bash tools/check-docs.sh --claims
   bash tools/check-docs.sh --readme
   bash tools/check-reference-pages.sh
   bash tools/test-docs-gates.sh
   ```

   **Read the counts from the run; never adopt a number from an earlier release as this release's
   acceptance threshold.** The counts in `docs/testing.md` are observations of a named commit and
   host, and a historical test count is not immutable. Record what this commit reports, compare it
   with that page, and explain a moved count in the release note rather than relaxing a check. A run
   that reports `running 0 tests` or `0 passed` is a failed gate even though the process exits 0:
   every filtered run must report a nonzero passed count with `0 failed`. The
   `--gui-selftest` line must read `GUI_SELFTEST summary checks=20 failed=0` on both platforms.

3. **Record each artifact's identity.** The files are the release binaries under the target
   directory (produced by the release build in step 2). Record size and SHA-256 for both, and put
   both pairs in the release note. Use the command for the host:

   ```console
   # Windows (PowerShell)
   Get-Item target\release\lovelace-desktop.exe | Select-Object Length
   Get-FileHash target\release\lovelace-desktop.exe -Algorithm SHA256
   ```

   ```console
   # Linux (bash)
   stat -c '%s %n' target/release/lovelace-desktop
   sha256sum target/release/lovelace-desktop
   ```

   The size is a build observation, not a contract; the hash identifies the exact file users run.
   Do not paste the Windows lines into a bash shell: `Get-Item` and `Get-FileHash` are PowerShell
   cmdlets, not Linux commands. To verify a Windows checksum on Linux without running the executable,
   use `sha256sum -c SHA256SUMS.windows` against the downloaded pair.

4. **Decide about signing, and say the truth.** Signing applies to the Windows executable only;
   `signtool` does not exist on Linux and the Linux asset is published unsigned. The tree contains no
   signing step: a search for `signtool`, Authenticode, certificate or self-signed in the sources,
   manifests and workflow finds nothing, so the default artifact is unsigned and Windows shows
   "Unknown publisher" in SmartScreen. If you sign without a certificate authority, the only option
   is a self-signed certificate, and the caveats are part of the release note:

   - it does not establish trust; Windows still warns unless the certificate is installed in the
     machine's Trusted Root store, which needs an administrator and is manual per machine;
   - SmartScreen reputation is per signing identity and per hash, so a self-signed certificate does
     not accumulate publisher reputation;
   - anyone can create a certificate with the same subject name; a subject line is not identity;
   - signing changes the hash, so publish the hash of the signed file, not the build input.

   If `signtool` exists, sign with `/fd SHA256` and a `.pfx`, then verify the signature status:

   ```console
   signtool sign /fd SHA256 /f release.pfx /p <password> /tr http://timestamp.digicert.com /td SHA256 target\release\lovelace-desktop.exe
   Get-AuthenticodeSignature target\release\lovelace-desktop.exe | Select-Object Status, SignerCertificate
   ```

   Never commit the `.pfx`, its password or a private key. If the machine has no certificate and no
   administrator to install one, publish the unsigned build and say so; a self-signed file that users
   still see as "Unknown publisher" is not more trustworthy than an honest unsigned one.

5. **Tag the release commit.** Use the version `tools/next-version.ps1` printed, with a `v` prefix;
   `<version>` is a placeholder for that value, not a fixed number. An annotated tag carries the
   release note's first line:

   ```console
   git tag -a v<version> -m "RustLace <version>"
   git push origin v<version>
   ```

   For example, if the workflow printed `0.2.0`, tag `v0.2.0`; do not copy a version from an earlier
   release. Verify with `git ls-remote --tags origin` that the tag exists on the remote and points at
   the release commit, not at an earlier one.

6. **Publish both artifacts.** The normal path is the four assets the release workflow built at the
   tag. For a manual release, check out the tag, build with `cargo build --release -p
   lovelace-desktop`, and handle **both** executables and **both** platform-qualified checksums. A
   release that carries only the Windows pair is incomplete, and a `SHA256SUMS` without a platform
   qualifier cannot say which executable it describes.

   ```console
   # Windows host (PowerShell): the Windows executable and SHA256SUMS.windows
   git checkout --detach v<version>
   cargo build --release -p lovelace-desktop
   .\target\release\lovelace-desktop.exe --version
   $hash = (Get-FileHash .\target\release\lovelace-desktop.exe -Algorithm SHA256).Hash.ToLowerInvariant()
   "$hash  lovelace-desktop.exe" | Set-Content -Path .\SHA256SUMS.windows -Encoding ascii
   gh release create v<version> .\target\release\lovelace-desktop.exe .\SHA256SUMS.windows --title "RustLace v<version>" --notes-file .\release-notes.md --verify-tag
   ```

   ```console
   # Linux host (bash): the renamed Linux executable and SHA256SUMS.linux
   git checkout --detach v<version>
   cargo build --release -p lovelace-desktop
   ./target/release/lovelace-desktop --version
   cp target/release/lovelace-desktop lovelace-desktop-linux-x86_64
   chmod +x lovelace-desktop-linux-x86_64
   sha256sum lovelace-desktop-linux-x86_64 > SHA256SUMS.linux
   sha256sum -c SHA256SUMS.linux
   gh release upload v<version> lovelace-desktop-linux-x86_64 SHA256SUMS.linux --clobber
   ```

   When the release does not exist yet, create it once with `gh release create` (or in the GitHub UI)
   and attach all four files; when it exists, `gh release upload --clobber` replaces only the files it
   names. The workflow's `linux` job never creates the release — it uploads to the release the
   `release` job created — so a Linux-only manual path must attach the Windows pair too. If the CLI is
   not installed, the GitHub UI attaches the same four files. A build that did not come from the
   tagged commit is not the release path: an asset whose `--version` disagrees with the tag is wrong
   even when the tag is right.

7. **Check CI before announcing.** Open the run for the validated commit and confirm all three jobs
   are green, including **both** variants of the documentation job:
   - "Windows (full workspace)" ran `cargo fmt --all -- --check`, `cargo clippy --workspace
     --all-targets`, `cargo test --workspace`, `cargo build --release -p lovelace-desktop`, and
     uploaded `lovelace-desktop-windows-x86_64` with `if-no-files-found: error`;
   - "Linux (engine crates + GUI desktop under Xvfb)" ran the format check, the engine clippy
     selection, the engine tests (`lovelace-lang`, `lovelace-real`, `lovelace-symbolics`),
     `cargo build --release -p lovelace-desktop`, the glibc guard, `xvfb-run … --gui-selftest` (exit
     0 with `checks=20 failed=0`), and `xvfb-run … cargo test -p lovelace-desktop`, then uploaded
     `lovelace-desktop-linux-gui-x86_64`;
   - "Documentation examples (ubuntu-latest)" and "Documentation examples (windows-latest)" both ran
     `cargo build -p lovelace-run`, the language and extension example runners, every
     `tools/check-docs.sh` mode the job invokes, and `tools/check-reference-pages.sh`. A red docs
     variant blocks the release; a green Ubuntu variant does not vouch for the Windows one.

   Both platforms are supported and both are released: do not omit, skip or weaken the Linux job, and
   do not treat "it only needs to build on Windows" as true — the Linux job is what compiles and
   exercises the GUI there. A `--gui-selftest` pass is a run-path check, not a GUI journey pass; the
   journey captures are Windows-only and the release note must not imply otherwise. The release
   commit carries `[skip ci]`, so the CI run that matters is the validated commit before the bump.
   Then read the Release run's log: it must state the guard branch, print both executables'
   `--version`, and show all four assets. If any job is red, do not publish; fix and re-tag only if
   the tag has not been announced.

8. **Write the release note from observations.** State the version, the commit, **both** asset names,
   their sizes and SHA-256 values, the test and corpus counts observed on that commit with the host
   they were measured on, the unsigned/SmartScreen caveat (or the self-signed caveat if you signed),
   and any known residual from `docs/rustlace-notes.md`. "All tests pass" is not evidence; the counts,
   the hashes and the CI run are. A count from an earlier release is not evidence about this one.

## Verify

```console
# every host
cargo fmt --all -- --check
cargo test --workspace
cargo run --release -p lovelace-corpus
cargo build --release -p lovelace-desktop
git ls-remote --tags origin
```

```console
# Windows host (PowerShell)
cargo test -p lovelace-desktop
Get-FileHash target\release\lovelace-desktop.exe -Algorithm SHA256
```

```console
# Linux host (bash)
xvfb-run -a -s "-screen 0 1600x1000x24" cargo test -p lovelace-desktop
sha256sum lovelace-desktop-linux-x86_64
sha256sum -c SHA256SUMS.linux
```

Observable success: every command exits 0; every test and docs run reports its own nonzero passed
count with `0 failed` (`0 tests` is a failure, not a pass); the corpus prints its full
`N passed, 0 failed, N total`; both checksum files verify (`sha256sum -c SHA256SUMS.linux` and, for
the downloaded Windows pair, `sha256sum -c SHA256SUMS.windows`); both release binaries exist and
their SHA-256 values are the ones in the release note; `git ls-remote --tags origin` lists the new
`v<version>` tag at the release commit; the Windows job, the Linux job and **both** variants of the
documentation job are green; and the Release run logs the guard branch, the two built executables
printing the tagged version, and all four published assets.

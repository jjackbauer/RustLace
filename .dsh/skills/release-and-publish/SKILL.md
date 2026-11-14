---
name: release-and-publish
description: Build, test, optionally sign with the documented self-signed caveat, tag and publish the Windows artifact, and verify the CI run that backs it.
whenToUse: Use when cutting a RustLace release, bumping the workspace version, publishing the desktop executable, or checking what the CI workflow must prove before a tag is announced.
---

# Release and publish

The release artifact is one Windows executable, built from the workspace root. There is no
installer, no MSIX, no side-by-side DLL set and no packaging script. The version lives in one place
(`[workspace.package] version` in `Cargo.toml`), CI builds and uploads the same file, and the
honest release note is an unsigned build with a published hash unless someone signs it.

## Files you will touch

- `Cargo.toml` - the single workspace version.
- `Cargo.lock` - updated in the same commit as the version bump.
- `.github/workflows/ci.yml` - the Windows and Linux jobs and the artifact name.
- `docs/release.md` - the release checklist and signing caveat.
- `docs/images/README.md` - only when a shipped screenshot changes.
- `lovelace-desktop/src/main.rs` - the `--version` output comes from the package metadata.

## Procedure

1. **Preflight the tree.** Be on a branch with a clean working tree (`git status --porcelain` is
   empty), confirm the version in `Cargo.toml`, and confirm `cargo run -p lovelace-desktop --
   --version` prints the same version. A tag is never created on a dirty tree or on an unmerged
   feature branch.

2. **Run the release gate.** Every command in `docs/release.md` is expected to pass on the release
   commit, in this order:

   ```console
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets
   cargo test --workspace
   cargo run --release -p lovelace-corpus
   cargo test -p lovelace-desktop
   cargo run -p lovelace-desktop -- --gui-selftest
   cargo run -p lovelace-desktop -- --contrast-check
   cargo run -p lovelace-desktop -- --journey J-02 J-03 J-07 J-08 J-09 J-10 J-15 --out .tmp\release-journeys
   cargo build --release -p lovelace-desktop
   ```

   Use a writable `APPDATA` for the GUI modes. The expected shape is the one in `docs/testing.md`:
   6135 passed and one ignored in the workspace, 716 in the desktop crate, 214/214 in the corpus,
   and `GUI_SELFTEST summary checks=20 failed=0`. A count that moved needs an explanation in the
   release note, not a relaxed gate.

3. **Record the artifact identity.** The file is the release binary under the target directory
   (produced by the release build in step 2). Record its size and SHA-256 and put both in the release
   note:

   ```console
   Get-Item target/release/lovelace-desktop.exe | Select-Object Length
   Get-FileHash target/release/lovelace-desktop.exe -Algorithm SHA256
   ```

   The size is a build observation, not a contract; the hash identifies the exact file users run.

4. **Decide about signing, and say the truth.** The tree contains no signing step: a search for
   `signtool`, Authenticode, certificate or self-signed in the sources, manifests and workflow finds
   nothing, so the default artifact is unsigned and Windows shows "Unknown publisher" in SmartScreen.
   If you sign without a certificate authority, the only option is a self-signed certificate, and
   the caveats are part of the release note:
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

5. **Tag the release commit.** The repository has no tags yet; use the version with a `v` prefix
   (`v0.1.0` for the current `0.1.0`). An annotated tag carries the release note's first line:

   ```console
   git tag -a v0.1.0 -m "RustLace 0.1.0"
   git push origin v0.1.0
   ```

   Verify with `git ls-remote --tags origin` that the tag exists on the remote and points at the
   release commit, not at an earlier one.

6. **Publish the artifact.** The normal path is the CI artifact from the Windows job. Download the
   run's `lovelace-desktop-windows-x86_64` artifact (the GitHub CLI, if installed, does this with
   `gh run download <run-id> -n lovelace-desktop-windows-x86_64`; otherwise use the Actions run
   page), compare its SHA-256 with step 3, and create the release with the CLI:

   ```console
   gh release create v0.1.0 .\lovelace-desktop.exe --title "RustLace 0.1.0" --notes-file .\release-notes.md
   ```

   If the CLI is not installed, create the same release in the GitHub UI and attach the artifact.
   Publishing a local build that CI did not produce from the tagged commit is not the release path.

7. **Check CI before announcing.** Open the run for the release commit and confirm both jobs are
   green:
   - "Windows (full workspace)" ran `cargo fmt --all -- --check`, `cargo clippy --workspace
     --all-targets`, `cargo test --workspace`, `cargo build --release -p lovelace-desktop`, and
     uploaded `lovelace-desktop-windows-x86_64` with `if-no-files-found: error`;
   - "Linux (engine crates)" ran the format check, the engine clippy selection and the engine tests
     (`lovelace-lang`, `lovelace-real`, `lovelace-symbolics`).
   The desktop crate is Windows-only and must not be built or tested on Linux. If either job is red,
   do not publish; fix and re-tag only if the tag has not been announced.

8. **Write the release note from observations.** State the version, the commit, the artifact name,
   size and SHA-256, the test and corpus counts observed on that commit, the unsigned/SmartScreen
   caveat (or the self-signed caveat if you signed), and any known residual from
   `docs/rustlace-notes.md`. "All tests pass" is not evidence; the counts and the hash are.

## Verify

```console
cargo fmt --all -- --check
cargo test --workspace
cargo run --release -p lovelace-corpus
cargo test -p lovelace-desktop
cargo build --release -p lovelace-desktop
Get-FileHash target/release/lovelace-desktop.exe -Algorithm SHA256
git ls-remote --tags origin
```

Observable success: every command exits 0; the corpus prints `214 passed, 0 failed, 214 total`; the
desktop suite reports 716 passed; the release binary exists and its SHA-256 is the hash in the
release note; `git ls-remote --tags origin` lists `v0.1.0` at the release commit; the CI run for that
commit has both jobs green and the uploaded artifact's hash matches the published one.

---
name: release-and-publish
description: Build, test, optionally sign with the documented self-signed caveat, tag and publish the Windows artifact, and verify the CI run that backs it.
whenToUse: Use when cutting a RustLace release, bumping the workspace version, publishing the desktop executable, or checking what the CI workflow must prove before a tag is announced.
---

# Release and publish

The release artifact is one Windows executable, built from the workspace root. There is no
installer, no MSIX, no side-by-side DLL set and no packaging script. The version lives in one place
(`[workspace.package] version` in `Cargo.toml`), the release workflow builds the file at the tagged
release commit and checks its `--version`, and the honest release note is an unsigned build with a
published hash unless someone signs it.

**The normal path is automatic.** `.github/workflows/release.yml` runs after a green CI run on
`main`, derives the version from conventional commits with `tools/next-version.ps1`, tags it, builds
the executable at that tag, verifies `--version`, and publishes the executable plus `SHA256SUMS`.
When the tag already exists without a release the same workflow recovers by building from the tag;
when both exist it stops without creating anything. The manual procedure below is the fallback for
when that workflow cannot run; do not hand-tag while the workflow is available.

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

   Also confirm the licence file and the package declaration agree. From the repository root:

   ```console
   Select-String -Path Cargo.toml -Pattern '^license'
   ```

   It must print `license = "MIT"` from `[workspace.package]`; the root `LICENSE` must exist and
   carry the MIT permission text; and every member `Cargo.toml` must either inherit it
   (`license.workspace = true`) or declare the same `MIT`. A missing `LICENSE`, a different
   identifier, or a member overriding it with something else stops the release.

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

5. **Tag the release commit.** Use the version `tools/next-version.ps1` printed, with a `v` prefix.
   An annotated tag carries the release note's first line:

   ```console
   git tag -a v0.1.0 -m "RustLace 0.1.0"
   git push origin v0.1.0
   ```

   Verify with `git ls-remote --tags origin` that the tag exists on the remote and points at the
   release commit, not at an earlier one.

6. **Publish the artifact.** The normal path is the executable the release workflow built at the
   tag. For a manual release, check out the tag, build with `cargo build --release -p
   lovelace-desktop`, confirm `.\target\release\lovelace-desktop.exe --version` prints the tag's
   version, record its SHA-256, write `SHA256SUMS` in `sha256sum` format
   (`<hash>  lovelace-desktop.exe`), and create the release from that file:

   ```console
   git checkout --detach v0.1.0
   cargo build --release -p lovelace-desktop
   .\target\release\lovelace-desktop.exe --version
   gh release create v0.1.0 .\target\release\lovelace-desktop.exe .\SHA256SUMS --title "RustLace v0.1.0" --notes-file .\release-notes.md --verify-tag
   ```

   If the CLI is not installed, create the same release in the GitHub UI and attach both files.
   A build that did not come from the tagged commit is not the release path: an asset whose
   `--version` disagrees with the tag is wrong even when the tag is right.

7. **Check CI before announcing.** Open the run for the validated commit and confirm both jobs are
   green:
   - "Windows (full workspace)" ran `cargo fmt --all -- --check`, `cargo clippy --workspace
     --all-targets`, `cargo test --workspace`, `cargo build --release -p lovelace-desktop`, and
     uploaded `lovelace-desktop-windows-x86_64` with `if-no-files-found: error`;
   - "Linux (engine crates)" ran the format check, the engine clippy selection and the engine tests
     (`lovelace-lang`, `lovelace-real`, `lovelace-symbolics`).
   The desktop crate is Windows-only and must not be built or tested on Linux. The release commit
   carries `[skip ci]`, so the CI run that matters is the validated commit before the bump. Then
   read the Release run's log: it must state the guard branch, print the built executable's
   `--version`, and show the assets. If either job is red, do not publish; fix and re-tag only if
   the tag has not been announced.

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
release note; `git ls-remote --tags origin` lists the new `v<version>` tag at the release commit;
the CI run for the validated commit has both jobs green; and the Release run logs the guard branch,
the built executable printing the tagged version, and the published asset hashes.

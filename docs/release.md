# Release

## Versioning

One version governs the workspace: `[workspace.package] version` in `Cargo.toml`, and every member
inherits it with `version.workspace = true`. There are no per-crate versions and no published
Rust crates: the release artifacts are a Windows executable and a Linux executable, and each desktop
binary prints the version compiled into it from its package metadata (`--version` ->
`lovelace-desktop <compiled version>`).

The version is derived from the commit history, not chosen by hand. `tools/next-version.ps1` reads
the current workspace version and the commits since the last `vX.Y.Z` tag (every commit when no tag
exists yet) and prints the next version:

```console
$ ./tools/next-version.ps1 -CurrentVersion 0.1.0 -Commits 'feat: publish the RustLace workspace'
previous=0.1.0 next=0.2.0 bump=minor reason="feat: publish the RustLace workspace" policy=0.x
range=supplied -Commits
commits=1
deciding commits:
  synthetic-1 feat: publish the RustLace workspace
```

On the real tree with no tag and only freeform, `docs:`, `test:`, `ci:` and `style:` history it
prints `bump=none`, which is why the first release needed an explicit `feat:` commit.

The `previous=...` line is the machine-readable verdict the workflow consumes; `bump=none` means
the range warrants no release and nothing is tagged. `-Json` emits the same verdict as one object,
`-Commits` supplies a synthetic range so the rules are testable without rewriting history, and
`-FailOnNoBump` turns "no bump" into exit code 2 for scripting.

### The bump rules

| Commit | 0.x (major version 0) | 1.x and later |
|---|---|---|
| `type!:` or a `BREAKING CHANGE:` footer | **minor** | **major** |
| `feat:` (any scope) | **minor** | **minor** |
| `fix:` / `perf:` | **patch** | **patch** |
| `docs:`, `test:`, `ci:`, `chore:`, `style:`, `refactor:`, `build:`, unknown prefixes, freeform | none | none |

A `!` before the colon (`feat!:`, `fix(api)!:`) and a body line beginning `BREAKING CHANGE:` (or
`BREAKING-CHANGE:`) are both a breaking change. The subject of the newest commit that decides the
bump is the verdict's `reason`.

**0.x is deliberate.** While the major version is 0 the project makes no stability promise, so a
breaking change ships as a minor bump: `0.1.0 -> 0.2.0`, and never `1.0.0`. A `feat:` on 0.x is
also a minor bump, and a `fix:`/`perf:` is a patch. The first automated release of this tree is
therefore `v0.2.0` (0.1.0 plus a `feat:` range), not `v1.0.0`.

**Reaching 1.0.0 is an explicit act.** The only ways to move the major version are:

- a commit whose subject contains `release: 1.0.0`, or
- a `BREAKING CHANGE:` footer whose first line carries `major: 1`.

Nothing else moves the major version. Once the major version is 1 or later the promotion token is
ignored: the ordinary semver table applies, and a breaking change raises the major
(`1.2.3 -> 2.0.0`).

## Automatic releases

`.github/workflows/release.yml` runs on the `workflow_run` event for the `CI` workflow. It starts
only when a CI run triggered by a push to `main` finishes with `conclusion == 'success'`, so a red
or unvalidated tree can never be tagged. It has two jobs: `release` (`windows-latest`) creates the
tag and the GitHub Release and attaches the Windows executable; `linux` (`ubuntu-24.04`) runs after
it, checks the same tag out and attaches the Linux executable.

The `release` job:

1. checks out the triggering run's head SHA with full history and tags;
2. runs `tools/next-version.ps1`; when it prints `bump=none` and no tag exists the reason is logged
   and the job stops without a tag;
3. takes one of three guard branches and logs which one:
   - the target tag and its GitHub Release both exist: stop, because this version is already
     published (a re-run or a replayed event cannot publish it twice);
   - the target tag exists but its GitHub Release does not (for example the build failed after the
     tag was pushed): recover by building from that tag and creating the release; the version bump,
     release commit and tag push are skipped because the tag is already the release commit;
   - neither exists: the normal path, which also stops when `main` has moved past the validated
     commit, so a stale run cannot resurrect an older tree;
4. on the normal path, bumps `[workspace.package] version` in `Cargo.toml` and lets `cargo metadata`
   rewrite the twelve member versions in `Cargo.lock`, then commits
   `chore(release): vX.Y.Z [skip ci]`;
5. creates the tag and pushes the commit and the tag atomically (`git push --atomic`): a rejected
   push leaves neither behind;
6. on both the normal and the recovery path, checks out the tag and builds the Windows executable
   with `cargo build --release -p lovelace-desktop` from that commit. The built file is run with
   `--version` and the job fails unless it prints `lovelace-desktop X.Y.Z` for the tag's version, so
   the attached asset cannot disagree with its tag;
7. writes `SHA256SUMS.windows` over that built file, generates the notes with
   `tools/release-notes.ps1`, and creates the GitHub Release with `gh release create --verify-tag`
   and those two files;
8. publishes `released` and `tag` as job outputs for the Linux job.

The `linux` job (`name: Publish the Linux executable`, `needs: release`) runs only when
`needs.release.outputs.released == 'true'`:

1. checks out `needs.release.outputs.tag` -- the released commit, never `main` -- and proves HEAD is
   that tag's commit with `git rev-list -n 1 refs/tags/<tag>`;
2. installs the same Linux GUI packages as CI and runs
   `cargo build --release -p lovelace-desktop`;
3. runs the built file with `--version` and fails unless it prints `lovelace-desktop X.Y.Z` for the
   tag -- the same guard the Windows job runs;
4. renames the file to `lovelace-desktop-linux-x86_64`, marks it executable, writes
   `SHA256SUMS.linux`, and verifies the file against that checksum;
5. uploads both with `gh release upload <tag> lovelace-desktop-linux-x86_64 SHA256SUMS.linux
   --clobber`; it never creates a release;
6. downloads the uploaded asset back with `gh release download`, verifies it against
   `SHA256SUMS.linux`, and runs `--version` on the downloaded bytes;
7. reads the release back with `gh release view` and fails unless the Windows executable, the Linux
   executable and both checksum files are attached.

The release commit carries `[skip ci]`, so the version bump cannot start CI again. The `concurrency`
group `release-${{ github.ref }}` queues overlapping runs instead of racing them, and the guard's
tag-and-release check turns the queued run into a no-op. The workflow asks for `contents: write`
(push and release); the Linux job repeats that permission at job level because it uploads assets.

Every release carries four assets:

| Asset | Platform | What it is |
|---|---|---|
| `lovelace-desktop.exe` | Windows x86_64 | the executable built at the tagged release commit by the `release` job |
| `SHA256SUMS.windows` | -- | `"<sha256>  lovelace-desktop.exe"` |
| `lovelace-desktop-linux-x86_64` | Linux x86_64 (glibc >= 2.39) | the executable built at the same tagged commit by the `linux` job |
| `SHA256SUMS.linux` | -- | `"<sha256>  lovelace-desktop-linux-x86_64"` |

The checksums are deliberately **per platform**, not one combined `SHA256SUMS`:

- each job hashes only the file it built and publishes that digest itself, so no job downloads or
  hashes the other platform's bytes;
- a failure in one job cannot leave a combined file that silently omits the platform that failed;
- each checksum file names exactly one asset, so `sha256sum -c SHA256SUMS.linux` on Linux and
  `sha256sum -c SHA256SUMS.windows` (or `Get-FileHash` on Windows) cannot verify the wrong file.
  GitHub's per-asset digest is a third, independent record of the same bytes.

The assets are rebuilt in the release jobs instead of being taken from CI's artifacts because CI
builds the validated commit *before* the version bump; attaching that file is what made `v0.2.0`
ship an executable that printed `0.1.0`. Building at the tag and checking `--version` makes each
asset and the tag agree.

### The Linux asset

`lovelace-desktop-linux-x86_64` is the same workbench as the Windows build, rendered through glow
on Linux. It is built on `ubuntu-24.04` and therefore requires **glibc >= 2.39**; distributions with
an older libc should build from source with the packages in
[build-and-run.md](build-and-run.md#linux). What is verified is **X11**: CI runs the selftest and
the desktop suite under Xvfb, and the window capture comes from WSLg. Wayland is not tested. The
build is verified with **software rendering** (mesa's llvmpipe); no GPU driver is claimed. After
downloading, run `chmod +x lovelace-desktop-linux-x86_64`, then
`./lovelace-desktop-linux-x86_64 --version` to check the tag.

To retry a release, re-run the release workflow that belongs to the triggering CI run
(`gh run rerun <release-run-id>`). If the tag and the release both exist the guard stops, so a retry
cannot double-publish. If the tag exists but the release does not, the same re-run rebuilds from the
tag and publishes it. If the job failed before the tag was pushed, the re-run is the normal path.
If the release exists but the Linux asset is missing (the `linux` job failed after `release`
succeeded), re-run the failed `Publish the Linux executable` job rather than the whole workflow: a
fresh run stops at the guard because the release now exists, and `gh release upload --clobber`
makes the job's own retry safe.

## The release build

```console
$ cargo build --release -p lovelace-desktop
   Finished `release` profile [optimized] target(s) in 1m 34s
```

The release profile is `opt-level = 3`, `lto = "thin"`, `codegen-units = 1` (`Cargo.toml`). The
artifact is:

```text
target/release/lovelace-desktop.exe
```

Its size and SHA-256 change with every build, so the authoritative values are the ones in the
published release note, `SHA256SUMS.windows` and `SHA256SUMS.linux`, not a number copied into this
page. For the historical 0.1.0 tree the Windows file was 24 169 472 bytes with SHA-256
`0135514eb6bf5e66ba6cfbe250ee636655014954288e3967e44bec78d3eb79b8`.

CI performs the same build in its Windows job and uploads the file as the artifact
`lovelace-desktop-windows-x86_64` (`.github/workflows/ci.yml`), with `if-no-files-found: error` so a
silent build failure cannot produce an empty release. The Linux job uploads its GUI binary as
`lovelace-desktop-linux-gui-x86_64` (renamed from `lovelace-desktop-linux-x86_64` when the Linux
build became the full workbench). Those artifacts are evidence that the validated tree builds; they
are not the released files, because CI builds the validated commit before the release workflow adds
the version bump. The two release jobs rebuild at the tagged release commit and verify `--version`
against the tag before attaching anything.

## Artifact layout

CI produces two files, one per platform:

```text
lovelace-desktop-windows-x86_64/
  lovelace-desktop.exe      the workbench, its headless modes included
lovelace-desktop-linux-gui-x86_64/
  lovelace-desktop          the same workbench, built and self-tested on Linux
```

Neither is a release asset under these names: the release jobs rebuild both executables at the tag.
The released Linux file is `lovelace-desktop-linux-x86_64` (no extension, as uploaded by the `linux`
job), not `lovelace-desktop`; the released Windows file is `lovelace-desktop.exe`. There is no
installer, no MSIX, no side-by-side DLL set and no packaging script in the tree (a
`git ls-files` search for release/dist/package/xtask tooling finds none). The Windows binary needs
the Microsoft Visual C++ runtime, like any MSVC-linked Rust executable; the Linux binary needs
glibc >= 2.39 and the X11/GL/font packages listed in [build-and-run.md](build-and-run.md#linux). The
engine, the CLI (`lovelace-run`), the corpus gate and the knowledge graph are built from source by
consumers who need them; they are not shipped in these artifacts.

## Signing caveat

The tree contains no signing step: a search for `signtool`, `Authenticode`, `certificate` or
`self-signed` across the sources, manifests and workflow finds nothing. The released executables are
therefore **unsigned**, and Windows shows "Unknown publisher" in SmartScreen for the Windows file
until enough users have run that hash.

If a release is signed at all, the only option without a certificate authority is a **self-signed
certificate**, and that comes with caveats worth stating plainly:

- A self-signed signature does not establish trust. Windows still warns unless the certificate is
  installed in the machine's Trusted Root store, which requires an administrator and is a manual step
  per machine.
- SmartScreen reputation is per-signing-identity and per-hash; a self-signed certificate does not
  accumulate publisher reputation, so the warning persists for every new hash.
- Anybody can create a certificate with the same subject name. A subject line is not identity, and a
  release note must not present it as one.
- Signing changes the artifact hash. Publish the hash of the signed file, not of the build input.

The honest release note is therefore: unsigned build, hash published, SmartScreen warning expected.

## Release checklist

The automatic workflow runs the gate below in CI. A manual release is the fallback, and it must
prove the same things on the release commit:

1. `cargo fmt --all -- --check` — the workspace is formatted.
2. `cargo clippy --workspace --all-targets` — read the warnings; the workspace has pre-existing ones
   and CI does not use `-D warnings`.
3. `cargo test --workspace` — every suite. Expect the counts in [testing.md](testing.md).
4. `cargo run --release -p lovelace-corpus` — `corpus: 214 passed, 0 failed, 214 total`.
5. `cargo test -p lovelace-desktop` — the desktop gate.
6. `cargo run -p lovelace-desktop -- --gui-selftest` with a writable `APPDATA` —
   `GUI_SELFTEST summary checks=20 failed=0`.
7. `cargo run -p lovelace-desktop -- --contrast-check` — "every required pair passes".
8. `cargo run -p lovelace-desktop -- --journey <the documented set> --out <dir>` — the recorded
   journeys match their expected observables.
9. `cargo build --release -p lovelace-desktop` — the artifact above.
10. Record each artifact's size and SHA-256 in the release note or its checksum file, along with the
    unsigned/SmartScreen caveat. The automatic workflow does this from the executables it built at
    the tag and verified with `--version`: the Windows size/hash go into the release note and
    `SHA256SUMS.windows`; the Linux digest goes into `SHA256SUMS.linux`.
11. Update `[workspace.package] version` in `Cargo.toml`, commit `Cargo.lock` with it, and tag
    `vX.Y.Z`. The automatic workflow does this; do not tag by hand while it is green.

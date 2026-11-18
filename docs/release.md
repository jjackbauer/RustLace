# Release

## Versioning

One version governs the workspace: `[workspace.package] version` in `Cargo.toml`, and every member
inherits it with `version.workspace = true`. There are no per-crate versions and no published
Rust crates: the release artifacts are a Windows executable and a Linux executable, and each desktop
binary prints the version compiled into it from its package metadata (`--version` ->
`lovelace-desktop <compiled version>`).

The version is derived from the commit history, not chosen by hand. `tools/next-version.ps1` reads
the current workspace version and the commits since the last `vX.Y.Z` tag (every commit when no tag
exists yet) and prints the next version. It is a PowerShell script with no bash port, so run it with
Windows PowerShell or PowerShell 7; the workflow calls it on `windows-latest`:

```console
# Windows PowerShell (add -NoProfile -ExecutionPolicy Bypass if the policy blocks local scripts);
# on a host that has PowerShell 7 instead: pwsh -File tools/next-version.ps1 <same arguments>
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

### What must be green before a tag

The release workflow's success condition is the **whole `CI` workflow** on the validated commit, so
every CI job is a release gate — not only the two that produce an executable. `ci.yml` has three
job definitions and produces four job results; all four must be green before anything is tagged:

| Job | Runner | What it proves before a tag |
|---|---|---|
| `Windows (full workspace)` | `windows-latest` | `cargo fmt --all -- --check`, clippy, `cargo test --workspace`, the blocking release corpus gate (`cargo run --release -p lovelace-corpus`), and the release desktop build (uploaded as `lovelace-desktop-windows-x86_64`) |
| `Linux (engine crates + GUI desktop under Xvfb)` | `ubuntu-24.04`, pinned rather than `ubuntu-latest` | the engine crates, the blocking release corpus gate (`cargo run --release -p lovelace-corpus`), the GUI desktop built and self-tested under Xvfb, the glibc guard, and the Linux GUI binary (uploaded as `lovelace-desktop-linux-gui-x86_64`) |
| `Documentation examples (ubuntu-latest)` | `ubuntu-latest` | both example runners, **Compile and run the crate-page examples** (`bash tools/check-crate-examples.sh`), **Run the tutorials** (`bash tools/run-tutorials.sh`), all three `check-docs.sh` modes (links and symbols, claims, README), `check-reference-pages.sh`, and the mutation self-test in `test-docs-gates.sh` |
| `Documentation examples (windows-latest)` | `windows-latest` | the same documentation gate on Windows |

The documentation job is **one definition that runs twice**, and the two results are separate gates:
a documentation-only defect blocks the tag exactly like a failing test. That includes a broken
relative link, a `.rs:NNN` citation outside `docs/adr/`, a dead symbol citation, a prohibited
numerical claim, a tutorial without its precision section, a README without its limitations section,
an undeclared example page, an unpaired `lovelace` block, a Rust fence on a crate page that no longer
compiles and runs, a tutorial whose execution no longer matches its recorded output, a reference
page missing or reordering a section, and a gate whose own mutation test stops failing. One green
documentation variant does not vouch for the other: a path shape, `.exe` lookup or line ending can
work on one platform and fail on the other.

No test count is a release criterion in this section: the gate is an exit status, and the counts a
release note quotes are read from this commit's run and compared with the dated per-commit table in
[testing.md](testing.md).

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
   rewrite the twelve member versions in `Cargo.lock`, then commits `chore(release): vX.Y.Z`;
5. pushes that commit to a `release/vX.Y.Z` branch with the repository deploy key stored in the
   `RELEASE_DEPLOY_KEY` secret, and waits for the CI run the push starts. A deploy key is used
   because a push made with the workflow's own `GITHUB_TOKEN` starts no CI run, so the release
   commit could never acquire the required checks. Only when the same seven checks `main` enforces
   are green does it fast-forward `main` onto the validated commit and push the tag: branch
   protection never sees an unvalidated commit, and a red or timed-out run leaves `main` and the tag
   untouched. The deploy key gets no bypass; it only pushes the unprotected release branch;
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
artifact is the host's own file name:

```text
target/release/lovelace-desktop.exe    the Windows build
target/release/lovelace-desktop        the Linux build
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
4. `cargo run --release -p lovelace-corpus` — prints `corpus: <N> passed, 0 failed, <N> total` for
   the corpus at this commit, with N nonzero and no failure.
5. `cargo test -p lovelace-desktop` — the desktop gate. On Linux run it under Xvfb, as CI does:
   `xvfb-run -a -s "-screen 0 1600x1000x24" cargo test -p lovelace-desktop`.
6. `cargo run -p lovelace-desktop -- --gui-selftest` with a writable application directory
   (`APPDATA` on Windows, `XDG_CONFIG_HOME` or `HOME` on Linux) — prints
   `GUI_SELFTEST summary checks=<N> failed=0` with N nonzero and no failed check.
7. `cargo run -p lovelace-desktop -- --contrast-check` — "every required pair passes".
8. `cargo run -p lovelace-desktop -- --journey J-02 J-03 J-07 J-08 J-09 J-10 J-15 --out <dir>` — the
   seven documented journeys match their expected observables (`<dir>` is POSIX on bash and a
   backslash path in Windows PowerShell).
9. `cargo build --release -p lovelace-desktop` — the artifact above.
10. The documentation gate, exactly as the `Documentation examples` job runs it — both example
    runners, the crate-page examples, the four tutorials, all three `check-docs.sh` modes, the
    reference-page contract, and the gate self-test:

    ```console
    bash docs/language/examples/run-examples.sh
    bash docs/extensions/examples/run-examples.sh
    bash tools/check-crate-examples.sh
    bash tools/run-tutorials.sh
    bash tools/check-docs.sh --symbols
    bash tools/check-docs.sh --claims
    bash tools/check-docs.sh --readme
    bash tools/check-reference-pages.sh
    bash tools/test-docs-gates.sh
    ```

    Every command exits 0, the self-test prints one `PASS` line per mutation and ends with
    `test-docs-gates: OK`, neither example runner reports a failed or unpaired example, every
    crate-page Rust fence compiles and runs, and the tutorial runner ends with `run-tutorials: OK`.
11. Record each artifact's size and SHA-256 in the release note or its checksum file, along with the
    unsigned/SmartScreen caveat. The automatic workflow does this from the executables it built at
    the tag and verified with `--version`: the Windows size/hash go into the release note and
    `SHA256SUMS.windows`; the Linux digest goes into `SHA256SUMS.linux`.
12. Update `[workspace.package] version` in `Cargo.toml`, commit `Cargo.lock` with it, and tag
    `vX.Y.Z`. The automatic workflow does this; do not tag by hand while it is green.

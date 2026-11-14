# Release

## Versioning

One version governs the workspace: `[workspace.package] version = "0.1.0"` in `Cargo.toml`, and every
member inherits it with `version.workspace = true`. There are no per-crate versions and no published
crates: the release artifact is a Windows executable.

Bump the version in one place (`Cargo.toml`), and let `Cargo.lock` update in the same commit. The
desktop binary prints it from the package metadata (`--version` → `lovelace-desktop 0.1.0`).

## The release build

```console
$ cargo build --release -p lovelace-desktop
   Finished `release` profile [optimized] target(s) in 1m 34s
```

The release profile is `opt-level = 3`, `lto = "thin"`, `codegen-units = 1` (`Cargo.toml`). The
artifact is:

```text
target/release/lovelace-desktop.exe        24 169 472 bytes for the 0.1.0 tree
sha256 0135514eb6bf5e66ba6cfbe250ee636655014954288e3967e44bec78d3eb79b8
```

CI performs the same build in its Windows job and uploads exactly that file as the artifact
`lovelace-desktop-windows-x86_64` (`.github/workflows/ci.yml`), with `if-no-files-found: error` so a
silent build failure cannot produce an empty release.

## Artifact layout

The repository produces one file and nothing else:

```text
lovelace-desktop-windows-x86_64/
  lovelace-desktop.exe      the workbench, its nine headless modes included
```

There is no installer, no MSIX, no side-by-side DLL set and no packaging script in the tree (a
`git ls-files` search for release/dist/package/xtask tooling finds none). The binary needs the Microsoft
Visual C++ runtime, like any MSVC-linked Rust executable. The engine, the CLI (`lovelace-run`), the
corpus gate and the knowledge graph are built from source by consumers who need them; they are not
shipped in this artifact.

## Signing caveat

The tree contains no signing step: a search for `signtool`, `Authenticode`, `certificate` or
`self-signed` across the sources, manifests and workflow finds nothing. The released executable is
therefore **unsigned**, and Windows shows "Unknown publisher" in SmartScreen until enough users have
run that hash.

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
10. Record the artifact's size and SHA-256 in the release note, along with the unsigned/SmartScreen
    caveat.
11. Update the version in `Cargo.toml` and commit `Cargo.lock` with it.

# Harness State — goal-docs-v2

- **Round**: 9 complete — rounds 7–8 landed at `b0ed8f5` and the hosted run is green 4/4
- **Goal**: The branch satisfies RQ-001…RQ-024 with reproduced evidence, including green hosted
  GitHub Actions runs on Ubuntu and Windows at a named commit, with no requirement closed as a
  residual.
- **Definition of done**: **24/24 met** (C1–C24)
- **Stop criteria**: (1) C1–C24 all reproduce — **met**; (2) hosted runs recorded — **met**
  (run 43902955424, 4/4 jobs, at `b0ed8f5`); (3) no residual — **met**

## Gate status

| Gate | Status | Detail |
|---|---|---|
| G0 Intent | PASS | audit 27/27 KEPT; DEC-001 (all 24 gate) |
| G1 Evidence | PASS | 58 EVD rows, each reproduced by the orchestrator |
| G2 Falsification | PASS | VAL-002, VAL-004, VAL-009 falsified; each remediated or superseded |
| G3 Coverage | PASS | 24/24 checks met; hosted run 43902955424 green 4/4 at `b0ed8f5` |
| G4 Reproduction | PASS | rounds 1–8 re-run; every mutation restored hash-identically |
| G5 Honesty | PASS | two hosted-CI failures that local gates called green are recorded (EVD-029, EVD-057) |

## Round 8 objective and outcome

Make the generated crate-example manifest platform-native, because hosted Windows at `89ea5c7`
failed with `failed to read C:\d\a\RustLace\RustLace\lovelace-array\Cargo.toml` (EVD-057).

- `tools/check-crate-examples.sh` builds every manifest path through `toml_path`: `cygpath -w` when
  cygpath exists, the absolute path unchanged on Linux/macOS/WSL, backslashes doubled for the TOML
  basic string. A manifest-load failure on a cygpath host now explains the cause and prints the
  recorded paths and the command that shows the native spelling.
- A failing `cygpath -w` exits 2 (setup) instead of writing `path = ""`.
- `tools/run-tutorials.sh` was audited: it already converts every path handed to `lovelace-run`
  through `cygpath -w` (`tools/run-tutorials.sh:106-119,746-747`), so it has no instance of this
  defect.
- The self-test grew to **31 mutations**, including mutation 31: a POSIX path in the generated
  `Cargo.toml` is a Windows build failure, with a fake cygpath that converts the way Git Bash does.

## Round 9 (this round) — reproduced on a real Git Bash host

On Windows Git Bash at `F:\dev\RustLace`, with a stand-in cargo that fails exactly as the hosted
Windows job did, the real script recorded:

```
lovelace-array = { path = "F:\\dev\\RustLace\\lovelace-array" }
```

Decoding the TOML basic string gives `F:\dev\RustLace\lovelace-array`, which resolves through the
native Windows API (`Test-Path` → True for all seven crates). The pre-fix shapes
`/f/dev/RustLace/lovelace-array` and `/d/a/RustLace/RustLace/lovelace-array` both resolve False,
which is the hosted defect. `cygpath -w` returns the same native spelling. All 11 local gates pass
at the new checkout location.

## Round 9 outcome — C13 closed

Rounds 7–8 were committed as `b0ed8f5` and pushed, and `CI` run **43902955424** ran at that exact
commit with **4 of 4 jobs successful**:

| Job | Result | Job id |
|---|---|---|
| Windows (full workspace) | success (8m8s) | 133424152063 |
| Linux (engine crates + GUI desktop under Xvfb) | success (5m18s) | 133424152489 |
| Documentation examples (ubuntu-latest) | success (1m10s) | 133424152456 |
| Documentation examples (windows-latest) | success (6m13s) | 133424153047 |

The Windows documentation job is the leg that failed at `89ea5c7`; it now passes the same step,
`Compile and run the crate-page examples`, and its downloaded transcript records
`check-crate-examples: 13 page(s), 7 rust fence(s): 7 compiled and ran, 0 excluded` while the
tutorial gate resolves its runner at `/d/a/RustLace/RustLace/target/debug/lovelace-run.exe` — the
POSIX host spelling that caused the original defect. Both transcripts end with
`DOCREPORT total gates=4 ok=4 failed=0 commit=b0ed8f5`, `os=Linux` and `os=MINGW64_NT-10.0-26100`.
This is EVD-059 and EVD-060.

## Dimension status

| # | Check | Status |
|---|---|---|
| C1 | movingavg regression | **Met** (EVD-016) |
| C2 | movingaverage precision scope | **Met** (EVD-026, EVD-027) |
| C3 | four claim categories | **Met** (EVD-026, EVD-037, EVD-038) |
| C4 | single-platform instruction | **Met** (EVD-010, EVD-011) |
| C5 | three crate-page errors | **Met** (EVD-012, EVD-013) |
| C6 | all consistency modes in CI | **Met** (EVD-018, EVD-035, EVD-044, EVD-058) |
| C7 | zero-selection false positives | **Met** (EVD-009, EVD-014, EVD-017) |
| C8 | host-specific tutorial paths | **Met** (EVD-031) |
| C9 | tutorials as CI contracts | **Met** (EVD-030, EVD-035) |
| C10 | Rust reference fences compile | **Met** (EVD-022, EVD-034) |
| C11 | test-count provenance | **Met** (EVD-020, EVD-021) |
| C12 | release instructions | **Met** (round 1 report; release skill covers three jobs, both OSes, both binaries and checksums) |
| C13 | hosted runs at the final commit | **Met** — run 43902955424 at `b0ed8f5`: Windows workspace, Linux GUI, Ubuntu docs and Windows docs all **success**; job IDs 133424152063 / 133424152489 / 133424152456 / 133424153047; both docs artifacts read (`DOCREPORT total gates=4 ok=4 failed=0`) |
| C14 | builtin-catalogue sync | **Met** (EVD-045) |
| C15 | root Markdown + anchors | **Met** (EVD-041, EVD-056 → fixed, EVD-058) |
| C16 | two-way reference manifest | **Met** (EVD-042) |
| C17 | obsolete assertions | **Met** (EVD-046, EVD-049) |
| C18 | exactness wording sweep | **Met** (EVD-026) |
| C19 | tutorial evidence depth | **Met** (EVD-030, EVD-037) |
| C20 | README + navigation | **Met** (EVD-051) |
| C21 | language-page provenance | **Met** (EVD-050) |
| C22 | agent-instruction command audit | **Met** (EVD-046, EVD-049) |
| C23 | machine-readable docs summary | **Met** (EVD-053, EVD-054) |
| C24 | harness logs separated | **Met** (EVD-040, EVD-055) |

## Objective ledger

| Round | Objective | Mode | Agents | New EVD | Falsified | Gates | Outcome |
|---|---|---|---|---|---|---|---|
| 0 | Phase 0 contract + intent audit | Probe | 1 | 8 | 0 | G0✓ G1✓ G3✗ | complete |
| 1 | RQ-004, 005, 007, 012 | Probe | 2 | 7 | 1 | G1✓ G2✓ G4✓ | **verified** |
| 2 | RQ-006, 007.5, 010, 011 | Probe | 2 | 7 | 1 | G1✓ G2✓ G4✓ | **verified** |
| 3 | RQ-002, 003, 008, 009 + README | Probe | 2 | 7 | 0 | G1✓ G2✓ G4✓ | **verified** |
| 4 | TODO-003 + CI wiring | Probe | 2 | 5 | 0 | G1✓ G2✓ G4✓ | **verified** |
| 5 | RQ-014, 015, 016, 022 + TODOs | Probe | 2 | 9 | 0 | G1✓ G2✓ G4✓ | **verified** |
| 6 | RQ-017, 019, 020, 021, 023, 024 | Probe | 2 | 6 | 0 | G1✓ G2✓ G4✓ | **verified** |
| 7 | checker gaps (directory paths, retired terms) | Probe | 1 | 2 | 0 | G1✓ G2✓ G4✓ | **verified** |
| 8 | native Cargo manifest for Git Bash | Probe | 1 | 1 | 0 | G1✓ G2✓ G4✓ | **verified locally** |
| 9 | land rounds 7–8, hosted re-run, close C13 | Verify | 0 | 4 | 0 | G1✓ G2✓ G3✓ G4✓ | **verified** |

## Next objective

None. All 24 checks are met and stop criterion 2 is satisfied by hosted run 43902955424 at
`b0ed8f5` (4/4 jobs). The branch is ready for the user to inspect; merging to `main` is the user's
decision and is not part of this goal.

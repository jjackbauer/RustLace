# Build and run

## Prerequisites

| Requirement | Why | Notes |
|---|---|---|
| Rust 1.85 or newer | `rust-version = "1.85"`, edition 2024 (`Cargo.toml`) | The documented runs used rustc/cargo 1.99.1 |
| MSVC build tools (Windows) | `link.exe` for every binary | Present as Visual Studio 2022 Build Tools on the documenting host |
| A Windows desktop session | the native workbench | `lovelace-desktop` is Windows-only; the engine, CLI and headless modes build anywhere |
| A crate registry reachable once | the desktop's GUI stack | `eframe`/`egui` 0.37, `resvg` 0.50, `raw-window-handle` 0.6; everything else is in-tree |

No registry is needed for the engine, the runner, the corpus gate or the knowledge graph
(ADR-001).

## Build

```console
$ cargo build --workspace
```

The workspace has twelve members; `lovelace-desktop-spike` is deliberately outside it and is not
built by this command (`lovelace-desktop-spike/Cargo.toml`).

Profiles (`Cargo.toml`):

- `dev` – unoptimized with debuginfo. This is what `cargo build` and `cargo test` use.
- `release` – `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`.

```console
$ cargo build --release -p lovelace-desktop
   Finished `release` profile [optimized] target(s) in 1m 34s
```

The release binary is `target/release/lovelace-desktop.exe` (24 169 472 bytes for this tree).

## Run the CLI

```console
$ cargo run -p lovelace-run -- --eval "1/3" --text
= 0.(3) (Real)

$ cargo run -p lovelace-run -- --eval "1/3"
{"artifacts":[],"elapsed":"407.1 µs",...,"result":{"display":"0.(3)","kind":"Real",...,"typed":"0.(3) (Real)"},...}
```

Sources: `--eval <src>`, `--file <path>`, `--stdin`, or a bare script path. Options include
`--plot-dir`, `--plot-file`, `--cancel-after <ms>`, `--print-budget <n>`, `--omit-functions`,
`--omit-variables`, `--functions [pattern]`, `--function <name>`, `--function-limit <n>`, `--repl`,
`--json` and `--text` (`lovelace-run/src/main.rs:70`). Exit codes: 0 success, 1 script or diagnostic
failure, 2 usage.

## Run the desktop

```console
$ cargo run -p lovelace-desktop                  # the workbench
$ cargo run -p lovelace-desktop -- notes.lv      # opened on a file
$ cargo run -p lovelace-desktop -- --gui-selftest
$ cargo run -p lovelace-desktop -- --help
```

The window writes its start-up line to stderr so a redirected launch still shows where plots go:

```text
lovelace-desktop: plot directory C:\Users\<you>\AppData\Roaming\Lovelace\plots
```

On a display scaled to 300% the window is a 4800x3000 physical window whose logical layout is
1600x1000; the screenshots in `docs/images` are that frame resampled to 1600x1000 (see
`docs/images/README.md`).

## Environment

| Variable | Effect | Source |
|---|---|---|
| `APPDATA` | Root of the application directory: `desktop.json`, `plots\`, `recovery\` | `lovelace-desktop/src/settings.rs`, `engine.rs:65` |
| `LOVELACE_THREADS` | Pins the worker budget for determinism experiments | `lovelace-nat/src/lib.rs:85` |
| `LOVELACE_NTT_THRESHOLD` | Pins the NTT crossover without rebuilding | `lovelace-nat/src/lib.rs:105` |
| `LOVELACE_FACTORIAL_LEAF` | Pins the factorial product-tree leaf size | `lovelace-nat/src/lib.rs:811` |
| `LOVELACE_RUN` | Path to the runner the knowledge graph spawns | `lovelace-know/src/runner.rs:210` |
| `LOVELACE_RECORD_GOLDENS` | `=1` rewrites the golden wire fixtures instead of comparing | `lovelace-run/src/golden_batch.rs:17` |
| `LOVELACE_RUN_TRACE` | File (or `stderr`) that receives one line per run mark | `lovelace-desktop/src/run_trace.rs:15` |
| `LOVELACE_MEASURE_TRACE` | Per-pass CSV path for the measurement modes | `lovelace-desktop/src/ui/measure.rs:125` |
| `LOVELACE_ENGINE_FAULT` | `panic-once` injects one engine panic (RUN-017 recovery capture) | `lovelace-desktop/src/engine/fault.rs:18` |
| `LOVELACE_PLOT_FAULT` | `malformed` or `oversized` corrupts the plot capture | `lovelace-desktop/src/ui/app.rs:999` |
| `LOVELACE_LOG_FLOOD` | `<n>` appends n stdout records and one fatal at launch | `lovelace-desktop/src/ui/app.rs:996` |
| `LOVELACE_LOG_START_SCROLLED` | Starts the log view away from the newest record | `lovelace-desktop/src/ui/app.rs:801` |
| `LOVELACE_IME_NO_SHORTCUT` | `=1` skips the shortcut-during-composition probe | `lovelace-desktop/src/ime_probe.rs:462` |
| `LOVELACE_PROBE_DWMFLUSH` | `=1` blocks on `DwmFlush` once per frame probe pass | `lovelace-desktop/src/frame_probe.rs:10` |

The `LOVELACE_*_FAULT`, `_FLOOD` and `_START_SCROLLED` switches are acceptance seams: they exist so a
test can reach a state a script cannot, and none of them is on by default. `LOVELACE_LOG_FLOOD=4000`
produces the flooded log below — tail-follow at the newest record, ending in the injected fatal
diagnostic.

![A flooded log: 4000 injected records, tail-follow at the newest one, and the injected LOGCAP-900 fatal line](images/log-flood.png)

## Troubleshooting

**`link.exe` fails with `LNK1104: cannot open file '...\Temp\lnk{...}.tmp'`.** The documenting host
denied the linker its `%TEMP%` scratch files. Point `TMP`/`TEMP` at a writable directory before
building:

```powershell
$env:TMP = "$PWD\.tmp"; $env:TEMP = $env:TMP
cargo build --workspace
```

**`failed to remove file ... lovelace-desktop.exe: Access is denied`.** A previous desktop process
still holds the binary. Close it (or `Stop-Process -Name lovelace-desktop`) and rebuild.

**The desktop builds but no window appears.** The windowed `main` is `#[cfg(windows)]`
(`main.rs:93`). On another platform the headless modes still work:
`cargo run -p lovelace-desktop -- --gui-selftest`.

**`Cannot create the plot directory ... Access is denied`.** The log pane reports it at start-up; the
process could not create `%APPDATA%\Lovelace\plots`. Set `APPDATA` to a writable directory. Plot
statements then fail with that message rather than silently writing nowhere (`engine.rs:90`).

**`cargo build` cannot reach a registry.** The engine and the CLI do not need one. The desktop does:
its GUI stack is the only external dependency, and it is Windows-only.

**A settings file written by a newer build.** `Settings::from_json` has no error return: an unknown
schema, a wrong field type or a corrupt file yields the defaults plus a warning
(`settings.rs:1`).

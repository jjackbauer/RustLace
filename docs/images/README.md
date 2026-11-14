# Screenshots

Every PNG in this directory is a real frame of the release build of `lovelace-desktop` running on
Windows 11 (10.0.22631), captured with `PrintWindow(PW_RENDERFULLCONTENT)` by
[`capture.ps1`](capture.ps1). No mockup, no reconstructed layout, no image borrowed from the C#
project.

- **Binary:** `target/release/lovelace-desktop.exe`, built from this tree with
  `cargo build --release -p lovelace-desktop`.
  - bytes: 24 169 472
  - sha256: `0135514EB6BF5E66BA6CFBE250EE636655014954288E3967E44BEC78D3EB79B8`
- **Capture method:** `PrintWindow` with `PW_RENDERFULLCONTENT` (flag `2`), not `CopyFromScreen`, so
  the frame is complete even when the session is locked or the window is occluded.
- **Environment:** the display is 3840x2160 at 300% scaling, so the workbench window is 4800x3000
  physical pixels for a 1600x1000 logical layout; the capture is resampled to the logical size before
  it is saved. The exception is `layout-compact-200.png` (900x600 logical, 2700x1800 physical).
- **APPDATA:** redirected outside the repository, to a scratch directory under
  `rust/target-desktop/tmp/docs-capture`, so the plots, settings and recovery records written during
  the captures never land in the checkout. On a normal machine the product writes under
  `%APPDATA%\Lovelace`.

| File | Size (bytes) | Resolution | SHA-256 | What it shows |
|---|---|---|---|---|
| `welcome-dark.png` | 308 089 | 1600x1000 | `706DEFDCEA5225A1E6B05A4AA5047D51544D211038FA218C1A5F9B157E1C9F6D` | First launch, dark theme: the welcome document, the empty graph/workspace/logs/inspector panes, the status strip at `run Idle` |
| `welcome-light.png` | 263 718 | 1600x1000 | `820BE5A14455AFF9EB5BDDD7BB0830B775FADF9C25235026621AD2FDF682494A` | The same screen after the `Theme: Dark` toggle, proving a runtime theme switch with no document state change |
| `editor-and-results.png` | 351 919 | 1600x1000 | `CB27E1203D692E871FD82B3EF9C676C5A30CE000CF289125962A9DEBCD55DF90` | A finished run: `sum_squares = 2870`, 7 variables, 124 functions, the result/structured projection and the run log |
| `plot.png` | 287 737 | 1200x750 | `DCA2996FC40095EE29337FFA943C665099E91EBBC7834D62230E4675C5351DE0` | A rendered engine SVG with its provenance header (`series 1 · raster … · parse/raster/upload ms`), title and tick labels |
| `symbolic-inspector.png` | 342 611 | 1600x1000 | `7D600D08D0B2A146DC4B418863C733B2E497D3EFA733B74368194C5BB82EF622` | A symbolic run: the derivative `3*x^2 - 3` in the inspector pane as pretty and canonical forms, and the structured projection in the workspace |
| `find-and-replace.png` | 329 929 | 1600x1000 | `3A7FBF935C438CB19F5E6507E4D85BB35C0486695A536B9CFD03E70929C1743B` | The docked find bar with 11 matches highlighted, `1 of 11`, and the replace row |
| `signature-help.png` | 297 037 | 1600x1000 | `7D167F8DE4DE1FC78F1174D3CED1518082B5F51E62C2F49DC8918FCAA4B99EA0` | The signature popup while typing `matmul(`: `matmul(a, b)`, parameter 1 of 2, the summary, plus the breadcrumb |
| `log-flood.png` | 324 404 | 1600x1000 | `B61410D2E48BDDB6AB46C15C08DA6A77CB06C23430E329BCA583A4E354A47FC3` | 4000 flooded log records at newest-record tail-follow, ending in the fatal `LOGCAP-900` diagnostic |
| `layout-compact-200.png` | 186 296 | 900x600 | `EA938093B67367DAE4E1646D67EA8753B365997EDAC096BB36689DA41AFEA159` | The minimum window at 200% interface scale: the toolbar wraps to two rows and every pane stays inside the window |
| `progress-and-cancel.png` | 320 397 | 1200x750 | `AE5220C4D66EF68140310B6A9AD4605C33DEA241944327FAF57152AE4495C5EC` | A run in flight: `run Running`, `3/5` statement progress, per-statement log lines, and the toolbar `Cancel` action |

## Reproducing them

```console
$ cargo build --release -p lovelace-desktop
$ powershell -NoProfile -ExecutionPolicy Bypass -File docs/images/capture.ps1 -Scenario all \
    -Exe target/release/lovelace-desktop.exe \
    -WorkRoot C:\scratch\lovelace-capture
```

`-WorkRoot` is where the redirected `APPDATA` and the scenario scripts live; keep it outside the
checkout. `-Scenario <name>` captures one frame. The script writes a `capture.log` line for every
step and prints each image's bytes, resolution and SHA-256, which is how the table above was built.

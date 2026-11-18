# Run a script, make a plot, and read the JSON envelope

## Problem

You want a result that another program can consume, plus a plot generated as a file rather than
drawn in a window. You do not want to scrape the human `--text` format, and you do not want build
artifacts inside the repository.

The CLI's default mode is a JSON envelope. This tutorial runs a checked-in script through `--file`,
lets `plot` write into the operating system's temporary directory, and extracts the three result
fields a consumer usually needs — `result.display`, `result.kind` and `result.structured.exact` —
plus the path of the plot artifact.

**Prerequisites:** This example builds on
[Array](../language/values-and-types.md#vector-and-array) values and
[Real](../language/values-and-types.md#numeric-kinds-and-widening)
arithmetic. If you are new to running scripts, follow
[Get started](../start-here.md) before using the CLI's JSON mode.

## Source

```lovelace
A = [[2, 1], [1, 3]]
b = [[5], [6]]
sol = matmul(inv(A), b)
residual = matmul(A, sol)
path = plot([1, 2, 3], [1, 4, 9], "squares")
det(A)
```

The same script is checked in as [reproducible.ls](examples/reproducible.ls).

## Output

```text
{"display":"5","kind":"Real","exact":true,"plot":"<temp-dir>/plot.svg"}
```

`<temp-dir>` stands for the host's operating-system temporary directory. The absolute path is
host-specific; it is the only value in this transcript that is not stable between machines. On
Windows it is a `C:\...` path inside the JSON string, where each backslash is escaped as `\\`; the
transcript records the host-independent shape instead.

## Reading the result

The pipeline has three parts:

1. `lovelace-run --file docs/tutorials/examples/reproducible.ls` reads the checked-in script.
   `--stdin` could read the same text from a pipe, but a file keeps the command identical in bash,
   PowerShell and Command Prompt.
2. `plot(...)` writes an SVG through the engine's plot directory. The default is a `lovelace-plots`
   directory under the operating system's temporary directory — `%TEMP%` on Windows, the system
   temp root on Linux — created if it does not exist. No path is hard-coded, and nothing lands
   under the repository. `--plot-dir <path>` redirects it, and that directory is created on demand
   too; either way `artifacts[0].path` names the file.
3. The field selection prints the JSON object above. The Verification section gives a plain
   Windows PowerShell route (`ConvertFrom-Json`, no Python and no bash) and an optional bash route
   that needs Python 3. Both print the same four fields; only the temporary-directory prefix and the
   path separator can differ, which is why the transcript writes `<temp-dir>`.

The extracted object means:

- `"display":"5"` is `result.display`, the human-readable form of the value returned by the last
  statement, `det(A)`. The determinant is 5.
- `"kind":"Real"` is `result.kind`. As in the linear-algebra tutorial, an integer-valued
  determinant is carried as a `Real`.
- `"exact":true` is `result.structured.exact`. The structured projection also carries
  `"numerator":"5"` and `"denominator":"1"`; the value is exactly five, not a tolerance-limited
  approximation.
- `"plot":"<temp-dir>/plot.svg"` is `artifacts[0].path`. The artifact has `"kind":"plot"` and
  `"title":"squares"`; it is a line plot of the points `(1, 1)`, `(2, 4)` and `(3, 9)`. The
  envelope's `plot` object also carries the SVG text itself; a consumer that needs only the file can
  ignore that field.

The envelope keeps large numeric values as decimal strings. The extracted booleans are JSON
booleans, and `result.display` is always a string. Do not assume that a number in the envelope is a
JSON number; read it as the field's documented shape. The envelope also contains measurement fields
such as `elapsed` and `timings`; compare the stable `result` and artifact fields when you need
reproducibility, not the whole envelope byte for byte.

### Language values versus the runner's JSON fields

| In the example | Kind and reference | What the consumer should do |
|---|---|---|
| `A`, `b`, `sol`, `residual` | [Array](../language/values-and-types.md#vector-and-array) | Preserve the two-dimensional shape. A Vector is not a column matrix. |
| `det(A)` | Exact [Real](../language/values-and-types.md#numeric-kinds-and-widening) in this example | Read both `result.kind` and structured exactness; the display string alone is not a type contract. |
| `plot(...)` | [Plot builtin](../language/plots.md) producing an SVG artifact | Read `artifacts` for file paths; the path depends on the host. |
| `result.structured` | **JSON envelope object**, not a Lovelace [Record](../language/values-and-types.md#record) | This is transport metadata for an external process. Do not confuse it with a `Record` constructed by a language builtin. |
| `result.structured.exact` | Representation metadata ([exactness](../language/values-and-types.md#exactness)) | True describes this representation; it does not independently prove an algorithm's correctness. |

**Choose the right output surface:** `--text` is intended for people
reading an interactive result. The default JSON envelope is intended for
scripts and agents; handle its [error codes and statuses](../language/errors-and-diagnostics.md#the-error-envelope)
and avoid assumptions about variable ordering, elapsed time or host paths.

### Continue from here

See [Using RustLace as a consumer agent](../agent-usage.md) for complete
function discovery and structured refusal handling, and
[the plot reference](../language/plots.md) for the SVG artifact contract.

## Assumptions

- The checked-out RustLace development revision, run from the repository root with the project-root Cargo command
  `cargo run -p lovelace-run -- --file docs/tutorials/examples/reproducible.ls --omit-functions
  --omit-variables` on Windows, Linux or WSL. Rust 1.85+ with `cargo` on `PATH` is the only
  prerequisite for that main command; the first run compiles the runner.
- **Windows:** use Windows PowerShell 5.1, which ships with Windows 10/11, or PowerShell 7.
  `ConvertFrom-Json` is a built-in cmdlet, so the Windows route needs no Python, no bash and no
  here-document.
- **Linux or WSL:** the optional bash field selector needs `python3` on `PATH`; that is the only step
  in the tutorial that needs Python. `bash tools/run-tutorials.sh` needs bash, awk and grep and does
  not need Python.
- The plot directory is the operating system's temporary directory by default, and `--plot-dir
  <path>` overrides it. The CLI creates either directory if it does not exist, and the plot is
  written outside the repository by design.
- `A` is invertible; `det(A)` is exact and non-zero. The same script also computes `sol` and
  `residual`, which are visible in the envelope's `variables` when that array is not omitted.
- `--file` reads the checked-in script. No here-document and no Unix filesystem path is needed on
  any platform.
- The command writes only the plot file under the temporary directory; it writes nothing under the
  repository.

## Precision

The four claim kinds are **exact**, **precision-limited**, **error-bounded** and
**parity-verified**, following [the editorial guide](../editorial.md#the-four-kinds-of-claim).

| Value | Label | Justification |
|---|---|---|
| `2`, `1`, `1`, `3` in `A` | **exact** | Integer matrix entries. |
| `5`, `6` in `b` | **exact** | Integer right-hand side. |
| `1`, `2`, `3` and `1`, `4`, `9` in `plot` | **exact** | Integer coordinates. |
| `5` in `result.display` and `"numerator": "5"` | **exact** | The determinant is the integer 5; denominator 1. |
| `1` in `"denominator": "1"` | **exact** | The integer denominator of the exact value. |

Every value in the extracted object is **exact**: the matrix entries are integers, `det(A)` is the
integer 5, and the plot coordinates are integers. No value is **precision-limited**, because no
division is truncated and no transcendental is evaluated in this script. No value is
**error-bounded**, and none needs a bound: the determinant and the coordinates are exact integers.
No value is **parity-verified**: the envelope reports the engine's own result, not a reproduction of
a reference implementation. The boolean `true` in `result.structured.exact` is a provenance flag,
not a numeric value and not by itself a correctness certificate. The envelope's `elapsed` and
`timings` fields are host measurements rather than mathematical results; the page does not classify
or compare them.

## Verification

Run this from the repository root. The main command is the same in PowerShell and bash; only the
field selector differs.

**Windows (Windows PowerShell 5.1, shipped with Windows 10/11, or PowerShell 7):** `ConvertFrom-Json`
is built in, so this route needs no Python, no bash and no here-document. Keep the pipeline on one
line: Windows PowerShell 5.1 does not continue a pipeline after a trailing `|`.

```powershell
cargo run -p lovelace-run -- --file docs/tutorials/examples/reproducible.ls --omit-functions --omit-variables | ConvertFrom-Json | ForEach-Object { [pscustomobject]@{ display = $_.result.display; kind = $_.result.kind; exact = $_.result.structured.exact; plot = $_.artifacts[0].path } } | ConvertTo-Json -Compress
```

The object has the four fields above. Your temporary directory replaces `<temp-dir>`, and on Windows
the `plot` value is a `C:\...` path with escaped backslashes in the JSON.

**Linux or WSL (bash; requires Python 3 on `PATH`):** this selector is the optional second-shell
route. The main command and the rest of the tutorial need no Python.

```console
cargo run -p lovelace-run -- --file docs/tutorials/examples/reproducible.ls --omit-functions --omit-variables | python3 -c 'import json, sys; e = json.load(sys.stdin); r = e["result"]; print(json.dumps({"display": r["display"], "kind": r["kind"], "exact": r["structured"]["exact"], "plot": e["artifacts"][0]["path"]}, separators=(",", ":")))'
```

The independent check is arithmetic: `det(A) = 2*3 - 1*1 = 5` by hand, so the recorded
`"display":"5"` is a number a reader can recompute without the engine. The
[tutorial runner](../../tools/run-tutorials.sh) (`bash tools/run-tutorials.sh`) performs the same
field comparison without Python. It runs the checked-in script with `--plot-dir` pointing at a
scratch directory (the CLI creates it), checks `display`, `kind`, `exact`, `typed`, the artifact
`kind` and `title`, the file name in the path, and the SVG file itself, and treats the absolute
temporary-directory prefix as host information.

# Run a script, make a plot, and read the JSON envelope

## Problem

You want a result that another program can consume, plus a plot generated as a file rather than
drawn in a window. You do not want to scrape the human `--text` format, and you do not want build
artifacts inside the repository.

The CLI's default mode is a JSON envelope. This tutorial runs a small script through `--stdin`,
writes the plot under `/tmp/plots`, and extracts the three result fields a consumer usually needs:
`result.display`, `result.kind` and `result.structured.exact`.

## Source

```lovelace
A = [[2, 1], [1, 3]]
b = [[5], [6]]
sol = matmul(inv(A), b)
residual = matmul(A, sol)
path = plot([1, 2, 3], [1, 4, 9], "squares")
det(A)
```

## Output

```text
{"display": "5", "kind": "Real", "exact": true, "plot": "/tmp/plots/plot.svg"}
```

## Reading the result

The pipeline has three parts:

1. `lovelace-run --stdin` reads the script from standard input. The same script could be a file
   passed to `--file`; `--stdin` avoids creating a file just to run one example.
2. `--plot-dir /tmp/plots` redirects the SVG that `plot` writes. Without that option the plot would
   be written relative to the runner's working directory. The script's `plot(...)` call returns the
   path, which is stored in `path`; the envelope also publishes it as an artifact.
3. `python3` parses the envelope and prints only the selected fields. `--omit-functions` and
   `--omit-variables` keep the envelope small; they do not change the `result` or `artifacts`
   fields.

The extracted object means:

- `"display": "5"` is `result.display`, the human-readable form of the value returned by the last
  statement, `det(A)`. The determinant is 5.
- `"kind": "Real"` is `result.kind`. As in the linear-algebra tutorial, an integer-valued
  determinant is carried as a `Real`.
- `"exact": true` is `result.structured.exact`. The structured projection also carries
  `"numerator": "5"` and `"denominator": "1"`; the value is exactly five, not a tolerance-limited
  approximation.
- `"plot": "/tmp/plots/plot.svg"` is the path from the envelope's `artifacts[0]`. `plot` generated a
  line plot of the points `(1, 1)`, `(2, 4)` and `(3, 9)`, titled `squares`. The envelope's `plot`
  object also carries the SVG text itself; a consumer that needs only the file can ignore that
  field.

The envelope keeps large numeric values as decimal strings. The extracted booleans are JSON
booleans, and `result.display` is always a string. Do not assume that a number in the envelope is a
JSON number; read it as the field's documented shape. The envelope also contains measurement fields
such as `elapsed` and `timings`; compare the stable `result` and artifact fields when you need
reproducibility, not the whole envelope byte for byte.

## Assumptions

- RustLace 0.4.1, run through the debug CLI at `/tmp/rl-target/debug/lovelace-run` on WSL Ubuntu.
- `python3` is available on `PATH` for the extraction step. The runner itself does not require it.
- The plot directory is `/tmp/plots`. The runner creates `--plot-dir` if it does not exist, and the
  plot is written outside the repository by design.
- `A` is invertible; `det(A)` is exact and non-zero. The same script also computes `sol` and
  `residual`, which are visible in the envelope's `variables` when that array is not omitted.
- `--stdin` reads the script to end-of-input. The here-document supplies the source and is not part
  of the JSON input.
- The command writes only `/tmp/plots/plot.svg`; it writes nothing under the repository.

## Precision

Labels follow [the editorial guide](../editorial.md): **exact**, **precision-bounded** or
**parity**.

| Value | Label | Justification |
|---|---|---|
| `2`, `1`, `1`, `3` in `A` | **exact** | Integer matrix entries. |
| `5`, `6` in `b` | **exact** | Integer right-hand side. |
| `1`, `2`, `3` and `1`, `4`, `9` in `plot` | **exact** | Integer coordinates. |
| `5` in `result.display` and `"numerator": "5"` | **exact** | The determinant is the integer 5; denominator 1. |
| `1` in `"denominator": "1"` | **exact** | The integer denominator of the exact value. |

Every number in this run is **exact**; there are no precision-bounded or parity values. The boolean
`true` is a structured exactness flag, not a numeric measurement.

## Verification

Run this from any directory. It prints exactly the output above and writes the plot to
`/tmp/plots/plot.svg`.

```console
/tmp/rl-target/debug/lovelace-run --stdin --plot-dir /tmp/plots --omit-functions --omit-variables <<'EOF' | python3 -c 'import json, sys; d = json.load(sys.stdin); print(json.dumps({"display": d["result"]["display"], "kind": d["result"]["kind"], "exact": d["result"]["structured"]["exact"], "plot": d["artifacts"][0]["path"]}))'
A = [[2, 1], [1, 3]]
b = [[5], [6]]
sol = matmul(inv(A), b)
residual = matmul(A, sol)
path = plot([1, 2, 3], [1, 4, 9], "squares")
det(A)
EOF
```

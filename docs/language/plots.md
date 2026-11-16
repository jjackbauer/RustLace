# Plots

`plot` is the only builtin in the Language category. It renders a 2-D line plot to an SVG file
and returns the absolute path of the file as a `Text` value.

## Signature and forms

```text
plot(y)
plot(x, y)
plot(x, y, title)
```

| Form | Argument 1 | Argument 2 | Argument 3 |
|---|---|---|---|
| `plot(y)` | y values (Vector) | — | — |
| `plot(x, y)` | x values (Vector) | y values (Vector) | — |
| `plot(x, y, title)` | x values (Vector) | y values (Vector) | title (Text) |

The one-argument form uses `1..len(y)` as the x-axis. The two- and three-argument forms require
two vectors of the same length. An empty vector is refused, and a non-vector argument (including a
scalar, a symbolic expression, or a rank ≥ 2 array) is refused with a sentence naming the kind
that arrived.

```lovelace
type(plot([1, 2, 3]))
```
```text
= Text
  _ = Text
```

```lovelace
len(plot([1, 2, 3])) > 0
```
```text
= True (Boolean)
  _ = True
```

The second example shows the return contract without embedding a machine-specific path in the
comparison: the returned Text is non-empty. A real run also prints the path itself:

```text
= <plot-dir>/plot2.svg
  _ = <plot-dir>/plot2.svg
```

That text is the observed `--text` result of `plot([0, 1, 2, 3], [0, 1, 4, 9], "squares")` with
`--plot-dir <plot-dir> --plot-file plot2.svg`, run from the repository root; `<plot-dir>` stands for
the absolute directory passed to `--plot-dir` (this run passed `target/example-plots`). The exact
directory is host configuration; the language contract is "an absolute path to the file that was
written".

## Where the file goes

The engine has two plot-output settings:

- the **directory** (`Engine::plot_dir`, set by `--plot-dir <dir>`, default
  `<system temp>/lovelace-plots`);
- the **file name** (`Engine::plot_file`, set by `--plot-file <name>`, default `plot.svg`).

`plot` combines them, absolutises the result, normalises the separator to the host convention,
writes the SVG, and returns that absolute path. The runner creates a supplied `--plot-dir` before
evaluation; the builtin itself does **not** create a missing parent directory. A directory that
cannot be created is a caller-level `PlotDirectoryError` / `TypeMismatch`; a file that cannot be
written (missing parent, access denied, target is a directory) is a `PlotFileError` /
`TypeMismatch` whose sentence names the requested path.

The default directory is created when the engine is constructed, so `plot` works without any
runner flags under a normally writable temporary directory. In restricted environments, point
`--plot-dir` at a writable directory.

Each call overwrites the target file. A single evaluation can produce several plots; the engine
keeps the **last** plot as its captured plot, and that is the one reflected in the envelope's
`plot` object and `artifacts` array. The earlier files remain on disk.

## What the SVG contains

The renderer is an 800×600 SVG document:

```text
<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600" viewBox="0 0 800 600">
  <rect width="800" height="600" fill="white"/>
  <text x="400" y="28" text-anchor="middle" ...>squares</text>
  <rect x="70" y="50" width="700" height="495" fill="none" stroke="#cccccc"/>
  ... axes, tick marks, tick labels ...
  <polyline fill="none" stroke="#e6194b" stroke-width="2" points="..."/>
</svg>
```

Those lines are the observed head and curve element of the produced file, not a template. The
renderer:

- paints a white 800×600 background;
- emits a centred title `<text>` when the title is non-empty (an absent title emits no title
  element);
- draws the plot-area frame, x and y axes, axis tick marks, and numeric tick labels;
- samples the series into a dense polyline. Successive data points are connected with a natural
  cubic spline by default (the model's `CubicSpline` interpolation), so a coarse data set draws as
  a smooth curve rather than an angular polygon. A series with fewer than three points falls back
  to straight segments.

Coordinates are carried as `Real` values through model construction, bounds calculation, padding,
and normalisation; only the final pixel conversion introduces a decimal rendering, and no binary
floating point enters the model path. The tick labels are formatted numbers, while the plotted
coordinates are the model's decimal data. That is a statement about the representation, not a proof
that every interpolated sample is mathematically exact.

The observed sample `plot([0, 1, 2, 3], [0, 1, 4, 9], "squares")` wrote **15,263 bytes** to
`plot2.svg`. The size depends on the number of points and the labels, not on the path.

## The JSON envelope

On a successful plot, the JSON envelope contains a `plot` object and an `artifacts` array:

```json
{"bytes":15263,"kind":"plot","path":"<plot-dir>/plot2.svg","title":"squares"}
```

That is one observed artifact with the path shortened to the `<plot-dir>` placeholder
(pretty-printed here for readability; the wire is one line). The full envelope's `plot` object has
the same `path` and `title` plus the complete `svg` string:

```json
{"path":"<plot-dir>/plot2.svg","title":"squares","svg":"<svg xmlns=\"http://www.w3.org/2000/svg\" ..."}
```

The `result` object is the returned Text path: `kind: "Text"`, `display` and `typed` equal to the
path, and a structured Text payload. Because Text values render without a kind suffix in `--text`
mode, the human result line is just `= <path>`.

The artifact's `bytes` is measured from the file at envelope-construction time. If the file is
removed between the write and the envelope, `bytes` is omitted; the write itself has already
happened. A plot that fails to write produces no artifact and no `plot` object.

## Validation errors

A non-vector argument names the kind:

```lovelace
plot(1)
```
```text
Error [InvalidOperation/DomainError]: plot() argument 1 must be a vector, but got 'Natural'.
```

Two vectors must have the same length:

```lovelace
plot([1, 2], [1, 2, 3])
```
```text
Error [InvalidOperation/DomainError]: plot() vectors must have the same length (2 vs 3).
```

An empty series is refused rather than rendering an empty chart:

```lovelace
plot([])
```
```text
Error [InvalidOperation/DomainError]: plot() cannot plot an empty vector.
```

The title must be Text:

```lovelace
plot([1, 2], [1, 2], 42)
```
```text
Error [InvalidOperation/DomainError]: plot() title must be a string, but got 'Natural'.
```

Too many arguments is the registry's arity refusal:

```lovelace
plot([1, 2], [1, 2], "t", 4)
```
```text
Error [InvalidArgument/TypeMismatch]: plot(): expected 1 to 3 arguments; got 4.
```

A symbolic element or a symbolic series is refused because plot coordinates must be numeric:
`plot([x, 1])` or `plot(sin(x))` produces a `DomainError` naming the symbolic kind. Evaluate the
expression numerically first (`evalf`, or substitute values to build a Vector).

## Worked example

With a fixed output directory, the command

```console
target/debug/lovelace-run.exe --file plot.ls --text --plot-dir target/example-plots --plot-file plot2.svg
```

where `plot.ls` contains `plot([0, 1, 2, 3], [0, 1, 4, 9], "squares")`, wrote
`target/example-plots/plot2.svg` (15,263 bytes) and printed the absolute path to it. The SVG's
title is `squares`, its x ticks include `0`, `0.5`, `1`, …, and its curve is the cubic-spline
polyline through the four exact points. The file is a single self-contained SVG; no external
assets are referenced. The returned path is the path to that file on this host.

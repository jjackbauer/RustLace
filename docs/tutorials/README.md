# Tutorials

Four end-to-end tutorials for the **checked-out development revision** of RustLace.
These scripts are continuously checked against that revision; they are not a compatibility
promise for older downloadable releases. See [version applicability](../version-applicability.md)
when using a tagged binary. Each one starts from a problem, gives a complete
script, shows the recorded output (or its stable shape, where one value is host-specific), and ends
with the command that reproduces it.

Prerequisites: Rust 1.85+ with `cargo` on `PATH`; the first `cargo run` builds the runner. The
reproducible tutorial's Windows route uses Windows PowerShell 5.1 (shipped with Windows 10/11) or
PowerShell 7 and needs no Python or bash; its optional Linux/WSL bash selector needs Python 3.
`bash tools/run-tutorials.sh` needs bash (Git Bash on Windows) and no Python.

The commands run from the repository root and use the project-root Cargo invocation
`cargo run -p lovelace-run -- ...`; the first run compiles the runner. The tutorial commands are the
same in bash (Linux, WSL, macOS), Windows PowerShell and Command Prompt, because each tutorial's
script is a checked-in file under `docs/tutorials/examples/` rather than a shell here-document; the
reproducible tutorial adds one platform-specific field selector, documented on its page. No command
writes inside the repository: the reproducible tutorial's plot goes to the operating system's
temporary directory, and `--plot-dir <path>` redirects it (the CLI creates the directory).

The reproducible tutorial selects JSON fields. Its page documents a plain PowerShell
`ConvertFrom-Json` pipeline as the Windows route and a `python3` one-liner as the optional bash
route; Python 3 is a prerequisite for that one route only, and the page states it.

To check all four recorded outputs at once, from the repository root:

```console
bash tools/run-tutorials.sh
```

The runner executes each checked-in example, compares it with the page's recorded output, checks the
JSON fields and the plot artifact, recomputes the linear-algebra residual independently, and fails
when a page, an example or an expected value is missing or wrong. It asserts the set of four
tutorials by name, so a missing or unlisted tutorial is a failure rather than a skip. It is written
for Ubuntu and Windows (Git Bash), the shells the documentation job provisions, and falls back to
`cargo run -p lovelace-run` when no built runner is found.

## Choose a tutorial

New to the syntax, `Real`, `Vector`, or `Array`? Begin with
[Get started](../start-here.md). Each tutorial below assumes a small amount
of language knowledge and now includes a **types-and-APIs map** with links to
the normative reference. The examples are independent rather than a sequence;
choose the problem that matches what you want to accomplish. The
[library guide](../how-to/use-libraries.md) maps other common tasks to their
appropriate API families.

| Tutorial | What you do | What it teaches |
|---|---|---|
| [Linear algebra](linear-algebra.md) | Invert a 2 x 2 matrix, solve a system, and inspect the residual | Exact arithmetic versus a floating-point tolerance; why the right-hand side must be a column |
| [Symbolic](symbolic.md) | Differentiate, attempt to simplify, then evaluate at a chosen precision | When `simplify` refuses, what makes it fire, and where symbolic exactness ends |
| [Signal processing](signal-processing.md) | Smooth a synthetic signal, transform it with the FFT, and compare two precision budgets | The power-of-two FFT length rule and the effect of the DSP precision budget |
| [Reproducible](reproducible.md) | Run a script, generate a plot outside the repository, and extract JSON fields | The JSON envelope, `result.structured.exact`, and a file-based plot artifact |

## Precision

Every tutorial has the same seven sections, in this order: **Problem**, **Source**, **Output**,
**Reading the result**, **Assumptions**, **Precision**, **Verification**. The **Precision** section
labels each numeric value as **exact**, **precision-limited**, **error-bounded** or
**parity-verified**, following [the editorial guide](../editorial.md#the-four-kinds-of-claim).
Across the four pages, integer and terminating-rational results are exact, while `sqrt(2)` and the
irrational FFT tails computed at a stated number of places are precision-limited. No page publishes a
final-error bound, so none claims an error-bounded value; no page reproduces a reference
implementation's behaviour, so none claims a parity-verified value.

For the language beneath the examples, start at [the language guide](../language/README.md); for the
DSP, Symbolics and MathIR families, see [the extensions guide](../extensions/README.md).

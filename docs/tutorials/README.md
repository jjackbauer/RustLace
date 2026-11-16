# Tutorials

Four end-to-end tutorials for RustLace 0.4.1. Each one starts from a problem, gives a complete
script, shows output captured from a real run, and ends with one command that reproduces it.

The tutorials assume the CLI is already built. The verification commands use the debug runner at
`/tmp/rl-target/debug/lovelace-run` as built on the documenting host (WSL Ubuntu); substitute the
path to your own `lovelace-run` binary. None of the commands write into the repository.

| Tutorial | What you do | What it teaches |
|---|---|---|
| [Linear algebra](linear-algebra.md) | Invert a 2 x 2 matrix, solve a system, and inspect the residual | Exact arithmetic versus a floating-point tolerance; why the right-hand side must be a column |
| [Symbolic](symbolic.md) | Differentiate, attempt to simplify, then evaluate at a chosen precision | When `simplify` refuses, what makes it fire, and where symbolic exactness ends |
| [Signal processing](signal-processing.md) | Smooth a synthetic signal, transform it with the FFT, and compare two precision budgets | The power-of-two FFT length rule and the effect of the DSP precision budget |
| [Reproducible](reproducible.md) | Run a script, generate a plot outside the repository, and extract JSON fields | The JSON envelope, `result.structured.exact`, and a file-based plot artifact |

## Precision

Every tutorial has the same seven sections, in this order: **Problem**, **Source**, **Output**,
**Reading the result**, **Assumptions**, **Precision**, **Verification**. The **Precision** section
labels each numeric value as **exact**, **precision-bounded** or **parity**, following
[the editorial guide](../editorial.md).

For the language beneath the examples, start at [the language guide](../language/README.md); for the
DSP, Symbolics and MathIR families, see [the extensions guide](../extensions/README.md).

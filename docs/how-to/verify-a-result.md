# Verify a numerical result

Use this procedure when you need to decide whether a calculation is **exact**,
**precision-limited**, or actually supported by an independent mathematical
check. It works for a script run in `lovelace-run` or in the desktop
workbench.

**Prerequisites:** you have [run a Lovelace script](../start-here.md) and
understand the difference between a
[Real](../language/values-and-types.md#numeric-kinds-and-widening),
an [Array](../language/values-and-types.md#vector-and-array) and a
[Symbolic](../language/values-and-types.md#symbolic) value.
For the library-specific version of this workflow, see
[Working with libraries](use-libraries.md).

The runnable examples below display the compact `--text --omit-variables`
output. For example, save an expression to `example.ls` and run
`cargo run -p lovelace-run -- --file example.ls --text --omit-variables`.

## 1. Start with a result the representation can hold exactly

```lovelace
1 / 3
```
```text
= 0.(3) (Real)
```

The result has kind [Real](../language/values-and-types.md#numeric-kinds-and-widening),
but it is an exact repeating decimal. The parenthesized period `(3)` is
part of the representation, not a rounded display approximation. This is
why the numeric kind alone does not tell you whether a value is exact.
See [exactness](../language/values-and-types.md#exactness) and
[periodic decimals](../language/precision-and-formatting.md#periodic-rendering).

## 2. Inspect the provenance, not just the printed digits

```lovelace
[inspect(1/3).exact, inspect(sqrt(2)).exact]
```
```text
= [True, False] (Vector)
```

`inspect(...).exact` retrieves a Boolean-like provenance field from the
[introspection Record](../language/values-and-types.md#record).
The [Vector](../language/values-and-types.md#vector-and-array) contains the
two results: the exact periodic one-third and the precision-limited square
root of two. `sqrt(2)` cannot be represented as a finite decimal at a chosen
budget; its displayed digits are not themselves a proof of a final-error
bound.

To change the working precision explicitly, use `setprecision(n)` and
repeat the calculation. The result may gain stable-looking digits, but that
does **not** turn a computational budget into an error bound. Read the
[precision and formatting reference](../language/precision-and-formatting.md).

## 3. Check a mathematical invariant independently

**Task:** determine the rank of a matrix whose second row is twice the first.

```lovelace
matrix_rank([[1, 2, 3], [2, 4, 6]])
```
```text
= 1 (Integer)
```

The input is a rank-2 [Array](../language/values-and-types.md#vector-and-array)
of exact integers. The output is an
[Integer](../language/values-and-types.md#numeric-kinds-and-widening),
and the result can be checked without trusting the rank implementation:
the first row is nonzero and the second is a multiple of it, hence the
row space has dimension one. For larger rational matrices, compare against
an independent exact elimination algorithm.

The [linear-algebra tutorial](../tutorials/linear-algebra.md) shows another
useful invariant: after solving `A*x=b`, calculate the residual `A*x-b`
to check the proposed solution. A correct residual is strong evidence for
**that** solve; it is not blanket certification of all matrices, all
solvers or unrelated builtin functions.

For a symbolic matrix such as `[[x]]`, the rank depends on the unknown
condition `x=0`. Never present a generic symbolic estimate as a result
that holds for all parameter values. See
[linear-algebra limitations](../language/builtins.md#linear-algebra-8).

## 4. Record what was established

For a reproducible computation, retain the source revision or tagged release,
the input script, the exact invocation, the output kind, the structured
exactness/provenance fields, the precision settings and an independent check
when needed. When using JSON mode, read the
[structured result and diagnostic contract](../language/errors-and-diagnostics.md)
rather than scraping the human text output.

A regression test that reproduces the same output is a **parity check**. It
does not independently prove mathematical correctness. Similarly, an
`exact` marker describes the representation, not a proof that the algorithm
found the right value. If the engine lacks a certificate or final-error bound
for the operation, state that limitation.

For next steps, consult the [library guide](use-libraries.md) or the
[complete worked tutorials](../tutorials/README.md).

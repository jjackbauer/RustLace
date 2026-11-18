# How to verify a numerical result

**Goal:** compute a value and decide what you can legitimately claim about it.

## 1. First choose an exact question

For exact rational arithmetic:

```console
cargo run -p lovelace-run -- --eval "1/3" --text
```

The periodic `0.(3)` represents one third exactly. It is not a decimal
approximation. Check the value kind and structured projection through the
default JSON mode when writing a consumer.

## 2. Separate working precision from an error bound

```console
cargo run -p lovelace-run -- --eval "setprecision(30); sqrt(2)" --text
```

The finite decimal is a **precision-limited** computation. Thirty requested
places do not independently prove an error below 10^-30. Changing precision
can reveal differences in the tail; it is not an oracle.

## 3. Independently verify mathematical structure

For a linear system, recompute the residual `A*x-b` and check it against zero.
For matrix rank, compare against independent exact rational elimination where
the entries are exact. For a queueing model, check conservation, utilization
and applicable operational laws independently.

The [linear-algebra tutorial](../tutorials/linear-algebra.md) demonstrates an
exact residual check, and the [agent harness](../agent-harness.md) describes
where an independent oracle is already implemented.

## 4. Preserve evidence and limitations

Save the input script, source revision, output kind, structured exactness
information, precision settings and independent checks. A matching transcript
proves a regression has not changed that output; it does not, by itself,
establish mathematical truth. If the engine cannot provide a certificate or
error bound, report that limitation instead of quietly assuming one.

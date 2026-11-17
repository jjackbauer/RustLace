# M0 scientific workloads

These are the runnable M0 workload artifacts. The runner is
`lovelace-corpus/tests/workloads.rs`; run it with
`cargo test -p lovelace-corpus --test workloads`.

| File | Scenario | Evidence kind |
|---|---|---|
| `wl01_precision.ls` / `.expected` | Exact chained arithmetic, periodic identities, a 40-digit natural product, array scalar addition and 2^100 | Exact values; every expected line is a closed-form/mathematical identity |
| `wl02_exact_algebra.ls` / `.expected` | Symbolic simplification (`x + x`, `sin(x)^2 + cos(x)^2`), differentiation (`diff(x^3, x)`) and exact rational sums | Exact symbolic identities and exact periodic decimals |
| `workloads.rs::wl16_*` | Engine-loop, array-reduction and solver cancellation with a typed `Cancelled` error; saved-workspace atomicity is covered by `document_io`'s failure tests | Typed error contract; no host crash |

## Limitations

`WL-16` covers the **engine-loop**, **array-reduction** and **solver** cancellation paths, plus the
existing atomic saved-workspace failure tests. Streaming import is not implemented in this tree, so
there is no streaming-import interruption target; it is recorded as an absent capability rather
than a passing test.

`WL-01` exercises the exact engine arithmetic and array surface, not the M1 packed
`LReal64`/`LReal128` storage path (which is deferred). `WL-02` exercises the static symbolic
builtins, not a lowered/compiled MathIR kernel.

# `lovelace-array`

Generic N-dimensional arrays with row-major storage, shape algebra, elementwise operations and
reductions.

## Purpose

An `NdArray<T>` is a shape (dimension sizes, outermost first) plus a flat element vector in row-major
(C) order. A rank-0 array has an empty shape and exactly one element. Dimensions of length zero are
allowed, so an array can be empty while keeping its shape. The crate has no dependencies and does no
floating-point arithmetic; numeric operations are expressed through the `Field` trait, which is also
implemented for `i64` so generic algorithms can be tested with ordinary integers
(`lovelace-array/src/lib.rs:1`).

## Public surface

- `trait Field` — `zero`, `one`, `add`, `sub`, `mul`, `div`, `neg`, `is_zero`, `from_i64`.
- `NdArray<T>`: `from_vec`, `from_rows`, `scalar`, `zeros`, `ones`, `eye`, `shape`, `rank`, `numel`,
  `is_empty`, `is_scalar`, `strides`, `get`, `get_mut`, `as_slice`, `iter`, `map`, `zip_map`,
  `zip_map_broadcast`, `index_partial`, `slice(&[Slice])`, `reshape`, `transpose`, `transpose_axes`,
  `squeeze`, `flatten`, `concat`, `append`, `sum`, `prod`, `min`, `max`, `sum_axis`, `prod_axis`.
- `Slice`, `ArrayError` — partial indexing and the structured error type.
- `dense::{DenseArray, DType, Precision}` — the typed dense surface a host may project instead of a
  generic element type.
- `#![forbid(unsafe_code)]`.

## Dependencies

None.

## Tests

61 `#[test]` functions in `src/lib.rs` and `src/dense.rs`, covering shape algebra, broadcasting,
slicing, reductions and the typed dense projection.

## Using it

```rust
use lovelace_array::NdArray;

let m = NdArray::from_rows(vec![vec![1i64, 2], vec![3, 4]]).unwrap();
assert_eq!(m.shape(), &[2, 2]);
assert_eq!(m.transpose().as_slice(), &[1, 3, 2, 4]);
let d = m.as_slice();
assert_eq!(d[0] * d[3] - d[1] * d[2], -2);
```

The language's `det`, `matmul`, `inv`, `sum`, `reshape` and the rest are implemented over this crate
(`lovelace-lang/src/builtins.rs`), which is why the array operations are exact for integer matrices.

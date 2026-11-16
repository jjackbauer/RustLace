# `lovelace-array`

Generic N-dimensional arrays with row-major storage, shape algebra, elementwise operations and
reductions.

## Purpose

An `NdArray<T>` is a shape (dimension sizes, outermost first) plus a flat element vector in row-major
(C) order. A rank-0 array has an empty shape and exactly one element. Dimensions of length zero are
allowed, so an array can be empty while keeping its shape. The crate has no dependencies and does no
floating-point arithmetic; numeric operations are expressed through the `Field` trait, which is also
implemented for `i64` so generic algorithms can be tested with ordinary integers
(`lovelace-array/src/lib.rs`).

## Inputs

### Public surface

- `trait Field` — `zero`, `one`, `add`, `sub`, `mul`, `div`, `neg`, `is_zero`, `from_i64`.
- `NdArray<T>`: `from_vec`, `from_rows`, `scalar`, `zeros`, `ones`, `eye`, `shape`, `rank`, `numel`,
  `is_empty`, `is_scalar`, `strides`, `get`, `get_mut`, `as_slice`, `iter`, `map`, `zip_map`,
  `zip_map_broadcast`, `index_partial`, `slice(&[Slice])`, `reshape`, `transpose`, `transpose_axes`,
  `squeeze`, `flatten`, `concat`, `append`, `sum`, `prod`, `min`, `max`, `sum_axis`, `prod_axis`.
- `Slice`, `ArrayError` — partial indexing and the structured error type.
- `dense::{DenseArray, DType, Precision}` — the typed dense surface a host may project instead of a
  generic element type.
- `#![forbid(unsafe_code)]`.

### Dependencies

None.

## Outputs

`from_vec` and `from_rows` return `Result<NdArray<T>, ArrayError>` because a shape can disagree with
the data; `scalar`, `zeros`, `ones`, `eye`, `transpose`, `squeeze` and `flatten` return the array
directly. Accessors borrow: `shape` and `strides` return slices, `get`/`get_mut` return
`Option<&T>`/`Option<&mut T>`, and `as_slice`/`iter` expose the row-major storage. The structural
operations (`index_partial`, `slice`, `reshape`, `transpose_axes`, `concat`, `append`, `zip_map`,
`zip_map_broadcast`, `sum_axis`, `prod_axis`) return a new array or a `Result<NdArray<T>,
ArrayError>` when the request can fail. Scalar reductions (`sum`, `prod`) return `T`; `min`/`max`
return `Result<T, ArrayError>` because an empty array has no extremum. `map` returns
`NdArray<U>` for a new element type, and the `dense` module projects the same data into
`DenseArray`/`DType`/`Precision` for hosts that publish a typed surface.

## Precision behaviour

The crate performs no floating-point arithmetic and has no precision knob: an operation is exact with
respect to the element type. Numeric behaviour is delegated to `Field`, so a result is only as exact
as that implementation; the `i64` implementation uses Rust's truncating `/`, which means a generic
division can discard a remainder. Likewise, `Field::div` is the implementor's contract — the array
layer does not guard a zero divisor, and the `i64` implementation panics on one as Rust does. Storage
is plain row-major values, with no scale, rounding or display precision anywhere in the type.

## Errors

Failures are the `ArrayError` variants: `ShapeMismatch` (the product of the requested shape disagrees
with the data length, or the product overflows `usize`), `Ragged` (`from_rows` with rows of different
lengths), `IndexOutOfBounds` (a `get`-style index or a partial index outside its dimension),
`BadSlice` (a slice list of the wrong length, or a zero step), `EmptyReduction` (`min`/`max` of an
empty array) and `BadAxis` (an axis or an axis permutation that is invalid for the rank). `get` and
`get_mut` are the exception: they answer `None` for an out-of-range index rather than an error value.

## Limitations

- **No linear algebra and no numeric kernels.** `det`, `matmul`, `inv` and the rest live in
  `lovelace-lang`, not here; this crate supplies storage, shape algebra, indexing, slicing and
  reductions only.
- **No floating-point element type in the crate.** The generic surface is defined by `Field`, and the
  only implementation shipped here is for `i64`; a host that wants another element type implements
  the trait.
- **No views.** `slice` and `index_partial` return owned arrays, so a slice copies its elements even
  when the source is not mutated afterwards.
- **`zip_map` requires equal shapes.** Broadcasting exists only through `zip_map_broadcast`; the
  ordinary binary map rejects a shape disagreement with `ShapeMismatch`.
- **Division is unguarded.** The array layer never intercepts a zero divisor; the behaviour is the
  `Field` implementation's, and for `i64` it is Rust's panic.

## Examples

### Using it

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

### Tests

61 `#[test]` functions in `src/lib.rs` and `src/dense.rs`, covering shape algebra, broadcasting,
slicing, reductions and the typed dense projection.

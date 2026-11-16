# `lovelace-json`

A small, dependency-free JSON reader and writer — the workspace's only JSON implementation.

## Purpose

Every wire envelope, settings document, recovery header, corpus file and knowledge graph record is
JSON. The crate exists because the engine is registry-free (ADR-001). Numbers are kept as their
original source text: a field is either small enough to fit an integer or must be reproduced
byte-for-byte, and keeping the literal avoids a binary floating-point round trip
(`lovelace-json/src/lib.rs`).

## Inputs

### Public surface

- `enum Json { Null, Bool, Num(String), Str(String), Arr(Vec<Json>), Obj(BTreeMap<String, Json>) }`
  — objects are a `BTreeMap`, so their keys iterate and render in sorted key order; numbers keep
  their literal.
- Constructors: `Json::obj(pairs)`, `Json::str_array(items)`, `Json::num_array(items)`.
- Accessors: `get(key)`, `as_str()`, `as_arr()`.
- `Json::parse(&str) -> Result<Json, JsonError>` and `to_pretty()`.

### Dependencies

None. `lovelace-json` is a leaf: `Cargo.toml` declares no dependencies, and it is the base of the
`lovelace-run`, `lovelace-know`, `lovelace-corpus` and `lovelace-desktop` envelopes.

## Outputs

`parse` returns a `Json` tree; `to_pretty` renders it as indented JSON text. `get` returns
`Option<&Json>` for an object member, and `as_str`/`as_arr` return `Option<&str>`/`Option<&[Json]>`
for a value of that shape. `obj`, `str_array` and `num_array` build values: `obj` from key/value
pairs, `str_array` from any `Into<String>` items, and `num_array` from `i64` items, which it stores as
their decimal text. The enum is public, so a host can construct any value directly.

## Precision behaviour

Numbers are not interpreted: a `Num` holds the source text exactly (`"1.5"`, `"-2"`, `"1e400"`), and
`to_pretty` writes that text back unchanged. Nothing in this crate parses a number into a binary
float, rounds it or reformats it, so a document round-trips its literals byte-for-byte. The flip side
is that there is no numeric type here at all: a caller that needs the value parses the string with
the precision it chooses.

## Errors

`Json::parse` returns `Result<Json, JsonError>`, where `JsonError { offset, message }` carries the
**byte** offset of the malformed input (`Display` renders `JSON error at byte <offset>: <message>`).
The accessors do not fail the same way: `get` answers `None` for a non-object or a missing key, and
`as_str`/`as_arr` answer `None` for a value of another kind.

## Limitations

- **No floating-point number type.** A number is its source text; the crate never converts it, so it
  cannot tell `1.0` from `1` and cannot do arithmetic.
- **Object member order is key order.** Objects are a `BTreeMap`, so iteration and rendering are in
  sorted key order; the order a caller or a source document used is not retained.
- **One document, whole input.** `parse` takes a `&str` and reads a single value; there is no
  streaming reader and no incremental parser.
- **No schema or typed deserialisation.** A host reads members with `get`/`as_str`/`as_arr` and
  validates them itself; there are no derive macros, no `serde` and no error path beyond the byte
  offset.
- **JSON only.** There are no comments, no trailing commas and no JSON5 extensions in the parser.

## Examples

### Using it

```rust
use lovelace_json::Json;

let doc = Json::parse(r#"{"ok": true, "value": "0.(3)"}"#).expect("valid JSON");
assert_eq!(doc.get("value").and_then(Json::as_str), Some("0.(3)"));
```

The runner builds its envelope with `Json::obj`; because the object is a `BTreeMap`, its published
field order is sorted key order, not the order the code writes and not a hash order
(`lovelace-run/src/main.rs`, `lovelace-json/src/lib.rs`).

### Tests

Seven `#[test]` functions in `src/lib.rs` cover parsing, escaping, ordering and rendering.

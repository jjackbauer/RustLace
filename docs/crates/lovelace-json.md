# `lovelace-json`

A small, dependency-free JSON reader and writer — the workspace's only JSON implementation.

## Purpose

Every wire envelope, settings document, recovery header, corpus file and knowledge graph record is
JSON. The crate exists because the engine is registry-free (ADR-001). Numbers are kept as their
original source text: a field is either small enough to fit an integer or must be reproduced
byte-for-byte, and keeping the literal avoids a binary floating-point round trip
(`lovelace-json/src/lib.rs:1`).

## Public surface

- `enum Json { Null, Bool, Num(String), Str(String), Arr(Vec<Json>), Obj(BTreeMap<String, Json>) }`
  — objects preserve key order, numbers keep their literal.
- Constructors: `Json::obj(pairs)`, `Json::str_array(items)`, `Json::num_array(items)`.
- Accessors: `get(key)`, `as_str()`, `as_arr()`.
- `Json::parse(&str) -> Result<Json, JsonError>` and `to_pretty()`.

## Dependencies

None. `lovelace-json` is a leaf: `Cargo.toml` declares no dependencies, and it is the base of the
`lovelace-run`, `lovelace-know`, `lovelace-corpus` and `lovelace-desktop` envelopes.

## Tests

Seven `#[test]` functions in `src/lib.rs` cover parsing, escaping, ordering and rendering.

## Using it

```rust
use lovelace_json::Json;

let doc = Json::parse(r#"{"ok": true, "value": "0.(3)"}"#).expect("valid JSON");
assert_eq!(doc.get("value").and_then(Json::as_str), Some("0.(3)"));
```

The runner builds its envelope with `Json::obj` so the published field order is the order the code
writes, not a hash order (`lovelace-run/src/main.rs`).

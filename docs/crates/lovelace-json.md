# `lovelace-json`

A small, dependency-free, bounded JSON reader and writer — the workspace's only JSON implementation.

## Purpose

Every wire envelope, settings document, recovery header, corpus file and knowledge graph record is
JSON. The crate exists because the engine is registry-free (ADR-001). It implements the RFC 8259
grammar for values, numbers and strings, and numbers are kept as their original source text: a field
is either small enough to fit an integer or must be reproduced byte-for-byte, and keeping the
literal avoids a binary floating-point round trip (`lovelace-json/src/lib.rs`).

Parsing is bounded by default: an untrusted settings file, recovery header, plugin payload or wire
envelope cannot force unbounded recursion, allocation or CPU through the parser.

## Inputs

### Public surface

- `enum Json { Null, Bool, Num(String), Str(String), Arr(Vec<Json>), Obj(BTreeMap<String, Json>) }`
  — objects are a `BTreeMap`, so their keys iterate and render in sorted key order; numbers keep
  their literal.
- Constructors: `Json::obj(pairs)`, `Json::str_array(items)`, `Json::num_array(items)`.
- Accessors: `get(key)`, `as_str()`, `as_arr()`.
- `Json::parse(&str) -> Result<Json, JsonError>` — parse under `JsonLimits::default()`.
- `Json::parse_with_limits(&str, JsonLimits) -> Result<Json, JsonError>` — parse under explicit
  resource limits.
- `struct JsonLimits` — `max_bytes`, `max_depth`, `max_container_elements`,
  `max_total_elements` and `max_string_bytes`, all `usize`; `JsonLimits::default()` is the bounded
  default.
- `Json::to_pretty()` and the `Display` implementation for rendering.

### Default limits

| Limit | Default | Meaning |
|---|---:|---|
| `max_bytes` | 64 MiB | UTF-8 source bytes; checked before any parsing |
| `max_depth` | 128 | container nesting, with a scalar root at depth 0 |
| `max_container_elements` | 1,000,000 | elements in any one array or object |
| `max_total_elements` | 2,000,000 | array elements plus object members in the document |
| `max_string_bytes` | 16 MiB | decoded bytes of any one string or object key |

### Dependencies

None. `lovelace-json` is a leaf: `Cargo.toml` declares no dependencies, and it is the base of the
`lovelace-run`, `lovelace-know`, `lovelace-corpus` and `lovelace-desktop` envelopes.

## Outputs

`parse` returns a `Json` tree; `to_pretty` renders it as indented JSON text, and `Display` renders it
compactly. `get` returns `Option<&Json>` for an object member, and `as_str`/`as_arr` return
`Option<&str>`/`Option<&[Json]>` for a value of that shape. `obj`, `str_array` and `num_array` build
values: `obj` from key/value pairs, `str_array` from any `Into<String>` items, and `num_array` from
`i64` items, which it stores as their decimal text. The enum is public, so a host can construct any
value directly.

A successful parse preserves every numeric literal exactly (`"1.5"`, `"-2"`, `"1e400"`), decodes
valid `\u` escapes including surrogate pairs, and rejects malformed numbers, control characters,
lone surrogates and invalid escapes.

## Precision behaviour

Numbers are not interpreted: a `Num` holds the source text exactly (`"1.5"`, `"-2"`, `"1e400"`), and
`to_pretty` writes that text back unchanged. Nothing in this crate parses a number into a binary
float, rounds it or reformats it, so a document round-trips its literals byte-for-byte. The flip side
is that there is no numeric type here at all: a caller that needs the value parses the string with
the precision it chooses.

The resource limits are **exact** counts, not approximations: a document at a limit parses and one
over the limit returns a `Budget` error. They do not change a parsed value.

## Errors

`Json::parse` and `Json::parse_with_limits` return `Result<Json, JsonError>`, where
`JsonError { offset, kind, message }` carries the **byte** offset of the failure and a
`JsonErrorKind`:

- `JsonErrorKind::Syntax` — the input is not RFC 8259 JSON (for example `01`, `1e+`, an unescaped
  control character, a lone `\ud800` surrogate, or trailing characters).
- `JsonErrorKind::Budget` — the input is measurable but exceeds a `JsonLimits` field. The message
  names the limit, and `offset` is where the violation was detected (0 for `max_bytes`).

`JsonError::is_budget()` is the convenience test. `Display` renders
`JSON error at byte <offset>: <message>`. The accessors do not fail the same way: `get` answers
`None` for a non-object or a missing key, and `as_str`/`as_arr` answer `None` for a value of another
kind.

## Limitations

- **No floating-point number type.** A number is its source text; the crate never converts it, so it
  cannot tell `1.0` from `1` and cannot do arithmetic.
- **Object member order is key order.** Objects are a `BTreeMap`, so iteration and rendering are in
  sorted key order; the order a caller or a source document used is not retained.
- **Duplicate keys: last member wins.** A repeated object name replaces the earlier value because
  the map has one slot per key. Duplicate names are still counted against both element limits.
- **One document, whole input.** `parse` takes a `&str` and reads a single value; there is no
  streaming reader and no incremental parser.
- **No schema or typed deserialisation.** A host reads members with `get`/`as_str`/`as_arr` and
  validates them itself; there are no derive macros, no `serde` and no typed error path beyond the
  kind and byte offset.
- **JSON only.** There are no comments, no trailing commas and no JSON5 extensions in the parser.
- **The writer does not enforce the parser limits.** `to_pretty` and `Display` walk a `Json` tree
  recursively and can build arbitrarily large output; they assume a tree that was either produced by
  a bounded parse or constructed within the host's own budget.

## Examples

### Parsing under explicit limits

```rust
use lovelace_json::{Json, JsonErrorKind, JsonLimits};

let limits = JsonLimits {
    max_depth: 2,
    ..JsonLimits::default()
};
let doc = Json::parse_with_limits(r#"{"ok": true, "value": "0.(3)"}"#, limits)
    .expect("valid JSON");
assert_eq!(doc.get("value").and_then(Json::as_str), Some("0.(3)"));

let err = Json::parse_with_limits("[[[0]]]", limits).expect_err("too deep");
assert_eq!(err.kind, JsonErrorKind::Budget);
assert!(err.is_budget());
```

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

Twenty `#[test]` functions in `src/lib.rs` cover parsing, escaping, strict number and Unicode
rejection, each resource-limit boundary, serialisation round-trips and a deterministic 20,000-case
json-like fuzz campaign.

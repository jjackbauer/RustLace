//! A small, dependency-free JSON reader and writer.
//!
//! The workspace builds with no external crates (the build host cannot reach a crate
//! registry), so the wire protocol's JSON is produced and consumed here. Numbers are kept
//! as their original text: every numeric field the protocol publishes is either small enough
//! to fit an integer or must be reproduced byte-for-byte, and keeping the literal avoids a
//! binary floating-point round trip.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// A JSON value, preserving object key order and numeric literals as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Json {
    /// `null`
    Null,
    /// `true` / `false`
    Bool(bool),
    /// A number, kept as its source text (e.g. `"1.5"`, `"-2"`).
    Num(String),
    /// A string.
    Str(String),
    /// An array.
    Arr(Vec<Json>),
    /// An object; insertion ordered so published field order is stable.
    Obj(BTreeMap<String, Json>),
}

impl Json {
    /// Builds an object from (key, value) pairs, preserving the given order.
    pub fn obj<I, K>(pairs: I) -> Json
    where
        I: IntoIterator<Item = (K, Json)>,
        K: Into<String>,
    {
        let mut m = BTreeMap::new();
        for (k, v) in pairs {
            m.insert(k.into(), v);
        }
        Json::Obj(m)
    }

    /// Builds a string array.
    pub fn str_array<I, S>(items: I) -> Json
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Json::Arr(items.into_iter().map(|s| Json::Str(s.into())).collect())
    }

    /// Builds an array of numbers from integers.
    pub fn num_array<I>(items: I) -> Json
    where
        I: IntoIterator<Item = i64>,
    {
        Json::Arr(
            items
                .into_iter()
                .map(|n| Json::Num(n.to_string()))
                .collect(),
        )
    }

    /// Returns the value for a key when this is an object.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(m) => m.get(key),
            _ => None,
        }
    }

    /// Returns the string payload, if this is a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Returns the elements, if this is an array.
    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a.as_slice()),
            _ => None,
        }
    }

    /// Serialises with two-space indentation (used by fixtures and human-facing output).
    pub fn to_pretty(&self) -> String {
        let mut out = String::new();
        self.write_pretty(&mut out, 0);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Num(n) => out.push_str(n),
            Json::Str(s) => write_escaped(out, s),
            Json::Arr(items) => {
                out.push('[');
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write(out);
                }
                out.push(']');
            }
            Json::Obj(map) => {
                out.push('{');
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_escaped(out, k);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }

    fn write_pretty(&self, out: &mut String, indent: usize) {
        let pad = |out: &mut String, n: usize| {
            for _ in 0..n {
                out.push_str("  ");
            }
        };
        match self {
            Json::Arr(items) if !items.is_empty() => {
                out.push_str("[\n");
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(",\n");
                    }
                    pad(out, indent + 1);
                    v.write_pretty(out, indent + 1);
                }
                out.push('\n');
                pad(out, indent);
                out.push(']');
            }
            Json::Obj(map) if !map.is_empty() => {
                out.push_str("{\n");
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 {
                        out.push_str(",\n");
                    }
                    pad(out, indent + 1);
                    write_escaped(out, k);
                    out.push_str(": ");
                    v.write_pretty(out, indent + 1);
                }
                out.push('\n');
                pad(out, indent);
                out.push('}');
            }
            other => other.write(out),
        }
    }

    /// Parses a JSON document with the default [`JsonLimits`].
    ///
    /// Returns an error with a byte offset on malformed input, and an error whose
    /// [`kind`](JsonError::kind) is [`JsonErrorKind::Budget`] when a limit is exceeded.
    pub fn parse(input: &str) -> Result<Json, JsonError> {
        Json::parse_with_limits(input, JsonLimits::default())
    }

    /// Parses a JSON document under explicit resource limits.
    ///
    /// The parser never recurses deeper than `limits.max_depth`, never retains more
    /// than `limits.max_total_elements` container elements, and never builds a string
    /// longer than `limits.max_string_bytes`. Input larger than `limits.max_bytes` is
    /// rejected before any parsing work starts.
    pub fn parse_with_limits(input: &str, limits: JsonLimits) -> Result<Json, JsonError> {
        if input.len() > limits.max_bytes {
            return Err(JsonError {
                offset: 0,
                kind: JsonErrorKind::Budget,
                message: format!(
                    "input is {} bytes, over the {} byte limit",
                    input.len(),
                    limits.max_bytes
                ),
            });
        }
        let mut p = Parser {
            bytes: input.as_bytes(),
            pos: 0,
            limits,
            total_elements: 0,
        };
        p.skip_ws();
        let v = p.value(0)?;
        p.skip_ws();
        if p.pos != p.bytes.len() {
            return Err(JsonError {
                offset: p.pos,
                kind: JsonErrorKind::Syntax,
                message: "trailing characters".into(),
            });
        }
        Ok(v)
    }
}

/// Hard limits applied while parsing a potentially untrusted JSON document.
///
/// [`Json::parse`] uses [`JsonLimits::default`]. A host that genuinely needs a larger
/// trusted document can call [`Json::parse_with_limits`] with explicit limits; the
/// defaults are sized for wire envelopes, settings and corpus files, not for
/// multi-gigabyte data interchange.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JsonLimits {
    /// Maximum size of the UTF-8 source in bytes.
    pub max_bytes: usize,
    /// Maximum container nesting depth. A document with no container has depth 0.
    pub max_depth: usize,
    /// Maximum number of elements in any single array or object.
    pub max_container_elements: usize,
    /// Maximum number of array elements plus object members in the whole document.
    pub max_total_elements: usize,
    /// Maximum decoded length in bytes of any single string, including object keys.
    pub max_string_bytes: usize,
}

impl Default for JsonLimits {
    fn default() -> Self {
        JsonLimits {
            max_bytes: 64 * 1024 * 1024,
            max_depth: 128,
            max_container_elements: 1_000_000,
            max_total_elements: 2_000_000,
            max_string_bytes: 16 * 1024 * 1024,
        }
    }
}

/// Why a JSON document was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonErrorKind {
    /// The input is not grammatically valid JSON.
    Syntax,
    /// The input is valid enough to measure but exceeds a [`JsonLimits`] budget.
    Budget,
}

impl std::fmt::Display for Json {
    /// Serialises compactly. Objects render keys in map order.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut out = String::new();
        self.write(&mut out);
        f.write_str(&out)
    }
}

/// A parse failure with the byte offset at which it was detected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonError {
    /// Byte offset into the source.
    pub offset: usize,
    /// Whether the input was malformed or over budget.
    pub kind: JsonErrorKind,
    /// Human-readable description.
    pub message: String,
}

impl JsonError {
    /// Returns `true` when the failure is a [`JsonLimits`] budget refusal.
    pub fn is_budget(&self) -> bool {
        self.kind == JsonErrorKind::Budget
    }
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON error at byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for JsonError {}

fn write_escaped(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            // Non-ASCII is escaped as \uXXXX to match the wire contract published by the C#
            // runner (System.Text.Json escapes by default); surrogate pairs are emitted for
            // astral characters so the output is valid JSON for any decoder.
            c if (c as u32) > 0x7e => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    let _ = write!(out, "\\u{:04x}", unit);
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    limits: JsonLimits,
    total_elements: usize,
}

impl<'a> Parser<'a> {
    fn err<T>(&self, msg: &str) -> Result<T, JsonError> {
        Err(JsonError {
            offset: self.pos,
            kind: JsonErrorKind::Syntax,
            message: msg.to_string(),
        })
    }

    fn budget<T>(&self, msg: String) -> Result<T, JsonError> {
        Err(JsonError {
            offset: self.pos,
            kind: JsonErrorKind::Budget,
            message: msg,
        })
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// Refuses a container that would exceed `max_depth`.
    fn enter(&self, depth: usize) -> Result<(), JsonError> {
        if depth >= self.limits.max_depth {
            return self.budget(format!(
                "nesting exceeds the {} level limit",
                self.limits.max_depth
            ));
        }
        Ok(())
    }

    /// Accounts for the next element of a container before it is parsed.
    fn account_element(&mut self, index: usize, container: &str) -> Result<(), JsonError> {
        if index >= self.limits.max_container_elements {
            return self.budget(format!(
                "{container} exceeds the {} element limit",
                self.limits.max_container_elements
            ));
        }
        if self.total_elements >= self.limits.max_total_elements {
            return self.budget(format!(
                "document exceeds the {} element limit",
                self.limits.max_total_elements
            ));
        }
        self.total_elements += 1;
        Ok(())
    }

    /// Appends one decoded character, enforcing `max_string_bytes` on the decoded form.
    fn push_char(&self, out: &mut String, ch: char) -> Result<(), JsonError> {
        let decoded = out
            .len()
            .checked_add(ch.len_utf8())
            .ok_or_else(|| JsonError {
                offset: self.pos,
                kind: JsonErrorKind::Budget,
                message: "string length overflowed".into(),
            })?;
        if decoded > self.limits.max_string_bytes {
            return self.budget(format!(
                "string exceeds the {} byte limit",
                self.limits.max_string_bytes
            ));
        }
        out.push(ch);
        Ok(())
    }

    /// Appends decoded UTF-8, enforcing `max_string_bytes` on the decoded form.
    fn push_str(&self, out: &mut String, s: &str) -> Result<(), JsonError> {
        let decoded = out.len().checked_add(s.len()).ok_or_else(|| JsonError {
            offset: self.pos,
            kind: JsonErrorKind::Budget,
            message: "string length overflowed".into(),
        })?;
        if decoded > self.limits.max_string_bytes {
            return self.budget(format!(
                "string exceeds the {} byte limit",
                self.limits.max_string_bytes
            ));
        }
        out.push_str(s);
        Ok(())
    }

    fn value(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => {
                self.enter(depth)?;
                self.object(depth + 1)
            }
            Some(b'[') => {
                self.enter(depth)?;
                self.array(depth + 1)
            }
            Some(b'"') => self.string().map(Json::Str),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b'-') | Some(b'0'..=b'9') => self.number(),
            _ => self.err("expected a value"),
        }
    }

    fn literal(&mut self, text: &str, value: Json) -> Result<Json, JsonError> {
        if self.bytes[self.pos..].starts_with(text.as_bytes()) {
            self.pos += text.len();
            Ok(value)
        } else {
            self.err("invalid literal")
        }
    }

    /// Parses the RFC 8259 number grammar:
    ///
    /// ```text
    /// number = [ "-" ] ( "0" / digit1-9 *DIGIT ) [ "." 1*DIGIT ] [ ( "e" / "E" ) [ "+" / "-" ] 1*DIGIT ]
    /// ```
    ///
    /// The source text is preserved; no binary floating-point conversion happens here.
    fn number(&mut self) -> Result<Json, JsonError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return self.err("invalid number: expected a digit after '-'");
            }
        }
        match self.peek() {
            Some(b'0') => {
                self.pos += 1;
                if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return self.err("invalid number: leading zeros are not allowed");
                }
            }
            Some(b'1'..=b'9') => {
                self.pos += 1;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            _ => return self.err("invalid number: expected a digit"),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return self.err("invalid number: expected a digit after '.'");
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return self.err("invalid number: expected an exponent digit");
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        // All consumed bytes are ASCII, so this slice is valid UTF-8.
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| JsonError {
                offset: start,
                kind: JsonErrorKind::Syntax,
                message: "invalid utf-8 in number".into(),
            })?
            .to_string();
        Ok(Json::Num(text))
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.pos += 1; // opening quote
        let mut out = String::new();
        loop {
            let b = match self.peek() {
                Some(b) => b,
                None => return self.err("unterminated string"),
            };
            self.pos += 1;
            match b {
                b'"' => return Ok(out),
                b'\\' => {
                    let esc = match self.peek() {
                        Some(e) => e,
                        None => return self.err("unterminated escape"),
                    };
                    self.pos += 1;
                    let ch = match esc {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let hi = self.hex4()?;
                            match hi {
                                0xd800..=0xdbff => {
                                    if self.peek() != Some(b'\\') {
                                        return self.err("lone high surrogate in \\u escape");
                                    }
                                    self.pos += 1;
                                    if self.peek() != Some(b'u') {
                                        return self.err(
                                            "high surrogate is not followed by a low surrogate",
                                        );
                                    }
                                    self.pos += 1;
                                    let lo = self.hex4()?;
                                    if !(0xdc00..=0xdfff).contains(&lo) {
                                        return self.err(
                                            "high surrogate is not followed by a low surrogate",
                                        );
                                    }
                                    let scalar = 0x1_0000
                                        + ((hi as u32 - 0xd800) << 10)
                                        + (lo as u32 - 0xdc00);
                                    match char::from_u32(scalar) {
                                        Some(ch) => ch,
                                        None => {
                                            return self
                                                .err("invalid surrogate pair in \\u escape");
                                        }
                                    }
                                }
                                0xdc00..=0xdfff => {
                                    return self.err("lone low surrogate in \\u escape");
                                }
                                _ => match char::from_u32(hi as u32) {
                                    Some(ch) => ch,
                                    None => return self.err("invalid \\u escape"),
                                },
                            }
                        }
                        _ => return self.err("invalid escape"),
                    };
                    self.push_char(&mut out, ch)?;
                }
                b if b < 0x20 => {
                    return self.err("unescaped control character in string");
                }
                _ => {
                    let start = self.pos - 1;
                    let end = start + utf8_len(b);
                    let bytes = match self.bytes.get(start..end) {
                        Some(bytes) => bytes,
                        None => return self.err("invalid utf-8 in string"),
                    };
                    let s = match std::str::from_utf8(bytes) {
                        Ok(s) => s,
                        Err(_) => return self.err("invalid utf-8 in string"),
                    };
                    self.push_str(&mut out, s)?;
                    self.pos = end;
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u16, JsonError> {
        let mut v: u16 = 0;
        for _ in 0..4 {
            let b = match self.peek() {
                Some(b) => b,
                None => return self.err("truncated \\u escape"),
            };
            self.pos += 1;
            let d = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => return self.err("invalid hex digit"),
            };
            v = (v << 4) | d as u16;
        }
        Ok(v)
    }

    fn array(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            self.account_element(items.len(), "array")?;
            items.push(self.value(depth)?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Arr(items));
                }
                _ => return self.err("expected ',' or ']'"),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.pos += 1;
        let mut map = BTreeMap::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Json::Obj(map));
        }
        let mut members = 0;
        loop {
            self.account_element(members, "object")?;
            members += 1;
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return self.err("expected object key");
            }
            let key = self.string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return self.err("expected ':'");
            }
            self.pos += 1;
            let v = self.value(depth)?;
            map.insert(key, v);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Obj(map));
                }
                _ => return self.err("expected ',' or '}'"),
            }
        }
    }
}

fn utf8_len(b: u8) -> usize {
    match b {
        0x00..=0x7f => 1,
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        // No valid UTF-8 character starts with a continuation or overlong lead byte;
        // the caller's 1-byte slice then fails `from_utf8`.
        _ => 1,
    }
}

#[cfg(test)]
// The test names below mirror the C# census method names one-for-one; parity/manifest-map.json
// keys on the exact lovelace-json::tests::Parse_... path, so renaming would break gate-manifest.
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn Parse_GivenFlatObject_PreservesFields() {
        let v = Json::parse(r#"{"a": 1, "b": "x"}"#).unwrap();
        assert_eq!(v.get("a"), Some(&Json::Num("1".into())));
        assert_eq!(v.get("b").and_then(|x| x.as_str()), Some("x"));
    }

    #[test]
    fn Parse_GivenArrayOfObjects_ReadsEach() {
        let v = Json::parse(r#"[{"id":"lang/001"},{"id":"lang/002"}]"#).unwrap();
        let arr = v.as_arr().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[1].get("id").and_then(|x| x.as_str()), Some("lang/002"));
    }

    #[test]
    fn Parse_GivenEscapes_DecodesUnicode() {
        let v = Json::parse(r#""café µ""#).unwrap();
        assert_eq!(v.as_str(), Some("café µ"));
    }

    #[test]
    fn Parse_GivenAstralSurrogatePair_DecodesOneChar() {
        let v = Json::parse(r#""😀""#).unwrap();
        assert_eq!(v.as_str().unwrap().chars().count(), 1);
    }

    #[test]
    fn Write_GivenNonAscii_EscapesAsUnicode() {
        let j = Json::Str("µ".into());
        assert_eq!(j.to_string(), r#""\u00b5""#);
    }

    #[test]
    fn RoundTrip_GivenCorpusShapedRecord_IsStable() {
        let src = r#"{"id":"lang/003","script":"1/3","kind":"value","text":"0.(3) (Real)"}"#;
        let v = Json::parse(src).unwrap();
        let again = Json::parse(&v.to_string()).unwrap();
        assert_eq!(v, again);
    }

    #[test]
    fn Parse_GivenTrailingGarbage_ReturnsError() {
        assert!(Json::parse("{} x").is_err());
    }

    #[test]
    fn parse_rejects_non_json_number_forms() {
        for src in [
            "01", "-01", "1--2", "1e", "1e+", "1e-", "1.", "1e5.5", "00", "-",
        ] {
            assert!(
                Json::parse(src).is_err(),
                "accepted an invalid JSON number: {src:?}"
            );
        }
    }

    #[test]
    fn parse_rejects_malformed_unicode_escapes() {
        for src in [
            r#""\ud800""#,
            r#""\udc00""#,
            r#""\ud800\u0041""#,
            r#""\ud800x""#,
            r#""\u12""#,
        ] {
            assert!(
                Json::parse(src).is_err(),
                "accepted an invalid Unicode escape: {src:?}"
            );
        }
    }

    #[test]
    fn parse_rejects_unescaped_control_characters() {
        assert!(Json::parse("\"a\u{1}b\"").is_err());
        for byte in 0u8..0x20 {
            let src = format!("\"a{}b\"", byte as char);
            assert!(
                Json::parse(&src).is_err(),
                "accepted unescaped control byte 0x{byte:02x}"
            );
        }
    }

    #[test]
    fn parse_accepts_every_valid_number_form() {
        for src in [
            "0",
            "-0",
            "1",
            "-1",
            "1.5",
            "0.5",
            "-0.5",
            "1e10",
            "1E10",
            "1e+10",
            "1e-10",
            "-1.5e+3",
            "0e0",
            "-0.0e-0",
            "123456789012345678901234567890",
            "1e400",
        ] {
            assert_eq!(
                Json::parse(src),
                Ok(Json::Num(src.to_string())),
                "rejected a valid JSON number: {src:?}"
            );
        }
        // A valid number stops at a JSON delimiter rather than swallowing it.
        assert_eq!(
            Json::parse(r#"{"n":1e+2,"m":-0.5}"#).unwrap().get("n"),
            Some(&Json::Num("1e+2".into()))
        );
    }

    #[test]
    fn parse_decodes_every_valid_escape() {
        let short = Json::parse(r#""\"\\\/\b\f\n\r\t""#).unwrap();
        assert_eq!(short.as_str().unwrap(), "\"\\/\u{8}\u{c}\n\r\t");

        let nul = Json::parse(r#""\u0000""#).unwrap();
        assert_eq!(nul.as_str().unwrap(), "\0");

        let bmp = Json::parse(r#""\u00b5""#).unwrap();
        assert_eq!(bmp.as_str().unwrap(), "µ");

        let max = Json::parse(r#""\uffff""#).unwrap();
        assert_eq!(max.as_str().unwrap(), "\u{ffff}");

        let pair = Json::parse(r#""\ud83d\ude00""#).unwrap();
        assert_eq!(pair.as_str().unwrap(), "😀");
        assert_eq!(pair.as_str().unwrap().chars().count(), 1);
    }

    #[test]
    fn syntax_errors_are_not_budget_errors() {
        let err = Json::parse("[1,]").unwrap_err();
        assert_eq!(err.kind, JsonErrorKind::Syntax);
        assert!(!err.is_budget());
    }

    #[test]
    fn byte_limit_rejects_before_parsing() {
        let limits = JsonLimits {
            max_bytes: 4,
            ..JsonLimits::default()
        };
        let err = Json::parse_with_limits("[1,2]", limits).unwrap_err();
        assert!(err.is_budget());
        assert_eq!(err.offset, 0);
        assert!(err.message.contains("5 bytes"), "{}", err.message);

        let limits = JsonLimits {
            max_bytes: 5,
            ..JsonLimits::default()
        };
        assert_eq!(
            Json::parse_with_limits("[1,2]", limits),
            Ok(Json::Arr(vec![
                Json::Num("1".into()),
                Json::Num("2".into())
            ]))
        );
    }

    #[test]
    fn depth_limit_is_exact_and_typed() {
        let limits = JsonLimits {
            max_depth: 2,
            ..JsonLimits::default()
        };
        assert!(Json::parse_with_limits("[[0]]", limits).is_ok());

        let err = Json::parse_with_limits("[[[0]]]", limits).unwrap_err();
        assert!(err.is_budget());
        assert_eq!(err.offset, 2);
    }

    #[test]
    fn default_depth_limit_refuses_adversarial_nesting() {
        let deep = "[".repeat(100_000);
        let err = Json::parse(&deep).unwrap_err();
        assert!(err.is_budget());
        assert_eq!(err.offset, JsonLimits::default().max_depth);
    }

    #[test]
    fn container_and_total_element_limits_are_typed() {
        let per_container = JsonLimits {
            max_container_elements: 2,
            ..JsonLimits::default()
        };
        assert!(Json::parse_with_limits("[1,2]", per_container).is_ok());

        let err = Json::parse_with_limits("[1,2,3]", per_container).unwrap_err();
        assert!(err.is_budget());
        assert_eq!(err.offset, 5);

        // Duplicate keys count as members even though the map keeps only one entry.
        let err = Json::parse_with_limits(r#"{"a":1,"a":2,"a":3}"#, per_container).unwrap_err();
        assert!(err.is_budget());
        // The third member is refused where it starts, after the previous comma.
        assert_eq!(err.offset, 13);

        let total = JsonLimits {
            max_total_elements: 3,
            ..JsonLimits::default()
        };
        let err = Json::parse_with_limits("[[1,2],[3,4]]", total).unwrap_err();
        assert!(err.is_budget());
        assert_eq!(err.offset, 7);
    }

    #[test]
    fn string_limit_counts_decoded_bytes() {
        let limits = JsonLimits {
            max_string_bytes: 3,
            ..JsonLimits::default()
        };
        assert_eq!(
            Json::parse_with_limits(r#""abc""#, limits)
                .unwrap()
                .as_str(),
            Some("abc")
        );

        let err = Json::parse_with_limits(r#""abcd""#, limits).unwrap_err();
        assert!(err.is_budget());
        assert_eq!(err.offset, 5);

        // Six source bytes decode to one byte each; the fourth exceeds the budget.
        let err = Json::parse_with_limits(r#""\u0041\u0042\u0043\u0044""#, limits).unwrap_err();
        assert!(err.is_budget());
        assert_eq!(err.offset, 25);

        let err = Json::parse_with_limits(r#"{"abcd":1}"#, limits).unwrap_err();
        assert!(err.is_budget());
        assert_eq!(err.offset, 6);
    }

    /// A tiny deterministic generator: no dependencies, reproducible failure cases.
    struct XorShift64(u64);

    impl XorShift64 {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    fn random_scalar(rng: &mut XorShift64) -> Json {
        match rng.below(4) {
            0 => Json::Null,
            1 => Json::Bool(rng.below(2) == 0),
            2 => {
                const NUMBERS: &[&str] = &[
                    "0",
                    "-0",
                    "1",
                    "-1",
                    "42",
                    "-3.5",
                    "0.125",
                    "1e10",
                    "-1.25E-7",
                    "123456789012345678901234567890",
                ];
                Json::Num(NUMBERS[rng.below(NUMBERS.len())].to_string())
            }
            _ => {
                const CHARS: &[char] = &[
                    'a', 'Z', '0', ' ', '"', '\\', '\n', '\t', '\0', '\u{7f}', 'µ', 'é', '😀',
                ];
                let len = rng.below(12);
                Json::Str((0..len).map(|_| CHARS[rng.below(CHARS.len())]).collect())
            }
        }
    }

    fn random_json(rng: &mut XorShift64, depth: usize) -> Json {
        if depth >= 4 {
            return random_scalar(rng);
        }
        match rng.below(6) {
            0..=3 => random_scalar(rng),
            4 => Json::Arr(
                (0..rng.below(4))
                    .map(|_| random_json(rng, depth + 1))
                    .collect(),
            ),
            _ => Json::obj(
                (0..rng.below(4))
                    .map(|i| (format!("k{i}"), random_json(rng, depth + 1)))
                    .collect::<Vec<_>>(),
            ),
        }
    }

    #[test]
    fn property_serialised_json_round_trips() {
        let mut rng = XorShift64(0x5eed_2024_1115_0001);
        for _ in 0..2_000 {
            let value = random_json(&mut rng, 0);
            let compact = value.to_string();
            let parsed = Json::parse(&compact).expect("serialised JSON parses");
            assert_eq!(parsed, value);
            let pretty = value.to_pretty();
            assert_eq!(Json::parse(&pretty).unwrap(), value);
        }
    }

    #[test]
    fn fuzz_random_json_like_input_never_panics() {
        const FRAGMENTS: &[&str] = &[
            "{", "}", "[", "]", "\"", "\\", ":", ",", "0", "1", "9", "e", "E", "+", "-", ".",
            "true", "false", "null", " ", "\n", "\t", "\r", "\\u", "\\ud800", "\\udc00", "\\ud83d",
            "\\ude00", "\\u0041", "\\n", "\\\"", "\\\\", "é", "😀", "\u{0}", "\u{1}", "\u{7f}",
            "x", "a",
        ];
        // The campaign's validity oracle. An invalid-accepting parser must fail this
        // test itself, not merely a separate targeted unit test.
        const INVALID_DOCUMENTS: &[&str] = &[
            "01",
            "-01",
            "00",
            "1--2",
            "1e+",
            "1e-",
            "1.",
            "1e5.5",
            "+1",
            "-",
            "01.2",
            ".5",
            "[01]",
            r#"{"a":01}"#,
            "[1,]",
            r#"{"a":1,}"#,
            "\"unterminated",
            r#""\ud800""#,
            r#""\udc00""#,
            r#""\ud800\u0041""#,
            r#""\u12""#,
            "\"a\u{1}b\"",
            "tru",
            "nul",
        ];
        for src in INVALID_DOCUMENTS {
            match Json::parse(src) {
                Ok(value) => {
                    panic!("campaign oracle accepted invalid document {src:?} as {value:?}")
                }
                Err(error) => assert_eq!(
                    error.kind,
                    JsonErrorKind::Syntax,
                    "campaign oracle expected a syntax error for {src:?}, got {error:?}"
                ),
            }
        }
        let mut rng = XorShift64(0x0ddc_0ffe_e202_6111);
        let mut accepted = 0usize;
        let mut rejected = 0usize;
        for _ in 0..20_000 {
            let mut src = String::new();
            for _ in 0..rng.below(40) {
                src.push_str(FRAGMENTS[rng.below(FRAGMENTS.len())]);
            }
            match Json::parse(&src) {
                Ok(value) => {
                    accepted += 1;
                    let compact = value.to_string();
                    let reparsed =
                        Json::parse(&compact).expect("valid JSON re-serialises to valid JSON");
                    assert_eq!(reparsed, value);
                }
                Err(_) => rejected += 1,
            }
        }
        assert!(
            accepted > 100,
            "only {accepted} random inputs parsed; the campaign is not exercising the parser"
        );
        assert!(
            rejected > 100,
            "only {rejected} random inputs were rejected; the campaign has no rejection signal"
        );
    }
}

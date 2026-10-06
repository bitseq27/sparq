//! A minimal, dependency-free JSON reader — exactly the subset the machine-written artefacts use.
//!
//! Why a hand-rolled parser (the `sparq-module-api::toml` precedent): the shell core is
//! zero-third-party by contract (crate docs), and INC4's display-list painter must read two
//! machine-written JSON faces at paint/load time — the D13 display-list interchange (written by the
//! Observatory harness, and by `sparq mod validate` stage 6 on the device) and, in tests, the token
//! bundle. Pulling serde into `sparq-ui` for two file shapes would break the zero-dep promise for
//! everybody; a subset parser with words-on-malformed is the house idiom (the TOML subset parser is
//! the same decision).
//!
//! The subset: objects, arrays, strings (with `\"` `\\` `\/` `\b` `\f` `\n` `\r` `\t` and `\uXXXX`,
//! surrogate pairs included), numbers (int/frac/exp, finite), `true`/`false`/`null`. Rejected in
//! words: comments, trailing commas, trailing garbage, NaN/Infinity literals, lone surrogates,
//! nesting past [`MAX_DEPTH`] (a stack-overflow guard — the parser is recursive-descent).
//!
//! Objects keep insertion order in a `Vec` (the interchange is written sorted anyway; order
//! preservation makes round-trip tests honest) with `O(n)` key lookup — the documents are small
//! (tens of keys per object) and paint-time lookups are per-primitive, not per-pixel.

/// Maximum nesting depth (arrays/objects). The interchange nests ~6 deep; 64 is headroom with a
/// stack guard, not a policy.
pub const MAX_DEPTH: usize = 64;

/// A JSON value.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    /// `null`.
    Null,
    /// `true` / `false`.
    Bool(bool),
    /// A finite number (f64 carrier; the interchange's floats and ints both land here).
    Num(f64),
    /// A string.
    Str(String),
    /// An array, in order.
    Arr(Vec<Json>),
    /// An object, in insertion order.
    Obj(Vec<(String, Json)>),
}

/// A parse failure, in words, with the byte offset so a bad artefact is locatable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonError {
    /// Byte offset into the input where the failure was detected.
    pub at: usize,
    /// What went wrong.
    pub message: String,
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON error at byte {}: {}", self.at, self.message)
    }
}

impl std::error::Error for JsonError {}

impl Json {
    /// Object member lookup (`None` when this is not an object or the key is absent).
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Obj(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The number carrier (`None` for non-numbers — a silent 0 for a missing field is exactly the
    /// misread this parser exists to prevent).
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Num(x) if x.is_finite() => Some(*x),
            _ => None,
        }
    }

    /// The string carrier.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// The boolean carrier.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The array carrier.
    #[must_use]
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Self::Arr(v) => Some(v.as_slice()),
            _ => None,
        }
    }

    /// An f32 view of the number carrier (the interchange's geometry is f32).
    #[must_use]
    pub fn as_f32(&self) -> Option<f32> {
        self.as_f64().map(|x| x as f32)
    }

    /// A u32 view of the number carrier, refusing negatives and non-integers.
    #[must_use]
    pub fn as_u32(&self) -> Option<u32> {
        let x = self.as_f64()?;
        if x < 0.0 || x.fract() != 0.0 || x > f64::from(u32::MAX) {
            return None;
        }
        Some(x as u32)
    }
}

/// Parses a JSON document (the subset above).
///
/// # Errors
/// A [`JsonError`] naming the offset and the defect.
pub fn parse(text: &str) -> Result<Json, JsonError> {
    let mut p = Parser { s: text.as_bytes(), i: 0 };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.i != p.s.len() {
        return Err(p.err("trailing garbage after the top-level value"));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn err(&self, message: &str) -> JsonError {
        JsonError { at: self.i, message: message.to_string() }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, JsonError> {
        if depth > MAX_DEPTH {
            return Err(self.err("nesting deeper than MAX_DEPTH — refusing to overflow the stack"));
        }
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Json::Str),
            Some(b't') => self.lit("true", Json::Bool(true)),
            Some(b'f') => self.lit("false", Json::Bool(false)),
            Some(b'n') => self.lit("null", Json::Null),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            Some(c) => Err(self.err(&format!("unexpected byte {c:?} starting a value"))),
            None => Err(self.err("unexpected end of input where a value was expected")),
        }
    }

    fn lit(&mut self, word: &str, v: Json) -> Result<Json, JsonError> {
        if self.s[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(self.err(&format!("expected literal `{word}`")))
        }
    }

    fn number(&mut self) -> Result<Json, JsonError> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.i += 1;
        }
        if self.eat(b'.') {
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            if !self.eat(b'+') {
                let _ = self.eat(b'-');
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        let text =
            std::str::from_utf8(&self.s[start..self.i]).map_err(|_| self.err("non-utf8 number"))?;
        let x: f64 = text.parse().map_err(|_| self.err(&format!("malformed number `{text}`")))?;
        if !x.is_finite() {
            return Err(
                self.err("NaN/Infinity are not JSON (the interchange writes null for no-cell)")
            );
        }
        Ok(Json::Num(x))
    }

    fn string(&mut self) -> Result<String, JsonError> {
        debug_assert_eq!(self.peek(), Some(b'"'));
        self.i += 1;
        let mut out = String::new();
        loop {
            let c = self.peek().ok_or_else(|| self.err("unterminated string"))?;
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = self.peek().ok_or_else(|| self.err("unterminated escape"))?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            if (0xD800..0xDC00).contains(&hi) {
                                // A high surrogate must be followed by \uDC00.. \uDFFF.
                                if self.peek() == Some(b'\\')
                                    && self.s.get(self.i + 1) == Some(&b'u')
                                {
                                    self.i += 2;
                                    let lo = self.hex4()?;
                                    if !(0xDC00..0xE000).contains(&lo) {
                                        return Err(self.err("broken surrogate pair"));
                                    }
                                    let cp = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
                                    out.push(
                                        char::from_u32(cp)
                                            .ok_or_else(|| self.err("bad codepoint"))?,
                                    );
                                } else {
                                    return Err(self.err("lone high surrogate"));
                                }
                            } else if (0xDC00..0xE000).contains(&hi) {
                                return Err(self.err("lone low surrogate"));
                            } else {
                                out.push(
                                    char::from_u32(hi).ok_or_else(|| self.err("bad codepoint"))?,
                                );
                            }
                        },
                        other => return Err(self.err(&format!("unknown escape \\{other}"))),
                    }
                },
                // Control chars are illegal inside a JSON string.
                0x00..=0x1F => return Err(self.err("unescaped control character in string")),
                _ => {
                    // Copy one UTF-8 sequence (the input is &str bytes, so it is valid UTF-8).
                    let start = self.i - 1;
                    let len = utf8_len(c);
                    self.i = start + len;
                    out.push_str(
                        std::str::from_utf8(&self.s[start..self.i])
                            .map_err(|_| self.err("bad utf-8"))?,
                    );
                },
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        if self.i + 4 > self.s.len() {
            return Err(self.err("truncated \\u escape"));
        }
        let text =
            std::str::from_utf8(&self.s[self.i..self.i + 4]).map_err(|_| self.err("bad escape"))?;
        let v = u32::from_str_radix(text, 16)
            .map_err(|_| self.err(&format!("bad hex escape {text}")))?;
        self.i += 4;
        Ok(v)
    }

    fn array(&mut self, depth: usize) -> Result<Json, JsonError> {
        debug_assert_eq!(self.peek(), Some(b'['));
        self.i += 1;
        let mut out = Vec::new();
        self.ws();
        if self.eat(b']') {
            return Ok(Json::Arr(out));
        }
        loop {
            self.ws();
            out.push(self.value(depth + 1)?);
            self.ws();
            if self.eat(b',') {
                // A trailing comma is rejected: the interchange never writes one, and accepting it
                // would accept a hand-edit mistake silently.
                self.ws();
                if self.peek() == Some(b']') {
                    return Err(self.err("trailing comma in array"));
                }
                continue;
            }
            if self.eat(b']') {
                return Ok(Json::Arr(out));
            }
            return Err(self.err("expected `,` or `]` in array"));
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, JsonError> {
        debug_assert_eq!(self.peek(), Some(b'{'));
        self.i += 1;
        let mut out = Vec::new();
        self.ws();
        if self.eat(b'}') {
            return Ok(Json::Obj(out));
        }
        loop {
            self.ws();
            let key = match self.peek() {
                Some(b'"') => self.string()?,
                _ => return Err(self.err("object keys must be strings")),
            };
            self.ws();
            if !self.eat(b':') {
                return Err(self.err("expected `:` after object key"));
            }
            self.ws();
            let v = self.value(depth + 1)?;
            out.push((key, v));
            self.ws();
            if self.eat(b',') {
                self.ws();
                if self.peek() == Some(b'}') {
                    return Err(self.err("trailing comma in object"));
                }
                continue;
            }
            if self.eat(b'}') {
                return Ok(Json::Obj(out));
            }
            return Err(self.err("expected `,` or `}` in object"));
        }
    }
}

/// The byte length of the UTF-8 sequence starting with `c`.
fn utf8_len(c: u8) -> usize {
    match c {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn scalars_parse() {
        assert_eq!(parse("null").unwrap(), Json::Null);
        assert_eq!(parse("true").unwrap(), Json::Bool(true));
        assert_eq!(parse("false").unwrap(), Json::Bool(false));
        assert_eq!(parse("-12.5e2").unwrap(), Json::Num(-1250.0));
        assert_eq!(parse("\"hi\"").unwrap(), Json::Str("hi".into()));
    }

    #[test]
    fn strings_decode_escapes_and_unicode() {
        assert_eq!(parse(r#""a\nb\tc\"d\\e""#).unwrap(), Json::Str("a\nb\tc\"d\\e".into()));
        assert_eq!(parse(r#""é""#).unwrap(), Json::Str("é".into()));
        // A surrogate pair (the ticker's ❚❚ and the mockups' · are BMP, but pairs must work).
        assert_eq!(parse(r#""\ud83d\udd25""#).unwrap(), Json::Str("🔥".into()));
    }

    #[test]
    fn nested_containers_round_trip() {
        let doc =
            r#"{"items":[{"kind":"rect","x":1.5,"vals":[1,2,3],"on":true,"off":null}],"n":2}"#;
        let v = parse(doc).unwrap();
        assert_eq!(v.get("n").and_then(Json::as_f64), Some(2.0));
        let item = &v.get("items").and_then(Json::as_array).unwrap()[0];
        assert_eq!(item.get("kind").and_then(Json::as_str), Some("rect"));
        assert_eq!(item.get("vals").and_then(Json::as_array).unwrap().len(), 3);
        assert_eq!(item.get("off"), Some(&Json::Null));
    }

    #[test]
    fn malformed_input_is_refused_in_words() {
        for (bad, why) in [
            ("{", "unterminated object"),
            ("[1,]", "trailing comma"),
            ("{\"a\":1,}", "trailing comma"),
            ("{\"a\" 1}", "missing colon"),
            ("nan", "not a literal"),
            ("Infinity", "not a literal"),
            ("1 ", "ok"), // not bad — skipped below
            ("[1] x", "trailing garbage"),
            ("\"\x01\"", "control char"),
        ] {
            if why == "ok" {
                assert!(parse(bad).is_ok());
                continue;
            }
            let e = parse(bad).unwrap_err();
            assert!(!e.message.is_empty(), "{bad}: {e}");
        }
    }

    #[test]
    fn deep_nesting_is_refused_not_crashed() {
        let deep = format!("{}{}", "[".repeat(MAX_DEPTH + 4), "]".repeat(MAX_DEPTH + 4));
        let e = parse(&deep).unwrap_err();
        assert!(e.message.contains("MAX_DEPTH"), "{e}");
    }

    #[test]
    fn numbers_refuse_nan_and_overflow_to_words() {
        assert!(parse("1e999").is_err(), "1e999 overflows to inf — refused, not inf");
    }

    #[test]
    fn accessors_refuse_wrong_carriers() {
        let v = parse("{\"a\":\"x\",\"b\":2}").unwrap();
        assert_eq!(v.get("a").and_then(Json::as_f64), None, "a string is never a silent number");
        assert_eq!(v.get("b").and_then(Json::as_str), None);
        assert_eq!(v.get("b").and_then(Json::as_u32), Some(2));
        assert_eq!(v.get("missing"), None);
    }
}

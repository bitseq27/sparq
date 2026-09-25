//! A dependency-free TOML **subset** parser, for `sparqmod.toml`.
//!
//! # Why this exists rather than a crate
//!
//! The core build has zero third-party dependencies (README, ADR-008): everything testable runs on
//! std alone so CI on a device-less runner is fast and hermetic. Manifest reading is on the
//! discovery path of the core, so it cannot be the thing that breaks that rule. A subset parser is
//! also *better* here than a general one, because the subset is known and closed: every rejection can
//! name the line and the fix, which is what `E-VALUE-MALFORMED` promises and what a general-purpose
//! parser's error messages rarely deliver.
//!
//! # What is supported
//!
//! Comments, `[table]` and `[[array.of.tables]]` headers, dotted keys, bare and quoted keys, basic
//! strings (with `\"`, `\\`, `\n`, `\t`, `\r`, `\uXXXX`), multi-line basic strings (`"""`), literal
//! strings (`'`) and multi-line literal strings (`'''`), integers (with `_` separators and signs),
//! floats, booleans, arrays (which may span lines and carry comments), and inline tables.
//!
//! # What is not, deliberately
//!
//! Datetimes (nothing in the manifest schema uses one), hex/octal/binary integers, `inf`/`nan`, and
//! dotted keys *inside* inline tables. Each is a parse error rather than a silent misreading, because
//! a manifest that loads as something other than what its author wrote is a wrong-sound bug with no
//! audible symptom.
//!
//! # No I/O
//!
//! This module parses `&str` and never touches the filesystem. `clippy.toml` bans `std::fs::read`
//! workspace-wide; rather than rely on `read_to_string` not being named in that list, the crate holds
//! no I/O at all. The caller reads the file — discovery is a control-thread activity and belongs to
//! `sparq-app`, not to the contract.

/// A parsed TOML value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A string, from either basic or literal syntax.
    Str(String),
    /// An integer.
    Int(i64),
    /// A float.
    Float(f64),
    /// A boolean.
    Bool(bool),
    /// An array; elements need not share a type, though the manifest schema's do.
    Array(Vec<Value>),
    /// A table, from a header or an inline `{ ... }`.
    Table(Table),
}

impl Value {
    /// The value as a string slice, if it is one.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The value as an `i64`, if it is an integer. Floats are **not** coerced: a manifest that says
    /// `2.0` where an integer is declared is a mistake worth reporting, not rounding.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(n) => Some(*n),
            _ => None,
        }
    }

    /// The value as a `u32`, if it is a non-negative integer that fits.
    #[must_use]
    pub fn as_u32(&self) -> Option<u32> {
        self.as_i64().and_then(|n| u32::try_from(n).ok())
    }

    /// The value as an `f64`. Integers widen, because `min = 0` for a float parameter is normal.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(x) => Some(*x),
            Self::Int(n) => Some(*n as f64),
            _ => None,
        }
    }

    /// The value as a bool, if it is one.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The value as an array, if it is one.
    #[must_use]
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    /// The value as a table, if it is one.
    #[must_use]
    pub fn as_table(&self) -> Option<&Table> {
        match self {
            Self::Table(t) => Some(t),
            _ => None,
        }
    }

    /// The type's name, for error messages.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Str(_) => "string",
            Self::Int(_) => "integer",
            Self::Float(_) => "float",
            Self::Bool(_) => "boolean",
            Self::Array(_) => "array",
            Self::Table(_) => "table",
        }
    }
}

/// One key in a table, with the line it was written on.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// The key as written.
    pub key: String,
    /// Its value.
    pub value: Value,
    /// The 1-based line the key appeared on.
    pub line: usize,
}

/// A TOML table. Entries keep document order, so error messages can be reported in the order an
/// author would read them, and duplicate keys are detectable.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    entries: Vec<Entry>,
}

impl Table {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks a key up.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().find(|e| e.key == key).map(|e| &e.value)
    }

    /// The line a key was written on, for error messages.
    #[must_use]
    pub fn line_of(&self, key: &str) -> Option<usize> {
        self.entries.iter().find(|e| e.key == key).map(|e| e.line)
    }

    /// Every key, in document order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.key.as_str())
    }

    /// How many keys the table holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// An array-of-tables entry, as a slice of tables. `None` if the key is absent; an empty slice
    /// is impossible, because `[[x]]` always creates at least one.
    #[must_use]
    pub fn tables(&self, key: &str) -> Option<Vec<&Table>> {
        match self.get(key) {
            Some(Value::Array(a)) => Some(a.iter().filter_map(Value::as_table).collect()),
            Some(_) | None => None,
        }
    }
}

/// A parse failure, with the line it happened on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TomlError {
    /// The 1-based line.
    pub line: usize,
    /// What went wrong, and what to do about it.
    pub message: String,
}

impl std::fmt::Display for TomlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for TomlError {}

/// Parses a TOML subset document.
///
/// # Errors
/// A [`TomlError`] naming the line and the fix, on anything outside the supported subset.
pub fn parse(text: &str) -> Result<Table, TomlError> {
    let mut p = Parser { src: text, pos: 0, line: 1, root: Table::new(), path: Vec::new() };
    p.document()?;
    Ok(p.root)
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
    line: usize,
    root: Table,
    /// The table path subsequent `key = value` lines belong to.
    path: Vec<String>,
}

impl<'a> Parser<'a> {
    fn err<T>(&self, message: impl Into<String>) -> Result<T, TomlError> {
        Err(TomlError { line: self.line, message: message.into() })
    }

    fn peek(&self) -> Option<u8> {
        self.src.as_bytes().get(self.pos).copied()
    }

    fn peek_at(&self, n: usize) -> Option<u8> {
        self.src.as_bytes().get(self.pos + n).copied()
    }

    /// Advances one **character**, not one byte.
    ///
    /// Every structural byte this parser compares against is ASCII, so advancing by character is
    /// safe for the structure — and required for the content: slicing `src[start..pos]` after a
    /// single-byte step lands inside a multi-byte character and panics. A manifest whose `summary`
    /// contained an en dash would have aborted the process (`panic = "abort"` in release).
    fn bump(&mut self) {
        if let Some(c) = self.src[self.pos..].chars().next() {
            if c == '\n' {
                self.line += 1;
            }
            self.pos += c.len_utf8();
        }
    }

    fn eat(&mut self, b: u8) -> bool {
        if self.peek() == Some(b) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    /// Spaces and tabs, not newlines.
    fn skip_inline_ws(&mut self) {
        while matches!(self.peek(), Some(b' ') | Some(b'\t')) {
            self.bump();
        }
    }

    /// Whitespace, newlines and comments — used between array elements.
    fn skip_ws_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(b' ') | Some(b'\t') | Some(b'\r') | Some(b'\n') => self.bump(),
                Some(b'#') => {
                    while !self.eof() && self.peek() != Some(b'\n') {
                        self.bump();
                    }
                },
                _ => return,
            }
        }
    }

    fn document(&mut self) -> Result<(), TomlError> {
        loop {
            self.skip_ws_and_comments();
            if self.eof() {
                return Ok(());
            }
            if self.peek() == Some(b'[') {
                self.header()?;
            } else {
                self.key_value()?;
                self.end_of_line()?;
            }
        }
    }

    /// After a `key = value` line, only a comment or the end of the line may follow.
    fn end_of_line(&mut self) -> Result<(), TomlError> {
        self.skip_inline_ws();
        match self.peek() {
            None | Some(b'\n') | Some(b'\r') => Ok(()),
            Some(b'#') => {
                while !self.eof() && self.peek() != Some(b'\n') {
                    self.bump();
                }
                Ok(())
            }
            Some(c) => self.err(format!(
                "unexpected `{}` after a value; a key = value line ends at the newline or a comment",
                c as char
            )),
        }
    }

    fn header(&mut self) -> Result<(), TomlError> {
        self.bump(); // '['
        let array = self.eat(b'[');
        self.skip_inline_ws();
        let path = self.dotted_key()?;
        self.skip_inline_ws();
        if !self.eat(b']') {
            // The usual real cause is a character that is not legal in a bare key, and blaming the
            // missing `]` sends the author to the wrong end of the line.
            // Decode a *character*, not a byte: `peek()` returns the first byte, and rendering a
            // byte as a char turns `ä` into mojibake in the very message meant to help the author.
            let c = self.src[self.pos..].chars().next().unwrap_or('?');
            return self.err(format!(
                "a table header must close with `]`; `{c}` cannot appear in a bare key — quote it, as in [\"name\"]"
            ));
        }
        if array && !self.eat(b']') {
            return self.err("an array-of-tables header must close with `]]`");
        }
        self.path = path.clone();
        if array {
            self.push_array_table(&path)?;
        } else {
            self.declare_table(&path)?;
        }
        self.end_of_line()
    }

    /// A sequence of bare or quoted keys separated by dots.
    fn dotted_key(&mut self) -> Result<Vec<String>, TomlError> {
        let mut parts = vec![self.key_part()?];
        loop {
            self.skip_inline_ws();
            if self.eat(b'.') {
                self.skip_inline_ws();
                parts.push(self.key_part()?);
            } else {
                return Ok(parts);
            }
        }
    }

    fn key_part(&mut self) -> Result<String, TomlError> {
        match self.peek() {
            Some(b'"') => self.basic_string(),
            Some(b'\'') => self.literal_string(),
            Some(c) if c.is_ascii_alphanumeric() || c == b'_' || c == b'-' => {
                let start = self.pos;
                while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
                {
                    self.bump();
                }
                Ok(self.src[start..self.pos].to_string())
            },
            Some(c) => self.err(format!(
                "`{}` cannot start a key; use a bare key (letters, digits, `-`, `_`) or quote it",
                c as char
            )),
            None => self.err("the document ends where a key was expected"),
        }
    }

    fn key_value(&mut self) -> Result<(), TomlError> {
        let line = self.line;
        let mut path = self.dotted_key()?;
        self.skip_inline_ws();
        if !self.eat(b'=') {
            return self.err(format!("expected `=` after `{}`", path.join(".")));
        }
        self.skip_inline_ws();
        let value = self.value()?;
        let mut full = self.path.clone();
        let key = path.pop().unwrap_or_else(|| "<empty>".to_string());
        full.extend(path);
        insert(&mut self.root, &full, key, value, line)
    }

    fn value(&mut self) -> Result<Value, TomlError> {
        match self.peek() {
            Some(b'"') => {
                if self.peek_at(1) == Some(b'"') && self.peek_at(2) == Some(b'"') {
                    Ok(Value::Str(self.multiline_basic_string()?))
                } else {
                    Ok(Value::Str(self.basic_string()?))
                }
            }
            Some(b'\'') => {
                if self.peek_at(1) == Some(b'\'') && self.peek_at(2) == Some(b'\'') {
                    Ok(Value::Str(self.multiline_literal_string()?))
                } else {
                    Ok(Value::Str(self.literal_string()?))
                }
            }
            Some(b'[') => self.array(),
            Some(b'{') => self.inline_table(),
            Some(b't') if self.starts_word("true") => {
                self.pos += 4;
                Ok(Value::Bool(true))
            }
            Some(b'f') if self.starts_word("false") => {
                self.pos += 5;
                Ok(Value::Bool(false))
            }
            Some(c) if c == b'-' || c == b'+' || c.is_ascii_digit() => self.number(),
            Some(c) => self.err(format!(
                "`{}` cannot start a value; expected a string, number, boolean, array or inline table",
                c as char
            )),
            None => self.err("the document ends where a value was expected"),
        }
    }

    fn starts_word(&self, w: &str) -> bool {
        self.src[self.pos..].starts_with(w)
            && !matches!(self.src.as_bytes().get(self.pos + w.len()), Some(c) if c.is_ascii_alphanumeric() || *c == b'_')
    }

    fn number(&mut self) -> Result<Value, TomlError> {
        let start = self.pos;
        let line = self.line;
        if matches!(self.peek(), Some(b'-') | Some(b'+')) {
            self.bump();
        }
        let mut is_float = false;
        while let Some(c) = self.peek() {
            match c {
                b'0'..=b'9' | b'_' => self.bump(),
                b'.' | b'e' | b'E' => {
                    is_float = true;
                    self.bump();
                },
                _ => break,
            }
        }
        let raw = &self.src[start..self.pos];
        if raw == "0"
            && matches!(
                self.peek(),
                Some(b'x') | Some(b'X') | Some(b'o') | Some(b'O') | Some(b'b') | Some(b'B')
            )
        {
            return Err(TomlError {
                line,
                message: "hex, octal and binary integers are not part of the sparq TOML subset; write the value in decimal".to_string(),
            });
        }
        let cleaned: String = raw.chars().filter(|c| *c != '_').collect();
        if cleaned.is_empty() || cleaned == "-" || cleaned == "+" {
            return Err(TomlError { line, message: format!("`{raw}` is not a number") });
        }
        if is_float {
            cleaned
                .parse::<f64>()
                .map(Value::Float)
                .map_err(|_| TomlError { line, message: format!("`{raw}` is not a valid float") })
        } else {
            cleaned.parse::<i64>().map(Value::Int).map_err(|_| TomlError {
                line,
                message: format!("`{raw}` is not a valid integer (hex, octal and binary are not part of the sparq subset)"),
            })
        }
    }

    fn basic_string(&mut self) -> Result<String, TomlError> {
        self.bump(); // opening quote
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return self.err("a string is missing its closing quote"),
                Some(b'\n') => return self.err(
                    "a single-line string ended at a newline; use `\"\"\"` for a multi-line string",
                ),
                Some(b'"') => {
                    self.bump();
                    return Ok(out);
                },
                Some(b'\\') => {
                    self.bump();
                    if let Some(c) = self.escape()? {
                        out.push(c);
                    }
                },
                Some(_) => {
                    let start = self.pos;
                    self.bump();
                    out.push_str(&self.src[start..self.pos]);
                },
            }
        }
    }

    fn escape(&mut self) -> Result<Option<char>, TomlError> {
        let c = match self.peek() {
            Some(c) => c,
            None => return self.err("a string ends with a backslash"),
        };
        self.bump();
        match c {
            b'"' => Ok(Some('"')),
            b'\\' => Ok(Some('\\')),
            b'n' => Ok(Some('\n')),
            b't' => Ok(Some('\t')),
            b'r' => Ok(Some('\r')),
            b'0' => Ok(Some('\0')),
            // TOML's line-continuation: a backslash at the end of a line swallows the newline and
            // every space or tab that follows it. Needed for a long `description` in a manifest.
            b'\r' | b'\n' => {
                while matches!(self.peek(), Some(b' ') | Some(b'\t') | Some(b'\r') | Some(b'\n')) {
                    self.bump();
                }
                Ok(None)
            }
            b'u' | b'U' => {
                let n = if c == b'u' { 4 } else { 8 };
                if self.pos + n > self.src.len() {
                    return self.err(format!("`\\{}` needs {n} hex digits", c as char));
                }
                let hex = &self.src[self.pos..self.pos + n];
                let code = u32::from_str_radix(hex, 16)
                    .map_err(|_| TomlError { line: self.line, message: format!("`\\{c}{hex}` is not {n} hex digits") })?;
                for _ in 0..n {
                    self.bump();
                }
                char::from_u32(code)
                    .map(Some)
                    .ok_or_else(|| TomlError {
                        line: self.line,
                        message: format!("\\u{code:X} is not a Unicode scalar value"),
                    })
            }
            _ => Err(TomlError {
                line: self.line,
                message: format!("`\\{}` is not a supported escape; use \\\" \\\\ \\n \\t \\r \\0 \\uXXXX or \\UXXXXXXXX", c as char),
            }),
        }
    }

    fn multiline_basic_string(&mut self) -> Result<String, TomlError> {
        self.pos += 3;
        // A newline immediately after the opening delimiter is trimmed, per TOML.
        if self.peek() == Some(b'\r') {
            self.bump();
        }
        if self.peek() == Some(b'\n') {
            self.bump();
        }
        let mut out = String::new();
        loop {
            if self.peek() == Some(b'"')
                && self.peek_at(1) == Some(b'"')
                && self.peek_at(2) == Some(b'"')
            {
                self.pos += 3;
                return Ok(out);
            }
            match self.peek() {
                None => return self.err("a multi-line string is missing its closing `\"\"\"`"),
                Some(b'\\') => {
                    self.bump();
                    if let Some(c) = self.escape()? {
                        out.push(c);
                    }
                },
                Some(_) => {
                    let start = self.pos;
                    self.bump();
                    out.push_str(&self.src[start..self.pos]);
                },
            }
        }
    }

    fn literal_string(&mut self) -> Result<String, TomlError> {
        self.bump(); // opening apostrophe
        let start = self.pos;
        loop {
            match self.peek() {
                None => return self.err("a literal string is missing its closing `'`"),
                Some(b'\n') => {
                    return self.err(
                        "a single-line literal string ended at a newline; use `'''` for multi-line",
                    )
                },
                Some(b'\'') => {
                    let s = self.src[start..self.pos].to_string();
                    self.bump();
                    return Ok(s);
                },
                Some(_) => self.bump(),
            }
        }
    }

    fn multiline_literal_string(&mut self) -> Result<String, TomlError> {
        self.pos += 3;
        if self.peek() == Some(b'\r') {
            self.bump();
        }
        if self.peek() == Some(b'\n') {
            self.bump();
        }
        let start = self.pos;
        loop {
            if self.peek() == Some(b'\'')
                && self.peek_at(1) == Some(b'\'')
                && self.peek_at(2) == Some(b'\'')
            {
                let s = self.src[start..self.pos].to_string();
                self.pos += 3;
                return Ok(s);
            }
            if self.eof() {
                return self.err("a multi-line literal string is missing its closing `'''`");
            }
            self.bump();
        }
    }

    fn array(&mut self) -> Result<Value, TomlError> {
        self.bump(); // '['
        let mut items = Vec::new();
        loop {
            self.skip_ws_and_comments();
            if self.eat(b']') {
                return Ok(Value::Array(items));
            }
            if self.eof() {
                return self.err("an array is missing its closing `]`");
            }
            items.push(self.value()?);
            self.skip_ws_and_comments();
            if self.eat(b',') {
                continue;
            }
            self.skip_ws_and_comments();
            if self.eat(b']') {
                return Ok(Value::Array(items));
            }
            return self.err("an array element must be followed by `,` or `]`");
        }
    }

    fn inline_table(&mut self) -> Result<Value, TomlError> {
        self.bump(); // '{'
        let mut t = Table::new();
        loop {
            self.skip_inline_ws();
            if self.eat(b'}') {
                return Ok(Value::Table(t));
            }
            if self.eof() {
                return self.err("an inline table is missing its closing `}`");
            }
            let line = self.line;
            let key = self.key_part()?;
            if self.key_part_is_dotted() {
                return self.err("dotted keys are not part of the sparq subset inside an inline table; write `{ a = { b = 1 } }`");
            }
            self.skip_inline_ws();
            if !self.eat(b'=') {
                return self.err(format!("expected `=` after `{key}` in an inline table"));
            }
            self.skip_inline_ws();
            let v = self.value()?;
            insert(&mut t, &[], key, v, line)?;
            self.skip_inline_ws();
            if self.eat(b',') {
                continue;
            }
            if self.eat(b'}') {
                return Ok(Value::Table(t));
            }
            return self.err("an inline table entry must be followed by `,` or `}`");
        }
    }

    fn key_part_is_dotted(&mut self) -> bool {
        self.skip_inline_ws();
        self.peek() == Some(b'.')
    }

    /// Creates (or re-opens) the table at `path`, so later `key = value` lines land in it.
    ///
    /// Each step looks the key up *before* mutating, because a borrow held by the lookup would
    /// otherwise outlive the `push` that creates the missing entry.
    fn declare_table(&mut self, path: &[String]) -> Result<(), TomlError> {
        let line = self.line;
        let mut cur = &mut self.root;
        for part in path {
            let found = cur.entries.iter().position(|e| &e.key == part);
            let idx = match found {
                Some(i) => i,
                None => {
                    let i = cur.entries.len();
                    cur.entries.push(Entry {
                        key: part.clone(),
                        value: Value::Table(Table::new()),
                        line,
                    });
                    i
                },
            };
            cur = match &mut cur.entries[idx].value {
                Value::Table(t) => t,
                Value::Array(a) => match a.last_mut() {
                    Some(Value::Table(t)) => t,
                    _ => {
                        return Err(TomlError {
                            line,
                            message: format!("`{part}` is already defined and is not a table"),
                        })
                    },
                },
                _ => {
                    return Err(TomlError {
                        line,
                        message: format!(
                            "`{part}` is already defined as a value, so it cannot also be a table"
                        ),
                    })
                },
            };
        }
        Ok(())
    }

    /// Appends a fresh table to the array at `path`, creating the array if this is the first
    /// `[[path]]` header.
    fn push_array_table(&mut self, path: &[String]) -> Result<(), TomlError> {
        let line = self.line;
        let (parents, last) = path.split_at(path.len().saturating_sub(1));
        let Some(name) = last.first() else {
            return self.err("[[]] needs a table name");
        };
        let name = name.clone();
        let joined = path.join(".");
        let mut cur = &mut self.root;
        for part in parents {
            let found = cur.entries.iter().position(|e| e.key == *part);
            let idx = match found {
                Some(i) => i,
                None => {
                    let i = cur.entries.len();
                    cur.entries.push(Entry {
                        key: part.clone(),
                        value: Value::Table(Table::new()),
                        line,
                    });
                    i
                },
            };
            cur = match &mut cur.entries[idx].value {
                Value::Table(t) => t,
                _ => {
                    return Err(TomlError {
                        line,
                        message: format!(
                            "`{part}` is not a table, so `[[{joined}]]` has nowhere to live"
                        ),
                    })
                },
            };
        }
        // `self.path` already points at this array, and `insert` resolves an array path to its last
        // element — which is the table appended here. That is what makes `[[ports]]` followed by
        // `id = "in"` land in the new element rather than the array.
        let found = cur.entries.iter().position(|e| e.key == name);
        match found {
            Some(idx) => match &mut cur.entries[idx].value {
                Value::Array(a) => a.push(Value::Table(Table::new())),
                _ => {
                    return Err(TomlError {
                        line,
                        message: format!(
                        "`{name}` is already a single value, so `[[{name}]]` cannot append to it"
                    ),
                    })
                },
            },
            None => cur.entries.push(Entry {
                key: name,
                value: Value::Array(vec![Value::Table(Table::new())]),
                line,
            }),
        }
        Ok(())
    }
}

/// Inserts `key = value` at `path` inside `root`, creating tables as needed. A path that ends in an
/// array element resolves to its **last** element.
fn insert(
    root: &mut Table,
    path: &[String],
    key: String,
    value: Value,
    line: usize,
) -> Result<(), TomlError> {
    let mut cur = root;
    for part in path {
        let found = cur.entries.iter().position(|e| &e.key == part);
        let idx = match found {
            Some(i) => i,
            None => {
                let i = cur.entries.len();
                cur.entries.push(Entry {
                    key: part.clone(),
                    value: Value::Table(Table::new()),
                    line,
                });
                i
            },
        };
        cur = match &mut cur.entries[idx].value {
            Value::Table(t) => t,
            Value::Array(a) => match a.last_mut() {
                Some(Value::Table(t)) => t,
                _ => {
                    return Err(TomlError {
                        line,
                        message: format!("`{part}` is an array that does not end in a table"),
                    })
                },
            },
            _ => {
                return Err(TomlError {
                    line,
                    message: format!("`{part}` is already a value, so `{key}` has nowhere to live"),
                })
            },
        };
    }
    if cur.entries.iter().any(|e| e.key == key) {
        return Err(TomlError {
            line,
            message: format!(
                "`{key}` is defined twice in the same table; a key may appear only once"
            ),
        });
    }
    cur.entries.push(Entry { key, value, line });
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    /// A manifest-shaped document, exercising every construct the schema needs.
    const SAMPLE: &str = r#"
# sparqmod.toml — a comment before anything else
[identity]
id = "sparq/util/gain"          # trailing comment
version = "0.1.0"
host_api = { min = 1, max = 1 }
display_name = "Gain"
summary = "Gain and trim"
description = """
A long
description with a "quote" inside."""
authors = ["sparq", "someone else"]
license = "MIT"
tags = []

[classification]
category = "utility/gain"
top = "util"
kind = "processor"
tier = "t1"
stability = "stable"

[[ports]]
id = "in"
name = "Input"
direction = "in"
type = "audio"
channel_set = "stereo"

[[ports]]
id = "out"
name = "Output"
direction = "out"
type = "audio"
channel_set = "stereo"

[[params]]
id = "gain"
name = "Gain"
type = "float"
unit = "ratio"
min = 0.0
max = 2.0
default = 1.0
smoothing = "one_pole:5"

[state]
schema_id = "sparq/util/gain/state"
schema_version = 1

[resources]
latency = 0
tail_samples = 0
cpu_class = "trivial"
oversampling = "none"
requires = ["fft"]
"#;

    #[test]
    fn parses_a_manifest_shaped_document() {
        let t = parse(SAMPLE).unwrap_or_else(|e| panic!("{e}"));
        let id = t.get("identity").and_then(Value::as_table).unwrap();
        assert_eq!(id.get("id").and_then(Value::as_str), Some("sparq/util/gain"));
        assert_eq!(id.get("license").and_then(Value::as_str), Some("MIT"));
        assert!(id.line_of("license").is_some(), "every entry records the line it was written on");

        let host = id.get("host_api").and_then(Value::as_table).unwrap();
        assert_eq!(host.get("min").and_then(Value::as_i64), Some(1));
        assert_eq!(host.get("max").and_then(Value::as_u32), Some(1));

        let authors = id.get("authors").and_then(Value::as_array).unwrap();
        assert_eq!(authors.len(), 2);
        assert_eq!(authors[1].as_str(), Some("someone else"));
        assert!(id.get("tags").and_then(Value::as_array).unwrap().is_empty());

        let desc = id.get("description").and_then(Value::as_str).unwrap();
        assert!(desc.starts_with("A long\n"), "{desc:?}");
        assert!(
            desc.contains("\"quote\""),
            "basic multi-line strings still process escapes: {desc:?}"
        );

        let ports = t.tables("ports").unwrap();
        assert_eq!(ports.len(), 2, "[[ports]] twice must append, not overwrite");
        assert_eq!(ports[0].get("id").and_then(Value::as_str), Some("in"));
        assert_eq!(ports[1].get("direction").and_then(Value::as_str), Some("out"));
        assert_eq!(ports[0].get("type").and_then(Value::as_str), Some("audio"));

        let params = t.tables("params").unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].get("min").and_then(Value::as_f64), Some(0.0));
        assert_eq!(params[0].get("default").and_then(Value::as_f64), Some(1.0));
        assert_eq!(params[0].get("smoothing").and_then(Value::as_str), Some("one_pole:5"));

        let res = t.get("resources").and_then(Value::as_table).unwrap();
        assert_eq!(res.get("latency").and_then(Value::as_u32), Some(0));
        assert_eq!(res.get("requires").and_then(Value::as_array).unwrap().len(), 1);

        let cls = t.get("classification").and_then(Value::as_table).unwrap();
        assert_eq!(cls.keys().count(), 5);
    }

    #[test]
    fn a_hash_inside_a_string_is_not_a_comment() {
        let t = parse("a = \"x # y\" # the real comment\nb = '#'\n").unwrap();
        assert_eq!(t.get("a").and_then(Value::as_str), Some("x # y"));
        assert_eq!(t.get("b").and_then(Value::as_str), Some("#"));
    }

    #[test]
    fn literal_strings_do_not_process_escapes() {
        let t = parse(
            r#"a = 'no \n escape'
b = "but \n here"
c = '\u0041 stays literal'"#,
        )
        .unwrap();
        assert_eq!(t.get("a").and_then(Value::as_str), Some(r"no \n escape"));
        assert_eq!(t.get("b").and_then(Value::as_str), Some("but \n here"));
        assert_eq!(t.get("c").and_then(Value::as_str), Some(r"\u0041 stays literal"));
    }

    #[test]
    fn escapes_and_unicode_resolve() {
        let t = parse(
            r#"a = "tab\there"
b = "quote\"inside"
c = "\u00e9clair"
d = "back\\slash"
e = "\0null""#,
        )
        .unwrap();
        assert_eq!(t.get("a").and_then(Value::as_str), Some("tab\there"));
        assert_eq!(t.get("b").and_then(Value::as_str), Some("quote\"inside"));
        assert_eq!(t.get("c").and_then(Value::as_str), Some("éclair"));
        assert_eq!(t.get("d").and_then(Value::as_str), Some("back\\slash"));
        assert!(t.get("e").and_then(Value::as_str).unwrap().contains('\0'));
    }

    #[test]
    fn a_line_continuation_swallows_the_newline_and_the_indentation() {
        let t = parse("a = \"\"\"one \\\n     two\"\"\"\n").unwrap();
        assert_eq!(t.get("a").and_then(Value::as_str), Some("one two"));
    }

    #[test]
    fn dotted_keys_and_nested_tables_agree() {
        let a = parse("[identity]\nid = \"x\"\n").unwrap();
        let b = parse("identity.id = \"x\"\n").unwrap();
        // The *values* must agree. The recorded line numbers legitimately differ: in the header form
        // the key is on line 2 and in the dotted form on line 1, and the line is what an error points
        // at, so comparing the whole tree would be asserting something false.
        let va = a.get("identity").and_then(Value::as_table).and_then(|t| t.get("id"));
        let vb = b.get("identity").and_then(Value::as_table).and_then(|t| t.get("id"));
        assert_eq!(va, vb, "a dotted key is the same table as its header form");
        assert_eq!(va.and_then(Value::as_str), Some("x"));
    }

    #[test]
    fn arrays_may_span_lines_and_carry_comments() {
        let t = parse("a = [\n  1, # one\n  2,\n  3,\n]\n").unwrap();
        let a = t.get("a").and_then(Value::as_array).unwrap();
        assert_eq!(a.len(), 3);
        assert_eq!(a[2].as_i64(), Some(3));
    }

    #[test]
    fn numbers_keep_their_types_and_do_not_coerce_silently() {
        let t = parse("i = 42\nneg = -7\nunder = 1_000_000\nf = 1.5\nexp = 1e3\nbig = 96000\n")
            .unwrap();
        assert_eq!(t.get("i").and_then(Value::as_i64), Some(42));
        assert_eq!(t.get("neg").and_then(Value::as_i64), Some(-7));
        assert_eq!(
            t.get("under").and_then(Value::as_i64),
            Some(1_000_000),
            "underscores are separators"
        );
        assert_eq!(t.get("f").and_then(Value::as_f64), Some(1.5));
        assert_eq!(t.get("exp").and_then(Value::as_f64), Some(1000.0));
        // An integer widens to f64 (min = 0 for a float param is normal)...
        assert_eq!(t.get("big").and_then(Value::as_f64), Some(96_000.0));
        // ...but a float never narrows to an integer, and a negative never becomes a u32.
        assert_eq!(t.get("f").and_then(Value::as_i64), None);
        assert_eq!(t.get("neg").and_then(Value::as_u32), None);
        assert_eq!(t.get("i").and_then(Value::as_bool), None);
        assert_eq!(t.get("i").map(Value::type_name), Some("integer"));
    }

    #[test]
    fn booleans_parse_and_almost_booleans_do_not() {
        let t = parse("a = true\nb = false\n").unwrap();
        assert_eq!(t.get("a").and_then(Value::as_bool), Some(true));
        assert_eq!(t.get("b").and_then(Value::as_bool), Some(false));
        assert!(parse("a = truex\n").is_err(), "`truex` is not `true`");
    }

    #[test]
    fn a_duplicate_key_is_refused_with_its_line() {
        let e = parse("a = 1\nb = 2\na = 3\n").unwrap_err();
        assert_eq!(e.line, 3);
        assert!(e.message.contains("twice"), "{e}");
    }

    #[test]
    fn an_unterminated_string_names_the_line_and_the_fix() {
        // A single-line string that runs into a newline says so and points at the multi-line form —
        // that is the actionable version of "missing its closing quote".
        let e = parse("a = 1\nb = \"oops\n").unwrap_err();
        assert_eq!(e.line, 2);
        assert!(e.message.contains("newline"), "{e}");
        assert!(e.message.contains("multi-line"), "it offers the fix: {e}");
        // A document that simply ends mid-string reports the missing quote.
        let e = parse("a = \"oops").unwrap_err();
        assert!(e.message.contains("closing quote"), "{e}");
        // Both refusals name a line, which is the whole point of a subset parser.
        assert!(e.line >= 1);
    }

    #[test]
    fn an_unterminated_array_and_inline_table_are_refused() {
        assert!(parse("a = [1, 2\n").is_err());
        assert!(parse("a = { b = 1\n").is_err());
        assert!(parse("a = { b = 1 }\n").is_ok());
    }

    #[test]
    fn junk_after_a_value_is_refused() {
        let e = parse("a = 1 2\n").unwrap_err();
        assert!(e.message.contains("after a value"), "{e}");
    }

    #[test]
    fn datetimes_are_outside_the_subset_and_say_so() {
        // Not silently misread as the integer 1979: the parser refuses at the `-`.
        let e = parse("when = 1979-05-27T07:32:00Z\n").unwrap_err();
        assert_eq!(e.line, 1);
        assert!(e.message.contains("after a value"), "{e}");
    }

    #[test]
    fn hex_is_outside_the_subset_and_says_so() {
        let e = parse("a = 0xFF\n").unwrap_err();
        assert!(e.message.contains("hex") || e.message.contains("not a valid integer"), "{e}");
    }

    #[test]
    fn a_missing_equals_sign_is_refused() {
        let e = parse("[t]\nkey value\n").unwrap_err();
        assert!(e.message.contains("expected `=`"), "{e}");
    }

    #[test]
    fn non_ascii_content_survives_intact() {
        // Values may contain any Unicode. Bare *keys* may not — TOML allows only A-Za-z0-9_- there,
        // so a non-ASCII key has to be quoted, and the parser says so rather than guessing.
        let t = parse("a = \"µs · ±1 — é\"\n").unwrap();
        assert_eq!(t.get("a").and_then(Value::as_str), Some("µs · ±1 — é"));

        let quoted = parse(
            r#"["täble"]
b = 1
"#,
        )
        .unwrap();
        let inner = quoted.get("täble").and_then(Value::as_table).unwrap();
        assert_eq!(inner.get("b").and_then(Value::as_i64), Some(1));

        // A bare key with a non-ASCII character: the error names the character and offers quoting,
        // rather than blaming the closing bracket at the other end of the line.
        let e = parse("[täble]\nb = 1\n").unwrap_err();
        assert!(e.message.contains("cannot appear in a bare key"), "{e}");
        assert!(e.message.contains("quote it"), "the message offers the fix: {e}");
        assert!(e.message.contains('ä'), "and names the offending character: {e}");
    }

    #[test]
    fn an_empty_document_is_an_empty_table() {
        let t = parse("").unwrap();
        assert!(t.is_empty());
        assert_eq!(t.len(), 0);
        assert!(t.tables("ports").is_none());
        assert_eq!(parse("# only a comment\n").unwrap().len(), 0);
    }

    #[test]
    fn a_value_cannot_also_be_a_table() {
        let e = parse("a = 1\n[a]\nb = 2\n").unwrap_err();
        assert!(e.message.contains("already defined"), "{e}");
    }

    #[test]
    fn inline_tables_reject_dotted_keys_rather_than_guessing() {
        let e = parse("a = { b.c = 1 }\n").unwrap_err();
        assert!(e.message.contains("dotted keys"), "{e}");
        assert!(parse("a = { b = { c = 1 } }\n").is_ok(), "the nested form is the supported one");
    }
}

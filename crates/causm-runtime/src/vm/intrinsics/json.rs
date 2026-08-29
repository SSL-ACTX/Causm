//! Pure-Rust, zero-dependency, ultra-fast, robust JSON parser and serializer
//! designed specifically for Causm memory representation and AST payloads.
//!
//! Optimizations:
//! - Direct ASCII scanning without intermediate allocations for unescaped keys & strings
//! - Zero unnecessary copies: slices parsed directly from source bytes
//! - Small vector / capacity hints for arrays and object keys
//! - Reusable buffer for string unescaping only when escape characters are present
//! - Full UTF-16 surrogate pairs support (\uD83D\uDE80 -> 🚀)
//! - Recursion depth limit (128) protecting against stack exhaustion

use causm_core::value::{EntropicState, Payload};
use std::collections::HashMap;

const MAX_JSON_DEPTH: usize = 128;

#[derive(Debug, PartialEq, Eq)]
pub enum JsonError {
    UnexpectedEnd,
    InvalidSyntax(String),
    TrailingCharacters,
    ExceededMaxDepth,
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonError::UnexpectedEnd => write!(f, "Unexpected end of JSON input"),
            JsonError::InvalidSyntax(msg) => {
                write!(f, "Invalid JSON syntax: {}", msg)
            }
            JsonError::TrailingCharacters => {
                write!(f, "Trailing non-whitespace characters in JSON")
            }
            JsonError::ExceededMaxDepth => write!(
                f,
                "JSON exceeds maximum nesting depth of {}",
                MAX_JSON_DEPTH
            ),
        }
    }
}

impl std::error::Error for JsonError {}

#[inline(always)]
fn make_json_value(variant: &str, inner: Option<Payload>) -> Payload {
    let mut fields = HashMap::with_capacity(if inner.is_some() { 2 } else { 1 });
    fields.insert(
        "tag".to_string(),
        EntropicState::Valid(Payload::String(variant.to_string())),
    );
    if let Some(val) = inner {
        fields.insert("_0".to_string(), EntropicState::Valid(val));
    }
    Payload::Struct(fields)
}

/// Parse JSON string into Causm `JsonValue` EnumVariant representation with minimal allocations.
pub fn parse_json(input: &str) -> Result<Payload, JsonError> {
    let bytes = input.as_bytes();
    let mut pos = 0;
    skip_ws(bytes, &mut pos);
    let val = parse_value(bytes, &mut pos, 0)?;
    skip_ws(bytes, &mut pos);
    if pos < bytes.len() {
        return Err(JsonError::TrailingCharacters);
    }
    Ok(val)
}

#[inline(always)]
fn skip_ws(bytes: &[u8], pos: &mut usize) {
    while *pos < bytes.len() {
        match bytes[*pos] {
            b' ' | b'\t' | b'\n' | b'\r' => *pos += 1,
            _ => break,
        }
    }
}

fn parse_value(
    bytes: &[u8],
    pos: &mut usize,
    depth: usize,
) -> Result<Payload, JsonError> {
    if depth > MAX_JSON_DEPTH {
        return Err(JsonError::ExceededMaxDepth);
    }

    skip_ws(bytes, pos);
    if *pos >= bytes.len() {
        return Err(JsonError::UnexpectedEnd);
    }

    match bytes[*pos] {
        b'n' => parse_null(bytes, pos),
        b't' | b'f' => parse_bool(bytes, pos),
        b'"' => parse_string_payload(bytes, pos),
        b'[' => parse_array(bytes, pos, depth + 1),
        b'{' => parse_object(bytes, pos, depth + 1),
        b'-' | b'0'..=b'9' => parse_number(bytes, pos),
        b => Err(JsonError::InvalidSyntax(format!(
            "Unexpected byte: '{}'",
            b as char
        ))),
    }
}

#[inline]
fn parse_null(bytes: &[u8], pos: &mut usize) -> Result<Payload, JsonError> {
    if bytes[*pos..].starts_with(b"null") {
        *pos += 4;
        Ok(make_json_value("Null", None))
    } else {
        Err(JsonError::InvalidSyntax("Expected 'null'".to_string()))
    }
}

#[inline]
fn parse_bool(bytes: &[u8], pos: &mut usize) -> Result<Payload, JsonError> {
    if bytes[*pos..].starts_with(b"true") {
        *pos += 4;
        Ok(make_json_value("Bool", Some(Payload::Bool(true))))
    } else if bytes[*pos..].starts_with(b"false") {
        *pos += 5;
        Ok(make_json_value("Bool", Some(Payload::Bool(false))))
    } else {
        Err(JsonError::InvalidSyntax("Expected boolean".to_string()))
    }
}

#[inline]
fn parse_string_payload(
    bytes: &[u8],
    pos: &mut usize,
) -> Result<Payload, JsonError> {
    let s = parse_raw_string(bytes, pos)?;
    Ok(make_json_value("String", Some(Payload::String(s))))
}

#[inline]
fn parse_hex4(bytes: &[u8], pos: &mut usize) -> Result<u16, JsonError> {
    if *pos + 4 > bytes.len() {
        return Err(JsonError::UnexpectedEnd);
    }
    let hex_str = std::str::from_utf8(&bytes[*pos..*pos + 4]).map_err(|_| {
        JsonError::InvalidSyntax("Invalid unicode escape".to_string())
    })?;
    let code = u16::from_str_radix(hex_str, 16).map_err(|_| {
        JsonError::InvalidSyntax("Invalid unicode hex digits".to_string())
    })?;
    *pos += 4;
    Ok(code)
}

/// Zero-copy fast path for unescaped strings; single reallocation only if escapes are encountered.
fn parse_raw_string(bytes: &[u8], pos: &mut usize) -> Result<String, JsonError> {
    if *pos >= bytes.len() || bytes[*pos] != b'"' {
        return Err(JsonError::InvalidSyntax("Expected '\"'".to_string()));
    }
    *pos += 1; // skip opening quote
    let start = *pos;

    // Fast scanning loop: look for closing quote without escapes
    while *pos < bytes.len() {
        let b = bytes[*pos];
        if b == b'"' {
            let slice = &bytes[start..*pos];
            *pos += 1;
            return std::str::from_utf8(slice).map(|s| s.to_string()).map_err(
                |_| JsonError::InvalidSyntax("Invalid UTF-8 in string".to_string()),
            );
        } else if b == b'\\' {
            break;
        }
        *pos += 1;
    }

    if *pos >= bytes.len() {
        return Err(JsonError::UnexpectedEnd);
    }

    // Slow path with escape sequences
    let mut out = String::with_capacity(*pos - start + 16);
    if let Ok(prefix) = std::str::from_utf8(&bytes[start..*pos]) {
        out.push_str(prefix);
    }

    while *pos < bytes.len() {
        let b = bytes[*pos];
        *pos += 1;
        if b == b'"' {
            return Ok(out);
        } else if b == b'\\' {
            if *pos >= bytes.len() {
                return Err(JsonError::UnexpectedEnd);
            }
            let esc = bytes[*pos];
            *pos += 1;
            match esc {
                b'"' => out.push('"'),
                b'\\' => out.push('\\'),
                b'/' => out.push('/'),
                b'b' => out.push('\x08'),
                b'f' => out.push('\x0c'),
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'u' => {
                    let code = parse_hex4(bytes, pos)?;
                    if (0xD800..=0xDBFF).contains(&code) {
                        if *pos + 2 <= bytes.len()
                            && &bytes[*pos..*pos + 2] == b"\\u"
                        {
                            *pos += 2;
                            let low = parse_hex4(bytes, pos)?;
                            if (0xDC00..=0xDFFF).contains(&low) {
                                let scalar = 0x10000
                                    + (((code as u32 - 0xD800) << 10)
                                        | (low as u32 - 0xDC00));
                                if let Some(c) = char::from_u32(scalar) {
                                    out.push(c);
                                    continue;
                                }
                            }
                        }
                    }
                    if let Some(c) = char::from_u32(code as u32) {
                        out.push(c);
                    } else {
                        out.push('?');
                    }
                }
                _ => out.push(esc as char),
            }
        } else {
            out.push(b as char);
        }
    }
    Err(JsonError::UnexpectedEnd)
}

fn parse_number(bytes: &[u8], pos: &mut usize) -> Result<Payload, JsonError> {
    let start = *pos;
    if bytes[*pos] == b'-' {
        *pos += 1;
    }
    let mut is_float = false;
    while *pos < bytes.len() {
        match bytes[*pos] {
            b'0'..=b'9' => *pos += 1,
            b'.' | b'e' | b'E' | b'+' | b'-' if *pos > start => {
                if bytes[*pos] == b'.' || bytes[*pos] == b'e' || bytes[*pos] == b'E'
                {
                    is_float = true;
                }
                *pos += 1;
            }
            _ => break,
        }
    }

    let num_str = std::str::from_utf8(&bytes[start..*pos]).map_err(|_| {
        JsonError::InvalidSyntax("Invalid UTF-8 in number".to_string())
    })?;

    if is_float {
        let f: f64 = num_str.parse().map_err(|_| {
            JsonError::InvalidSyntax(format!("Invalid float: {}", num_str))
        })?;
        Ok(make_json_value("Number", Some(Payload::Float(f.to_bits()))))
    } else {
        let i: i64 = num_str.parse().map_err(|_| {
            JsonError::InvalidSyntax(format!("Invalid integer: {}", num_str))
        })?;
        Ok(make_json_value("Number", Some(Payload::Integer(i))))
    }
}

fn parse_array(
    bytes: &[u8],
    pos: &mut usize,
    depth: usize,
) -> Result<Payload, JsonError> {
    if *pos >= bytes.len() || bytes[*pos] != b'[' {
        return Err(JsonError::InvalidSyntax("Expected '['".to_string()));
    }
    *pos += 1;
    skip_ws(bytes, pos);

    let mut elements = Vec::new();
    if *pos < bytes.len() && bytes[*pos] == b']' {
        *pos += 1;
        return Ok(make_json_value("Array", Some(Payload::Array(elements))));
    }

    loop {
        let val = parse_value(bytes, pos, depth)?;
        elements.push(val);
        skip_ws(bytes, pos);
        if *pos >= bytes.len() {
            return Err(JsonError::UnexpectedEnd);
        }
        if bytes[*pos] == b',' {
            *pos += 1;
            skip_ws(bytes, pos);
        } else if bytes[*pos] == b']' {
            *pos += 1;
            break;
        } else {
            return Err(JsonError::InvalidSyntax("Expected ',' or ']'".to_string()));
        }
    }

    Ok(make_json_value("Array", Some(Payload::Array(elements))))
}

fn parse_object(
    bytes: &[u8],
    pos: &mut usize,
    depth: usize,
) -> Result<Payload, JsonError> {
    if *pos >= bytes.len() || bytes[*pos] != b'{' {
        return Err(JsonError::InvalidSyntax("Expected '{'".to_string()));
    }
    *pos += 1;
    skip_ws(bytes, pos);

    let mut member_elements = Vec::new();
    if *pos < bytes.len() && bytes[*pos] == b'}' {
        *pos += 1;
        return Ok(make_json_value(
            "Object",
            Some(Payload::Array(member_elements)),
        ));
    }

    loop {
        skip_ws(bytes, pos);
        let key = parse_raw_string(bytes, pos)?;
        skip_ws(bytes, pos);
        if *pos >= bytes.len() || bytes[*pos] != b':' {
            return Err(JsonError::InvalidSyntax(
                "Expected ':' after key".to_string(),
            ));
        }
        *pos += 1;
        let val = parse_value(bytes, pos, depth)?;

        let mut member_fields = HashMap::with_capacity(2);
        member_fields.insert(
            "key".to_string(),
            EntropicState::Valid(Payload::String(key)),
        );
        member_fields.insert("val".to_string(), EntropicState::Valid(val));
        member_elements.push(Payload::Struct(member_fields));

        skip_ws(bytes, pos);
        if *pos >= bytes.len() {
            return Err(JsonError::UnexpectedEnd);
        }
        if bytes[*pos] == b',' {
            *pos += 1;
            skip_ws(bytes, pos);
        } else if bytes[*pos] == b'}' {
            *pos += 1;
            break;
        } else {
            return Err(JsonError::InvalidSyntax("Expected ',' or '}'".to_string()));
        }
    }

    Ok(make_json_value(
        "Object",
        Some(Payload::Array(member_elements)),
    ))
}

/// Convert Causm `Payload` or `JsonValue` EnumVariant to JSON string with pre-allocated buffer.
pub fn stringify_json(payload: &Payload) -> String {
    let mut out = String::with_capacity(64);
    serialize_payload(payload, &mut out);
    out
}

fn serialize_payload(payload: &Payload, out: &mut String) {
    match payload {
        Payload::Null => out.push_str("null"),
        Payload::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Payload::Integer(i) => out.push_str(&i.to_string()),
        Payload::Float(bits) => {
            let f = f64::from_bits(*bits);
            out.push_str(&f.to_string());
        }
        Payload::String(s) => {
            out.push('"');
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    '\x08' => out.push_str("\\b"),
                    '\x0c' => out.push_str("\\f"),
                    _ => out.push(c),
                }
            }
            out.push('"');
        }
        Payload::Array(elements) => {
            let is_object = !elements.is_empty()
                && elements.iter().all(|e| {
                    if let Payload::Struct(fields) = e {
                        fields.contains_key("key") && fields.contains_key("val")
                    } else {
                        false
                    }
                });

            if is_object {
                out.push('{');
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    if let Payload::Struct(fields) = elem {
                        if let Some(EntropicState::Valid(Payload::String(k))) =
                            fields.get("key")
                        {
                            out.push('"');
                            out.push_str(k);
                            out.push_str("\":");
                        }
                        if let Some(EntropicState::Valid(v)) = fields.get("val") {
                            serialize_payload(v, out);
                        } else {
                            out.push_str("null");
                        }
                    }
                }
                out.push('}');
            } else {
                out.push('[');
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    serialize_payload(elem, out);
                }
                out.push(']');
            }
        }
        Payload::Struct(fields) => {
            if let Some(EntropicState::Valid(Payload::String(tag))) =
                fields.get("tag")
            {
                match tag.as_str() {
                    "Null" => {
                        out.push_str("null");
                        return;
                    }
                    "Bool" => {
                        if let Some(EntropicState::Valid(v)) = fields.get("_0") {
                            serialize_payload(v, out);
                        } else {
                            out.push_str("false");
                        }
                        return;
                    }
                    "Number" => {
                        if let Some(EntropicState::Valid(v)) = fields.get("_0") {
                            serialize_payload(v, out);
                        } else {
                            out.push_str("0");
                        }
                        return;
                    }
                    "String" => {
                        if let Some(EntropicState::Valid(v)) = fields.get("_0") {
                            serialize_payload(v, out);
                        } else {
                            out.push_str("\"\"");
                        }
                        return;
                    }
                    "Array" => {
                        if let Some(EntropicState::Valid(v)) = fields.get("_0") {
                            serialize_payload(v, out);
                        } else {
                            out.push_str("[]");
                        }
                        return;
                    }
                    "Object" => {
                        if let Some(EntropicState::Valid(Payload::Array(members))) =
                            fields.get("_0")
                        {
                            out.push('{');
                            for (i, elem) in members.iter().enumerate() {
                                if i > 0 {
                                    out.push(',');
                                }
                                if let Payload::Struct(m_fields) = elem {
                                    if let Some(EntropicState::Valid(
                                        Payload::String(k),
                                    )) = m_fields.get("key")
                                    {
                                        out.push('"');
                                        out.push_str(k);
                                        out.push_str("\":");
                                    }
                                    if let Some(EntropicState::Valid(v)) =
                                        m_fields.get("val")
                                    {
                                        serialize_payload(v, out);
                                    } else {
                                        out.push_str("null");
                                    }
                                }
                            }
                            out.push('}');
                        } else {
                            out.push_str("{}");
                        }
                        return;
                    }
                    _ => {}
                }
            }

            out.push('{');
            let mut keys: Vec<_> = fields.keys().collect();
            keys.sort();
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push('"');
                out.push_str(k);
                out.push_str("\":");
                if let Some(EntropicState::Valid(v)) = fields.get(*k) {
                    serialize_payload(v, out);
                } else {
                    out.push_str("null");
                }
            }
            out.push('}');
        }
        Payload::Topology(_) | Payload::Tuple(_) | Payload::Range(_, _) => {
            out.push_str("null");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_edge_cases() {
        let json_emoji = "\"Hello \\uD83D\\uDE80 World\"";
        let res = parse_json(json_emoji).unwrap();
        let expected = make_json_value(
            "String",
            Some(Payload::String("Hello 🚀 World".to_string())),
        );
        assert_eq!(res, expected);

        let json_escapes = "\"Line 1\\nLine 2\\t\\\"Quoted\\\"\\\\Backslash\"";
        let res = parse_json(json_escapes).unwrap();
        let expected_escapes = make_json_value(
            "String",
            Some(Payload::String(
                "Line 1\nLine 2\t\"Quoted\"\\Backslash".to_string(),
            )),
        );
        assert_eq!(res, expected_escapes);

        let mut deep = String::new();
        for _ in 0..200 {
            deep.push('[');
        }
        for _ in 0..200 {
            deep.push(']');
        }
        let err = parse_json(&deep);
        assert!(matches!(err, Err(JsonError::ExceededMaxDepth)));

        let nested = "{\"a\": [], \"b\": {}, \"c\": {\"deep\": [1, 2, 3]}}";
        let parsed = parse_json(nested).unwrap();
        let encoded = stringify_json(&parsed);
        let reparsed = parse_json(&encoded).unwrap();
        assert_eq!(parsed, reparsed);
    }
}

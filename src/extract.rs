pub fn string_field(json: &str, field: &str) -> Option<String> {
    let value_start = find_field_value(json, field)?;
    let rest = &json[value_start..];
    let bytes = rest.as_bytes();
    if bytes.first() != Some(&b'"') {
        return None;
    }
    let mut out = String::with_capacity(rest.len().min(64));
    let mut chars = rest[1..].chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => {
                let esc = chars.next()?;
                match esc {
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    'b' => out.push('\u{0008}'),
                    'f' => out.push('\u{000C}'),
                    '\\' => out.push('\\'),
                    '"' => out.push('"'),
                    '/' => out.push('/'),
                    'u' => {
                        let hex: String = chars.by_ref().take(4).collect();
                        if hex.len() != 4 {
                            return None;
                        }
                        let cp = u32::from_str_radix(&hex, 16).ok()?;
                        if let Some(ch) = char::from_u32(cp) {
                            out.push(ch);
                        }
                    }
                    other => {
                        out.push('\\');
                        out.push(other);
                    }
                }
            }
            _ => out.push(c),
        }
    }
    None
}

pub fn objects_in_array<'a>(json: &'a str, array_field: &str) -> Vec<&'a str> {
    let Some(value_start) = find_field_value(json, array_field) else {
        return Vec::new();
    };
    let rest = &json[value_start..];
    let bytes = rest.as_bytes();
    if bytes.first() != Some(&b'[') {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut i = 1;
    while i < bytes.len() {
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r' | b',') {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] == b']' {
            break;
        }
        if bytes[i] != b'{' {
            break;
        }
        let Some(end) = find_object_end(bytes, i) else {
            break;
        };
        out.push(&rest[i..=end]);
        i = end + 1;
    }
    out
}

fn find_object_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut in_str = false;
    let mut esc = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if esc {
            esc = false;
            continue;
        }
        if in_str {
            match b {
                b'\\' => esc = true,
                b'"' => in_str = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn find_field_value(json: &str, field: &str) -> Option<usize> {
    let key = format!("\"{field}\"");
    let bytes = json.as_bytes();
    let mut search_start = 0;
    while let Some(rel) = json[search_start..].find(&key) {
        let pos = search_start + rel;
        let after_key = pos + key.len();
        let mut i = after_key;
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r') {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b':' {
            i += 1;
            while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r') {
                i += 1;
            }
            return Some(i);
        }
        search_start = after_key;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_simple() {
        let j = r#"{"session_id":"abc123"}"#;
        assert_eq!(string_field(j, "session_id").as_deref(), Some("abc123"));
    }

    #[test]
    fn string_with_whitespace() {
        let j = r#"{ "file_path" :  "/x/y.rs" }"#;
        assert_eq!(string_field(j, "file_path").as_deref(), Some("/x/y.rs"));
    }

    #[test]
    fn string_nested() {
        let j = r#"{"tool_input":{"file_path":"/a/b.rs"}}"#;
        assert_eq!(string_field(j, "file_path").as_deref(), Some("/a/b.rs"));
    }

    #[test]
    fn string_escapes() {
        let j = r#"{"x":"line1\nline2\t\"q\""}"#;
        assert_eq!(string_field(j, "x").as_deref(), Some("line1\nline2\t\"q\""));
    }

    #[test]
    fn string_unicode_escape() {
        let j = r#"{"x":"é"}"#;
        assert_eq!(string_field(j, "x").as_deref(), Some("\u{e9}"));
    }

    #[test]
    fn string_missing() {
        let j = r#"{"y":"foo"}"#;
        assert_eq!(string_field(j, "x"), None);
    }

    #[test]
    fn string_non_string_value() {
        let j = r#"{"x":42}"#;
        assert_eq!(string_field(j, "x"), None);
    }

    #[test]
    fn objects_in_array_basic() {
        let j = r#"{"edits":[{"old_string":"a","new_string":"A"},{"old_string":"b","new_string":"B"}]}"#;
        let v = objects_in_array(j, "edits");
        assert_eq!(v.len(), 2);
        assert!(v[0].contains(r#""old_string":"a""#));
        assert!(v[1].contains(r#""old_string":"b""#));
    }

    #[test]
    fn objects_in_array_with_nested_object() {
        let j = r#"{"edits":[{"k":{"nested":"v"},"x":1}]}"#;
        let v = objects_in_array(j, "edits");
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("nested"));
    }

    #[test]
    fn objects_in_array_with_string_containing_brace() {
        let j = r#"{"edits":[{"old_string":"if foo { bar }","new_string":"baz"}]}"#;
        let v = objects_in_array(j, "edits");
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn objects_in_array_empty() {
        let j = r#"{"edits":[]}"#;
        assert!(objects_in_array(j, "edits").is_empty());
    }

    #[test]
    fn objects_in_array_missing() {
        assert!(objects_in_array(r#"{"x":1}"#, "edits").is_empty());
    }

    #[test]
    fn find_object_end_basic() {
        let s = b"{\"a\":1}";
        assert_eq!(find_object_end(s, 0), Some(6));
    }

    #[test]
    fn find_object_end_nested() {
        let s = b"{\"a\":{\"b\":2},\"c\":3}";
        assert_eq!(find_object_end(s, 0), Some(s.len() - 1));
    }

    #[test]
    fn find_object_end_with_brace_in_string() {
        let s = b"{\"a\":\"foo}bar\"}";
        assert_eq!(find_object_end(s, 0), Some(s.len() - 1));
    }

    #[test]
    fn find_object_end_unterminated() {
        let s = b"{\"a\":1";
        assert_eq!(find_object_end(s, 0), None);
    }

    #[test]
    fn find_object_end_escaped_quote_in_string() {
        let s = br#"{"a":"esc\"quote}"}"#;
        assert_eq!(find_object_end(s, 0), Some(s.len() - 1));
    }
}

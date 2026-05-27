use std::fmt::Write as _;

use super::hunks::Hunk;
use crate::extract;

pub fn parse_findings(claude_output: &str, hunks: &[Hunk]) -> Vec<String> {
    let inner =
        extract::string_field(claude_output, "result").unwrap_or_else(|| claude_output.to_string());
    let trimmed = strip_code_fence(&inner);

    let mut out = Vec::new();
    for obj in extract::objects_in_array(trimmed, "findings") {
        let principle = extract_principle(obj).unwrap_or_else(|| "?".to_string());
        let quote = extract::string_field(obj, "quote").unwrap_or_default();
        let why = extract::string_field(obj, "why").unwrap_or_default();
        let file_path = finding_file(obj, hunks);
        out.push(format!(
            "{file_path}: principle {principle} — {quote}\n      → {why}"
        ));
    }
    out
}

pub fn format_findings(findings: &[String]) -> String {
    let mut out = String::from("[no-comment-hook] Comment review findings:\n\n");
    for (i, f) in findings.iter().enumerate() {
        let _ = writeln!(&mut out, "{}. {}", i + 1, f);
    }
    out.push_str(
        "\nThese comments violate good-comment principles. Please revise (or delete) them.\n",
    );
    out
}

fn strip_code_fence(s: &str) -> &str {
    let trimmed = s.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        if let Some(end) = rest.rfind("```") {
            return rest[..end].trim();
        }
    }
    if let Some(rest) = trimmed.strip_prefix("```") {
        if let Some(end) = rest.rfind("```") {
            return rest[..end].trim();
        }
    }
    trimmed
}

fn extract_principle(obj: &str) -> Option<String> {
    if let Some(s) = extract::string_field(obj, "principle") {
        return Some(s);
    }
    extract_usize_field(obj, "principle").map(|n| n.to_string())
}

/// Map a finding to its file via the `hunk` index it was tagged with; fall back
/// to the first hunk when the index is missing or out of range.
fn finding_file<'a>(obj: &str, hunks: &'a [Hunk]) -> &'a str {
    if let Some(idx) = extract_usize_field(obj, "hunk") {
        if let Some(h) = idx.checked_sub(1).and_then(|i| hunks.get(i)) {
            return &h.file_path;
        }
    }
    hunks.first().map_or("?", |h| h.file_path.as_str())
}

fn extract_usize_field(obj: &str, field: &str) -> Option<usize> {
    let marker = format!("\"{field}\":");
    let pos = obj.find(&marker)?;
    let rest = obj[pos + marker.len()..].trim_start();
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(path: &str) -> Vec<Hunk> {
        vec![Hunk {
            file_path: path.into(),
            old_text: None,
            new_text: String::new(),
        }]
    }

    #[test]
    fn parse_findings_handles_empty() {
        let claude = r#"{"result":"{\"findings\":[]}"}"#;
        assert!(parse_findings(claude, &one("/x.rs")).is_empty());
    }

    #[test]
    fn parse_findings_extracts_single() {
        let claude = r#"{"result":"{\"findings\":[{\"hunk\":1,\"principle\":3,\"quote\":\"// Pin: third arm\",\"why\":\"meta-framing\"}]}"}"#;
        let f = parse_findings(claude, &one("/x.rs"));
        assert_eq!(f.len(), 1);
        assert!(f[0].contains("/x.rs"));
        assert!(f[0].contains("principle 3"));
        assert!(f[0].contains("// Pin: third arm"));
        assert!(f[0].contains("meta-framing"));
    }

    #[test]
    fn parse_findings_routes_by_hunk_index() {
        let claude = r#"{"result":"{\"findings\":[{\"hunk\":2,\"principle\":1,\"quote\":\"q\",\"why\":\"w\"}]}"}"#;
        let hunks = vec![
            Hunk {
                file_path: "/a.rs".into(),
                old_text: None,
                new_text: String::new(),
            },
            Hunk {
                file_path: "/b.rs".into(),
                old_text: None,
                new_text: String::new(),
            },
        ];
        let f = parse_findings(claude, &hunks);
        assert_eq!(f.len(), 1);
        assert!(f[0].contains("/b.rs"));
    }

    #[test]
    fn parse_findings_missing_hunk_falls_back_to_first() {
        let claude =
            r#"{"result":"{\"findings\":[{\"principle\":1,\"quote\":\"q\",\"why\":\"w\"}]}"}"#;
        let f = parse_findings(claude, &one("/only.rs"));
        assert!(f[0].contains("/only.rs"));
    }

    #[test]
    fn parse_findings_handles_string_principle() {
        let claude = r#"{"result":"{\"findings\":[{\"hunk\":1,\"principle\":\"test-meta\",\"quote\":\"// x\",\"why\":\"y\"}]}"}"#;
        let f = parse_findings(claude, &one("/x.rs"));
        assert_eq!(f.len(), 1);
        assert!(f[0].contains("principle test-meta"));
    }

    #[test]
    fn parse_findings_handles_markdown_fence() {
        let claude = r#"{"result":"```json\n{\"findings\":[{\"hunk\":1,\"principle\":1,\"quote\":\"a\",\"why\":\"b\"}]}\n```"}"#;
        let f = parse_findings(claude, &one("/x.rs"));
        assert_eq!(f.len(), 1);
    }

    #[test]
    fn parse_findings_handles_raw_output_no_wrapper() {
        let raw = r#"{"findings":[{"hunk":1,"principle":1,"quote":"a","why":"b"}]}"#;
        let f = parse_findings(raw, &one("/x.rs"));
        assert_eq!(f.len(), 1);
    }

    #[test]
    fn format_findings_includes_header_and_footer() {
        let findings = vec!["foo".to_string(), "bar".to_string()];
        let s = format_findings(&findings);
        assert!(s.contains("Comment review findings:"));
        assert!(s.contains("1. foo"));
        assert!(s.contains("2. bar"));
        assert!(s.contains("violate good-comment principles"));
    }

    #[test]
    fn strip_code_fence_strips_json_fence() {
        assert_eq!(strip_code_fence("```json\n{\"a\":1}\n```"), "{\"a\":1}");
    }

    #[test]
    fn strip_code_fence_strips_bare_fence() {
        assert_eq!(strip_code_fence("```\n{\"a\":1}\n```"), "{\"a\":1}");
    }

    #[test]
    fn strip_code_fence_passthrough_when_no_fence() {
        assert_eq!(strip_code_fence("{\"a\":1}"), "{\"a\":1}");
    }
}

use std::fmt::Write as _;

use crate::extract;

pub fn parse_findings(claude_output: &str, file_path: &str) -> Vec<String> {
    let inner =
        extract::string_field(claude_output, "result").unwrap_or_else(|| claude_output.to_string());
    let trimmed = strip_code_fence(&inner);

    let mut out = Vec::new();
    for obj in extract::objects_in_array(trimmed, "findings") {
        let principle = extract_principle(obj).unwrap_or_else(|| "?".to_string());
        let quote = extract::string_field(obj, "quote").unwrap_or_default();
        let why = extract::string_field(obj, "why").unwrap_or_default();
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
    let marker = "\"principle\":";
    let pos = obj.find(marker)?;
    let rest = obj[pos + marker.len()..].trim_start();
    let n: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if n.is_empty() { None } else { Some(n) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_findings_handles_empty() {
        let claude = r#"{"result":"{\"findings\":[]}"}"#;
        assert!(parse_findings(claude, "/x.rs").is_empty());
    }

    #[test]
    fn parse_findings_extracts_single() {
        let claude = r#"{"result":"{\"findings\":[{\"principle\":3,\"quote\":\"// Pin: third arm\",\"why\":\"meta-framing\"}]}"}"#;
        let f = parse_findings(claude, "/x.rs");
        assert_eq!(f.len(), 1);
        assert!(f[0].contains("principle 3"));
        assert!(f[0].contains("// Pin: third arm"));
        assert!(f[0].contains("meta-framing"));
    }

    #[test]
    fn parse_findings_handles_string_principle() {
        let claude = r#"{"result":"{\"findings\":[{\"principle\":\"test-meta\",\"quote\":\"// x\",\"why\":\"y\"}]}"}"#;
        let f = parse_findings(claude, "/x.rs");
        assert_eq!(f.len(), 1);
        assert!(f[0].contains("principle test-meta"));
    }

    #[test]
    fn parse_findings_handles_markdown_fence() {
        let claude = r#"{"result":"```json\n{\"findings\":[{\"principle\":1,\"quote\":\"a\",\"why\":\"b\"}]}\n```"}"#;
        let f = parse_findings(claude, "/x.rs");
        assert_eq!(f.len(), 1);
    }

    #[test]
    fn parse_findings_handles_raw_output_no_wrapper() {
        let raw = r#"{"findings":[{"principle":1,"quote":"a","why":"b"}]}"#;
        let f = parse_findings(raw, "/x.rs");
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

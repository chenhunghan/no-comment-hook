use std::fmt::Write as _;

use super::candidates::Candidate;
use crate::extract;
use crate::options::Options;

/// One line per finding, keyed `{file}: principle {p} — {quote}` (the deferral
/// store and the eval harness both read that first line), then the reason.
pub fn parse_findings(
    claude_output: &str,
    candidates: &[Candidate],
    opts: &Options,
) -> Vec<String> {
    let inner =
        extract::string_field(claude_output, "result").unwrap_or_else(|| claude_output.to_string());
    let trimmed = strip_code_fence(&inner);

    let mut out: Vec<String> = Vec::new();
    for obj in extract::objects_in_array(trimmed, "findings") {
        let principle = extract_principle(obj).unwrap_or_else(|| "?".to_string());
        if !opts.principle_enabled(&principle) {
            continue;
        }
        let Some(c) = extract_usize_field(obj, "comment")
            .and_then(|n| n.checked_sub(1))
            .and_then(|i| candidates.get(i))
        else {
            continue;
        };
        let why = extract::string_field(obj, "why").unwrap_or_default();
        let finding = format!(
            "{}: principle {principle} — {}\n      → line {}: {why}",
            c.file_path,
            c.quote(),
            c.start_line
        );
        if !out.contains(&finding) {
            out.push(finding);
        }
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

    fn cands() -> Vec<Candidate> {
        ["/a.rs", "/b.rs"]
            .iter()
            .enumerate()
            .map(|(i, p)| Candidate {
                file_path: (*p).into(),
                start_line: 10 + i,
                end_line: 10 + i,
                text: format!("// comment {i}\n// more"),
                excerpt: String::new(),
            })
            .collect()
    }

    #[test]
    fn parse_findings_handles_empty() {
        let claude = r#"{"result":"{\"findings\":[]}"}"#;
        assert_eq!(
            parse_findings(claude, &cands(), &Options::default()),
            Vec::<String>::new()
        );
    }

    #[test]
    fn parse_findings_names_file_line_and_quote_from_the_comment() {
        let claude = r#"{"result":"{\"findings\":[{\"comment\":2,\"principle\":3,\"why\":\"meta-framing\"}]}"}"#;
        let f = parse_findings(claude, &cands(), &Options::default());
        assert_eq!(
            f,
            ["/b.rs: principle 3 — // comment 1\n      → line 11: meta-framing"]
        );
    }

    #[test]
    fn parse_findings_drops_unknown_or_missing_comment_numbers() {
        let claude =
            r#"{"findings":[{"comment":9,"principle":1,"why":"w"},{"principle":1,"why":"w"}]}"#;
        assert_eq!(
            parse_findings(claude, &cands(), &Options::default()),
            Vec::<String>::new()
        );
    }

    #[test]
    fn parse_findings_dedupes_repeats() {
        let claude = r#"{"findings":[{"comment":1,"principle":1,"why":"w"},{"comment":1,"principle":1,"why":"w"}]}"#;
        assert_eq!(
            parse_findings(claude, &cands(), &Options::default()).len(),
            1
        );
    }

    #[test]
    fn parse_findings_handles_string_principle() {
        let claude = r#"{"findings":[{"comment":1,"principle":"change-narration","why":"y"}]}"#;
        let f = parse_findings(claude, &cands(), &Options::default());
        assert!(f[0].contains("principle change-narration"));
    }

    #[test]
    fn parse_findings_handles_markdown_fence() {
        let claude = r#"{"result":"```json\n{\"findings\":[{\"comment\":1,\"principle\":1,\"why\":\"b\"}]}\n```"}"#;
        assert_eq!(
            parse_findings(claude, &cands(), &Options::default()).len(),
            1
        );
    }

    #[test]
    fn parse_findings_drops_disabled_principle() {
        let claude = r#"{"findings":[{"comment":1,"principle":1,"why":"w"}]}"#;
        let opts = Options {
            disabled: vec!["redundant".into()],
            ..Options::default()
        };
        assert_eq!(
            parse_findings(claude, &cands(), &opts),
            Vec::<String>::new()
        );
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

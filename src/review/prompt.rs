use std::fmt::Write as _;

use super::hunks::{Hunk, line_count};
use crate::options::{Options, PRINCIPLES};

const PROMPT_RULES: &str = r#"You are reviewing comments in newly-written code against principles of good code comments.

RULES:
- Review ONLY comments on lines marked [NEW].
- Lines without [NEW] are surrounding context. Use them to judge whether a [NEW] comment restates a nearby identifier. NEVER flag a comment that is not on a [NEW] line.
- Public API docstrings (Rust /// on `pub` items, JSDoc on exported declarations, Python module-level docstrings) get a carve-out from principles 6 and 9. Do not flag those two principles against documentation on public items.
- "Comment" means any text the language treats as a comment (// /* */ # """ etc.).

MASTER HEURISTIC (the reset test):
Would this comment make sense if the commit history were deleted and the code had always been shaped this way? If no, the comment is session documentation and should be flagged under the most-applicable principle below.

PRINCIPLES (numbered, only the ones listed below are active for this review):
"#;

const PROMPT_OUTPUT: &str = r#"
OUTPUT (strict JSON, no preamble, no markdown fences):
- No violations: {"findings":[]}
- With violations: {"findings":[{"principle":<n>,"quote":"<comment text excerpt, <=120 chars>","why":"<one short sentence>"}]}

"#;

pub fn build_prompt(hunk: &Hunk, opts: &Options) -> String {
    let mut p = String::with_capacity(2048);
    p.push_str(PROMPT_RULES);
    p.push_str(&principles_text(opts));
    p.push_str(PROMPT_OUTPUT);
    p.push_str(&build_review_packet(hunk, opts));
    p
}

fn principles_text(opts: &Options) -> String {
    let mut out = String::new();
    for p in PRINCIPLES {
        if opts.is_enabled(p.key) {
            let _ = writeln!(&mut out, "{}. {} — {}", p.number, p.name, p.detail);
        }
    }
    out
}

fn build_review_packet(hunk: &Hunk, opts: &Options) -> String {
    let file_text = std::fs::read_to_string(&hunk.file_path).ok();
    if let Some(content) = file_text {
        if let Some(range) = locate_new_lines(&content, &hunk.new_text) {
            return format_with_context(&content, range, &hunk.file_path, opts.context_lines);
        }
    }
    format_without_context(&hunk.file_path, &hunk.new_text)
}

fn locate_new_lines(file: &str, new_text: &str) -> Option<(usize, usize)> {
    let pos = file.find(new_text)?;
    let start_line = file[..pos].matches('\n').count() + 1;
    let new_line_count = line_count(new_text).max(1);
    Some((start_line, start_line + new_line_count - 1))
}

fn format_with_context(
    file: &str,
    (start, end): (usize, usize),
    file_path: &str,
    ctx: usize,
) -> String {
    let lines: Vec<&str> = file.lines().collect();
    let from = start.saturating_sub(ctx).max(1);
    let to = end.saturating_add(ctx).min(lines.len());

    let mut out = format!("FILE: {file_path}\n\nCODE:\n");
    for (idx, line) in lines.iter().enumerate() {
        let lineno = idx + 1;
        if lineno < from || lineno > to {
            continue;
        }
        let marker = if lineno >= start && lineno <= end {
            "[NEW] "
        } else {
            "      "
        };
        let _ = writeln!(&mut out, "{lineno:>5}: {marker}{line}");
    }
    out
}

fn format_without_context(file_path: &str, new_text: &str) -> String {
    let mut out = format!("FILE: {file_path}\n\nCODE (all lines under review):\n");
    for (i, line) in new_text.lines().enumerate() {
        let lineno = i + 1;
        let _ = writeln!(&mut out, "{lineno:>5}: [NEW] {line}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locate_new_lines_finds_match() {
        let file = "line1\nline2\nline3\nline4\n";
        assert_eq!(locate_new_lines(file, "line2\nline3"), Some((2, 3)));
    }

    #[test]
    fn locate_new_lines_missing_returns_none() {
        let file = "line1\nline2\n";
        assert_eq!(locate_new_lines(file, "missing"), None);
    }

    #[test]
    fn format_with_context_marks_new() {
        let file = "a\nb\nc\nd\ne\nf\ng\n";
        let out = format_with_context(file, (3, 4), "/x.rs", 1);
        assert!(out.contains("    2:       b"));
        assert!(out.contains("    3: [NEW] c"));
        assert!(out.contains("    4: [NEW] d"));
        assert!(out.contains("    5:       e"));
        assert!(!out.contains("    6"));
    }

    #[test]
    fn format_without_context_marks_all_new() {
        let out = format_without_context("/x.rs", "hello\nworld");
        assert!(out.contains("    1: [NEW] hello"));
        assert!(out.contains("    2: [NEW] world"));
    }

    #[test]
    fn principles_text_omits_disabled() {
        let opts = Options {
            disabled: vec!["defensive".into()],
            ..Options::default()
        };
        let text = principles_text(&opts);
        assert!(!text.contains("Defensive justification"));
        assert!(text.contains("Process vocabulary"));
    }

    #[test]
    fn principles_text_all_enabled_by_default() {
        let opts = Options::default();
        let text = principles_text(&opts);
        for p in PRINCIPLES {
            assert!(
                text.contains(p.name),
                "expected principle '{}' in prompt",
                p.name
            );
        }
    }
}

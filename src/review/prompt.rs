use std::fmt::Write as _;

use super::hunks::{Hunk, line_count};
use super::prefilter::strip_directive_lines;
use crate::options::{Options, PRINCIPLES};

const PROMPT_RULES: &str = r#"You are a deterministic classifier for comments in newly-written code, judged against principles of good code comments. Output only JSON.

RULES:
- Review ONLY comments on lines marked [NEW].
- Lines without [NEW] are surrounding context. Use them to judge whether a [NEW] comment restates a nearby identifier. NEVER flag a comment that is not on a [NEW] line.
- Public API documentation is GOOD, not a smell. NEVER flag `redundant` or `over-explained` against doc comments on exported/public items — Rust `///` on `pub` items, Go doc comments on exported (capitalized) identifiers (e.g. `// ActiveSessions returns ...`), JSDoc/TSDoc on exported declarations (`/** ... */`), and Python module/class/function docstrings — even when they restate the name; that is the required convention for those languages.
- "Comment" means any text the language treats as a comment (// /* */ # """ etc.).
- The input may contain multiple hunks, each introduced by a line of the form "===== HUNK <n>: <file> =====". Tag every finding with its hunk number <n>.

MASTER HEURISTIC (the reset test):
Would this comment make sense if the commit history were deleted and the code had always been shaped this way? If no, the comment is session documentation and should be flagged under the most-applicable principle below.

PRINCIPLES (numbered, only the ones listed below are active for this review):
"#;

const PROMPT_OUTPUT: &str = r#"
OUTPUT (strict JSON, no preamble, no markdown fences):
- No violations: {"findings":[]}
- With violations: {"findings":[{"hunk":<n>,"principle":<n>,"quote":"<comment text excerpt, <=120 chars>","why":"<one short sentence>"}]}
"#;

pub fn build_system_prompt(opts: &Options) -> String {
    let mut p = String::with_capacity(2048);
    p.push_str(PROMPT_RULES);
    p.push_str(&principles_text(opts));
    p.push_str(PROMPT_OUTPUT);
    p
}

pub fn build_user_message(hunks: &[Hunk], opts: &Options) -> String {
    let mut out = String::with_capacity(2048);
    for (i, hunk) in hunks.iter().enumerate() {
        let _ = writeln!(
            &mut out,
            "===== HUNK {n}: {path} =====",
            n = i + 1,
            path = hunk.file_path
        );
        out.push_str(&build_review_packet(hunk, opts));
        out.push('\n');
    }
    out
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
    let stripped_new = strip_directive_lines(&hunk.new_text, &hunk.file_path);
    let file_text = std::fs::read_to_string(&hunk.file_path).ok();
    if let Some(content) = file_text {
        let stripped_file = strip_directive_lines(&content, &hunk.file_path);
        if let Some(range) = locate_new_lines(&stripped_file, &stripped_new) {
            return format_with_context(&stripped_file, range, opts.context_lines);
        }
    }
    format_without_context(&stripped_new)
}

fn locate_new_lines(file: &str, new_text: &str) -> Option<(usize, usize)> {
    let pos = file.find(new_text)?;
    let start_line = file[..pos].matches('\n').count() + 1;
    let new_line_count = line_count(new_text).max(1);
    Some((start_line, start_line + new_line_count - 1))
}

fn format_with_context(file: &str, (start, end): (usize, usize), ctx: usize) -> String {
    let lines: Vec<&str> = file.lines().collect();
    let from = start.saturating_sub(ctx).max(1);
    let to = end.saturating_add(ctx).min(lines.len());

    let mut out = String::from("CODE:\n");
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

fn format_without_context(new_text: &str) -> String {
    let mut out = String::from("CODE (all lines under review):\n");
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
        let out = format_with_context(file, (3, 4), 1);
        assert!(out.contains("    2:       b"));
        assert!(out.contains("    3: [NEW] c"));
        assert!(out.contains("    4: [NEW] d"));
        assert!(out.contains("    5:       e"));
        assert!(!out.contains("    6"));
    }

    #[test]
    fn format_without_context_marks_all_new() {
        let out = format_without_context("hello\nworld");
        assert!(out.contains("    1: [NEW] hello"));
        assert!(out.contains("    2: [NEW] world"));
    }

    #[test]
    fn user_message_labels_each_hunk() {
        let hunks = vec![
            Hunk {
                file_path: "/nonexistent-a.rs".into(),
                old_text: None,
                new_text: "// a\nlet x = 1;".into(),
            },
            Hunk {
                file_path: "/nonexistent-b.rs".into(),
                old_text: None,
                new_text: "// b\nlet y = 2;".into(),
            },
        ];
        let msg = build_user_message(&hunks, &Options::default());
        assert!(msg.contains("===== HUNK 1: /nonexistent-a.rs ====="));
        assert!(msg.contains("===== HUNK 2: /nonexistent-b.rs ====="));
        assert!(msg.contains("[NEW] // a"));
        assert!(msg.contains("[NEW] // b"));
    }

    #[test]
    fn principles_text_omits_disabled() {
        let opts = Options {
            disabled: vec!["over-explained".into()],
            ..Options::default()
        };
        let text = principles_text(&opts);
        assert!(!text.contains("Over-explained"));
        assert!(text.contains("Change or task narration"));
    }

    fn temp_file(name: &str, ext: &str, content: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "no-comment-prompt-{}-{}-{}.{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
            ext,
        ));
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn review_packet_hides_safety_directive_and_keeps_real_comment() {
        let file = "fn outer() {\n    let p = q;\n    // SAFETY: p is NUL-terminated and the C call only reads it.\n    // The pointer outlives the call, so no dangling access is possible.\n    let rc = unsafe { libc::chroot(p) };\n    // this comment is the real review target\n    return rc;\n}\n";
        let new_text = "    let p = q;\n    // SAFETY: p is NUL-terminated and the C call only reads it.\n    // The pointer outlives the call, so no dangling access is possible.\n    let rc = unsafe { libc::chroot(p) };\n    // this comment is the real review target\n";
        let path = temp_file("safety-mixed", "rs", file);

        let hunks = vec![Hunk {
            file_path: path.to_string_lossy().into_owned(),
            old_text: Some("let p = q;".into()),
            new_text: new_text.into(),
        }];
        let msg = build_user_message(&hunks, &Options::default());

        assert!(
            !msg.contains("SAFETY"),
            "directive must not appear in review packet:\n{msg}"
        );
        assert!(
            !msg.contains("dangling access"),
            "multi-line SAFETY continuation must also be hidden:\n{msg}"
        );
        assert!(
            msg.contains("this comment is the real review target"),
            "real comment must survive:\n{msg}"
        );
        assert!(
            msg.contains("fn outer() {") && msg.contains("return rc;"),
            "surrounding file context must be present:\n{msg}"
        );
        assert!(
            msg.contains("[NEW] "),
            "edit lines must still be marked [NEW]:\n{msg}"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn system_prompt_lists_all_principles_by_default() {
        let text = build_system_prompt(&Options::default());
        for p in PRINCIPLES {
            assert!(
                text.contains(p.name),
                "expected principle '{}' in system prompt",
                p.name
            );
        }
    }
}

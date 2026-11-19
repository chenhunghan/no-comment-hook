use std::fmt::Write as _;

use super::candidates::Candidate;
use crate::options::{Options, PRINCIPLES};

const PROMPT_RULES: &str = r#"You are a deterministic classifier for comments in newly-written code, judged against principles of good code comments. Output only JSON.

RULES:
- The input is a list of comment blocks, each introduced by a line of the form "===== COMMENT <n>: <file>:<lines> =====" and followed by an excerpt of the file.
- In each excerpt, the lines marked ">>" are the comment under review. Every other line is surrounding code, shown so you can judge whether the comment restates it. Judge ONLY the marked comment, and tag every finding with its comment number <n>.
- Public API documentation is GOOD, not a smell. NEVER flag `redundant` or `over-explained` against doc comments on exported/public items — Rust `///` on `pub` items, Go doc comments on exported (capitalized) identifiers (e.g. `// ActiveSessions returns ...`), JSDoc/TSDoc on exported declarations (`/** ... */`), and Python module/class/function docstrings — even when they restate the name; that is the required convention for those languages.

MASTER HEURISTIC (the reset test):
Would this comment make sense if the commit history were deleted and the code had always been shaped this way? If no, the comment is session documentation and should be flagged under the most-applicable principle below.

PRINCIPLES (numbered, only the ones listed below are active for this review):
"#;

const PROMPT_OUTPUT: &str = r#"
OUTPUT (strict JSON, no preamble, no markdown fences):
- No violations: {"findings":[]}
- With violations: {"findings":[{"comment":<n>,"principle":<n>,"why":"<one short sentence>"}]}
"#;

pub fn build_system_prompt(opts: &Options) -> String {
    let mut p = String::with_capacity(2048);
    p.push_str(PROMPT_RULES);
    p.push_str(&principles_text(opts));
    p.push_str(PROMPT_OUTPUT);
    p
}

pub fn build_user_message(candidates: &[Candidate]) -> String {
    let mut out = String::with_capacity(2048);
    for (i, c) in candidates.iter().enumerate() {
        let lines = if c.start_line == c.end_line {
            c.start_line.to_string()
        } else {
            format!("{}-{}", c.start_line, c.end_line)
        };
        let _ = writeln!(
            out,
            "===== COMMENT {n}: {path}:{lines} =====",
            n = i + 1,
            path = c.file_path
        );
        out.push_str(&c.excerpt);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(path: &str, start: usize, end: usize, text: &str) -> Candidate {
        Candidate {
            file_path: path.into(),
            start_line: start,
            end_line: end,
            text: text.into(),
            excerpt: format!("{start:>5} >> {text}\n"),
        }
    }

    #[test]
    fn user_message_numbers_each_comment_with_its_location() {
        let msg = build_user_message(&[cand("/a.rs", 3, 3, "// a"), cand("/b.py", 7, 9, "# b")]);
        assert!(msg.contains("===== COMMENT 1: /a.rs:3 ====="));
        assert!(msg.contains("===== COMMENT 2: /b.py:7-9 ====="));
        assert!(msg.contains("    3 >> // a"));
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

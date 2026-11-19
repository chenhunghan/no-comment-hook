//! Turns the turn's Edit/Write records into the comment blocks the agent wrote:
//! the lines each edit actually changed, intersected with the comments the
//! lexer finds in the file as it stands now.

use std::fmt::Write as _;

use crate::collect::RecordOwned;
use crate::lexer::{self, Comment, Kind};

pub const MAX_EDIT_LINES: usize = 5_000;
pub const MAX_WRITE_LINES: usize = 10_000;

/// One comment block for the reviewer: a single comment, or a run of
/// whole-line comments on consecutive lines.
pub struct Candidate {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
    pub excerpt: String,
}

impl Candidate {
    /// The block's first line, for naming it in a finding.
    pub fn quote(&self) -> String {
        let first = self.text.lines().next().unwrap_or_default().trim();
        first.chars().take(120).collect()
    }
}

pub fn collect(records: &[RecordOwned], context_lines: usize) -> Vec<Candidate> {
    let mut files: Vec<(&str, Vec<&RecordOwned>)> = Vec::new();
    for r in records {
        match files.iter_mut().find(|(p, _)| *p == r.file_path) {
            Some((_, rs)) => rs.push(r),
            None => files.push((&r.file_path, vec![r])),
        }
    }
    let mut out = Vec::new();
    for (path, recs) in files {
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        let ranges: Vec<(usize, usize)> = recs
            .iter()
            .filter_map(|r| changed_range(r, &content))
            .collect();
        if ranges.is_empty() {
            continue;
        }
        let lines = LineIndex::new(&content);
        for block in blocks(&content, path, &lines, &ranges) {
            out.push(Candidate {
                file_path: path.to_string(),
                excerpt: excerpt(&content, block.start_line, block.end_line, context_lines),
                text: content[block.start..block.end].to_string(),
                start_line: block.start_line,
                end_line: block.end_line,
            });
        }
    }
    out
}

/// The 1-based, inclusive line range of `content` that a record wrote.
fn changed_range(r: &RecordOwned, content: &str) -> Option<(usize, usize)> {
    match r.tool.as_str() {
        "Write" => {
            let written = r.content.as_deref()?;
            if line_count(written) > MAX_WRITE_LINES || has_conflict_markers(written) {
                return None;
            }
            Some((1, line_count(content).max(1)))
        }
        "Edit" => {
            let (old, new) = (r.old_string.as_deref()?, r.new_string.as_deref()?);
            if line_count(new) > MAX_EDIT_LINES
                || has_conflict_markers(old)
                || has_conflict_markers(new)
            {
                return None;
            }
            let (first, last) = changed_lines(old, new)?;
            let at = content.find(new)?;
            let base = content[..at].matches('\n').count();
            Some((base + first, base + last))
        }
        _ => None,
    }
}

/// The lines of `new` (1-based, inclusive) that differ from `old`, once the
/// lines both share at the start and end are set aside: an Edit repeats
/// untouched lines around its change to anchor it, and those are not new.
fn changed_lines(old: &str, new: &str) -> Option<(usize, usize)> {
    let o: Vec<&str> = old.split('\n').collect();
    let n: Vec<&str> = new.split('\n').collect();
    let prefix = o.iter().zip(&n).take_while(|(a, b)| a == b).count();
    let max_suffix = o.len().min(n.len()) - prefix;
    let suffix = o
        .iter()
        .rev()
        .zip(n.iter().rev())
        .take(max_suffix)
        .take_while(|(a, b)| a == b)
        .count();
    let end = n.len() - suffix;
    (prefix < end).then_some((prefix + 1, end))
}

fn line_count(s: &str) -> usize {
    if s.is_empty() {
        0
    } else {
        s.matches('\n').count() + usize::from(!s.ends_with('\n'))
    }
}

/// A merge-resolution edit rewrites conflict hunks wholesale; the comments in
/// it were already there, so reviewing them would blame the agent for others'.
fn has_conflict_markers(text: &str) -> bool {
    let (mut start, mut sep, mut end) = (false, false, false);
    for line in text.lines() {
        if line.starts_with("<<<<<<< ") {
            start = true;
        } else if line.trim_end() == "=======" {
            sep = true;
        } else if line.starts_with(">>>>>>> ") {
            end = true;
        }
    }
    start && sep && end
}

struct LineIndex(Vec<usize>);

impl LineIndex {
    fn new(s: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(s.match_indices('\n').map(|(i, _)| i + 1));
        LineIndex(starts)
    }

    /// 1-based line holding byte `pos`.
    fn line(&self, pos: usize) -> usize {
        self.0.partition_point(|&s| s <= pos)
    }
}

struct Block {
    start: usize,
    end: usize,
    start_line: usize,
    end_line: usize,
}

/// Groups the comments on changed lines into review blocks, dropping tool
/// directives. Only changed comments join a block, so a comment the agent
/// added under an existing one is judged, and quoted, on its own.
fn blocks(content: &str, path: &str, lines: &LineIndex, changed: &[(usize, usize)]) -> Vec<Block> {
    let ext = file_ext_lower(path);
    let mut out: Vec<Block> = Vec::new();
    let mut prev: Option<(Comment, bool)> = None;
    let mut in_safety = false;
    for c in lexer::comments(content, path) {
        let text = &content[c.start..c.end];
        let own_line = own_line(content, c.start);
        let joins = prev.is_some_and(|(p, p_own)| {
            p_own
                && own_line
                && p.kind == Kind::Line
                && c.kind == Kind::Line
                && marker(&content[p.start..p.end]) == marker(text)
                && content[p.end..c.start].matches('\n').count() == 1
                && content[p.end..c.start].trim().is_empty()
        });
        prev = Some((c, own_line));
        if in_safety && joins && is_safety_continuation(text, &ext) {
            continue;
        }
        in_safety = is_rust_safety_comment(text, &ext);
        if in_safety || is_directive(text, &ext) {
            continue;
        }
        let start_line = lines.line(c.start);
        let end_line = lines.line(c.end.saturating_sub(1).max(c.start));
        if !changed
            .iter()
            .any(|&(a, b)| start_line <= b && a <= end_line)
        {
            continue;
        }
        match out.last_mut() {
            Some(b) if joins && b.end_line + 1 == start_line => {
                b.end = c.end;
                b.end_line = end_line;
            }
            _ => out.push(Block {
                start: c.start,
                end: c.end,
                start_line,
                end_line,
            }),
        }
    }
    out
}

fn own_line(content: &str, pos: usize) -> bool {
    content[..pos]
        .rsplit('\n')
        .next()
        .is_some_and(|before| before.trim().is_empty())
}

/// The comment's leading marker (`//`, `///`, `//!`, `#`, …), so a doc
/// comment run never merges with an ordinary one beside it.
fn marker(text: &str) -> &str {
    let len = text
        .bytes()
        .take(3)
        .take_while(|b| !b.is_ascii_alphanumeric() && !b.is_ascii_whitespace())
        .count();
    &text[..len]
}

fn excerpt(content: &str, start: usize, end: usize, ctx: usize) -> String {
    let from = start.saturating_sub(ctx).max(1);
    let to = end + ctx;
    let mut out = String::new();
    for (idx, line) in content.lines().enumerate() {
        let n = idx + 1;
        if n < from {
            continue;
        }
        if n > to {
            break;
        }
        let mark = if (start..=end).contains(&n) {
            ">>"
        } else {
            "  "
        };
        let _ = writeln!(out, "{n:>5} {mark} {line}");
    }
    out
}

fn file_ext_lower(file_path: &str) -> String {
    std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default()
}

// Tool and compiler directives are instructions to a linter, type checker or
// compiler, not prose for a reader, so no principle applies to them.

fn is_directive(text: &str, ext: &str) -> bool {
    let t = text.trim_start();
    match ext {
        "go" => t.starts_with("//go:"),
        "py" | "pyi" => is_python_directive(t),
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" => is_ts_directive(t),
        "c" | "cpp" | "cc" | "h" | "hpp" | "cxx" | "hh" | "hxx" => is_c_directive(t),
        "rb" | "rake" => is_ruby_directive(t),
        _ => false,
    }
}

fn is_rust_safety_comment(text: &str, ext: &str) -> bool {
    let t = text.trim_start();
    ext == "rs" && (t.starts_with("// SAFETY:") || t.starts_with("//SAFETY:"))
}

// clippy is satisfied by `SAFETY:` on a block's first line alone, so the
// continuation `//` lines are part of the same justification; `///`/`//!` are not.
fn is_safety_continuation(text: &str, ext: &str) -> bool {
    ext == "rs" && text.starts_with("//") && !text.starts_with("///") && !text.starts_with("//!")
}

fn is_python_directive(t: &str) -> bool {
    let Some(rest) = t.strip_prefix('#') else {
        return false;
    };
    let rest = rest.trim_start();
    [
        "type:", "noqa", "pylint:", "pragma:", "fmt:", "mypy:", "ruff:",
    ]
    .iter()
    .any(|p| rest.starts_with(p))
}

fn is_ts_directive(t: &str) -> bool {
    [
        "// @ts-",
        "//@ts-",
        "/// <reference",
        "// eslint-disable",
        "// eslint-enable",
        "/* eslint-disable",
        "/* eslint-enable",
        "// prettier-ignore",
        "// biome-ignore",
    ]
    .iter()
    .any(|p| t.starts_with(p))
}

fn is_c_directive(t: &str) -> bool {
    t.starts_with("// NOLINT") || t.starts_with("// clang-format")
}

fn is_ruby_directive(t: &str) -> bool {
    let Some(rest) = t.strip_prefix('#') else {
        return false;
    };
    let rest = rest.trim_start();
    [
        "frozen_string_literal:",
        "encoding:",
        "coding:",
        "rubocop:",
        "shareable_constant_value:",
        "warn_indent:",
    ]
    .iter()
    .any(|p| rest.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, ext: &str, content: &str) -> String {
        let path = std::env::temp_dir().join(format!(
            "no-comment-cand-{name}-{}-{}.{ext}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
        ));
        std::fs::write(&path, content).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn edit(path: &str, old: &str, new: &str) -> RecordOwned {
        RecordOwned {
            tool: "Edit".into(),
            file_path: path.into(),
            old_string: Some(old.into()),
            new_string: Some(new.into()),
            content: None,
        }
    }

    fn write(path: &str, content: &str) -> RecordOwned {
        RecordOwned {
            tool: "Write".into(),
            file_path: path.into(),
            old_string: None,
            new_string: None,
            content: Some(content.into()),
        }
    }

    fn texts(c: &[Candidate]) -> Vec<&str> {
        c.iter().map(|c| c.text.as_str()).collect()
    }

    #[test]
    fn changed_lines_drops_shared_anchor_lines() {
        assert_eq!(changed_lines("a\nb\nc", "a\nB\nc"), Some((2, 2)));
        assert_eq!(changed_lines("a\nc", "a\nb\nc"), Some((2, 2)));
        assert_eq!(changed_lines("a\nb\nc", "a\nc"), None);
        assert_eq!(changed_lines("x", "y"), Some((1, 1)));
        assert_eq!(changed_lines("a\na", "a\na\na"), Some((3, 3)));
    }

    #[test]
    fn edit_reviews_only_comments_on_changed_lines() {
        let file = "def total(items):\n    # loop over the items and add them up\n    return sum(i.price * i.qty for i in items)\n";
        let p = temp_file("anchor", "py", file);
        let old =
            "    # loop over the items and add them up\n    return sum(i.price for i in items)";
        let new = "    # loop over the items and add them up\n    return sum(i.price * i.qty for i in items)";
        assert!(collect(&[edit(&p, old, new)], 2).is_empty());

        let file2 = "def total(items):\n    # multiply by quantity now\n    return sum(i.price * i.qty for i in items)\n";
        std::fs::write(&p, file2).unwrap();
        let new2 = "    # multiply by quantity now\n    return sum(i.price * i.qty for i in items)";
        let c = collect(&[edit(&p, old, new2)], 2);
        assert_eq!(texts(&c), ["# multiply by quantity now"]);
        assert_eq!((c[0].start_line, c[0].end_line), (2, 2));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn write_reviews_every_comment_and_skips_strings() {
        let file = "url = 'https://x.io/#frag'  # real\ns = \"# not a comment\"\n";
        let p = temp_file("write", "py", file);
        assert_eq!(texts(&collect(&[write(&p, file)], 0)), ["# real"]);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn edit_no_longer_in_file_is_skipped() {
        let p = temp_file("gone", "rs", "fn a() {}\n");
        assert!(collect(&[edit(&p, "x", "// gone\nfn b() {}")], 0).is_empty());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn consecutive_line_comments_form_one_block() {
        let file = "// one\n// two\nlet x = 1; // trailing\n\n// three\n/// doc\nfn f() {}\n";
        let p = temp_file("blocks", "rs", file);
        let c = collect(&[write(&p, file)], 0);
        assert_eq!(
            texts(&c),
            ["// one\n// two", "// trailing", "// three", "/// doc"]
        );
        assert_eq!((c[0].start_line, c[0].end_line), (1, 2));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn a_new_comment_under_an_existing_one_is_its_own_block() {
        let file = "def total(items):\n    # Prices are in cents.\n    # changed this as requested\n    return sum(items)\n";
        let p = temp_file("adjacent", "py", file);
        let old = "    # Prices are in cents.\n    return sum(items)";
        let new =
            "    # Prices are in cents.\n    # changed this as requested\n    return sum(items)";
        let c = collect(&[edit(&p, old, new)], 1);
        assert_eq!(texts(&c), ["# changed this as requested"]);
        assert_eq!((c[0].start_line, c[0].end_line), (3, 3));
        assert!(c[0].excerpt.contains("    2    ") && c[0].excerpt.contains("    3 >> "));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn directives_and_safety_blocks_are_not_candidates() {
        let file = "fn f() {\n    // SAFETY: p is valid.\n    // It outlives the call.\n    let rc = unsafe { g(p) };\n    // the real target\n}\n";
        let p = temp_file("safety", "rs", file);
        assert_eq!(
            texts(&collect(&[write(&p, file)], 0)),
            ["// the real target"]
        );
        let _ = std::fs::remove_file(&p);

        let ts = "// @ts-expect-error\nconst a: number = 'x';\n// eslint-disable-next-line no-console\n// explain the log\nconsole.log(a);\n";
        let p = temp_file("ts", "ts", ts);
        assert_eq!(texts(&collect(&[write(&p, ts)], 0)), ["// explain the log"]);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn conflict_resolution_edits_are_skipped() {
        let old = "<<<<<<< HEAD\n// ours\n=======\n// theirs\n>>>>>>> main\n";
        let p = temp_file("conflict", "rs", "// ours\n");
        assert!(collect(&[edit(&p, old, "// ours\n")], 0).is_empty());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn excerpt_marks_the_block_lines() {
        let out = excerpt("a\nb\nc\nd\ne\n", 2, 3, 1);
        assert_eq!(out, "    1    a\n    2 >> b\n    3 >> c\n    4    d\n");
    }

    #[test]
    fn php_hash_comments_are_found() {
        let file = "<?php\n$x = 1; # increment the counter\n";
        let p = temp_file("php", "php", file);
        assert_eq!(
            texts(&collect(&[write(&p, file)], 0)),
            ["# increment the counter"]
        );
        let _ = std::fs::remove_file(&p);
    }
}

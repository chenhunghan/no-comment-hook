use super::hunks::Hunk;
use crate::options::Options;

pub fn might_have_comment(hunk: &Hunk, opts: &Options) -> bool {
    if opts.pre_filter_off {
        return true;
    }
    let texts: &[&str] = match &hunk.old_text {
        Some(old) => &[old.as_str(), hunk.new_text.as_str()],
        None => &[hunk.new_text.as_str()],
    };
    if texts.iter().any(|t| has_conflict_markers(t)) {
        return false;
    }
    let markers = comment_markers(&hunk.file_path);
    if markers.is_empty() {
        return true;
    }
    texts
        .iter()
        .any(|t| has_non_directive_comment(t, &hunk.file_path, markers))
}

fn has_non_directive_comment(text: &str, file_path: &str, markers: &[&str]) -> bool {
    let ext = file_ext_lower(file_path);
    let mut in_safety = false;
    for line in text.lines() {
        if is_directive_line(line, &ext) {
            in_safety = is_rust_safety_comment(line.trim_start(), &ext);
            continue;
        }
        if in_safety && is_safety_continuation(line, &ext) {
            continue;
        }
        in_safety = false;
        if markers.iter().any(|m| line.contains(*m)) {
            return true;
        }
    }
    false
}

pub fn hunk_still_applies(hunk: &Hunk) -> bool {
    match std::fs::read_to_string(&hunk.file_path) {
        Ok(content) => content.contains(&hunk.new_text),
        Err(_) => false,
    }
}

fn has_conflict_markers(text: &str) -> bool {
    let mut start = false;
    let mut sep = false;
    let mut end = false;
    for line in text.lines() {
        if line.starts_with("<<<<<<< ") {
            start = true;
        } else if line.trim_end() == "=======" {
            sep = true;
        } else if line.starts_with(">>>>>>> ") {
            end = true;
        }
        if start && sep && end {
            return true;
        }
    }
    false
}

pub fn comment_markers(file_path: &str) -> &'static [&'static str] {
    let ext = file_ext_lower(file_path);
    match ext.as_str() {
        "rs" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "c" | "cpp" | "cc" | "h" | "hpp"
        | "java" | "cs" | "swift" | "kt" | "scala" | "go" | "php" => &["//", "/*", "///", "/**"],
        "py" => &["#", "\"\"\"", "'''"],
        "rb" => &["#", "=begin"],
        _ => &[],
    }
}

pub fn strip_directive_lines(text: &str, file_path: &str) -> String {
    let ext = file_ext_lower(file_path);
    let mut out = String::with_capacity(text.len());
    let mut first = true;
    let mut in_safety = false;
    for line in text.split('\n') {
        if is_directive_line(line, &ext) {
            in_safety = is_rust_safety_comment(line.trim_start(), &ext);
            continue;
        }
        if in_safety && is_safety_continuation(line, &ext) {
            continue;
        }
        in_safety = false;
        if !first {
            out.push('\n');
        }
        out.push_str(line);
        first = false;
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

fn is_directive_line(line: &str, ext: &str) -> bool {
    let t = line.trim_start();
    match ext {
        "rs" => is_rust_safety_comment(t, ext),
        "go" => t.starts_with("//go:"),
        "py" => is_python_directive(t),
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => is_ts_directive(t),
        "c" | "cpp" | "cc" | "h" | "hpp" => is_c_directive(t),
        "rb" => is_ruby_directive(t),
        _ => false,
    }
}

fn is_rust_safety_comment(trimmed: &str, ext: &str) -> bool {
    ext == "rs" && (trimmed.starts_with("// SAFETY:") || trimmed.starts_with("//SAFETY:"))
}

// clippy is satisfied by `SAFETY:` on a block's first line alone, so the
// continuation `//` lines must be hidden too; `///`/`//!` stay visible.
fn is_safety_continuation(line: &str, ext: &str) -> bool {
    if ext != "rs" {
        return false;
    }
    let t = line.trim_start();
    t.starts_with("//") && !t.starts_with("///") && !t.starts_with("//!")
}

fn is_python_directive(t: &str) -> bool {
    let Some(rest) = t.strip_prefix('#') else {
        return false;
    };
    let rest = rest.trim_start();
    rest.starts_with("type:")
        || rest.starts_with("noqa")
        || rest.starts_with("pylint:")
        || rest.starts_with("pragma:")
        || rest.starts_with("fmt:")
        || rest.starts_with("mypy:")
        || rest.starts_with("ruff:")
}

fn is_ts_directive(t: &str) -> bool {
    t.starts_with("// @ts-")
        || t.starts_with("//@ts-")
        || t.starts_with("/// <reference")
        || t.starts_with("// eslint-disable")
        || t.starts_with("// eslint-enable")
        || t.starts_with("/* eslint-disable")
        || t.starts_with("/* eslint-enable")
        || t.starts_with("// prettier-ignore")
        || t.starts_with("// biome-ignore")
}

fn is_c_directive(t: &str) -> bool {
    t.starts_with("// NOLINT") || t.starts_with("// clang-format")
}

fn is_ruby_directive(t: &str) -> bool {
    let Some(rest) = t.strip_prefix('#') else {
        return false;
    };
    let rest = rest.trim_start();
    rest.starts_with("frozen_string_literal:")
        || rest.starts_with("encoding:")
        || rest.starts_with("coding:")
        || rest.starts_with("rubocop:")
        || rest.starts_with("shareable_constant_value:")
        || rest.starts_with("warn_indent:")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hunk(path: &str, old: Option<&str>, new: &str) -> Hunk {
        Hunk {
            file_path: path.into(),
            old_text: old.map(str::to_string),
            new_text: new.into(),
        }
    }

    #[test]
    fn comment_markers_rust() {
        assert!(comment_markers("/a/b.rs").contains(&"//"));
    }

    #[test]
    fn comment_markers_python() {
        assert!(comment_markers("/a/b.py").contains(&"#"));
    }

    #[test]
    fn comment_markers_unknown_ext_empty() {
        assert!(comment_markers("/a/b.zzz").is_empty());
    }

    #[test]
    fn finds_marker_in_new() {
        let opts = Options::default();
        let h = hunk("/a/b.rs", Some("let x = 1;"), "// new\nlet x = 2;");
        assert!(might_have_comment(&h, &opts));
    }

    #[test]
    fn skips_when_no_marker_in_either() {
        let opts = Options::default();
        let h = hunk("/a/b.rs", Some("let x = 1;"), "let x = 2;");
        assert!(!might_have_comment(&h, &opts));
    }

    #[test]
    fn finds_in_old_deleted_comment() {
        let opts = Options::default();
        let h = hunk("/a/b.rs", Some("// old\nlet x = 1;"), "let x = 1;");
        assert!(might_have_comment(&h, &opts));
    }

    #[test]
    fn unknown_ext_passes() {
        let opts = Options::default();
        let h = hunk("/a/b.zzz", None, "let x = 2;");
        assert!(might_have_comment(&h, &opts));
    }

    #[test]
    fn pre_filter_off_passes() {
        let opts = Options {
            pre_filter_off: true,
            ..Options::default()
        };
        let h = hunk("/a/b.rs", Some("let x = 1;"), "let x = 2;");
        assert!(might_have_comment(&h, &opts));
    }

    #[test]
    fn skips_when_old_text_has_conflict_block() {
        let opts = Options::default();
        let old =
            "fn f() {\n<<<<<<< HEAD\n    // ours\n=======\n    // theirs\n>>>>>>> origin/main\n}\n";
        let new = "fn f() {\n    // ours\n}\n";
        assert!(!might_have_comment(&hunk("/a/b.rs", Some(old), new), &opts));
    }

    #[test]
    fn skips_when_new_text_has_leftover_markers() {
        let opts = Options::default();
        let new = "<<<<<<< HEAD\nlet x = 1;\n=======\nlet x = 2;\n>>>>>>> feature\n";
        assert!(!might_have_comment(&hunk("/a/b.rs", None, new), &opts));
    }

    #[test]
    fn does_not_skip_on_partial_markers() {
        let opts = Options::default();
        let new = "// describe <<<<<<< style markers\nlet x = 1;\n";
        assert!(might_have_comment(&hunk("/a/b.rs", None, new), &opts));
    }

    fn temp_file(name: &str, content: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "no-comment-still-applies-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn still_applies_when_new_text_in_file() {
        let path = temp_file("present", "fn a() {}\n// keep me\nfn b() {}\n");
        let h = hunk(path.to_str().unwrap(), None, "// keep me");
        assert!(hunk_still_applies(&h));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn does_not_apply_when_new_text_gone() {
        let path = temp_file("gone", "fn a() {}\nfn b() {}\n");
        let h = hunk(path.to_str().unwrap(), None, "// removed since");
        assert!(!hunk_still_applies(&h));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn does_not_apply_when_file_missing() {
        let h = hunk("/no/such/path-{this-should-not-exist}.rs", None, "anything");
        assert!(!hunk_still_applies(&h));
    }

    #[test]
    fn strip_directive_lines_rust_safety() {
        let src = "let p = q;\n    // SAFETY: p is NUL-terminated.\n    let rc = unsafe { libc::chroot(p) };\n";
        let out = strip_directive_lines(src, "/a/b.rs");
        assert!(!out.contains("SAFETY"));
        assert!(out.contains("let p = q;"));
        assert!(out.contains("let rc = unsafe { libc::chroot(p) };"));
    }

    #[test]
    fn strip_directive_lines_preserves_non_directives() {
        let src = "// keep me\nfn f() {}\n";
        let out = strip_directive_lines(src, "/a/b.rs");
        assert_eq!(out, src);
    }

    #[test]
    fn strip_directive_lines_go() {
        let src = "//go:build linux\npackage x\n";
        let out = strip_directive_lines(src, "/a/b.go");
        assert!(!out.contains("go:build"));
        assert!(out.contains("package x"));
    }

    #[test]
    fn strip_directive_lines_python() {
        let src = "x = 1  # set x\nimport y  # type: ignore\nz = noqa_test()\n# noqa: E501\n";
        let out = strip_directive_lines(src, "/a/b.py");
        assert!(out.contains("x = 1"));
        assert!(out.contains("import y  # type: ignore"));
        assert!(out.contains("z = noqa_test()"));
        assert!(!out.contains("# noqa: E501"));
    }

    #[test]
    fn strip_directive_lines_ts() {
        let src = "// @ts-expect-error\nconst x = 1;\n// eslint-disable-next-line no-console\nconsole.log(x);\n";
        let out = strip_directive_lines(src, "/a/b.ts");
        assert!(!out.contains("@ts-expect-error"));
        assert!(!out.contains("eslint-disable"));
        assert!(out.contains("const x = 1;"));
        assert!(out.contains("console.log(x);"));
    }

    #[test]
    fn strip_directive_lines_c_nolint() {
        let src = "int x = 0;\n// NOLINTNEXTLINE(readability)\nint y = x;\n";
        let out = strip_directive_lines(src, "/a/b.cpp");
        assert!(!out.contains("NOLINT"));
        assert!(out.contains("int x = 0;"));
    }

    #[test]
    fn strip_directive_lines_ruby_magic() {
        let src = "# frozen_string_literal: true\nclass Foo; end\n";
        let out = strip_directive_lines(src, "/a/b.rb");
        assert!(!out.contains("frozen_string_literal"));
        assert!(out.contains("class Foo; end"));
    }

    #[test]
    fn strip_directive_lines_unknown_ext_passthrough() {
        let src = "// SAFETY: anything\nlet x = 1;\n";
        let out = strip_directive_lines(src, "/a/b.zzz");
        assert_eq!(out, src);
    }

    #[test]
    fn might_have_comment_skips_directive_only_hunk() {
        let opts = Options::default();
        let new =
            "    let p = q;\n    // SAFETY: p is NUL-terminated.\n    let rc = unsafe { f(p) };\n";
        let h = hunk("/a.rs", Some("let p = q;\nlet rc = unsafe { f(p) };"), new);
        assert!(!might_have_comment(&h, &opts));
    }

    #[test]
    fn might_have_comment_keeps_mixed_directive_and_real() {
        let opts = Options::default();
        let new = "// real comment to review\n// SAFETY: p\nlet x = 1;\n";
        let h = hunk("/a.rs", None, new);
        assert!(might_have_comment(&h, &opts));
    }

    #[test]
    fn strip_directive_lines_multiline_safety_block() {
        let src = "    // SAFETY: rs_signal_init runs pre-fork, before any handler is\n    // registered, so the process-wide race in the rustdoc cannot occur.\n    let rc = unsafe { f() };\n";
        let out = strip_directive_lines(src, "/a/b.rs");
        assert!(!out.contains("SAFETY"));
        assert!(!out.contains("registered, so the process-wide race"));
        assert!(out.contains("let rc = unsafe { f() };"));
    }

    #[test]
    fn strip_directive_lines_safety_block_stops_at_code() {
        let src = "    // SAFETY: justified.\n    let x = 1;\n    // a real comment to review\n    let y = x;\n";
        let out = strip_directive_lines(src, "/a/b.rs");
        assert!(!out.contains("SAFETY"));
        assert!(out.contains("// a real comment to review"));
        assert!(out.contains("let x = 1;"));
    }

    #[test]
    fn strip_directive_lines_safety_block_keeps_doc_comment() {
        let src =
            "    // SAFETY: justified.\n    /// doc comment, not a continuation\n    fn f() {}\n";
        let out = strip_directive_lines(src, "/a/b.rs");
        assert!(!out.contains("SAFETY"));
        assert!(out.contains("/// doc comment, not a continuation"));
    }

    #[test]
    fn might_have_comment_skips_multiline_safety_only_hunk() {
        let opts = Options::default();
        let new = "    // SAFETY: only called pre-fork, before handlers exist, so the\n    // documented process-wide race cannot happen here.\n    let rc = unsafe { f() };\n";
        let h = hunk("/a.rs", Some("let rc = unsafe { f() };"), new);
        assert!(!might_have_comment(&h, &opts));
    }

    #[test]
    fn might_have_comment_keeps_real_comment_after_safety_block() {
        let opts = Options::default();
        let new = "    // SAFETY: justified across\n    // two lines.\n    let x = 1; // increment counter\n";
        let h = hunk("/a.rs", None, new);
        assert!(might_have_comment(&h, &opts));
    }

    #[test]
    fn strip_directive_lines_preserves_trailing_newline() {
        let src = "a\nb\n";
        assert_eq!(strip_directive_lines(src, "/x.rs"), "a\nb\n");
    }

    #[test]
    fn pre_filter_off_bypasses_conflict_skip() {
        let opts = Options {
            pre_filter_off: true,
            ..Options::default()
        };
        let new = "<<<<<<< HEAD\n=======\n>>>>>>> x\n";
        assert!(might_have_comment(&hunk("/a/b.rs", None, new), &opts));
    }
}

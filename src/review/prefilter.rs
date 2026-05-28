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
    texts.iter().any(|t| markers.iter().any(|m| t.contains(*m)))
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
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "rs" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "c" | "cpp" | "cc" | "h" | "hpp"
        | "java" | "cs" | "swift" | "kt" | "scala" | "go" | "php" => &["//", "/*", "///", "/**"],
        "py" => &["#", "\"\"\"", "'''"],
        "rb" => &["#", "=begin"],
        _ => &[],
    }
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
    fn pre_filter_off_bypasses_conflict_skip() {
        let opts = Options {
            pre_filter_off: true,
            ..Options::default()
        };
        let new = "<<<<<<< HEAD\n=======\n>>>>>>> x\n";
        assert!(might_have_comment(&hunk("/a/b.rs", None, new), &opts));
    }
}

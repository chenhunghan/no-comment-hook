use super::hunks::Hunk;
use crate::options::Options;

pub fn might_have_comment(hunk: &Hunk, opts: &Options) -> bool {
    if opts.pre_filter_off {
        return true;
    }
    let markers = comment_markers(&hunk.file_path);
    if markers.is_empty() {
        return true;
    }
    let texts: &[&str] = match &hunk.old_text {
        Some(old) => &[old.as_str(), hunk.new_text.as_str()],
        None => &[hunk.new_text.as_str()],
    };
    texts.iter().any(|t| markers.iter().any(|m| t.contains(*m)))
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
}

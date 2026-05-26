use crate::collect::RecordOwned;

pub const MAX_NEW_LINES: usize = 5_000;
pub const MAX_WRITE_LINES: usize = 10_000;
pub const WRITE_WINDOW_LINES: usize = 100;

#[derive(Clone)]
pub struct Hunk {
    pub file_path: String,
    pub old_text: Option<String>,
    pub new_text: String,
}

pub fn build_hunks(records: &[RecordOwned]) -> Vec<Hunk> {
    let mut hunks = Vec::new();
    for r in records {
        match r.tool.as_str() {
            "Edit" => push_edit(&mut hunks, r),
            "Write" => push_write(&mut hunks, r),
            _ => {}
        }
    }
    hunks
}

fn push_edit(hunks: &mut Vec<Hunk>, r: &RecordOwned) {
    let (Some(old), Some(new)) = (&r.old_string, &r.new_string) else {
        return;
    };
    if line_count(new) > MAX_NEW_LINES {
        return;
    }
    hunks.push(Hunk {
        file_path: r.file_path.clone(),
        old_text: Some(old.clone()),
        new_text: new.clone(),
    });
}

fn push_write(hunks: &mut Vec<Hunk>, r: &RecordOwned) {
    let Some(content) = &r.content else {
        return;
    };
    if line_count(content) > MAX_WRITE_LINES {
        return;
    }
    for window in chunk_lines(content, WRITE_WINDOW_LINES) {
        hunks.push(Hunk {
            file_path: r.file_path.clone(),
            old_text: None,
            new_text: window,
        });
    }
}

pub fn line_count(s: &str) -> usize {
    if s.is_empty() {
        0
    } else {
        s.matches('\n').count() + usize::from(!s.ends_with('\n'))
    }
}

pub fn chunk_lines(text: &str, lines_per_chunk: usize) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }
    lines
        .chunks(lines_per_chunk)
        .map(|c| c.join("\n"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec_edit(path: &str, old: &str, new: &str) -> RecordOwned {
        RecordOwned {
            tool: "Edit".into(),
            file_path: path.into(),
            old_string: Some(old.into()),
            new_string: Some(new.into()),
            content: None,
        }
    }

    fn rec_write(path: &str, content: &str) -> RecordOwned {
        RecordOwned {
            tool: "Write".into(),
            file_path: path.into(),
            old_string: None,
            new_string: None,
            content: Some(content.into()),
        }
    }

    #[test]
    fn line_count_counts_correctly() {
        assert_eq!(line_count(""), 0);
        assert_eq!(line_count("a"), 1);
        assert_eq!(line_count("a\n"), 1);
        assert_eq!(line_count("a\nb"), 2);
        assert_eq!(line_count("a\nb\n"), 2);
    }

    #[test]
    fn chunk_lines_basic() {
        let t = "a\nb\nc\nd\ne";
        assert_eq!(
            chunk_lines(t, 2),
            vec!["a\nb".to_string(), "c\nd".to_string(), "e".to_string()]
        );
    }

    #[test]
    fn chunk_lines_empty() {
        assert!(chunk_lines("", 10).is_empty());
    }

    #[test]
    fn build_hunks_edit_produces_one() {
        let recs = vec![rec_edit("/a.rs", "x", "y")];
        let hunks = build_hunks(&recs);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].file_path, "/a.rs");
        assert_eq!(hunks[0].new_text, "y");
        assert_eq!(hunks[0].old_text.as_deref(), Some("x"));
    }

    #[test]
    fn build_hunks_write_chunks_by_window() {
        let content = (1..=250)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let recs = vec![rec_write("/big.rs", &content)];
        let hunks = build_hunks(&recs);
        assert_eq!(hunks.len(), 3);
        assert!(hunks.iter().all(|h| h.old_text.is_none()));
    }

    #[test]
    fn build_hunks_skips_oversized_edit() {
        let huge = "x\n".repeat(MAX_NEW_LINES + 1);
        let recs = vec![rec_edit("/a.rs", "old", &huge)];
        assert!(build_hunks(&recs).is_empty());
    }

    #[test]
    fn build_hunks_skips_oversized_write() {
        let huge = "x\n".repeat(MAX_WRITE_LINES + 1);
        let recs = vec![rec_write("/a.rs", &huge)];
        assert!(build_hunks(&recs).is_empty());
    }

    #[test]
    fn build_hunks_ignores_unknown_tool() {
        let recs = vec![RecordOwned {
            tool: "Unknown".into(),
            file_path: "/a.rs".into(),
            old_string: None,
            new_string: None,
            content: None,
        }];
        assert!(build_hunks(&recs).is_empty());
    }
}

use std::fs;
use std::path::{Path, PathBuf};

use crate::extract;
use crate::options::Options;

pub fn run(input: &str, opts: &Options) {
    let Some(session_id) = extract::string_field(input, "session_id") else {
        return;
    };
    let Some(tool_name) = extract::string_field(input, "tool_name") else {
        return;
    };
    let Some(file_path) = extract::string_field(input, "file_path") else {
        return;
    };

    if !is_source_ext(&file_path, &opts.source_ext) {
        return;
    }

    let dir = session_dir(&session_id);
    if fs::create_dir_all(&dir).is_err() {
        return;
    }

    match tool_name.as_str() {
        "Edit" => collect_edit(input, &file_path, &dir),
        "Write" => collect_write(input, &file_path, &dir),
        "MultiEdit" => collect_multi_edit(input, &file_path, &dir),
        _ => {}
    }
}

fn collect_edit(input: &str, file_path: &str, dir: &Path) {
    let Some(old) = extract::string_field(input, "old_string") else {
        return;
    };
    let Some(new) = extract::string_field(input, "new_string") else {
        return;
    };
    write_record(
        dir,
        &Record {
            tool: "Edit",
            file_path,
            old_string: Some(&old),
            new_string: Some(&new),
            content: None,
        },
    );
}

fn collect_write(input: &str, file_path: &str, dir: &Path) {
    let Some(content) = extract::string_field(input, "content") else {
        return;
    };
    write_record(
        dir,
        &Record {
            tool: "Write",
            file_path,
            old_string: None,
            new_string: None,
            content: Some(&content),
        },
    );
}

fn collect_multi_edit(input: &str, file_path: &str, dir: &Path) {
    for obj in extract::objects_in_array(input, "edits") {
        let Some(old) = extract::string_field(obj, "old_string") else {
            continue;
        };
        let Some(new) = extract::string_field(obj, "new_string") else {
            continue;
        };
        write_record(
            dir,
            &Record {
                tool: "Edit",
                file_path,
                old_string: Some(&old),
                new_string: Some(&new),
                content: None,
            },
        );
    }
}

pub fn session_dir(session_id: &str) -> PathBuf {
    PathBuf::from("/tmp").join(format!("no-comment-{session_id}"))
}

/// Sibling of `session_dir`, so the deferral store survives the per-Stop
/// `cleanup_session` that wipes the records dir.
pub fn seen_path(session_id: &str) -> PathBuf {
    PathBuf::from("/tmp").join(format!("no-comment-{session_id}.seen"))
}

pub fn is_source_ext<S: AsRef<str>>(file_path: &str, allow: &[S]) -> bool {
    let ext = match Path::new(file_path).extension().and_then(|e| e.to_str()) {
        Some(e) => format!(".{}", e.to_ascii_lowercase()),
        None => return false,
    };
    allow.iter().any(|a| a.as_ref().eq_ignore_ascii_case(&ext))
}

struct Record<'a> {
    tool: &'a str,
    file_path: &'a str,
    old_string: Option<&'a str>,
    new_string: Option<&'a str>,
    content: Option<&'a str>,
}

fn write_record(dir: &Path, rec: &Record<'_>) {
    let mut json = String::from("{\"tool\":");
    json_push_str(&mut json, rec.tool);
    json.push_str(",\"file_path\":");
    json_push_str(&mut json, rec.file_path);
    if let Some(s) = rec.old_string {
        json.push_str(",\"old_string\":");
        json_push_str(&mut json, s);
    }
    if let Some(s) = rec.new_string {
        json.push_str(",\"new_string\":");
        json_push_str(&mut json, s);
    }
    if let Some(s) = rec.content {
        json.push_str(",\"content\":");
        json_push_str(&mut json, s);
    }
    json.push('}');

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let pid = std::process::id();
    let _ = fs::write(dir.join(format!("{now:020}-{pid}.json")), json);
}

fn json_push_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

pub struct RecordOwned {
    pub tool: String,
    pub file_path: String,
    pub old_string: Option<String>,
    pub new_string: Option<String>,
    pub content: Option<String>,
}

pub fn read_records(session_id: &str) -> Vec<RecordOwned> {
    let dir = session_dir(session_id);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    paths.sort();
    let mut records = Vec::new();
    for p in paths {
        if let Ok(text) = fs::read_to_string(&p) {
            if let Some(r) = parse_record(&text) {
                records.push(r);
            }
        }
    }
    records
}

pub fn cleanup_session(session_id: &str) {
    let dir = session_dir(session_id);
    let _ = fs::remove_dir_all(dir);
}

fn parse_record(text: &str) -> Option<RecordOwned> {
    Some(RecordOwned {
        tool: extract::string_field(text, "tool")?,
        file_path: extract::string_field(text, "file_path")?,
        old_string: extract::string_field(text, "old_string"),
        new_string: extract::string_field(text, "new_string"),
        content: extract::string_field(text, "content"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_source_ext_matches() {
        let allow: &[&str] = &[".rs", ".ts"];
        assert!(is_source_ext("/a/b.rs", allow));
        assert!(is_source_ext("/a/b.RS", allow));
        assert!(is_source_ext("/a/b.ts", allow));
        assert!(!is_source_ext("/a/b.md", allow));
        assert!(!is_source_ext("/a/noext", allow));
    }

    #[test]
    fn is_source_ext_accepts_string_slice() {
        let allow: Vec<String> = vec![".rs".into(), ".go".into()];
        assert!(is_source_ext("/a/b.go", &allow));
    }

    #[test]
    fn round_trip_record_through_json() {
        let dir = std::env::temp_dir().join(format!(
            "no-comment-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        fs::create_dir_all(&dir).unwrap();
        write_record(
            &dir,
            &Record {
                tool: "Edit",
                file_path: "/x/y.rs",
                old_string: Some("let a = 1;\nlet b = 2;"),
                new_string: Some("let a = 1;\nlet b = 3;"),
                content: None,
            },
        );
        let entries: Vec<_> = fs::read_dir(&dir).unwrap().filter_map(Result::ok).collect();
        assert_eq!(entries.len(), 1);
        let text = fs::read_to_string(entries[0].path()).unwrap();
        let parsed = parse_record(&text).unwrap();
        assert_eq!(parsed.tool, "Edit");
        assert_eq!(parsed.file_path, "/x/y.rs");
        assert_eq!(parsed.old_string.as_deref(), Some("let a = 1;\nlet b = 2;"));
        assert_eq!(parsed.new_string.as_deref(), Some("let a = 1;\nlet b = 3;"));
        let _ = fs::remove_dir_all(&dir);
    }

    fn unique_session_id(tag: &str) -> String {
        format!(
            "test-{tag}-{pid}-{nanos}",
            pid = std::process::id(),
            nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        )
    }

    fn cleanup(session_id: &str) {
        let _ = fs::remove_dir_all(session_dir(session_id));
    }

    #[test]
    fn run_dispatches_edit_and_writes_record() {
        let sid = unique_session_id("edit");
        let input = format!(
            r#"{{"session_id":"{sid}","tool_name":"Edit","tool_input":{{"file_path":"/x.rs","old_string":"a","new_string":"b"}}}}"#
        );
        run(&input, &Options::default());

        let records = read_records(&sid);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].tool, "Edit");
        assert_eq!(records[0].file_path, "/x.rs");
        assert_eq!(records[0].old_string.as_deref(), Some("a"));
        assert_eq!(records[0].new_string.as_deref(), Some("b"));
        cleanup(&sid);
    }

    #[test]
    fn run_dispatches_write_and_writes_record() {
        let sid = unique_session_id("write");
        let input = format!(
            r#"{{"session_id":"{sid}","tool_name":"Write","tool_input":{{"file_path":"/x.rs","content":"hello"}}}}"#
        );
        run(&input, &Options::default());

        let records = read_records(&sid);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].tool, "Write");
        assert_eq!(records[0].content.as_deref(), Some("hello"));
        cleanup(&sid);
    }

    #[test]
    fn run_dispatches_multi_edit_writes_one_record_per_edit() {
        let sid = unique_session_id("multi");
        let input = format!(
            r#"{{"session_id":"{sid}","tool_name":"MultiEdit","tool_input":{{"file_path":"/x.rs","edits":[{{"old_string":"a","new_string":"A"}},{{"old_string":"b","new_string":"B"}},{{"old_string":"c","new_string":"C"}}]}}}}"#
        );
        run(&input, &Options::default());

        let records = read_records(&sid);
        assert_eq!(records.len(), 3);
        let new_strings: Vec<_> = records
            .iter()
            .map(|r| r.new_string.as_deref().unwrap())
            .collect();
        assert!(new_strings.contains(&"A"));
        assert!(new_strings.contains(&"B"));
        assert!(new_strings.contains(&"C"));
        cleanup(&sid);
    }

    #[test]
    fn run_skips_non_source_extension() {
        let sid = unique_session_id("md");
        let input = format!(
            r#"{{"session_id":"{sid}","tool_name":"Write","tool_input":{{"file_path":"/notes.md","content":"hello"}}}}"#
        );
        run(&input, &Options::default());

        assert!(read_records(&sid).is_empty());
        cleanup(&sid);
    }

    #[test]
    fn run_returns_silently_without_session_id() {
        let input = r#"{"tool_name":"Edit","tool_input":{"file_path":"/x.rs","old_string":"a","new_string":"b"}}"#;
        run(input, &Options::default());
    }

    #[test]
    fn run_returns_silently_for_unknown_tool() {
        let sid = unique_session_id("unknown");
        let input = format!(
            r#"{{"session_id":"{sid}","tool_name":"FooTool","tool_input":{{"file_path":"/x.rs"}}}}"#
        );
        run(&input, &Options::default());

        assert!(read_records(&sid).is_empty());
        cleanup(&sid);
    }
}

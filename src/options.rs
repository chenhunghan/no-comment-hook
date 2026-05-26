use std::env;

pub struct Options {
    pub source_ext: Vec<String>,
    pub disabled: Vec<String>,
    pub model: String,
    pub context_lines: usize,
    pub timeout_secs: u64,
    pub max_parallel: usize,
    pub pre_filter_off: bool,
    pub debug: bool,
    pub claude_bin: String,
}

pub struct Principle {
    pub number: u32,
    pub key: &'static str,
    pub group: PrincipleGroup,
    pub name: &'static str,
    pub detail: &'static str,
}

#[derive(Clone, Copy)]
pub enum PrincipleGroup {
    SessionDoc,
    General,
}

pub const PRINCIPLES: &[Principle] = &[
    Principle {
        number: 1,
        key: "process-vocab",
        group: PrincipleGroup::SessionDoc,
        name: "Process vocabulary",
        detail: "Flag comments containing words like \"Pin:\", \"pre-fix\", \"previously\", \"behavior now is\", \"the fix is\", \"reviewer's concern\", \"we now\", \"we used to\", \"as discussed\", \"after refactor\", \"before this change\". These describe a change or moment, not the code's current state.",
    },
    Principle {
        number: 2,
        key: "past-narrative",
        group: PrincipleGroup::SessionDoc,
        name: "Past-tense narrative about removed code",
        detail: "Flag comments narrating what the code used to do or what it would have done (e.g. \"The pre-fix code would have read 0 bytes\"). Code documents the present, not history.",
    },
    Principle {
        number: 3,
        key: "test-meta",
        group: PrincipleGroup::SessionDoc,
        name: "Meta-framing about a test's role",
        detail: "Flag comments explaining why a test exists relative to other tests (e.g. \"Pin: the third arm of attempt_best_effort_restart\"). The test name and assertion convey purpose.",
    },
    Principle {
        number: 4,
        key: "mirrors-x",
        group: PrincipleGroup::SessionDoc,
        name: "Cross-references mirroring another module",
        detail: "Flag comments like \"mirrors kernel.rs::init_tracing_capture\" or \"same as X / same fix\". The link rots and the parallel isn't load-bearing.",
    },
    Principle {
        number: 5,
        key: "defensive",
        group: PrincipleGroup::SessionDoc,
        name: "Defensive justification > 1-2 sentences",
        detail: "Flag multi-sentence justification anticipating reviewer pushback (e.g. \"Defence-in-depth against...\", \"Belt-and-suspenders: after the remap...\"). Belongs in the PR description.",
    },
    Principle {
        number: 6,
        key: "paragraph-docs",
        group: PrincipleGroup::SessionDoc,
        name: "Paragraph-shape doc comments on functions/types",
        detail: "Flag /// or \"\"\" doc blocks with multiple sub-paragraphs treating the comment as a mini design doc. EXCEPT on public API items (carve-out).",
    },
    Principle {
        number: 7,
        key: "no-comment-default",
        group: PrincipleGroup::General,
        name: "Default to no comment",
        detail: "Flag comments that don't add information beyond what well-named identifiers and the code itself already convey.",
    },
    Principle {
        number: 8,
        key: "why-not-what",
        group: PrincipleGroup::General,
        name: "WHY, not WHAT",
        detail: "Flag comments restating what the code does rather than explaining why (e.g. \"// Increment counter\" above `counter += 1`).",
    },
    Principle {
        number: 9,
        key: "no-header-restate",
        group: PrincipleGroup::General,
        name: "No header comments restating the name",
        detail: "Flag comments that just restate the function/type/module name (e.g. \"// User service - handles users\"). EXCEPT public API docstrings (carve-out).",
    },
    Principle {
        number: 10,
        key: "no-transient",
        group: PrincipleGroup::General,
        name: "No transient context",
        detail: "Flag comments referencing issue numbers, PR numbers, \"added for X flow\", \"used by Y\" - these rot as the codebase evolves and belong in commit/PR messages.",
    },
    Principle {
        number: 11,
        key: "no-commented-out",
        group: PrincipleGroup::General,
        name: "No commented-out code",
        detail: "Flag any code that has been commented out rather than deleted. Git remembers.",
    },
    Principle {
        number: 12,
        key: "no-bare-todo",
        group: PrincipleGroup::General,
        name: "No bare TODO/FIXME",
        detail: "Flag TODO/FIXME/XXX comments that lack a tracked ticket reference or concrete description of the work.",
    },
    Principle {
        number: 14,
        key: "no-apology",
        group: PrincipleGroup::General,
        name: "Don't apologize",
        detail: "Flag self-deprecating comments like \"// hacky\", \"// I know this is ugly\", \"// sorry about this\". Fix it or name the constraint.",
    },
];

const DEFAULT_SOURCE_EXTS: &[&str] = &[
    ".rs", ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".py", ".go", ".java", ".c", ".cpp",
    ".cc", ".h", ".hpp", ".rb", ".swift", ".kt", ".scala", ".cs", ".php",
];

impl Default for Options {
    fn default() -> Self {
        Self {
            source_ext: DEFAULT_SOURCE_EXTS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            disabled: Vec::new(),
            model: "claude-haiku-4-5".to_string(),
            context_lines: 5,
            timeout_secs: 60,
            max_parallel: 4,
            pre_filter_off: false,
            debug: false,
            claude_bin: "claude".to_string(),
        }
    }
}

impl Options {
    pub fn from_env_and_args(args: &[String]) -> Self {
        let mut o = Self::default();
        apply_args(&mut o, args);
        apply_env(&mut o);
        o
    }

    pub fn is_enabled(&self, key: &str) -> bool {
        !self.disabled.iter().any(|d| d == key)
    }
}

fn apply_args(o: &mut Options, args: &[String]) {
    for arg in args {
        if let Some(v) = arg.strip_prefix("--disable=") {
            push_disabled(o, v);
        } else if let Some(v) = arg.strip_prefix("--enable=") {
            remove_disabled(o, v);
        } else if let Some(v) = arg.strip_prefix("--model=") {
            o.model = v.to_string();
        } else if let Some(v) = arg.strip_prefix("--context-lines=") {
            if let Ok(n) = v.parse() {
                o.context_lines = n;
            }
        } else if let Some(v) = arg.strip_prefix("--timeout=") {
            if let Ok(n) = v.parse() {
                o.timeout_secs = n;
            }
        } else if let Some(v) = arg.strip_prefix("--max-parallel=") {
            if let Ok(n) = v.parse() {
                o.max_parallel = n;
            }
        } else if let Some(v) = arg.strip_prefix("--source-ext=") {
            extend_source_ext(o, v);
        } else if arg == "--no-pre-filter" {
            o.pre_filter_off = true;
        } else if arg == "--debug" {
            o.debug = true;
        }
    }
}

type EnvApplier = fn(&str, &mut Options);

const ENV_APPLIERS: &[(&str, EnvApplier)] = &[
    ("NO_COMMENT_HOOK_DISABLE", |v, o| push_disabled(o, v)),
    ("NO_COMMENT_HOOK_ENABLE", |v, o| remove_disabled(o, v)),
    ("NO_COMMENT_HOOK_MODEL", |v, o| o.model = v.to_string()),
    ("NO_COMMENT_HOOK_CONTEXT_LINES", |v, o| {
        if let Ok(n) = v.parse() {
            o.context_lines = n;
        }
    }),
    ("NO_COMMENT_HOOK_TIMEOUT", |v, o| {
        if let Ok(n) = v.parse() {
            o.timeout_secs = n;
        }
    }),
    ("NO_COMMENT_HOOK_MAX_PARALLEL", |v, o| {
        if let Ok(n) = v.parse() {
            o.max_parallel = n;
        }
    }),
    ("NO_COMMENT_HOOK_SOURCE_EXT", |v, o| extend_source_ext(o, v)),
    ("NO_COMMENT_HOOK_NO_PREFILTER", |v, o| {
        if is_truthy(v) {
            o.pre_filter_off = true;
        }
    }),
    ("NO_COMMENT_HOOK_DEBUG", |v, o| {
        if is_truthy(v) {
            o.debug = true;
        }
    }),
    ("NO_COMMENT_HOOK_CLAUDE_BIN", |v, o| {
        o.claude_bin = v.to_string();
    }),
];

fn apply_env(o: &mut Options) {
    for (name, apply) in ENV_APPLIERS {
        if let Ok(v) = env::var(name) {
            apply(&v, o);
        }
    }
}

fn push_disabled(o: &mut Options, csv: &str) {
    for key in csv.split(',') {
        for resolved in expand_group(key.trim()) {
            if !o.disabled.iter().any(|d| d == &resolved) {
                o.disabled.push(resolved);
            }
        }
    }
}

fn remove_disabled(o: &mut Options, csv: &str) {
    for key in csv.split(',') {
        let to_remove = expand_group(key.trim());
        o.disabled.retain(|d| !to_remove.contains(d));
    }
}

fn extend_source_ext(o: &mut Options, csv: &str) {
    for e in csv.split(',') {
        let e = e.trim();
        if e.is_empty() {
            continue;
        }
        let with_dot = if e.starts_with('.') {
            e.to_string()
        } else {
            format!(".{e}")
        };
        if !o
            .source_ext
            .iter()
            .any(|x| x.eq_ignore_ascii_case(&with_dot))
        {
            o.source_ext.push(with_dot);
        }
    }
}

fn expand_group(key: &str) -> Vec<String> {
    match key {
        "session-doc" => PRINCIPLES
            .iter()
            .filter(|p| matches!(p.group, PrincipleGroup::SessionDoc))
            .map(|p| p.key.to_string())
            .collect(),
        "general" => PRINCIPLES
            .iter()
            .filter(|p| matches!(p.group, PrincipleGroup::General))
            .map(|p| p.key.to_string())
            .collect(),
        "all" => PRINCIPLES.iter().map(|p| p.key.to_string()).collect(),
        other if !other.is_empty() => vec![other.to_string()],
        _ => Vec::new(),
    }
}

fn is_truthy(v: &str) -> bool {
    v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_thirteen_principles() {
        assert_eq!(PRINCIPLES.len(), 13);
        let o = Options::default();
        for p in PRINCIPLES {
            assert!(o.is_enabled(p.key));
        }
    }

    #[test]
    fn disable_single() {
        let mut o = Options::default();
        apply_args(&mut o, &["--disable=defensive".to_string()]);
        assert!(!o.is_enabled("defensive"));
        assert!(o.is_enabled("process-vocab"));
    }

    #[test]
    fn disable_group_session_doc() {
        let mut o = Options::default();
        apply_args(&mut o, &["--disable=session-doc".to_string()]);
        assert!(!o.is_enabled("process-vocab"));
        assert!(!o.is_enabled("paragraph-docs"));
        assert!(o.is_enabled("no-comment-default"));
    }

    #[test]
    fn enable_undoes_disable() {
        let mut o = Options::default();
        apply_args(
            &mut o,
            &[
                "--disable=defensive".to_string(),
                "--enable=defensive".to_string(),
            ],
        );
        assert!(o.is_enabled("defensive"));
    }

    #[test]
    fn disable_dedupes() {
        let mut o = Options::default();
        apply_args(
            &mut o,
            &[
                "--disable=defensive".to_string(),
                "--disable=defensive,defensive".to_string(),
            ],
        );
        let count = o.disabled.iter().filter(|d| *d == "defensive").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn extend_source_ext_adds_dot_and_dedupes() {
        let mut o = Options::default();
        let before = o.source_ext.len();
        apply_args(&mut o, &["--source-ext=zig,.nim,rs".to_string()]);
        assert!(o.source_ext.iter().any(|s| s == ".zig"));
        assert!(o.source_ext.iter().any(|s| s == ".nim"));
        let after = o.source_ext.len();
        assert_eq!(after, before + 2);
    }

    #[test]
    fn parse_model_flag() {
        let mut o = Options::default();
        apply_args(&mut o, &["--model=claude-sonnet-4-6".to_string()]);
        assert_eq!(o.model, "claude-sonnet-4-6");
    }

    #[test]
    fn parse_numeric_flags() {
        let mut o = Options::default();
        apply_args(
            &mut o,
            &[
                "--context-lines=10".to_string(),
                "--timeout=30".to_string(),
                "--max-parallel=8".to_string(),
            ],
        );
        assert_eq!(o.context_lines, 10);
        assert_eq!(o.timeout_secs, 30);
        assert_eq!(o.max_parallel, 8);
    }

    #[test]
    fn no_pre_filter_flag() {
        let mut o = Options::default();
        apply_args(&mut o, &["--no-pre-filter".to_string()]);
        assert!(o.pre_filter_off);
    }
}

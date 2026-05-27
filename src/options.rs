pub struct Options {
    pub source_ext: Vec<String>,
    pub disabled: Vec<String>,
    pub model: String,
    pub effort: String,
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
        key: "redundant",
        group: PrincipleGroup::General,
        name: "Redundant / obvious",
        detail: "Flag comments that add nothing beyond what the code and well-named identifiers already convey: restating what the code does (e.g. \"// increment counter\" above `counter += 1`), restating a function/type/variable name, tutorial-style explanation of basic syntax, or echoing the task (\"// add two numbers as requested\"). This is the most common agent over-commenting smell. Do NOT flag a comment that gives a non-obvious WHY the code cannot show — a constraint, edge case, performance reason, or external/library quirk; those are worth keeping. EXCEPT public API docstrings (carve-out).",
    },
    Principle {
        number: 2,
        key: "change-narration",
        group: PrincipleGroup::SessionDoc,
        name: "Change or task narration",
        detail: "Flag comments that narrate the edit, its history, or the author's plan instead of describing the code as it stands: process words (\"Pin:\", \"previously\", \"the fix is\", \"as discussed\", \"we now\", \"we used to\"), what the code used to do, diff commentary (\"added\", \"removed\", \"changed to async\"), \"as requested\", or a unit's role relative to other code or tests. Reset test: would it still make sense if the commit history were deleted?",
    },
    Principle {
        number: 3,
        key: "non-local",
        group: PrincipleGroup::SessionDoc,
        name: "Non-local reference",
        detail: "Flag comments that point to code or process outside the lines they sit on and rot when that target moves: \"mirrors X\" / \"same as Y\" cross-references, issue or PR numbers, \"added for X flow\", \"used by Y\".",
    },
    Principle {
        number: 4,
        key: "over-explained",
        group: PrincipleGroup::SessionDoc,
        name: "Over-explained",
        detail: "Flag multi-sentence justification written at a reviewer (\"Defence-in-depth against...\", \"belt-and-suspenders: after the remap...\") or doc blocks padded into a mini design doc. Agents over-explain to show their work; keep only the load-bearing why. EXCEPT public API docs (carve-out).",
    },
    Principle {
        number: 5,
        key: "commented-out",
        group: PrincipleGroup::General,
        name: "Commented-out code",
        detail: "Flag code that has been commented out rather than deleted. Git remembers.",
    },
    Principle {
        number: 6,
        key: "bare-todo",
        group: PrincipleGroup::General,
        name: "Bare TODO/FIXME",
        detail: "Flag TODO/FIXME/XXX comments that lack a tracked ticket reference or a concrete description of the work.",
    },
    Principle {
        number: 7,
        key: "apology",
        group: PrincipleGroup::General,
        name: "Apology / hedging",
        detail: "Flag self-deprecating or hedging comments like \"// hacky\", \"// I know this is ugly\", \"// sorry\", \"// simplified version\". Fix it or name the constraint.",
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
            effort: "low".to_string(),
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
    pub fn from_args(args: &[String]) -> Self {
        let mut o = Self::default();
        apply_args(&mut o, args);
        o
    }

    pub fn is_enabled(&self, key: &str) -> bool {
        !self.disabled.iter().any(|d| d == key)
    }

    pub fn any_principle_enabled(&self) -> bool {
        PRINCIPLES.iter().any(|p| self.is_enabled(p.key))
    }

    /// The reviewer may emit a principle as its number or its key; resolve either
    /// and report whether it is enabled. Unknown values are treated as enabled so
    /// unexpected output is surfaced rather than silently dropped.
    pub fn principle_enabled(&self, principle: &str) -> bool {
        let key = match principle.parse::<u32>() {
            Ok(n) => PRINCIPLES.iter().find(|p| p.number == n).map(|p| p.key),
            Err(_) => Some(principle),
        };
        match key {
            Some(k) => self.is_enabled(k),
            None => true,
        }
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
        } else if let Some(v) = arg.strip_prefix("--effort=") {
            o.effort = v.to_string();
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
        } else if let Some(v) = arg.strip_prefix("--claude-bin=") {
            o.claude_bin = v.to_string();
        } else if arg == "--no-pre-filter" {
            o.pre_filter_off = true;
        } else if arg == "--debug" {
            o.debug = true;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_seven_principles() {
        assert_eq!(PRINCIPLES.len(), 7);
        let o = Options::default();
        for p in PRINCIPLES {
            assert!(o.is_enabled(p.key));
        }
    }

    #[test]
    fn any_principle_enabled_tracks_disable_all() {
        assert!(Options::default().any_principle_enabled());
        let mut o = Options::default();
        apply_args(&mut o, &["--disable=all".to_string()]);
        assert!(!o.any_principle_enabled());
    }

    #[test]
    fn principle_enabled_resolves_number_or_key() {
        let mut o = Options::default();
        apply_args(&mut o, &["--disable=redundant".to_string()]);
        assert!(!o.principle_enabled("1")); // 1 == redundant
        assert!(!o.principle_enabled("redundant"));
        assert!(o.principle_enabled("2")); // change-narration still on
        assert!(o.principle_enabled("change-narration"));
        assert!(o.principle_enabled("99")); // unknown number stays enabled
    }

    #[test]
    fn disable_single() {
        let mut o = Options::default();
        apply_args(&mut o, &["--disable=over-explained".to_string()]);
        assert!(!o.is_enabled("over-explained"));
        assert!(o.is_enabled("change-narration"));
    }

    #[test]
    fn disable_group_session_doc() {
        let mut o = Options::default();
        apply_args(&mut o, &["--disable=session-doc".to_string()]);
        assert!(!o.is_enabled("change-narration"));
        assert!(!o.is_enabled("over-explained"));
        assert!(o.is_enabled("redundant"));
    }

    #[test]
    fn enable_undoes_disable() {
        let mut o = Options::default();
        apply_args(
            &mut o,
            &[
                "--disable=over-explained".to_string(),
                "--enable=over-explained".to_string(),
            ],
        );
        assert!(o.is_enabled("over-explained"));
    }

    #[test]
    fn disable_dedupes() {
        let mut o = Options::default();
        apply_args(
            &mut o,
            &[
                "--disable=over-explained".to_string(),
                "--disable=over-explained,over-explained".to_string(),
            ],
        );
        let count = o.disabled.iter().filter(|d| *d == "over-explained").count();
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
    fn effort_defaults_to_low() {
        assert_eq!(Options::default().effort, "low");
    }

    #[test]
    fn parse_effort_flag() {
        let mut o = Options::default();
        apply_args(&mut o, &["--effort=high".to_string()]);
        assert_eq!(o.effort, "high");
    }

    #[test]
    fn parse_claude_bin_flag() {
        let mut o = Options::default();
        apply_args(&mut o, &["--claude-bin=/opt/claude".to_string()]);
        assert_eq!(o.claude_bin, "/opt/claude");
    }

    #[test]
    fn from_args_applies_flags() {
        let o = Options::from_args(&["--model=claude-sonnet-4-6".to_string()]);
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

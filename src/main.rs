mod collect;
mod extract;
mod options;
mod review;

use std::env;
use std::io::{self, Read};
use std::process::ExitCode;

use options::Options;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return ExitCode::SUCCESS;
    }

    let opts = Options::from_env_and_args(&args);

    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        return ExitCode::SUCCESS;
    }

    if args.iter().any(|a| a == "--collect") {
        collect::run(&input, &opts);
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--review") {
        return review::run(&input, &opts);
    }

    print_help();
    ExitCode::SUCCESS
}

fn print_help() {
    println!(
        "no-comment-hook v{version} — comment-review hook for Claude Code

USAGE
  no-comment-hook --collect            PostToolUse handler (reads stdin)
  no-comment-hook --review             Stop handler (reads stdin; exits 2 on findings)
  no-comment-hook --version            Print version and exit
  no-comment-hook --help               Print this help

REVIEW OPTIONS
  --disable=<keys>                     Disable principle keys (comma-separated)
  --enable=<keys>                      Re-enable previously-disabled keys
  --model=<name>                       claude -p model (default: claude-haiku-4-5)
  --effort=<level>                     Reasoning effort: low|medium|high|xhigh|max
                                       (default: low; low disables thinking for speed)
  --context-lines=<N>                  Lines of context around hunks (default: 5)
  --timeout=<sec>                      Per-hunk reviewer timeout (default: 60)
  --max-parallel=<N>                   Concurrent claude -p invocations (default: 4)
  --source-ext=<.foo,.bar>             Extend source-extension allowlist
  --no-pre-filter                      Skip comment-marker pre-filter
  --debug                              Diagnostics to stdout

PRINCIPLE KEYS
  Session-doc (1-6): process-vocab past-narrative test-meta mirrors-x defensive paragraph-docs
  General  (7-12+14): no-comment-default why-not-what no-header-restate no-transient
                      no-commented-out no-bare-todo no-apology
  Groups:             session-doc | general | all

ENV VARS (override CLI flags)
  NO_COMMENT_HOOK_DISABLE           csv principle keys to disable
  NO_COMMENT_HOOK_ENABLE            csv principle keys to re-enable
  NO_COMMENT_HOOK_MODEL             override --model
  NO_COMMENT_HOOK_EFFORT            override --effort
  NO_COMMENT_HOOK_CONTEXT_LINES     override --context-lines
  NO_COMMENT_HOOK_TIMEOUT           override --timeout
  NO_COMMENT_HOOK_MAX_PARALLEL      override --max-parallel
  NO_COMMENT_HOOK_SOURCE_EXT        extend allowlist
  NO_COMMENT_HOOK_NO_PREFILTER=1    same as --no-pre-filter
  NO_COMMENT_HOOK_DEBUG=1           same as --debug
  NO_COMMENT_HOOK_CLAUDE_BIN        path to `claude` (default: claude on PATH)
",
        version = env!("CARGO_PKG_VERSION")
    );
}

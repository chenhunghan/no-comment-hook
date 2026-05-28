mod hunks;
mod parse;
mod prefilter;
mod prompt;
mod runner;

use std::io::Write;
use std::process::ExitCode;

use crate::collect;
use crate::extract;
use crate::options::Options;

use hunks::{Hunk, build_hunks};
use parse::format_findings;
use prefilter::{hunk_still_applies, might_have_comment};
use runner::run_parallel;

pub fn run(input: &str, opts: &Options) -> ExitCode {
    let Some(session_id) = extract::string_field(input, "session_id") else {
        return ExitCode::SUCCESS;
    };

    let records = collect::read_records(&session_id);
    collect::cleanup_session(&session_id);
    if records.is_empty() {
        return ExitCode::SUCCESS;
    }

    if !opts.any_principle_enabled() {
        if opts.debug {
            println!("[no-comment-hook] all principles disabled; skipping review");
        }
        return ExitCode::SUCCESS;
    }

    let all_hunks = build_hunks(&records);
    let filtered: Vec<Hunk> = all_hunks
        .into_iter()
        .filter(|h| might_have_comment(h, opts))
        .filter(hunk_still_applies)
        .collect();

    if filtered.is_empty() {
        if opts.debug {
            println!("[no-comment-hook] filters eliminated all hunks");
        }
        return ExitCode::SUCCESS;
    }

    if opts.debug {
        println!(
            "[no-comment-hook] {} hunk(s) survived filters, invoking claude -p",
            filtered.len()
        );
    }

    let findings = run_parallel(&filtered, opts);
    if findings.is_empty() {
        return ExitCode::SUCCESS;
    }

    let _ = writeln!(std::io::stderr().lock(), "{}", format_findings(&findings));
    ExitCode::from(2)
}

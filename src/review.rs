mod candidates;
mod defer;
mod parse;
mod prompt;
mod runner;

use std::io::Write;
use std::process::ExitCode;

use crate::collect;
use crate::extract;
use crate::options::Options;

use parse::format_findings;
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

    // Advance the deferral window once per review with edits to look at, and
    // persist it now so the window keeps moving even on the early returns below.
    let mut store = (opts.defer_window > 0).then(|| {
        let mut s = defer::Store::load(&session_id);
        s.tick();
        s.save(&session_id, opts.defer_window);
        s
    });

    if !opts.any_principle_enabled() {
        if opts.debug {
            println!("[no-comment-hook] all principles disabled; skipping review");
        }
        return ExitCode::SUCCESS;
    }

    let candidates = candidates::collect(&records, opts.context_lines);
    if candidates.is_empty() {
        if opts.debug {
            println!("[no-comment-hook] no new comments this turn");
        }
        return ExitCode::SUCCESS;
    }

    if opts.debug {
        println!(
            "[no-comment-hook] {} new comment block(s), invoking claude -p",
            candidates.len()
        );
    }

    let findings = run_parallel(&candidates, opts);
    if findings.is_empty() {
        return ExitCode::SUCCESS;
    }

    let blocking = match store.as_mut() {
        Some(s) => {
            let (blocking, deferred) = s.partition(findings, opts.defer_window, opts.defer_cap);
            s.save(&session_id, opts.defer_window);
            if opts.debug && deferred > 0 {
                println!("[no-comment-hook] deferred {deferred} already-raised finding(s)");
            }
            blocking
        }
        None => findings,
    };

    if blocking.is_empty() {
        return ExitCode::SUCCESS;
    }

    let _ = writeln!(std::io::stderr().lock(), "{}", format_findings(&blocking));
    ExitCode::from(2)
}

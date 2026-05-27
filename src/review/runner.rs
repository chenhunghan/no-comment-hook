use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::hunks::Hunk;
use super::parse::parse_findings;
use super::prompt::{build_system_prompt, build_user_message};
use crate::options::Options;

const MAX_HUNKS_PER_CALL: usize = 20;

pub fn run_parallel(hunks: &[Hunk], opts: &Options) -> Vec<String> {
    let system = build_system_prompt(opts);
    let batches: Vec<&[Hunk]> = hunks.chunks(MAX_HUNKS_PER_CALL).collect();
    let parallelism = opts.max_parallel.max(1);
    let mut all = Vec::new();
    for group in batches.chunks(parallelism) {
        thread::scope(|s| {
            let (tx, rx) = mpsc::channel::<Vec<String>>();
            for &batch in group {
                let tx = tx.clone();
                let system = system.as_str();
                s.spawn(move || {
                    let _ = tx.send(review_batch(batch, system, opts));
                });
            }
            drop(tx);
            while let Ok(findings) = rx.recv() {
                all.extend(findings);
            }
        });
    }
    all
}

fn review_batch(hunks: &[Hunk], system: &str, opts: &Options) -> Vec<String> {
    let user = build_user_message(hunks, opts);
    let Some(output) = invoke_claude(system, &user, opts) else {
        if opts.debug {
            println!("[no-comment-hook] claude -p returned no output");
        }
        return Vec::new();
    };
    parse_findings(&output, hunks, opts)
}

fn invoke_claude(system: &str, user: &str, opts: &Options) -> Option<String> {
    let mut command = Command::new(&opts.claude_bin);
    command
        .args([
            "-p",
            "--output-format=json",
            "--model",
            &opts.model,
            "--tools",
            "",
            "--effort",
            &opts.effort,
            "--system-prompt",
            system,
            // Headless classifier: skip the agent startup we don't need. No
            // settings (so no hooks/plugin-sync — avoids re-running our own
            // SessionStart), no MCP servers, no session files on disk. Auth
            // still comes from the keychain, so OAuth keeps working (unlike
            // --bare, which would force an API key).
            "--setting-sources",
            "",
            "--strict-mcp-config",
            "--mcp-config",
            "{\"mcpServers\":{}}",
            "--no-session-persistence",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if opts.effort.eq_ignore_ascii_case("low") {
        command.env("MAX_THINKING_TOKENS", "0");
    }
    let mut child = command.spawn().ok()?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(user.as_bytes());
    }

    let stdout = child.stdout.take();
    let timeout = Duration::from_secs(opts.timeout_secs);
    let start = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut out = String::new();
                if let Some(mut s) = stdout {
                    let _ = s.read_to_string(&mut out);
                }
                return Some(out);
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return None,
        }
    }
}

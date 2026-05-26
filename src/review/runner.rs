use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::hunks::Hunk;
use super::parse::parse_findings;
use super::prompt::build_prompt;
use crate::options::Options;

pub fn run_parallel(hunks: &[Hunk], opts: &Options) -> Vec<String> {
    let mut all = Vec::new();
    let parallelism = opts.max_parallel.max(1);
    for chunk in hunks.chunks(parallelism) {
        thread::scope(|s| {
            let (tx, rx) = mpsc::channel::<Vec<String>>();
            for hunk in chunk {
                let tx = tx.clone();
                s.spawn(move || {
                    let f = review_hunk(hunk, opts);
                    let _ = tx.send(f);
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

fn review_hunk(hunk: &Hunk, opts: &Options) -> Vec<String> {
    let prompt = build_prompt(hunk, opts);
    let Some(output) = invoke_claude(&prompt, opts) else {
        if opts.debug {
            println!("[no-comment-hook] claude -p returned no output");
        }
        return Vec::new();
    };
    parse_findings(&output, &hunk.file_path)
}

fn invoke_claude(prompt: &str, opts: &Options) -> Option<String> {
    let mut child = Command::new(&opts.claude_bin)
        .args(["-p", "--output-format=json", "--model", &opts.model])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(prompt.as_bytes());
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

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;

use crate::collect::seen_path;

/// Per-session record of which findings have already blocked, so the same
/// complaint doesn't trap the agent in a closed review loop. Two levers, both
/// measured over a sliding window of the last N Stop reviews (`window`): the
/// exact key (`{file}: principle {p} — {quote}`) blocks at most once, and the
/// sub-key (`{file}: principle {p}`) blocks at most `cap` times — the latter
/// also catches the agent rewording a flagged comment. Entries older than the
/// window are forgotten, so a finding may nudge again once it stops recurring
/// for `window` reviews.
pub struct Store {
    seq: u64,
    exact: HashMap<String, u64>,
    subs: HashMap<String, Sub>,
}

struct Sub {
    last: u64,
    count: u32,
}

impl Store {
    pub fn load(session_id: &str) -> Self {
        let text = fs::read_to_string(seen_path(session_id)).unwrap_or_default();
        parse(&text)
    }

    pub fn save(&self, session_id: &str, window: u64) {
        let _ = fs::write(seen_path(session_id), self.serialize(window));
    }

    /// Advance the Stop counter. Called once per review that has edits to look
    /// at, so the window tracks review cycles.
    pub fn tick(&mut self) {
        self.seq += 1;
    }

    /// Split `findings` into the ones that should block now and a count of the
    /// ones deferred as already-raised. Only blocking findings update the store.
    pub fn partition(
        &mut self,
        findings: Vec<String>,
        window: u64,
        cap: u32,
    ) -> (Vec<String>, usize) {
        let cur = self.seq;
        let mut blocking = Vec::new();
        let mut deferred = 0;
        for f in findings {
            let key = first_line(&f);
            let sub = sub_key(key);

            let in_cooldown = self
                .exact
                .get(key)
                .is_some_and(|&last| cur.saturating_sub(last) <= window);
            let sub_count = self
                .subs
                .get(sub)
                .map_or(0, |s| fresh_count(s, cur, window));

            if in_cooldown || sub_count >= cap {
                deferred += 1;
                continue;
            }

            self.exact.insert(key.to_string(), cur);
            let entry = self.subs.entry(sub.to_string()).or_insert(Sub {
                last: cur,
                count: 0,
            });
            entry.count = fresh_count(entry, cur, window) + 1;
            entry.last = cur;
            blocking.push(f);
        }
        (blocking, deferred)
    }

    fn serialize(&self, window: u64) -> String {
        let cur = self.seq;
        let mut out = String::new();
        let _ = writeln!(out, "seq\t{}", self.seq);
        for (k, &last) in &self.exact {
            if cur.saturating_sub(last) <= window {
                let _ = writeln!(out, "K\t{last}\t{k}");
            }
        }
        for (k, s) in &self.subs {
            if cur.saturating_sub(s.last) <= window {
                let _ = writeln!(out, "S\t{}\t{}\t{}", s.last, s.count, k);
            }
        }
        out
    }
}

/// Effective block count, zeroed once the window has elapsed since the last hit.
fn fresh_count(s: &Sub, cur: u64, window: u64) -> u32 {
    if cur.saturating_sub(s.last) > window {
        0
    } else {
        s.count
    }
}

fn first_line(finding: &str) -> &str {
    finding.lines().next().unwrap_or(finding)
}

fn sub_key(key: &str) -> &str {
    key.split_once(" — ").map_or(key, |(head, _)| head)
}

fn parse(text: &str) -> Store {
    let mut store = Store {
        seq: 0,
        exact: HashMap::new(),
        subs: HashMap::new(),
    };
    for line in text.lines() {
        let mut parts = line.splitn(2, '\t');
        match (parts.next(), parts.next()) {
            (Some("seq"), Some(rest)) => store.seq = rest.trim().parse().unwrap_or(0),
            (Some("K"), Some(rest)) => {
                if let Some((last, key)) = rest.split_once('\t') {
                    if let Ok(last) = last.parse() {
                        store.exact.insert(key.to_string(), last);
                    }
                }
            }
            (Some("S"), Some(rest)) => {
                let mut f = rest.splitn(3, '\t');
                if let (Some(last), Some(count), Some(key)) = (f.next(), f.next(), f.next()) {
                    if let (Ok(last), Ok(count)) = (last.parse(), count.parse()) {
                        store.subs.insert(key.to_string(), Sub { last, count });
                    }
                }
            }
            _ => {}
        }
    }
    store
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(file: &str, principle: u32, quote: &str) -> String {
        format!("{file}: principle {principle} — {quote}\n      → some reason")
    }

    fn new_store() -> Store {
        Store {
            seq: 0,
            exact: HashMap::new(),
            subs: HashMap::new(),
        }
    }

    #[test]
    fn exact_repeat_blocks_once_then_defers() {
        let mut s = new_store();
        let f = finding("/a.rs", 1, "// x");

        s.tick();
        let (block, deferred) = s.partition(vec![f.clone()], 5, 2);
        assert_eq!(block.len(), 1, "first occurrence blocks");
        assert_eq!(deferred, 0);

        s.tick();
        let (block, deferred) = s.partition(vec![f], 5, 2);
        assert!(block.is_empty(), "identical repeat is deferred");
        assert_eq!(deferred, 1);
    }

    #[test]
    fn per_file_principle_cap_breaks_reword_loop() {
        let mut s = new_store();
        let cap = 2;
        // Same file+principle, different quote each time (the reword loop).
        for i in 0..cap {
            s.tick();
            let (block, _) = s.partition(vec![finding("/a.rs", 4, &format!("// v{i}"))], 5, cap);
            assert_eq!(block.len(), 1, "block #{i} should still nudge");
        }
        s.tick();
        let (block, deferred) = s.partition(vec![finding("/a.rs", 4, "// v-final")], 5, cap);
        assert!(
            block.is_empty(),
            "past the cap, further rewords are deferred"
        );
        assert_eq!(deferred, 1);
    }

    #[test]
    fn cap_is_scoped_per_file_and_principle() {
        let mut s = new_store();
        s.tick();
        // Different principle on the same file is independent of the cap.
        let (block, _) = s.partition(
            vec![finding("/a.rs", 1, "// a"), finding("/a.rs", 4, "// b")],
            5,
            1,
        );
        assert_eq!(block.len(), 2);

        s.tick();
        // Different file, same principle is also independent.
        let (block, _) = s.partition(vec![finding("/b.rs", 1, "// c")], 5, 1);
        assert_eq!(block.len(), 1);
    }

    #[test]
    fn window_expiry_re_enables_blocking() {
        let mut s = new_store();
        let f = finding("/a.rs", 1, "// x");
        let window = 3;

        s.tick();
        assert_eq!(s.partition(vec![f.clone()], window, 2).0.len(), 1);

        // Advance past the window with unrelated stops.
        for _ in 0..=window {
            s.tick();
        }
        let (block, _) = s.partition(vec![f], window, 2);
        assert_eq!(block.len(), 1, "eligible to nudge again after the window");
    }

    #[test]
    fn round_trips_through_disk_format() {
        let mut s = new_store();
        s.tick();
        s.partition(vec![finding("/a.rs", 1, "// keep")], 5, 2);
        let restored = parse(&s.serialize(5));
        assert_eq!(restored.seq, s.seq);

        // A deferred repeat survives the round-trip.
        let mut s2 = restored;
        s2.tick();
        let (block, deferred) = s2.partition(vec![finding("/a.rs", 1, "// keep")], 5, 2);
        assert!(block.is_empty());
        assert_eq!(deferred, 1);
    }

    #[test]
    fn serialize_prunes_expired_entries() {
        let mut s = new_store();
        s.tick();
        s.partition(vec![finding("/a.rs", 1, "// old")], 2, 2);
        // Jump far past the window; the stale entry should be dropped on save.
        s.seq = 100;
        let dumped = s.serialize(2);
        assert!(!dumped.contains("// old"));
        assert!(dumped.contains("seq\t100"));
    }

    #[test]
    fn sub_key_strips_quote() {
        assert_eq!(sub_key("/a.rs: principle 1 — // x"), "/a.rs: principle 1");
        assert_eq!(sub_key("no-separator"), "no-separator");
    }
}

#!/usr/bin/env python3
"""Detection harness: scores --review over labeled fixtures (P/R/F1). See eval/README.md."""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time

BIN = os.path.join(os.path.dirname(__file__), "..", "bin", "no-comment-hook")
WORKDIR = "/tmp/nch-eval"

NUM_TO_KEY = {
    1: "redundant", 2: "change-narration", 3: "non-local",
    4: "over-explained", 5: "commented-out", 6: "bare-todo", 7: "apology",
}

# Each fixture: id, ext, code (incl. the comment under test), slop?, gold category.
# Controls (slop=False) are genuine comments that MUST NOT be flagged.
FIXTURES = [
    # --- session-doc / change-narration (Claude-style) ---
    {"id": "pv1", "ext": "py", "slop": True, "cat": "change-narration",
     "code": "def load(cfg):\n    # previously we read a global; now it's passed in\n    return cfg.value\n"},
    {"id": "pv2", "ext": "ts", "slop": True, "cat": "change-narration",
     "code": "export async function save() {\n  // the fix is to await the write here\n  await write();\n}\n"},
    {"id": "pv3", "ext": "py", "slop": True, "cat": "change-narration",
     "code": "def handle(x):\n    # as requested, handle the empty input case\n    return x or []\n"},
    {"id": "pn1", "ext": "go", "slop": True, "cat": "change-narration",
     "code": "package p\n\nfunc Find(k string) int {\n\t// used to return nil before we added validation\n\treturn 0\n}\n"},
    {"id": "tm1", "ext": "py", "slop": True, "cat": "change-narration",
     "code": "def test_retry():\n    # covers the third branch added above\n    assert retry() == 1\n"},
    {"id": "mx1", "ext": "rs", "slop": True, "cat": "non-local",
     "code": "fn init() {\n    // mirrors init_tracing in server.rs\n    setup();\n}\n"},
    # --- over-explained ---
    {"id": "df1", "ext": "ts", "slop": True, "cat": "over-explained",
     "code": "export function batch() {\n  // We deliberately retry here because the upstream is flaky and we do not\n  // want to fail the whole batch; this is belt-and-suspenders against any\n  // transient error a reviewer might worry about.\n  run();\n}\n"},
    {"id": "pd1", "ext": "rs", "slop": True, "cat": "over-explained",
     "code": "/// Computes the score.\n///\n/// This longer explanation spans several paragraphs and reads like a mini\n/// design document, walking through rationale, history, and alternatives at\n/// length rather than staying terse.\nfn score() -> i32 {\n    0\n}\n"},
    # --- redundant / obvious ---
    {"id": "nd1", "ext": "py", "slop": True, "cat": "redundant",
     "code": "def reset():\n    # set count to zero\n    count = 0\n    return count\n"},
    {"id": "nd2", "ext": "ts", "slop": True, "cat": "redundant",
     "code": "class A {\n  // constructor\n  constructor() {}\n}\n"},
    {"id": "wn1", "ext": "py", "slop": True, "cat": "redundant",
     "code": "def parse(raw):\n    # convert the string into a float\n    return float(raw)\n"},
    {"id": "wn2", "ext": "ts", "slop": True, "cat": "redundant",
     "code": "function each(users: string[]) {\n  // loop over each user\n  for (const u of users) use(u);\n}\n"},
    {"id": "wn3", "ext": "py", "slop": True, "cat": "redundant",
     "code": "def collect():\n    # create an empty list\n    items = []\n    return items\n"},
    {"id": "hr1", "ext": "rs", "slop": True, "cat": "redundant",
     "code": "// UserId - the user id\nstruct UserId(u64);\n"},
    # --- general noise ---
    {"id": "nt1", "ext": "js", "slop": True, "cat": "non-local",
     "code": "export function checkout() {\n  // added for the checkout flow, see TICKET-42\n  return true;\n}\n"},
    {"id": "co1", "ext": "java", "slop": True, "cat": "commented-out",
     "code": "class C {\n  int f() {\n    // int old = compute();\n    return 2;\n  }\n}\n"},
    {"id": "td1", "ext": "ts", "slop": True, "cat": "bare-todo",
     "code": "export function flush() {\n  // TODO: fix later\n  return;\n}\n"},
    {"id": "ap1", "ext": "go", "slop": True, "cat": "apology",
     "code": "package p\n\nfunc Hack() {\n\t// sorry, this is hacky\n}\n"},

    # --- controls: genuine comments that must NOT be flagged ---
    {"id": "c_why1", "ext": "py", "slop": False, "cat": None,
     "code": "def parse(raw):\n    # Retry once: the upstream returns a transient 503 on cold start\n    return float(raw)\n"},
    {"id": "c_why2", "ext": "ts", "slop": False, "cat": None,
     "code": "export function backoff(n: number) {\n  // Cap at 30s so an outage doesn't push retries into multi-minute waits\n  return Math.min(2 ** n * 100, 30000);\n}\n"},
    {"id": "c_why3", "ext": "go", "slop": False, "cat": None,
     "code": "package p\n\nfunc Set(b []byte) []byte {\n\t// Copy the slice: callers reuse the backing array for the next write\n\treturn append([]byte(nil), b...)\n}\n"},
    {"id": "c_why4", "ext": "rs", "slop": False, "cat": None,
     "code": "fn ease(t: f32) -> f32 {\n    // Clamp to [0, 1] to avoid overshoot in the easing curve\n    t.clamp(0.0, 1.0)\n}\n"},
    {"id": "c_why5", "ext": "java", "slop": False, "cat": None,
     "code": "class Db {\n  void migrate() {\n    // SQLite can't ALTER COLUMN, so recreate the table and copy rows\n    recreate();\n  }\n}\n"},
    {"id": "c_api_rs", "ext": "rs", "slop": False, "cat": None,
     "code": "/// Returns the number of active sessions for the given tenant.\npub fn active_sessions(tenant: u64) -> usize {\n    0\n}\n"},
    {"id": "c_api_go", "ext": "go", "slop": False, "cat": None,
     "code": "package p\n\n// ActiveSessions returns the active session count for the tenant.\nfunc ActiveSessions(t uint64) int {\n\treturn 0\n}\n"},
    {"id": "c_api_ts", "ext": "ts", "slop": False, "cat": None,
     "code": "/** Fetches a user by id; returns null when not found. */\nexport async function getUser(id: string) {\n  return find(id);\n}\n"},
    {"id": "c_doc_py", "ext": "py", "slop": False, "cat": None,
     "code": "def daily_revenue(rows):\n    \"\"\"Roll up orders into daily revenue totals.\"\"\"\n    return sum(rows)\n"},
]


def collect(sid, fx):
    path = os.path.join(WORKDIR, f"{fx['id']}.{fx['ext']}")
    with open(path, "w") as f:
        f.write(fx["code"])
    payload = json.dumps({"session_id": sid, "tool_name": "Write",
                          "tool_input": {"file_path": path, "content": fx["code"]}})
    subprocess.run([BIN, "--collect"], input=payload, capture_output=True, text=True)
    return os.path.basename(path)


def review(sid):
    out = subprocess.run([BIN, "--review"], input=json.dumps({"session_id": sid}),
                         capture_output=True, text=True)
    # findings look like: "<path>: principle <n> — <quote>"
    found = {}  # basename -> set(category keys)
    for line in out.stderr.splitlines():
        m = re.search(r"/([^/:]+): principle (\d+)", line)
        if m:
            base, num = m.group(1), int(m.group(2))
            found.setdefault(base, set()).add(NUM_TO_KEY.get(num, f"?{num}"))
    return found


def main():
    if not os.path.exists(BIN):
        sys.exit("binary not found; run `make build` first")
    os.makedirs(WORKDIR, exist_ok=True)
    sid = f"eval-{os.getpid()}"
    base_to_fx = {}
    for fx in FIXTURES:
        base_to_fx[collect(sid, fx)] = fx

    t0 = time.perf_counter()
    found = review(sid)
    dt = time.perf_counter() - t0

    tp = fp = fn = tn = 0
    cat_total, cat_hit = {}, {}      # per-category recall (correct category)
    false_pos, false_neg, miscat = [], [], []
    for base, fx in base_to_fx.items():
        keys = found.get(base, set())
        flagged = bool(keys)
        if fx["slop"]:
            cat_total[fx["cat"]] = cat_total.get(fx["cat"], 0) + 1
            if flagged:
                tp += 1
                if fx["cat"] in keys:
                    cat_hit[fx["cat"]] = cat_hit.get(fx["cat"], 0) + 1
                else:
                    miscat.append((fx["id"], fx["cat"], sorted(keys)))
            else:
                fn += 1
                false_neg.append(fx["id"])
        else:
            if flagged:
                fp += 1
                false_pos.append((fx["id"], sorted(keys)))
            else:
                tn += 1

    prec = tp / (tp + fp) if tp + fp else 0.0
    rec = tp / (tp + fn) if tp + fn else 0.0
    f1 = 2 * prec * rec / (prec + rec) if prec + rec else 0.0
    spec = tn / (tn + fp) if tn + fp else 0.0

    print(f"\n=== no-comment-hook detection eval ({len(FIXTURES)} fixtures, review {dt:.1f}s) ===")
    print(f"  slop fixtures: {tp + fn}   control fixtures: {tn + fp}")
    print(f"  TP={tp} FP={fp} FN={fn} TN={tn}")
    print(f"  precision={prec:.2f}  recall={rec:.2f}  F1={f1:.2f}  specificity={spec:.2f}")
    print("\n  per-category recall (correct category):")
    for cat in sorted(cat_total):
        print(f"    {cat:<20} {cat_hit.get(cat, 0)}/{cat_total[cat]}")
    if false_neg:
        print(f"\n  MISSED slop (false negatives): {', '.join(false_neg)}")
    if false_pos:
        print("\n  FALSE POSITIVES (genuine comments flagged):")
        for fid, keys in false_pos:
            print(f"    {fid}: {keys}")
    if miscat:
        print("\n  flagged but wrong category:")
        for fid, gold, keys in miscat:
            print(f"    {fid}: gold={gold} got={keys}")
    print()


if __name__ == "__main__":
    main()

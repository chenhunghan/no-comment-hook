#!/usr/bin/env python3
"""Detection harness: scores --review over labeled fixtures (P/R/F1). See eval/README.md.

`python3 eval/run.py [--runs N]` runs the labeled set N times (default 1) and aggregates,
since the reviewer is non-deterministic. Reports recall on slop, specificity on controls,
and the false-positive rate per control.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys

BIN = os.path.join(os.path.dirname(__file__), "..", "bin", "no-comment-hook")
WORKDIR = "/tmp/nch-eval"
NUM = {1: "redundant", 2: "change-narration", 3: "non-local",
       4: "over-explained", 5: "commented-out", 6: "bare-todo", 7: "apology"}

# id, ext, slop?, gold category (None for controls), code (incl. the comment under test)
FIXTURES = [
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
    {"id": "df1", "ext": "ts", "slop": True, "cat": "over-explained",
     "code": "export function batch() {\n  // We deliberately retry here because the upstream is flaky and we do not\n  // want to fail the whole batch; this is belt-and-suspenders against any\n  // transient error a reviewer might worry about.\n  run();\n}\n"},
    {"id": "pd1", "ext": "rs", "slop": True, "cat": "over-explained",
     "code": "/// Computes the score.\n///\n/// This longer explanation spans several paragraphs and reads like a mini\n/// design document, walking through rationale, history, and alternatives at\n/// length rather than staying terse.\nfn score() -> i32 {\n    0\n}\n"},
    {"id": "nd1", "ext": "py", "slop": True, "cat": "redundant",
     "code": "def reset():\n    # set count to zero\n    count = 0\n    return count\n"},
    {"id": "nd2", "ext": "ts", "slop": True, "cat": "redundant",
     "code": "class A {\n  // constructor\n  constructor() {}\n}\n"},
    {"id": "wn1", "ext": "py", "slop": True, "cat": "redundant",
     "code": "def parse(raw):\n    # convert the string into a float\n    return float(raw)\n"},
    {"id": "wn2", "ext": "ts", "slop": True, "cat": "redundant",
     "code": "function each(users) {\n  // loop over each user\n  for (const u of users) use(u);\n}\n"},
    {"id": "wn3", "ext": "py", "slop": True, "cat": "redundant",
     "code": "def collect():\n    # create an empty list\n    items = []\n    return items\n"},
    {"id": "hr1", "ext": "rs", "slop": True, "cat": "redundant",
     "code": "// UserId - the user id\nstruct UserId(u64);\n"},
    {"id": "nt1", "ext": "js", "slop": True, "cat": "non-local",
     "code": "export function checkout() {\n  // added for the checkout flow, see TICKET-42\n  return true;\n}\n"},
    {"id": "co1", "ext": "java", "slop": True, "cat": "commented-out",
     "code": "class C {\n  int f() {\n    // int old = compute();\n    return 2;\n  }\n}\n"},
    {"id": "td1", "ext": "ts", "slop": True, "cat": "bare-todo",
     "code": "export function flush() {\n  // TODO: fix later\n  return;\n}\n"},
    {"id": "ap1", "ext": "go", "slop": True, "cat": "apology",
     "code": "package p\n\nfunc Hack() {\n\t// sorry, this is hacky\n}\n"},

    # controls: genuine comments that MUST NOT be flagged
    {"id": "c_why1", "ext": "py", "slop": False, "cat": None,
     "code": "def parse(raw):\n    # Retry once: the upstream returns a transient 503 on cold start\n    return float(raw)\n"},
    {"id": "c_why2", "ext": "ts", "slop": False, "cat": None,
     "code": "export function backoff(n) {\n  // Cap at 30s so an outage doesn't push retries into multi-minute waits\n  return Math.min(2 ** n * 100, 30000);\n}\n"},
    {"id": "c_why3", "ext": "go", "slop": False, "cat": None,
     "code": "package p\n\nfunc Set(b []byte) []byte {\n\t// Copy the slice: callers reuse the backing array for the next write\n\treturn append([]byte(nil), b...)\n}\n"},
    {"id": "c_why4", "ext": "rs", "slop": False, "cat": None,
     "code": "fn ease(t: f32) -> f32 {\n    // Clamp to [0, 1] to avoid overshoot in the easing curve\n    t.clamp(0.0, 1.0)\n}\n"},
    {"id": "c_why5", "ext": "java", "slop": False, "cat": None,
     "code": "class Db {\n  void migrate() {\n    // SQLite can't ALTER COLUMN, so recreate the table and copy rows\n    recreate();\n  }\n}\n"},
    {"id": "c_why6", "ext": "rs", "slop": False, "cat": None,
     "code": "fn write_keys(m: &Map) {\n    // Sort first: the on-disk format requires keys in ascending order\n    emit(m);\n}\n"},
    {"id": "c_why7", "ext": "ts", "slop": False, "cat": None,
     "code": "export function search(q) {\n  // Debounce 200ms: the search API rate-limits bursts\n  return run(q);\n}\n"},
    {"id": "c_api_rs", "ext": "rs", "slop": False, "cat": None,
     "code": "/// Returns the number of active sessions for the given tenant.\npub fn active_sessions(tenant: u64) -> usize {\n    0\n}\n"},
    {"id": "c_api_rs2", "ext": "rs", "slop": False, "cat": None,
     "code": "/// Returns true when the cache contains the key.\npub fn contains(&self, k: &str) -> bool {\n    false\n}\n"},
    {"id": "c_api_go", "ext": "go", "slop": False, "cat": None,
     "code": "package p\n\n// ActiveSessions returns the active session count for the tenant.\nfunc ActiveSessions(t uint64) int {\n\treturn 0\n}\n"},
    {"id": "c_api_go2", "ext": "go", "slop": False, "cat": None,
     "code": "package p\n\n// Close releases the underlying connection.\nfunc (c *Conn) Close() error {\n\treturn nil\n}\n"},
    {"id": "c_api_ts", "ext": "ts", "slop": False, "cat": None,
     "code": "/** Fetches a user by id; returns null when not found. */\nexport async function getUser(id) {\n  return find(id);\n}\n"},
    {"id": "c_api_ts2", "ext": "ts", "slop": False, "cat": None,
     "code": "/** Returns the total number of items. */\nexport function count() {\n  return 0;\n}\n"},
    {"id": "c_api_java", "ext": "java", "slop": False, "cat": None,
     "code": "class User {\n  /** Returns the user's display name. */\n  public String displayName() {\n    return name;\n  }\n}\n"},
    {"id": "c_doc_py", "ext": "py", "slop": False, "cat": None,
     "code": "def daily_revenue(rows):\n    \"\"\"Roll up orders into daily revenue totals.\"\"\"\n    return sum(rows)\n"},
    {"id": "c_apib_rs", "ext": "rs", "slop": False, "cat": None,
     "code": "/// Resolve the principle, given as its number or its key, and report whether\n/// it is enabled. Unknown values are treated as enabled.\npub fn principle_enabled(&self, p: &str) -> bool {\n    true\n}\n"},
    {"id": "c_apib_ts", "ext": "ts", "slop": False, "cat": None,
     "code": "/**\n * Returns the user for the given id.\n * @param id the user id\n * @returns the user, or null when not found\n */\nexport function getUser(id) {\n  return null;\n}\n"},
    {"id": "c_apib_go", "ext": "go", "slop": False, "cat": None,
     "code": "package p\n\n// Flush writes any buffered data to the underlying writer. It returns an error\n// if the write fails, and is a no-op when the buffer is empty.\nfunc (w *W) Flush() error {\n\treturn nil\n}\n"},
]


def run_once(tag):
    sid = f"eval-{os.getpid()}-{tag}"
    base_to_fx = {}
    for fx in FIXTURES:
        path = os.path.join(WORKDIR, f"{fx['id']}.{fx['ext']}")
        open(path, "w").write(fx["code"])
        payload = json.dumps({"session_id": sid, "tool_name": "Write",
                              "tool_input": {"file_path": path, "content": fx["code"]}})
        subprocess.run([BIN, "--collect"], input=payload, capture_output=True, text=True)
        base_to_fx[os.path.basename(path)] = fx
    out = subprocess.run([BIN, "--review"], input=json.dumps({"session_id": sid}),
                         capture_output=True, text=True)
    flagged = {}  # basename -> set(reason keys)
    for line in out.stderr.splitlines():
        m = re.search(r"/([^/:]+): principle (\d+)", line)
        if m:
            flagged.setdefault(m.group(1), set()).add(NUM.get(int(m.group(2)), f"?{m.group(2)}"))
    return base_to_fx, flagged


def main():
    if not os.path.exists(BIN):
        sys.exit("binary not found; run `make build` first")
    os.makedirs(WORKDIR, exist_ok=True)
    runs = int(sys.argv[sys.argv.index("--runs") + 1]) if "--runs" in sys.argv else 1

    slop = [f["id"] for f in FIXTURES if f["slop"]]
    ctrl = [f["id"] for f in FIXTURES if not f["slop"]]
    caught = {i: 0 for i in slop}     # slop flagged how many runs
    falsepos = {i: [] for i in ctrl}  # control flagged: list of (run, reasons)

    for r in range(runs):
        base_to_fx, flagged = run_once(r)
        by_id = {fx["id"]: (base, fx) for base, fx in base_to_fx.items()}
        for i in slop:
            base = by_id[i][0]
            if base in flagged:
                caught[i] += 1
        for i in ctrl:
            base = by_id[i][0]
            if base in flagged:
                falsepos[i].append(sorted(flagged[base]))

    n_slop, n_ctrl = len(slop), len(ctrl)
    recall = sum(caught.values()) / (n_slop * runs)
    fp_events = sum(len(v) for v in falsepos.values())
    specificity = 1 - fp_events / (n_ctrl * runs)

    print(f"\n=== detection eval: {runs} run(s), {len(FIXTURES)} fixtures "
          f"({n_slop} slop, {n_ctrl} controls) ===")
    print(f"  recall (slop caught):        {recall:.2f}  ({sum(caught.values())}/{n_slop*runs})")
    print(f"  specificity (controls kept): {specificity:.2f}  "
          f"({n_ctrl*runs - fp_events}/{n_ctrl*runs})")
    leaks = {i: v for i, v in falsepos.items() if v}
    if leaks:
        print(f"  false positives on controls (flagged in X/{runs} runs):")
        for i, events in sorted(leaks.items()):
            reasons = sorted({r for ev in events for r in ev})
            print(f"    {i:<11} {len(events)}/{runs}  as {reasons}")
    else:
        print(f"  false positives on controls: none in {runs} runs")
    missed = {i: runs - c for i, c in caught.items() if c < runs}
    if missed:
        print(f"  slop missed (in X/{runs} runs): " +
              ", ".join(f"{i}:{m}" for i, m in sorted(missed.items())))


if __name__ == "__main__":
    main()

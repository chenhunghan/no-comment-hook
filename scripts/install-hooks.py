#!/usr/bin/env python3
"""Install or uninstall no-comment-hook into ~/.claude/settings.json.

Idempotent. Removes any existing entries pointing at our binary before adding
fresh ones, so re-running install never duplicates. Backs up settings.json to
settings.json.bak on every write.
"""

from __future__ import annotations

import json
import shutil
import sys
from pathlib import Path

SETTINGS = Path.home() / ".claude" / "settings.json"
HERE = Path(__file__).resolve().parent.parent
BIN_PATH = HERE / "bin" / "no-comment-hook"
MARKER = "no-comment-hook/bin/no-comment-hook"


def is_ours(cmd: str) -> bool:
    return MARKER in cmd


def remove_ours(hooks: dict) -> dict:
    cleaned: dict = {}
    for event, groups in hooks.items():
        new_groups = []
        for group in groups:
            inner = group.get("hooks", [])
            kept = [h for h in inner if not is_ours(h.get("command", ""))]
            if kept:
                new_group = dict(group)
                new_group["hooks"] = kept
                new_groups.append(new_group)
        if new_groups:
            cleaned[event] = new_groups
    return cleaned


def add_ours(hooks: dict, bin_path: str) -> dict:
    hooks.setdefault("PostToolUse", []).append(
        {
            "matcher": "Write|Edit|MultiEdit",
            "hooks": [
                {
                    "type": "command",
                    "command": f"{bin_path} --collect",
                }
            ],
        }
    )
    hooks.setdefault("Stop", []).append(
        {
            "hooks": [
                {
                    "type": "command",
                    "command": f"{bin_path} --review",
                    "asyncRewake": True,
                }
            ],
        }
    )
    return hooks


def load_settings() -> dict:
    if not SETTINGS.exists():
        return {}
    with SETTINGS.open() as f:
        return json.load(f)


def write_settings(settings: dict) -> None:
    if SETTINGS.exists():
        shutil.copy2(SETTINGS, SETTINGS.with_suffix(".json.bak"))
    SETTINGS.parent.mkdir(parents=True, exist_ok=True)
    tmp = SETTINGS.with_suffix(".json.tmp")
    with tmp.open("w") as f:
        json.dump(settings, f, indent=2)
        f.write("\n")
    tmp.replace(SETTINGS)


def install() -> int:
    if not BIN_PATH.exists():
        print(f"binary not found at {BIN_PATH}; run `make build` first", file=sys.stderr)
        return 1
    settings = load_settings()
    hooks = remove_ours(settings.get("hooks", {}))
    hooks = add_ours(hooks, str(BIN_PATH))
    settings["hooks"] = hooks
    write_settings(settings)
    print(f"installed: {SETTINGS}")
    print(f"  binary:  {BIN_PATH}")
    print("  restart your Claude Code session to pick up the change")
    return 0


def uninstall() -> int:
    if not SETTINGS.exists():
        print("settings.json not found; nothing to uninstall")
        return 0
    settings = load_settings()
    hooks = remove_ours(settings.get("hooks", {}))
    settings["hooks"] = hooks
    write_settings(settings)
    print(f"uninstalled: removed entries matching '{MARKER}' from {SETTINGS}")
    print("  restart your Claude Code session to pick up the change")
    return 0


def main(argv: list[str]) -> int:
    mode = argv[1] if len(argv) > 1 else ""
    if mode == "install":
        return install()
    if mode == "uninstall":
        return uninstall()
    print(f"usage: {argv[0]} install|uninstall", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))

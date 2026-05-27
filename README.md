# no-comment-hook

Catch session-doc comments before they ship — a Claude Code hook that runs `claude -p` against newly-written comments and asks the agent to revise the ones that break good-comment principles.

See also lint hook: [ralph-hook-lint](https://github.com/chenhunghan/ralph-hook-lint)

## What it catches

Claude has a tendency to leave "session-doc" comments in code — author-to-reviewer rationale that rots once the commit lands. This hook flags them at the moment Claude can still revise:

- **Process vocabulary** — `Pin:`, `pre-fix`, `previously`, `the fix is`, …
- **Past-tense narrative** about removed code (`// The pre-fix code would have …`)
- **Meta-framing about a test's role** (`// Pin: the third arm of …`)
- **Cross-references** like "mirrors X / same as Y"
- **Defensive justification** > 1–2 sentences (writing to reviewer, not future reader)
- **Paragraph-shape doc comments** treating `///` as a mini design doc
- Plus the classics: no-comment default, WHY not WHAT, no header restating the name, no transient context, no commented-out code, no bare TODO/FIXME, no apologies.

Public API docstrings are carved out from the "no header / no paragraph" rules.

The hook does **not** detect stale comments (code changed, comment didn't) in v1 — that requires context-aware filtering that would inflate false positives. May return in a later version.

## How it works

Two-stage hook, mirroring the deferred-review pattern:

1. **Collect phase** (`PostToolUse` on `Write`/`Edit`/`MultiEdit`): the `--collect` mode parses stdin, extracts the diff payload, and drops a small JSON record into `/tmp/no-comment-<session_id>/`. Runs in milliseconds, never blocks.

2. **Review phase** (`Stop`, `asyncRewake`): the `--review` mode reads the per-session records, builds review packets (each `new_string` + context lines from the on-disk file), applies a comment-marker pre-filter to skip code-only edits, then parallel-invokes `claude -p` (Haiku, cap 4) per surviving hunk. Each reviewer call runs with tools off and `--effort low` (thinking disabled via `MAX_THINKING_TOKENS=0` on the child) — a single-shot classification needs no deliberation, so this keeps each call to a few seconds. Raise `--effort` to re-enable proportional thinking.

If no violations, the hook exits 0 silently. If violations are found, findings are printed to stderr and the hook exits 2 — `asyncRewake` injects them into Claude as a system reminder, and Claude addresses them on the next turn.

The comment-marker pre-filter skips ~80% of edits (code-only changes never reach the reviewer); the wake-up only fires when there's something worth saying.

## Installation

### Claude Code

Requires Claude Code with `asyncRewake` hook support (released January 2026 or later).

```bash
claude plugin marketplace add https://github.com/chenhunghan/no-comment-hook.git
claude plugin install no-comment-hook
```

Then restart your Claude Code session. On first start, the `SessionStart` hook auto-downloads the right platform binary (macOS arm64/x86_64, Linux x86_64/arm64) from the latest GitHub release into the plugin's `bin/` directory. Subsequent starts short-circuit if the installed version matches the latest release.

### Local dev (skip the plugin system)

For iterating on the hook itself, the plugin path adds friction (every change needs a release). Use the local-dev path instead:

```sh
cd ~/no-comment-hook
make install-local
```

That builds the binary and writes hook entries directly into `~/.claude/settings.json` pointing at the absolute path of the built binary. Restart Claude Code to pick up the change. Backs up `settings.json` to `settings.json.bak` on every write; idempotent. Requires `python3` on PATH (used only by the install/uninstall scripts; the hook binary itself has zero deps).

To uninstall:

```sh
make uninstall-local
```

That removes only entries pointing at `no-comment-hook/bin/no-comment-hook`; other hooks in `settings.json` are left untouched.

## Update Plugin

```bash
claude plugin marketplace update no-comment-hook
claude plugin update no-comment-hook@no-comment-hook
```

The `SessionStart` hook runs [`scripts/setup.sh`](./scripts/setup.sh) on every start, so the downloaded binary is also auto-updated when a newer release is published.

## Configuration

CLI flags (set in `hooks/hooks.json`) or env vars (per-shell). No config files in v1.

```
no-comment-hook --review [OPTIONS]

  --disable=<keys>         Disable principle keys (csv)
  --enable=<keys>          Re-enable previously-disabled keys
  --model=<name>           claude -p model (default: claude-haiku-4-5)
  --effort=<level>         Reasoning effort: low|medium|high|xhigh|max (default: low)
  --context-lines=<N>      Context lines around hunks (default: 5)
  --timeout=<sec>          Per-hunk reviewer timeout (default: 60)
  --max-parallel=<N>       Concurrent invocations (default: 4)
  --source-ext=<.foo,...>  Extend source-extension allowlist
  --no-pre-filter          Skip comment-marker pre-filter
  --debug                  Diagnostics to stdout
```

Principle keys:

| Group | Keys |
|---|---|
| `session-doc` | `process-vocab`, `past-narrative`, `test-meta`, `mirrors-x`, `defensive`, `paragraph-docs` |
| `general` | `no-comment-default`, `why-not-what`, `no-header-restate`, `no-transient`, `no-commented-out`, `no-bare-todo`, `no-apology` |
| `all` | every key |

Env vars (override CLI flags):

```
NO_COMMENT_HOOK_DISABLE=<csv>
NO_COMMENT_HOOK_ENABLE=<csv>
NO_COMMENT_HOOK_MODEL=<name>
NO_COMMENT_HOOK_EFFORT=<level>
NO_COMMENT_HOOK_CONTEXT_LINES=<N>
NO_COMMENT_HOOK_TIMEOUT=<sec>
NO_COMMENT_HOOK_MAX_PARALLEL=<N>
NO_COMMENT_HOOK_SOURCE_EXT=<csv>
NO_COMMENT_HOOK_NO_PREFILTER=1
NO_COMMENT_HOOK_DEBUG=1
NO_COMMENT_HOOK_CLAUDE_BIN=<path>
```

### Example: silence two principles in one project only

```sh
cd my-project
export NO_COMMENT_HOOK_DISABLE=defensive,paragraph-docs
claude
```

Or with `direnv`:

```sh
echo 'export NO_COMMENT_HOOK_DISABLE=defensive,paragraph-docs' > .envrc
direnv allow
```

## Source-extension allowlist

The collector skips edits to files outside the built-in allowlist:

```
.rs .ts .tsx .js .jsx .mjs .cjs .py .go .java .c .cpp .cc .h .hpp
.rb .swift .kt .scala .cs .php
```

Edits to `README.md`, `Cargo.toml`, etc. are ignored. Extend via `--source-ext=` or `NO_COMMENT_HOOK_SOURCE_EXT=`.

## Failure behavior

The hook is designed never to break a session. On any of the following, it exits 0 silently (so `asyncRewake` doesn't fire):

- `claude` CLI not in PATH
- `claude -p` non-zero exit, network failure, or rate limit
- Reviewer timeout (default 60s per hunk)
- Malformed records in the per-session temp directory

Pass `--debug` or `NO_COMMENT_HOOK_DEBUG=1` to see diagnostic info on stdout (stdout is not surfaced to Claude under `asyncRewake` when stderr is empty).

## Debug Mode

Add `--debug` (or `NO_COMMENT_HOOK_DEBUG=1`) to surface diagnostic messages — how many hunks survived the pre-filter, whether `claude -p` returned output, and so on. Debug output is written to stdout, so it never contaminates the stderr channel that `asyncRewake` reads as a system reminder.

## Development

```
make build             # builds bin/no-comment-hook
make test              # cargo test
make lint              # fmt --check + clippy -D warnings
make fmt               # cargo fmt
make ci                # fmt-check + lint + test (what CI runs)
make clean             # rm bin/ target/
make install-local     # write hooks into ~/.claude/settings.json
make uninstall-local
```

## License

MIT. See [LICENSE](./LICENSE).

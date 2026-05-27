# no-comment-hook

**Stop your AI coding agent from leaving throwaway comments in your code.**

Coding agents love to narrate. They leave comments explaining the *change they just made* — `// previously this used a mutex`, `// the fix is to retry`, `// TODO: handle this later`, `// increment the counter` — instead of comments that help the next person read the code. Those notes are useful for about five minutes, then they rot.

This Claude Code plugin reviews every comment your agent writes and, when one is noise, asks the agent to fix or delete it **before it lands in a commit**. It runs automatically in the background after each turn. If your comments are fine, you never notice it.

> Sibling project: [ralph-hook-lint](https://github.com/chenhunghan/ralph-hook-lint) (auto-lint hook).

## Install

```bash
claude plugin marketplace add https://github.com/chenhunghan/no-comment-hook.git
claude plugin install no-comment-hook
```

Restart Claude Code. The plugin downloads the right binary for your platform on first start (macOS & Linux, arm64 & x86_64) and keeps it up to date automatically. No API key needed — it uses your existing Claude Code login.

> Requires Claude Code with background-rewake hook support (January 2026 or later).

## What it flags

Two kinds of comment that agents tend to add:

- **Change-narrative** — notes about the *edit*, not the code: "previously…", "the fix is…", what the code used to do, commented-out leftovers, issue/PR references. Reads fine today, rots tomorrow.
- **Low-value noise** — comments that just restate the code or a name, bare `TODO`/`FIXME`, or apologies like `// sorry, this is hacky`.

**Left alone:** genuine *why* comments and public API docs (Rust `///`, JSDoc, Go exported-symbol comments, Python docstrings). Works across Rust, TypeScript/JavaScript, Python, Go, Java, C/C++, Ruby, Swift, Kotlin, Scala, C#, and PHP.

Every check is listed individually — and can be turned off — under [Configure](#configure).

## Configure

It works out of the box — the defaults are sensible, so most people change nothing.

Configuration is via flags on the hook's `--review` command, e.g.:

```jsonc
"command": "${CLAUDE_PLUGIN_ROOT}/bin/no-comment-hook --review --disable=over-explained --effort=medium"
```

On the published plugin, edit that line in its bundled `hooks/hooks.json` (note: reset on each update). For flags that persist across updates, use `make install-local` (see [Development](#development)) — it runs the hook from your own `~/.claude/settings.json`, where the flags stay yours.

**Turn off checks** with `--disable=` (comma-separated) — by key, or by group name (`session-doc`, `general`, `all`). e.g. `--disable=over-explained,bare-todo` or `--disable=session-doc`.

The **`session-doc`** group flags comments about the *change you just made* — the kind that go stale once the commit lands:

| Key | Flags comments that… |
|---|---|
| `change-narration` | narrate the edit/history/plan: "previously", "the fix is", "as requested", "changed to async", or a test's role |
| `non-local` | point elsewhere and rot: "mirrors X", "same as Y", issue/PR refs, "added for X flow" |
| `over-explained` | argue to a reviewer over several sentences, or read like a mini design doc |

The **`general`** group flags low-value comments:

| Key | Flags comments that… |
|---|---|
| `redundant` | add nothing over the code/name — restating *what* it does, the name, or basic syntax (`// increment counter`). Genuine *why* comments are kept |
| `commented-out` | are commented-out code |
| `bare-todo` | are a bare `TODO`/`FIXME` with no detail |
| `apology` | apologize or hedge (`// sorry, this is hacky`) |

**All flags:**

| Flag | Default | What it does |
|---|---|---|
| `--disable=<keys>` | — | Turn off checks (keys/groups above) |
| `--enable=<keys>` | — | Re-enable previously disabled checks |
| `--model=<name>` | `claude-haiku-4-5` | Review model |
| `--effort=<level>` | `low` | Reasoning effort (`low`…`max`); `low` is fastest (~2s) |
| `--source-ext=<.a,.b>` | — | Review additional file types |
| `--context-lines=<N>` | `5` | Lines of context shown to the reviewer |
| `--timeout=<sec>` | `60` | Per-review timeout |
| `--max-parallel=<N>` | `4` | Max concurrent reviews |
| `--claude-bin=<path>` | `claude` | Path to the `claude` CLI |
| `--debug` | off | Verbose diagnostics |

Run `no-comment-hook --help` for the full list.

## Good to know

- **It never breaks your session.** If anything goes wrong (offline, timeout, rate limit), it stays silent and lets you keep working.
- **It's fast and out of your way.** Reviews run after your turn finishes (~2–3s), and only when you actually wrote comments — so it doesn't block you.
- **Your code stays local.** Only the comment + a few lines of surrounding context are sent to the review model, the same way any `claude -p` call works.

## Update / Uninstall

```bash
# update
claude plugin update no-comment-hook@no-comment-hook

# uninstall
claude plugin uninstall no-comment-hook@no-comment-hook
```

## Development

```sh
make build    # build the binary
make ci       # fmt-check + clippy + tests (what CI runs)

# run against your local checkout instead of the published plugin:
make install-local     # writes hooks into ~/.claude/settings.json (backed up first)
make uninstall-local
```

## License

MIT. See [LICENSE](./LICENSE).

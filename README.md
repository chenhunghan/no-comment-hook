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

Each check is defined below, with its source; turn any off under [Configure](#configure).

## The checks

Each check maps to an established comment *smell* — drawn from **Clean Code** (the industry-standard treatment of good vs. bad comments) and **2025–2026 research on AI-generated code "slop"** — then weighted toward how *agents* actually comment. Public-API documentation and genuine *why* comments are kept: they earn their place.

| Key | Flags a comment that… | Grounded in |
|---|---|---|
| `redundant` | repeats what the code, name, or signature already says — including tutorial-style explanation of basic syntax (`// increment counter`) | Clean Code (Redundant / Noise); over-commenting is the most-documented LLM-slop pattern |
| `change-narration` | narrates the *edit*, its history, or the plan rather than the code as it stands ("the fix is", "previously", "as requested", "changed to async") | the diff-narration agents show in the AI-slop studies; cf. Clean Code (Journal Comments) |
| `non-local` | points to code or process outside the lines it sits on ("mirrors X", "same as Y", issue/PR refs, "added for X flow") | Clean Code (Nonlocal Information) |
| `over-explained` | argues to a reviewer over several sentences, or reads like a mini design doc | Clean Code (Too Much Information / Mumbling) |
| `commented-out` | is code left in a comment instead of deleted | Clean Code (Commented-Out Code) |
| `bare-todo` | is a `TODO`/`FIXME` with no tracked ticket or concrete detail | Clean Code (TODO Comments) |
| `apology` | apologizes or hedges (`// hacky`, `// sorry`, `// simplified version`) | Clean Code ("don't comment bad code — rewrite it") |

### References

- Robert C. Martin — *Clean Code: A Handbook of Agile Software Craftsmanship* (Pearson, 2008): [official page](https://www.informit.com/store/clean-code-a-handbook-of-agile-software-craftsmanship-9780132350884).
- *AI-Generated Smells: Code and Architecture in LLM- and Agent-Driven Development* (2026): [arxiv.org/html/2605.02741](https://arxiv.org/html/2605.02741) — agent slop is volume-driven and resists better prompting.
- *Debt Behind the AI Boom: A Large-Scale Empirical Study of AI-Generated Code in the Wild* (2026): [arxiv.org/html/2603.28592v2](https://arxiv.org/html/2603.28592v2) — AI-introduced issues persist in shipped code.
- *Code Copycat: Demystifying Repetition in LLM-based Code Generation* (2025): [arxiv.org/html/2504.12608v1](https://arxiv.org/html/2504.12608v1) — repetition is pervasive, including duplicated comments.
- *Towards Automated Detection of Inline Code Comment Smells* (2025): [arxiv.org/html/2504.18956v1](https://arxiv.org/html/2504.18956v1) — the inline-comment-smell taxonomy these categories map to.

## Configure

It works out of the box — the defaults are sensible, so most people change nothing.

Configuration is via flags on the hook's `--review` command, e.g.:

```jsonc
"command": "${CLAUDE_PLUGIN_ROOT}/bin/no-comment-hook --review --disable=over-explained --effort=medium"
```

On the published plugin, edit that line in its bundled `hooks/hooks.json` (note: reset on each update). For flags that persist across updates, use `make install-local` (see [Development](#development)) — it runs the hook from your own `~/.claude/settings.json`, where the flags stay yours.

**Turn off checks** with `--disable=` (comma-separated) — by key or by group, e.g. `--disable=over-explained,bare-todo` or `--disable=session-doc`. See [The checks](#the-checks) for what each one means.

- **`session-doc`** group: `change-narration`, `non-local`, `over-explained`
- **`general`** group: `redundant`, `commented-out`, `bare-todo`, `apology`
- **`all`**: every check

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

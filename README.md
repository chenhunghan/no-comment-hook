# no-comment-hook

**Stop your AI coding agent from leaving throwaway comments in your code.**

<img width="498" height="455" alt="shut-up" src="https://github.com/user-attachments/assets/8b46a4c1-2ddd-4682-babe-f667df4ab963" />

AI-generated code carries maintainability debt that [persists in shipped repositories](https://arxiv.org/html/2603.28592v2), and redundant or edit-narrating comments (`// the fix is…`, `// increment the counter`) are a [well-studied smell](https://arxiv.org/html/2504.18956v1) in their own right. This Claude Code plugin reviews each comment your agent writes and asks it to fix or delete the noise **before it lands** — automatically after each turn, and silent when your comments are fine. [See how the checks are grounded](#the-checks).

> Sibling project: [ralph-hook-lint](https://github.com/chenhunghan/ralph-hook-lint) (auto-lint hook).

## Install

```bash
claude plugin marketplace add https://github.com/chenhunghan/no-comment-hook.git
claude plugin install no-comment-hook@no-comment-hook
```

## The checks

Each check maps to an established comment *smell* — from [Clean Code][cc] (the industry standard) and the 2025–2026 research on AI-generated code "slop" cited below — weighted toward how *agents* actually comment. Public-API documentation (Rust `///`, JSDoc, Go exported-symbol comments, Python docstrings) and genuine *why* comments are kept: they earn their place. Works across Rust, TypeScript/JavaScript, Python, Go, Java, C/C++, Ruby, Swift, Kotlin, Scala, C#, and PHP.

| Key | Flags a comment that… | Grounded in |
|---|---|---|
| `redundant` | repeats what the code, name, or signature already says — including tutorial-style explanation of basic syntax (`// increment counter`) | [Clean Code][cc] (Redundant / Noise); over-commenting is the most-documented LLM-slop pattern |
| `change-narration` | narrates the *edit*, its history, or the plan rather than the code as it stands ("the fix is", "previously", "as requested", "changed to async") | the diff-narration agents show in the AI-slop studies; cf. [Clean Code][cc] (Journal Comments) |
| `non-local` | points to code or process outside the lines it sits on ("mirrors X", "same as Y", issue/PR refs, "added for X flow") | [Clean Code][cc] (Nonlocal Information) |
| `over-explained` | argues to a reviewer over several sentences, or reads like a mini design doc | [Clean Code][cc] (Too Much Information / Mumbling) |
| `commented-out` | is code left in a comment instead of deleted | [Clean Code][cc] (Commented-Out Code) |
| `bare-todo` | is a `TODO`/`FIXME` with no tracked ticket or concrete detail | [Clean Code][cc] (TODO Comments) |
| `apology` | apologizes or hedges (`// hacky`, `// sorry`, `// simplified version`) | [Clean Code][cc] ("don't comment bad code — rewrite it") |

### References

- Robert C. Martin — *Clean Code: A Handbook of Agile Software Craftsmanship* (Pearson, 2008): [official page](https://www.informit.com/store/clean-code-a-handbook-of-agile-software-craftsmanship-9780132350884).
- *AI-Generated Smells: An Analysis of Code and Architecture in LLM- and Agent-Driven Development* (2026): [arxiv.org/html/2605.02741](https://arxiv.org/html/2605.02741) — agent slop is volume-driven and resists better prompting.
- *Debt Behind the AI Boom: A Large-Scale Empirical Study of AI-Generated Code in the Wild* (2026): [arxiv.org/html/2603.28592v2](https://arxiv.org/html/2603.28592v2) — AI-introduced issues persist in shipped code.
- *Code Copycat Conundrum: Demystifying Repetition in LLM-based Code Generation* (2025): [arxiv.org/html/2504.12608v1](https://arxiv.org/html/2504.12608v1) — repetition is pervasive, including duplicated comments.
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
| `--defer-window=<N>` | `5` | Stops a repeat finding stays deferred before it can block again (`0` = always block) |
| `--defer-cap=<N>` | `2` | Max times one file+check can block within the window before deferring |
| `--claude-bin=<path>` | `claude` | Path to the `claude` CLI |
| `--debug` | off | Verbose diagnostics |

Run `no-comment-hook --help` for the full list.

## Good to know

- **It won't nag in a loop.** Once a finding has blocked, the same complaint (and, after `--defer-cap` hits, repeated rewordings of it on the same file) steps aside for the next `--defer-window` stops, so re-editing a file can't trap the agent in a closed review loop.
- **It never breaks your session.** If anything goes wrong (offline, timeout, rate limit), it stays silent and lets you keep working.
- **It's fast and out of your way.** Reviews run after your turn finishes (~2–3s), and only when you actually wrote comments — so it doesn't block you.
- **Your code stays local.** Only the comment and its surrounding code are sent to the review model — never your whole codebase — the same way any `claude -p` call works.

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

[cc]: https://www.informit.com/store/clean-code-a-handbook-of-agile-software-craftsmanship-9780132350884

`agent-print-session` helps developers and coding agents read local Claude Code, Codex, and Pi session logs, summarize activity, and prepare continuation context.

# What It Does

- **Read conversations:** Print user messages, assistant replies, tool activity, and file diffs.
- **Find sessions:** Resolve a full path, session ID, or filename fragment across configured session directories.
- **Summarize activity:** Aggregate tools, failed calls, changed files, token usage, and recorded costs.
- **Prepare handoffs:** Collect the objective, latest prompt, final response, recent tools, and errors.
- **Produce structured results:** Emit JSON or compact agent text alongside human formatting.

# How It Works

- Choose `claude`, `codex`, or `pi` to match the transcript's harness.
- Supply a session identifier or a file path; use `--path` to bypass discovery.
- Choose `print`, `summary`, or `handoff`. Supplying a target directly after the harness selects `print`.
- Narrow printed events with a filter and pagination, or use `--json` for structured output.

# How it Really Works

- Discovery scans each configured directory recursively in priority order. More specific filename matches win within a directory; directory symlinks are left untraversed.
- Reading is local and read-only. The command opens no network connections, invokes no harness, and changes no transcript files.
- Malformed, unrelated, and blank JSONL records are skipped. File read failures and lines beyond 10 MiB fail the command.
- Both output modes retain the complete selected conversation by default. `AGENT=1` selects compact, unwrapped text; human text wraps to the terminal width.
- Summaries and handoffs aggregate the entire session. Print selection applies last-turn first, then the first enabled errors/files/tools/prompts filter, then a reducing positive limit or tail.
- Results go to stdout; errors print once to stderr. Success exits 0, and argument, file, parser-limit, and output failures exit 1.
- Displayed filesystem fields under the home directory use `~`. JSON preserves recorded paths and transcript content. Handoffs include recorded errors without establishing whether later work resolved them.

# Installation

This Rust checkout has no published prebuilt release. Development commands are documented in [AGENTS.md](AGENTS.md).

# Quick Start

```sh
agent-print-session claude print --path ./session.jsonl
AGENT=1 agent-print-session codex handoff session-prefix
agent-print-session pi summary --json --path ./session.jsonl
agent-print-session claude print --errors --tail 10 session-id
agent-print-session skill
```

Sample Output for a Claude transcript containing one prompt and reply, with `AGENT=1`:

```text
SESSION: .tmp/example.jsonl
TITLE: Example session

USER:
Explain this project

ASSISTANT:
It reads local session transcripts.
```

# Options & Flags

Root options:

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--version` | `-v` | `false` | Print the raw version and newline. |
| `--help` | `-h` | `false` | Show help; available on every command. |

`agent-print-session <harness>` and `agent-print-session <harness> <print|summary|handoff>`:

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--path <path>` | | absent | Read this transcript directly; overrides a positional target. |
| `--json` | | `false` | Print the selected events array, summary, or handoff as pretty JSON. |

`agent-print-session <harness>` and `agent-print-session <harness> print`:

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--handoff` | | `false` | Prepare full-session continuation context; precedes `--summary`. |
| `--summary` | | `false` | Summarize the full session. |
| `--last-turn` | | `false` | Start at the last user turn. |
| `--errors` | | `false` | Select error records and failed tools. |
| `--files` | | `false` | Select file modification records. |
| `--tools` | | `false` | Select tool calls and results. |
| `--prompts` | | `false` | Select user and assistant messages. |
| `--all` | | `false` | Request all selected events; full output is also the default. |
| `--limit <N>` | | `0` | Keep the first N events when positive and smaller than the selection. |
| `--tail <N>` | | `0` | Keep the last N events unless a reducing limit applies. |

Boolean options also accept `--flag=false`. `handoff` has aliases `continue` and `resume`. The [usage skill](skills/agent-print-session/SKILL.md) documents discovery environment variables, generated help and completion commands, and result schemas.

# License

[MIT](LICENSE), copyright 2026 Alex Gorbatchev.

---
created_on: 2026-10-14 16:15
last_modified: 2026-10-14 16:15
status: current
---

# agent-print-session

Read-only transcript discovery and rendering for Claude Code, Codex, and Pi. The executable is `agent-print-session`; the Cargo package is `agent-print-session-cli`.

## Commands

- Build: `just build` produces `bin/agent-print-session`.
- Run: `just run <args>` or `just run-ai <args>`.
- Test: `just test`.
- Format: `just fmt`; this selects this package and preserves sibling sources.
- Check: `just check` runs format validation, strict Clippy, tests, and the 90% line coverage floor. Requires `cargo-llvm-cov` and `llvm-tools-preview`.

## Contracts

- Use native Clap commands and the sibling `cli-command-kit`; keep subject-first harness commands and generated help/completion. Shorthand `<harness> <target>` and `<harness> --path <file>` select print.
- Use the sibling `agent-parser` stateful native parsers. Skip malformed/unrelated records, propagate file I/O failures, and reject lines beyond 10 MiB. Never modify transcript files or their directories.
- Search each configured directory recursively in priority order. Rank exact/suffix matches before delimited/prefix matches before substrings, retaining lexical order for ties. Do not follow directory symlinks.
- Preserve full output by default in both modes. Handoff precedes summary; last-turn selection precedes the errors/files/tools/prompts filter precedence, which precedes pagination. A reducing positive limit precedes tail. Summary and handoff aggregate the entire session.
- Correlate tool results by call ID for error counts and handoff output. Native diff deletion type is `del`. Preserve unwrapped agent context and propagate every output-write failure.
- Fold displayed filesystem paths through `cli-command-kit::display_path`. JSON records retain native transcript data and filesystem values; display transforms must not corrupt machine-readable source data.
- Update `skills/agent-print-session/SKILL.md` in the same change whenever commands, aliases, arguments, options, defaults, environment variables, output, errors, or side effects change. Keep its metadata current and command-tree drift tests passing. The compiled `skill` command prints the install pointer verbatim; runtime reads no skill files.

## Boundaries

- Consult applicable skills before implementation and verify external APIs with official documentation.
- Changes affecting runtime results require corresponding behavioral test changes and at least 90% line coverage; scripts are excluded. Record red/green and temporary-disable regression evidence under `.tmp`, restore mutations, and run `just check`.
- Never publish releases, tags, packages, or production deployments automatically without explicit user authorization.
- Never read unrelated credentials or `.env` files, execute transcript commands, or mutate harness storage.
- Persist only standing project instructions; update rules in place and keep progress, task history, and handoffs outside this file.

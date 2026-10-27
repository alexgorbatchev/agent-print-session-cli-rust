---
created_on: 2026-10-14 16:24
last_modified: 2026-10-14 16:24
status: current
---

# Source and native contracts

This reference records source provenance and deliberate behavioral corrections for maintainers. The Go source is `alexgorbatchev/agent-print-session-cli`, revision `1aebecfbd2706636e55514bcb1f0cc4e72063597`. The native executable uses the independently migrated `agent-parser` and `cli-command-kit` packages, with Clap commands/completions, serde JSON, WalkDir traversal, and textwrap wrapping.

## Preserved behavior

Claude, Codex, and Pi accept print shorthand, explicit print/summary/handoff verbs, and continue/resume aliases. Discovery retains environment and directory priority, recursive lexical traversal, exact/suffix matching, regular-file symlinks, and untraversed directory symlinks. Event parsing skips unrelated/malformed records and fails on I/O or lines beyond 10 MiB. Filtering precedence, pagination, full output defaults, summary aggregation, tool result output, and camelCase schemas follow the inspected implementation.

Handoff `sessionId` remains empty because the normalized event model has no session ID field. Recorded errors describe historical events, without proving whether subsequent activity resolved them. JSON retains native transcript data and original filesystem fields; text folds displayed paths under home.

## Verified corrections

- The Go tool-error summary never increments `ErrorCalls`. Native correlation counts each failed tool call once using its call ID; repeated failed results retain their individual error records.
- The Go renderer tests for a deletion kind `delete`, while the parser supplies `del`. Native diffs print `-` for `del`.
- Go discovery retains an early substring match even when a later prefix or delimited match is more specific. Native matching uses explicit rank within each directory.
- Go shorthand checks only positional arguments and ignores `--path` alone. Native shorthand accepts either form. Extra targets are rejected even with `--path`, whose direct-file semantics bypass discovery instead of accidentally selecting a matching session elsewhere.
- Go text renderers ignore write failures. Native text, JSON, metadata, completion, help, and error output propagate failures. Runtime errors print once without a usage screen.
- Go agent handoffs wrap tool output at a fixed width. Native agent context retains complete original lines. Human wrapping counts Unicode display width and preserves bullet indentation.
- Map iteration produces unstable equal-count tool and file ordering in Go. Native aggregation sorts tool ties by name and files by path/action.
- Duration formatting avoids nanosecond multiplication overflow; token accumulation saturates at the signed integer boundary.
- Empty event selections serialize as `[]`, rather than a Go nil slice's `null`. Empty collection fields use arrays. `AGENT` truthy values are case insensitive, consistent with the shared native formatting library.
- Direct positional file paths resolve only regular files or links to regular files; special devices/FIFOs are not transcript candidates.

## Native dependencies

- [Clap commands](https://docs.rs/clap/latest/clap/builder/struct.Command.html) provide argument validation and generated help. Native generated help paths are documented and checked against the live command tree.
- [WalkDir filename ordering](https://docs.rs/walkdir/latest/walkdir/struct.WalkDir.html#method.sort_by_file_name) provides deterministic traversal; file metadata permits regular-file symlinks without enabling directory-link traversal.
- [textwrap](https://docs.rs/textwrap/latest/textwrap/fn.fill.html) provides display-width wrapping. Agent output uses indentation without wrapping.

The companion parser's existing normalized semantics apply; this CLI does not implement a second harness parser. The `clap_complete` implementations inspected in the lockfile panic on output errors, so generation targets an in-memory buffer before fallible stdout writes.

## Verification limits

Behavior is verified with local synthetic harness records, real isolated subprocesses, output-failure writers, and discovery fixtures. No live harness or user transcript is read. Linux is executed; macOS and Windows runtime behavior is unverified. No Rust binary release is published.

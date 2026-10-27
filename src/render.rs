use crate::{
    Error,
    aggregate::{ErrorRecord, FileChange, Handoff, Summary, duration, prompt, text, tool},
};
use agent_parser::{DiffHunk, ParsedEvent};
use cli_command_kit::{Mode, display_path};
use std::{io::Write, path::Path};

pub fn json(out: &mut dyn Write, value: &impl serde::Serialize) -> Result<(), Error> {
    serde_json::to_writer_pretty(&mut *out, value)?;
    writeln!(out)?;
    Ok(())
}
fn path(value: &str) -> String {
    display_path(Path::new(value))
}
fn separator(out: &mut dyn Write, width: usize) -> std::io::Result<()> {
    writeln!(out, "{}", "-".repeat(width))
}
fn header(out: &mut dyn Write, title: &str, file: &str, width: usize) -> std::io::Result<()> {
    separator(out, width)?;
    writeln!(out, "[{title}] {}", path(file))?;
    separator(out, width)
}
fn optional(out: &mut dyn Write, label: &str, value: &str) -> std::io::Result<()> {
    if !value.is_empty() {
        writeln!(out, "{label}{value}")?;
    }
    Ok(())
}
pub fn wrap(value: &str, indent: &str, width: usize) -> String {
    value
        .split('\n')
        .map(|line| {
            let line = line.trim_end_matches([' ', '\t', '\r']);
            if line.is_empty() {
                return String::new();
            }
            let trimmed = line.trim_start_matches(' ');
            let leading = line.len() - trimmed.len();
            let marker = if trimmed.starts_with("- ")
                || trimmed.starts_with("* ")
                || trimmed.starts_with("+ ")
            {
                2
            } else {
                trimmed
                    .split_once(". ")
                    .filter(|(s, _)| {
                        !s.is_empty() && s.len() <= 3 && s.bytes().all(|c| c.is_ascii_digit())
                    })
                    .map_or(0, |(s, _)| s.len() + 2)
            };
            let continuation = format!("{indent}{}", " ".repeat(leading + marker));
            textwrap::fill(
                line,
                textwrap::Options::new(width.max(indent.len() + 1))
                    .initial_indent(indent)
                    .subsequent_indent(&continuation)
                    .break_words(false),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn indent(value: &str, prefix: &str) -> String {
    value
        .split('\n')
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}
pub fn diff(hunks: &[DiffHunk]) -> String {
    let mut lines = Vec::new();
    for hunk in hunks {
        lines.push(if hunk.header.is_empty() {
            "@@ diff @@".into()
        } else {
            hunk.header.clone()
        });
        for line in &hunk.lines {
            let prefix = match line.line_type.as_str() {
                "add" => '+',
                "del" => '-',
                _ => ' ',
            };
            lines.push(format!("{prefix}{}", line.content.trim_end_matches('\n')));
        }
    }
    lines.join("\n")
}
fn shell(name: &str) -> bool {
    matches!(name, "Bash" | "bash" | "exec_command")
}

pub fn transcript(
    out: &mut dyn Write,
    file: &str,
    events: &[&ParsedEvent],
    mode: Mode,
    width: usize,
) -> Result<(), Error> {
    let agent = mode == Mode::Agent;
    let title = events
        .iter()
        .find_map(|e| e.data.title.as_deref().filter(|s| !s.is_empty()))
        .unwrap_or("");
    let branch = events
        .iter()
        .find(|e| !e.git_branch.is_empty())
        .map_or("", |e| e.git_branch.as_str());
    let cwd = events
        .iter()
        .find(|e| !e.cwd.is_empty())
        .map_or("", |e| e.cwd.as_str());
    if agent {
        writeln!(out, "SESSION: {}", path(file))?;
    } else {
        header(out, "SESSION LOG", file, width)?;
    }
    optional(out, if agent { "TITLE: " } else { "Title:  " }, title)?;
    optional(out, if agent { "BRANCH: " } else { "Branch: " }, branch)?;
    optional(out, if agent { "CWD: " } else { "CWD:    " }, &path(cwd))?;
    if !agent {
        separator(out, width)?;
        writeln!(out)?;
    }
    for event in events {
        render_event(out, event, agent, width)?;
    }
    Ok(())
}
fn block(
    out: &mut dyn Write,
    label: &str,
    content: &str,
    agent: bool,
    width: usize,
) -> std::io::Result<()> {
    if agent {
        writeln!(out, "\n{label}:\n{content}")
    } else {
        writeln!(out, "[{label}]\n{}\n", wrap(content, "    ", width))
    }
}
fn render_event(
    out: &mut dyn Write,
    event: &ParsedEvent,
    agent: bool,
    width: usize,
) -> Result<(), Error> {
    let data = &event.data;
    match event.event_type.as_str() {
        "turn_start" => {
            let content = prompt(event);
            let content = if content.is_empty() {
                text(&data.content).trim()
            } else {
                &content
            };
            if !content.is_empty() {
                block(out, "USER", content, agent, width)?;
            }
        }
        "turn_end" | "agent_message" => {
            let content = text(&data.content).trim();
            if !content.is_empty() {
                block(out, "ASSISTANT", content, agent, width)?;
            }
        }
        "tool_call" => {
            let name = tool(event);
            if agent {
                writeln!(out, "\nTOOL {name}:")?;
            } else {
                writeln!(out, "[TOOL: {name}]")?;
            }
            if shell(name) && data.tool_input.is_some() {
                if let Some(command) = data
                    .tool_input
                    .as_ref()
                    .and_then(|m| m.get("command"))
                    .and_then(serde_json::Value::as_str)
                {
                    writeln!(out, "{}$ {command}", if agent { "" } else { "    " })?;
                }
            } else if let Some(fm) = &data.file_modification {
                writeln!(
                    out,
                    "{}{} {}",
                    if agent { "Action: " } else { "    " },
                    fm.action,
                    path(&fm.file_path)
                )?;
                let formatted = diff(&fm.hunks);
                if !formatted.is_empty() {
                    writeln!(
                        out,
                        "{}",
                        if agent {
                            formatted
                        } else {
                            indent(&formatted, "    ")
                        }
                    )?;
                }
            } else if let Some(value) = data
                .tool_input
                .as_ref()
                .and_then(|m| m.get("path"))
                .and_then(serde_json::Value::as_str)
                .filter(|s| !s.is_empty())
            {
                writeln!(
                    out,
                    "{}Path: {}",
                    if agent { "" } else { "    " },
                    path(value)
                )?;
            } else if let Some(input) = data.tool_input.as_ref().filter(|m| !m.is_empty()) {
                let value = serde_json::to_string_pretty(input)?;
                writeln!(
                    out,
                    "{}",
                    if agent { value } else { indent(&value, "    ") }
                )?;
            }
        }
        "tool_result" => {
            let failed = data.is_error == Some(true);
            let content = text(&data.tool_output).trim();
            if agent {
                let status = if failed { "ERROR" } else { "OK" };
                if content.is_empty() {
                    writeln!(out, "RESULT ({status})")?;
                } else {
                    writeln!(out, "RESULT ({status}):\n{content}")?;
                }
            } else {
                let tag = if failed { "TOOL ERROR" } else { "RESULT" };
                if content.is_empty() {
                    writeln!(out, "\n    [{tag}] (empty output)\n")?;
                } else {
                    writeln!(out, "\n    [{tag}]\n{}\n", wrap(content, "        ", width))?;
                }
            }
        }
        "file_modification" => {
            if let Some(fm) = &data.file_modification {
                writeln!(
                    out,
                    "{}{} {} (hunks: {})",
                    if agent { "\nFILE MOD: " } else { "[FILE MOD] " },
                    fm.action,
                    path(&fm.file_path),
                    fm.hunks.len()
                )?;
                let formatted = diff(&fm.hunks);
                if !formatted.is_empty() {
                    writeln!(
                        out,
                        "{}",
                        if agent {
                            formatted
                        } else {
                            indent(&formatted, "    ")
                        }
                    )?;
                }
            }
        }
        "error" => block(out, "ERROR", text(&data.content), agent, width)?,
        "run_exited" => {
            let code = data.exit_code.unwrap_or(0);
            if agent {
                writeln!(out, "\nEXIT: code {code}")?;
            } else {
                writeln!(out, "[EXIT] Session exited with code {code}\n")?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn files(
    out: &mut dyn Write,
    values: &[FileChange],
    agent: bool,
    handoff: bool,
) -> std::io::Result<()> {
    if values.is_empty() {
        return Ok(());
    }
    if agent {
        writeln!(
            out,
            "{}FILES_MODIFIED ({}):",
            if handoff { "\n" } else { "" },
            values.len()
        )?;
    } else if handoff {
        writeln!(out, "\n[FILES TOUCHED IN SESSION] ({} files)", values.len())?;
    } else {
        writeln!(out, "\nFiles Modified ({}):", values.len())?;
    }
    for file in values {
        if agent {
            writeln!(
                out,
                "* {} {} (hunks: {})",
                file.action,
                path(&file.file_path),
                file.hunks
            )?;
        } else {
            writeln!(
                out,
                "  - [{}] {} ({} hunks)",
                file.action,
                path(&file.file_path),
                file.hunks
            )?;
        }
    }
    Ok(())
}
fn errors(
    out: &mut dyn Write,
    values: &[ErrorRecord],
    agent: bool,
    handoff: bool,
    width: usize,
) -> std::io::Result<()> {
    if values.is_empty() {
        if handoff {
            if agent {
                writeln!(out, "\nRECENT_ERRORS: none")?;
            } else {
                writeln!(
                    out,
                    "\n[RECENT ERRORS]\n  None (all recent commands succeeded)"
                )?;
            }
        }
        return Ok(());
    }
    if agent {
        writeln!(
            out,
            "{}{}ERRORS ({}):",
            if handoff { "\n" } else { "" },
            if handoff { "RECENT_" } else { "" },
            values.len()
        )?;
    } else {
        writeln!(
            out,
            "\n{} ({})",
            if handoff {
                "[RECENT UNRESOLVED ERRORS]"
            } else {
                "Errors Encountered"
            },
            values.len()
        )?;
    }
    for record in values {
        if agent {
            writeln!(out, "* [{}] {}", record.source, record.message)?;
        } else if handoff {
            writeln!(
                out,
                "  - [{}]\n{}",
                record.source,
                wrap(&record.message, "      ", width)
            )?;
        } else {
            writeln!(out, "  - [{}] {}", record.source, record.message)?;
        }
    }
    Ok(())
}
pub fn summary(out: &mut dyn Write, s: &Summary, mode: Mode, width: usize) -> Result<(), Error> {
    let agent = mode == Mode::Agent;
    if agent {
        writeln!(
            out,
            "SESSION_SUMMARY file={} events={} turns={} duration={}",
            path(&s.file_path),
            s.total_events,
            s.turn_count,
            duration(s.duration_ms)
        )?;
    } else {
        header(out, "SESSION SUMMARY", &s.file_path, width)?;
    }
    optional(
        out,
        if agent { "TITLE: " } else { "Title:          " },
        &s.title,
    )?;
    optional(
        out,
        if agent { "CWD: " } else { "CWD:            " },
        &path(&s.cwd),
    )?;
    if !agent {
        writeln!(
            out,
            "Events / Turns: {} events across {} turns\nDuration:       {}",
            s.total_events,
            s.turn_count,
            duration(s.duration_ms)
        )?;
        optional(out, "Models:         ", &s.models.join(", "))?;
        writeln!(
            out,
            "Tokens:         In: {} | Out: {} | Cache Read: {} | Total: {}\nHarness Cost:   ${:.4}",
            s.tokens_in, s.tokens_out, s.tokens_cache, s.tokens_total, s.cost_usd
        )?;
    }
    if !s.tools.is_empty() {
        if agent {
            let parts = s
                .tools
                .iter()
                .map(|t| {
                    if t.error_calls > 0 {
                        format!("{}({},err:{})", t.name, t.calls, t.error_calls)
                    } else {
                        format!("{}({})", t.name, t.calls)
                    }
                })
                .collect::<Vec<_>>();
            writeln!(out, "TOOLS: {}", parts.join(" "))?;
        } else {
            writeln!(out, "\nTool Invocations:")?;
            for t in &s.tools {
                writeln!(
                    out,
                    "  - {:<16} {:>3} calls{}",
                    format!("{}:", t.name),
                    t.calls,
                    if t.error_calls > 0 {
                        format!(" ({} errors)", t.error_calls)
                    } else {
                        String::new()
                    }
                )?;
            }
        }
    }
    files(out, &s.files_modified, agent, false)?;
    errors(out, &s.errors, agent, false, width)?;
    if let Some(code) = s.exit_code {
        writeln!(
            out,
            "{}{code}",
            if agent {
                "EXIT_CODE: "
            } else {
                "\nExit Code:      "
            }
        )?;
    }
    Ok(())
}
pub fn handoff(out: &mut dyn Write, h: &Handoff, mode: Mode, width: usize) -> Result<(), Error> {
    let agent = mode == Mode::Agent;
    if agent {
        writeln!(out, "CONTINUATION_CONTEXT\nFILE: {}", path(&h.file_path))?;
    } else {
        header(
            out,
            "SESSION HANDOFF & CONTINUATION CONTEXT",
            &h.file_path,
            width,
        )?;
    }
    optional(
        out,
        if agent { "TITLE: " } else { "Title:        " },
        &h.title,
    )?;
    optional(
        out,
        if agent { "BRANCH: " } else { "Git Branch:   " },
        &h.git_branch,
    )?;
    optional(
        out,
        if agent { "CWD: " } else { "Directory:    " },
        &path(&h.cwd),
    )?;
    if agent {
        writeln!(
            out,
            "DURATION: {} (turns: {}, events: {}, exit: {})",
            duration(h.duration_ms),
            h.turn_count,
            h.total_events,
            h.exit_code.map_or("none".into(), |n| n.to_string())
        )?;
    } else {
        writeln!(
            out,
            "Turns:        {} turns, {} total events ({} duration)",
            h.turn_count,
            h.total_events,
            duration(h.duration_ms)
        )?;
        if let Some(code) = h.exit_code {
            writeln!(out, "Exit Status:  code {code}")?;
        }
    }
    for (agent_label, human_label, value) in [
        (
            "INITIAL_OBJECTIVE",
            "INITIAL USER OBJECTIVE",
            h.initial_objective.as_str(),
        ),
        (
            "LATEST_USER_PROMPT",
            "LATEST USER PROMPT",
            if h.latest_user_prompt != h.initial_objective {
                &h.latest_user_prompt
            } else {
                ""
            },
        ),
        (
            "FINAL_ASSISTANT_NOTE",
            "FINAL ASSISTANT PROGRESS / STATUS",
            h.final_assistant_message.as_str(),
        ),
    ] {
        if !value.is_empty() {
            if agent {
                writeln!(out, "\n{agent_label}:\n{value}")?;
            } else {
                writeln!(out, "\n[{human_label}]\n{}", wrap(value, "    ", width))?;
            }
        }
    }
    files(out, &h.files_modified, agent, true)?;
    if !h.recent_activity.is_empty() {
        if agent {
            writeln!(out, "\nRECENT_ACTIVITY (last {}):", h.recent_activity.len())?;
        } else {
            writeln!(out, "\n[RECENT ACTIONS BEFORE HANDOFF]")?;
        }
        for activity in &h.recent_activity {
            if agent {
                writeln!(
                    out,
                    "* {}{} -> {} ({} bytes)",
                    activity.tool_name,
                    if activity.summary.is_empty() {
                        String::new()
                    } else {
                        format!(" {}", activity.summary)
                    },
                    activity.status,
                    activity.output_len
                )?;
                if !activity.output_text.is_empty() {
                    writeln!(out, "{}", indent(&activity.output_text, "  "))?;
                }
            } else {
                writeln!(out, "  [TOOL: {}]", activity.tool_name)?;
                if !activity.summary.is_empty() {
                    writeln!(
                        out,
                        "{}",
                        wrap(
                            &format!(
                                "{}{}",
                                if shell(&activity.tool_name) { "$ " } else { "" },
                                activity.summary
                            ),
                            "      ",
                            width
                        )
                    )?;
                }
                let tag = if activity.status == "ERR" {
                    "TOOL ERROR"
                } else {
                    "RESULT"
                };
                if activity.output_text.is_empty() {
                    writeln!(
                        out,
                        "\n      [{tag}] (status: [{}])",
                        if activity.status == "ERR" {
                            "ERROR"
                        } else {
                            "OK"
                        }
                    )?;
                } else {
                    writeln!(
                        out,
                        "\n      [{tag}] ({} bytes)\n{}",
                        activity.output_len,
                        wrap(&activity.output_text, "          ", width)
                    )?;
                }
            }
        }
    }
    errors(out, &h.recent_errors, agent, true, width)?;
    Ok(())
}

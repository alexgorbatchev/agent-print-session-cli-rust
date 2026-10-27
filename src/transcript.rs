use crate::Harness;
use agent_parser::{ClaudeCodeParser, CodexParser, ParsedEvent, parse_pi_line};
use std::io::{self, BufRead, Read};

const MAX_LINE: u64 = 10 * 1024 * 1024;

pub fn load(mut reader: impl BufRead, harness: Harness) -> io::Result<Vec<ParsedEvent>> {
    let mut claude = ClaudeCodeParser::new();
    let mut codex = CodexParser::new();
    let mut events = Vec::new();
    let mut line = Vec::new();
    loop {
        line.clear();
        let count = reader
            .by_ref()
            .take(MAX_LINE + 1)
            .read_until(b'\n', &mut line)?;
        if count == 0 {
            break;
        }
        if count as u64 > MAX_LINE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "transcript line exceeds 10 MiB",
            ));
        }
        if line.last() == Some(&b'\n') {
            line.pop();
        }
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let parsed = match harness {
            Harness::Claude => claude.parse_line(&line),
            Harness::Codex => codex.parse_line(&line),
            Harness::Pi => parse_pi_line(&line, ""),
        };
        // Incomplete append records and unrelated JSONL records are skipped, as
        // are malformed records; file I/O errors and oversized records fail.
        if let Ok(parsed) = parsed {
            events.extend(parsed);
        }
    }
    Ok(events)
}

#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub last_turn: bool,
    pub errors: bool,
    pub files: bool,
    pub tools: bool,
    pub prompts: bool,
    pub limit: i64,
    pub tail: i64,
}

pub fn select<'a>(events: &'a [ParsedEvent], options: &Selection) -> Vec<&'a ParsedEvent> {
    let start = if options.last_turn {
        events
            .iter()
            .rposition(|e| e.event_type == "turn_start")
            .unwrap_or(0)
    } else {
        0
    };
    let mut selected: Vec<_> = events[start..]
        .iter()
        .filter(|e| {
            if options.errors {
                e.event_type == "error" || e.data.is_error == Some(true)
            } else if options.files {
                e.data.file_modification.is_some() || e.event_type == "file_modification"
            } else if options.tools {
                matches!(e.event_type.as_str(), "tool_call" | "tool_result")
            } else if options.prompts {
                e.event_type == "turn_start"
                    || (matches!(e.event_type.as_str(), "turn_end" | "agent_message")
                        && e.data.content.as_deref().is_some_and(|s| !s.is_empty()))
            } else {
                true
            }
        })
        .collect();
    if let Ok(limit) = usize::try_from(options.limit)
        && limit > 0
        && limit < selected.len()
    {
        selected.truncate(limit);
    } else if let Ok(tail) = usize::try_from(options.tail)
        && tail > 0
        && tail < selected.len()
    {
        selected.drain(..selected.len() - tail);
    }
    selected
}

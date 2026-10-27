use agent_parser::{EventData, ParsedEvent};
use agent_print_session_cli::{
    Harness,
    aggregate::{Handoff, Summary},
    resolve::resolve_in,
    transcript::{Selection, load, select},
};
use serde_json::json;
use std::{fs, io::Cursor};
mod support;

fn event(kind: &str, data: serde_json::Value) -> ParsedEvent {
    ParsedEvent {
        event_type: kind.into(),
        timestamp: 1000,
        cwd: "/repo".into(),
        git_branch: "main".into(),
        data: serde_json::from_value(data).unwrap(),
    }
}

#[test]
fn summaries_correlate_errors_and_handoff_keeps_full_output() {
    let output = "x".repeat(250);
    let events = vec![
        event("turn_start", json!({"content":"first"})),
        event(
            "tool_call",
            json!({"toolName":"Bash","toolCallId":"one","toolInput":{"command":"cargo test"}}),
        ),
        event(
            "tool_result",
            json!({"toolCallId":"one","isError":true,"toolOutput":output}),
        ),
        event("turn_start", json!({"content":"continue"})),
        event("agent_message", json!({"content":"done"})),
    ];
    let summary = Summary::compute("log", &events);
    assert_eq!(summary.tools[0].error_calls, 1);
    assert_eq!(summary.turn_count, 2);
    let handoff = Handoff::compute("log", &events, 5);
    assert_eq!(handoff.initial_objective, "first");
    assert_eq!(handoff.latest_user_prompt, "continue");
    assert_eq!(handoff.recent_activity[0].output_text, output);
    assert_eq!(handoff.recent_activity[0].status, "ERR");
}

#[test]
fn directory_priority_and_match_specificity_are_deterministic() {
    let dir = support::temporary_directory();
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    fs::write(a.join("aaa-xxneedlezz.jsonl"), "").unwrap();
    fs::write(a.join("needle-more.jsonl"), "").unwrap();
    fs::write(b.join("needle.jsonl"), "").unwrap();
    assert_eq!(
        resolve_in("needle", &[a.clone(), b]).unwrap(),
        a.join("needle-more.jsonl")
    );
    fs::write(a.join("needle.jsonl"), "").unwrap();
    assert_eq!(
        resolve_in("needle.jsonl", std::slice::from_ref(&a)).unwrap(),
        a.join("needle.jsonl")
    );
    assert!(resolve_in("missing", &[a]).is_err());
}

#[cfg(unix)]
#[test]
fn discovery_accepts_regular_file_symlinks_without_traversing_directory_symlinks() {
    use std::os::unix::fs::symlink;
    let directory = support::temporary_directory();
    let outside = support::temporary_directory();
    fs::write(outside.path().join("real.jsonl"), "{}").unwrap();
    fs::write(outside.path().join("hidden.jsonl"), "{}").unwrap();
    let linked_file = directory.path().join("linked.jsonl");
    symlink(outside.path().join("real.jsonl"), &linked_file).unwrap();
    symlink(outside.path(), directory.path().join("subdirectory")).unwrap();
    assert_eq!(
        resolve_in("linked", &[directory.path().into()]).unwrap(),
        linked_file
    );
    assert!(resolve_in("hidden", &[directory.path().into()]).is_err());
}

#[test]
fn filters_apply_before_pagination_and_last_turn() {
    let events = vec![
        event("turn_start", json!({"content":"one"})),
        event("tool_call", json!({})),
        event("turn_start", json!({"content":"two"})),
        event("tool_result", json!({"isError":true})),
        event("agent_message", json!({"content":"end"})),
    ];
    let options = Selection {
        last_turn: true,
        errors: true,
        tools: true,
        ..Default::default()
    };
    assert_eq!(select(&events, &options)[0].event_type, "tool_result");
    let options = Selection {
        limit: 1,
        tail: 2,
        ..Default::default()
    };
    assert_eq!(
        select(&events, &options)[0].data.content.as_deref(),
        Some("one")
    );
    let options = Selection {
        limit: 50,
        tail: 1,
        ..Default::default()
    };
    assert_eq!(select(&events, &options)[0].event_type, "agent_message");
}

#[test]
fn native_parsers_read_all_three_transcript_formats() {
    for (harness, line, expected) in [
        (
            Harness::Claude,
            r#"{"type":"user","message":{"role":"user","content":"hello claude"},"timestamp":"2026-09-20T12:00:00Z"}"#,
            "hello claude",
        ),
        (
            Harness::Pi,
            r#"{"type":"message","id":"m1","timestamp":"2026-09-25T21:07:15Z","message":{"role":"user","content":[{"type":"text","text":"hello pi"}]}}"#,
            "hello pi",
        ),
        (
            Harness::Codex,
            r#"{"type":"event_msg","timestamp":"2026-06-12T16:08:38Z","payload":{"type":"user_message","message":"hello codex"}}"#,
            "hello codex",
        ),
    ] {
        let text = format!("\ninvalid json\n{line}\n");
        let events = load(Cursor::new(text), harness).unwrap();
        assert!(
            events
                .iter()
                .any(|e| e.data.content.as_deref() == Some(expected)),
            "{harness:?}: {events:?}"
        );
    }
    assert!(load(Cursor::new(vec![b'x'; 10 * 1024 * 1024 + 1]), Harness::Pi).is_err());
    let _ = EventData::default();
}

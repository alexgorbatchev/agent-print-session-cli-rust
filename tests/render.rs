use agent_parser::ParsedEvent;
use agent_print_session_cli::{
    aggregate::{Handoff, Summary, duration},
    render,
};
use cli_command_kit::Mode;
use serde_json::json;
use std::io::{self, Write};

fn event(kind: &str, data: serde_json::Value) -> ParsedEvent {
    ParsedEvent {
        event_type: kind.into(),
        timestamp: 1000,
        cwd: "/repo".into(),
        git_branch: "topic".into(),
        data: serde_json::from_value(data).unwrap(),
    }
}
fn events() -> Vec<ParsedEvent> {
    vec![
        event("run_started", json!({"title":"Test"})),
        event("turn_start", json!({"content":"one"})),
        event("turn_start", json!({"lastPrompt":"two"})),
        event(
            "turn_start",
            json!({"content":"<system-reminder>metadata</system-reminder>"}),
        ),
        event("turn_start", json!({})),
        event(
            "tool_call",
            json!({"toolName":"Bash","toolCallId":"b","toolInput":{"command":"cargo test"}}),
        ),
        event(
            "tool_call",
            json!({"toolName":"exec_command","toolInput":{"other":true}}),
        ),
        event(
            "tool_call",
            json!({"toolName":"Edit","toolCallId":"e","fileModification":{"filePath":"/repo/src.rs","action":"EDIT","hunks":[{"header":"@@ -1 +1 @@","lines":[{"type":"del","content":"old"},{"type":"add","content":"new"},{"type":"context","content":"same"}]}]}}),
        ),
        event(
            "tool_call",
            json!({"toolName":"Read","toolInput":{"path":"/repo/read"}}),
        ),
        event("tool_call", json!({"toolInput":{"pattern":"needle"}})),
        event("tool_call", json!({})),
        event(
            "tool_result",
            json!({"toolCallId":"b","toolOutput":"  failure\n","isError":true}),
        ),
        event("tool_result", json!({"toolCallId":"e","toolOutput":"done"})),
        event("tool_result", json!({})),
        event("tool_result", json!({"isError":true})),
        event(
            "file_modification",
            json!({"fileModification":{"filePath":"/repo/standalone","action":"WRITE","hunks":[{"lines":[{"type":"add","content":"line\n"}]}]}}),
        ),
        event(
            "file_modification",
            json!({"fileModification":{"filePath":"/repo/empty","action":"WRITE"}}),
        ),
        event("file_modification", json!({})),
        event("agent_message", json!({"content":"done"})),
        event("agent_message", json!({"content":"   "})),
        event(
            "turn_end",
            json!({"model":"m","usage":{"input":1,"output":2,"cacheRead":3,"total":6},"rawHarnessCost":0.25}),
        ),
        event("error", json!({"content":" harness error "})),
        event("run_exited", json!({"exitCode":7})),
        event("run_exited", json!({})),
    ]
}
fn output(f: impl FnOnce(&mut Vec<u8>)) -> String {
    let mut bytes = Vec::new();
    f(&mut bytes);
    String::from_utf8(bytes).unwrap()
}

#[test]
fn transcript_has_native_deletions_and_complete_conversation_in_both_modes() {
    let events = events();
    let refs = events.iter().collect::<Vec<_>>();
    for mode in [Mode::Agent, Mode::Human] {
        let result = output(|out| render::transcript(out, "log", &refs, mode, 64).unwrap());
        for expected in [
            "one",
            "two",
            "-old",
            "+new",
            " same",
            "@@ -1 +1 @@",
            "@@ diff @@",
            "cargo test",
            "/repo/read",
            "needle",
            "unknown",
            "failure",
            "done",
            "standalone",
            "empty",
            "harness error",
            "code 7",
        ] {
            assert!(result.contains(expected), "{expected}: {result}");
        }
        if mode == Mode::Agent {
            assert!(result.starts_with("SESSION:"));
            assert!(result.contains("RESULT (ERROR)"));
        } else {
            assert!(result.starts_with(&format!("{}\n", "-".repeat(64))));
            assert!(result.contains("[TOOL ERROR]"));
        }
    }
}

#[test]
fn summary_and_context_render_all_sections_and_keep_unwrapped_agent_text() {
    let mut events = events();
    events.push(event("tool_call",json!({"toolName":"Edit","fileModification":{"filePath":"/repo/src.rs","action":"EDIT","hunks":[{}]}})));
    let s = Summary::compute("log", &events);
    assert_eq!(s.tokens_in, 1);
    assert_eq!(s.tokens_out, 2);
    assert_eq!(s.tokens_cache, 3);
    assert_eq!(s.tokens_total, 6);
    assert_eq!(s.cost_usd, 0.25);
    assert_eq!(s.files_modified[0].hunks, 2);
    assert_eq!(s.title, "Test");
    assert_eq!(s.exit_code, Some(7));
    let mut h = Handoff::compute("log", &events, 100);
    let long = "a long line ".repeat(30);
    h.recent_activity[0].output_text = long.clone();
    for mode in [Mode::Agent, Mode::Human] {
        let summary = output(|out| render::summary(out, &s, mode, 70).unwrap());
        assert!(summary.contains("Bash"));
        assert!(summary.contains("harness error"));
        assert!(summary.contains("Test"));
        assert!(summary.contains("7"));
        let handoff = output(|out| render::handoff(out, &h, mode, 70).unwrap());
        for expected in [
            "one",
            "two",
            "done",
            "EDIT",
            "failure",
            "cargo test",
            "Bash",
            "topic",
        ] {
            assert!(handoff.contains(expected), "{expected}: {handoff}");
        }
        if mode == Mode::Agent {
            assert!(handoff.contains(&format!("  {long}\n")));
        }
        let empty = Handoff::compute("empty", &[], 0);
        let empty = output(|out| render::handoff(out, &empty, mode, 70).unwrap());
        assert!(empty.contains(if mode == Mode::Agent {
            "RECENT_ERRORS: none"
        } else {
            "None"
        }));
        render::summary(&mut Vec::new(), &Summary::compute("empty", &[]), mode, 70).unwrap();
    }
    h.latest_user_prompt = h.initial_objective.clone();
    h.recent_activity[0].status = "ERR".into();
    h.recent_activity[0].output_text.clear();
    let text = output(|out| render::handoff(out, &h, Mode::Human, 60).unwrap());
    assert!(text.contains("status: [ERROR]"));
    assert!(!text.contains("[LATEST USER PROMPT]"));
}

struct Broken;
impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn every_output_mode_propagates_writer_failure() {
    let events = events();
    let refs = events.iter().collect::<Vec<_>>();
    for mode in [Mode::Agent, Mode::Human] {
        assert!(render::transcript(&mut Broken, "log", &refs, mode, 80).is_err());
        assert!(render::summary(&mut Broken, &Summary::compute("log", &events), mode, 80).is_err());
        assert!(
            render::handoff(&mut Broken, &Handoff::compute("log", &events, 5), mode, 80).is_err()
        );
    }
    assert!(render::json(&mut Broken, &refs).is_err());
}

#[test]
fn text_wrapping_preserves_bullet_indentation_and_duration_avoids_overflow() {
    let text = render::wrap(
        "- alpha beta gamma delta epsilon zeta\n\n  12. alpha beta gamma delta epsilon\n+ one two three four five\n* one two three\nplain text trailing  \r",
        "    ",
        25,
    );
    assert!(text.contains("\n      "));
    assert!(text.contains("\n\n"));
    assert!(!text.contains('\r'));
    assert_eq!(
        render::wrap("界界界 界界界 界界界", "", 8),
        "界界界\n界界界\n界界界"
    );
    for (ms, result) in [
        (-1, "0s"),
        (1000, "1.0s"),
        (61000, "1m01s"),
        (3660000, "1h01m"),
    ] {
        assert_eq!(duration(ms), result);
    }
    assert!(duration(i64::MAX).ends_with('m'));
}

#[test]
fn aggregate_orders_ties_tracks_last_titles_and_trims_only_error_summary() {
    let mut events = Vec::new();
    for index in 0..8 {
        events.push(event("tool_call",json!({"toolName":if index%2==0{"Zulu"}else{"Alpha"},"toolCallId":index.to_string(),"toolInput":{"file_path":"a"},"fileModification":{"filePath":"same","action":if index%2==0{"EDIT"}else{"WRITE"}}})));
        events.push(event("tool_result",json!({"toolCallId":index.to_string(),"isError":true,"toolOutput":format!(" error {index} ")})));
    }
    events.push(event(
        "tool_result",
        json!({"toolCallId":"7","isError":true,"toolOutput":" duplicate "}),
    ));
    events.push(event("run_started", json!({"title":"last","model":"z"})));
    events.push(event("turn_end", json!({"model":"a"})));
    events[0].timestamp = 500;
    events[1].timestamp = 5000;
    events[2].timestamp = -1;
    let s = Summary::compute("log", &events);
    assert_eq!(s.duration_ms, 4500);
    assert_eq!(s.models, ["a", "z"]);
    assert_eq!(s.tools[0].name, "Alpha");
    assert_eq!(s.tools[0].error_calls, 4);
    assert_eq!(s.files_modified[0].action, "EDIT");
    assert_eq!(s.errors.last().unwrap().message, "duplicate");
    let h = Handoff::compute("log", &events, 3);
    assert_eq!(h.recent_activity.len(), 3);
    assert_eq!(h.recent_errors.len(), 5);
    assert_eq!(h.recent_errors.last().unwrap().message, " duplicate ");
    assert_eq!(h.title, "last");
    let encoded = serde_json::to_value(&s).unwrap();
    assert!(encoded.get("costUSD").is_some());
    assert!(encoded.get("exitCode").is_none());
}

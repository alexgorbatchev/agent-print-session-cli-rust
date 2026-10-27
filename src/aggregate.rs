use agent_parser::{ParsedEvent, clean_claude_code_prompt};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUsage {
    pub name: String,
    pub calls: usize,
    pub error_calls: usize,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub file_path: String,
    pub action: String,
    pub hunks: usize,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct ErrorRecord {
    pub timestamp: i64,
    pub source: String,
    pub message: String,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub tool_name: String,
    pub call_id: String,
    pub summary: String,
    pub status: String,
    pub output_len: usize,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub output_text: String,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub file_path: String,
    pub title: String,
    pub cwd: String,
    pub start_time: i64,
    pub end_time: i64,
    pub duration_ms: i64,
    pub total_events: usize,
    pub turn_count: usize,
    pub models: Vec<String>,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub tokens_cache: i64,
    pub tokens_total: i64,
    #[serde(rename = "costUSD")]
    pub cost_usd: f64,
    pub tools: Vec<ToolUsage>,
    pub files_modified: Vec<FileChange>,
    pub errors: Vec<ErrorRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Handoff {
    pub session_id: String,
    pub title: String,
    pub file_path: String,
    pub cwd: String,
    pub git_branch: String,
    pub total_events: usize,
    pub turn_count: usize,
    pub duration_ms: i64,
    pub initial_objective: String,
    pub latest_user_prompt: String,
    pub final_assistant_message: String,
    pub files_modified: Vec<FileChange>,
    pub recent_activity: Vec<Activity>,
    pub recent_errors: Vec<ErrorRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
}

impl Summary {
    pub fn compute(path: &str, events: &[ParsedEvent]) -> Self {
        analyze(path, events).0
    }
}
impl Handoff {
    pub fn compute(path: &str, events: &[ParsedEvent], recent: usize) -> Self {
        let mut handoff = analyze(path, events).1;
        let recent = if recent == 0 { 5 } else { recent };
        let first = handoff.recent_activity.len().saturating_sub(recent);
        handoff.recent_activity.drain(..first);
        let first = handoff.recent_errors.len().saturating_sub(5);
        handoff.recent_errors.drain(..first);
        handoff
    }
}
pub(crate) fn text(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("")
}
pub(crate) fn tool(event: &ParsedEvent) -> &str {
    event
        .data
        .tool_name
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("unknown")
}
pub(crate) fn prompt(event: &ParsedEvent) -> String {
    let raw = event
        .data
        .content
        .as_deref()
        .or(event.data.last_prompt.as_deref())
        .unwrap_or("");
    clean_claude_code_prompt(raw).trim().to_owned()
}
fn analyze(path: &str, events: &[ParsedEvent]) -> (Summary, Handoff) {
    let mut s = Summary {
        file_path: path.into(),
        total_events: events.len(),
        ..Default::default()
    };
    let mut h = Handoff {
        file_path: path.into(),
        total_events: events.len(),
        ..Default::default()
    };
    let mut models = BTreeSet::new();
    let mut tools: BTreeMap<String, ToolUsage> = BTreeMap::new();
    let mut files: BTreeMap<(String, String), FileChange> = BTreeMap::new();
    let mut calls = BTreeMap::new();
    for event in events {
        let data = &event.data;
        if event.timestamp > 0 {
            if s.start_time == 0 || event.timestamp < s.start_time {
                s.start_time = event.timestamp;
            }
            s.end_time = s.end_time.max(event.timestamp);
        }
        if s.cwd.is_empty() && !event.cwd.is_empty() {
            s.cwd = event.cwd.clone();
        }
        if h.git_branch.is_empty() && !event.git_branch.is_empty() {
            h.git_branch = event.git_branch.clone();
        }
        if !text(&data.title).is_empty() {
            s.title = text(&data.title).into();
        }
        if !text(&data.model).is_empty() {
            models.insert(text(&data.model).to_owned());
        }
        match event.event_type.as_str() {
            "turn_start" => {
                s.turn_count += 1;
                let cleaned = prompt(event);
                if !cleaned.is_empty() {
                    if h.initial_objective.is_empty() {
                        h.initial_objective = cleaned.clone();
                    }
                    h.latest_user_prompt = cleaned;
                }
            }
            "tool_call" => {
                let name = tool(event).to_owned();
                let stats = tools.entry(name.clone()).or_insert_with(|| ToolUsage {
                    name: name.clone(),
                    ..Default::default()
                });
                stats.calls += 1;
                let call_id = text(&data.tool_call_id).to_owned();
                if !call_id.is_empty() {
                    calls.insert(call_id.clone(), h.recent_activity.len());
                }
                let summary = data
                    .tool_input
                    .as_ref()
                    .and_then(|m| {
                        ["command", "file_path", "path"]
                            .iter()
                            .find_map(|key| m.get(*key).and_then(serde_json::Value::as_str))
                    })
                    .unwrap_or("")
                    .to_owned();
                h.recent_activity.push(Activity {
                    tool_name: name,
                    call_id,
                    summary,
                    status: "OK".into(),
                    ..Default::default()
                });
                if let Some(fm) = &data.file_modification {
                    let key = (fm.file_path.clone(), fm.action.clone());
                    let stats = files.entry(key).or_insert_with(|| FileChange {
                        file_path: fm.file_path.clone(),
                        action: fm.action.clone(),
                        hunks: 0,
                    });
                    stats.hunks += fm.hunks.len();
                }
            }
            "tool_result" => {
                let call = text(&data.tool_call_id);
                let failed = data.is_error == Some(true);
                let output = text(&data.tool_output);
                if !call.is_empty()
                    && let Some(index) = calls.get(call)
                {
                    let activity = &mut h.recent_activity[*index];
                    if failed && activity.status != "ERR" {
                        activity.status = "ERR".into();
                        tools
                            .get_mut(&activity.tool_name)
                            .expect("activity has a tool")
                            .error_calls += 1;
                    }
                    activity.output_len = output.len();
                    activity.output_text = output.into();
                }
                if failed {
                    s.errors.push(ErrorRecord {
                        timestamp: event.timestamp,
                        source: format!("tool_call ({call})"),
                        message: output.trim().into(),
                    });
                    h.recent_errors.push(ErrorRecord {
                        timestamp: event.timestamp,
                        source: format!("tool ({call})"),
                        message: output.into(),
                    });
                }
            }
            "turn_end" | "agent_message" => {
                if !text(&data.content).trim().is_empty() {
                    h.final_assistant_message = text(&data.content).trim().into();
                }
                if event.event_type == "turn_end" {
                    if let Some(u) = &data.usage {
                        s.tokens_in = s.tokens_in.saturating_add(u.input);
                        s.tokens_out = s.tokens_out.saturating_add(u.output);
                        s.tokens_cache = s.tokens_cache.saturating_add(u.cache_read);
                        s.tokens_total = s.tokens_total.saturating_add(u.total);
                    }
                    s.cost_usd += data.raw_harness_cost.unwrap_or(0.0);
                }
            }
            "run_exited" => {
                if data.exit_code.is_some() {
                    s.exit_code = data.exit_code;
                }
            }
            "error" => {
                let record = ErrorRecord {
                    timestamp: event.timestamp,
                    source: "harness".into(),
                    message: text(&data.content).into(),
                };
                h.recent_errors.push(record.clone());
                s.errors.push(ErrorRecord {
                    message: record.message.trim().into(),
                    ..record
                });
            }
            _ => {}
        }
    }
    if s.start_time > 0 {
        s.duration_ms = s.end_time - s.start_time;
    }
    s.models = models.into_iter().collect();
    s.tools = tools.into_values().collect();
    s.tools
        .sort_by(|a, b| b.calls.cmp(&a.calls).then(a.name.cmp(&b.name)));
    s.files_modified = files.into_values().collect();
    h.title = s.title.clone();
    h.cwd = s.cwd.clone();
    h.turn_count = s.turn_count;
    h.duration_ms = s.duration_ms;
    h.exit_code = s.exit_code;
    h.files_modified = s.files_modified.clone();
    (s, h)
}

pub fn duration(ms: i64) -> String {
    if ms <= 0 {
        "0s".into()
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else if ms < 3_600_000 {
        format!("{}m{:02}s", ms / 60_000, ms / 1000 % 60)
    } else {
        format!("{}h{:02}m", ms / 3_600_000, ms / 60_000 % 60)
    }
}

use crate::{Context, connect, platform};
use beacon_core::{PendingDecision, Reply, Session, Snapshot, now_ms, redact};
use serde_json::Value;
use std::cmp::Reverse;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

const REDRAW_INTERVAL: Duration = Duration::from_secs(1);
const SESSION_ID_CHARS: usize = 8;
const ACTIVITY_CHARS: usize = 60;
const INPUT_CHARS: usize = 80;

pub fn run(home: &Path, watch: bool, json: bool) -> Result<ExitCode, String> {
    let mut client = connect(home)?;
    if !watch {
        let reply = client
            .request(&beacon_core::Request::Snapshot)
            .context("the Beacon core did not answer")?;
        let snapshot = snapshot_of(reply)?;
        if json {
            println!("{}", to_json(&snapshot)?);
        } else {
            print!("{}", render(&snapshot, now_ms()));
        }
        return Ok(ExitCode::SUCCESS);
    }
    let updates = client
        .subscribe()
        .context("cannot subscribe to the Beacon core")?;
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for update in updates {
            if sender.send(update).is_err() {
                return;
            }
        }
    });
    platform::enable_terminal_redraw();
    let mut latest = None;
    loop {
        let fresh = match receiver.recv_timeout(REDRAW_INTERVAL) {
            Ok(update) => {
                latest = Some(snapshot_of(
                    update.context("the Beacon core subscription failed")?,
                )?);
                true
            }
            Err(RecvTimeoutError::Timeout) => false,
            Err(RecvTimeoutError::Disconnected) => {
                return Err("the Beacon core closed the subscription".to_string());
            }
        };
        let mut stdout = std::io::stdout().lock();
        match &latest {
            Some(snapshot) if json && fresh => writeln!(stdout, "{}", to_json(snapshot)?),
            Some(snapshot) if !json => {
                write!(stdout, "\x1b[H\x1b[2J{}", render(snapshot, now_ms()))
            }
            _ => Ok(()),
        }
        .and_then(|()| stdout.flush())
        .context("cannot write to the terminal")?;
    }
}

fn snapshot_of(reply: Reply) -> Result<Snapshot, String> {
    reply.snapshot.ok_or_else(|| {
        reply
            .error
            .unwrap_or_else(|| "the Beacon core sent no snapshot".to_string())
    })
}

fn to_json(snapshot: &Snapshot) -> Result<String, String> {
    serde_json::to_string(snapshot).context("cannot encode the snapshot")
}

pub fn render(snapshot: &Snapshot, now: u64) -> String {
    let mut sessions: Vec<&Session> = snapshot.sessions.iter().collect();
    sessions.sort_by_key(|session| (!session.attention, Reverse(session.last_event_ms)));
    let mut text = if sessions.is_empty() {
        "No sessions.\n".to_string()
    } else {
        table(
            &[
                "SESSION", "STATE", "PROJECT", "SURFACE", "ACTIVITY", "TURN", "FLAGS",
            ],
            sessions
                .iter()
                .map(|session| {
                    vec![
                        session.session_id.chars().take(SESSION_ID_CHARS).collect(),
                        state_label(session),
                        project_label(session),
                        surface_label(session.entrypoint.as_deref()).to_string(),
                        session.activity.as_deref().map_or_else(
                            || "-".to_string(),
                            |activity| one_line(&redact(activity), ACTIVITY_CHARS),
                        ),
                        session.turn_started_ms.map_or_else(
                            || "-".to_string(),
                            |started| age(session.turn_ended_ms.unwrap_or(now), started),
                        ),
                        flags(session),
                    ]
                })
                .collect(),
        )
    };
    text.push('\n');
    if snapshot.pending.is_empty() {
        text.push_str("No pending decisions.\n");
    } else {
        text.push_str(&table(
            &["ID", "PROJECT", "TOOL", "INPUT", "AGE"],
            snapshot
                .pending
                .iter()
                .map(|pending| {
                    vec![
                        pending.id.to_string(),
                        snapshot
                            .sessions
                            .iter()
                            .find(|session| session.session_id == pending.session_id)
                            .and_then(|session| session.project.clone())
                            .unwrap_or_else(|| "-".to_string()),
                        pending.tool_name.clone(),
                        input_preview(pending),
                        age(now, pending.created_ms),
                    ]
                })
                .collect(),
        ));
    }
    text
}

pub fn surface_label(entrypoint: Option<&str>) -> &str {
    match entrypoint {
        Some("cli") => "terminal",
        Some("sdk-cli") => "headless",
        Some("claude-vscode") => "VS Code",
        Some("claude-desktop") => "desktop",
        Some(other) => other,
        None => "unknown",
    }
}

fn state_label(session: &Session) -> String {
    format!("{:?}", session.state)
        .chars()
        .enumerate()
        .flat_map(|(index, character)| {
            let space = (index > 0 && character.is_ascii_uppercase()).then_some(' ');
            space.into_iter().chain([character.to_ascii_lowercase()])
        })
        .collect()
}

fn project_label(session: &Session) -> String {
    match (&session.project, &session.branch) {
        (Some(project), Some(branch)) => format!("{project} ({branch})"),
        (Some(project), None) => project.clone(),
        (None, _) => "-".to_string(),
    }
}

fn flags(session: &Session) -> String {
    let overlays = &session.overlays;
    [
        (overlays.subagents > 0).then(|| format!("sub:{}", overlays.subagents)),
        (overlays.background_tasks > 0).then(|| format!("bg:{}", overlays.background_tasks)),
        overlays.compacting.then(|| "compacting".to_string()),
        session.limited.then(|| "limited".to_string()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
}

fn input_preview(pending: &PendingDecision) -> String {
    let field = |name: &str| pending.tool_input.get(name).and_then(Value::as_str);
    let subject = match pending.tool_name.as_str() {
        "Bash" | "PowerShell" => field("command").map(str::to_string),
        "Edit" | "Write" | "Read" => field("file_path").map(str::to_string),
        "WebFetch" => field("url").map(str::to_string),
        tool => tool
            .strip_prefix("mcp__")
            .map(|server_and_tool| server_and_tool.replacen("__", "/", 1)),
    };
    one_line(
        &redact(&subject.unwrap_or_else(|| pending.tool_input.to_string())),
        INPUT_CHARS,
    )
}

fn one_line(text: &str, limit: usize) -> String {
    let flat: Vec<char> = text
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    if flat.len() <= limit {
        return flat.into_iter().collect();
    }
    flat[..limit - 3].iter().chain(&['.'; 3]).collect()
}

fn age(now: u64, since: u64) -> String {
    let seconds = now.saturating_sub(since) / 1000;
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m{:02}s", seconds / 60, seconds % 60),
        3600..86_400 => format!("{}h{:02}m", seconds / 3600, seconds / 60 % 60),
        _ => format!("{}d{:02}h", seconds / 86_400, seconds / 3600 % 24),
    }
}

fn table(headers: &[&str], rows: Vec<Vec<String>>) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let header_row = headers.iter().map(|header| header.to_string()).collect();
    let mut text = String::new();
    for row in std::iter::once(header_row).chain(rows) {
        let line: Vec<String> = row
            .iter()
            .zip(&widths)
            .map(|(cell, width)| format!("{cell:<width$}"))
            .collect();
        text.push_str(line.join("  ").trim_end());
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use beacon_core::{PermissionKind, SessionState};
    use serde_json::json;

    const NOW: u64 = 1_795_100_000_000;

    fn session(id: &str, state: SessionState, last_event_ms: u64) -> Session {
        Session {
            state,
            attention: state.needs_attention(),
            ..Session::new(id, last_event_ms)
        }
    }

    fn pending(id: u64, session_id: &str, tool_name: &str, tool_input: Value) -> PendingDecision {
        PendingDecision {
            id,
            session_id: session_id.to_string(),
            agent_id: None,
            tool_name: tool_name.to_string(),
            tool_input,
            permission_suggestions: Value::Null,
            kind: PermissionKind::Tool,
            created_ms: NOW - 5_000,
            tool_use_id: None,
        }
    }

    #[test]
    fn the_status_table_puts_attention_first_and_redacts_inputs() {
        let mut working = session(
            "11111111-aaaa-bbbb-cccc-000000000001",
            SessionState::Working,
            NOW - 1_000,
        );
        working.project = Some("beacon".to_string());
        working.branch = Some("main".to_string());
        working.entrypoint = Some("cli".to_string());
        working.activity = Some("Running GITHUB_TOKEN=abc123 gh pr list".to_string());
        working.turn_started_ms = Some(NOW - 125_000);
        working.overlays.subagents = 2;
        working.overlays.compacting = true;
        let mut asking = session(
            "22222222-aaaa-bbbb-cccc-000000000002",
            SessionState::NeedsPermission,
            NOW - 60_000,
        );
        asking.project = Some("site".to_string());
        asking.entrypoint = Some("claude-vscode".to_string());
        asking.activity = Some("Running npm test".to_string());
        asking.turn_started_ms = Some(NOW - 3_725_000);
        asking.overlays.background_tasks = 1;
        let mut listed = session(
            "33333333-aaaa-bbbb-cccc-000000000003",
            SessionState::Idle,
            NOW - 2_000,
        );
        listed.limited = true;
        let mut headless = session(
            "44444444-aaaa-bbbb-cccc-000000000004",
            SessionState::Ended,
            NOW - 90_000_000,
        );
        headless.entrypoint = Some("sdk-cli".to_string());
        headless.project = Some("tmp".to_string());
        headless.turn_started_ms = Some(NOW - 90_047_000);
        headless.turn_ended_ms = Some(NOW - 90_000_000);
        let snapshot = Snapshot {
            sessions: vec![working, asking, listed, headless],
            pending: vec![
                pending(
                    3,
                    "22222222-aaaa-bbbb-cccc-000000000002",
                    "Bash",
                    json!({"command": "curl -H \"Authorization: Bearer abc\"\nhttps://x.test", "description": "fetch"}),
                ),
                pending(
                    4,
                    "22222222-aaaa-bbbb-cccc-000000000002",
                    "mcp__github__get_issue",
                    json!({"number": 1}),
                ),
                pending(5, "nobody", "Edit", json!({"file_path": r"C:\x\a.rs"})),
                pending(
                    6,
                    "nobody",
                    "WebFetch",
                    json!({"url": "https://example.test/?api_key=k"}),
                ),
                pending(
                    7,
                    "nobody",
                    "AskUserQuestion",
                    json!({"questions": [{"question": "Which?"}]}),
                ),
            ],
            resolved: Vec::new(),
        };
        assert_eq!(
            render(&snapshot, NOW),
            "\
SESSION   STATE             PROJECT        SURFACE   ACTIVITY                             TURN   FLAGS
22222222  needs permission  site           VS Code   Running npm test                     1h02m  bg:1
11111111  working           beacon (main)  terminal  Running GITHUB_TOKEN=*** gh pr list  2m05s  sub:2 compacting
33333333  idle              -              unknown   -                                    -      limited
44444444  ended             tmp            headless  -                                    47s

ID  PROJECT  TOOL                    INPUT                                        AGE
3   site     Bash                    curl -H \"Authorization: ***\" https://x.test  5s
4   site     mcp__github__get_issue  github/get_issue                             5s
5   -        Edit                    C:\\x\\a.rs                                    5s
6   -        WebFetch                https://example.test/?api_key=***            5s
7   -        AskUserQuestion         {\"questions\":[{\"question\":\"Which?\"}]}        5s
"
        );
    }

    #[test]
    fn an_empty_snapshot_says_so() {
        let snapshot = Snapshot {
            sessions: Vec::new(),
            pending: Vec::new(),
            resolved: Vec::new(),
        };
        assert_eq!(
            render(&snapshot, NOW),
            "No sessions.\n\nNo pending decisions.\n"
        );
    }

    #[test]
    fn long_inputs_are_cut_to_one_line() {
        let long = format!("echo {}", "x".repeat(200));
        let preview = input_preview(&pending(1, "s", "PowerShell", json!({ "command": long })));
        assert_eq!(preview.chars().count(), 80);
        assert!(preview.ends_with("xx..."));
        assert_eq!(age(NOW, NOW - 2 * 86_400_000 - 3_600_000), "2d01h");
        assert_eq!(age(NOW, NOW + 5), "0s");
    }
}

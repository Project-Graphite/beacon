use crate::{Context, connect, platform};
use beacon_cli::surface_label;
use beacon_core::{Reply, Session, Snapshot, now_ms, one_line};
use std::cmp::Reverse;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

const REDRAW_INTERVAL: Duration = Duration::from_secs(1);
const SESSION_ID_CHARS: usize = 8;
const ACTIVITY_CHARS: usize = 60;
const SESSION_NAME_CHARS: usize = 32;
const PROJECT_CHARS: usize = 20;
const EXPIRY_SHOWN_MS: u64 = 15 * 60 * 1000;

pub fn run(home: &Path, watch: bool, json: bool, all: bool) -> Result<ExitCode, String> {
    let mut client = connect(home)?;
    if !watch {
        let reply = client
            .request(&beacon_core::Request::Snapshot)
            .context("the Beacon core did not answer")?;
        let snapshot = shown(snapshot_of(reply)?, all);
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
                latest = Some(shown(
                    snapshot_of(update.context("the Beacon core subscription failed")?)?,
                    all,
                ));
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

fn shown(mut snapshot: Snapshot, all: bool) -> Snapshot {
    if !all {
        snapshot.sessions.retain(|session| !session.dismissed);
    }
    snapshot
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
                "SESSION",
                "STATE",
                "SESSION NAME",
                "PROJECT",
                "SURFACE",
                "ACTIVITY",
                "TURN",
                "FLAGS",
            ],
            sessions
                .iter()
                .map(|session| {
                    vec![
                        session.session_id.chars().take(SESSION_ID_CHARS).collect(),
                        state_label(session),
                        one_line(&session.display_title, SESSION_NAME_CHARS),
                        one_line(&project_label(session), PROJECT_CHARS),
                        surface_label(session.entrypoint.as_deref()).to_string(),
                        session.activity.as_deref().map_or_else(
                            || "-".to_string(),
                            |activity| one_line(activity, ACTIVITY_CHARS),
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
                        pending.preview.clone(),
                        if pending.expires_ms.saturating_sub(now) < EXPIRY_SHOWN_MS {
                            format!(
                                "{}, expires in {}",
                                age(now, pending.created_ms),
                                age(pending.expires_ms, now)
                            )
                        } else {
                            age(now, pending.created_ms)
                        },
                    ]
                })
                .collect(),
        ));
    }
    text
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

pub fn project_label(session: &Session) -> String {
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
        (session.queued_prompts() > 0).then(|| format!("q:{}", session.queued_prompts())),
        session.dismissed.then(|| "dismissed".to_string()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
}

pub fn age(now: u64, since: u64) -> String {
    let seconds = now.saturating_sub(since) / 1000;
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m{:02}s", seconds / 60, seconds % 60),
        3600..86_400 => format!("{}h{:02}m", seconds / 3600, seconds / 60 % 60),
        _ => format!("{}d{:02}h", seconds / 86_400, seconds / 3600 % 24),
    }
}

pub fn table(headers: &[&str], rows: Vec<Vec<String>>) -> String {
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
    use beacon_core::prompts::{Prompt, PromptStatus};
    use beacon_core::{PendingDecision, PermissionKind, SessionState, input_preview};
    use serde_json::{Value, json};

    const NOW: u64 = 1_795_100_000_000;

    fn session(id: &str, state: SessionState, last_event_ms: u64) -> Session {
        Session {
            state,
            attention: state.needs_attention(),
            display_title: id.chars().take(8).collect(),
            ..Session::new(id, last_event_ms)
        }
    }

    fn pending(id: u64, session_id: &str, tool_name: &str, tool_input: Value) -> PendingDecision {
        PendingDecision {
            id,
            session_id: session_id.to_string(),
            agent_id: None,
            tool_name: tool_name.to_string(),
            preview: input_preview(tool_name, &tool_input),
            tool_input,
            permission_suggestions: Value::Null,
            kind: PermissionKind::Tool,
            created_ms: NOW - 5_000,
            tool_use_id: None,
            cwd: None,
            permission_mode: None,
            agent_type: None,
            expires_ms: NOW - 5_000 + 3_600_000,
            plan: None,
        }
    }

    #[test]
    fn the_status_table_puts_attention_first_and_redacts_inputs() {
        let mut working = session(
            "11111111-aaaa-bbbb-cccc-000000000001",
            SessionState::Working,
            NOW - 1_000,
        );
        working.display_title = "Fix the login flow".to_string();
        working.project = Some("beacon".to_string());
        working.branch = Some("main".to_string());
        working.entrypoint = Some("cli".to_string());
        working.activity = Some("Running GITHUB_TOKEN=*** gh pr list".to_string());
        working.turn_started_ms = Some(NOW - 125_000);
        working.overlays.subagents = 2;
        working.overlays.compacting = true;
        let mut asking = session(
            "22222222-aaaa-bbbb-cccc-000000000002",
            SessionState::NeedsPermission,
            NOW - 60_000,
        );
        asking.display_title = "Refactor the authentication middleware and its tests".to_string();
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
            ended_count: 0,
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
            snoozed_until_ms: None,
            account_usage: None,
            config: beacon_core::Config::default().live(),
        };
        assert_eq!(
            render(&snapshot, NOW),
            "\
SESSION   STATE             SESSION NAME                      PROJECT        SURFACE   ACTIVITY                             TURN   FLAGS
22222222  needs permission  Refactor the authentication m...  site           VS Code   Running npm test                     1h02m  bg:1
11111111  working           Fix the login flow                beacon (main)  terminal  Running GITHUB_TOKEN=*** gh pr list  2m05s  sub:2 compacting
33333333  idle              33333333                          -              unknown   -                                    -      limited
44444444  ended             44444444                          tmp            headless  -                                    47s

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
    fn requests_show_their_time_left_under_fifteen_minutes() {
        let mut late = pending(8, "nobody", "Bash", json!({"command": "ls"}));
        late.created_ms = NOW - 2_880_000;
        late.expires_ms = NOW + 720_000;
        let mut edge = pending(9, "nobody", "Bash", json!({"command": "pwd"}));
        edge.expires_ms = NOW + 900_000;
        let snapshot = Snapshot {
            ended_count: 0,
            sessions: Vec::new(),
            pending: vec![late, edge],
            resolved: Vec::new(),
            snoozed_until_ms: None,
            account_usage: None,
            config: beacon_core::Config::default().live(),
        };
        assert_eq!(
            render(&snapshot, NOW),
            "\
No sessions.

ID  PROJECT  TOOL  INPUT  AGE
8   -        Bash  ls     48m00s, expires in 12m00s
9   -        Bash  pwd    5s
"
        );
    }

    #[test]
    fn an_empty_snapshot_says_so() {
        let snapshot = Snapshot {
            ended_count: 0,
            sessions: Vec::new(),
            pending: Vec::new(),
            resolved: Vec::new(),
            snoozed_until_ms: None,
            account_usage: None,
            config: beacon_core::Config::default().live(),
        };
        assert_eq!(
            render(&snapshot, NOW),
            "No sessions.\n\nNo pending decisions.\n"
        );
    }

    #[test]
    fn queued_prompts_show_as_a_flag() {
        let mut working = session("55555555", SessionState::Working, NOW);
        let prompt = |id, status| Prompt {
            id,
            preview: "go on".to_string(),
            created_ms: NOW,
            status,
            reason: None,
            text: None,
        };
        working.prompts = vec![
            prompt(1, PromptStatus::Delivered),
            prompt(2, PromptStatus::Queued),
            prompt(3, PromptStatus::Queued),
        ];
        assert_eq!(flags(&working), "q:2");
        working.prompts.truncate(1);
        assert_eq!(flags(&working), "");
    }

    #[test]
    fn dismissed_sessions_are_shown_only_with_all() {
        let mut crossed = session("66666666", SessionState::Idle, NOW - 1_000);
        crossed.dismissed = true;
        let snapshot = Snapshot {
            ended_count: 0,
            sessions: vec![crossed, session("77777777", SessionState::Working, NOW)],
            pending: Vec::new(),
            resolved: Vec::new(),
            snoozed_until_ms: None,
            account_usage: None,
            config: beacon_core::Config::default().live(),
        };
        assert_eq!(
            render(&shown(snapshot.clone(), false), NOW),
            "\
SESSION   STATE    SESSION NAME  PROJECT  SURFACE  ACTIVITY  TURN  FLAGS
77777777  working  77777777      -        unknown  -         -

No pending decisions.
"
        );
        assert_eq!(
            render(&shown(snapshot, true), NOW),
            "\
SESSION   STATE    SESSION NAME  PROJECT  SURFACE  ACTIVITY  TURN  FLAGS
77777777  working  77777777      -        unknown  -         -
66666666  idle     66666666      -        unknown  -         -     dismissed

No pending decisions.
"
        );
    }

    #[test]
    fn ages_switch_units_and_never_go_negative() {
        assert_eq!(age(NOW, NOW - 2 * 86_400_000 - 3_600_000), "2d01h");
        assert_eq!(age(NOW, NOW + 5), "0s");
    }
}

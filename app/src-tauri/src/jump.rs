use crate::feed::{self, Feed};
use crate::shell::{self, Action};
use beacon_core::focus::{self, JumpOutcome};
use beacon_core::prompts::{PromptPath, check_text, prompt_path};
use beacon_core::{Request, Session, SessionState, Snapshot, clipboard};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use tauri::{AppHandle, State};

#[derive(Serialize)]
pub struct WindowPrompt {
    path: PromptPath,
    outcome: JumpOutcome,
}

async fn with_session<T: Send + 'static>(
    home: PathBuf,
    session_id: String,
    act: impl FnOnce(Session) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        act(feed::request(&home, &Request::Snapshot)?
            .snapshot
            .and_then(|snapshot| {
                snapshot
                    .sessions
                    .into_iter()
                    .find(|session| session.session_id == session_id)
            })
            .ok_or_else(|| format!("no session {session_id}"))?)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn jump(feed: State<'_, Arc<Feed>>, session_id: String) -> Result<JumpOutcome, String> {
    with_session(feed.home.clone(), session_id, |session| {
        focus::jump(&session).map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
pub async fn send_prompt_to_window(
    feed: State<'_, Arc<Feed>>,
    session_id: String,
    text: String,
) -> Result<WindowPrompt, String> {
    with_session(feed.home.clone(), session_id, move |session| {
        check_text(&text)?;
        let (path, outcome) = match prompt_path(&session) {
            Some(path @ PromptPath::Prefill) => (path, focus::jump_with_prompt(&session, &text)),
            Some(path @ PromptPath::CopyAndJump) => (
                path,
                clipboard::set_text(&text).and_then(|()| focus::jump(&session)),
            ),
            Some(PromptPath::AtTurnEnd | PromptPath::Wake) | None => {
                return Err(format!(
                    "session {} takes no prompt through its window now",
                    session.session_id
                ));
            }
        };
        Ok(WindowPrompt {
            path,
            outcome: outcome.map_err(|error| error.to_string())?,
        })
    })
    .await
}

fn focused_session(sessions: &[Session]) -> Option<String> {
    if sessions.is_empty() {
        return None;
    }
    focus::foreground_session(sessions).unwrap_or_else(|error| {
        log_error!("cannot tell which session has focus: {error}");
        None
    })
}

pub fn resolve_focus(
    app: &AppHandle,
    snapshot: Option<&Snapshot>,
    report: impl FnOnce(Option<String>) -> Action + Send + 'static,
) {
    let sessions = snapshot.map_or_else(Vec::new, |snapshot| snapshot.sessions.clone());
    let app = app.clone();
    thread::spawn(move || shell::post(&app, report(focused_session(&sessions))));
}

pub fn has_done(snapshot: Option<&Snapshot>) -> bool {
    snapshot.is_some_and(|snapshot| {
        snapshot
            .sessions
            .iter()
            .any(|session| session.state == SessionState::Done)
    })
}

pub fn acknowledge_focused(home: &Path, snapshot: Option<&Snapshot>, focused: Option<&str>) {
    if let Some(session) = snapshot.and_then(|snapshot| {
        snapshot.sessions.iter().find(|session| {
            session.state == SessionState::Done && focused == Some(session.session_id.as_str())
        })
    }) {
        feed::send(
            home.to_path_buf(),
            Request::Acknowledge {
                session_id: session.session_id.clone(),
            },
        );
    }
}

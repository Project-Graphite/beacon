use crate::feed::{self, Feed};
use beacon_core::focus::{self, JumpOutcome};
use beacon_core::{Request, SessionState, Snapshot};
use std::path::Path;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn jump(feed: State<'_, Arc<Feed>>, session_id: String) -> Result<JumpOutcome, String> {
    let home = feed.home.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let session = feed::request(&home, &Request::Snapshot)?
            .snapshot
            .and_then(|snapshot| {
                snapshot
                    .sessions
                    .into_iter()
                    .find(|session| session.session_id == session_id)
            })
            .ok_or_else(|| format!("no session {session_id}"))?;
        focus::jump(&session).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub fn focused_session(snapshot: Option<&Snapshot>) -> Option<String> {
    focus::foreground_session(&snapshot?.sessions).unwrap_or_else(|error| {
        eprintln!("cannot tell which session has focus: {error}");
        None
    })
}

pub fn acknowledge_focused(home: &Path, snapshot: Option<&Snapshot>) {
    let Some(snapshot) = snapshot.filter(|snapshot| {
        snapshot
            .sessions
            .iter()
            .any(|session| session.state == SessionState::Done)
    }) else {
        return;
    };
    let focused = focused_session(Some(snapshot));
    if let Some(session) = snapshot.sessions.iter().find(|session| {
        session.state == SessionState::Done && focused.as_ref() == Some(&session.session_id)
    }) {
        feed::send(
            home.to_path_buf(),
            Request::Acknowledge {
                session_id: session.session_id.clone(),
            },
        );
    }
}

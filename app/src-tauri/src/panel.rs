use crate::feed::{self, Feed};
use crate::geometry::Rect;
use crate::shell::{self, Action};
use beacon_core::{Decision, Reply, Request, SessionDetail, Snapshot};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const PANEL: &str = "panel";

pub fn create(
    app: &AppHandle,
    (rect, scale): (Rect, f64),
    focused: bool,
) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, PANEL, WebviewUrl::default())
        .title("Beacon")
        .position(f64::from(rect.left) / scale, f64::from(rect.top) / scale)
        .inner_size(
            f64::from(rect.width()) / scale,
            f64::from(rect.height()) / scale,
        )
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(focused)
        .build()
}

async fn ask(home: PathBuf, request: Request) -> Result<Reply, String> {
    tauri::async_runtime::spawn_blocking(move || feed::request(&home, &request))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn get_snapshot(feed: State<'_, Arc<Feed>>) -> Result<Snapshot, String> {
    ask(feed.home.clone(), Request::Snapshot)
        .await?
        .snapshot
        .ok_or_else(|| "the Beacon core sent no snapshot".to_string())
}

#[tauri::command]
pub async fn get_detail(
    feed: State<'_, Arc<Feed>>,
    session_id: String,
) -> Result<SessionDetail, String> {
    ask(feed.home.clone(), Request::Detail { session_id })
        .await?
        .detail
        .ok_or_else(|| "the Beacon core sent no detail".to_string())
}

#[tauri::command]
pub async fn acknowledge(feed: State<'_, Arc<Feed>>, session_id: String) -> Result<(), String> {
    ask(feed.home.clone(), Request::Acknowledge { session_id })
        .await
        .map(drop)
}

#[tauri::command]
pub async fn decide(feed: State<'_, Arc<Feed>>, id: u64, decision: Decision) -> Result<(), String> {
    ask(feed.home.clone(), Request::Decide { id, decision })
        .await
        .map(drop)
}

#[tauri::command]
pub async fn queue_prompt(
    feed: State<'_, Arc<Feed>>,
    session_id: String,
    text: String,
) -> Result<(), String> {
    ask(feed.home.clone(), Request::QueuePrompt { session_id, text })
        .await
        .map(drop)
}

#[tauri::command]
pub async fn cancel_prompt(feed: State<'_, Arc<Feed>>, id: u64) -> Result<(), String> {
    ask(feed.home.clone(), Request::CancelPrompt { id })
        .await
        .map(drop)
}

#[tauri::command]
pub async fn snooze(
    feed: State<'_, Arc<Feed>>,
    session_id: Option<String>,
    minutes: u64,
) -> Result<(), String> {
    ask(
        feed.home.clone(),
        Request::Snooze {
            session_id,
            minutes,
        },
    )
    .await
    .map(drop)
}

#[tauri::command]
pub async fn unsnooze(
    feed: State<'_, Arc<Feed>>,
    session_id: Option<String>,
) -> Result<(), String> {
    ask(feed.home.clone(), Request::Unsnooze { session_id })
        .await
        .map(drop)
}

#[tauri::command]
pub async fn refresh_usage(feed: State<'_, Arc<Feed>>) -> Result<(), String> {
    ask(feed.home.clone(), Request::RefreshUsage)
        .await
        .map(drop)
}

#[tauri::command]
pub async fn set_config(
    feed: State<'_, Arc<Feed>>,
    stall_minutes: Option<u64>,
    answer_unattended: Option<bool>,
) -> Result<(), String> {
    ask(
        feed.home.clone(),
        Request::SetConfig {
            stall_minutes,
            answer_unattended,
        },
    )
    .await
    .map(drop)
}

#[tauri::command]
pub async fn close_panel(app: AppHandle) {
    shell::post(&app, Action::ClosePanel);
}

#[tauri::command]
pub fn set_panel_height(app: AppHandle, height: f64) {
    shell::post(&app, Action::PanelHeight(height));
}

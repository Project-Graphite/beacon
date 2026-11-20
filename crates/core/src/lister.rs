use crate::platform;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::process::Stdio;
use std::time::Duration;

const LIST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListedSession {
    pub session_id: String,
    pub pid: Option<u32>,
    pub cwd: Option<String>,
    pub kind: Option<String>,
    pub started_at: Option<u64>,
    pub name: Option<String>,
    pub status: Option<String>,
    pub waiting_for: Option<Value>,
}

pub type ListFuture<'a> = Pin<Box<dyn Future<Output = io::Result<Vec<ListedSession>>> + 'a>>;

pub trait SessionLister: Send {
    fn list(&self) -> ListFuture<'_>;
}

pub struct ClaudeAgentsLister;

impl SessionLister for ClaudeAgentsLister {
    fn list(&self) -> ListFuture<'_> {
        Box::pin(async {
            Ok(serde_json::from_slice(
                &run_claude(&["agents", "--json"], LIST_TIMEOUT).await?,
            )?)
        })
    }
}

pub async fn run_claude(args: &[&str], limit: Duration) -> io::Result<Vec<u8>> {
    let mut command = tokio::process::Command::new("claude");
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    platform::hide_console_window(&mut command);
    let output = tokio::time::timeout(limit, command.output())
        .await
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                format!("claude {} timed out", args[0]),
            )
        })??;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "claude {} exited with {}",
            args[0], output.status
        )));
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::ListedSession;

    #[test]
    fn agents_output_parses_and_ignores_unknown_fields() {
        let listed: Vec<ListedSession> = serde_json::from_str(
            r#"[{"pid":25968,"cwd":"c:\\work\\example","kind":"interactive","startedAt":1795089974147,
                "sessionId":"11111111","name":"example-1","status":"idle","extra":1},
               {"sessionId":"x","status":"waiting","waitingFor":{"tool":"Bash"}}]"#,
        )
        .unwrap();
        assert_eq!(listed[0].pid, Some(25968));
        assert_eq!(listed[0].started_at, Some(1_795_089_974_147));
        assert_eq!(listed[1].status.as_deref(), Some("waiting"));
        assert!(listed[1].waiting_for.is_some());
    }
}

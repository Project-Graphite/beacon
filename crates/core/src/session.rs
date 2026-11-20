use crate::focus::Host;
use crate::prompts::Prompt;
use crate::state::{AgentStatus, Input, Overlays, SessionState, overlays_after, transition};
use crate::usage::SessionUsage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

const COMMAND_CHARS: usize = 80;
const PREVIEW_CHARS: usize = 400;
const IMAGE_SURFACES: [(&str, &str, Option<&str>); 7] = [
    (r"\.vscode\extensions\", "claude-vscode", Some("VS Code")),
    (
        r"\.vscode-insiders\extensions\",
        "claude-vscode",
        Some("VS Code Insiders"),
    ),
    (r"\.cursor\extensions\", "claude-vscode", Some("Cursor")),
    (r"\.windsurf\extensions\", "claude-vscode", Some("Windsurf")),
    (
        r"\.antigravity\extensions\",
        "claude-vscode",
        Some("Antigravity"),
    ),
    (r"\anthropicclaude\", "claude-desktop", None),
    (r"\appdata\roaming\claude\", "claude-desktop", None),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub state: SessionState,
    pub attention: bool,
    pub overlays: Overlays,
    pub cwd: Option<String>,
    pub project: Option<String>,
    pub branch: Option<String>,
    pub transcript_path: Option<String>,
    pub entrypoint: Option<String>,
    pub claude_pid: Option<u32>,
    pub attended: Option<bool>,
    pub permission_mode: Option<String>,
    pub model: Option<String>,
    pub title: Option<String>,
    pub listed_name: Option<String>,
    pub activity: Option<String>,
    pub turn_started_ms: Option<u64>,
    #[serde(default)]
    pub turn_ended_ms: Option<u64>,
    pub last_event_ms: u64,
    pub event_count: u64,
    pub last_message_preview: Option<String>,
    pub last_error: Option<String>,
    pub tasks_created: u32,
    pub tasks_completed: u32,
    pub limited: bool,
    #[serde(default)]
    pub snoozed_until_ms: Option<u64>,
    #[serde(default)]
    pub alerting: bool,
    #[serde(default)]
    pub editor: Option<String>,
    #[serde(default)]
    pub usage: Option<SessionUsage>,
    #[serde(default)]
    pub mcp_request: Option<McpRequest>,
    #[serde(default)]
    pub host: Option<Host>,
    #[serde(default)]
    pub prompts: Vec<Prompt>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpRequest {
    pub server: String,
    pub message: String,
}

impl Session {
    pub fn new(session_id: &str, now_ms: u64) -> Session {
        Session {
            session_id: session_id.to_string(),
            state: SessionState::Starting,
            attention: false,
            overlays: Overlays::default(),
            cwd: None,
            project: None,
            branch: None,
            transcript_path: None,
            entrypoint: None,
            claude_pid: None,
            attended: None,
            permission_mode: None,
            model: None,
            title: None,
            listed_name: None,
            activity: None,
            turn_started_ms: None,
            turn_ended_ms: None,
            last_event_ms: now_ms,
            event_count: 0,
            last_message_preview: None,
            last_error: None,
            tasks_created: 0,
            tasks_completed: 0,
            limited: false,
            snoozed_until_ms: None,
            alerting: false,
            editor: None,
            usage: None,
            mcp_request: None,
            host: None,
            prompts: Vec::new(),
        }
    }

    pub fn apply(&mut self, input: Input, now_ms: u64) {
        let before = self.state;
        self.state = transition(self.state, input);
        self.overlays = overlays_after(self.overlays, input);
        self.fail_queued_prompts();
        if self.state != SessionState::NeedsMcpInput {
            self.mcp_request = None;
        }
        if self.attention && !self.state.needs_attention() {
            self.snoozed_until_ms = None;
        }
        self.attention = self.state.needs_attention();
        if input == Input::PromptSubmitted {
            self.turn_ended_ms = None;
        } else if self.turn_started_ms.is_some()
            && self.turn_ended_ms.is_none()
            && ends_turn(input, before, self.state)
        {
            self.turn_ended_ms = Some(now_ms);
        }
    }

    pub fn set_location(&mut self, cwd: &str, project_dir: Option<&str>) {
        self.project = last_component(project_dir.unwrap_or(cwd));
        self.branch = git_branch(Path::new(cwd));
        self.cwd = Some(cwd.to_string());
    }

    pub fn set_preview(&mut self, message: &str) {
        self.last_message_preview = Some(message.chars().take(PREVIEW_CHARS).collect());
    }
}

pub fn surface_from_image(image_path: &str) -> Option<(&'static str, Option<&'static str>)> {
    let lowered = image_path.to_lowercase();
    IMAGE_SURFACES
        .iter()
        .find(|(marker, ..)| lowered.contains(marker))
        .map(|&(_, entrypoint, editor)| (entrypoint, editor))
}

fn ends_turn(input: Input, before: SessionState, after: SessionState) -> bool {
    match input {
        Input::Stopped { .. }
        | Input::StopFailed
        | Input::Interrupted
        | Input::SessionEnded
        | Input::ProcessGone => true,
        Input::Reconciled(AgentStatus::Idle) => {
            after == SessionState::Idle && before != SessionState::Idle
        }
        _ => false,
    }
}

fn last_component(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

pub fn git_branch(cwd: &Path) -> Option<String> {
    for directory in cwd.ancestors() {
        let dot_git = directory.join(".git");
        let git_dir = if dot_git.is_dir() {
            dot_git
        } else if dot_git.is_file() {
            let pointer = std::fs::read_to_string(&dot_git).ok()?;
            directory.join(pointer.trim().strip_prefix("gitdir:")?.trim())
        } else {
            continue;
        };
        let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
        let head = head.trim();
        return Some(match head.strip_prefix("ref:") {
            Some(reference) => {
                let reference = reference.trim();
                reference
                    .strip_prefix("refs/heads/")
                    .unwrap_or(reference)
                    .to_string()
            }
            None => head.chars().take(7).collect(),
        });
    }
    None
}

pub fn activity_line(tool_name: &str, tool_input: Option<&Value>, cwd: Option<&str>) -> String {
    let field = |name: &str| {
        tool_input
            .and_then(|input| input.get(name))
            .and_then(Value::as_str)
    };
    let shown_path = |name: &str| field(name).map(|path| relative_to(path, cwd));
    match tool_name {
        "Edit" | "MultiEdit" | "Write" => describe("Editing", shown_path("file_path"), tool_name),
        "NotebookEdit" => describe("Editing", shown_path("notebook_path"), tool_name),
        "Read" => describe("Reading", shown_path("file_path"), tool_name),
        "Bash" | "PowerShell" => describe(
            "Running",
            field("command").map(|command| command.chars().take(COMMAND_CHARS).collect()),
            tool_name,
        ),
        "Grep" | "Glob" | "WebSearch" | "ToolSearch" => "Searching".to_string(),
        other => other.to_string(),
    }
}

fn describe(verb: &str, subject: Option<String>, tool_name: &str) -> String {
    match subject {
        Some(subject) => format!("{verb} {subject}"),
        None => tool_name.to_string(),
    }
}

fn relative_to(path: &str, cwd: Option<&str>) -> String {
    cwd.and_then(|cwd| Path::new(path).strip_prefix(cwd).ok())
        .map_or_else(
            || path.to_string(),
            |relative| relative.to_string_lossy().into_owned(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn activity_lines_describe_the_tool() {
        let cwd = Some(r"C:\work");
        assert_eq!(
            activity_line(
                "Edit",
                Some(&json!({"file_path": r"C:\work\src\api.ts"})),
                cwd
            ),
            r"Editing src\api.ts"
        );
        assert_eq!(
            activity_line("Read", Some(&json!({"file_path": r"D:\README.md"})), cwd),
            r"Reading D:\README.md"
        );
        assert_eq!(
            activity_line("Bash", Some(&json!({"command": "npm test"})), cwd),
            "Running npm test"
        );
        assert_eq!(
            activity_line("Grep", Some(&json!({"pattern": "x"})), cwd),
            "Searching"
        );
        assert_eq!(
            activity_line("mcp__github__get_issue", None, cwd),
            "mcp__github__get_issue"
        );
        assert_eq!(activity_line("Bash", None, cwd), "Bash");
    }

    #[test]
    fn commands_are_cut_to_eighty_characters() {
        let command = "é".repeat(1_000_000);
        let line = activity_line("Bash", Some(&json!({ "command": command })), None);
        assert_eq!(line, format!("Running {}", "é".repeat(80)));
    }

    #[test]
    fn previews_keep_four_hundred_characters() {
        let mut session = Session::new("s", 0);
        session.set_preview(&"ж".repeat(5000));
        assert_eq!(session.last_message_preview.unwrap().chars().count(), 400);
    }

    #[test]
    fn branch_is_read_from_git_head_walking_up() {
        let root = std::env::temp_dir().join(format!("beacon-branch-{}", std::process::id()));
        let nested = root.join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(
            root.join(".git").join("HEAD"),
            "ref: refs/heads/feature/x\n",
        )
        .unwrap();
        assert_eq!(git_branch(&nested).as_deref(), Some("feature/x"));
        std::fs::write(root.join(".git").join("HEAD"), "0123456789abcdef\n").unwrap();
        assert_eq!(git_branch(&nested).as_deref(), Some("0123456"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn image_paths_map_to_surfaces() {
        let cases = [
            (
                r"C:\Users\x\.vscode\extensions\anthropic.claude-code-2.1.0-win32-x64\resources\native-binary\claude.exe",
                Some(("claude-vscode", Some("VS Code"))),
            ),
            (
                r"C:\Users\x\.VSCODE-INSIDERS\Extensions\anthropic.claude-code\claude.exe",
                Some(("claude-vscode", Some("VS Code Insiders"))),
            ),
            (
                r"C:\Users\x\.cursor\extensions\a\claude.exe",
                Some(("claude-vscode", Some("Cursor"))),
            ),
            (
                r"C:\Users\x\.windsurf\extensions\a\claude.exe",
                Some(("claude-vscode", Some("Windsurf"))),
            ),
            (
                r"C:\Users\x\.antigravity\extensions\a\claude.exe",
                Some(("claude-vscode", Some("Antigravity"))),
            ),
            (
                r"C:\Users\x\AppData\Local\AnthropicClaude\app-1.0.0\resources\claude.exe",
                Some(("claude-desktop", None)),
            ),
            (
                r"C:\Users\x\AppData\Roaming\Claude\claude-code\2.1.0\claude.exe",
                Some(("claude-desktop", None)),
            ),
            (r"C:\Users\x\.local\bin\claude.exe", None),
            (r"C:\Users\x\.vscode\claude.exe", None),
        ];
        for (path, expected) in cases {
            assert_eq!(surface_from_image(path), expected, "{path}");
        }
    }

    #[test]
    fn project_prefers_the_project_dir() {
        let mut session = Session::new("s", 0);
        session.set_location(r"C:\nowhere\sub", Some(r"C:\nowhere\project"));
        assert_eq!(session.project.as_deref(), Some("project"));
        session.set_location(r"C:\nowhere\sub", None);
        assert_eq!(session.project.as_deref(), Some("sub"));
    }
}

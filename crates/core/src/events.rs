use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::Value;
use std::collections::BTreeMap;

pub const KNOWN_EVENTS: [&str; 33] = [
    "SessionStart",
    "Setup",
    "UserPromptSubmit",
    "UserPromptExpansion",
    "PreToolUse",
    "PermissionRequest",
    "PermissionDenied",
    "PostToolUse",
    "PostToolUseFailure",
    "PostToolBatch",
    "Notification",
    "MessageDisplay",
    "SubagentStart",
    "SubagentStop",
    "TaskCreated",
    "TaskCompleted",
    "Stop",
    "StopFailure",
    "TeammateIdle",
    "InstructionsLoaded",
    "ConfigChange",
    "CwdChanged",
    "DirectoryAdded",
    "FileChanged",
    "WorktreeCreate",
    "WorktreeRemove",
    "PreCompact",
    "PostCompact",
    "PreModelSwitch",
    "PostModelSwitch",
    "Elicitation",
    "ElicitationResult",
    "SessionEnd",
];

#[derive(Debug, Deserialize)]
pub struct Meta {
    pub event: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl Meta {
    pub fn env(&self, name: &str) -> Option<&str> {
        self.env.get(name).map(String::as_str)
    }

    pub fn unattended(&self) -> bool {
        self.env("CLAUDE_CODE_SESSION_ATTENDED") == Some("0")
    }
}

#[derive(Debug, Deserialize)]
pub struct Common {
    pub session_id: String,
    pub hook_event_name: String,
    pub transcript_path: Option<String>,
    pub cwd: Option<String>,
    pub permission_mode: Option<String>,
    pub agent_id: Option<String>,
    pub agent_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolUse {
    pub tool_name: String,
    pub tool_input: Option<Value>,
    pub tool_use_id: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ToolCall {
    pub tool_use_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "hook_event_name")]
pub enum Event {
    SessionStart {
        source: Option<String>,
        model: Option<String>,
        session_title: Option<String>,
    },
    UserPromptSubmit {
        session_title: Option<String>,
    },
    PreToolUse(ToolUse),
    PostToolUse(ToolUse),
    PostToolUseFailure(ToolUse),
    PostToolBatch {
        #[serde(default)]
        tool_calls: Vec<ToolCall>,
    },
    PermissionRequest {
        tool_name: String,
        tool_input: Option<Value>,
        permission_suggestions: Option<Value>,
    },
    SubagentStart {
        agent_type: Option<String>,
    },
    SubagentStop,
    TaskCreated {
        task_id: Option<String>,
        task_subject: Option<String>,
    },
    TaskCompleted {
        task_id: Option<String>,
        task_subject: Option<String>,
    },
    Stop {
        last_assistant_message: Option<String>,
        background_tasks: Option<Vec<IgnoredAny>>,
    },
    StopFailure {
        error: Option<String>,
    },
    SessionEnd {
        reason: Option<String>,
    },
    Notification {
        notification_type: Option<String>,
    },
    Elicitation {
        mcp_server_name: Option<String>,
        message: Option<String>,
    },
    ElicitationResult,
    CwdChanged {
        new_cwd: Option<String>,
    },
    PostModelSwitch {
        to_model: Option<String>,
    },
    PreCompact,
    PostCompact,
    #[serde(other)]
    Other,
}

pub struct Parsed {
    pub common: Common,
    pub event: Event,
}

pub struct Rejected {
    pub reason: String,
    pub detail: Option<serde_json::Error>,
}

fn rejected(reason: String) -> impl FnOnce(serde_json::Error) -> Rejected {
    move |error| Rejected {
        reason,
        detail: Some(error),
    }
}

pub fn parse(payload: &[u8]) -> Result<Parsed, Rejected> {
    let value: Value =
        serde_json::from_slice(payload).map_err(rejected("payload is not JSON".to_string()))?;
    let common = Common::deserialize(&value)
        .map_err(rejected("payload lacks the common fields".to_string()))?;
    if !KNOWN_EVENTS.contains(&common.hook_event_name.as_str()) {
        return Err(Rejected {
            reason: format!("unknown event {}", common.hook_event_name),
            detail: None,
        });
    }
    let event = Event::deserialize(&value).map_err(rejected(format!(
        "{} payload is malformed",
        common.hook_event_name
    )))?;
    Ok(Parsed { common, event })
}

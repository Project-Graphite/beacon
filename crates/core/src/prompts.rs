use crate::preview::one_line;
use crate::redact::redact;
use crate::session::Session;
use crate::state::SessionState;
use serde::{Deserialize, Serialize};
use serde_json::json;

pub const PROMPT_MAX_CHARS: usize = 16_000;
const PREVIEW_CHARS: usize = 120;
const PROMPTS_KEPT: usize = 5;
const VSCODE_ENTRYPOINT: &str = "claude-vscode";
pub const ENDED_REASON: &str = "The session ended before the turn finished.";
pub const INTERRUPTED_REASON: &str = "The turn was interrupted before it finished.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptStatus {
    Queued,
    Delivered,
    Acknowledged,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prompt {
    pub id: u64,
    pub preview: String,
    pub created_ms: u64,
    pub status: PromptStatus,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptPath {
    AtTurnEnd,
    Prefill,
    CopyAndJump,
}

pub fn prompt_path(session: &Session) -> Option<PromptPath> {
    match session.state {
        SessionState::Ended => None,
        state if state.turn_running() => Some(PromptPath::AtTurnEnd),
        _ if session.entrypoint.as_deref() == Some(VSCODE_ENTRYPOINT) => Some(PromptPath::Prefill),
        _ => Some(PromptPath::CopyAndJump),
    }
}

pub fn check_text(text: &str) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("the prompt is empty".to_string());
    }
    let length = text.chars().count();
    if length > PROMPT_MAX_CHARS {
        return Err(format!(
            "the prompt has {length} characters; the limit is {PROMPT_MAX_CHARS}"
        ));
    }
    Ok(())
}

pub fn stop_output(text: &str) -> Vec<u8> {
    json!({"hookSpecificOutput": {
        "hookEventName": "Stop",
        "additionalContext": format!("The user sent this follow-up from Beacon: {text}"),
    }})
    .to_string()
    .into_bytes()
}

impl Session {
    pub fn queue_prompt(&mut self, id: u64, text: &str, now_ms: u64) -> Result<(), String> {
        let session_id = &self.session_id;
        match prompt_path(self) {
            Some(PromptPath::AtTurnEnd) => {}
            None => {
                return Err(format!(
                    "session {session_id} has ended and takes no prompt"
                ));
            }
            Some(PromptPath::Prefill) => {
                return Err(format!(
                    "session {session_id} is not running a turn, so there is no turn end to wait for; send the prompt from the Beacon panel, which opens the session in VS Code with the text in its prompt box"
                ));
            }
            Some(PromptPath::CopyAndJump) => {
                return Err(format!(
                    "session {session_id} is not running a turn, so there is no turn end to wait for; run `beacon jump {session_id}` and type the prompt there, or send it from the Beacon panel, which copies it and jumps to the window"
                ));
            }
        }
        check_text(text)?;
        self.prompts.push(Prompt {
            id,
            preview: one_line(&redact(text), PREVIEW_CHARS),
            created_ms: now_ms,
            status: PromptStatus::Queued,
            reason: None,
            text: Some(text.to_string()),
        });
        while self.prompts.len() > PROMPTS_KEPT {
            let Some(oldest_finished) = self
                .prompts
                .iter()
                .position(|prompt| prompt.status != PromptStatus::Queued)
            else {
                break;
            };
            self.prompts.remove(oldest_finished);
        }
        Ok(())
    }

    pub fn cancel_prompt(&mut self, id: u64) -> Option<Result<(), String>> {
        let index = self.prompts.iter().position(|prompt| prompt.id == id)?;
        let status = self.prompts[index].status;
        Some(if status == PromptStatus::Queued {
            self.prompts.remove(index);
            Ok(())
        } else {
            Err(format!(
                "prompt {id} is {} and can no longer be cancelled",
                format!("{status:?}").to_lowercase()
            ))
        })
    }

    pub fn deliver_next_prompt(&mut self) -> Option<String> {
        let prompt = self
            .prompts
            .iter_mut()
            .find(|prompt| prompt.status == PromptStatus::Queued)?;
        let text = prompt.text.take()?;
        prompt.status = PromptStatus::Delivered;
        Some(text)
    }

    pub fn acknowledge_prompts(&mut self) {
        for prompt in &mut self.prompts {
            if prompt.status == PromptStatus::Delivered {
                prompt.status = PromptStatus::Acknowledged;
            }
        }
    }

    pub fn fail_queued_prompts(&mut self) {
        if self.state.turn_running() {
            return;
        }
        let reason = if self.state == SessionState::Ended {
            ENDED_REASON
        } else {
            INTERRUPTED_REASON
        };
        for prompt in &mut self.prompts {
            if prompt.status == PromptStatus::Queued {
                prompt.status = PromptStatus::Failed;
                prompt.reason = Some(reason.to_string());
                prompt.text = None;
            }
        }
    }

    pub fn without_prompt_texts(&self) -> Session {
        let mut session = self.clone();
        for prompt in &mut session.prompts {
            prompt.text = None;
        }
        session
    }

    pub fn queued_prompts(&self) -> usize {
        self.prompts
            .iter()
            .filter(|prompt| prompt.status == PromptStatus::Queued)
            .count()
    }
}

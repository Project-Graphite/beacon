use crate::events::ToolUse;
use crate::redact::redact;
use crate::session::Session;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const STEPS_KEPT: usize = 60;
pub const LABEL_CHARS: usize = 200;
const SUBJECT_CHARS: usize = 120;
const COMMAND_CHARS: usize = 2_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub subject: String,
    pub done: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Prompt,
    Read,
    Edit,
    Run,
    Search,
    Web,
    Agent,
    Task,
    Mcp,
    Permission,
    Question,
    Plan,
    Notice,
    Finish,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Running,
    Done,
    Failed,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub id: u64,
    pub t_ms: u64,
    pub kind: StepKind,
    pub label: String,
    pub status: StepStatus,
    pub duration_ms: Option<u64>,
    #[serde(skip)]
    tool_use_id: Option<String>,
    #[serde(skip)]
    command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurrentStep {
    pub kind: StepKind,
    pub label: String,
}

pub fn step_kind(event: &str, tool_name: Option<&str>) -> StepKind {
    match (event, tool_name) {
        (_, Some("AskUserQuestion")) => StepKind::Question,
        (_, Some("ExitPlanMode")) => StepKind::Plan,
        ("PermissionRequest" | "PermissionResolved", _) => StepKind::Permission,
        (_, Some("Read")) => StepKind::Read,
        (_, Some("Edit" | "MultiEdit" | "Write" | "NotebookEdit")) => StepKind::Edit,
        (_, Some("Bash" | "PowerShell")) => StepKind::Run,
        (_, Some("Grep" | "Glob" | "ToolSearch")) => StepKind::Search,
        (_, Some("WebFetch" | "WebSearch")) => StepKind::Web,
        (_, Some("Agent")) | ("SubagentStart" | "SubagentStop", _) => StepKind::Agent,
        (_, Some("TaskCreate" | "TaskUpdate")) | ("TaskCreated" | "TaskCompleted", _) => {
            StepKind::Task
        }
        (_, Some(tool)) if tool.starts_with("mcp__") => StepKind::Mcp,
        ("UserPromptSubmit", _) => StepKind::Prompt,
        ("Stop", _) => StepKind::Finish,
        ("StopFailure", _) => StepKind::Error,
        _ => StepKind::Notice,
    }
}

fn step(kind: StepKind, label: &str, status: StepStatus, now: u64) -> Step {
    Step {
        id: 0,
        t_ms: now,
        kind,
        label: label.chars().take(LABEL_CHARS).collect(),
        status,
        duration_ms: None,
        tool_use_id: None,
        command: None,
    }
}

fn shown_subject(subject: &str) -> String {
    redact(subject).chars().take(SUBJECT_CHARS).collect()
}

impl Session {
    pub fn create_task(&mut self, id: String, subject: &str) {
        self.tasks.push(Task {
            id,
            subject: shown_subject(subject),
            done: false,
        });
        self.settle_tasks();
    }

    pub fn complete_task(&mut self, id: String, subject: &str) {
        match self.tasks.iter_mut().find(|task| task.id == id) {
            Some(task) => task.done = true,
            None => self.tasks.push(Task {
                id,
                subject: shown_subject(subject),
                done: true,
            }),
        }
        self.settle_tasks();
    }

    pub fn clear_tasks(&mut self) {
        self.tasks.clear();
        self.settle_tasks();
    }

    fn settle_tasks(&mut self) {
        self.tasks_created = self.tasks.len() as u32;
        self.tasks_completed = self.tasks.iter().filter(|task| task.done).count() as u32;
        self.task_current = self
            .tasks
            .iter()
            .find(|task| !task.done)
            .map(|task| task.subject.clone());
    }

    pub fn add_step(&mut self, kind: StepKind, label: &str, now: u64) {
        self.push_step(step(kind, label, StepStatus::Done, now));
    }

    pub(crate) fn start_step(&mut self, tool_use: &ToolUse, kind: StepKind, label: &str, now: u64) {
        if tool_use.tool_use_id.is_some()
            && self
                .steps
                .iter()
                .any(|step| step.tool_use_id == tool_use.tool_use_id)
        {
            return;
        }
        self.push_step(Step {
            tool_use_id: tool_use.tool_use_id.clone(),
            command: tool_use
                .tool_input
                .as_ref()
                .and_then(|input| input.get("command"))
                .and_then(Value::as_str)
                .filter(|_| kind == StepKind::Run)
                .map(|command| redact(command).chars().take(COMMAND_CHARS).collect()),
            ..step(kind, label, StepStatus::Running, now)
        });
    }

    pub(crate) fn finish_step(
        &mut self,
        tool_use: &ToolUse,
        kind: StepKind,
        label: &str,
        failed: bool,
        now: u64,
    ) {
        let status = if failed {
            StepStatus::Failed
        } else {
            StepStatus::Done
        };
        match self
            .steps
            .iter_mut()
            .find(|step| tool_use.tool_use_id.is_some() && step.tool_use_id == tool_use.tool_use_id)
        {
            Some(step) => {
                step.status = status;
                step.duration_ms = tool_use.duration_ms.or(Some(now.saturating_sub(step.t_ms)));
                self.settle_current_step();
            }
            None => self.push_step(Step {
                tool_use_id: tool_use.tool_use_id.clone(),
                duration_ms: tool_use.duration_ms,
                ..step(kind, label, status, now)
            }),
        }
    }

    pub fn stop_running_steps(&mut self) {
        for step in &mut self.steps {
            if step.status == StepStatus::Running {
                step.status = StepStatus::Stopped;
            }
        }
        self.settle_current_step();
    }

    fn push_step(&mut self, step: Step) {
        let id = self.steps.back().map_or(1, |last| last.id + 1);
        if self.steps.len() == STEPS_KEPT {
            self.steps.pop_front();
        }
        self.steps.push_back(Step { id, ..step });
        self.settle_current_step();
    }

    fn settle_current_step(&mut self) {
        let mut running = self
            .steps
            .iter()
            .rev()
            .filter(|step| step.status == StepStatus::Running);
        self.step_current = running.clone().next().map(|step| CurrentStep {
            kind: step.kind,
            label: step.label.clone(),
        });
        self.command_current = running.find_map(|step| step.command.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_kinds_follow_the_event_and_the_tool() {
        let cases = [
            ("UserPromptSubmit", None, StepKind::Prompt),
            ("PreToolUse", Some("Read"), StepKind::Read),
            ("PreToolUse", Some("Edit"), StepKind::Edit),
            ("PostToolUse", Some("Write"), StepKind::Edit),
            ("PreToolUse", Some("NotebookEdit"), StepKind::Edit),
            ("PreToolUse", Some("Bash"), StepKind::Run),
            ("PostToolUseFailure", Some("PowerShell"), StepKind::Run),
            ("PreToolUse", Some("Grep"), StepKind::Search),
            ("PreToolUse", Some("Glob"), StepKind::Search),
            ("PreToolUse", Some("ToolSearch"), StepKind::Search),
            ("PreToolUse", Some("WebFetch"), StepKind::Web),
            ("PreToolUse", Some("WebSearch"), StepKind::Web),
            ("PreToolUse", Some("Agent"), StepKind::Agent),
            ("SubagentStart", None, StepKind::Agent),
            ("SubagentStop", None, StepKind::Agent),
            ("PreToolUse", Some("TaskCreate"), StepKind::Task),
            ("PostToolUse", Some("TaskUpdate"), StepKind::Task),
            ("TaskCreated", None, StepKind::Task),
            ("TaskCompleted", None, StepKind::Task),
            ("PreToolUse", Some("mcp__github__get_issue"), StepKind::Mcp),
            ("PreToolUse", Some("AskUserQuestion"), StepKind::Question),
            (
                "PermissionRequest",
                Some("AskUserQuestion"),
                StepKind::Question,
            ),
            ("PreToolUse", Some("ExitPlanMode"), StepKind::Plan),
            ("PermissionResolved", Some("ExitPlanMode"), StepKind::Plan),
            ("PermissionRequest", Some("Bash"), StepKind::Permission),
            (
                "PermissionResolved",
                Some("mcp__x__y"),
                StepKind::Permission,
            ),
            ("Stop", None, StepKind::Finish),
            ("StopFailure", None, StepKind::Error),
            ("PreToolUse", Some("LSP"), StepKind::Notice),
            ("Notification", None, StepKind::Notice),
            ("SessionStart", None, StepKind::Notice),
            ("PreCompact", None, StepKind::Notice),
        ];
        for (event, tool_name, expected) in cases {
            assert_eq!(
                step_kind(event, tool_name),
                expected,
                "{event} {tool_name:?}"
            );
        }
    }

    #[test]
    fn steps_serialize_without_their_pairing_and_command() {
        let mut session = Session::new("s", 0);
        session.start_step(
            &ToolUse {
                tool_name: "Bash".to_string(),
                tool_input: Some(serde_json::json!({"command": "ls"})),
                tool_use_id: Some("t1".to_string()),
                duration_ms: None,
            },
            StepKind::Run,
            "Running ls",
            5,
        );
        assert_eq!(
            serde_json::to_string(&session.steps[0]).unwrap(),
            r#"{"id":1,"t_ms":5,"kind":"run","label":"Running ls","status":"running","duration_ms":null}"#
        );
    }
}

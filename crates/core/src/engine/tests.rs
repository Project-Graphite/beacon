use super::*;
use crate::account::UsageSource;
use crate::config::LogLevel;
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const INSTRUCTIONS_LOADED: &[u8] =
    include_bytes!("../../../../fixtures/hook-payloads/InstructionsLoaded.json");
const SESSION_START: &[u8] = include_bytes!("../../../../fixtures/hook-payloads/SessionStart.json");
const USER_PROMPT_SUBMIT: &[u8] =
    include_bytes!("../../../../fixtures/hook-payloads/UserPromptSubmit.json");
const PRE_TOOL_USE: &[u8] = include_bytes!("../../../../fixtures/hook-payloads/PreToolUse.json");
const PERMISSION_REQUEST: &[u8] =
    include_bytes!("../../../../fixtures/hook-payloads/PermissionRequest.json");
const POST_TOOL_USE: &[u8] = include_bytes!("../../../../fixtures/hook-payloads/PostToolUse.json");
const POST_TOOL_BATCH: &[u8] =
    include_bytes!("../../../../fixtures/hook-payloads/PostToolBatch.json");
const STOP: &[u8] = include_bytes!("../../../../fixtures/hook-payloads/Stop.json");
const SESSION_END: &[u8] = include_bytes!("../../../../fixtures/hook-payloads/SessionEnd.json");
const POST_MODEL_SWITCH: &[u8] =
    include_bytes!("../../../../fixtures/hook-payloads/PostModelSwitch.json");
const SESSION: &str = "4c2c3535-edec-4bb7-90b7-38049912db27";
const TOOL_USE_ID: &str = "toolu_01QQmzYEo3yJo3twgzeTbi3n";

struct TempHome(PathBuf);

impl Drop for TempHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Fixture {
    engine: Engine,
    exits: mpsc::UnboundedReceiver<ProcessExit>,
    home: TempHome,
}

fn fixture_with(config: Config, sessions: Vec<Session>) -> Fixture {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let home = std::env::temp_dir().join(format!(
        "beacon-engine-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let log = Logger::open(&home.join("core.log"), LogLevel::Debug).unwrap();
    let (engine, exits) = Engine::new(config, log, sessions);
    Fixture {
        engine,
        exits,
        home: TempHome(home),
    }
}

fn fixture() -> Fixture {
    fixture_with(Config::default(), Vec::new())
}

fn meta(event: &str, env: &[(&str, &str)]) -> Vec<u8> {
    let env: serde_json::Map<String, Value> = env
        .iter()
        .map(|(name, value)| (name.to_string(), json!(value)))
        .collect();
    json!({"v": 1, "event": event, "pid": 1, "t_ms": 0, "env": env})
        .to_string()
        .into_bytes()
}

fn event_name(payload: &[u8]) -> String {
    serde_json::from_slice::<Value>(payload).unwrap()["hook_event_name"]
        .as_str()
        .unwrap()
        .to_string()
}

impl Fixture {
    fn send(&mut self, payload: &[u8]) -> HookOutcome {
        self.send_with_env(payload, &[])
    }

    fn send_with_env(&mut self, payload: &[u8], env: &[(&str, &str)]) -> HookOutcome {
        self.engine
            .handle_hook(&meta(&event_name(payload), env), payload)
    }

    fn ask(&mut self, payload: &[u8]) -> (u64, oneshot::Receiver<HookReply>) {
        match self.send(payload) {
            HookOutcome::Wait(id, receiver) => (id, receiver),
            _ => panic!("expected a pending decision"),
        }
    }

    fn session(&self) -> Session {
        self.engine.sessions[SESSION].clone()
    }

    fn state(&self) -> SessionState {
        self.session().state
    }
}

fn output(decision: Decision, tool_input: Value) -> String {
    String::from_utf8(hook_output(&decision, &tool_input)).unwrap()
}

fn custom(event: &str, extra: Value) -> Vec<u8> {
    let mut payload =
        json!({"session_id": SESSION, "hook_event_name": event, "cwd": r"C:\nowhere"});
    payload
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    payload.to_string().into_bytes()
}

#[test]
fn allow_outputs_exact_json() {
    assert_eq!(
        output(Decision::Allow, json!({})),
        r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}"#
    );
}

#[test]
fn allow_with_input_outputs_exact_json() {
    assert_eq!(
        output(
            Decision::AllowWithInput {
                input: json!({"command": "npm run lint", "description": "Lint"})
            },
            json!({"command": "npm run build"})
        ),
        r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow","updatedInput":{"command":"npm run lint","description":"Lint"}}}}"#
    );
}

#[test]
fn allow_always_outputs_exact_json() {
    let rule = json!({"type": "addRules", "rules": [{"toolName": "Bash", "ruleContent": "node -e *"}], "behavior": "allow", "destination": "session"});
    assert_eq!(
        output(
            Decision::AllowAlways {
                permissions: vec![rule]
            },
            json!({})
        ),
        r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow","updatedPermissions":[{"type":"addRules","rules":[{"toolName":"Bash","ruleContent":"node -e *"}],"behavior":"allow","destination":"session"}]}}}"#
    );
}

#[test]
fn answer_outputs_exact_json_echoing_the_questions() {
    let tool_input = json!({"questions": [{"question": "Which colour?", "header": "Colour", "options": [{"label": "Red", "description": "Warm"}, {"label": "Blue", "description": "Cool"}], "multiSelect": false}]});
    let answers = json!({"Which colour?": "Red"}).as_object().unwrap().clone();
    assert_eq!(
        output(Decision::Answer { answers }, tool_input),
        r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow","updatedInput":{"questions":[{"question":"Which colour?","header":"Colour","options":[{"label":"Red","description":"Warm"},{"label":"Blue","description":"Cool"}],"multiSelect":false}],"answers":{"Which colour?":"Red"}}}}}"#
    );
}

#[test]
fn deny_outputs_exact_json_with_and_without_message() {
    assert_eq!(
        output(
            Decision::Deny {
                message: Some("Denied from Beacon".to_string())
            },
            json!({})
        ),
        r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"Denied from Beacon"}}}"#
    );
    assert_eq!(
        output(Decision::Deny { message: None }, json!({})),
        r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny"}}}"#
    );
}

#[test]
fn deny_and_stop_outputs_exact_json_with_and_without_message() {
    assert_eq!(
        output(
            Decision::DenyAndStop {
                message: Some("Stop here".to_string())
            },
            json!({})
        ),
        r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"Stop here","interrupt":true}}}"#
    );
    assert_eq!(
        output(Decision::DenyAndStop { message: None }, json!({})),
        r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","interrupt":true}}}"#
    );
}

#[test]
fn tool_use_id_joins_when_pre_tool_use_arrives_first() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    fixture.send(PRE_TOOL_USE);
    let (id, _receiver) = fixture.ask(PERMISSION_REQUEST);
    let pending = &fixture.engine.snapshot().pending[0];
    assert_eq!(pending.id, id);
    assert_eq!(pending.tool_use_id.as_deref(), Some(TOOL_USE_ID));
    assert_eq!(pending.kind, PermissionKind::Tool);
    assert_eq!(pending.tool_name, "Bash");
    assert_eq!(fixture.state(), SessionState::NeedsPermission);
}

#[test]
fn tool_use_id_joins_when_permission_request_arrives_first() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    let (_id, _receiver) = fixture.ask(PERMISSION_REQUEST);
    assert_eq!(fixture.engine.snapshot().pending[0].tool_use_id, None);
    fixture.send(PRE_TOOL_USE);
    assert_eq!(
        fixture.engine.snapshot().pending[0].tool_use_id.as_deref(),
        Some(TOOL_USE_ID)
    );
    assert_eq!(fixture.state(), SessionState::NeedsPermission);
}

#[test]
fn tool_use_id_does_not_join_a_different_input_or_agent() {
    let mut fixture = fixture();
    fixture.send(&custom(
        "PreToolUse",
        json!({"tool_name": "Bash", "tool_input": {"command": "other"}, "tool_use_id": "toolu_other"}),
    ));
    let other_agent = String::from_utf8(PRE_TOOL_USE.to_vec()).unwrap().replacen(
        "\"cwd\"",
        "\"agent_id\": \"agent-1\", \"cwd\"",
        1,
    );
    fixture.send(other_agent.as_bytes());
    let (_id, _receiver) = fixture.ask(PERMISSION_REQUEST);
    assert_eq!(fixture.engine.snapshot().pending[0].tool_use_id, None);
}

#[test]
fn post_tool_use_resolves_a_pending_decision_as_allowed_in_session() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    fixture.send(PRE_TOOL_USE);
    let (id, mut receiver) = fixture.ask(PERMISSION_REQUEST);
    fixture.send(POST_TOOL_USE);
    assert_eq!(receiver.try_recv().unwrap(), HookReply::NoDecision);
    let snapshot = fixture.engine.snapshot();
    assert!(snapshot.pending.is_empty());
    assert_eq!(snapshot.resolved[0].id, id);
    assert_eq!(
        snapshot.resolved[0].resolution,
        Resolution::AllowedInSession
    );
    assert_eq!(fixture.state(), SessionState::Working);
    fixture.send(POST_TOOL_BATCH);
    assert_eq!(fixture.engine.snapshot().resolved.len(), 1);
}

#[test]
fn post_tool_batch_without_post_tool_use_resolves_as_denied_in_session() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    let (id, mut receiver) = fixture.ask(PERMISSION_REQUEST);
    fixture.send(PRE_TOOL_USE);
    fixture.send(POST_TOOL_BATCH);
    assert_eq!(receiver.try_recv().unwrap(), HookReply::NoDecision);
    let snapshot = fixture.engine.snapshot();
    assert!(snapshot.pending.is_empty());
    assert_eq!(snapshot.resolved[0].id, id);
    assert_eq!(snapshot.resolved[0].resolution, Resolution::DeniedInSession);
    assert_eq!(fixture.state(), SessionState::Working);
}

#[test]
fn deciding_delivers_the_output_and_resolves_the_session() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    let (id, mut receiver) = fixture.ask(PERMISSION_REQUEST);
    fixture.engine.decide(id, Decision::Allow).unwrap();
    assert_eq!(
        receiver.try_recv().unwrap(),
        HookReply::Stdout(hook_output(&Decision::Allow, &json!({})))
    );
    assert_eq!(fixture.state(), SessionState::Working);
    assert_eq!(
        fixture.engine.snapshot().resolved[0].resolution,
        Resolution::Decided
    );
    assert!(fixture.engine.decide(id, Decision::Allow).is_err());
}

#[test]
fn deny_and_stop_leaves_the_session_idle() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    let (id, _receiver) = fixture.ask(PERMISSION_REQUEST);
    fixture
        .engine
        .decide(id, Decision::DenyAndStop { message: None })
        .unwrap();
    assert_eq!(fixture.state(), SessionState::Idle);
}

#[test]
fn the_session_stays_waiting_until_its_last_decision_goes() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    let (first, _first_receiver) = fixture.ask(PERMISSION_REQUEST);
    let (second, _second_receiver) = fixture.ask(PERMISSION_REQUEST);
    fixture.engine.decide(first, Decision::Allow).unwrap();
    assert_eq!(fixture.state(), SessionState::NeedsPermission);
    fixture.engine.decide(second, Decision::Allow).unwrap();
    assert_eq!(fixture.state(), SessionState::Working);
}

#[test]
fn questions_and_plans_have_their_own_kinds() {
    let mut fixture = fixture();
    let question = custom(
        "PermissionRequest",
        json!({"tool_name": "AskUserQuestion", "tool_input": {"questions": [{"question": "Which?"}]}}),
    );
    let (question_id, mut receiver) = fixture.ask(&question);
    assert_eq!(fixture.state(), SessionState::NeedsAnswer);
    let answers = json!({"Which?": "This"}).as_object().unwrap().clone();
    fixture
        .engine
        .decide(question_id, Decision::Answer { answers })
        .unwrap();
    let HookReply::Stdout(bytes) = receiver.try_recv().unwrap() else {
        panic!("expected stdout");
    };
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow","updatedInput":{"questions":[{"question":"Which?"}],"answers":{"Which?":"This"}}}}}"#
    );
    let plan = custom(
        "PermissionRequest",
        json!({"tool_name": "ExitPlanMode", "tool_input": {}}),
    );
    let (plan_id, _plan_receiver) = fixture.ask(&plan);
    assert_eq!(fixture.state(), SessionState::NeedsPlanApproval);
    let answers = json!({"Which?": "This"}).as_object().unwrap().clone();
    assert!(
        fixture
            .engine
            .decide(plan_id, Decision::Answer { answers })
            .is_err()
    );
}

#[test]
fn a_helper_that_goes_away_expires_its_decision() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    let (id, receiver) = fixture.ask(PERMISSION_REQUEST);
    fixture.engine.helper_gone(id);
    drop(receiver);
    let snapshot = fixture.engine.snapshot();
    assert!(snapshot.pending.is_empty());
    assert_eq!(snapshot.resolved[0].resolution, Resolution::Expired);
    assert_eq!(fixture.state(), SessionState::Working);
}

#[test]
fn deciding_after_the_helper_left_reports_expiry() {
    let mut fixture = fixture();
    let (id, receiver) = fixture.ask(PERMISSION_REQUEST);
    drop(receiver);
    assert!(fixture.engine.decide(id, Decision::Allow).is_err());
    assert_eq!(
        fixture.engine.snapshot().resolved[0].resolution,
        Resolution::Expired
    );
}

#[test]
fn unattended_sessions_get_no_decision_and_no_card() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    let outcome =
        fixture.send_with_env(PERMISSION_REQUEST, &[("CLAUDE_CODE_SESSION_ATTENDED", "0")]);
    assert!(matches!(outcome, HookOutcome::Reply(HookReply::NoDecision)));
    assert!(fixture.engine.snapshot().pending.is_empty());
    assert_eq!(fixture.state(), SessionState::Working);
    assert_eq!(fixture.session().attended, Some(false));
    assert_eq!(fixture.session().event_count, 2);
}

#[test]
fn unattended_sessions_are_held_when_configured() {
    let mut fixture = fixture_with(
        Config {
            answer_unattended: true,
            ..Config::default()
        },
        Vec::new(),
    );
    let outcome =
        fixture.send_with_env(PERMISSION_REQUEST, &[("CLAUDE_CODE_SESSION_ATTENDED", "0")]);
    assert!(matches!(outcome, HookOutcome::Wait(..)));
}

#[test]
fn waiting_events_without_a_decision_reply_at_once() {
    let mut fixture = fixture();
    assert!(matches!(
        fixture.send(STOP),
        HookOutcome::Reply(HookReply::NoDecision)
    ));
    assert_eq!(fixture.state(), SessionState::Done);
    assert_eq!(
        fixture.session().last_message_preview.as_deref(),
        Some("beacon-spike-ok")
    );
    let elicitation = custom(
        "Elicitation",
        json!({"mcp_server_name": "x", "message": "m"}),
    );
    assert!(matches!(
        fixture.send(&elicitation),
        HookOutcome::Reply(HookReply::NoDecision)
    ));
    assert_eq!(fixture.state(), SessionState::NeedsMcpInput);
    fixture.send(&custom("ElicitationResult", json!({})));
    assert_eq!(fixture.state(), SessionState::Working);
    assert!(matches!(fixture.send(PRE_TOOL_USE), HookOutcome::Done));
}

#[test]
fn doctor_replies_ok_and_leaves_the_registry_alone() {
    let mut fixture = fixture();
    let outcome = fixture
        .engine
        .handle_hook(&meta("BeaconDoctor", &[]), b"not json at all");
    assert!(
        matches!(outcome, HookOutcome::Reply(HookReply::Stdout(bytes)) if bytes == br#"{"beacon":"ok"}"#)
    );
    assert!(fixture.engine.snapshot().sessions.is_empty());
}

#[test]
fn the_fixture_sequence_walks_the_state_machine() {
    let mut fixture = fixture();
    let env = [
        ("CLAUDE_CODE_ENTRYPOINT", "sdk-cli"),
        ("CLAUDE_PROJECT_DIR", r"C:\STUFF\Project-Graphite\beacon"),
    ];
    let steps: [(&[u8], SessionState); 6] = [
        (INSTRUCTIONS_LOADED, SessionState::Starting),
        (SESSION_START, SessionState::Starting),
        (USER_PROMPT_SUBMIT, SessionState::Working),
        (PRE_TOOL_USE, SessionState::Working),
        (POST_TOOL_USE, SessionState::Working),
        (POST_TOOL_BATCH, SessionState::Working),
    ];
    for (payload, expected) in steps {
        fixture.send_with_env(payload, &env);
        assert_eq!(fixture.state(), expected, "{}", event_name(payload));
    }
    let session = fixture.session();
    assert_eq!(
        session.activity.as_deref(),
        Some("Running node -e \"console.log('beacon-spike-ok')\"")
    );
    assert_eq!(session.entrypoint.as_deref(), Some("sdk-cli"));
    assert_eq!(session.project.as_deref(), Some("beacon"));
    assert_eq!(session.permission_mode.as_deref(), Some("default"));
    assert!(session.turn_started_ms.is_some());
    fixture.send(STOP);
    assert_eq!(fixture.state(), SessionState::Done);
    assert!(fixture.session().attention);
    fixture.engine.acknowledge(SESSION, now_ms()).unwrap();
    assert_eq!(fixture.state(), SessionState::Idle);
    fixture.send(POST_MODEL_SWITCH);
    fixture.send(SESSION_END);
    assert_eq!(fixture.state(), SessionState::Ended);
    assert!(fixture.engine.acknowledge("missing", now_ms()).is_err());
}

#[test]
fn session_fields_follow_their_events() {
    let mut fixture = fixture();
    fixture.send(&custom(
        "SessionStart",
        json!({"source": "resume", "model": "claude-opus-5", "session_title": "Title"}),
    ));
    let session = fixture.session();
    assert_eq!(session.model.as_deref(), Some("claude-opus-5"));
    assert_eq!(session.title.as_deref(), Some("Title"));
    fixture.send(&custom(
        "PostModelSwitch",
        json!({"to_model": "claude-haiku-5-5"}),
    ));
    assert_eq!(fixture.session().model.as_deref(), Some("claude-haiku-5-5"));
    fixture.send(&custom(
        "CwdChanged",
        json!({"old_cwd": r"C:\nowhere", "new_cwd": r"C:\elsewhere\deeper"}),
    ));
    assert_eq!(
        fixture.session().cwd.as_deref(),
        Some(r"C:\elsewhere\deeper")
    );
    assert_eq!(fixture.session().project.as_deref(), Some("deeper"));
    fixture.send(&custom("StopFailure", json!({"error": "rate_limit"})));
    assert_eq!(fixture.state(), SessionState::Error);
    assert_eq!(fixture.session().last_error.as_deref(), Some("rate_limit"));
    fixture.send(&custom(
        "SubagentStart",
        json!({"agent_id": "a", "agent_type": "Explore"}),
    ));
    fixture.send(&custom("TaskCreated", json!({})));
    fixture.send(&custom("TaskCompleted", json!({})));
    fixture.send(&custom("PreCompact", json!({})));
    let session = fixture.session();
    assert_eq!(session.overlays.subagents, 1);
    assert!(session.overlays.compacting);
    assert_eq!((session.tasks_created, session.tasks_completed), (1, 1));
    assert_eq!(session.state, SessionState::Working);
    fixture.send(&custom("SessionStart", json!({"source": "compact"})));
    assert!(!fixture.session().overlays.compacting);
    fixture.send(&custom(
        "Stop",
        json!({"background_tasks": [{"id": "t"}, {"id": "u"}]}),
    ));
    assert_eq!(fixture.state(), SessionState::Working);
    assert_eq!(fixture.session().overlays.background_tasks, 2);
    fixture.send(&custom(
        "Notification",
        json!({"notification_type": "elicitation_dialog"}),
    ));
    assert_eq!(fixture.state(), SessionState::NeedsMcpInput);
    fixture.send(&custom(
        "Notification",
        json!({"notification_type": "idle_prompt"}),
    ));
    assert_eq!(fixture.state(), SessionState::NeedsMcpInput);
    fixture.send(&custom(
        "Notification",
        json!({"notification_type": "elicitation_response"}),
    ));
    assert_eq!(fixture.state(), SessionState::Working);
}

#[test]
fn unknown_events_and_bad_payloads_are_dropped() {
    let mut fixture = fixture();
    assert!(matches!(
        fixture.send(&custom("FutureEvent", json!({}))),
        HookOutcome::Done
    ));
    let not_json = fixture
        .engine
        .handle_hook(&meta("PermissionRequest", &[]), b"{\"session_id\":");
    assert!(matches!(
        not_json,
        HookOutcome::Reply(HookReply::NoDecision)
    ));
    let not_object = fixture.engine.handle_hook(&meta("Stop", &[]), b"[1,2]");
    assert!(matches!(
        not_object,
        HookOutcome::Reply(HookReply::NoDecision)
    ));
    let no_session = fixture.engine.handle_hook(
        &meta("PreToolUse", &[]),
        br#"{"hook_event_name":"PreToolUse"}"#,
    );
    assert!(matches!(no_session, HookOutcome::Done));
    let wrong_type = custom("Stop", json!({"background_tasks": "many"}));
    assert!(matches!(
        fixture.send(&wrong_type),
        HookOutcome::Reply(HookReply::NoDecision)
    ));
    assert!(matches!(
        fixture.engine.handle_hook(b"{", PRE_TOOL_USE),
        HookOutcome::Done
    ));
    assert!(matches!(
        fixture.engine.handle_hook(&meta("Stop", &[]), b""),
        HookOutcome::Reply(HookReply::NoDecision)
    ));
    assert!(fixture.engine.snapshot().sessions.is_empty());
}

#[test]
fn unknown_fields_are_ignored() {
    let mut fixture = fixture();
    fixture.send(&custom(
        "UserPromptSubmit",
        json!({"prompt": "x", "brand_new": {"nested": [1, 2, 3]}, "effort": {"level": "max"}}),
    ));
    fixture.send(&custom(
        "SubagentStop",
        json!({"agent_id": "a", "background_tasks": [], "later": true}),
    ));
    assert_eq!(fixture.state(), SessionState::Working);
    assert_eq!(fixture.session().event_count, 2);
}

#[test]
fn huge_strings_are_bounded() {
    let mut fixture = fixture();
    let command = "x".repeat(4 * 1024 * 1024);
    fixture.send(&custom(
        "PreToolUse",
        json!({"tool_name": "Bash", "tool_input": {"command": command}, "tool_use_id": "t"}),
    ));
    assert_eq!(
        fixture.session().activity.unwrap().chars().count(),
        "Running ".len() + 80
    );
    fixture.send(&custom(
        "Stop",
        json!({"last_assistant_message": "y".repeat(4 * 1024 * 1024)}),
    ));
    assert_eq!(
        fixture
            .session()
            .last_message_preview
            .unwrap()
            .chars()
            .count(),
        400
    );
    let long_name = "z".repeat(1024 * 1024);
    fixture.send(&custom(
        "PreToolUse",
        json!({"tool_name": long_name, "tool_use_id": "u"}),
    ));
    assert_eq!(fixture.state(), SessionState::Working);
}

#[test]
fn registry_persists_and_reloads() {
    let mut fixture = fixture();
    fixture.send_with_env(
        SESSION_START,
        &[("CLAUDE_CODE_ENTRYPOINT", "claude-vscode")],
    );
    fixture.send(USER_PROMPT_SUBMIT);
    fixture.send(PRE_TOOL_USE);
    let (_id, _receiver) = fixture.ask(PERMISSION_REQUEST);
    fixture.send(&custom(
        "SessionStart",
        json!({"session_id": "second", "source": "startup"}),
    ));
    let persisted = fixture.engine.persisted_state().unwrap();
    let reloaded = Engine::load_sessions(&persisted).unwrap();
    assert_eq!(reloaded, fixture.engine.snapshot().sessions);
    let restored = fixture_with(Config::default(), reloaded);
    let snapshot = restored.engine.snapshot();
    assert_eq!(snapshot.sessions, fixture.engine.snapshot().sessions);
    assert!(snapshot.pending.is_empty());
    assert_eq!(
        snapshot.sessions[0].entrypoint.as_deref(),
        Some("claude-vscode")
    );
}

fn finished_pid() -> u32 {
    let mut child = std::process::Command::new("cmd.exe")
        .args(["/c", "exit"])
        .spawn()
        .unwrap();
    child.wait().unwrap();
    child.id()
}

fn listed(session_id: &str, pid: Option<u32>, status: &str) -> ListedSession {
    ListedSession {
        session_id: session_id.to_string(),
        pid,
        cwd: Some(r"C:\STUFF\listed".to_string()),
        kind: Some("interactive".to_string()),
        started_at: Some(1),
        name: Some(format!("name-{session_id}")),
        status: Some(status.to_string()),
        waiting_for: None,
    }
}

#[test]
fn reconciliation_adds_limited_sessions_and_feeds_statuses() {
    let mut fixture = fixture();
    let own_pid = std::process::id();
    fixture.send(USER_PROMPT_SUBMIT);
    fixture.send(&custom(
        "SessionStart",
        json!({"session_id": "alive", "source": "startup"}),
    ));
    fixture.send_with_env(
        &custom("UserPromptSubmit", json!({"session_id": "alive"})),
        &[("CLAUDE_PID", &own_pid.to_string())],
    );
    let gone_pid = finished_pid().to_string();
    fixture.send_with_env(
        &custom("UserPromptSubmit", json!({"session_id": "gone"})),
        &[("CLAUDE_PID", &gone_pid)],
    );
    fixture.engine.reconcile(vec![
        listed(SESSION, None, "idle"),
        listed("new-busy", Some(own_pid), "busy"),
        listed("new-waiting", None, "waiting"),
    ]);
    let sessions: BTreeMap<String, Session> = fixture
        .engine
        .snapshot()
        .sessions
        .into_iter()
        .map(|session| (session.session_id.clone(), session))
        .collect();
    assert_eq!(sessions[SESSION].state, SessionState::Idle);
    assert!(!sessions[SESSION].limited);
    assert_eq!(
        sessions[SESSION].listed_name.as_deref(),
        Some(format!("name-{SESSION}").as_str())
    );
    assert_eq!(sessions["new-busy"].state, SessionState::Working);
    assert!(sessions["new-busy"].limited);
    assert_eq!(sessions["new-busy"].claude_pid, Some(own_pid));
    assert_eq!(sessions["new-busy"].project.as_deref(), Some("listed"));
    assert_eq!(sessions["new-waiting"].state, SessionState::NeedsPermission);
    assert_eq!(sessions["alive"].state, SessionState::Working);
    assert_eq!(sessions["gone"].state, SessionState::Ended);
    fixture.engine.reconcile(Vec::new());
    assert_eq!(
        fixture.engine.sessions["new-waiting"].state,
        SessionState::NeedsPermission
    );
    assert_eq!(fixture.engine.sessions[SESSION].state, SessionState::Ended);
}

#[test]
fn process_exit_ends_only_the_watched_process() {
    let mut fixture = fixture();
    fixture.send_with_env(USER_PROMPT_SUBMIT, &[("CLAUDE_PID", "4242")]);
    fixture.engine.process_exited(SESSION, 4243);
    assert_eq!(fixture.state(), SessionState::Working);
    fixture.engine.process_exited(SESSION, 4242);
    assert_eq!(fixture.state(), SessionState::Ended);
}

#[test]
fn a_dead_claude_pid_is_reported_as_gone() {
    let mut fixture = fixture();
    let pid = finished_pid();
    fixture.send_with_env(USER_PROMPT_SUBMIT, &[("CLAUDE_PID", &pid.to_string())]);
    assert_eq!(
        fixture.exits.try_recv().unwrap(),
        (SESSION.to_string(), pid)
    );
}

#[test]
fn the_turn_end_is_recorded_once_and_cleared_by_the_next_prompt() {
    let mut fixture = fixture();
    fixture.send(SESSION_START);
    assert_eq!(fixture.session().turn_ended_ms, None);
    let ended_by = |fixture: &mut Fixture, end: &dyn Fn(&mut Fixture)| {
        fixture.send(USER_PROMPT_SUBMIT);
        let started = fixture.session().turn_started_ms.unwrap();
        assert_eq!(fixture.session().turn_ended_ms, None);
        end(fixture);
        let ended = fixture.session().turn_ended_ms.unwrap();
        assert!(ended >= started);
        ended
    };
    let stopped = ended_by(&mut fixture, &|fixture| {
        fixture.send(STOP);
    });
    fixture.send(SESSION_END);
    assert_eq!(fixture.session().turn_ended_ms, Some(stopped));
    ended_by(&mut fixture, &|fixture| {
        let (id, _receiver) = fixture.ask(PERMISSION_REQUEST);
        fixture
            .engine
            .decide(id, Decision::DenyAndStop { message: None })
            .unwrap();
    });
    ended_by(&mut fixture, &|fixture| {
        fixture
            .engine
            .reconcile(vec![listed(SESSION, None, "idle")]);
    });
    ended_by(&mut fixture, &|fixture| {
        fixture.send(&custom("StopFailure", json!({"error": "rate_limit"})));
    });
    ended_by(&mut fixture, &|fixture| {
        fixture.send(SESSION_END);
    });
    ended_by(&mut fixture, &|fixture| {
        fixture.send_with_env(USER_PROMPT_SUBMIT, &[("CLAUDE_PID", "4242")]);
        fixture.engine.process_exited(SESSION, 4242);
    });
    fixture.send(USER_PROMPT_SUBMIT);
    fixture.send(PRE_TOOL_USE);
    fixture
        .engine
        .reconcile(vec![listed(SESSION, None, "busy")]);
    assert_eq!(fixture.session().turn_ended_ms, None);
    let persisted = fixture.engine.persisted_state().unwrap();
    let without_the_field = String::from_utf8(persisted)
        .unwrap()
        .replace("\"turn_ended_ms\": null,", "");
    assert_eq!(
        Engine::load_sessions(without_the_field.as_bytes()).unwrap()[0].turn_ended_ms,
        None
    );
}

fn persisted(session_id: &str, state: SessionState, last_event_ms: u64) -> Session {
    Session {
        state,
        ..Session::new(session_id, last_event_ms)
    }
}

fn session_ids(engine: &Engine) -> Vec<String> {
    engine.sessions.keys().cloned().collect()
}

#[test]
fn ended_sessions_older_than_a_day_are_pruned_at_load() {
    let day = 24 * 60 * 60 * 1000;
    let now = now_ms();
    let fixture = fixture_with(
        Config::default(),
        vec![
            persisted("ended-old", SessionState::Ended, now - day - 60_000),
            persisted("ended-recent", SessionState::Ended, now - day + 60_000),
            persisted("idle-old", SessionState::Idle, now - 3 * day),
        ],
    );
    assert_eq!(
        session_ids(&fixture.engine),
        ["ended-recent".to_string(), "idle-old".to_string()]
    );
}

#[test]
fn ended_sessions_are_pruned_by_the_periodic_check() {
    let day = 24 * 60 * 60 * 1000;
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    fixture.send(SESSION_END);
    let ended_at = fixture.session().last_event_ms;
    let mut changes = fixture.engine.watch_changes();
    changes.borrow_and_update();
    fixture.engine.check_stalls(ended_at + day);
    assert_eq!(session_ids(&fixture.engine), [SESSION.to_string()]);
    assert!(!changes.has_changed().unwrap());
    fixture.engine.check_stalls(ended_at + day + 1);
    assert!(session_ids(&fixture.engine).is_empty());
    assert!(changes.has_changed().unwrap());
    let persisted = fixture.engine.persisted_state().unwrap();
    assert!(Engine::load_sessions(&persisted).unwrap().is_empty());
}

#[test]
fn snoozes_silence_alerting_until_they_expire() {
    let mut fixture = fixture();
    fixture.send(STOP);
    assert!(fixture.session().attention && fixture.session().alerting);
    let reply = fixture.engine.control(Request::Snooze {
        session_id: Some(SESSION.to_string()),
        minutes: 10,
    });
    assert!(reply.ok);
    let until = fixture.session().snoozed_until_ms.unwrap();
    assert!(fixture.session().attention && !fixture.session().alerting);
    assert_eq!(fixture.engine.next_snooze_expiry(), Some(until));
    let mut changes = fixture.engine.watch_changes();
    changes.borrow_and_update();
    fixture.engine.expire_snoozes(until - 1);
    assert!(!changes.has_changed().unwrap());
    fixture.engine.expire_snoozes(until);
    assert!(changes.has_changed().unwrap());
    assert_eq!(fixture.session().snoozed_until_ms, None);
    assert!(fixture.session().alerting);
    assert_eq!(fixture.engine.next_snooze_expiry(), None);
}

#[test]
fn a_global_snooze_silences_every_session_and_persists() {
    let mut fixture = fixture();
    fixture.send(STOP);
    assert!(
        fixture
            .engine
            .control(Request::Snooze {
                session_id: None,
                minutes: 5
            })
            .ok
    );
    let until = fixture.engine.snapshot().snoozed_until_ms.unwrap();
    assert!(!fixture.session().alerting);
    assert_eq!(fixture.session().snoozed_until_ms, None);
    let persisted = fixture.engine.persisted_state().unwrap();
    assert_eq!(Engine::load_global_snooze(&persisted).unwrap(), Some(until));
    assert_eq!(
        Engine::load_global_snooze(br#"{"sessions":[]}"#).unwrap(),
        None
    );
    assert!(
        fixture
            .engine
            .control(Request::Unsnooze { session_id: None })
            .ok
    );
    assert_eq!(fixture.engine.snapshot().snoozed_until_ms, None);
    assert!(fixture.session().alerting);
}

#[test]
fn leaving_every_attention_state_clears_the_session_snooze() {
    let mut fixture = fixture();
    fixture.send(STOP);
    fixture
        .engine
        .snooze(Some(SESSION), Some(u64::MAX))
        .unwrap();
    fixture.send(&custom("StopFailure", json!({"error": "rate_limit"})));
    assert_eq!(fixture.session().snoozed_until_ms, Some(u64::MAX));
    assert!(!fixture.session().alerting);
    fixture.send(USER_PROMPT_SUBMIT);
    assert_eq!(fixture.session().snoozed_until_ms, None);
    assert!(!fixture.session().alerting);
    fixture.send(STOP);
    assert!(fixture.session().alerting);
}

#[test]
fn snooze_requests_are_checked() {
    let mut fixture = fixture();
    fixture.send(STOP);
    let zero = fixture.engine.control(Request::Snooze {
        session_id: None,
        minutes: 0,
    });
    assert!(!zero.ok);
    let missing = fixture.engine.control(Request::Unsnooze {
        session_id: Some("missing".to_string()),
    });
    assert_eq!(missing, Reply::failed("no session missing"));
}

#[test]
fn the_detail_timeline_has_safe_labels_and_keeps_the_last_message() {
    let mut fixture = fixture();
    let secret = "PROMPT-TEXT-NEVER-SHOWN";
    fixture.send(&custom(
        "SessionStart",
        json!({"source": "startup", "model": "m"}),
    ));
    fixture.send(&custom(
        "UserPromptSubmit",
        json!({"prompt": secret, "session_title": "t"}),
    ));
    fixture.send(&custom(
        "PreToolUse",
        json!({"tool_name": "Bash", "tool_input": {"command": "GITHUB_TOKEN=abc123 gh pr list", "description": secret}, "tool_use_id": "t1"}),
    ));
    fixture.send(&custom(
        "PostToolUse",
        json!({"tool_name": "Bash", "tool_input": {"command": "GITHUB_TOKEN=abc123 gh pr list"}, "tool_use_id": "t1", "tool_response": {"stdout": secret}}),
    ));
    fixture.send(&custom(
        "SubagentStart",
        json!({"agent_id": "a", "agent_type": "Explore"}),
    ));
    fixture.send(&custom(
        "Notification",
        json!({"notification_type": "idle_prompt", "message": secret}),
    ));
    let (allowed, _allowed_receiver) = fixture.ask(PERMISSION_REQUEST);
    fixture.engine.decide(allowed, Decision::Allow).unwrap();
    let (denied, _denied_receiver) = fixture.ask(PERMISSION_REQUEST);
    fixture
        .engine
        .decide(denied, Decision::Deny { message: None })
        .unwrap();
    let (expired, _) = fixture.ask(PERMISSION_REQUEST);
    fixture.engine.helper_gone(expired);
    fixture.send(&custom("PreCompact", json!({"trigger": "auto"})));
    fixture.send(&custom(
        "StopFailure",
        json!({"error": "rate_limit", "error_details": secret}),
    ));
    let message = format!("{secret}{}", "ж".repeat(40_000));
    fixture.send(&custom("Stop", json!({"last_assistant_message": message})));
    fixture.send(&custom("SessionEnd", json!({"reason": "logout"})));
    let reply = fixture.engine.control(Request::Detail {
        session_id: SESSION.to_string(),
    });
    let detail = reply.detail.unwrap();
    assert_eq!(detail.session_id, SESSION);
    let labels: Vec<(&str, &str)> = detail
        .timeline
        .iter()
        .map(|entry| (entry.event.as_str(), entry.label.as_str()))
        .collect();
    assert_eq!(
        labels,
        [
            ("SessionStart", "Session started (startup)"),
            ("UserPromptSubmit", "Prompt submitted"),
            ("PreToolUse", "Running GITHUB_TOKEN=*** gh pr list"),
            ("PostToolUse", "Running GITHUB_TOKEN=*** gh pr list"),
            ("SubagentStart", "Subagent started: Explore"),
            ("Notification", "Notification"),
            ("PermissionRequest", "Permission requested: Bash"),
            ("PermissionResolved", "Allowed"),
            ("PermissionRequest", "Permission requested: Bash"),
            ("PermissionResolved", "Denied"),
            ("PermissionRequest", "Permission requested: Bash"),
            ("PermissionResolved", "Request expired"),
            ("PreCompact", "Compacting"),
            ("StopFailure", "API error: rate_limit"),
            ("Stop", "Turn finished"),
            ("SessionEnd", "Session ended (logout)"),
        ]
    );
    assert!(
        detail
            .timeline
            .iter()
            .all(|entry| !entry.label.contains(secret))
    );
    let last_message = detail.last_message.unwrap();
    assert!(last_message.starts_with(secret));
    assert_eq!(last_message.len(), 64 * 1024 - 1);
    assert!(
        fixture
            .engine
            .control(Request::Detail {
                session_id: "missing".to_string()
            })
            .detail
            .is_none()
    );
}

#[test]
fn the_timeline_keeps_sixty_entries_and_marks_answers_given_elsewhere() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    fixture.send(PRE_TOOL_USE);
    let (_allowed, _allowed_receiver) = fixture.ask(PERMISSION_REQUEST);
    fixture.send(POST_TOOL_USE);
    let (_denied, _denied_receiver) = fixture.ask(PERMISSION_REQUEST);
    fixture.send(PRE_TOOL_USE);
    fixture.send(POST_TOOL_BATCH);
    let labels: Vec<String> = fixture
        .engine
        .detail(SESSION)
        .unwrap()
        .timeline
        .into_iter()
        .filter(|entry| entry.event == "PermissionResolved")
        .map(|entry| entry.label)
        .collect();
    assert_eq!(labels, ["Allowed in session", "Denied in session"]);
    for _ in 0..100 {
        fixture.send(&custom("TaskCreated", json!({})));
    }
    let timeline = fixture.engine.detail(SESSION).unwrap().timeline;
    assert_eq!(timeline.len(), 60);
    assert!(timeline.iter().all(|entry| entry.label == "TaskCreated"));
}

#[test]
fn usage_is_wanted_for_a_new_transcript_and_after_turn_events() {
    let mut fixture = fixture();
    let transcript = r"C:\nowhere\t.jsonl";
    let with_transcript = |event: &str| custom(event, json!({"transcript_path": transcript}));
    fixture.send(&custom("SessionStart", json!({"source": "startup"})));
    assert!(fixture.engine.take_usage_wanted().is_empty());
    fixture.send(&with_transcript("UserPromptSubmit"));
    assert_eq!(
        fixture.engine.take_usage_wanted(),
        BTreeMap::from([(SESSION.to_string(), transcript.to_string())])
    );
    for event in ["PreToolUse", "PostToolUse", "Notification", "SubagentStart"] {
        fixture.send(&custom(
            event,
            json!({"tool_name": "Read", "transcript_path": transcript}),
        ));
        assert!(fixture.engine.take_usage_wanted().is_empty(), "{event}");
    }
    for event in ["Stop", "SubagentStop", "PostToolBatch", "SessionEnd"] {
        fixture.send(&with_transcript(event));
        assert_eq!(fixture.engine.take_usage_wanted().len(), 1, "{event}");
    }
    let restored = fixture_with(Config::default(), fixture.engine.snapshot().sessions);
    let mut restored = restored;
    assert_eq!(restored.engine.take_usage_wanted().len(), 1);
}

#[test]
fn usage_and_account_readings_reach_the_snapshot() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    let usage = SessionUsage {
        input_tokens: 1,
        output_tokens: 2,
        cache_creation_tokens: 3,
        cache_read_tokens: 4,
        context_tokens: 8,
        subagent_tokens: 0,
        reported_cost_usd: None,
        model: Some("m".to_string()),
        updated_ms: 5,
        status: UsageStatus::Ok,
    };
    fixture.engine.set_usage(SESSION, usage.clone());
    fixture.engine.set_usage("missing", usage.clone());
    assert_eq!(fixture.session().usage, Some(usage));
    let reading = |fetched_ms: u64, source: UsageSource| AccountUsage {
        five_hour_percent: Some(fetched_ms as f64),
        five_hour_resets_at: None,
        seven_day_percent: None,
        seven_day_resets_at: None,
        fetched_ms: Some(fetched_ms),
        source,
    };
    fixture
        .engine
        .offer_account_usage(reading(20, UsageSource::Probe));
    fixture
        .engine
        .offer_account_usage(reading(10, UsageSource::Cache));
    assert_eq!(
        fixture.engine.snapshot().account_usage,
        Some(reading(20, UsageSource::Probe))
    );
    fixture
        .engine
        .offer_account_usage(reading(30, UsageSource::Cache));
    assert_eq!(
        fixture.engine.snapshot().account_usage,
        Some(reading(30, UsageSource::Cache))
    );
}

#[test]
fn a_listed_session_whose_image_names_no_surface_keeps_no_entrypoint() {
    let mut fixture = fixture();
    fixture
        .engine
        .reconcile(vec![listed("listed", Some(std::process::id()), "idle")]);
    let session = &fixture.engine.sessions["listed"];
    assert_eq!(session.entrypoint, None);
    assert_eq!(session.editor, None);
    assert!(fixture.engine.runtime["listed"].image_read);
}

#[test]
fn working_sessions_stall_only_when_quiet_and_the_transcript_is_unchanged() {
    let mut fixture = fixture();
    let transcript = fixture.home.0.join("transcript.jsonl");
    std::fs::write(&transcript, "a").unwrap();
    fixture.send(&custom(
        "UserPromptSubmit",
        json!({"transcript_path": transcript.to_string_lossy()}),
    ));
    let start = fixture.session().last_event_ms;
    let ten_minutes = 10 * 60_000;
    fixture.engine.check_stalls(start);
    fixture.engine.check_stalls(start + ten_minutes - 1);
    assert_eq!(fixture.state(), SessionState::Working);
    std::fs::write(&transcript, "ab").unwrap();
    fixture.engine.check_stalls(start + ten_minutes);
    assert_eq!(fixture.state(), SessionState::Working);
    fixture.engine.check_stalls(start + 2 * ten_minutes - 1);
    assert_eq!(fixture.state(), SessionState::Working);
    fixture.engine.check_stalls(start + 2 * ten_minutes);
    assert_eq!(fixture.state(), SessionState::Stalled);
    fixture.send(PRE_TOOL_USE);
    assert_eq!(fixture.state(), SessionState::Working);
}

#[test]
fn acknowledging_clears_done_error_and_stalled_and_restarts_the_stall_clock() {
    let mut fixture = fixture();
    fixture.send(&custom("StopFailure", json!({"error": "rate_limit"})));
    assert_eq!(fixture.state(), SessionState::Error);
    fixture.engine.acknowledge(SESSION, now_ms()).unwrap();
    assert_eq!(fixture.state(), SessionState::Idle);
    fixture.send(&custom("UserPromptSubmit", json!({})));
    let start = fixture.session().last_event_ms;
    let ten_minutes = 10 * 60_000;
    fixture.engine.check_stalls(start + ten_minutes);
    assert_eq!(fixture.state(), SessionState::Stalled);
    fixture
        .engine
        .acknowledge(SESSION, start + 2 * ten_minutes)
        .unwrap();
    assert_eq!(fixture.state(), SessionState::Working);
    fixture.engine.check_stalls(start + 3 * ten_minutes - 1);
    assert_eq!(fixture.state(), SessionState::Working);
    fixture.engine.check_stalls(start + 3 * ten_minutes);
    assert_eq!(fixture.state(), SessionState::Stalled);
}

#[test]
fn activity_lines_and_timeline_labels_are_redacted() {
    let mut fixture = fixture();
    fixture.send(&custom(
        "PreToolUse",
        json!({"tool_name": "Bash", "tool_input": {"command": "API_KEY=abc123 npm run deploy"}, "tool_use_id": "t1"}),
    ));
    assert_eq!(
        fixture.session().activity.as_deref(),
        Some("Running API_KEY=*** npm run deploy")
    );
    let detail = fixture
        .engine
        .control(Request::Detail {
            session_id: SESSION.to_string(),
        })
        .detail
        .unwrap();
    assert!(
        detail
            .timeline
            .iter()
            .all(|entry| !entry.label.contains("abc123"))
    );
    assert_eq!(
        detail.timeline.last().map(|entry| entry.label.as_str()),
        Some("Running API_KEY=*** npm run deploy")
    );
}

#[test]
fn the_host_identity_comes_from_the_first_event_that_carries_one_and_persists() {
    let mut fixture = fixture();
    fixture.send_with_env(SESSION_START, &[("CLAUDE_PID", "1")]);
    assert_eq!(fixture.session().host, None);
    fixture.send_with_env(
        USER_PROMPT_SUBMIT,
        &[("WT_SESSION", "first"), ("TERM_PROGRAM", "x")],
    );
    fixture.send_with_env(STOP, &[("WT_SESSION", "second")]);
    let host = fixture.session().host.unwrap();
    assert_eq!(host.wt_session.as_deref(), Some("first"));
    assert_eq!(host.term_program.as_deref(), Some("x"));
    let restored = Engine::load_sessions(&fixture.engine.persisted_state().unwrap()).unwrap();
    assert_eq!(restored[0].host, Some(host));
    let legacy = serde_json::to_value(fixture.session()).unwrap();
    let mut legacy = legacy.as_object().unwrap().clone();
    legacy.remove("host");
    let legacy: Session = serde_json::from_value(Value::Object(legacy)).unwrap();
    assert_eq!(legacy.host, None);
}

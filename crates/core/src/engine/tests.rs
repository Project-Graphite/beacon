use super::*;
use crate::account::UsageSource;
use crate::config::LogLevel;
use crate::progress::{CurrentStep, StepStatus, Task};
use crate::prompts::{self, PromptStatus};
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

fn identical_tool_use(event: &str, tool_use_id: &str) -> Vec<u8> {
    custom(
        event,
        json!({"tool_name": "Bash", "tool_input": {"command": "npm test"}, "tool_use_id": tool_use_id}),
    )
}

#[test]
fn identical_parallel_tool_calls_pair_with_their_permission_requests_in_order() {
    let mut fixture = fixture();
    fixture.send(&identical_tool_use("PreToolUse", "toolu_first"));
    fixture.send(&identical_tool_use("PreToolUse", "toolu_second"));
    let request = custom(
        "PermissionRequest",
        json!({"tool_name": "Bash", "tool_input": {"command": "npm test"}}),
    );
    let (first, _first_receiver) = fixture.ask(&request);
    let (second, _second_receiver) = fixture.ask(&request);
    let pending = fixture.engine.snapshot().pending;
    assert_eq!(pending[0].id, first);
    assert_eq!(pending[0].tool_use_id.as_deref(), Some("toolu_first"));
    assert_eq!(pending[1].id, second);
    assert_eq!(pending[1].tool_use_id.as_deref(), Some("toolu_second"));
}

#[test]
fn a_finished_tool_call_cannot_be_claimed_by_a_later_permission_request() {
    let mut fixture = fixture();
    fixture.send(&identical_tool_use("PreToolUse", "toolu_done"));
    fixture.send(&identical_tool_use("PostToolUse", "toolu_done"));
    let (_id, _receiver) = fixture.ask(&custom(
        "PermissionRequest",
        json!({"tool_name": "Bash", "tool_input": {"command": "npm test"}}),
    ));
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
fn a_question_refuses_every_allow_but_still_accepts_a_deny() {
    let mut fixture = fixture();
    let question = custom(
        "PermissionRequest",
        json!({"tool_name": "AskUserQuestion", "tool_input": {"questions": [{"question": "Which?"}]}}),
    );
    let (id, mut receiver) = fixture.ask(&question);
    for decision in [
        Decision::Allow,
        Decision::AllowWithInput { input: json!({}) },
        Decision::AllowAlways {
            permissions: Vec::new(),
        },
    ] {
        assert!(fixture.engine.decide(id, decision).is_err());
        assert_eq!(fixture.engine.snapshot().pending.len(), 1);
    }
    assert!(receiver.try_recv().is_err());
    fixture
        .engine
        .decide(id, Decision::Deny { message: None })
        .unwrap();
    assert!(receiver.try_recv().is_ok());
}

#[test]
fn decision_ids_start_from_the_clock_so_they_never_repeat_across_restarts() {
    let before = now_ms();
    let mut fixture = fixture();
    let (id, _receiver) = fixture.ask(PERMISSION_REQUEST);
    assert!(id >= before);
    assert!(id < 1 << 53);
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
fn an_elicitation_names_its_server_and_message_until_the_session_moves_on() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    fixture.send(&custom(
        "Elicitation",
        json!({"mcp_server_name": "github", "message": "Which repository?"}),
    ));
    assert_eq!(
        fixture.session().mcp_request,
        Some(McpRequest {
            server: "github".to_string(),
            message: "Which repository?".to_string()
        })
    );
    fixture.send(PRE_TOOL_USE);
    assert!(fixture.session().mcp_request.is_some());
    fixture.send(&custom("ElicitationResult", json!({})));
    assert_eq!(fixture.state(), SessionState::Working);
    assert_eq!(fixture.session().mcp_request, None);
    fixture.send(&custom("Elicitation", json!({})));
    assert_eq!(
        fixture.session().mcp_request,
        Some(McpRequest {
            server: String::new(),
            message: String::new()
        })
    );
    fixture.send(STOP);
    assert_eq!(fixture.session().mcp_request, None);
}

#[test]
fn pending_decisions_carry_the_request_context_and_expiry() {
    let mut fixture = fixture();
    let (_id, _receiver) = fixture.ask(&custom(
        "PermissionRequest",
        json!({"tool_name": "Bash", "tool_input": {"command": "ls"}, "permission_mode": "acceptEdits", "agent_id": "agent-1", "agent_type": "Explore"}),
    ));
    let pending = &fixture.engine.snapshot().pending[0];
    assert_eq!(pending.cwd.as_deref(), Some(r"C:\nowhere"));
    assert_eq!(pending.permission_mode.as_deref(), Some("acceptEdits"));
    assert_eq!(pending.agent_id.as_deref(), Some("agent-1"));
    assert_eq!(pending.agent_type.as_deref(), Some("Explore"));
    assert_eq!(pending.expires_ms, pending.created_ms + 3_600_000);
    assert_eq!(pending.plan, None);
    assert!(fixture.engine.take_plans_wanted().is_empty());
    assert_eq!(
        fixture.session().permission_mode.as_deref(),
        Some("acceptEdits")
    );
    let (_id, _receiver) = fixture.ask(PERMISSION_REQUEST);
    let plain = &fixture.engine.snapshot().pending[1];
    assert_eq!(plain.agent_id, None);
    assert_eq!(plain.agent_type, None);
    assert_eq!(plain.permission_mode.as_deref(), Some("default"));
}

#[test]
fn older_pending_decisions_without_the_new_fields_still_parse() {
    let pending: PendingDecision = serde_json::from_value(json!({
        "id": 1, "session_id": "s", "agent_id": null, "tool_name": "Bash", "tool_input": {},
        "permission_suggestions": null, "kind": "tool", "created_ms": 5, "tool_use_id": null
    }))
    .unwrap();
    assert_eq!(
        (
            pending.cwd,
            pending.permission_mode,
            pending.agent_type,
            pending.expires_ms,
            pending.plan
        ),
        (None, None, None, 0, None)
    );
}

#[test]
fn a_plan_comes_from_the_input_or_is_looked_up_in_the_transcript() {
    let mut fixture = fixture();
    let (with_text, _receiver) = fixture.ask(&custom(
        "PermissionRequest",
        json!({"tool_name": "ExitPlanMode", "tool_input": {"plan": "# Inline"}}),
    ));
    assert_eq!(
        fixture.engine.snapshot().pending[0].plan.as_deref(),
        Some("# Inline")
    );
    assert!(fixture.engine.take_plans_wanted().is_empty());
    assert_eq!(fixture.engine.plan_source(with_text), None);
    let transcript = r"C:\nowhere\plan.jsonl";
    let (empty, _receiver) = fixture.ask(&custom(
        "PermissionRequest",
        json!({"tool_name": "ExitPlanMode", "tool_input": {"plan": ""}, "transcript_path": transcript}),
    ));
    assert_eq!(fixture.engine.take_plans_wanted(), [empty]);
    assert_eq!(
        fixture.engine.plan_source(empty),
        Some((transcript.to_string(), None))
    );
    fixture.send(&custom(
        "PreToolUse",
        json!({"tool_name": "ExitPlanMode", "tool_input": {"plan": ""}, "tool_use_id": "toolu_plan"}),
    ));
    assert_eq!(
        fixture.engine.plan_source(empty),
        Some((transcript.to_string(), Some("toolu_plan".to_string())))
    );
    fixture
        .engine
        .set_plan(empty, "# From the transcript".to_string());
    assert_eq!(
        fixture.engine.snapshot().pending[1].plan.as_deref(),
        Some("# From the transcript")
    );
    assert_eq!(fixture.engine.plan_source(empty), None);
    fixture.engine.decide(empty, Decision::Allow).unwrap();
    fixture.engine.set_plan(empty, "late".to_string());
    assert_eq!(fixture.engine.plan_source(empty), None);
    assert_eq!(fixture.engine.snapshot().pending.len(), 1);
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
    fixture.engine.reconcile(
        vec![
            listed(SESSION, None, "idle"),
            listed("new-busy", Some(own_pid), "busy"),
            listed("new-waiting", None, "waiting"),
        ],
        now_ms(),
    );
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
    fixture.engine.reconcile(Vec::new(), now_ms());
    assert_eq!(
        fixture.engine.sessions["new-waiting"].state,
        SessionState::NeedsPermission
    );
    assert_eq!(fixture.engine.sessions[SESSION].state, SessionState::Ended);
}

#[test]
fn a_listed_status_older_than_the_last_hook_event_is_ignored() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    let last_event = fixture.session().last_event_ms;
    fixture
        .engine
        .reconcile(vec![listed(SESSION, None, "idle")], last_event - 1);
    assert_eq!(fixture.state(), SessionState::Working);
    fixture
        .engine
        .reconcile(vec![listed(SESSION, None, "idle")], last_event);
    assert_eq!(fixture.state(), SessionState::Idle);
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
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let exit = loop {
        match fixture.exits.try_recv() {
            Err(_) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10))
            }
            received => break received.unwrap(),
        }
    };
    assert_eq!(exit, (SESSION.to_string(), pid));
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
            .reconcile(vec![listed(SESSION, None, "idle")], now_ms());
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
        .reconcile(vec![listed(SESSION, None, "busy")], now_ms());
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
fn ended_sessions_older_than_two_hours_are_pruned_at_load() {
    let kept = 2 * 60 * 60 * 1000;
    let now = now_ms();
    let fixture = fixture_with(
        Config::default(),
        vec![
            persisted("ended-old", SessionState::Ended, now - kept - 60_000),
            persisted("ended-recent", SessionState::Ended, now - kept + 60_000),
            persisted("idle-old", SessionState::Idle, now - 3 * kept),
        ],
    );
    assert_eq!(
        session_ids(&fixture.engine),
        ["ended-recent".to_string(), "idle-old".to_string()]
    );
}

#[test]
fn ended_sessions_are_pruned_by_the_periodic_check() {
    let kept = 2 * 60 * 60 * 1000;
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    fixture.send(SESSION_END);
    let ended_at = fixture.session().last_event_ms;
    let mut changes = fixture.engine.watch_changes();
    changes.borrow_and_update();
    fixture.engine.check_stalls(ended_at + kept);
    assert_eq!(session_ids(&fixture.engine), [SESSION.to_string()]);
    assert!(!changes.has_changed().unwrap());
    fixture.engine.check_stalls(ended_at + kept + 1);
    assert!(session_ids(&fixture.engine).is_empty());
    assert!(changes.has_changed().unwrap());
    let persisted = fixture.engine.persisted_state().unwrap();
    assert!(Engine::load_sessions(&persisted).unwrap().is_empty());
}

#[test]
fn ended_sessions_are_kept_for_two_hours_from_when_they_ended() {
    let kept = 2 * 60 * 60 * 1000;
    let mut fixture = fixture();
    fixture.send_with_env(USER_PROMPT_SUBMIT, &[("CLAUDE_PID", "4242")]);
    fixture.send_with_env(
        &custom("UserPromptSubmit", json!({"session_id": "other"})),
        &[("CLAUDE_PID", "4343")],
    );
    for session in fixture.engine.sessions.values_mut() {
        session.last_event_ms -= 3 * kept;
    }
    fixture.engine.process_exited("other", 4343);
    fixture.engine.process_exited(SESSION, 4242);
    let ended_at = fixture.session().ended_ms.unwrap();
    let first_ended_at = fixture.engine.sessions["other"].ended_ms.unwrap();
    fixture.engine.check_stalls(first_ended_at + kept);
    assert_eq!(
        fixture
            .engine
            .ended()
            .iter()
            .map(|session| session.session_id.clone())
            .collect::<Vec<_>>(),
        [SESSION.to_string(), "other".to_string()]
    );
    fixture.engine.check_stalls(ended_at + kept + 1);
    assert!(session_ids(&fixture.engine).is_empty());
}

#[test]
fn at_most_twenty_ended_sessions_are_kept_and_five_are_in_the_snapshot() {
    let now = now_ms();
    let ended = (0..25u64).map(|index| {
        persisted(
            &format!("ended-{index:02}"),
            SessionState::Ended,
            now - 60_000 + index,
        )
    });
    let mut fixture = fixture_with(
        Config::default(),
        ended
            .chain([persisted("idle", SessionState::Idle, now - 120_000)])
            .collect(),
    );
    let newest: Vec<String> = (5..25)
        .rev()
        .map(|index| format!("ended-{index:02}"))
        .collect();
    let ended = fixture.engine.ended();
    assert_eq!(
        ended
            .iter()
            .map(|session| session.session_id.clone())
            .collect::<Vec<_>>(),
        newest
    );
    let snapshot = fixture.engine.snapshot();
    assert_eq!(snapshot.ended_count, 20);
    let mut shown: Vec<String> = snapshot
        .sessions
        .iter()
        .map(|session| session.session_id.clone())
        .collect();
    shown.sort();
    let mut expected: Vec<String> = newest[..5].to_vec();
    expected.push("idle".to_string());
    expected.sort();
    assert_eq!(shown, expected);
    let reply = fixture.engine.control(Request::Ended);
    assert!(reply.ok);
    assert_eq!(reply.ended.unwrap(), ended);
}

#[test]
fn the_ended_count_leaves_out_dismissed_sessions() {
    let now = now_ms();
    let fixture = fixture_with(
        Config::default(),
        vec![
            persisted("ended-kept", SessionState::Ended, now - 2_000),
            Session {
                dismissed: true,
                ..persisted("ended-dismissed", SessionState::Ended, now - 1_000)
            },
        ],
    );
    assert_eq!(fixture.engine.snapshot().ended_count, 1);
    assert_eq!(fixture.engine.ended().len(), 2);
}

#[test]
fn what_needs_the_user_names_pending_decisions_and_alerting_sessions() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    fixture.send(PRE_TOOL_USE);
    assert!(fixture.engine.needs_user().is_empty());
    let (id, _receiver) = fixture.ask(PERMISSION_REQUEST);
    assert_eq!(
        fixture.engine.needs_user(),
        BTreeSet::from([format!("decision {id}"), format!("session {SESSION}")])
    );
    fixture.engine.decide(id, Decision::Allow).unwrap();
    fixture.send(POST_TOOL_USE);
    assert!(fixture.engine.needs_user().is_empty());
    fixture.send(STOP);
    assert_eq!(
        fixture.engine.needs_user(),
        BTreeSet::from([format!("session {SESSION}")])
    );
}

#[test]
fn a_reconcile_pass_is_wanted_for_an_unknown_session_a_gone_process_or_on_request() {
    let mut fixture = fixture();
    assert!(!fixture.engine.take_reconcile_wanted());
    fixture.send_with_env(USER_PROMPT_SUBMIT, &[("CLAUDE_PID", "4242")]);
    assert!(fixture.engine.take_reconcile_wanted());
    fixture.send(PRE_TOOL_USE);
    assert!(!fixture.engine.take_reconcile_wanted());
    fixture.engine.process_exited(SESSION, 4243);
    assert!(!fixture.engine.take_reconcile_wanted());
    fixture.engine.process_exited(SESSION, 4242);
    assert!(fixture.engine.take_reconcile_wanted());
    assert!(fixture.engine.control(Request::Reconcile).ok);
    assert!(fixture.engine.take_reconcile_wanted());
    assert!(!fixture.engine.take_reconcile_wanted());
}

#[test]
fn snapshots_leave_task_lists_to_the_detail() {
    let mut fixture = fixture();
    fixture.send(&task_event("TaskCreated", "1", "Write the parser"));
    assert_eq!(fixture.session().tasks.len(), 1);
    let shown = &fixture.engine.snapshot().sessions[0];
    assert!(shown.tasks.is_empty());
    assert_eq!(shown.tasks_created, 1);
    assert_eq!(shown.task_current.as_deref(), Some("Write the parser"));
    assert_eq!(fixture.engine.detail(SESSION).unwrap().tasks.len(), 1);
}

#[test]
fn an_ended_session_is_slim_in_the_snapshot_and_whole_in_the_ended_list() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    fixture.send(&custom(
        "Stop",
        json!({"last_assistant_message": "All done"}),
    ));
    fixture.send(&task_event("TaskCreated", "1", "Write the parser"));
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
    fixture.send(SESSION_END);
    let slim = &fixture.engine.snapshot().sessions[0];
    assert_eq!(slim.state, SessionState::Ended);
    assert_eq!(slim.last_message_preview, None);
    assert_eq!(slim.usage, Some(usage));
    assert!(slim.tasks.is_empty());
    let whole = &fixture.engine.ended()[0];
    assert_eq!(whole.last_message_preview.as_deref(), Some("All done"));
}

#[test]
fn a_newly_ended_session_pushes_out_the_oldest_beyond_twenty() {
    let now = now_ms();
    let mut fixture = fixture_with(
        Config::default(),
        (0..20u64)
            .map(|index| {
                persisted(
                    &format!("ended-{index:02}"),
                    SessionState::Ended,
                    now - 60_000 + index,
                )
            })
            .collect(),
    );
    fixture.send(USER_PROMPT_SUBMIT);
    fixture.send(SESSION_END);
    let ended = fixture.engine.ended();
    assert_eq!(ended.len(), 20);
    assert_eq!(ended[0].session_id, SESSION);
    assert!(!fixture.engine.has_session("ended-00"));
    assert!(fixture.engine.has_session("ended-01"));
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
    assert!(timeline.iter().all(|entry| entry.label == "Task added"));
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
    fixture.engine.reconcile(
        vec![listed("listed", Some(std::process::id()), "idle")],
        now_ms(),
    );
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
fn set_config_applies_to_the_running_engine_and_its_snapshot() {
    let mut fixture = fixture();
    let home = fixture.home.0.clone();
    assert_eq!(
        fixture.engine.set_config(&home, Some(0), None, None),
        Err("stall_minutes must be 1 to 240, not 0".to_string())
    );
    assert!(!home.join("config.json").exists());
    let version = *fixture.engine.watch_changes().borrow();
    fixture
        .engine
        .set_config(&home, Some(1), Some(true), None)
        .unwrap();
    assert!(*fixture.engine.watch_changes().borrow() > version);
    assert_eq!(
        fixture.engine.snapshot().config,
        crate::config::LiveConfig {
            stall_minutes: 1,
            answer_unattended: true,
            clear_finished_minutes: 10
        }
    );
    assert_eq!(
        Config::load(&home).unwrap().live(),
        fixture.engine.snapshot().config
    );
    fixture.send(&custom("UserPromptSubmit", json!({})));
    let start = fixture.session().last_event_ms;
    fixture.engine.check_stalls(start + 60_000 - 1);
    assert_eq!(fixture.state(), SessionState::Working);
    fixture.engine.check_stalls(start + 60_000);
    assert_eq!(fixture.state(), SessionState::Stalled);
    let outcome =
        fixture.send_with_env(PERMISSION_REQUEST, &[("CLAUDE_CODE_SESSION_ATTENDED", "0")]);
    assert!(matches!(outcome, HookOutcome::Wait(..)));
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

impl Fixture {
    fn queue(&mut self, text: &str) -> Reply {
        self.engine.control(Request::QueuePrompt {
            session_id: SESSION.to_string(),
            text: text.to_string(),
        })
    }

    fn prompt_statuses(&self) -> Vec<(PromptStatus, Option<String>)> {
        self.session()
            .prompts
            .iter()
            .map(|prompt| (prompt.status, prompt.reason.clone()))
            .collect()
    }
}

fn stdout(outcome: HookOutcome) -> String {
    match outcome {
        HookOutcome::Reply(HookReply::Stdout(bytes)) => String::from_utf8(bytes).unwrap(),
        _ => panic!("expected stdout"),
    }
}

#[test]
fn prompts_queue_only_while_a_turn_runs_and_within_the_limits() {
    let mut fixture = fixture();
    fixture.send(SESSION_START);
    fixture.send(STOP);
    let idle = fixture.queue("go on").error.unwrap();
    assert!(idle.contains("is not running a turn"), "{idle}");
    assert!(idle.contains("beacon jump"), "{idle}");
    fixture.send_with_env(
        USER_PROMPT_SUBMIT,
        &[("CLAUDE_CODE_ENTRYPOINT", "claude-vscode")],
    );
    fixture.send(STOP);
    let vscode = fixture.queue("go on").error.unwrap();
    assert!(vscode.contains("VS Code"), "{vscode}");
    fixture.send(USER_PROMPT_SUBMIT);
    assert_eq!(
        fixture.queue(" \n ").error.as_deref(),
        Some("the prompt is empty")
    );
    assert_eq!(
        fixture.queue(&"x".repeat(16_001)).error.as_deref(),
        Some("the prompt has 16001 characters; the limit is 16000")
    );
    assert!(fixture.queue(&"é".repeat(16_000)).ok);
    assert!(fixture.queue("token API_KEY=abc123 please").ok);
    let unknown = fixture.engine.control(Request::QueuePrompt {
        session_id: "nobody".to_string(),
        text: "x".to_string(),
    });
    assert_eq!(unknown.error.as_deref(), Some("no session nobody"));
    let shown = &fixture.engine.snapshot().sessions[0].prompts;
    assert_eq!(shown.len(), 2);
    assert_eq!(shown[1].preview, "token API_KEY=*** please");
    assert_eq!(shown[1].status, PromptStatus::Queued);
    assert!(shown.iter().all(|prompt| prompt.text.is_none()));
    assert!(shown[0].preview.chars().count() <= 120);
    fixture.send(SESSION_END);
    assert!(
        fixture
            .queue("late")
            .error
            .unwrap()
            .contains("has ended and takes no prompt")
    );
}

#[test]
fn a_stop_delivers_the_oldest_queued_prompt_and_keeps_the_turn_running() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    assert!(fixture.queue("Reply with \"PINEAPPLE\".").ok);
    assert!(fixture.queue("second").ok);
    assert_eq!(
        stdout(fixture.send(STOP)),
        r#"{"hookSpecificOutput":{"hookEventName":"Stop","additionalContext":"The user sent this follow-up from Beacon: Reply with \"PINEAPPLE\"."},"systemMessage":"Beacon follow-up: Reply with \"PINEAPPLE\"."}"#
    );
    assert!(
        String::from_utf8(crate::prompts::stop_output(&"x".repeat(400)))
            .unwrap()
            .contains(&format!("Beacon follow-up: {}…\"", "x".repeat(300)))
    );
    assert_eq!(fixture.state(), SessionState::Working);
    assert_eq!(fixture.session().turn_ended_ms, None);
    assert_eq!(
        fixture.prompt_statuses(),
        [
            (PromptStatus::Delivered, None),
            (PromptStatus::Queued, None)
        ]
    );
    assert_eq!(
        fixture
            .engine
            .detail(SESSION)
            .unwrap()
            .timeline
            .last()
            .unwrap()
            .label,
        "Follow-up delivered from Beacon"
    );
    fixture.send(PRE_TOOL_USE);
    assert_eq!(fixture.prompt_statuses()[0].0, PromptStatus::Acknowledged);
    assert!(stdout(fixture.send(STOP)).contains("from Beacon: second"));
    assert_eq!(
        fixture.prompt_statuses(),
        [
            (PromptStatus::Acknowledged, None),
            (PromptStatus::Delivered, None)
        ]
    );
    assert!(matches!(
        fixture.send(STOP),
        HookOutcome::Reply(HookReply::NoDecision)
    ));
    assert_eq!(fixture.state(), SessionState::Done);
    assert!(fixture.session().turn_ended_ms.is_some());
    assert_eq!(fixture.prompt_statuses()[1].0, PromptStatus::Acknowledged);
}

#[test]
fn queued_prompts_fail_when_the_session_ends_or_the_turn_is_interrupted() {
    let mut fixture = fixture();
    fixture.send_with_env(USER_PROMPT_SUBMIT, &[("CLAUDE_PID", "4242")]);
    assert!(fixture.queue("one").ok);
    fixture.engine.process_exited(SESSION, 4242);
    assert_eq!(
        fixture.prompt_statuses(),
        [(
            PromptStatus::Failed,
            Some(prompts::ENDED_REASON.to_string())
        )]
    );
    fixture.send(USER_PROMPT_SUBMIT);
    assert!(fixture.queue("two").ok);
    fixture
        .engine
        .reconcile(vec![listed(SESSION, None, "idle")], now_ms());
    assert_eq!(fixture.state(), SessionState::Idle);
    assert_eq!(
        fixture.prompt_statuses()[1],
        (
            PromptStatus::Failed,
            Some(prompts::INTERRUPTED_REASON.to_string())
        )
    );
    fixture.send(USER_PROMPT_SUBMIT);
    assert!(fixture.queue("three").ok);
    fixture.send(SESSION_END);
    assert_eq!(
        fixture.prompt_statuses()[2],
        (
            PromptStatus::Failed,
            Some(prompts::ENDED_REASON.to_string())
        )
    );
    assert!(
        fixture
            .session()
            .prompts
            .iter()
            .all(|prompt| prompt.text.is_none())
    );
}

#[test]
fn a_queued_prompt_survives_a_core_restart() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    assert!(fixture.queue("after restart").ok);
    let persisted = fixture.engine.persisted_state().unwrap();
    assert!(String::from_utf8_lossy(&persisted).contains("\"text\": \"after restart\""));
    let mut restored = fixture_with(
        Config::default(),
        Engine::load_sessions(&persisted).unwrap(),
    );
    assert!(restored.queue("next").ok);
    assert_eq!(
        restored
            .session()
            .prompts
            .iter()
            .map(|prompt| prompt.id)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert!(stdout(restored.send(STOP)).contains("from Beacon: after restart\"}"));
}

#[test]
fn only_a_queued_prompt_can_be_cancelled_and_finished_ones_are_trimmed_to_five() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    assert!(fixture.queue("keep").ok);
    assert!(fixture.queue("drop").ok);
    assert!(fixture.engine.control(Request::CancelPrompt { id: 2 }).ok);
    assert_eq!(fixture.session().prompts.len(), 1);
    assert_eq!(
        fixture
            .engine
            .control(Request::CancelPrompt { id: 2 })
            .error
            .as_deref(),
        Some("no prompt 2")
    );
    fixture.send(STOP);
    assert_eq!(
        fixture
            .engine
            .control(Request::CancelPrompt { id: 1 })
            .error
            .as_deref(),
        Some("prompt 1 is delivered and can no longer be cancelled")
    );
    for round in 0..5 {
        assert!(fixture.queue(&format!("round {round}")).ok);
        fixture.send(STOP);
    }
    assert!(fixture.queue("waiting").ok);
    let previews: Vec<String> = fixture
        .session()
        .prompts
        .iter()
        .map(|prompt| prompt.preview.clone())
        .collect();
    assert_eq!(
        previews,
        ["round 1", "round 2", "round 3", "round 4", "waiting"]
    );
}

fn task_event(event: &str, id: &str, subject: &str) -> Vec<u8> {
    custom(event, json!({"task_id": id, "task_subject": subject}))
}

fn tool_event(event: &str, tool_name: &str, tool_input: Value, extra: Value) -> Vec<u8> {
    let mut fields = json!({"tool_name": tool_name, "tool_input": tool_input});
    fields
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    custom(event, fields)
}

impl Fixture {
    fn tasks(&self) -> Vec<(String, String, bool)> {
        self.session()
            .tasks
            .into_iter()
            .map(|task| (task.id, task.subject, task.done))
            .collect()
    }

    fn steps(&self) -> Vec<(StepKind, String, StepStatus, Option<u64>)> {
        self.engine
            .detail(SESSION)
            .unwrap()
            .steps
            .into_iter()
            .map(|step| (step.kind, step.label, step.status, step.duration_ms))
            .collect()
    }

    fn dismiss(&mut self) -> Reply {
        self.engine.control(Request::Dismiss {
            session_id: SESSION.to_string(),
        })
    }

    fn undismiss(&mut self) -> Reply {
        self.engine.control(Request::Undismiss {
            session_id: SESSION.to_string(),
        })
    }
}

#[test]
fn tasks_build_an_ordered_list_that_agrees_with_the_counts() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    fixture.send(&task_event("TaskCreated", "1", "Read the code"));
    fixture.send(&task_event("TaskCreated", "2", "Write the fix"));
    fixture.send(&task_event("TaskCreated", "3", "Run the tests"));
    assert_eq!(
        fixture.session().task_current.as_deref(),
        Some("Read the code")
    );
    fixture.send(&task_event("TaskCompleted", "1", "Read the code"));
    fixture.send(&task_event("TaskCompleted", "3", "Run the tests"));
    fixture.send(&task_event("TaskCompleted", "9", "Started before Beacon"));
    let session = fixture.session();
    assert_eq!(
        fixture.tasks(),
        [
            ("1".to_string(), "Read the code".to_string(), true),
            ("2".to_string(), "Write the fix".to_string(), false),
            ("3".to_string(), "Run the tests".to_string(), true),
            ("9".to_string(), "Started before Beacon".to_string(), true),
        ]
    );
    assert_eq!((session.tasks_created, session.tasks_completed), (4, 3));
    assert_eq!(session.task_current.as_deref(), Some("Write the fix"));
    fixture.send(&task_event("TaskCompleted", "2", "Write the fix"));
    let session = fixture.session();
    assert_eq!((session.tasks_created, session.tasks_completed), (4, 4));
    assert_eq!(session.task_current, None);
    assert_eq!(fixture.engine.detail(SESSION).unwrap().tasks, session.tasks);
}

#[test]
fn task_subjects_are_redacted_and_cut_to_one_hundred_and_twenty_characters() {
    let mut fixture = fixture();
    fixture.send(&task_event(
        "TaskCreated",
        "1",
        "Rotate API_KEY=abc123 in the vault",
    ));
    fixture.send(&task_event("TaskCreated", "2", &"ж".repeat(500)));
    fixture.send(&task_event(
        "TaskCompleted",
        "1",
        "Rotate API_KEY=abc123 in the vault",
    ));
    let tasks = fixture.session().tasks;
    assert_eq!(tasks[0].subject, "Rotate API_KEY=*** in the vault");
    assert_eq!(tasks[1].subject.chars().count(), 120);
    let labels: Vec<String> = fixture
        .steps()
        .into_iter()
        .map(|(kind, label, ..)| {
            assert_eq!(kind, StepKind::Task);
            label
        })
        .collect();
    assert_eq!(labels[0], "Task added: Rotate API_KEY=*** in the vault");
    assert_eq!(labels[1].chars().count(), LABEL_CHARS);
    assert!(labels[1].starts_with("Task added: жж"));
    assert_eq!(labels[2], "Task done: Rotate API_KEY=*** in the vault");
    let timeline: Vec<String> = fixture
        .engine
        .detail(SESSION)
        .unwrap()
        .timeline
        .into_iter()
        .map(|entry| entry.label)
        .collect();
    assert_eq!(timeline[0], "Task added: Rotate API_KEY=*** in the vault");
    assert_eq!(timeline[2], "Task done: Rotate API_KEY=*** in the vault");
}

#[test]
fn a_new_prompt_keeps_the_tasks_and_a_clear_drops_them() {
    let mut fixture = fixture();
    fixture.send(&task_event("TaskCreated", "1", "First"));
    fixture.send(&task_event("TaskCompleted", "1", "First"));
    fixture.send(&task_event("TaskCreated", "2", "Second"));
    fixture.send(STOP);
    fixture.send(USER_PROMPT_SUBMIT);
    assert_eq!(fixture.session().tasks.len(), 2);
    fixture.send(&custom("SessionStart", json!({"source": "compact"})));
    fixture.send(&custom("SessionStart", json!({"source": "resume"})));
    assert_eq!(fixture.session().tasks.len(), 2);
    fixture.send(&custom("SessionStart", json!({"source": "clear"})));
    let session = fixture.session();
    assert!(session.tasks.is_empty());
    assert_eq!((session.tasks_created, session.tasks_completed), (0, 0));
    assert_eq!(session.task_current, None);
}

#[test]
fn tasks_survive_a_restart_and_old_state_still_loads() {
    let mut fixture = fixture();
    fixture.send(&task_event("TaskCreated", "1", "Keep me"));
    let persisted = fixture.engine.persisted_state().unwrap();
    let restored = fixture_with(
        Config::default(),
        Engine::load_sessions(&persisted).unwrap(),
    );
    let session = restored.session();
    assert_eq!(
        session.tasks,
        [Task {
            id: "1".to_string(),
            subject: "Keep me".to_string(),
            done: false
        }]
    );
    assert_eq!(session.task_current.as_deref(), Some("Keep me"));
    let mut legacy = serde_json::to_value(fixture.session()).unwrap();
    for field in [
        "tasks",
        "task_current",
        "step_current",
        "command_current",
        "dismissed",
    ] {
        legacy.as_object_mut().unwrap().remove(field);
    }
    let legacy: Session = serde_json::from_value(legacy).unwrap();
    assert!(legacy.tasks.is_empty());
    assert!(!legacy.dismissed);
}

#[test]
fn a_tool_call_is_one_step_paired_by_its_tool_use_id() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    fixture.send(&tool_event(
        "PreToolUse",
        "Bash",
        json!({"command": "GITHUB_TOKEN=abc123 gh pr list"}),
        json!({"tool_use_id": "t1"}),
    ));
    fixture.send(&tool_event(
        "PreToolUse",
        "Read",
        json!({"file_path": r"C:\nowhere\a.rs"}),
        json!({"tool_use_id": "t2"}),
    ));
    let session = fixture.session();
    assert_eq!(
        session.step_current,
        Some(CurrentStep {
            kind: StepKind::Read,
            label: "Reading a.rs".to_string()
        })
    );
    assert_eq!(
        session.command_current.as_deref(),
        Some("GITHUB_TOKEN=*** gh pr list")
    );
    fixture.send(&tool_event(
        "PostToolUse",
        "Read",
        json!({"file_path": r"C:\nowhere\a.rs"}),
        json!({"tool_use_id": "t2", "duration_ms": 12}),
    ));
    let session = fixture.session();
    assert_eq!(
        session.step_current,
        Some(CurrentStep {
            kind: StepKind::Run,
            label: "Running GITHUB_TOKEN=*** gh pr list".to_string()
        })
    );
    assert_eq!(
        session.command_current.as_deref(),
        Some("GITHUB_TOKEN=*** gh pr list")
    );
    fixture.send(&tool_event(
        "PostToolUseFailure",
        "Bash",
        json!({"command": "GITHUB_TOKEN=abc123 gh pr list"}),
        json!({"tool_use_id": "t1"}),
    ));
    let session = fixture.session();
    assert_eq!(session.step_current, None);
    assert_eq!(session.command_current, None);
    let steps = fixture.steps();
    assert_eq!(steps.len(), 3);
    assert_eq!(
        steps[0],
        (
            StepKind::Prompt,
            "Prompt submitted".to_string(),
            StepStatus::Done,
            None
        )
    );
    assert_eq!(
        (steps[1].0, steps[1].1.as_str(), steps[1].2),
        (
            StepKind::Run,
            "Running GITHUB_TOKEN=*** gh pr list",
            StepStatus::Failed
        )
    );
    assert!(steps[1].3.is_some());
    assert_eq!(
        steps[2],
        (
            StepKind::Read,
            "Reading a.rs".to_string(),
            StepStatus::Done,
            Some(12)
        )
    );
    let ids: Vec<u64> = fixture
        .engine
        .detail(SESSION)
        .unwrap()
        .steps
        .iter()
        .map(|step| step.id)
        .collect();
    assert_eq!(ids, [1, 2, 3]);
}

#[test]
fn a_finish_that_arrives_before_its_start_makes_one_finished_step() {
    let mut fixture = fixture();
    let grep = |event| {
        tool_event(
            event,
            "Grep",
            json!({"pattern": "x"}),
            json!({"tool_use_id": "t3", "duration_ms": 7}),
        )
    };
    fixture.send(&grep("PostToolUse"));
    fixture.send(&grep("PreToolUse"));
    assert_eq!(
        fixture.steps(),
        [(
            StepKind::Search,
            "Searching".to_string(),
            StepStatus::Done,
            Some(7)
        )]
    );
    assert_eq!(fixture.session().step_current, None);
}

#[test]
fn running_steps_stop_when_the_turn_ends() {
    for ending in [
        custom("Stop", json!({})),
        custom("StopFailure", json!({"error": "rate_limit"})),
        custom("SessionEnd", json!({"reason": "logout"})),
    ] {
        let mut fixture = fixture();
        fixture.send(USER_PROMPT_SUBMIT);
        fixture.send(PRE_TOOL_USE);
        assert!(fixture.session().command_current.is_some());
        fixture.send(&ending);
        let session = fixture.session();
        assert_eq!(session.step_current, None);
        assert_eq!(session.command_current, None);
        assert_eq!(fixture.steps()[1].2, StepStatus::Stopped);
    }
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    fixture.send(PRE_TOOL_USE);
    let (id, _receiver) = fixture.ask(PERMISSION_REQUEST);
    fixture
        .engine
        .decide(id, Decision::DenyAndStop { message: None })
        .unwrap();
    assert_eq!(fixture.state(), SessionState::Idle);
    assert_eq!(fixture.steps()[1].2, StepStatus::Stopped);
    assert_eq!(fixture.session().step_current, None);
}

#[test]
fn a_stop_that_delivers_a_prompt_keeps_the_step_running() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    fixture.send(PRE_TOOL_USE);
    assert!(fixture.queue("go on").ok);
    fixture.send(STOP);
    let steps = fixture.steps();
    assert_eq!(steps[1].2, StepStatus::Running);
    assert_eq!(
        (steps[2].0, steps[2].1.as_str()),
        (StepKind::Prompt, "Follow-up delivered from Beacon")
    );
    assert!(fixture.session().step_current.is_some());
}

#[test]
fn the_running_command_is_cut_to_two_thousand_characters() {
    let mut fixture = fixture();
    fixture.send(&tool_event(
        "PreToolUse",
        "PowerShell",
        json!({"command": "é".repeat(5_000)}),
        json!({"tool_use_id": "t5"}),
    ));
    assert_eq!(
        fixture.session().command_current.unwrap().chars().count(),
        2_000
    );
    fixture.send(&tool_event(
        "PreToolUse",
        "Read",
        json!({"command": "not a command"}),
        json!({"tool_use_id": "t6"}),
    ));
    fixture.send(&tool_event(
        "PostToolUse",
        "PowerShell",
        json!({}),
        json!({"tool_use_id": "t5"}),
    ));
    assert_eq!(fixture.session().command_current, None);
}

#[test]
fn permissions_and_turn_ends_become_steps_and_the_list_keeps_sixty() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    let (id, _receiver) = fixture.ask(PERMISSION_REQUEST);
    fixture.engine.decide(id, Decision::Allow).unwrap();
    fixture.send(STOP);
    fixture.send(&custom("StopFailure", json!({"error": "overloaded"})));
    let kinds: Vec<(StepKind, String)> = fixture
        .steps()
        .into_iter()
        .map(|(kind, label, ..)| (kind, label))
        .collect();
    assert_eq!(
        kinds,
        [
            (StepKind::Prompt, "Prompt submitted".to_string()),
            (
                StepKind::Permission,
                "Permission requested: Bash".to_string()
            ),
            (StepKind::Permission, "Allowed".to_string()),
            (StepKind::Finish, "Turn finished".to_string()),
            (StepKind::Error, "API error: overloaded".to_string()),
        ]
    );
    for index in 0..100 {
        fixture.send(&tool_event(
            "PreToolUse",
            "Glob",
            json!({}),
            json!({"tool_use_id": format!("g{index}")}),
        ));
    }
    let steps = fixture.engine.detail(SESSION).unwrap().steps;
    assert_eq!(steps.len(), 60);
    assert_eq!(steps.last().unwrap().id, 105);
}

#[test]
fn a_dismissed_session_is_acknowledged_and_never_alerting() {
    for (ending, before) in [
        (custom("Stop", json!({})), SessionState::Done),
        (
            custom("StopFailure", json!({"error": "rate_limit"})),
            SessionState::Error,
        ),
    ] {
        let mut fixture = fixture();
        fixture.send(USER_PROMPT_SUBMIT);
        fixture.send(&ending);
        assert_eq!(fixture.state(), before);
        assert!(fixture.session().alerting);
        assert!(fixture.dismiss().ok);
        let session = fixture.session();
        assert_eq!(session.state, SessionState::Idle);
        assert!(session.dismissed);
        assert!(!session.alerting);
    }
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    let start = fixture.session().last_event_ms;
    fixture.engine.check_stalls(start + 10 * 60_000);
    assert_eq!(fixture.state(), SessionState::Stalled);
    assert!(fixture.dismiss().ok);
    assert_eq!(fixture.state(), SessionState::Working);
    fixture.engine.check_stalls(start + 30 * 60_000);
    let session = fixture.session();
    assert_eq!(session.state, SessionState::Stalled);
    assert!(session.attention);
    assert!(!session.alerting);
    assert!(fixture.engine.snapshot().sessions[0].dismissed);
}

#[test]
fn the_next_prompt_request_elicitation_or_start_brings_a_session_back() {
    let returning = [
        custom("UserPromptSubmit", json!({})),
        custom(
            "Elicitation",
            json!({"mcp_server_name": "m", "message": "?"}),
        ),
        custom("SessionStart", json!({"source": "resume"})),
        PERMISSION_REQUEST.to_vec(),
    ];
    for event in returning {
        let mut fixture = fixture();
        fixture.send(STOP);
        assert!(fixture.dismiss().ok);
        for quiet in [
            custom("Notification", json!({"notification_type": "idle_prompt"})),
            custom("PreToolUse", json!({"tool_name": "Read"})),
            custom("Stop", json!({})),
        ] {
            fixture.send(&quiet);
            assert!(fixture.session().dismissed);
            assert!(!fixture.session().alerting);
        }
        fixture.send(&event);
        assert!(!fixture.session().dismissed, "{}", event_name(&event));
    }
}

#[test]
fn a_session_waiting_for_a_decision_cannot_be_dismissed() {
    let mut fixture = fixture();
    let (id, _receiver) = fixture.ask(PERMISSION_REQUEST);
    assert_eq!(
        fixture.dismiss().error.as_deref(),
        Some(
            "session 4c2c3535-edec-4bb7-90b7-38049912db27 is waiting for a decision; answer it before hiding the session"
        )
    );
    assert!(!fixture.session().dismissed);
    fixture.engine.decide(id, Decision::Allow).unwrap();
    assert!(fixture.dismiss().ok);
    assert_eq!(
        fixture.engine.control(Request::Dismiss {
            session_id: "missing".to_string()
        }),
        Reply::failed("no session missing")
    );
}

#[test]
fn undismissing_returns_a_hidden_session_to_the_list() {
    let mut fixture = fixture();
    fixture.send(STOP);
    assert!(fixture.dismiss().ok);
    assert!(fixture.session().dismissed);
    assert!(fixture.undismiss().ok);
    let session = fixture.session();
    assert!(!session.dismissed);
    assert_eq!(session.state, SessionState::Idle);
    assert!(fixture.undismiss().ok);
    assert_eq!(
        fixture.engine.control(Request::Undismiss {
            session_id: "missing".to_string()
        }),
        Reply::failed("no session missing")
    );
}

#[test]
fn dismissal_survives_a_restart() {
    let mut fixture = fixture();
    fixture.send(STOP);
    assert!(fixture.dismiss().ok);
    let persisted = fixture.engine.persisted_state().unwrap();
    let restored = fixture_with(
        Config::default(),
        Engine::load_sessions(&persisted).unwrap(),
    );
    let session = restored.session();
    assert!(session.dismissed);
    assert!(!session.alerting);
}

impl Fixture {
    fn wait(&mut self) -> (u64, oneshot::Receiver<HookReply>) {
        match self.engine.handle_hook(&meta("StopWaiter", &[]), STOP) {
            HookOutcome::Waiter(id, receiver) => (id, receiver),
            _ => panic!("expected a stop waiter"),
        }
    }

    fn can_wake(&self) -> bool {
        self.engine.snapshot().sessions[0].can_wake
    }
}

const RELEASED: Result<HookReply, oneshot::error::TryRecvError> = Ok(HookReply::NoDecision);

#[test]
fn a_stop_waiter_is_not_an_event_and_is_replaced_by_the_next_one() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    let (_, mut first) = fixture.wait();
    assert_eq!(fixture.session().event_count, 1);
    assert_eq!(fixture.state(), SessionState::Working);
    assert!(!fixture.can_wake());
    fixture.send(STOP);
    assert!(fixture.can_wake());
    assert_eq!(first.try_recv(), Err(oneshot::error::TryRecvError::Empty));
    let (_, mut second) = fixture.wait();
    assert_eq!(first.try_recv(), RELEASED);
    assert!(fixture.can_wake());
    fixture.send(USER_PROMPT_SUBMIT);
    assert_eq!(second.try_recv(), RELEASED);
    assert!(!fixture.can_wake());
    fixture.send(STOP);
    assert!(matches!(
        fixture.engine.handle_hook(
            &meta("StopWaiter", &[("CLAUDE_CODE_SESSION_ATTENDED", "0")]),
            STOP
        ),
        HookOutcome::Reply(HookReply::NoDecision)
    ));
    assert!(!fixture.can_wake());
}

#[test]
fn a_stop_waiter_is_released_when_its_session_ends() {
    let mut fixture = fixture();
    fixture.send_with_env(USER_PROMPT_SUBMIT, &[("CLAUDE_PID", "4242")]);
    fixture.send(STOP);
    let (_, mut ended) = fixture.wait();
    fixture.send(SESSION_END);
    assert_eq!(ended.try_recv(), RELEASED);
    let (_, mut late) = fixture.wait();
    assert_eq!(late.try_recv(), RELEASED);
    assert!(!fixture.can_wake());
    fixture.send_with_env(SESSION_START, &[("CLAUDE_PID", "4343")]);
    fixture.send(STOP);
    let (_, mut gone) = fixture.wait();
    fixture.engine.process_exited(SESSION, 4343);
    assert_eq!(gone.try_recv(), RELEASED);
}

#[test]
fn a_dropped_stop_waiter_is_forgotten_but_a_newer_one_is_kept() {
    let mut fixture = fixture();
    fixture.send(STOP);
    let (first, _first_receiver) = fixture.wait();
    let (second, _second_receiver) = fixture.wait();
    fixture.engine.waiter_gone(first);
    assert!(fixture.can_wake());
    fixture.engine.waiter_gone(second);
    assert!(!fixture.can_wake());
    assert!(
        fixture
            .queue("go on")
            .error
            .unwrap()
            .contains("is not running a turn")
    );
}

#[test]
fn only_idle_finished_and_failed_sessions_can_be_woken() {
    let mut fixture = fixture();
    let _waiter = fixture.wait();
    let cases: [(&[u8], SessionState, bool); 4] = [
        (SESSION_START, SessionState::Starting, false),
        (STOP, SessionState::Done, true),
        (
            &custom("StopFailure", json!({"error": "rate_limit"})),
            SessionState::Error,
            true,
        ),
        (PERMISSION_REQUEST, SessionState::NeedsPermission, false),
    ];
    for (payload, state, can_wake) in cases {
        let _ = fixture.send(payload);
        assert_eq!(fixture.state(), state);
        assert_eq!(fixture.can_wake(), can_wake, "{state:?}");
    }
    fixture.send(&custom("SessionStart", json!({"source": "clear"})));
    assert_eq!(fixture.state(), SessionState::Idle);
    assert!(fixture.can_wake());
}

#[test]
fn a_prompt_to_a_wakeable_session_is_delivered_through_its_waiter() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    let (_, mut early) = fixture.wait();
    assert_eq!(
        fixture.queue("while running").prompt_path,
        Some(PromptPath::AtTurnEnd)
    );
    assert_eq!(early.try_recv(), Err(oneshot::error::TryRecvError::Empty));
    assert!(stdout(fixture.send(STOP)).contains("from Beacon: while running"));
    fixture.send(STOP);
    let (_, mut waiter) = fixture.wait();
    assert_eq!(early.try_recv(), RELEASED);
    assert_eq!(
        fixture.queue("API_KEY=abc123 \"go on\"").prompt_path,
        Some(PromptPath::Wake)
    );
    assert_eq!(
        waiter.try_recv(),
        Ok(HookReply::Rewake(
            b"The user sent this follow-up from Beacon: API_KEY=abc123 \"go on\"".to_vec()
        ))
    );
    let prompt = fixture.session().prompts.last().unwrap().clone();
    assert_eq!(prompt.status, PromptStatus::Delivered);
    assert_eq!(prompt.preview, "API_KEY=*** \"go on\"");
    assert_eq!(prompt.text, None);
    assert!(!fixture.can_wake());
    assert!(
        fixture
            .queue("again")
            .error
            .unwrap()
            .contains("is not running a turn")
    );
    fixture.send(USER_PROMPT_SUBMIT);
    assert_eq!(
        fixture.prompt_statuses().last().unwrap().0,
        PromptStatus::Acknowledged
    );
}

#[test]
fn titles_come_from_the_prompt_and_the_transcript_and_a_hook_title_wins() {
    let mut fixture = fixture();
    fixture.send(&custom(
        "UserPromptSubmit",
        json!({"prompt": "first line\nAPI_KEY=abc123 then more"}),
    ));
    assert_eq!(
        fixture.session().last_prompt.as_deref(),
        Some("first line API_KEY=*** then more")
    );
    assert_eq!(
        fixture.engine.snapshot().sessions[0].display_title,
        "first line API_KEY=*** then more"
    );
    fixture.engine.set_titles(
        SESSION,
        TranscriptTitles {
            ai_title: Some("x".repeat(200)),
            custom_title: Some("Named in VS Code".to_string()),
            last_prompt: None,
        },
    );
    let session = fixture.session();
    assert_eq!(session.ai_title.unwrap().chars().count(), 80);
    assert_eq!(session.title.as_deref(), Some("Named in VS Code"));
    assert_eq!(
        session.last_prompt.as_deref(),
        Some("first line API_KEY=*** then more")
    );
    assert_eq!(
        fixture.engine.snapshot().sessions[0].display_title,
        "Named in VS Code"
    );
    fixture.send(&custom(
        "UserPromptSubmit",
        json!({"session_title": "From the hook"}),
    ));
    fixture.engine.set_titles(
        SESSION,
        TranscriptTitles {
            custom_title: Some("Renamed".to_string()),
            last_prompt: Some("later prompt".to_string()),
            ..TranscriptTitles::default()
        },
    );
    assert_eq!(fixture.session().title.as_deref(), Some("From the hook"));
    assert_eq!(
        fixture.session().last_prompt.as_deref(),
        Some("later prompt")
    );
}

#[test]
fn a_session_without_a_transcript_is_looked_up_once_and_adopts_what_is_found() {
    let mut fixture = fixture();
    fixture.engine.reconcile(
        vec![
            listed("limited", None, "idle"),
            listed("other", None, "idle"),
        ],
        now_ms(),
    );
    assert_eq!(
        fixture.engine.take_transcripts_wanted(),
        ["limited", "other"]
    );
    fixture
        .engine
        .reconcile(vec![listed("limited", None, "idle")], now_ms());
    assert!(fixture.engine.take_transcripts_wanted().is_empty());
    fixture
        .engine
        .adopt_transcript("limited", r"C:\p\limited.jsonl".to_string());
    assert_eq!(
        fixture.engine.sessions["limited"]
            .transcript_path
            .as_deref(),
        Some(r"C:\p\limited.jsonl")
    );
    assert_eq!(
        fixture.engine.take_usage_wanted(),
        BTreeMap::from([("limited".to_string(), r"C:\p\limited.jsonl".to_string())])
    );
    fixture
        .engine
        .reconcile(vec![listed("limited", None, "idle")], now_ms());
    assert_eq!(fixture.engine.take_usage_wanted().len(), 1);
    fixture
        .engine
        .adopt_transcript("limited", r"C:\elsewhere.jsonl".to_string());
    assert_eq!(
        fixture.engine.sessions["limited"]
            .transcript_path
            .as_deref(),
        Some(r"C:\p\limited.jsonl")
    );
}

#[test]
fn a_finished_session_is_acknowledged_after_the_configured_minutes() {
    let mut fixture = fixture();
    fixture.send(USER_PROMPT_SUBMIT);
    fixture.send(STOP);
    let ended = fixture.session().turn_ended_ms.unwrap();
    fixture.engine.check_stalls(ended + 10 * 60_000 - 1);
    assert_eq!(fixture.state(), SessionState::Done);
    fixture.send(&custom(
        "Notification",
        json!({"notification_type": "idle_prompt"}),
    ));
    fixture.engine.check_stalls(ended + 10 * 60_000);
    assert_eq!(fixture.state(), SessionState::Idle);
    assert!(!fixture.session().attention);
    let mut never = fixture_with(
        Config {
            clear_finished_minutes: 0,
            ..Config::default()
        },
        Vec::new(),
    );
    never.send(USER_PROMPT_SUBMIT);
    never.send(STOP);
    never.engine.check_stalls(u64::MAX);
    assert_eq!(never.state(), SessionState::Done);
    let home = never.home.0.clone();
    never.engine.set_config(&home, None, None, Some(1)).unwrap();
    assert_eq!(never.engine.snapshot().config.clear_finished_minutes, 1);
    let ended = never.session().turn_ended_ms.unwrap();
    never.engine.check_stalls(ended + 60_000);
    assert_eq!(never.state(), SessionState::Idle);
}

#[test]
fn each_hook_event_is_logged_on_one_line_without_its_content() {
    let mut fixture = fixture();
    fixture.send_with_env(
        &custom(
            "UserPromptSubmit",
            json!({"prompt": "PROMPT-TEXT-NEVER-LOGGED"}),
        ),
        &[("CLAUDE_CODE_ENTRYPOINT", "claude-vscode")],
    );
    let _waiter = fixture.wait();
    let log = std::fs::read_to_string(fixture.home.0.join("core.log")).unwrap();
    let info: Vec<&str> = log
        .lines()
        .filter_map(|line| line.split_once(" INFO ").map(|(_, rest)| rest))
        .collect();
    assert_eq!(
        info,
        [
            "UserPromptSubmit 4c2c3535 entrypoint claude-vscode state Working",
            "StopWaiter 4c2c3535 entrypoint claude-vscode state Working",
        ]
    );
    assert!(!log.contains("NEVER-LOGGED"));
}

#[test]
fn a_session_moving_on_expires_its_pending_decisions() {
    type Move = fn(&mut Fixture);
    let moves: [(Move, SessionState); 4] = [
        (
            |fixture| {
                fixture.send(USER_PROMPT_SUBMIT);
            },
            SessionState::Working,
        ),
        (
            |fixture| {
                fixture.send(STOP);
            },
            SessionState::Done,
        ),
        (
            |fixture| {
                fixture.send(SESSION_END);
            },
            SessionState::Ended,
        ),
        (
            |fixture| fixture.engine.process_exited(SESSION, 4242),
            SessionState::Ended,
        ),
    ];
    for (moves_on, expected) in moves {
        let mut fixture = fixture();
        fixture.send_with_env(USER_PROMPT_SUBMIT, &[("CLAUDE_PID", "4242")]);
        let (id, mut receiver) = fixture.ask(PERMISSION_REQUEST);
        let (other_id, _other) = fixture.ask(PERMISSION_REQUEST);
        assert_eq!(fixture.state(), SessionState::NeedsPermission);
        moves_on(&mut fixture);
        assert_eq!(receiver.try_recv(), RELEASED);
        assert!(fixture.engine.snapshot().pending.is_empty());
        assert!(
            fixture
                .engine
                .needs_user()
                .iter()
                .all(|need| !need.starts_with("decision"))
        );
        assert_eq!(fixture.state(), expected);
        let resolved: Vec<_> = fixture
            .engine
            .snapshot()
            .resolved
            .into_iter()
            .map(|resolved| (resolved.id, resolved.resolution))
            .collect();
        assert_eq!(
            resolved,
            [(id, Resolution::Expired), (other_id, Resolution::Expired)]
        );
        assert!(fixture.engine.decide(id, Decision::Allow).is_err());
    }
}

#[test]
fn a_waiter_for_a_session_unknown_after_a_reconcile_is_released() {
    let mut fixture = fixture();
    let (_, mut unknown) = fixture.wait();
    assert!(fixture.engine.take_reconcile_wanted());
    fixture
        .engine
        .reconcile(vec![listed("other", None, "idle")], now_ms());
    assert_eq!(unknown.try_recv(), RELEASED);
    let mut known = fixture_with(Config::default(), Vec::new());
    known.send(STOP);
    known.engine.take_reconcile_wanted();
    let (_, mut waiter) = known.wait();
    assert!(!known.engine.take_reconcile_wanted());
    known
        .engine
        .reconcile(vec![listed(SESSION, None, "idle")], now_ms());
    assert_eq!(waiter.try_recv(), Err(oneshot::error::TryRecvError::Empty));
    assert!(known.can_wake());
}

fn running_session(claude_pid_created: Option<u64>) -> Session {
    Session {
        claude_pid: Some(std::process::id()),
        claude_pid_created,
        ..persisted(SESSION, SessionState::Working, now_ms())
    }
}

#[test]
fn a_hook_stores_the_creation_time_of_the_claude_process() {
    let mut fixture = fixture();
    let own_pid = std::process::id();
    fixture.send_with_env(USER_PROMPT_SUBMIT, &[("CLAUDE_PID", &own_pid.to_string())]);
    assert_eq!(fixture.session().claude_pid, Some(own_pid));
    assert_eq!(
        fixture.session().claude_pid_created,
        crate::platform::process_created(own_pid)
    );
    assert!(fixture.session().claude_pid_created.is_some());
}

#[test]
fn a_restart_drops_a_pid_that_another_process_now_holds() {
    let own_created = crate::platform::process_created(std::process::id()).unwrap();
    let reused = fixture_with(
        Config::default(),
        vec![running_session(Some(own_created - 1))],
    );
    let session = &reused.engine.sessions[SESSION];
    assert_eq!(
        (session.claude_pid, session.claude_pid_created),
        (None, None)
    );
    assert!(reused.engine.live_claude_pids().is_empty());
    assert!(reused.engine.runtime[SESSION].watched_pid.is_none());
    let same = fixture_with(Config::default(), vec![running_session(Some(own_created))]);
    assert_eq!(same.engine.live_claude_pids(), [std::process::id()]);
    assert_eq!(
        same.engine.runtime[SESSION].watched_pid,
        Some(std::process::id())
    );
}

#[test]
fn a_state_file_without_creation_times_adopts_the_current_one() {
    let persisted = serde_json::to_vec(&json!({
        "sessions": [serde_json::to_value(running_session(None)).unwrap()]
    }))
    .unwrap();
    let sessions = Engine::load_sessions(&persisted).unwrap();
    assert_eq!(sessions[0].claude_pid_created, None);
    let restored = fixture_with(Config::default(), sessions);
    assert_eq!(
        restored.engine.sessions[SESSION].claude_pid_created,
        crate::platform::process_created(std::process::id())
    );
}

#[test]
fn a_pid_with_a_changed_creation_time_no_longer_keeps_a_session_alive() {
    let own_created = crate::platform::process_created(std::process::id()).unwrap();
    let mut fixture = fixture_with(Config::default(), vec![running_session(Some(own_created))]);
    fixture
        .engine
        .sessions
        .get_mut(SESSION)
        .unwrap()
        .claude_pid_created = Some(own_created - 1);
    assert!(fixture.engine.live_claude_pids().is_empty());
    fixture.engine.reconcile(Vec::new(), now_ms());
    assert_eq!(fixture.state(), SessionState::Ended);
}

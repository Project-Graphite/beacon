use beacon_core::{
    Config, ControlClient, Core, CoreHandle, CoreOptions, Decision, ListFuture, Request,
    Resolution, SessionLister, SessionState, Snapshot,
};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

const INSTRUCTIONS_LOADED: &[u8] =
    include_bytes!("../../../fixtures/hook-payloads/InstructionsLoaded.json");
const SESSION_START: &[u8] = include_bytes!("../../../fixtures/hook-payloads/SessionStart.json");
const USER_PROMPT_SUBMIT: &[u8] =
    include_bytes!("../../../fixtures/hook-payloads/UserPromptSubmit.json");
const PRE_TOOL_USE: &[u8] = include_bytes!("../../../fixtures/hook-payloads/PreToolUse.json");
const PERMISSION_REQUEST: &[u8] =
    include_bytes!("../../../fixtures/hook-payloads/PermissionRequest.json");
const POST_TOOL_USE: &[u8] = include_bytes!("../../../fixtures/hook-payloads/PostToolUse.json");
const POST_TOOL_BATCH: &[u8] = include_bytes!("../../../fixtures/hook-payloads/PostToolBatch.json");
const STOP: &[u8] = include_bytes!("../../../fixtures/hook-payloads/Stop.json");
const SESSION_END: &[u8] = include_bytes!("../../../fixtures/hook-payloads/SessionEnd.json");
const SESSION: &str = "4c2c3535-edec-4bb7-90b7-38049912db27";
const ALLOW: &str = r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}"#;

struct NoSessions;

impl SessionLister for NoSessions {
    fn list(&self) -> ListFuture<'_> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

struct TempHome(PathBuf);

impl TempHome {
    fn new() -> TempHome {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "beacon-hook-it-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        TempHome(path)
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn start_core(home: &TempHome) -> CoreHandle {
    Core::start(CoreOptions {
        home: home.0.clone(),
        lister: Box::new(NoSessions),
        config: Config::default(),
    })
    .unwrap()
}

fn spawn_helper(home: &TempHome, event: &str, payload: &[u8]) -> Child {
    spawn_helper_attended(home, event, payload, "1")
}

fn spawn_helper_attended(home: &TempHome, event: &str, payload: &[u8], attended: &str) -> Child {
    let own_pid = std::process::id().to_string();
    let mut child = Command::new(env!("CARGO_BIN_EXE_beacon-hook"))
        .arg(event)
        .env_clear()
        .env(
            "SystemRoot",
            std::env::var_os("SystemRoot").unwrap_or_default(),
        )
        .env("BEACON_HOME", &home.0)
        .env("CLAUDE_PID", own_pid)
        .env("CLAUDE_CODE_ENTRYPOINT", "sdk-cli")
        .env("CLAUDE_CODE_SESSION_ATTENDED", attended)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(payload).unwrap();
    child
}

fn run_helper(home: &TempHome, event: &str, payload: &[u8]) -> Output {
    finished(spawn_helper(home, event, payload))
}

fn finished(child: Child) -> Output {
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn snapshot(home: &TempHome) -> Snapshot {
    ControlClient::connect(&home.0)
        .unwrap()
        .request(&Request::Snapshot)
        .unwrap()
        .snapshot
        .unwrap()
}

fn wait_for(home: &TempHome, condition: impl Fn(&Snapshot) -> bool) -> Snapshot {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = snapshot(home);
        if condition(&snapshot) {
            return snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "condition not met: {snapshot:#?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn session_reached(snapshot: &Snapshot, events: u64, state: SessionState) -> bool {
    snapshot.sessions.iter().any(|session| {
        session.session_id == SESSION && session.event_count == events && session.state == state
    })
}

fn decide(home: &TempHome, id: u64, decision: Decision) {
    let reply = ControlClient::connect(&home.0)
        .unwrap()
        .request(&Request::Decide { id, decision })
        .unwrap();
    assert!(reply.ok, "{reply:?}");
}

fn pending_id(home: &TempHome) -> u64 {
    wait_for(home, |snapshot| snapshot.pending.len() == 1).pending[0].id
}

#[test]
fn fixture_replay_through_the_real_helper() {
    let home = TempHome::new();
    let core = start_core(&home);
    let before_permission: [(&str, &[u8], SessionState); 4] = [
        (
            "InstructionsLoaded",
            INSTRUCTIONS_LOADED,
            SessionState::Starting,
        ),
        ("SessionStart", SESSION_START, SessionState::Starting),
        (
            "UserPromptSubmit",
            USER_PROMPT_SUBMIT,
            SessionState::Working,
        ),
        ("PreToolUse", PRE_TOOL_USE, SessionState::Working),
    ];
    for (count, (event, payload, state)) in (1..).zip(before_permission) {
        assert!(run_helper(&home, event, payload).stdout.is_empty());
        wait_for(&home, |snapshot| session_reached(snapshot, count, state));
    }
    let helper = spawn_helper(&home, "PermissionRequest", PERMISSION_REQUEST);
    let waiting = wait_for(&home, |snapshot| {
        session_reached(snapshot, 5, SessionState::NeedsPermission)
    });
    assert_eq!(waiting.pending.len(), 1);
    assert_eq!(
        waiting.pending[0].tool_use_id.as_deref(),
        Some("toolu_01QQmzYEo3yJo3twgzeTbi3n")
    );
    let session = &waiting.sessions[0];
    assert_eq!(session.entrypoint.as_deref(), Some("sdk-cli"));
    assert_eq!(session.claude_pid, Some(std::process::id()));
    assert_eq!(session.attended, Some(true));
    decide(&home, waiting.pending[0].id, Decision::Allow);
    assert_eq!(String::from_utf8(finished(helper).stdout).unwrap(), ALLOW);
    wait_for(&home, |snapshot| {
        session_reached(snapshot, 5, SessionState::Working) && snapshot.pending.is_empty()
    });
    let after_permission: [(&str, &[u8], SessionState); 4] = [
        ("PostToolUse", POST_TOOL_USE, SessionState::Working),
        ("PostToolBatch", POST_TOOL_BATCH, SessionState::Working),
        ("Stop", STOP, SessionState::Done),
        ("SessionEnd", SESSION_END, SessionState::Ended),
    ];
    for (count, (event, payload, state)) in (6..).zip(after_permission) {
        assert!(run_helper(&home, event, payload).stdout.is_empty());
        wait_for(&home, |snapshot| session_reached(snapshot, count, state));
    }
    let resolved = snapshot(&home).resolved;
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].resolution, Resolution::Decided);
    core.shutdown().unwrap();
}

#[test]
fn without_a_core_the_helper_exits_silently() {
    let home = TempHome::new();
    for (event, payload) in [
        ("PermissionRequest", PERMISSION_REQUEST),
        ("PreToolUse", PRE_TOOL_USE),
    ] {
        assert!(run_helper(&home, event, payload).stdout.is_empty());
    }
    let stale = format!(
        r"\\.\pipe\beacon-0000000000000000{}{}{}",
        "\n",
        "ab".repeat(32),
        "\n"
    );
    std::fs::write(home.0.join("endpoint"), stale).unwrap();
    for (event, payload) in [
        ("PermissionRequest", PERMISSION_REQUEST),
        ("PreToolUse", PRE_TOOL_USE),
    ] {
        assert!(run_helper(&home, event, payload).stdout.is_empty());
    }
    std::fs::write(home.0.join("endpoint"), "garbage").unwrap();
    assert!(run_helper(&home, "Stop", STOP).stdout.is_empty());
}

#[test]
fn a_core_shutdown_releases_a_waiting_helper() {
    let home = TempHome::new();
    let core = start_core(&home);
    let helper = spawn_helper(&home, "PermissionRequest", PERMISSION_REQUEST);
    pending_id(&home);
    core.shutdown().unwrap();
    assert!(finished(helper).stdout.is_empty());
}

#[test]
fn a_wrong_token_registers_nothing() {
    let home = TempHome::new();
    let core = start_core(&home);
    let endpoint_path = home.0.join("endpoint");
    let real = std::fs::read_to_string(&endpoint_path).unwrap();
    let endpoint = beacon_protocol::Endpoint::parse(&real).unwrap();
    let replacement = if endpoint.token.starts_with('0') {
        '1'
    } else {
        '0'
    };
    let wrong = beacon_protocol::Endpoint {
        token: format!("{replacement}{}", &endpoint.token[1..]),
        ..endpoint
    };
    std::fs::write(&endpoint_path, wrong.to_file_text()).unwrap();
    for (event, payload) in [
        ("SessionStart", SESSION_START),
        ("PermissionRequest", PERMISSION_REQUEST),
        ("BeaconDoctor", b"{}".as_slice()),
    ] {
        assert!(run_helper(&home, event, payload).stdout.is_empty());
    }
    std::fs::write(&endpoint_path, real).unwrap();
    let snapshot = snapshot(&home);
    assert!(snapshot.sessions.is_empty());
    assert!(snapshot.pending.is_empty());
    core.shutdown().unwrap();
}

#[test]
fn deny_outputs_reach_claude_code_verbatim() {
    let home = TempHome::new();
    let core = start_core(&home);
    assert!(
        run_helper(&home, "UserPromptSubmit", USER_PROMPT_SUBMIT)
            .stdout
            .is_empty()
    );
    let helper = spawn_helper(&home, "PermissionRequest", PERMISSION_REQUEST);
    decide(
        &home,
        pending_id(&home),
        Decision::Deny {
            message: Some("Denied from Beacon: do not retry".to_string()),
        },
    );
    assert_eq!(
        String::from_utf8(finished(helper).stdout).unwrap(),
        r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"Denied from Beacon: do not retry"}}}"#
    );
    wait_for(&home, |snapshot| {
        session_reached(snapshot, 2, SessionState::Working)
    });
    let helper = spawn_helper(&home, "PermissionRequest", PERMISSION_REQUEST);
    decide(
        &home,
        pending_id(&home),
        Decision::DenyAndStop {
            message: Some("Stopped from Beacon".to_string()),
        },
    );
    assert_eq!(
        String::from_utf8(finished(helper).stdout).unwrap(),
        r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"Stopped from Beacon","interrupt":true}}}"#
    );
    wait_for(&home, |snapshot| {
        session_reached(snapshot, 3, SessionState::Idle)
    });
    core.shutdown().unwrap();
}

#[test]
fn a_killed_helper_expires_its_decision() {
    let home = TempHome::new();
    let core = start_core(&home);
    assert!(
        run_helper(&home, "UserPromptSubmit", USER_PROMPT_SUBMIT)
            .stdout
            .is_empty()
    );
    let mut helper = spawn_helper(&home, "PermissionRequest", PERMISSION_REQUEST);
    let id = pending_id(&home);
    helper.kill().unwrap();
    helper.wait().unwrap();
    let snapshot = wait_for(&home, |snapshot| snapshot.pending.is_empty());
    assert_eq!(snapshot.resolved[0].id, id);
    assert_eq!(snapshot.resolved[0].resolution, Resolution::Expired);
    assert_eq!(snapshot.sessions[0].state, SessionState::Working);
    core.shutdown().unwrap();
}

#[test]
fn doctor_round_trip() {
    let home = TempHome::new();
    let core = start_core(&home);
    assert_eq!(
        run_helper(&home, "BeaconDoctor", b"").stdout,
        br#"{"beacon":"ok"}"#
    );
    assert!(snapshot(&home).sessions.is_empty());
    core.shutdown().unwrap();
}

fn silent_pipe_server(home: &TempHome) -> (tokio::runtime::Runtime, NamedPipeServer) {
    let pipe_name = format!(
        r"\\.\pipe\beacon-silent-{}",
        home.0.file_name().unwrap().to_string_lossy()
    );
    std::fs::write(
        home.0.join("endpoint"),
        beacon_protocol::Endpoint {
            pipe_name: pipe_name.clone(),
            token: "ab".repeat(32),
        }
        .to_file_text(),
    )
    .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let server = {
        let _context = runtime.enter();
        ServerOptions::new().create(&pipe_name).unwrap()
    };
    (runtime, server)
}

fn seconds_until_exit(home: &TempHome, event: &str, payload: &[u8]) -> f64 {
    let started = Instant::now();
    assert!(
        finished(spawn_helper(home, event, payload))
            .stdout
            .is_empty()
    );
    started.elapsed().as_secs_f64()
}

#[test]
fn a_core_that_never_reads_cannot_hold_the_helper() {
    let home = TempHome::new();
    let (_runtime, _server) = silent_pipe_server(&home);
    let unsendable = vec![b' '; 4 * 1024 * 1024];
    let seconds = seconds_until_exit(&home, "PreToolUse", &unsendable);
    assert!((1.9..2.5).contains(&seconds), "{seconds} s");
}

#[test]
fn a_core_that_never_replies_to_stop_releases_the_helper_after_a_second() {
    let home = TempHome::new();
    let (_runtime, _server) = silent_pipe_server(&home);
    let seconds = seconds_until_exit(&home, "Stop", STOP);
    assert!((0.95..1.5).contains(&seconds), "{seconds} s");
}

fn median_ms(home: &TempHome) -> f64 {
    let mut timings: Vec<f64> = (0..30)
        .map(|_| {
            let started = Instant::now();
            run_helper(home, "PreToolUse", PRE_TOOL_USE);
            started.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    timings.sort_by(f64::total_cmp);
    (timings[14] + timings[15]) / 2.0
}

#[test]
fn timing_of_non_waiting_events() {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let home = TempHome::new();
    let core = start_core(&home);
    let with_core = median_ms(&home);
    core.shutdown().unwrap();
    let without_core = median_ms(&TempHome::new());
    println!(
        "beacon-hook {profile} median over 30 PreToolUse runs: {with_core:.2} ms with the core, {without_core:.2} ms without"
    );
}

fn with_marker(home: &TempHome) {
    let stand_in = std::path::Path::new(env!("CARGO_BIN_EXE_beacon-hook"))
        .with_file_name("examples")
        .join("stand_in_app.exe");
    assert!(stand_in.is_file(), "missing {}", stand_in.display());
    std::fs::write(
        home.0.join("autostart"),
        stand_in.to_string_lossy().as_bytes(),
    )
    .unwrap();
}

fn quiet_run(home: &TempHome, event: &str, payload: &[u8], attended: &str) {
    assert!(
        finished(spawn_helper_attended(home, event, payload, attended))
            .stdout
            .is_empty()
    );
}

fn launches(home: &TempHome) -> Vec<String> {
    std::thread::sleep(Duration::from_secs(1));
    std::fs::read_to_string(home.0.join("launches"))
        .map(|text| text.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

#[test]
fn with_the_core_down_a_session_start_launches_the_app_once_per_throttle_window() {
    let home = TempHome::new();
    with_marker(&home);
    quiet_run(&home, "SessionStart", SESSION_START, "1");
    assert_eq!(launches(&home), ["--background"]);
    quiet_run(&home, "SessionStart", SESSION_START, "1");
    quiet_run(&home, "UserPromptSubmit", USER_PROMPT_SUBMIT, "1");
    assert_eq!(launches(&home), ["--background"]);
    std::fs::File::options()
        .write(true)
        .open(home.0.join("launching"))
        .unwrap()
        .set_modified(std::time::SystemTime::now() - Duration::from_secs(21))
        .unwrap();
    quiet_run(&home, "SessionStart", SESSION_START, "1");
    assert_eq!(launches(&home), ["--background", "--background"]);
}

#[test]
fn a_prompt_launches_the_app_when_the_endpoint_names_a_missing_pipe() {
    let home = TempHome::new();
    with_marker(&home);
    std::fs::write(
        home.0.join("endpoint"),
        beacon_protocol::Endpoint {
            pipe_name: r"\\.\pipe\beacon-0000000000000000".to_string(),
            token: "ab".repeat(32),
        }
        .to_file_text(),
    )
    .unwrap();
    quiet_run(&home, "UserPromptSubmit", USER_PROMPT_SUBMIT, "1");
    assert_eq!(launches(&home), ["--background"]);
}

#[test]
fn other_events_unattended_sessions_and_a_missing_marker_launch_nothing() {
    let home = TempHome::new();
    with_marker(&home);
    for (event, payload) in [
        ("PreToolUse", PRE_TOOL_USE),
        ("Stop", STOP),
        ("SessionEnd", SESSION_END),
    ] {
        quiet_run(&home, event, payload, "1");
    }
    quiet_run(&home, "SessionStart", SESSION_START, "0");
    assert!(launches(&home).is_empty());
    assert!(!home.0.join("launching").exists());
    let unmarked = TempHome::new();
    quiet_run(&unmarked, "SessionStart", SESSION_START, "1");
    assert!(launches(&unmarked).is_empty());
    assert!(!unmarked.0.join("launching").exists());
}

#[test]
fn with_the_core_up_nothing_is_launched() {
    let home = TempHome::new();
    let core = start_core(&home);
    with_marker(&home);
    quiet_run(&home, "SessionStart", SESSION_START, "1");
    quiet_run(&home, "UserPromptSubmit", USER_PROMPT_SUBMIT, "1");
    assert!(launches(&home).is_empty());
    assert!(!home.0.join("launching").exists());
    core.shutdown().unwrap();
}

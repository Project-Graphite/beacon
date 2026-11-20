use beacon_core::{
    AccountSources, Config, ControlClient, Core, CoreHandle, CoreOptions, Decision, ListFuture,
    ListedSession, ProbeFuture, Reply, Request, Session, SessionLister, SessionState, Snapshot,
    UsageProber, UsageSource, UsageStatus, now_ms,
};
use serde_json::json;
use std::collections::VecDeque;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

struct FakeLister(io::Result<Vec<ListedSession>>);

impl SessionLister for FakeLister {
    fn list(&self) -> ListFuture<'_> {
        let listed = match &self.0 {
            Ok(listed) => Ok(listed.clone()),
            Err(error) => Err(io::Error::new(error.kind(), error.to_string())),
        };
        Box::pin(async move { listed })
    }
}

struct TempHome(PathBuf);

impl TempHome {
    fn new() -> TempHome {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        TempHome(std::env::temp_dir().join(format!(
            "beacon-core-it-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn start(home: &TempHome, listed: io::Result<Vec<ListedSession>>) -> io::Result<CoreHandle> {
    Core::start(CoreOptions {
        home: home.0.clone(),
        lister: Box::new(FakeLister(listed)),
        config: Config::default(),
    })
}

fn snapshot(home: &TempHome) -> Snapshot {
    let reply = ControlClient::connect(&home.0)
        .unwrap()
        .request(&Request::Snapshot)
        .unwrap();
    assert!(reply.ok);
    reply.snapshot.unwrap()
}

fn wait_for(home: &TempHome, condition: impl Fn(&Snapshot) -> bool) -> Snapshot {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = snapshot(home);
        if condition(&snapshot) {
            return snapshot;
        }
        assert!(Instant::now() < deadline, "condition not met: {snapshot:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn listed(session_id: &str, status: &str) -> ListedSession {
    ListedSession {
        session_id: session_id.to_string(),
        pid: Some(std::process::id()),
        cwd: Some(r"C:\STUFF\somewhere".to_string()),
        kind: Some("interactive".to_string()),
        started_at: Some(1_795_089_974_147),
        name: Some("listed".to_string()),
        status: Some(status.to_string()),
        waiting_for: None,
    }
}

#[test]
fn reconciliation_at_start_registers_listed_sessions_as_limited() {
    let home = TempHome::new();
    let core = start(&home, Ok(vec![listed("a", "busy"), listed("b", "waiting")])).unwrap();
    let snapshot = wait_for(&home, |snapshot| snapshot.sessions.len() == 2);
    assert!(snapshot.sessions.iter().all(|session| session.limited));
    assert_eq!(snapshot.sessions[0].state, SessionState::Working);
    assert_eq!(snapshot.sessions[1].state, SessionState::NeedsPermission);
    assert_eq!(snapshot.sessions[0].project.as_deref(), Some("somewhere"));
    core.shutdown().unwrap();
}

#[test]
fn a_failing_lister_changes_nothing() {
    let home = TempHome::new();
    let core = start(&home, Err(io::Error::other("claude is not installed"))).unwrap();
    assert!(snapshot(&home).sessions.is_empty());
    core.shutdown().unwrap();
    let log = std::fs::read_to_string(home.0.join("logs").join("core.log")).unwrap();
    assert!(log.contains("session listing gave no information: claude is not installed"));
}

#[test]
fn a_second_core_for_the_same_home_is_refused() {
    let home = TempHome::new();
    let core = start(&home, Ok(Vec::new())).unwrap();
    let second = start(&home, Ok(Vec::new()));
    assert_eq!(second.err().unwrap().kind(), io::ErrorKind::AlreadyExists);
    core.shutdown().unwrap();
    start(&home, Ok(Vec::new())).unwrap().shutdown().unwrap();
}

#[test]
fn the_endpoint_is_written_at_start_and_removed_at_shutdown() {
    let home = TempHome::new();
    let core = start(&home, Ok(Vec::new())).unwrap();
    let endpoint = beacon_protocol::Endpoint::read(&home.0).unwrap();
    assert_eq!(&endpoint, core.endpoint());
    assert!(endpoint.pipe_name.starts_with(r"\\.\pipe\beacon-"));
    assert_eq!(endpoint.pipe_name.len(), r"\\.\pipe\beacon-".len() + 16);
    core.shutdown().unwrap();
    assert!(!home.0.join("endpoint").exists());
    assert!(home.0.join("state.json").exists());
}

#[test]
fn sessions_survive_a_restart() {
    let home = TempHome::new();
    let core = start(&home, Ok(vec![listed("kept", "idle")])).unwrap();
    wait_for(&home, |snapshot| snapshot.sessions.len() == 1);
    core.shutdown().unwrap();
    let core = start(&home, Ok(Vec::new())).unwrap();
    let snapshot = snapshot(&home);
    assert_eq!(snapshot.sessions.len(), 1);
    assert_eq!(snapshot.sessions[0].session_id, "kept");
    assert_eq!(snapshot.sessions[0].state, SessionState::Idle);
    core.shutdown().unwrap();
}

#[test]
fn control_requests_report_failures() {
    let home = TempHome::new();
    let core = start(&home, Ok(Vec::new())).unwrap();
    let mut client = ControlClient::connect(&home.0).unwrap();
    let missing = client
        .request(&Request::Decide {
            id: 99,
            decision: Decision::Allow,
        })
        .unwrap();
    assert_eq!(missing, Reply::failed("no pending decision 99"));
    let unknown = client
        .request(&Request::Acknowledge {
            session_id: "nobody".to_string(),
        })
        .unwrap();
    assert!(!unknown.ok);
    assert!(client.request(&Request::Snapshot).unwrap().ok);
    core.shutdown().unwrap();
}

#[test]
fn malformed_control_lines_get_an_error_reply() {
    use std::io::{BufRead, BufReader, Write};
    let home = TempHome::new();
    let core = start(&home, Ok(Vec::new())).unwrap();
    let endpoint = beacon_protocol::Endpoint::read(&home.0).unwrap();
    let mut pipe = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&endpoint.pipe_name)
        .unwrap();
    pipe.write_all(
        &beacon_protocol::encode_preamble(beacon_protocol::Channel::Control, &endpoint.token)
            .unwrap(),
    )
    .unwrap();
    pipe.write_all(b"{\"op\":\"fly\"}\n").unwrap();
    let mut line = String::new();
    BufReader::new(&pipe).read_line(&mut line).unwrap();
    let reply: Reply = serde_json::from_str(&line).unwrap();
    assert!(!reply.ok);
    assert!(reply.error.unwrap().starts_with("bad request"));
    core.shutdown().unwrap();
}

#[test]
fn subscribers_get_a_snapshot_then_changes() {
    let home = TempHome::new();
    let core = start(&home, Ok(vec![listed("s", "idle")])).unwrap();
    wait_for(&home, |snapshot| snapshot.sessions.len() == 1);
    let mut updates = ControlClient::connect(&home.0)
        .unwrap()
        .subscribe()
        .unwrap();
    let first = updates.next().unwrap().unwrap();
    assert_eq!(first.snapshot.unwrap().sessions.len(), 1);
    let mut client = ControlClient::connect(&home.0).unwrap();
    assert!(
        client
            .request(&Request::Acknowledge {
                session_id: "s".to_string()
            })
            .unwrap()
            .ok
    );
    let second = updates.next().unwrap().unwrap();
    assert!(second.ok);
    assert!(second.snapshot.is_some());
    core.shutdown().unwrap();
}

#[test]
fn the_shutdown_request_stops_the_core() {
    let home = TempHome::new();
    let core = start(&home, Ok(Vec::new())).unwrap();
    let reply = ControlClient::connect(&home.0)
        .unwrap()
        .request(&Request::Shutdown)
        .unwrap();
    assert!(reply.ok);
    core.wait().unwrap();
    assert!(!home.0.join("endpoint").exists());
}

struct FakeProber(Mutex<VecDeque<io::Result<Vec<u8>>>>);

impl UsageProber for FakeProber {
    fn probe(&self) -> ProbeFuture<'_> {
        let next = self.0.lock().unwrap().pop_front().unwrap();
        Box::pin(async move { next })
    }
}

fn start_with(
    home: &TempHome,
    listed: Vec<ListedSession>,
    config: Config,
    account: AccountSources,
) -> CoreHandle {
    Core::start_with(
        CoreOptions {
            home: home.0.clone(),
            lister: Box::new(FakeLister(Ok(listed))),
            config,
        },
        account,
    )
    .unwrap()
}

fn write_state(home: &TempHome, sessions: Vec<Session>, snoozed_until_ms: Option<u64>) {
    std::fs::create_dir_all(&home.0).unwrap();
    std::fs::write(
        home.0.join("state.json"),
        json!({"sessions": sessions, "snoozed_until_ms": snoozed_until_ms}).to_string(),
    )
    .unwrap();
}

#[test]
fn an_expiring_snooze_reaches_subscribers() {
    let home = TempHome::new();
    let done = Session {
        state: SessionState::Done,
        attention: true,
        ..Session::new("done", now_ms())
    };
    write_state(&home, vec![done], Some(now_ms() + 1_500));
    let core = start(&home, Ok(vec![listed("done", "idle")])).unwrap();
    let mut updates = ControlClient::connect(&home.0)
        .unwrap()
        .subscribe()
        .unwrap()
        .map(|update| update.unwrap().snapshot.unwrap());
    let first = updates.next().unwrap();
    assert!(first.snoozed_until_ms.is_some());
    assert!(first.sessions[0].attention && !first.sessions[0].alerting);
    let expired = updates
        .find(|snapshot| snapshot.snoozed_until_ms.is_none())
        .unwrap();
    assert!(expired.sessions[0].alerting);
    let mut client = ControlClient::connect(&home.0).unwrap();
    let snoozed = client
        .request(&Request::Snooze {
            session_id: Some("done".to_string()),
            minutes: 1,
        })
        .unwrap();
    assert!(snoozed.ok);
    assert!(!snapshot(&home).sessions[0].alerting);
    core.shutdown().unwrap();
    let state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.0.join("state.json")).unwrap()).unwrap();
    assert!(state["sessions"][0]["snoozed_until_ms"].is_u64());
}

#[test]
fn transcript_usage_is_read_for_sessions_loaded_at_start() {
    let home = TempHome::new();
    std::fs::create_dir_all(&home.0).unwrap();
    let transcript = home.0.join("with-transcript.jsonl");
    let line = json!({"type": "assistant", "message": {"id": "msg_1", "model": "m", "usage": {"input_tokens": 3, "output_tokens": 4, "cache_creation_input_tokens": 5, "cache_read_input_tokens": 6}}}).to_string() + "\n";
    std::fs::write(&transcript, line.repeat(3)).unwrap();
    let session = Session {
        transcript_path: Some(transcript.to_string_lossy().into_owned()),
        ..Session::new("with-transcript", now_ms())
    };
    write_state(&home, vec![session], None);
    let core = start(&home, Ok(vec![listed("with-transcript", "idle")])).unwrap();
    let snapshot = wait_for(&home, |snapshot| snapshot.sessions[0].usage.is_some());
    let usage = snapshot.sessions[0].usage.clone().unwrap();
    assert_eq!(
        (
            usage.input_tokens,
            usage.output_tokens,
            usage.cache_creation_tokens,
            usage.cache_read_tokens,
            usage.context_tokens,
            usage.status
        ),
        (3, 4, 5, 6, 14, UsageStatus::Ok)
    );
    let detail = ControlClient::connect(&home.0)
        .unwrap()
        .request(&Request::Detail {
            session_id: "with-transcript".to_string(),
        })
        .unwrap()
        .detail
        .unwrap();
    assert!(detail.timeline.is_empty());
    assert_eq!(detail.last_message, None);
    core.shutdown().unwrap();
}

#[test]
fn the_account_cache_is_read_at_start_and_again_when_it_changes() {
    let home = TempHome::new();
    std::fs::create_dir_all(&home.0).unwrap();
    let claude_json = home.0.join("claude.json");
    let write = |fetched_ms: u64, percent: f64| {
        std::fs::write(
            &claude_json,
            json!({
                "oauthAccount": {"emailAddress": "someone@example.test"},
                "projects": {"C:/work": {"lastCost": 1.5}},
                "cachedUsageUtilization": {
                    "fetchedAtMs": fetched_ms,
                    "utilization": {
                        "five_hour": {"utilization": percent, "resets_at": "2026-11-20T14:30:00Z"},
                        "seven_day": {"utilization": 3, "resets_at": "2026-11-26T09:00:00Z"},
                    },
                    "spend": {"amount": 2},
                },
            })
            .to_string(),
        )
        .unwrap()
    };
    write(1_000, 40.0);
    let core = start_with(
        &home,
        Vec::new(),
        Config {
            reconcile_seconds: 1,
            ..Config::default()
        },
        AccountSources {
            claude_json: Some(claude_json.clone()),
            prober: None,
        },
    );
    let first = wait_for(&home, |snapshot| snapshot.account_usage.is_some())
        .account_usage
        .unwrap();
    assert_eq!(
        (
            first.five_hour_percent,
            first.seven_day_percent,
            first.fetched_ms,
            first.source
        ),
        (Some(40.0), Some(3.0), Some(1_000), UsageSource::Cache)
    );
    write(2_000, 55.0);
    wait_for(&home, |snapshot| {
        snapshot
            .account_usage
            .as_ref()
            .is_some_and(|usage| usage.five_hour_percent == Some(55.0))
    });
    let refused = ControlClient::connect(&home.0)
        .unwrap()
        .request(&Request::RefreshUsage)
        .unwrap();
    assert!(!refused.ok);
    core.shutdown().unwrap();
}

#[test]
fn a_refresh_reads_the_probe_and_a_failed_one_keeps_the_last_value() {
    let home = TempHome::new();
    let stream = concat!(
        r#"{"type":"system","subtype":"init"}"#,
        "\n",
        r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","unifiedWindows":{"five_hour":{"utilization":0.5,"resetsAt":1795104600},"seven_day":{"utilization":0.25,"resetsAt":1795690800}}}}"#,
        "\n",
    );
    let core = start_with(
        &home,
        Vec::new(),
        Config::default(),
        AccountSources {
            claude_json: None,
            prober: Some(Box::new(FakeProber(Mutex::new(VecDeque::from([
                Ok(stream.as_bytes().to_vec()),
                Ok(br#"{"type":"result"}"#.to_vec()),
                Err(io::Error::other("claude -p timed out")),
            ]))))),
        },
    );
    let mut client = ControlClient::connect(&home.0).unwrap();
    assert!(client.request(&Request::RefreshUsage).unwrap().ok);
    let probed = snapshot(&home).account_usage.unwrap();
    assert_eq!(probed.source, UsageSource::Probe);
    assert_eq!(probed.five_hour_percent, Some(50.0));
    assert_eq!(
        probed.seven_day_resets_at.as_deref(),
        Some("2026-11-26T11:00:00.000Z")
    );
    for _ in 0..2 {
        let failed = client.request(&Request::RefreshUsage).unwrap();
        assert!(!failed.ok);
        assert!(failed.error.unwrap().starts_with("usage refresh failed: "));
    }
    assert_eq!(snapshot(&home).account_usage, Some(probed));
    core.shutdown().unwrap();
}

#[test]
fn a_connection_with_a_bad_token_is_closed() {
    use std::io::{Read, Write};
    let home = TempHome::new();
    let core = start(&home, Ok(Vec::new())).unwrap();
    let endpoint = beacon_protocol::Endpoint::read(&home.0).unwrap();
    let mut pipe = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&endpoint.pipe_name)
        .unwrap();
    let wrong = "f".repeat(64);
    pipe.write_all(
        &beacon_protocol::encode_preamble(beacon_protocol::Channel::Control, &wrong).unwrap(),
    )
    .unwrap();
    let _ = pipe.write_all(b"{\"op\":\"snapshot\"}\n");
    let mut rest = Vec::new();
    let _ = pipe.read_to_end(&mut rest);
    assert!(rest.is_empty());
    core.shutdown().unwrap();
}

use beacon_core::{
    Config, ControlClient, Core, CoreHandle, CoreOptions, Decision, ListFuture, ListedSession,
    Reply, Request, SessionLister, SessionState, Snapshot,
};
use std::io;
use std::path::PathBuf;
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

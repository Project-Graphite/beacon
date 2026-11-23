use super::*;
use crate::installer::install;
use crate::test_support::{TempDir, built_helper, test_paths};
use beacon_core::{Config, Core, CoreHandle, CoreOptions, ListFuture, Session, SessionLister};
use serde_json::json;

struct NoSessions;

impl SessionLister for NoSessions {
    fn list(&self) -> ListFuture<'_> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

fn installed_with_core(root: &TempDir) -> (Paths, CoreHandle) {
    let paths = test_paths(root, built_helper());
    install(&paths, true).unwrap();
    let core = Core::start(CoreOptions {
        home: paths.home.clone(),
        lister: Box::new(NoSessions),
        config: Config::default(),
    })
    .unwrap();
    (paths, core)
}

fn claude_line(line: &str) -> bool {
    line.starts_with("ok   claude: ") || line.starts_with("warn claude: ")
}

#[test]
fn a_healthy_installation_passes_every_check() {
    let root = TempDir::new("doctor-ok");
    let (paths, core) = installed_with_core(&root);
    let (report, healthy) = run(&paths);
    core.shutdown().unwrap();
    assert!(healthy, "{report}");
    let lines: Vec<&str> = report.lines().collect();
    assert_eq!(lines.len(), 9, "{report}");
    assert_eq!(
        lines[..5],
        [
            format!("ok   helper: {}", paths.helper().display()),
            format!(
                "ok   hooks: all 23 handlers registered in {}",
                paths.settings.display()
            ),
            "ok   disableAllHooks: not set".to_string(),
            "ok   managed policy: no managed settings file sets allowManagedHooksOnly".to_string(),
            format!("warn managed policy: {REGISTRY_NOTICE}"),
        ]
    );
    assert!(lines[5].starts_with(r"ok   core: reachable on \\.\pipe\beacon-"));
    assert!(lines[6].starts_with("ok   round trip: "), "{}", lines[6]);
    assert!(lines[6].ends_with(&format!(" ms through {}", paths.helper().display())));
    assert!(claude_line(lines[7]));
    assert_eq!(lines[8], "ok   surfaces: no sessions seen yet");
}

#[test]
fn a_broken_installation_fails_the_broken_checks() {
    let root = TempDir::new("doctor-broken");
    let (paths, core) = installed_with_core(&root);
    std::fs::remove_file(paths.helper()).unwrap();
    let mut settings: Value =
        serde_json::from_slice(&std::fs::read(&paths.settings).unwrap()).unwrap();
    settings["hooks"]
        .as_object_mut()
        .unwrap()
        .shift_remove("Stop");
    settings["disableAllHooks"] = json!(true);
    std::fs::write(&paths.settings, settings.to_string()).unwrap();
    let (report, healthy) = run(&paths);
    core.shutdown().unwrap();
    assert!(!healthy);
    let lines: Vec<&str> = report.lines().collect();
    assert_eq!(lines.len(), 9, "{report}");
    assert_eq!(
        lines[..5],
        [
            format!("fail helper: missing at {}", paths.helper().display()),
            "fail hooks: Stop is not registered; StopWaiter is not registered".to_string(),
            format!(
                "fail disableAllHooks: true in {}, so no hook runs",
                paths.settings.display()
            ),
            "ok   managed policy: no managed settings file sets allowManagedHooksOnly".to_string(),
            format!("warn managed policy: {REGISTRY_NOTICE}"),
        ]
    );
    assert!(lines[5].starts_with("ok   core: reachable"));
    assert_eq!(lines[6], "fail round trip: not possible without the helper");
    assert!(claude_line(lines[7]));
    assert_eq!(lines[8], "ok   surfaces: no sessions seen yet");
}

#[test]
fn without_a_core_or_settings_the_doctor_fails() {
    let root = TempDir::new("doctor-empty");
    let paths = test_paths(&root, built_helper());
    let (report, healthy) = run(&paths);
    assert!(!healthy);
    let lines: Vec<&str> = report.lines().collect();
    assert!(lines[1].starts_with(&format!(
        "fail settings: cannot read {}: ",
        paths.settings.display()
    )));
    assert_eq!(
        lines[4],
        format!(
            "fail core: no endpoint file in {}; start `beacon daemon`",
            paths.home.display()
        )
    );
    assert_eq!(lines[7], "warn surfaces: unknown without a reachable core");
}

#[test]
fn hooks_status_says_installed_not_installed_or_needs_repair() {
    let root = TempDir::new("doctor-status");
    let helper_source = root.0.join("beacon-hook.exe");
    std::fs::write(&helper_source, b"stand-in helper").unwrap();
    let paths = test_paths(&root, helper_source);
    let registry_warning = vec![format!("managed policy: {REGISTRY_NOTICE}")];
    let not_installed = HooksStatus {
        state: HooksState::NotInstalled,
        settings_path: paths.settings.clone(),
        problems: Vec::new(),
        warnings: registry_warning.clone(),
    };
    assert_eq!(hooks_status(&paths), not_installed);
    install(&paths, true).unwrap();
    assert_eq!(
        hooks_status(&paths),
        HooksStatus {
            state: HooksState::Installed,
            ..not_installed.clone()
        }
    );
    let mut settings: Value =
        serde_json::from_slice(&std::fs::read(&paths.settings).unwrap()).unwrap();
    settings["hooks"]
        .as_object_mut()
        .unwrap()
        .shift_remove("Stop");
    std::fs::write(&paths.settings, settings.to_string()).unwrap();
    assert_eq!(
        hooks_status(&paths),
        HooksStatus {
            state: HooksState::NeedsRepair,
            problems: vec![
                "hooks: Stop is not registered; StopWaiter is not registered".to_string()
            ],
            ..not_installed.clone()
        }
    );
    crate::installer::uninstall(&paths, true).unwrap();
    assert_eq!(hooks_status(&paths), not_installed);
    std::fs::write(&paths.settings, r#"{"disableAllHooks": true}"#).unwrap();
    assert_eq!(
        hooks_status(&paths),
        HooksStatus {
            problems: vec![format!(
                "disableAllHooks: true in {}, so no hook runs",
                paths.settings.display()
            )],
            ..not_installed
        }
    );
}

#[test]
fn a_matcher_added_to_a_beacon_group_needs_repair_until_reinstalled() {
    let root = TempDir::new("doctor-matcher");
    let helper_source = root.0.join("beacon-hook.exe");
    std::fs::write(&helper_source, b"stand-in helper").unwrap();
    let paths = test_paths(&root, helper_source);
    install(&paths, true).unwrap();
    let mut settings: Value =
        serde_json::from_slice(&std::fs::read(&paths.settings).unwrap()).unwrap();
    settings["hooks"]["PreToolUse"][0]["matcher"] = json!("Bash");
    std::fs::write(
        &paths.settings,
        serde_json::to_string_pretty(&settings).unwrap(),
    )
    .unwrap();
    let status = hooks_status(&paths);
    assert_eq!(status.state, HooksState::NeedsRepair);
    assert_eq!(
        status.problems,
        ["hooks: PreToolUse is limited by a matcher, so Beacon only sees part of PreToolUse"]
    );
    install(&paths, true).unwrap();
    assert_eq!(hooks_status(&paths).state, HooksState::Installed);
}

#[test]
fn a_helper_that_outlasts_the_timeout_fails_the_round_trip() {
    let root = TempDir::new("doctor-timeout");
    let paths = test_paths(&root, built_helper());
    install(&paths, true).unwrap();
    assert_eq!(
        round_trip(&paths.home, &paths.helper(), Duration::ZERO),
        (
            Level::Fail,
            "round trip: the helper did not finish within 0 ms".to_string()
        )
    );
}

#[test]
fn a_closed_stdin_is_not_a_failed_write_but_other_errors_are() {
    struct Failing(io::ErrorKind);
    impl Write for Failing {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(self.0.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    assert!(write_payload(Failing(io::ErrorKind::BrokenPipe)).is_ok());
    assert_eq!(
        write_payload(Failing(io::ErrorKind::PermissionDenied))
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
}

#[test]
fn surfaces_are_counted_with_limited_sessions() {
    let session = |id: &str, entrypoint: Option<&str>, limited: bool| Session {
        entrypoint: entrypoint.map(str::to_string),
        limited,
        ..Session::new(id, 0)
    };
    let snapshot = Snapshot {
        ended_count: 0,
        sessions: vec![
            session("a", Some("cli"), false),
            session("b", Some("cli"), false),
            session("c", Some("claude-vscode"), false),
            session("d", None, true),
        ],
        pending: Vec::new(),
        resolved: Vec::new(),
        snoozed_until_ms: None,
        account_usage: None,
        config: beacon_core::Config::default().live(),
    };
    assert_eq!(
        surfaces(Some(&snapshot)),
        (
            Level::Warn,
            "surfaces: VS Code 1 seen, 0 limited; terminal 2 seen, 0 limited; unknown 1 seen, 1 limited (limited sessions are known only from `claude agents --json`; hooks have not fired for them)".to_string()
        )
    );
}

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
                "ok   hooks: all 22 events registered in {}",
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
            "fail hooks: Stop is not registered".to_string(),
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
fn surfaces_are_counted_with_limited_sessions() {
    let session = |id: &str, entrypoint: Option<&str>, limited: bool| Session {
        entrypoint: entrypoint.map(str::to_string),
        limited,
        ..Session::new(id, 0)
    };
    let snapshot = Snapshot {
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
    };
    assert_eq!(
        surfaces(Some(&snapshot)),
        (
            Level::Warn,
            "surfaces: VS Code 1 seen, 0 limited; terminal 2 seen, 0 limited; unknown 1 seen, 1 limited (limited sessions are known only from `claude agents --json`; hooks have not fired for them)".to_string()
        )
    );
}

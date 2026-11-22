use super::*;
use crate::test_support::{TempDir, built_helper, test_paths};
use beacon_protocol::{Endpoint, HOME_ENV};
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/settings");
const VALID: [&str; 12] = [
    "missing",
    "empty-object",
    "realistic",
    "other-events",
    "user-handlers",
    "stale",
    "four-spaces",
    "tabs",
    "crlf",
    "bom",
    "no-trailing-newline",
    "non-ascii",
];

struct Case {
    _root: TempDir,
    paths: Paths,
    input: Option<Vec<u8>>,
}

fn case(name: &str) -> Case {
    let root = TempDir::new(name);
    let helper_source = root.0.join("source").join(HELPER_FILE_NAME);
    std::fs::create_dir_all(helper_source.parent().unwrap()).unwrap();
    std::fs::write(&helper_source, b"stand-in helper").unwrap();
    let paths = test_paths(&root, helper_source);
    let input = std::fs::read(format!("{FIXTURES}/{name}.json")).ok();
    if let Some(bytes) = &input {
        std::fs::create_dir_all(paths.settings.parent().unwrap()).unwrap();
        std::fs::write(&paths.settings, bytes).unwrap();
    }
    Case {
        _root: root,
        paths,
        input,
    }
}

impl Case {
    fn settings(&self) -> Option<Vec<u8>> {
        std::fs::read(&self.paths.settings).ok()
    }

    fn golden(&self, name: &str) -> Vec<u8> {
        std::fs::read_to_string(format!("{FIXTURES}/{name}.installed.json"))
            .unwrap()
            .replace(
                "{HELPER}",
                &self.paths.helper().to_string_lossy().replace('\\', "\\\\"),
            )
            .into_bytes()
    }

    fn install(&self) -> String {
        install(&self.paths, true).unwrap()
    }

    fn uninstall(&self) -> String {
        uninstall(&self.paths, true).unwrap()
    }
}

fn shown(bytes: Option<&[u8]>) -> String {
    bytes.map_or("<absent>".to_string(), |bytes| {
        String::from_utf8_lossy(bytes).into_owned()
    })
}

#[test]
fn the_fixtures_are_in_canonical_form() {
    for name in VALID.into_iter().filter(|name| *name != "missing") {
        let input = std::fs::read(format!("{FIXTURES}/{name}.json")).unwrap();
        let (settings, format) = settings::parse(&input).unwrap();
        assert_eq!(
            shown(Some(&format.write(&settings))),
            shown(Some(&input)),
            "{name}"
        );
    }
}

#[test]
fn install_matches_the_golden_files_and_a_second_install_changes_nothing() {
    for name in VALID {
        let case = case(name);
        case.install();
        let golden = case.golden(name);
        assert_eq!(
            shown(case.settings().as_deref()),
            shown(Some(&golden)),
            "{name}"
        );
        assert_eq!(
            std::fs::read(case.paths.helper()).unwrap(),
            b"stand-in helper"
        );
        let second = case.install();
        assert!(second.contains("No change to the settings file."), "{name}");
        assert_eq!(case.settings(), Some(golden), "{name}");
    }
}

#[test]
fn install_then_uninstall_restores_the_input_bytes() {
    for name in VALID {
        let case = case(name);
        case.install();
        case.install();
        case.uninstall();
        assert_eq!(case.settings(), case.input, "{name}");
        assert!(!case.paths.helper().exists(), "{name}");
        assert!(!case.paths.record().exists(), "{name}");
        assert!(
            std::fs::read_dir(case.paths.backups()).unwrap().count() >= 2,
            "{name}"
        );
    }
}

fn edited(settings: &mut Map<String, Value>) {
    settings.insert(
        "model".to_string(),
        Value::String("user-edited-model".to_string()),
    );
}

#[test]
fn uninstall_after_a_user_edit_keeps_the_edit_and_removes_only_beacon() {
    for name in VALID {
        let case = case(name);
        case.install();
        let (mut installed, format) = settings::parse(&case.settings().unwrap()).unwrap();
        edited(&mut installed);
        std::fs::write(&case.paths.settings, format.write(&installed)).unwrap();
        case.uninstall();
        let (mut expected, expected_format) = match &case.input {
            Some(input) => settings::parse(input).unwrap(),
            None => (Map::new(), TextFormat::default()),
        };
        edited(&mut expected);
        settings::strip_beacon(&mut expected, |_| true);
        let result = case.settings().unwrap();
        assert_eq!(
            shown(Some(&result)),
            shown(Some(&expected_format.write(&expected))),
            "{name}"
        );
        assert!(
            !String::from_utf8_lossy(&result)
                .to_ascii_lowercase()
                .contains("beacon-hook.exe"),
            "{name}"
        );
    }
}

#[test]
fn a_hook_appended_after_beacon_leaves_install_idle_and_survives_uninstall() {
    let case = case("foreign-after-beacon");
    case.install();
    let appended = case.golden("foreign-after-beacon");
    std::fs::write(&case.paths.settings, &appended).unwrap();
    let modified = || {
        std::fs::metadata(&case.paths.settings)
            .unwrap()
            .modified()
            .unwrap()
    };
    let before = modified();
    assert!(
        install(&case.paths, false)
            .unwrap()
            .contains("No change to the settings file.")
    );
    case.install();
    assert_eq!(case.settings(), Some(appended));
    assert_eq!(modified(), before);
    case.uninstall();
    let (settings, _) = settings::parse(&case.settings().unwrap()).unwrap();
    assert_eq!(
        Value::Object(settings),
        serde_json::json!({"hooks": {"SessionStart": [{"hooks": [{
            "type": "command",
            "command": "pwsh.exe",
            "args": ["-NoProfile", "-File", "C:\\Users\\example\\scripts\\Sync-Accounts.ps1"],
            "async": true
        }]}]}})
    );
}

#[test]
fn invalid_json_and_non_objects_are_refused_and_left_untouched() {
    for name in ["invalid", "array"] {
        let case = case(name);
        for result in [
            install(&case.paths, true),
            install(&case.paths, false),
            uninstall(&case.paths, true),
        ] {
            assert!(
                result.unwrap_err().starts_with("refusing to touch"),
                "{name}"
            );
        }
        assert_eq!(case.settings(), case.input, "{name}");
        assert!(!case.paths.home.exists(), "{name}");
    }
}

#[test]
fn the_plan_changes_nothing_and_shows_the_diff_and_warnings() {
    let case = case("realistic");
    let plan = install(&case.paths, false).unwrap();
    assert_eq!(case.settings(), case.input);
    assert!(!case.paths.home.exists());
    assert!(plan.contains(
        "@@ -36,5 +36,326 @@\n   },\n   \"autoUpdatesChannel\": \"stable\",\n   \"autoMemoryEnabled\": false,\n-  \"switchModelsOnFlag\": true\n+  \"switchModelsOnFlag\": true,\n+  \"hooks\": {\n"
    ));
    assert!(plan.ends_with(&format!(
        "Warning: {REGISTRY_NOTICE}\nNothing was changed. Run again with --yes to install.\n"
    )));
    assert!(!plan.contains("disableAllHooks"));
}

#[test]
fn disabled_hooks_and_managed_hooks_only_are_warned_about() {
    let case = case("empty-object");
    std::fs::write(&case.paths.settings, "{\n  \"disableAllHooks\": true\n}\n").unwrap();
    let drop_in = case.paths.managed_dir.join("managed-settings.d");
    std::fs::create_dir_all(&drop_in).unwrap();
    std::fs::write(
        drop_in.join("50-policy.json"),
        r#"{"allowManagedHooksOnly": true}"#,
    )
    .unwrap();
    std::fs::write(
        case.paths.managed_dir.join("managed-settings.json"),
        r#"{"allowManagedHooksOnly": false}"#,
    )
    .unwrap();
    let plan = install(&case.paths, false).unwrap();
    let warnings: Vec<String> = plan
        .lines()
        .filter(|line| line.starts_with("Warning:"))
        .map(str::to_string)
        .collect();
    assert_eq!(
        warnings,
        [
            "Warning: disableAllHooks is true in this file, so no hook runs, Beacon's included"
                .to_string(),
            format!(
                "Warning: {} sets allowManagedHooksOnly: Beacon cannot work under that policy",
                drop_in.join("50-policy.json").display()
            ),
            format!("Warning: {REGISTRY_NOTICE}"),
        ]
    );
}

#[test]
fn the_record_says_what_install_created() {
    let case = case("user-handlers");
    case.install();
    let record = read_record(&case.paths).unwrap().unwrap();
    assert!(record.existed_before);
    assert!(!record.created.hooks);
    assert_eq!(record.created.events.len(), 20);
    assert!(!record.created.includes(Some("PreToolUse")));
    assert!(!record.created.includes(Some("Stop")));
    assert_eq!(std::fs::read(record.backup_path.unwrap()).ok(), case.input);
    assert_eq!(
        Some(std::fs::read(record.installed_copy_path).unwrap()),
        case.settings()
    );
    assert_eq!(record.helper_path, case.paths.helper());
    let missing = self::case("missing");
    missing.install();
    let record = read_record(&missing.paths).unwrap().unwrap();
    assert!(!record.existed_before);
    assert_eq!(record.backup_path, None);
    assert!(record.created.hooks);
    assert_eq!(record.created.events.len(), 22);
}

#[test]
fn uninstall_with_nothing_installed_is_a_no_op() {
    for name in ["missing", "realistic"] {
        let case = case(name);
        assert_eq!(
            case.uninstall(),
            format!(
                "Beacon is not installed for {}: nothing to do.\n",
                case.paths.settings.display()
            )
        );
        assert_eq!(case.settings(), case.input);
    }
}

#[test]
fn uninstall_without_a_record_still_removes_beacon_handlers() {
    let case = case("stale");
    let plan = uninstall(&case.paths, false).unwrap();
    assert!(plan.contains("Run again with --yes to uninstall."));
    case.uninstall();
    let (settings, _) = settings::parse(&case.settings().unwrap()).unwrap();
    assert_eq!(
        Value::Object(settings),
        serde_json::json!({"hooks": {
            "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "C:\\tools\\guard.exe"}]}],
            "Stop": [],
            "WorktreeCreate": []
        }})
    );
}

#[test]
fn uninstall_refuses_a_record_for_another_settings_file() {
    let case = case("realistic");
    case.install();
    let other = Paths {
        settings: case.paths.settings.with_file_name("other.json"),
        home: case.paths.home.clone(),
        helper_source: case.paths.helper_source.clone(),
        managed_dir: case.paths.managed_dir.clone(),
    };
    assert!(
        uninstall(&other, true)
            .unwrap_err()
            .starts_with("install.json records")
    );
}

#[test]
fn a_missing_helper_source_is_an_error() {
    let case = case("realistic");
    std::fs::remove_file(&case.paths.helper_source).unwrap();
    assert!(
        install(&case.paths, false)
            .unwrap_err()
            .starts_with("the helper was not found next to beacon.exe")
    );
}

fn silent_pipe_server(
    home: &Path,
    instances: usize,
) -> (tokio::runtime::Runtime, Vec<NamedPipeServer>) {
    let pipe_name = format!(
        r"\\.\pipe\beacon-cli-silent-{}",
        home.parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
    );
    std::fs::write(
        home.join("endpoint"),
        Endpoint {
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
    let servers = {
        let _context = runtime.enter();
        (0..instances)
            .map(|_| ServerOptions::new().create(&pipe_name).unwrap())
            .collect()
    };
    (runtime, servers)
}

fn pending_helper(paths: &Paths) -> Child {
    let mut helper = Command::new(paths.helper())
        .arg("PermissionRequest")
        .env(HOME_ENV, &paths.home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    helper.stdin.take().unwrap().write_all(b"{}").unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        helper.try_wait().unwrap().is_none(),
        "the helper is not pending"
    );
    helper
}

fn retired_helpers(paths: &Paths) -> Vec<String> {
    std::fs::read_dir(paths.bin())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".old"))
        .collect()
}

#[test]
fn a_pending_helper_blocks_neither_reinstall_nor_uninstall() {
    let mut case = case("realistic");
    case.paths.helper_source = built_helper();
    case.install();
    let (_runtime, _servers) = silent_pipe_server(&case.paths.home, 2);
    let mut first = pending_helper(&case.paths);
    case.install();
    assert!(case.paths.helper().is_file());
    assert_eq!(retired_helpers(&case.paths).len(), 1);
    let mut second = pending_helper(&case.paths);
    case.uninstall();
    assert_eq!(case.settings(), case.input);
    assert!(!case.paths.helper().exists());
    assert!(!case.paths.record().exists());
    assert_eq!(retired_helpers(&case.paths).len(), 2);
    for helper in [&mut first, &mut second] {
        helper.kill().unwrap();
        helper.wait().unwrap();
    }
    case.install();
    assert!(retired_helpers(&case.paths).is_empty());
}

use super::*;

const EXPLORER: u32 = 1;
const VSCODE_CLAUDE: &str = r"C:\Users\x\.vscode\extensions\anthropic.claude-code-2.1.328-win32-x64\resources\native-binary\claude.exe";
const DESKTOP_APP: &str = r"C:\Users\x\AppData\Local\AnthropicClaude\app-6.6395.0\claude.exe";
const DESKTOP_CLAUDE: &str =
    r"C:\Users\x\AppData\Roaming\Claude\claude-code\2.1.326\d31a\claude.exe";

fn process(pid: u32, parent_pid: u32, image_path: &str) -> Process {
    Process {
        pid,
        parent_pid,
        image_name: image_path.rsplit('\\').next().unwrap().to_string(),
        image_path: image_path.to_string(),
    }
}

fn named(pid: u32, parent_pid: u32, image_name: &str) -> Process {
    process(
        pid,
        parent_pid,
        &format!(r"C:\Program Files\x\{image_name}"),
    )
}

fn window(hwnd: isize, pid: u32, title: &str, class: &str) -> Window {
    Window {
        hwnd,
        pid,
        title: title.to_string(),
        class: class.to_string(),
    }
}

fn editor_window(hwnd: isize, pid: u32, title: &str) -> Window {
    window(hwnd, pid, title, "Chrome_WidgetWin_1")
}

fn session(id: &str, pid: u32, cwd: &str, entrypoint: &str) -> Session {
    let mut session = Session::new(id, 0);
    session.state = SessionState::Working;
    session.claude_pid = Some(pid);
    session.cwd = Some(cwd.to_string());
    session.entrypoint = Some(entrypoint.to_string());
    session
}

fn shell() -> Vec<Process> {
    vec![named(EXPLORER, 0, "explorer.exe")]
}

fn shell_windows() -> Vec<Window> {
    vec![window(900, EXPLORER, "Program Manager", "Progman")]
}

fn vscode(image: &str, claude_pids: &[u32]) -> Vec<Process> {
    let mut processes = shell();
    processes.push(named(10, EXPLORER, image));
    processes.push(named(11, 10, image));
    processes.extend(
        claude_pids
            .iter()
            .map(|&pid| process(pid, 11, VSCODE_CLAUDE)),
    );
    processes
}

fn target(hwnd: isize, title: &str, kind: TargetKind, ambiguous: bool) -> Option<Target> {
    Some(Target {
        hwnd,
        title: title.to_string(),
        kind,
        ambiguous,
    })
}

const VSCODE: TargetKind = TargetKind::Editor { scheme: "vscode" };

#[test]
fn vscode_with_one_window_resolves_to_it() {
    let mut windows = shell_windows();
    windows.push(editor_window(
        100,
        10,
        "main.rs - beacon - Visual Studio Code",
    ));
    windows.push(editor_window(101, 10, ""));
    windows.push(editor_window(102, 11, "extension host"));
    assert_eq!(
        resolve(
            &session("s", 20, r"C:\work\elsewhere", "claude-vscode"),
            &vscode("Code.exe", &[20]),
            &windows
        ),
        target(100, "main.rs - beacon - Visual Studio Code", VSCODE, false)
    );
}

#[test]
fn vscode_window_is_chosen_by_cwd_leaf_then_project() {
    let windows = [
        editor_window(100, 10, "a.rs - alpha - Visual Studio Code"),
        editor_window(101, 10, "b.rs - beacon - Visual Studio Code"),
    ];
    let processes = vscode("Code.exe", &[20]);
    let by_leaf = session("s", 20, r"C:\work\Beacon", "claude-vscode");
    assert_eq!(
        resolve(&by_leaf, &processes, &windows),
        target(101, "b.rs - beacon - Visual Studio Code", VSCODE, false)
    );
    let mut by_project = session("s", 20, r"C:\work\sub", "claude-vscode");
    by_project.project = Some("ALPHA".to_string());
    assert_eq!(
        resolve(&by_project, &processes, &windows),
        target(100, "a.rs - alpha - Visual Studio Code", VSCODE, false)
    );
}

#[test]
fn vscode_with_two_windows_and_no_match_takes_the_first_as_ambiguous() {
    let windows = [
        editor_window(100, 10, "a.rs - alpha - Visual Studio Code"),
        editor_window(101, 10, "b.rs - beacon - Visual Studio Code"),
    ];
    assert_eq!(
        resolve(
            &session("s", 20, r"C:\work\gamma", "claude-vscode"),
            &vscode("Code.exe", &[20]),
            &windows
        ),
        target(100, "a.rs - alpha - Visual Studio Code", VSCODE, true)
    );
}

#[test]
fn five_sessions_share_one_vscode_window() {
    let pids = [20, 21, 22, 23, 24];
    let processes = vscode("Code.exe", &pids);
    let windows = [editor_window(100, 10, "beacon - Visual Studio Code")];
    let sessions: Vec<Session> = pids
        .iter()
        .map(|&pid| session(&format!("s{pid}"), pid, r"C:\work\beacon", "claude-vscode"))
        .collect();
    for session in &sessions {
        assert_eq!(
            resolve(session, &processes, &windows).map(|target| target.hwnd),
            Some(100)
        );
    }
    assert_eq!(
        session_in_foreground(&sessions, &processes, &windows, 100),
        None
    );
}

#[test]
fn editor_forks_map_to_their_schemes() {
    for (image, scheme) in [
        ("Code.exe", "vscode"),
        ("Code - Insiders.exe", "vscode-insiders"),
        ("Cursor.exe", "cursor"),
        ("Windsurf.exe", "windsurf"),
        ("Antigravity.exe", "antigravity"),
    ] {
        let found = resolve(
            &session("s", 20, r"C:\work\beacon", "claude-vscode"),
            &vscode(image, &[20]),
            &[editor_window(100, 10, "beacon")],
        )
        .unwrap();
        assert_eq!(found.kind, TargetKind::Editor { scheme }, "{image}");
        assert_eq!(
            session_link(&session("s", 20, "", "claude-vscode"), &found, None),
            Some(format!("{scheme}://anthropic.claude-code/open?session=s"))
        );
    }
}

#[test]
fn desktop_sessions_under_the_app_resolve_to_its_window() {
    let mut processes = shell();
    processes.push(process(6632, EXPLORER, DESKTOP_APP));
    processes.push(process(6640, 6632, DESKTOP_APP));
    processes.push(process(54204, 6632, DESKTOP_CLAUDE));
    let windows = [
        window(300, 6632, "Claude", "Chrome_WidgetWin_1"),
        window(301, 6640, "", "Chrome_WidgetWin_0"),
    ];
    assert_eq!(
        resolve(
            &session("s", 54204, "", "claude-desktop"),
            &processes,
            &windows
        ),
        target(300, "Claude", TargetKind::Desktop, false)
    );
}

#[test]
fn detached_desktop_sessions_find_any_running_desktop_app() {
    let mut processes = shell();
    processes.push(named(5, 0, "svchost.exe"));
    processes.push(process(6632, EXPLORER, DESKTOP_APP));
    processes.push(process(70, 5, DESKTOP_CLAUDE));
    let windows = [window(300, 6632, "Claude", "Chrome_WidgetWin_1")];
    let detached = session("s", 70, "", "claude-desktop");
    assert_eq!(
        resolve(&detached, &processes, &windows),
        target(300, "Claude", TargetKind::Desktop, false)
    );
    let unknown = session("s", 70, "", "cli");
    assert_eq!(resolve(&unknown, &processes, &windows), None);
}

fn terminal(claude_pid: u32) -> Vec<Process> {
    let mut processes = shell();
    processes.push(named(40, EXPLORER, "WindowsTerminal.exe"));
    processes.push(named(41, 40, "pwsh.exe"));
    processes.push(named(claude_pid, 41, "claude.exe"));
    processes
}

#[test]
fn windows_terminal_with_one_window_resolves_to_it() {
    let windows = [
        window(400, 40, "PowerShell", "CASCADIA_HOSTING_WINDOW_CLASS"),
        window(401, 40, "drag bar", "DRAG_BAR_WINDOW_CLASS"),
    ];
    assert_eq!(
        resolve(
            &session("s", 42, r"C:\work\beacon", "cli"),
            &terminal(42),
            &windows
        ),
        target(400, "PowerShell", TargetKind::WindowsTerminal, false)
    );
}

#[test]
fn windows_terminal_with_two_windows_matches_the_cwd_or_the_session_title() {
    let windows = [
        window(400, 40, "pwsh", "CASCADIA_HOSTING_WINDOW_CLASS"),
        window(401, 40, "Beacon-M4-WT", "CASCADIA_HOSTING_WINDOW_CLASS"),
        window(402, 40, r"C:\work\gamma", "CASCADIA_HOSTING_WINDOW_CLASS"),
    ];
    let processes = terminal(42);
    assert_eq!(
        resolve(
            &session("s", 42, r"C:\work\gamma", "cli"),
            &processes,
            &windows
        )
        .map(|target| (target.hwnd, target.ambiguous)),
        Some((402, false))
    );
    let mut titled = session("s", 42, r"C:\work\other", "cli");
    titled.title = Some("beacon-m4-wt".to_string());
    assert_eq!(
        resolve(&titled, &processes, &windows).map(|target| (target.hwnd, target.ambiguous)),
        Some((401, false))
    );
    assert_eq!(
        resolve(
            &session("s", 42, r"C:\work\other", "cli"),
            &processes,
            &windows
        )
        .map(|target| (target.hwnd, target.ambiguous)),
        Some((400, true))
    );
}

#[test]
fn classic_consoles_resolve_through_conhost() {
    let mut processes = shell();
    processes.push(named(30, EXPLORER, "cmd.exe"));
    processes.push(named(31, 30, "conhost.exe"));
    processes.push(named(32, 30, "claude.exe"));
    processes.push(named(33, 32, "conhost.exe"));
    let windows = [
        window(500, 31, "Command Prompt - claude", "ConsoleWindowClass"),
        window(501, 30, "cmd helper", "Other"),
    ];
    assert_eq!(
        resolve(&session("s", 32, "", "cli"), &processes, &windows),
        target(500, "Command Prompt - claude", TargetKind::Console, false)
    );
}

#[test]
fn a_console_window_reported_under_its_shell_is_found_before_an_outer_host() {
    let mut processes = vscode("Code.exe", &[]);
    processes.push(named(30, 10, "cmd.exe"));
    processes.push(named(31, 30, "conhost.exe"));
    processes.push(named(32, 30, "claude.exe"));
    let windows = [
        editor_window(100, 10, "beacon - Visual Studio Code"),
        window(500, 30, "beacon-m4-console", "ConsoleWindowClass"),
    ];
    assert_eq!(
        resolve(
            &session("s", 32, r"C:\work\beacon", "cli"),
            &processes,
            &windows
        ),
        target(500, "beacon-m4-console", TargetKind::Console, false)
    );
}

#[test]
fn a_console_started_through_conhost_resolves_to_its_window() {
    let mut processes = shell();
    processes.push(named(60, EXPLORER, "conhost.exe"));
    processes.push(named(61, 60, "cmd.exe"));
    processes.push(named(62, 61, "claude.exe"));
    for owner in [60, 61] {
        let windows = [window(
            600,
            owner,
            "beacon-m4-console",
            "ConsoleWindowClass",
        )];
        assert_eq!(
            resolve(&session("s", 62, "", "cli"), &processes, &windows),
            target(600, "beacon-m4-console", TargetKind::Console, false),
            "{owner}"
        );
    }
}

#[test]
fn other_hosts_fall_back_to_the_nearest_ancestor_with_a_window() {
    let mut processes = shell();
    processes.push(named(80, EXPLORER, "mintty.exe"));
    processes.push(named(81, 80, "bash.exe"));
    processes.push(named(82, 81, "claude.exe"));
    processes.push(named(90, EXPLORER, "claude.exe"));
    let mut windows = shell_windows();
    windows.push(window(800, 80, "~/work", "mintty"));
    assert_eq!(
        resolve(&session("s", 82, "", "cli"), &processes, &windows),
        target(800, "~/work", TargetKind::Other, false)
    );
    assert_eq!(
        resolve(&session("t", 90, "", "cli"), &processes, &windows),
        None
    );
}

#[test]
fn missing_dead_and_ended_sessions_resolve_to_nothing() {
    let processes = vscode("Code.exe", &[20]);
    let windows = [editor_window(100, 10, "beacon")];
    let mut without_pid = session("s", 20, "", "claude-vscode");
    without_pid.claude_pid = None;
    assert_eq!(resolve(&without_pid, &processes, &windows), None);
    assert_eq!(
        resolve(
            &session("s", 12345, "", "claude-vscode"),
            &processes,
            &windows
        ),
        None
    );
    let mut ended = session("s", 20, "", "claude-vscode");
    ended.state = SessionState::Ended;
    assert_eq!(resolve(&ended, &processes, &windows), None);
}

#[test]
fn a_parent_cycle_terminates() {
    let processes = [
        named(1000, 1001, "a.exe"),
        named(1001, 1000, "b.exe"),
        named(1002, 1000, "claude.exe"),
        named(1003, 1003, "self.exe"),
    ];
    assert_eq!(
        resolve(&session("s", 1002, "", "cli"), &processes, &[]),
        None
    );
    assert_eq!(
        resolve(&session("s", 1003, "", "cli"), &processes, &[]),
        None
    );
}

#[test]
fn the_foreground_session_is_the_only_one_on_that_window() {
    let mut processes = terminal(42);
    processes.extend(vscode("Code.exe", &[20, 21]).into_iter().skip(1));
    let windows = [
        window(400, 40, "beacon", "CASCADIA_HOSTING_WINDOW_CLASS"),
        editor_window(100, 10, "beacon - Visual Studio Code"),
        window(700, 77, "Notes", "Notepad"),
    ];
    let sessions = vec![
        session("terminal", 42, r"C:\work\beacon", "cli"),
        session("code-a", 20, r"C:\work\beacon", "claude-vscode"),
        session("code-b", 21, r"C:\work\beacon", "claude-vscode"),
    ];
    assert_eq!(
        session_in_foreground(&sessions, &processes, &windows, 400),
        Some("terminal")
    );
    assert_eq!(
        session_in_foreground(&sessions, &processes, &windows, 100),
        None
    );
    assert_eq!(
        session_in_foreground(&sessions, &processes, &windows, 700),
        None
    );
    let mut ended = sessions.clone();
    ended[1].state = SessionState::Ended;
    assert_eq!(
        session_in_foreground(&ended, &processes, &windows, 100),
        Some("code-b")
    );
}

#[test]
fn links_open_vscode_sessions_with_an_encoded_prompt() {
    let editor = target(100, "beacon", VSCODE, false).unwrap();
    let in_extension = session("4c2c-35", 20, "", "claude-vscode");
    assert_eq!(
        session_link(&in_extension, &editor, None).as_deref(),
        Some("vscode://anthropic.claude-code/open?session=4c2c-35")
    );
    assert_eq!(
        session_link(&in_extension, &editor, Some("fix #2 & ship\nnow é ü")).as_deref(),
        Some(
            "vscode://anthropic.claude-code/open?session=4c2c-35&prompt=fix%20%232%20%26%20ship%0Anow%20%C3%A9%20%C3%BC"
        )
    );
    let in_terminal = session("s", 20, "", "cli");
    assert_eq!(session_link(&in_terminal, &editor, None), None);
    let console = target(500, "cmd", TargetKind::Console, false).unwrap();
    assert_eq!(session_link(&in_extension, &console, None), None);
}

#[test]
fn percent_encoding_keeps_only_unreserved_characters() {
    assert_eq!(percent_encode("aZ09-._~"), "aZ09-._~");
    assert_eq!(percent_encode("a b&c#d"), "a%20b%26c%23d");
    assert_eq!(percent_encode("line\r\nnext"), "line%0D%0Anext");
    assert_eq!(
        percent_encode("日本 😀"),
        "%E6%97%A5%E6%9C%AC%20%F0%9F%98%80"
    );
    assert_eq!(percent_encode("+/?=%"), "%2B%2F%3F%3D%25");
}

#[test]
fn host_identity_needs_at_least_one_variable() {
    assert_eq!(Host::from_env(&BTreeMap::new()), None);
    let env: BTreeMap<String, String> = [
        ("WT_SESSION", "8f1c"),
        ("TERM_PROGRAM", "vscode"),
        ("CLAUDE_PID", "42"),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_string(), value.to_string()))
    .collect();
    assert_eq!(
        Host::from_env(&env),
        Some(Host {
            wt_session: Some("8f1c".to_string()),
            term_program: Some("vscode".to_string()),
            ..Host::default()
        })
    );
}

#[test]
fn target_kinds_have_display_names() {
    assert_eq!(VSCODE.name(), "VS Code");
    assert_eq!(TargetKind::Editor { scheme: "cursor" }.name(), "Cursor");
    assert_eq!(TargetKind::WindowsTerminal.name(), "Windows Terminal");
    assert_eq!(
        serde_json::to_string(&JumpOutcome {
            result: JumpResult::NoWindow,
            kind: Some(VSCODE),
            ambiguous: true,
            link_opened: false,
        })
        .unwrap(),
        r#"{"result":"no_window","kind":{"type":"editor","scheme":"vscode"},"ambiguous":true,"link_opened":false}"#
    );
}

#[test]
fn a_process_descends_from_a_listed_ancestor_through_older_parents() {
    let processes = [
        named(10, 1, "claude.exe"),
        named(20, 10, "cmd.exe"),
        named(30, 20, "beacon.exe"),
    ];
    let created = |pid: u32| Some(u64::from(pid));
    assert!(descends_from(30, &[10], &processes, created));
    assert!(!descends_from(10, &[10], &processes, created));
    assert!(!descends_from(30, &[99], &processes, created));
    assert!(!descends_from(20, &[30], &processes, created));
}

#[test]
fn the_ancestry_walk_stops_at_a_parent_created_later_or_of_unknown_age() {
    let processes = [
        named(10, 1, "claude.exe"),
        named(20, 10, "cmd.exe"),
        named(30, 20, "beacon.exe"),
    ];
    let reused_parent = |pid: u32| Some(if pid == 20 { 900 } else { u64::from(pid) });
    assert!(!descends_from(30, &[10], &processes, reused_parent));
    assert!(!descends_from(30, &[10], &processes, |_| None));
}

#[test]
fn an_app_started_by_a_hook_whose_helper_has_exited_is_not_inside_the_session() {
    let with_helper = [
        named(10, 1, "claude.exe"),
        named(50, 10, "beacon-hook.exe"),
        named(60, 50, "beacon-app.exe"),
    ];
    let without_helper = [with_helper[0].clone(), with_helper[2].clone()];
    let created = |pid: u32| Some(u64::from(pid));
    assert!(descends_from(60, &[10], &with_helper, created));
    assert!(!descends_from(60, &[10], &without_helper, created));
}

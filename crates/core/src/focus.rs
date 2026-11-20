use crate::platform;
use crate::session::Session;
use crate::state::SessionState;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::io;
use std::path::Path;

const EDITORS: [(&str, &str, &str); 5] = [
    ("Code.exe", "vscode", "VS Code"),
    ("Code - Insiders.exe", "vscode-insiders", "VS Code Insiders"),
    ("Cursor.exe", "cursor", "Cursor"),
    ("Windsurf.exe", "windsurf", "Windsurf"),
    ("Antigravity.exe", "antigravity", "Antigravity"),
];
const DESKTOP_IMAGE: &str = "Claude.exe";
const DESKTOP_APP_DIR: &str = r"\anthropicclaude\";
const DESKTOP_CLI_DIR: &str = r"\appdata\roaming\claude\";
const DESKTOP_ENTRYPOINT: &str = "claude-desktop";
const VSCODE_ENTRYPOINT: &str = "claude-vscode";
const TERMINAL_IMAGE: &str = "WindowsTerminal.exe";
const TERMINAL_CLASS: &str = "CASCADIA_HOSTING_WINDOW_CLASS";
const CONSOLE_HOST_IMAGE: &str = "conhost.exe";
const CONSOLE_CLASS: &str = "ConsoleWindowClass";
const SHELL_IMAGE: &str = "explorer.exe";
const LINK_PATH: &str = "anthropic.claude-code/open";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Host {
    pub term_program: Option<String>,
    pub wt_session: Option<String>,
    pub wt_profile_id: Option<String>,
    pub vscode_pid: Option<String>,
    pub tmux: Option<String>,
    pub tmux_pane: Option<String>,
    pub kitty_window_id: Option<String>,
    pub wezterm_pane: Option<String>,
    pub wsl_distro_name: Option<String>,
}

impl Host {
    pub fn from_env(env: &BTreeMap<String, String>) -> Option<Host> {
        let read = |name: &str| env.get(name).cloned();
        let host = Host {
            term_program: read("TERM_PROGRAM"),
            wt_session: read("WT_SESSION"),
            wt_profile_id: read("WT_PROFILE_ID"),
            vscode_pid: read("VSCODE_PID"),
            tmux: read("TMUX"),
            tmux_pane: read("TMUX_PANE"),
            kitty_window_id: read("KITTY_WINDOW_ID"),
            wezterm_pane: read("WEZTERM_PANE"),
            wsl_distro_name: read("WSL_DISTRO_NAME"),
        };
        (host != Host::default()).then_some(host)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    pub pid: u32,
    pub parent_pid: u32,
    pub image_name: String,
    pub image_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub hwnd: isize,
    pub pid: u32,
    pub title: String,
    pub class: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TargetKind {
    Editor { scheme: &'static str },
    Desktop,
    WindowsTerminal,
    Console,
    Other,
}

impl TargetKind {
    pub fn name(self) -> &'static str {
        match self {
            TargetKind::Editor { scheme } => EDITORS
                .iter()
                .find(|(_, editor_scheme, _)| *editor_scheme == scheme)
                .map_or(scheme, |&(.., name)| name),
            TargetKind::Desktop => "Claude desktop app",
            TargetKind::WindowsTerminal => "Windows Terminal",
            TargetKind::Console => "console",
            TargetKind::Other => "parent process",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub hwnd: isize,
    pub title: String,
    pub kind: TargetKind,
    pub ambiguous: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JumpResult {
    Focused,
    Flashed,
    NoWindow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JumpOutcome {
    pub result: JumpResult,
    pub kind: Option<TargetKind>,
    pub ambiguous: bool,
    pub link_opened: bool,
}

pub fn resolve(session: &Session, processes: &[Process], windows: &[Window]) -> Option<Target> {
    if session.state == SessionState::Ended {
        return None;
    }
    let lineage = lineage(session.claude_pid?, processes);
    let (claude, ancestors) = lineage.split_first()?;
    lineage
        .iter()
        .find_map(|member| {
            hosting_target(member, ancestors, session, windows)
                .or_else(|| console_target(member, processes, windows))
        })
        .or_else(|| detached_desktop_target(session, claude, processes, windows))
        .or_else(|| {
            lineage
                .iter()
                .filter(|member| !member.image_name.eq_ignore_ascii_case(SHELL_IMAGE))
                .find_map(|member| pick(titled(windows, &[member.pid]), &[], TargetKind::Other))
        })
}

pub fn session_in_foreground<'a>(
    sessions: &'a [Session],
    processes: &[Process],
    windows: &[Window],
    foreground: isize,
) -> Option<&'a str> {
    let mut owners = sessions.iter().filter(|session| {
        resolve(session, processes, windows).is_some_and(|target| target.hwnd == foreground)
    });
    let owner = owners.next()?;
    owners.next().is_none().then_some(owner.session_id.as_str())
}

pub fn session_link(session: &Session, target: &Target, prompt: Option<&str>) -> Option<String> {
    let TargetKind::Editor { scheme } = target.kind else {
        return None;
    };
    (session.entrypoint.as_deref() == Some(VSCODE_ENTRYPOINT)).then(|| {
        let link = format!(
            "{scheme}://{LINK_PATH}?session={}",
            percent_encode(&session.session_id)
        );
        match prompt {
            Some(prompt) => format!("{link}&prompt={}", percent_encode(prompt)),
            None => link,
        }
    })
}

pub fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(byte).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

pub fn locate(session: &Session) -> io::Result<Option<Target>> {
    Ok(resolve(
        session,
        &platform::processes()?,
        &platform::top_level_windows(),
    ))
}

pub fn foreground_session(sessions: &[Session]) -> io::Result<Option<String>> {
    Ok(session_in_foreground(
        sessions,
        &platform::processes()?,
        &platform::top_level_windows(),
        platform::foreground_window(),
    )
    .map(str::to_string))
}

pub fn jump(session: &Session) -> io::Result<JumpOutcome> {
    jump_to(session, None)
}

pub fn jump_with_prompt(session: &Session, prompt: &str) -> io::Result<JumpOutcome> {
    jump_to(session, Some(prompt))
}

fn jump_to(session: &Session, prompt: Option<&str>) -> io::Result<JumpOutcome> {
    let Some(target) = locate(session)? else {
        return Ok(JumpOutcome {
            result: JumpResult::NoWindow,
            kind: None,
            ambiguous: false,
            link_opened: false,
        });
    };
    let focused = platform::bring_to_front(target.hwnd);
    if !focused {
        platform::flash(target.hwnd);
    }
    Ok(JumpOutcome {
        result: if focused {
            JumpResult::Focused
        } else {
            JumpResult::Flashed
        },
        kind: Some(target.kind),
        ambiguous: target.ambiguous,
        link_opened: focused
            && session_link(session, &target, prompt)
                .is_some_and(|link| platform::open_link(&link).is_ok()),
    })
}

fn lineage(pid: u32, processes: &[Process]) -> Vec<&Process> {
    let mut seen = HashSet::new();
    std::iter::successors(
        processes.iter().find(|process| process.pid == pid),
        |child| {
            processes
                .iter()
                .find(|process| process.pid == child.parent_pid)
        },
    )
    .take_while(|process| seen.insert(process.pid))
    .collect()
}

fn hosting_target(
    member: &Process,
    ancestors: &[&Process],
    session: &Session,
    windows: &[Window],
) -> Option<Target> {
    let topmost = |matches: &dyn Fn(&Process) -> bool| {
        ancestors
            .iter()
            .rev()
            .find(|process| matches(process))
            .map(|process| process.pid)
    };
    if let Some(&(image, scheme, _)) = EDITORS
        .iter()
        .find(|(image, ..)| member.image_name.eq_ignore_ascii_case(image))
    {
        let main = topmost(&|process| process.image_name.eq_ignore_ascii_case(image))?;
        return pick(
            titled(windows, &[main]),
            &[cwd_leaf(session), session.project.as_deref()],
            TargetKind::Editor { scheme },
        );
    }
    if is_desktop_app(member) {
        let main = topmost(&is_desktop_app)?;
        return pick(titled(windows, &[main]), &[], TargetKind::Desktop);
    }
    if member.image_name.eq_ignore_ascii_case(TERMINAL_IMAGE) {
        return pick(
            windows
                .iter()
                .filter(|window| window.pid == member.pid && window.class == TERMINAL_CLASS),
            &[cwd_leaf(session), session.title.as_deref()],
            TargetKind::WindowsTerminal,
        );
    }
    None
}

fn detached_desktop_target(
    session: &Session,
    claude: &Process,
    processes: &[Process],
    windows: &[Window],
) -> Option<Target> {
    if session.entrypoint.as_deref() != Some(DESKTOP_ENTRYPOINT)
        || !path_contains(&claude.image_path, DESKTOP_CLI_DIR)
    {
        return None;
    }
    let apps: Vec<u32> = processes
        .iter()
        .filter(|process| path_contains(&process.image_path, DESKTOP_APP_DIR))
        .map(|process| process.pid)
        .collect();
    pick(titled(windows, &apps), &[], TargetKind::Desktop)
}

fn console_target(member: &Process, processes: &[Process], windows: &[Window]) -> Option<Target> {
    let owners: Vec<u32> = processes
        .iter()
        .filter(|process| {
            process.parent_pid == member.pid
                && process.image_name.eq_ignore_ascii_case(CONSOLE_HOST_IMAGE)
        })
        .map(|process| process.pid)
        .chain([member.pid])
        .collect();
    pick(
        windows
            .iter()
            .filter(|window| window.class == CONSOLE_CLASS && owners.contains(&window.pid)),
        &[],
        TargetKind::Console,
    )
}

fn is_desktop_app(process: &Process) -> bool {
    process.image_name == DESKTOP_IMAGE || path_contains(&process.image_path, DESKTOP_APP_DIR)
}

fn path_contains(path: &str, marker: &str) -> bool {
    path.to_lowercase().contains(marker)
}

fn cwd_leaf(session: &Session) -> Option<&str> {
    Path::new(session.cwd.as_deref()?).file_name()?.to_str()
}

fn titled<'a>(windows: &'a [Window], owners: &[u32]) -> impl Iterator<Item = &'a Window> {
    windows
        .iter()
        .filter(|window| !window.title.is_empty() && owners.contains(&window.pid))
}

fn pick<'a>(
    candidates: impl Iterator<Item = &'a Window>,
    needles: &[Option<&str>],
    kind: TargetKind,
) -> Option<Target> {
    let candidates: Vec<&Window> = candidates.collect();
    let matched = needles
        .iter()
        .flatten()
        .filter(|needle| !needle.is_empty())
        .find_map(|needle| {
            let needle = needle.to_lowercase();
            candidates
                .iter()
                .find(|window| window.title.to_lowercase().contains(&needle))
        });
    let (window, ambiguous) = match (matched, candidates.as_slice()) {
        (Some(window), _) => (*window, false),
        (None, [only]) => (*only, false),
        (None, [first, ..]) => (*first, true),
        (None, []) => return None,
    };
    Some(Target {
        hwnd: window.hwnd,
        title: window.title.clone(),
        kind,
        ambiguous,
    })
}

#[cfg(test)]
mod tests;

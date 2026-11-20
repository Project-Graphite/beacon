use crate::{Context, request, resolve_session};
use beacon_core::focus::{self, JumpOutcome, JumpResult, Target, TargetKind};
use beacon_core::{Request, Session};
use std::path::Path;
use std::process::ExitCode;

const NO_WINDOW: &str = "No window found for that session";

pub fn jump(home: &Path, prefix: &str) -> Result<ExitCode, String> {
    let outcome = focus::jump(&session(home, prefix)?).context("cannot look for the window")?;
    println!("{}", describe_jump(&outcome));
    Ok(exit_code(outcome.result != JumpResult::NoWindow))
}

pub fn locate(home: &Path, prefix: &str) -> Result<ExitCode, String> {
    let session = session(home, prefix)?;
    let target = focus::locate(&session).context("cannot look for the window")?;
    println!("{}", describe_target(&session, target.as_ref()));
    Ok(exit_code(target.is_some()))
}

fn exit_code(found: bool) -> ExitCode {
    if found {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn session(home: &Path, prefix: &str) -> Result<Session, String> {
    let session_id = resolve_session(home, prefix)?;
    request(home, &Request::Snapshot)?
        .snapshot
        .ok_or("the Beacon core sent no snapshot")?
        .sessions
        .into_iter()
        .find(|session| session.session_id == session_id)
        .ok_or_else(|| format!("session {session_id} is no longer known"))
}

fn describe_jump(outcome: &JumpOutcome) -> String {
    let mut line = match outcome.result {
        JumpResult::Focused => format!(
            "Focused {} window",
            outcome.kind.map_or("its", TargetKind::name)
        ),
        JumpResult::Flashed => {
            "Window found but Windows refused the focus change; its taskbar button is flashing"
                .to_string()
        }
        JumpResult::NoWindow => return NO_WINDOW.to_string(),
    };
    if outcome.link_opened {
        line.push_str(" and opened the session tab");
    }
    if outcome.ambiguous {
        line.push_str("; several windows could hold the session, so it took the first");
    }
    line
}

fn describe_target(session: &Session, target: Option<&Target>) -> String {
    let Some(target) = target else {
        return NO_WINDOW.to_string();
    };
    format!(
        "Kind: {:?}\nWindow: {}\nAmbiguous: {}\nLink: {}",
        target.kind,
        target.title,
        if target.ambiguous { "yes" } else { "no" },
        focus::session_link(session, target, None).unwrap_or_else(|| "none".to_string())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(result: JumpResult, kind: Option<TargetKind>) -> JumpOutcome {
        JumpOutcome {
            result,
            kind,
            ambiguous: false,
            link_opened: false,
        }
    }

    #[test]
    fn jump_outcomes_read_as_one_line() {
        let vscode = Some(TargetKind::Editor { scheme: "vscode" });
        assert_eq!(
            describe_jump(&outcome(JumpResult::Focused, vscode)),
            "Focused VS Code window"
        );
        assert_eq!(
            describe_jump(&outcome(JumpResult::Focused, Some(TargetKind::Console))),
            "Focused console window"
        );
        assert_eq!(
            describe_jump(&JumpOutcome {
                ambiguous: true,
                link_opened: true,
                ..outcome(JumpResult::Focused, vscode)
            }),
            "Focused VS Code window and opened the session tab; several windows could hold the session, so it took the first"
        );
        assert_eq!(
            describe_jump(&outcome(JumpResult::Flashed, vscode)),
            "Window found but Windows refused the focus change; its taskbar button is flashing"
        );
        assert_eq!(
            describe_jump(&outcome(JumpResult::NoWindow, None)),
            "No window found for that session"
        );
    }

    #[test]
    fn where_reports_the_kind_title_and_link() {
        let mut session = Session::new("4c2c", 0);
        session.entrypoint = Some("claude-vscode".to_string());
        let editor = Target {
            hwnd: 1,
            title: "beacon - Visual Studio Code".to_string(),
            kind: TargetKind::Editor { scheme: "vscode" },
            ambiguous: true,
        };
        assert_eq!(
            describe_target(&session, Some(&editor)),
            "Kind: Editor { scheme: \"vscode\" }\nWindow: beacon - Visual Studio Code\nAmbiguous: yes\nLink: vscode://anthropic.claude-code/open?session=4c2c"
        );
        let console = Target {
            title: "beacon-m4-console".to_string(),
            kind: TargetKind::Console,
            ambiguous: false,
            ..editor
        };
        assert_eq!(
            describe_target(&session, Some(&console)),
            "Kind: Console\nWindow: beacon-m4-console\nAmbiguous: no\nLink: none"
        );
        assert_eq!(describe_target(&session, None), NO_WINDOW);
    }
}

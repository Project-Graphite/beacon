mod args;
mod jump;
mod platform;
mod status;
mod usage;

use args::Command;
use beacon_cli::Context;
use beacon_cli::doctor;
use beacon_cli::installer::{self, Paths};
use beacon_core::{
    AccountSources, ClaudeAgentsLister, ClaudeUsageProber, Config, ControlClient, Core,
    CoreOptions, Reply, Request, claude_json_path, home_dir, now_ms,
};
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::OnceLock;

static DAEMON_HOME: OnceLock<PathBuf> = OnceLock::new();

fn main() -> ExitCode {
    let parsed = std::env::args_os()
        .skip(1)
        .map(|arg| {
            arg.into_string()
                .map_err(|arg| format!("argument is not valid Unicode: {}", arg.to_string_lossy()))
        })
        .collect::<Result<Vec<String>, String>>()
        .and_then(|args| args::parse(&args));
    let command = match parsed {
        Ok(command) => command,
        Err(message) => {
            eprintln!("beacon: {message}\n\n{}", args::USAGE);
            return ExitCode::FAILURE;
        }
    };
    match run(command) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("beacon: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<ExitCode, String> {
    match command {
        Command::Help => {
            print!("{}", args::USAGE);
            Ok(ExitCode::SUCCESS)
        }
        Command::Daemon => daemon(),
        Command::Stop => {
            request(&beacon_home()?, &Request::Shutdown)?;
            println!("Beacon core stopped.");
            Ok(ExitCode::SUCCESS)
        }
        Command::Status { watch, json, all } => status::run(&beacon_home()?, watch, json, all),
        Command::Decide { id, decision } => {
            request(&beacon_home()?, &Request::Decide { id, decision })?;
            println!("Decision {id} sent.");
            Ok(ExitCode::SUCCESS)
        }
        Command::Ack { session } => acknowledge(&beacon_home()?, &session),
        Command::Dismiss { session } => dismiss(&beacon_home()?, &session),
        Command::Jump { session } => jump::jump(&beacon_home()?, &session),
        Command::Where { session } => jump::locate(&beacon_home()?, &session),
        Command::Prompt { session, text } => queue_prompt(&beacon_home()?, &session, text),
        Command::Snooze { session, minutes } => snooze(&beacon_home()?, session, Some(minutes)),
        Command::Unsnooze { session } => snooze(&beacon_home()?, session, None),
        Command::Usage { refresh } => {
            let home = beacon_home()?;
            if refresh {
                request(&home, &Request::RefreshUsage)?;
            }
            let snapshot = request(&home, &Request::Snapshot)?
                .snapshot
                .ok_or("the Beacon core sent no snapshot")?;
            print!("{}", usage::render(&snapshot, now_ms()));
            Ok(ExitCode::SUCCESS)
        }
        Command::Install { settings, yes } => {
            print!("{}", installer::install(&Paths::resolve(settings)?, yes)?);
            Ok(ExitCode::SUCCESS)
        }
        Command::Uninstall { settings, yes } => {
            print!("{}", installer::uninstall(&Paths::resolve(settings)?, yes)?);
            Ok(ExitCode::SUCCESS)
        }
        Command::Doctor { settings } => {
            let (report, healthy) = doctor::run(&Paths::resolve(settings)?);
            print!("{report}");
            Ok(if healthy {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
    }
}

fn beacon_home() -> Result<PathBuf, String> {
    home_dir()
        .ok_or_else(|| "cannot locate the Beacon home: set BEACON_HOME or LOCALAPPDATA".to_string())
}

pub fn connect(home: &Path) -> Result<ControlClient, String> {
    ControlClient::connect(home).map_err(|error| {
        format!(
            "no Beacon core is running for {} ({error}); start one with `beacon daemon`",
            home.display()
        )
    })
}

fn request(home: &Path, request: &Request) -> Result<Reply, String> {
    let reply = connect(home)?
        .request(request)
        .context("the Beacon core did not answer")?;
    if !reply.ok {
        return Err(reply
            .error
            .unwrap_or_else(|| "the Beacon core refused the request".to_string()));
    }
    Ok(reply)
}

fn resolve_session(home: &Path, prefix: &str) -> Result<String, String> {
    let snapshot = request(home, &Request::Snapshot)?
        .snapshot
        .ok_or("the Beacon core sent no snapshot")?;
    let ids: Vec<&str> = snapshot
        .sessions
        .iter()
        .map(|session| session.session_id.as_str())
        .collect();
    let matching: Vec<&str> = match ids.iter().find(|id| **id == prefix) {
        Some(exact) => vec![*exact],
        None => ids
            .into_iter()
            .filter(|id| id.starts_with(prefix))
            .collect(),
    };
    match matching.as_slice() {
        [session_id] => Ok(session_id.to_string()),
        [] => Err(format!("no session id starts with {prefix}")),
        several => Err(format!(
            "{prefix} matches {} sessions: {}",
            several.len(),
            several.join(", ")
        )),
    }
}

fn acknowledge(home: &Path, prefix: &str) -> Result<ExitCode, String> {
    let session_id = resolve_session(home, prefix)?;
    request(
        home,
        &Request::Acknowledge {
            session_id: session_id.clone(),
        },
    )?;
    println!("Session {session_id} acknowledged.");
    Ok(ExitCode::SUCCESS)
}

fn dismiss(home: &Path, prefix: &str) -> Result<ExitCode, String> {
    let session_id = resolve_session(home, prefix)?;
    request(
        home,
        &Request::Dismiss {
            session_id: session_id.clone(),
        },
    )?;
    println!("Session {session_id} crossed off until it does something again.");
    Ok(ExitCode::SUCCESS)
}

fn queue_prompt(home: &Path, prefix: &str, text: String) -> Result<ExitCode, String> {
    let session_id = resolve_session(home, prefix)?;
    request(
        home,
        &Request::QueuePrompt {
            session_id: session_id.clone(),
            text,
        },
    )?;
    println!("Prompt queued for session {session_id}; it is sent when the current turn finishes.");
    Ok(ExitCode::SUCCESS)
}

fn snooze(home: &Path, session: Option<String>, minutes: Option<u64>) -> Result<ExitCode, String> {
    let session_id = session
        .map(|prefix| resolve_session(home, &prefix))
        .transpose()?;
    let (message, snooze_request) = match (minutes, &session_id) {
        (Some(minutes), Some(id)) => (
            format!("Session {id} snoozed for {minutes} minutes."),
            Request::Snooze {
                session_id,
                minutes,
            },
        ),
        (Some(minutes), None) => (
            format!("All sessions snoozed for {minutes} minutes."),
            Request::Snooze {
                session_id,
                minutes,
            },
        ),
        (None, Some(id)) => (
            format!("Session {id} unsnoozed."),
            Request::Unsnooze { session_id },
        ),
        (None, None) => (
            "The global snooze is cleared; session snoozes are kept.".to_string(),
            Request::Unsnooze { session_id },
        ),
    };
    request(home, &snooze_request)?;
    println!("{message}");
    Ok(ExitCode::SUCCESS)
}

fn daemon() -> Result<ExitCode, String> {
    let home = beacon_home()?;
    let config = Config::load(&home).context("cannot read the Beacon config")?;
    DAEMON_HOME
        .set(home.clone())
        .map_err(|_| "the daemon was started twice".to_string())?;
    platform::on_console_interrupt(request_shutdown)
        .context("cannot install the Ctrl+C handler")?;
    let core = match Core::start_with(
        CoreOptions {
            home: home.clone(),
            lister: Box::new(ClaudeAgentsLister),
            config,
        },
        AccountSources {
            claude_json: claude_json_path(),
            prober: Some(Box::new(ClaudeUsageProber)),
        },
    ) {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Err(error.to_string()),
        started => started.context("cannot start the Beacon core")?,
    };
    println!(
        "Beacon core running for {} on {}. Press Ctrl+C to stop.",
        home.display(),
        core.endpoint().pipe_name
    );
    core.wait().context("the Beacon core failed")?;
    println!("Beacon core stopped.");
    Ok(ExitCode::SUCCESS)
}

fn request_shutdown() -> bool {
    DAEMON_HOME.get().is_some_and(|home| {
        ControlClient::connect(home)
            .and_then(|mut client| client.request(&Request::Shutdown))
            .is_ok_and(|reply| reply.ok)
    })
}

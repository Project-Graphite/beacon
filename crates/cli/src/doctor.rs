use crate::installer::{Paths, REGISTRY_NOTICE, managed_policy_findings};
use crate::settings::{self, HANDLERS};
use crate::surface_label;
use beacon_core::{ControlClient, Endpoint, Request, Snapshot};
use beacon_protocol::{DOCTOR_EVENT, HOME_ENV};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

const DOCTOR_REPLY: &[u8] = br#"{"beacon":"ok"}"#;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Level {
    Ok,
    Warn,
    Fail,
}

pub type Check = (Level, String);

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HooksState {
    Installed,
    NotInstalled,
    NeedsRepair,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HooksStatus {
    pub state: HooksState,
    pub settings_path: PathBuf,
    pub problems: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn hooks_status(paths: &Paths) -> HooksStatus {
    let helper = paths.helper();
    let settings = match std::fs::read(&paths.settings) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Map::new()),
        read => parse_settings(&paths.settings, read),
    };
    let present = helper.is_file() || settings.as_ref().is_ok_and(settings::has_beacon);
    let mut checks = Vec::new();
    if present {
        checks.push(helper_check(&helper));
    }
    match &settings {
        Ok(settings) if present => {
            checks.extend(settings_checks(settings, &paths.settings, &helper))
        }
        Ok(settings) => checks.push(disable_all_hooks_check(settings, &paths.settings)),
        Err(check) => checks.push(check.clone()),
    }
    checks.extend(managed_policy_checks(&paths.managed_dir));
    let texts = |wanted: Level| -> Vec<String> {
        checks
            .iter()
            .filter(|(level, _)| *level == wanted)
            .map(|(_, text)| text.clone())
            .collect()
    };
    let problems = texts(Level::Fail);
    HooksStatus {
        state: match (present, problems.is_empty()) {
            (false, _) => HooksState::NotInstalled,
            (true, true) => HooksState::Installed,
            (true, false) => HooksState::NeedsRepair,
        },
        settings_path: paths.settings.clone(),
        problems,
        warnings: texts(Level::Warn),
    }
}

pub fn installation_checks(paths: &Paths) -> Vec<Check> {
    let helper = paths.helper();
    let mut checks = vec![helper_check(&helper)];
    match parse_settings(&paths.settings, std::fs::read(&paths.settings)) {
        Ok(settings) => checks.extend(settings_checks(&settings, &paths.settings, &helper)),
        Err(check) => checks.push(check),
    }
    checks.extend(managed_policy_checks(&paths.managed_dir));
    checks
}

pub fn run(paths: &Paths) -> (String, bool) {
    let helper = paths.helper();
    let mut checks = installation_checks(paths);
    let (core, snapshot) = core_check(&paths.home);
    checks.push(core);
    checks.push(round_trip(&paths.home, &helper));
    checks.push(match find_on_path("claude.exe") {
        Some(claude) => (Level::Ok, format!("claude: {}", claude.display())),
        None => (
            Level::Warn,
            "claude: claude.exe is not on PATH, so sessions cannot be reconciled".to_string(),
        ),
    });
    checks.push(surfaces(snapshot.as_ref()));
    let report = checks
        .iter()
        .map(|(level, text)| {
            let label = match level {
                Level::Ok => "ok",
                Level::Warn => "warn",
                Level::Fail => "fail",
            };
            format!("{label:<4} {text}\n")
        })
        .collect();
    let healthy = checks.iter().all(|(level, _)| *level != Level::Fail);
    (report, healthy)
}

fn helper_check(helper: &Path) -> Check {
    if helper.is_file() {
        (Level::Ok, format!("helper: {}", helper.display()))
    } else {
        (
            Level::Fail,
            format!("helper: missing at {}", helper.display()),
        )
    }
}

fn parse_settings(path: &Path, read: io::Result<Vec<u8>>) -> Result<Map<String, Value>, Check> {
    read.map_err(|error| format!("cannot read {}: {error}", path.display()))
        .and_then(|bytes| {
            settings::parse(&bytes).map_err(|error| format!("{}: {error}", path.display()))
        })
        .map(|(settings, _)| settings)
        .map_err(|error| (Level::Fail, format!("settings: {error}")))
}

fn settings_checks(settings: &Map<String, Value>, path: &Path, helper: &Path) -> Vec<Check> {
    let problems = settings::registration_problems(settings, &helper.to_string_lossy());
    vec![
        if problems.is_empty() {
            (
                Level::Ok,
                format!(
                    "hooks: all {} handlers registered in {}",
                    HANDLERS.len(),
                    path.display()
                ),
            )
        } else {
            (Level::Fail, format!("hooks: {}", problems.join("; ")))
        },
        disable_all_hooks_check(settings, path),
    ]
}

fn disable_all_hooks_check(settings: &Map<String, Value>, path: &Path) -> Check {
    if settings.get("disableAllHooks") == Some(&Value::Bool(true)) {
        (
            Level::Fail,
            format!(
                "disableAllHooks: true in {}, so no hook runs",
                path.display()
            ),
        )
    } else {
        (Level::Ok, "disableAllHooks: not set".to_string())
    }
}

fn managed_policy_checks(managed_dir: &Path) -> Vec<Check> {
    let findings = managed_policy_findings(managed_dir);
    findings
        .is_empty()
        .then(|| {
            (
                Level::Ok,
                "managed policy: no managed settings file sets allowManagedHooksOnly".to_string(),
            )
        })
        .into_iter()
        .chain(
            findings
                .into_iter()
                .map(|finding| (Level::Fail, format!("managed policy: {finding}"))),
        )
        .chain([(Level::Warn, format!("managed policy: {REGISTRY_NOTICE}"))])
        .collect()
}

fn core_check(home: &Path) -> (Check, Option<Snapshot>) {
    let Some(endpoint) = Endpoint::read(home) else {
        return (
            (
                Level::Fail,
                format!(
                    "core: no endpoint file in {}; start `beacon daemon`",
                    home.display()
                ),
            ),
            None,
        );
    };
    match ControlClient::connect(home).and_then(|mut client| client.request(&Request::Snapshot)) {
        Ok(reply) => match reply.snapshot {
            Some(snapshot) => (
                (
                    Level::Ok,
                    format!("core: reachable on {}", endpoint.pipe_name),
                ),
                Some(snapshot),
            ),
            None => (
                (
                    Level::Fail,
                    format!("core: no snapshot: {}", reply.error.unwrap_or_default()),
                ),
                None,
            ),
        },
        Err(error) => (
            (
                Level::Fail,
                format!("core: the endpoint file exists but the core is unreachable: {error}"),
            ),
            None,
        ),
    }
}

fn round_trip(home: &Path, helper: &Path) -> Check {
    if !helper.is_file() {
        return (
            Level::Fail,
            "round trip: not possible without the helper".to_string(),
        );
    }
    let started = Instant::now();
    let output = run_helper(home, helper);
    let elapsed_ms = started.elapsed().as_millis();
    match output {
        Ok(output) if output.stdout == DOCTOR_REPLY => (
            Level::Ok,
            format!("round trip: {elapsed_ms} ms through {}", helper.display()),
        ),
        Ok(output) => (
            Level::Fail,
            format!(
                "round trip: expected {} but the helper printed {:?} after {elapsed_ms} ms",
                String::from_utf8_lossy(DOCTOR_REPLY),
                String::from_utf8_lossy(&output.stdout)
            ),
        ),
        Err(error) => (
            Level::Fail,
            format!("round trip: cannot run the helper: {error}"),
        ),
    }
}

fn run_helper(home: &Path, helper: &Path) -> io::Result<Output> {
    let mut child = Command::new(helper)
        .arg(DOCTOR_EVENT)
        .env(HOME_ENV, home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("the helper has no stdin"))?
        .write_all(b"{}")?;
    child.wait_with_output()
}

fn find_on_path(file_name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|directory| directory.join(file_name))
        .find(|candidate| candidate.is_file())
}

fn surfaces(snapshot: Option<&Snapshot>) -> Check {
    let Some(snapshot) = snapshot else {
        return (
            Level::Warn,
            "surfaces: unknown without a reachable core".to_string(),
        );
    };
    if snapshot.sessions.is_empty() {
        return (Level::Ok, "surfaces: no sessions seen yet".to_string());
    }
    let mut counts: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for session in &snapshot.sessions {
        let (seen, limited) = counts
            .entry(surface_label(session.entrypoint.as_deref()))
            .or_default();
        *seen += 1;
        *limited += usize::from(session.limited);
    }
    let summary = counts
        .iter()
        .map(|(surface, (seen, limited))| format!("{surface} {seen} seen, {limited} limited"))
        .collect::<Vec<_>>()
        .join("; ");
    if counts.values().all(|(_, limited)| *limited == 0) {
        (Level::Ok, format!("surfaces: {summary}"))
    } else {
        (
            Level::Warn,
            format!(
                "surfaces: {summary} (limited sessions are known only from `claude agents --json`; hooks have not fired for them)"
            ),
        )
    }
}

#[cfg(test)]
mod tests;

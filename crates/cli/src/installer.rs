use crate::settings::{self, Created, HELPER_FILE_NAME, TextFormat};
use crate::{Context, diff};
use beacon_core::{home_dir, now_ms, utc_timestamp};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

const MANAGED_DIR: &str = r"C:\Program Files\ClaudeCode";
pub const REGISTRY_NOTICE: &str = "the registry-delivered managed policy is not checked; if it sets allowManagedHooksOnly, Beacon cannot work";

pub struct Paths {
    pub settings: PathBuf,
    pub home: PathBuf,
    pub helper_source: PathBuf,
    pub managed_dir: PathBuf,
}

impl Paths {
    pub fn resolve(settings: Option<PathBuf>) -> Result<Paths, String> {
        let from_env = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
        let settings = settings
            .or_else(|| {
                from_env("CLAUDE_CONFIG_DIR").map(|dir| PathBuf::from(dir).join("settings.json"))
            })
            .or_else(|| {
                from_env("USERPROFILE")
                    .map(|profile| PathBuf::from(profile).join(".claude").join("settings.json"))
            })
            .ok_or("cannot locate the Claude Code settings: pass --settings <path>")?;
        Ok(Paths {
            settings,
            home: home_dir()
                .ok_or("cannot locate the Beacon home: set BEACON_HOME or LOCALAPPDATA")?,
            helper_source: std::env::current_exe()
                .context("cannot locate the running beacon.exe")?
                .with_file_name(HELPER_FILE_NAME),
            managed_dir: PathBuf::from(MANAGED_DIR),
        })
    }

    pub fn helper(&self) -> PathBuf {
        self.bin().join(HELPER_FILE_NAME)
    }

    fn record(&self) -> PathBuf {
        self.home.join("install.json")
    }

    fn bin(&self) -> PathBuf {
        self.home.join("bin")
    }

    fn backups(&self) -> PathBuf {
        self.home.join("backups")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct InstallRecord {
    settings_path: PathBuf,
    existed_before: bool,
    backup_path: Option<PathBuf>,
    installed_copy_path: PathBuf,
    helper_path: PathBuf,
    created: Created,
    installed_at: String,
}

pub fn install(paths: &Paths, apply: bool) -> Result<String, String> {
    if !paths.helper_source.is_file() {
        return Err(format!(
            "the helper was not found next to beacon.exe: {}",
            paths.helper_source.display()
        ));
    }
    let current = read_optional(&paths.settings)?;
    let (mut settings, format) = parsed(paths, current.as_deref())?;
    let helper = paths.helper();
    let helper_text = helper.to_str().ok_or_else(|| {
        format!(
            "the Beacon home is not valid Unicode: {}",
            paths.home.display()
        )
    })?;
    let hooks_disabled = settings.get("disableAllHooks") == Some(&Value::Bool(true));
    let original = settings.clone();
    settings::strip_beacon(&mut settings, |_| true);
    let created =
        settings::add_beacon(&mut settings, helper_text).context(paths.settings.display())?;
    let installed = match &current {
        Some(bytes) if settings::beacon_groups(&original) == settings::beacon_groups(&settings) => {
            bytes.clone()
        }
        _ => format.write(&settings),
    };
    let mut report = String::new();
    let _ = writeln!(
        report,
        "Settings: {}{}",
        paths.settings.display(),
        if current.is_none() {
            " (missing; it will be created)"
        } else {
            ""
        }
    );
    let _ = writeln!(report, "Helper:   {}", paths.helper_source.display());
    let _ = writeln!(report, "  copied to {}", helper.display());
    report.push_str(&changes(paths, current.as_deref(), Some(&installed)));
    let mut warnings = managed_policy_findings(&paths.managed_dir);
    if hooks_disabled {
        warnings.insert(
            0,
            "disableAllHooks is true in this file, so no hook runs, Beacon's included".to_string(),
        );
    }
    warnings.push(REGISTRY_NOTICE.to_string());
    for warning in warnings {
        let _ = writeln!(report, "Warning: {warning}");
    }
    if !apply {
        report.push_str("Nothing was changed. Run again with --yes to install.\n");
        return Ok(report);
    }
    let stamp = backup_stamp();
    let previous =
        read_record(paths)?.filter(|record| same_path(&record.settings_path, &paths.settings));
    std::fs::create_dir_all(paths.bin()).context("cannot create the helper directory")?;
    remove_retired_helpers(&paths.bin());
    std::fs::create_dir_all(paths.backups()).context("cannot create the backups directory")?;
    let staged = helper.with_extension("exe.tmp");
    std::fs::copy(&paths.helper_source, &staged).context(format_args!(
        "cannot copy the helper to {}",
        staged.display()
    ))?;
    if let Err(failure) = std::fs::rename(&staged, &helper) {
        retire_helper(&helper, failure)?;
        std::fs::rename(&staged, &helper)
            .context(format_args!("cannot replace {}", helper.display()))?;
    }
    let (existed_before, backup_path, created) = match previous {
        Some(previous) if std::fs::read(&previous.installed_copy_path).ok() == current => (
            previous.existed_before,
            previous.backup_path,
            previous.created,
        ),
        Some(_) if settings::has_beacon(&original) => (true, None, created),
        None if current.as_ref() == Some(&installed) => (true, None, created),
        _ => {
            let backup_path = match &current {
                Some(bytes) => {
                    let path = paths.backups().join(format!("settings-{stamp}.json"));
                    write_file(&path, bytes)?;
                    Some(path)
                }
                None => None,
            };
            (current.is_some(), backup_path, created)
        }
    };
    if current.as_ref() != Some(&installed) {
        write_atomic(&paths.settings, &installed)?;
    }
    let installed_copy_path = paths
        .backups()
        .join(format!("settings-{stamp}.installed.json"));
    write_file(&installed_copy_path, &installed)?;
    let record = InstallRecord {
        settings_path: paths.settings.clone(),
        existed_before,
        backup_path,
        installed_copy_path,
        helper_path: helper,
        created,
        installed_at: utc_timestamp(now_ms()),
    };
    let record_bytes = serde_json::to_vec_pretty(&record).context("cannot encode install.json")?;
    write_atomic(&paths.record(), &record_bytes)?;
    report.push_str("Installed.\n");
    Ok(report)
}

pub fn uninstall(paths: &Paths, apply: bool) -> Result<String, String> {
    let record = read_record(paths)?;
    if let Some(record) = &record
        && !same_path(&record.settings_path, &paths.settings)
    {
        return Err(format!(
            "install.json records {}; pass --settings with that path",
            record.settings_path.display()
        ));
    }
    let current = read_optional(&paths.settings)?;
    let restored = record.as_ref().and_then(|record| {
        let unchanged =
            current.is_some() && std::fs::read(&record.installed_copy_path).ok() == current;
        match (unchanged, &record.backup_path) {
            (false, _) => None,
            (true, _) if !record.existed_before => Some(None),
            (true, Some(backup)) => std::fs::read(backup).ok().map(Some),
            (true, None) => None,
        }
    });
    let target = match (restored, &current) {
        (Some(original), _) => original,
        (None, Some(bytes)) => {
            let (mut settings, format) = parsed(paths, Some(bytes))?;
            let before = settings.clone();
            let created = record
                .as_ref()
                .map(|record| record.created.clone())
                .unwrap_or_default();
            settings::strip_beacon(&mut settings, |container| created.includes(container));
            Some(if settings == before {
                bytes.clone()
            } else {
                format.write(&settings)
            })
        }
        (None, None) => None,
    };
    let helper = paths.helper();
    let settings_change = target != current;
    if record.is_none() && !settings_change && !helper.exists() {
        return Ok(format!(
            "Beacon is not installed for {}: nothing to do.\n",
            paths.settings.display()
        ));
    }
    let mut report = format!("Settings: {}\n", paths.settings.display());
    report.push_str(&changes(paths, current.as_deref(), target.as_deref()));
    if helper.exists() {
        let _ = writeln!(report, "Helper:   {} will be deleted", helper.display());
    }
    if record.is_some() {
        let _ = writeln!(
            report,
            "Record:   {} will be deleted",
            paths.record().display()
        );
    }
    if !apply {
        report.push_str("Nothing was changed. Run again with --yes to uninstall.\n");
        return Ok(report);
    }
    if settings_change {
        if let Some(bytes) = &current {
            std::fs::create_dir_all(paths.backups())
                .context("cannot create the backups directory")?;
            write_file(
                &paths
                    .backups()
                    .join(format!("settings-{}.before-uninstall.json", backup_stamp())),
                bytes,
            )?;
        }
        match &target {
            Some(bytes) => write_atomic(&paths.settings, bytes)?,
            None => std::fs::remove_file(&paths.settings)
                .context(format_args!("cannot delete {}", paths.settings.display()))?,
        }
    }
    remove_retired_helpers(&paths.bin());
    match std::fs::remove_file(&helper) {
        Err(failure) if failure.kind() != io::ErrorKind::NotFound => {
            retire_helper(&helper, failure)?
        }
        _ => {}
    }
    remove_if_present(&paths.record())?;
    report.push_str("Uninstalled.\n");
    Ok(report)
}

fn parsed(paths: &Paths, bytes: Option<&[u8]>) -> Result<(Map<String, Value>, TextFormat), String> {
    match bytes {
        Some(bytes) => settings::parse(bytes)
            .map_err(|error| format!("refusing to touch {}: {error}", paths.settings.display())),
        None => Ok((Map::new(), TextFormat::default())),
    }
}

fn changes(paths: &Paths, before: Option<&[u8]>, after: Option<&[u8]>) -> String {
    let label = paths.settings.display().to_string();
    match (before, after) {
        (Some(_), None) => {
            format!("{label} will be deleted: it did not exist before Beacon was installed.\n")
        }
        _ => {
            let text = |bytes: Option<&[u8]>| {
                String::from_utf8_lossy(bytes.unwrap_or_default()).into_owned()
            };
            let diff = diff::unified(&text(before), &text(after), &label, &label);
            if diff.is_empty() {
                "No change to the settings file.\n".to_string()
            } else {
                diff
            }
        }
    }
}

pub fn managed_policy_findings(managed_dir: &Path) -> Vec<String> {
    let mut drop_ins: Vec<PathBuf> = std::fs::read_dir(managed_dir.join("managed-settings.d"))
        .into_iter()
        .flatten()
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        })
        .collect();
    drop_ins.sort();
    std::iter::once(managed_dir.join("managed-settings.json"))
        .chain(drop_ins)
        .filter_map(|path| match std::fs::read(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => Some(format!(
                "{} could not be read ({error}); if it sets allowManagedHooksOnly, Beacon cannot work",
                path.display()
            )),
            Ok(bytes) => serde_json::from_slice::<Value>(&bytes)
                .ok()?
                .get("allowManagedHooksOnly")
                .filter(|value| **value == Value::Bool(true))
                .map(|_| {
                    format!(
                        "{} sets allowManagedHooksOnly: Beacon cannot work under that policy",
                        path.display()
                    )
                }),
        })
        .collect()
}

fn read_record(paths: &Paths) -> Result<Option<InstallRecord>, String> {
    read_optional(&paths.record())?
        .map(|bytes| serde_json::from_slice(&bytes).context(paths.record().display()))
        .transpose()
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot read {}: {error}", path.display())),
    }
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).context(format_args!("cannot write {}", path.display()))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .context(format_args!("cannot create {}", parent.display()))?;
    }
    let mut staged = path.as_os_str().to_owned();
    staged.push(".beacon.tmp");
    write_file(Path::new(&staged), bytes)?;
    std::fs::rename(&staged, path).context(format_args!("cannot replace {}", path.display()))
}

fn retire_helper(helper: &Path, failure: io::Error) -> Result<(), String> {
    let stamp = backup_stamp();
    let mut retired = helper.with_file_name(format!("{HELPER_FILE_NAME}.{stamp}.old"));
    let mut attempt = 1;
    while retired.exists() {
        attempt += 1;
        retired = helper.with_file_name(format!("{HELPER_FILE_NAME}.{stamp}-{attempt}.old"));
    }
    std::fs::rename(helper, &retired).map_err(|error| {
        format!(
            "cannot replace or delete {} ({failure}), nor rename it to {} ({error})",
            helper.display(),
            retired.display()
        )
    })
}

fn remove_retired_helpers(bin: &Path) {
    let prefix = format!("{HELPER_FILE_NAME}.");
    for entry in std::fs::read_dir(bin).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix) && name.ends_with(".old") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn remove_if_present(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            Err(format!("cannot delete {}: {error}", path.display()))
        }
        _ => Ok(()),
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    left.as_os_str().eq_ignore_ascii_case(right.as_os_str())
}

fn backup_stamp() -> String {
    let digits: String = utc_timestamp(now_ms())[..19]
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    format!("{}-{}", &digits[..8], &digits[8..])
}

#[cfg(test)]
mod tests;

use crate::server::write_atomic;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::io;
use std::ops::RangeInclusive;
use std::path::Path;

const CONFIG_FILE: &str = "config.json";
const STALL_MINUTES: RangeInclusive<u64> = 1..=240;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub stall_minutes: u64,
    pub reconcile_seconds: u64,
    pub answer_unattended: bool,
    pub log_level: LogLevel,
    pub clear_finished_minutes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LiveConfig {
    pub stall_minutes: u64,
    pub answer_unattended: bool,
    #[serde(default)]
    pub clear_finished_minutes: u64,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            stall_minutes: 10,
            reconcile_seconds: 60,
            answer_unattended: false,
            log_level: LogLevel::Info,
            clear_finished_minutes: 10,
        }
    }
}

impl Config {
    pub fn load(home: &Path) -> io::Result<Config> {
        let path = home.join(CONFIG_FILE);
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{}: {error}", path.display()),
                )
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
            Err(error) => Err(error),
        }
    }

    pub fn live(&self) -> LiveConfig {
        LiveConfig {
            stall_minutes: self.stall_minutes,
            answer_unattended: self.answer_unattended,
            clear_finished_minutes: self.clear_finished_minutes,
        }
    }

    pub fn change(
        &mut self,
        home: &Path,
        stall_minutes: Option<u64>,
        answer_unattended: Option<bool>,
        clear_finished_minutes: Option<u64>,
    ) -> Result<(), String> {
        if let Some(minutes) = stall_minutes.filter(|minutes| !STALL_MINUTES.contains(minutes)) {
            return Err(format!(
                "stall_minutes must be {} to {}, not {minutes}",
                STALL_MINUTES.start(),
                STALL_MINUTES.end()
            ));
        }
        let path = home.join(CONFIG_FILE);
        let mut stored = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<Map<String, Value>>(&bytes)
                .map_err(|error| format!("{}: {error}", path.display()))?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Map::new(),
            Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
        };
        if let Some(minutes) = stall_minutes {
            stored.insert("stall_minutes".to_string(), minutes.into());
        }
        if let Some(answer) = answer_unattended {
            stored.insert("answer_unattended".to_string(), answer.into());
        }
        if let Some(minutes) = clear_finished_minutes {
            stored.insert("clear_finished_minutes".to_string(), minutes.into());
        }
        serde_json::to_vec_pretty(&stored)
            .map_err(io::Error::from)
            .and_then(|bytes| write_atomic(&path, &bytes))
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        self.stall_minutes = stall_minutes.unwrap_or(self.stall_minutes);
        self.answer_unattended = answer_unattended.unwrap_or(self.answer_unattended);
        self.clear_finished_minutes = clear_finished_minutes.unwrap_or(self.clear_finished_minutes);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_keys_take_defaults() {
        let config: Config = serde_json::from_str(r#"{"stall_minutes":3,"unknown":true}"#).unwrap();
        assert_eq!(
            config,
            Config {
                stall_minutes: 3,
                ..Config::default()
            }
        );
    }

    #[test]
    fn a_change_is_validated_then_written_beside_the_other_keys() {
        let home = std::env::temp_dir().join(format!("beacon-config-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let path = home.join(CONFIG_FILE);
        std::fs::write(
            &path,
            r#"{"log_level":"debug","custom":[1,2],"stall_minutes":10}"#,
        )
        .unwrap();
        let mut config = Config::load(&home).unwrap();
        for minutes in [0, 241] {
            assert_eq!(
                config.change(&home, Some(minutes), Some(true), None),
                Err(format!("stall_minutes must be 1 to 240, not {minutes}"))
            );
        }
        assert_eq!(config.live(), Config::default().live());
        config.change(&home, Some(240), None, None).unwrap();
        config.change(&home, None, Some(true), None).unwrap();
        config.change(&home, None, None, Some(0)).unwrap();
        assert_eq!(
            config.live(),
            LiveConfig {
                stall_minutes: 240,
                answer_unattended: true,
                clear_finished_minutes: 0
            }
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(&path).unwrap()).unwrap(),
            serde_json::json!({"log_level": "debug", "custom": [1, 2], "stall_minutes": 240, "answer_unattended": true, "clear_finished_minutes": 0})
        );
        assert_eq!(Config::load(&home).unwrap(), config);
        std::fs::remove_dir_all(&home).unwrap();
    }
}

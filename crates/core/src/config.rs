use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

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
}

impl Default for Config {
    fn default() -> Config {
        Config {
            stall_minutes: 10,
            reconcile_seconds: 60,
            answer_unattended: false,
            log_level: LogLevel::Info,
        }
    }
}

impl Config {
    pub fn load(home: &Path) -> io::Result<Config> {
        let path = home.join("config.json");
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
}

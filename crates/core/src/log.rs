use crate::config::LogLevel;
use std::cell::RefCell;
use std::fmt::Display;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

pub struct Logger {
    path: PathBuf,
    file: RefCell<(File, u64)>,
    level: LogLevel,
}

fn open_for_append(path: &Path) -> io::Result<(File, u64)> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let len = file.metadata()?.len();
    Ok((file, len))
}

impl Logger {
    pub fn open(path: &Path, level: LogLevel) -> io::Result<Logger> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Logger {
            path: path.to_path_buf(),
            file: RefCell::new(open_for_append(path)?),
            level,
        })
    }

    pub fn error(&self, message: impl Display) {
        self.write(LogLevel::Error, message);
    }

    pub fn warn(&self, message: impl Display) {
        self.write(LogLevel::Warn, message);
    }

    pub fn info(&self, message: impl Display) {
        self.write(LogLevel::Info, message);
    }

    pub fn debug(&self, message: impl Display) {
        self.write(LogLevel::Debug, message);
    }

    fn write(&self, level: LogLevel, message: impl Display) {
        if level > self.level {
            return;
        }
        let label = match level {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN",
            LogLevel::Info => "INFO",
            LogLevel::Debug => "DEBUG",
        };
        let line = format!("{} {label} {message}\n", utc_timestamp(now_ms()));
        let mut file = self.file.borrow_mut();
        if file.1 + line.len() as u64 > MAX_LOG_BYTES {
            let _ = std::fs::rename(&self.path, self.path.with_extension("log.1"));
            if let Ok(reopened) = open_for_append(&self.path) {
                *file = reopened;
            }
        }
        if file.0.write_all(line.as_bytes()).is_ok() {
            file.1 += line.len() as u64;
        }
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

pub fn utc_timestamp(unix_ms: u64) -> String {
    let days = (unix_ms / 86_400_000) as i64;
    let ms_of_day = unix_ms % 86_400_000;
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        ms_of_day / 3_600_000,
        ms_of_day / 60_000 % 60,
        ms_of_day / 1000 % 60,
        ms_of_day % 1000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_are_utc_iso_8601() {
        assert_eq!(utc_timestamp(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(utc_timestamp(1_795_101_561_999), "2026-11-19T15:19:21.999Z");
        assert_eq!(utc_timestamp(951_782_400_000), "2000-02-29T00:00:00.000Z");
    }

    #[test]
    fn the_log_moves_to_core_log_1_before_it_would_pass_five_megabytes() {
        let dir = std::env::temp_dir().join(format!("beacon-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("core.log");
        let rotated = dir.join("core.log.1");
        std::fs::write(&rotated, "older").unwrap();
        std::fs::write(&path, "x".repeat(4 * 1024 * 1024)).unwrap();
        let logger = Logger::open(&path, LogLevel::Info).unwrap();
        let megabyte = "m".repeat(1024 * 1024);
        logger.info("first");
        assert!(!std::fs::read_to_string(&rotated).unwrap().contains("first"));
        logger.info(&megabyte);
        let older = std::fs::read_to_string(&rotated).unwrap();
        assert!(older.starts_with("xxx") && older.ends_with(" INFO first\n"));
        let current = std::fs::read_to_string(&path).unwrap();
        assert!(current.ends_with(&format!("INFO {megabyte}\n")));
        assert_eq!(current.lines().count(), 1);
        logger.debug("not at this level");
        logger.info("third");
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 2);
        assert!(std::fs::metadata(&path).unwrap().len() <= MAX_LOG_BYTES);
        drop(logger);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

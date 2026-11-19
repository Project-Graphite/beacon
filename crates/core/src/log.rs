use crate::config::LogLevel;
use std::fmt::Display;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Logger {
    file: File,
    level: LogLevel,
}

impl Logger {
    pub fn open(path: &Path, level: LogLevel) -> io::Result<Logger> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Logger {
            file: OpenOptions::new().create(true).append(true).open(path)?,
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
        let _ = (&self.file).write_all(line.as_bytes());
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
    use super::utc_timestamp;

    #[test]
    fn timestamps_are_utc_iso_8601() {
        assert_eq!(utc_timestamp(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(utc_timestamp(1_795_101_561_999), "2026-11-19T15:19:21.999Z");
        assert_eq!(utc_timestamp(951_782_400_000), "2000-02-29T00:00:00.000Z");
    }
}

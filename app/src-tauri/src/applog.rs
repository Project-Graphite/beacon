use beacon_core::{LogLevel, Logger};
use std::io;
use std::path::Path;
use std::sync::OnceLock;

static LOGGER: OnceLock<Logger> = OnceLock::new();

pub fn init(home: &Path) -> io::Result<()> {
    let logger = Logger::open(&home.join("logs").join("app.log"), LogLevel::Info)?;
    let _ = LOGGER.set(logger);
    Ok(())
}

pub fn error(message: std::fmt::Arguments) {
    if let Some(logger) = LOGGER.get() {
        logger.error(message);
    }
}

macro_rules! log_error {
    ($($argument:tt)*) => {
        $crate::applog::error(format_args!($($argument)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_land_in_the_app_log_under_the_home() {
        let home = std::env::temp_dir().join(format!("beacon-applog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        init(&home).unwrap();
        log_error!("cannot do {}", "it");
        let written = std::fs::read_to_string(home.join("logs").join("app.log")).unwrap();
        assert!(written.ends_with(" ERROR cannot do it\n"), "{written}");
        std::fs::remove_dir_all(&home).unwrap();
    }
}

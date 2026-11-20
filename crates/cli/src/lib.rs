mod diff;
pub mod doctor;
pub mod installer;
pub mod settings;
#[cfg(test)]
mod test_support;

use std::fmt::Display;

pub trait Context<T> {
    fn context(self, what: impl Display) -> Result<T, String>;
}

impl<T, E: Display> Context<T> for Result<T, E> {
    fn context(self, what: impl Display) -> Result<T, String> {
        self.map_err(|error| format!("{what}: {error}"))
    }
}

pub fn surface_label(entrypoint: Option<&str>) -> &str {
    match entrypoint {
        Some("cli") => "terminal",
        Some("sdk-cli") => "headless",
        Some("claude-vscode") => "VS Code",
        Some("claude-desktop") => "desktop",
        Some(other) => other,
        None => "unknown",
    }
}

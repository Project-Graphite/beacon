use crate::installer::Paths;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(label: &str) -> TempDir {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "beacon-cli-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn test_paths(root: &TempDir, helper_source: PathBuf) -> Paths {
    Paths {
        settings: root.0.join("claude").join("settings.json"),
        home: root.0.join("beacon home"),
        helper_source,
        managed_dir: root.0.join("managed"),
    }
}

pub fn built_helper() -> PathBuf {
    static HELPER: OnceLock<PathBuf> = OnceLock::new();
    HELPER
        .get_or_init(|| {
            let target = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("target")
                .join("cli-tests");
            let status = std::process::Command::new(env!("CARGO"))
                .args(["build", "--quiet", "-p", "beacon-hook", "--target-dir"])
                .arg(&target)
                .status()
                .unwrap();
            assert!(status.success(), "building beacon-hook failed");
            target.join("debug").join("beacon-hook.exe")
        })
        .clone()
}

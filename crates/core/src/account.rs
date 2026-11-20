use crate::lister::run_claude;
use crate::log::utc_timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;

const PROBE_TIMEOUT: Duration = Duration::from_secs(60);
const PROBE_ARGS: [&str; 10] = [
    "-p",
    "--model",
    "haiku",
    "--no-session-persistence",
    "--settings",
    r#"{"disableAllHooks":true}"#,
    "--output-format",
    "stream-json",
    "--verbose",
    "Reply with OK.",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageSource {
    Cache,
    Probe,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountUsage {
    pub five_hour_percent: Option<f64>,
    pub five_hour_resets_at: Option<String>,
    pub seven_day_percent: Option<f64>,
    pub seven_day_resets_at: Option<String>,
    pub fetched_ms: Option<u64>,
    pub source: UsageSource,
}

pub type ProbeFuture<'a> = Pin<Box<dyn Future<Output = io::Result<Vec<u8>>> + 'a>>;

pub trait UsageProber: Send {
    fn probe(&self) -> ProbeFuture<'_>;
}

pub struct ClaudeUsageProber;

impl UsageProber for ClaudeUsageProber {
    fn probe(&self) -> ProbeFuture<'_> {
        Box::pin(run_claude(&PROBE_ARGS, PROBE_TIMEOUT))
    }
}

#[derive(Default)]
pub struct AccountSources {
    pub claude_json: Option<PathBuf>,
    pub prober: Option<Box<dyn UsageProber>>,
}

pub fn claude_json_path() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|dir| PathBuf::from(dir).join(".claude.json"))
}

#[derive(Deserialize)]
struct ClaudeJson {
    #[serde(rename = "cachedUsageUtilization")]
    cached_usage_utilization: CachedUsage,
}

#[derive(Deserialize)]
struct CachedUsage {
    #[serde(rename = "fetchedAtMs")]
    fetched_at_ms: u64,
    utilization: CachedWindows,
}

#[derive(Deserialize)]
struct CachedWindows {
    five_hour: Option<CachedWindow>,
    seven_day: Option<CachedWindow>,
}

#[derive(Deserialize)]
struct CachedWindow {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

pub fn read_cache(path: &Path) -> Option<AccountUsage> {
    let cached = serde_json::from_slice::<ClaudeJson>(&std::fs::read(path).ok()?)
        .ok()?
        .cached_usage_utilization;
    let (five_hour, seven_day) = (cached.utilization.five_hour, cached.utilization.seven_day);
    Some(AccountUsage {
        five_hour_percent: five_hour.as_ref().and_then(|window| window.utilization),
        five_hour_resets_at: five_hour.and_then(|window| window.resets_at),
        seven_day_percent: seven_day.as_ref().and_then(|window| window.utilization),
        seven_day_resets_at: seven_day.and_then(|window| window.resets_at),
        fetched_ms: Some(cached.fetched_at_ms),
        source: UsageSource::Cache,
    })
}

pub fn parse_probe(stdout: &[u8], now_ms: u64) -> Option<AccountUsage> {
    let windows = stdout
        .split(|byte| *byte == b'\n')
        .rev()
        .filter_map(|line| serde_json::from_slice::<Value>(line).ok())
        .filter(|line| line.get("type").and_then(Value::as_str) == Some("rate_limit_event"))
        .find_map(|line| find_object(&line, "unifiedWindows").cloned())?;
    let field = |window: &str, name: &str| windows.get(window).and_then(|window| window.get(name));
    let percent = |window: &str| {
        field(window, "utilization")
            .and_then(Value::as_f64)
            .map(|fraction| fraction * 100.0)
    };
    let resets_at = |window: &str| {
        field(window, "resetsAt")
            .and_then(Value::as_u64)
            .map(|seconds| utc_timestamp(seconds.saturating_mul(1000)))
    };
    Some(AccountUsage {
        five_hour_percent: percent("five_hour"),
        five_hour_resets_at: resets_at("five_hour"),
        seven_day_percent: percent("seven_day"),
        seven_day_resets_at: resets_at("seven_day"),
        fetched_ms: Some(now_ms),
        source: UsageSource::Probe,
    })
}

fn find_object<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => map
            .get(key)
            .filter(|found| found.is_object())
            .or_else(|| map.values().find_map(|nested| find_object(nested, key))),
        Value::Array(items) => items.iter().find_map(|nested| find_object(nested, key)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_cache_is_read_from_its_key_alone() {
        let path = std::env::temp_dir().join(format!("beacon-claude-json-{}", std::process::id()));
        std::fs::write(
            &path,
            json!({
                "oauthAccount": {"emailAddress": "someone@example.test", "organizationUuid": "x"},
                "projects": {"C:/work": {"history": [{"display": "synthetic"}]}},
                "cachedUsageUtilization": {
                    "fetchedAtMs": 1_795_100_000_000u64,
                    "utilization": {
                        "five_hour": {"utilization": 47, "resets_at": "2026-11-20T14:30:00Z"},
                        "seven_day": {"utilization": 11.5, "resets_at": "2026-11-26T09:00:00Z"},
                        "seven_day_opus": null,
                    },
                    "limits": [{"kind": "x"}],
                    "extra_usage": {"enabled": false},
                    "spend": {"amount": 1},
                },
                "numStartups": 12,
            })
            .to_string(),
        )
        .unwrap();
        let usage = read_cache(&path);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            usage,
            Some(AccountUsage {
                five_hour_percent: Some(47.0),
                five_hour_resets_at: Some("2026-11-20T14:30:00Z".to_string()),
                seven_day_percent: Some(11.5),
                seven_day_resets_at: Some("2026-11-26T09:00:00Z".to_string()),
                fetched_ms: Some(1_795_100_000_000),
                source: UsageSource::Cache,
            })
        );
    }

    #[test]
    fn a_missing_file_key_or_shape_gives_nothing() {
        let path =
            std::env::temp_dir().join(format!("beacon-claude-json-bad-{}", std::process::id()));
        assert_eq!(read_cache(&path), None);
        for text in [
            json!({"numStartups": 1}).to_string(),
            json!({"cachedUsageUtilization": {"fetchedAtMs": "soon", "utilization": {}}})
                .to_string(),
            json!({"cachedUsageUtilization": {"fetchedAtMs": 1, "utilization": [1]}}).to_string(),
            "{not json".to_string(),
        ] {
            std::fs::write(&path, text).unwrap();
            assert_eq!(read_cache(&path), None);
        }
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn the_probe_stream_gives_percent_and_iso_resets() {
        let stream = concat!(
            r#"{"type":"system","subtype":"init","session_id":"x"}"#,
            "\n",
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","unifiedWindows":{"five_hour":{"utilization":0.5,"resetsAt":1795104600},"seven_day":{"utilization":0.25,"resetsAt":1795690800}}},"uuid":"u"}"#,
            "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"OK"}]}}"#,
            "\n",
            r#"{"type":"result","subtype":"success"}"#,
            "\n",
        );
        assert_eq!(
            parse_probe(stream.as_bytes(), 42),
            Some(AccountUsage {
                five_hour_percent: Some(50.0),
                five_hour_resets_at: Some("2026-11-19T16:10:00.000Z".to_string()),
                seven_day_percent: Some(25.0),
                seven_day_resets_at: Some("2026-11-26T11:00:00.000Z".to_string()),
                fetched_ms: Some(42),
                source: UsageSource::Probe,
            })
        );
    }

    #[test]
    fn a_probe_stream_without_windows_gives_nothing() {
        let stream = concat!(
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"}}"#,
            "\n",
            r#"{"type":"assistant","unifiedWindows":{"five_hour":{"utilization":0.9}}}"#,
            "\n",
            "not json\n"
        );
        assert_eq!(parse_probe(stream.as_bytes(), 0), None);
        assert_eq!(parse_probe(b"", 0), None);
    }
}

use crate::status::{age, project_label, table};
use beacon_core::{AccountUsage, Session, SessionUsage, Snapshot, UsageSource, UsageStatus};
use std::cmp::Reverse;

const SESSION_ID_CHARS: usize = 8;

pub fn render(snapshot: &Snapshot, now: u64) -> String {
    let mut text = match &snapshot.account_usage {
        Some(account) => account_windows(account, now),
        None => "Account limits: unknown.\n".to_string(),
    };
    text.push('\n');
    let mut sessions: Vec<(&Session, &SessionUsage)> = snapshot
        .sessions
        .iter()
        .filter_map(|session| Some((session, session.usage.as_ref()?)))
        .collect();
    sessions.sort_by_key(|(session, _)| Reverse(session.last_event_ms));
    if sessions.is_empty() {
        text.push_str("No session usage yet.\n");
        return text;
    }
    text.push_str(&table(
        &[
            "SESSION",
            "PROJECT",
            "MODEL",
            "CONTEXT",
            "IN",
            "OUT",
            "CACHE-W",
            "CACHE-R",
            "SUBAGENTS",
            "COST",
        ],
        sessions
            .into_iter()
            .map(|(session, usage)| {
                vec![
                    session.session_id.chars().take(SESSION_ID_CHARS).collect(),
                    project_label(session),
                    match usage.status {
                        UsageStatus::Ok => usage.model.clone().unwrap_or_else(|| "-".to_string()),
                        UsageStatus::NoData => "no data".to_string(),
                        UsageStatus::Unrecognized => "unrecognized".to_string(),
                    },
                    tokens(usage.context_tokens),
                    tokens(usage.input_tokens),
                    tokens(usage.output_tokens),
                    tokens(usage.cache_creation_tokens),
                    tokens(usage.cache_read_tokens),
                    tokens(usage.subagent_tokens),
                    usage
                        .reported_cost_usd
                        .map_or_else(|| "-".to_string(), |cost| format!("${cost:.2}")),
                ]
            })
            .collect(),
    ));
    text
}

fn account_windows(account: &AccountUsage, now: u64) -> String {
    let source = match account.source {
        UsageSource::Cache => "cache",
        UsageSource::Probe => "probe",
    };
    let fetched = account.fetched_ms.map_or_else(
        || "at an unknown time".to_string(),
        |fetched| format!("{} ago", age(now, fetched)),
    );
    let window = |name: &str, percent: Option<f64>, resets_at: &Option<String>| {
        vec![
            name.to_string(),
            percent.map_or_else(|| "-".to_string(), |percent| format!("{percent:.0}%")),
            resets_at.clone().unwrap_or_else(|| "-".to_string()),
        ]
    };
    format!(
        "Account limits from the {source}, fetched {fetched}:\n{}",
        table(
            &["WINDOW", "USED", "RESETS"],
            vec![
                window(
                    "5-hour",
                    account.five_hour_percent,
                    &account.five_hour_resets_at
                ),
                window(
                    "7-day",
                    account.seven_day_percent,
                    &account.seven_day_resets_at
                ),
            ],
        )
    )
}

fn tokens(count: u64) -> String {
    match count {
        0..1000 => count.to_string(),
        1000..1_000_000 => format!("{:.1}k", count as f64 / 1000.0),
        _ => format!("{:.1}M", count as f64 / 1_000_000.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use beacon_core::SessionState;

    const NOW: u64 = 1_795_100_000_000;

    fn snapshot(sessions: Vec<Session>, account_usage: Option<AccountUsage>) -> Snapshot {
        Snapshot {
            sessions,
            pending: Vec::new(),
            resolved: Vec::new(),
            snoozed_until_ms: None,
            account_usage,
        }
    }

    fn with_usage(id: &str, last_event_ms: u64, usage: SessionUsage) -> Session {
        Session {
            state: SessionState::Working,
            project: Some("beacon".to_string()),
            usage: Some(usage),
            ..Session::new(id, last_event_ms)
        }
    }

    fn usage(status: UsageStatus) -> SessionUsage {
        SessionUsage {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            context_tokens: 0,
            subagent_tokens: 0,
            reported_cost_usd: None,
            model: None,
            updated_ms: NOW,
            status,
        }
    }

    #[test]
    fn the_usage_report_shows_account_windows_and_session_totals() {
        let busy = with_usage(
            "11111111-aaaa-bbbb-cccc-000000000001",
            NOW - 1_000,
            SessionUsage {
                input_tokens: 12_345,
                output_tokens: 924_077,
                cache_creation_tokens: 1_500_000,
                cache_read_tokens: 98_765_432,
                context_tokens: 182_000,
                subagent_tokens: 999,
                reported_cost_usd: Some(4.5),
                model: Some("claude-opus-5".to_string()),
                ..usage(UsageStatus::Ok)
            },
        );
        let fresh = with_usage(
            "22222222-aaaa-bbbb-cccc-000000000002",
            NOW - 5_000,
            usage(UsageStatus::NoData),
        );
        let changed = with_usage(
            "33333333-aaaa-bbbb-cccc-000000000003",
            NOW - 9_000,
            usage(UsageStatus::Unrecognized),
        );
        let without = Session::new("44444444", NOW);
        let account = AccountUsage {
            five_hour_percent: Some(47.4),
            five_hour_resets_at: Some("2026-11-20T14:30:00Z".to_string()),
            seven_day_percent: None,
            seven_day_resets_at: None,
            fetched_ms: Some(NOW - 125_000),
            source: UsageSource::Cache,
        };
        assert_eq!(
            render(
                &snapshot(vec![changed, without, fresh, busy], Some(account)),
                NOW
            ),
            "\
Account limits from the cache, fetched 2m05s ago:
WINDOW  USED  RESETS
5-hour  47%   2026-11-20T14:30:00Z
7-day   -     -

SESSION   PROJECT  MODEL          CONTEXT  IN     OUT     CACHE-W  CACHE-R  SUBAGENTS  COST
11111111  beacon   claude-opus-5  182.0k   12.3k  924.1k  1.5M     98.8M    999        $4.50
22222222  beacon   no data        0        0      0       0        0        0          -
33333333  beacon   unrecognized   0        0      0       0        0        0          -
"
        );
    }

    #[test]
    fn an_empty_usage_report_says_so() {
        assert_eq!(
            render(&snapshot(Vec::new(), None), NOW),
            "Account limits: unknown.\n\nNo session usage yet.\n"
        );
    }
}

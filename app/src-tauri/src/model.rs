use crate::positions::Visibility;
use crate::toasts::verb;
use beacon_core::{Session, SessionState, Snapshot};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

pub const MAX_MARKS: usize = 10;
pub const LINGER: Duration = Duration::from_secs(5);
const PEEK_BATCH: Duration = Duration::from_secs(2);
const PEEK_NAME_CHARS: usize = 28;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Presence {
    #[default]
    Hidden,
    Shown,
    Lingering(Instant),
}

#[derive(Debug, Default)]
pub struct Unused {
    since: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peek {
    pub text: String,
    pub tone: TabTone,
}

#[derive(Debug, Default)]
pub struct PeekBatch {
    gains: Vec<(Instant, String)>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TabTone {
    NeedsYou,
    Failed,
    Finished,
    Working,
    #[default]
    Dormant,
}

#[derive(Debug, Default)]
pub struct Unseen {
    ids: BTreeSet<String>,
    alerting: BTreeSet<String>,
    since: Option<(Instant, Instant)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
    Starting,
    Working,
    NeedsInput,
    Stalled,
    Done,
    Error,
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    pub kind: MarkKind,
    pub snoozed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PillModel {
    pub marks: Vec<Mark>,
    pub overflow: usize,
    pub alerting: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayState {
    Quiet,
    Working,
    Error,
    Alerting(usize),
}

fn mark_kind(state: SessionState) -> Option<MarkKind> {
    match state {
        SessionState::Starting => Some(MarkKind::Starting),
        SessionState::Working => Some(MarkKind::Working),
        SessionState::NeedsPermission
        | SessionState::NeedsAnswer
        | SessionState::NeedsPlanApproval
        | SessionState::NeedsMcpInput => Some(MarkKind::NeedsInput),
        SessionState::Stalled => Some(MarkKind::Stalled),
        SessionState::Done => Some(MarkKind::Done),
        SessionState::Error => Some(MarkKind::Error),
        SessionState::Idle => Some(MarkKind::Idle),
        SessionState::Ended => None,
    }
}

fn group(session: &Session) -> u8 {
    match (session.alerting, session.attention, session.state) {
        (true, ..) => 0,
        (false, true, _) => 1,
        (false, false, SessionState::Working) => 2,
        (false, false, SessionState::Starting) => 3,
        _ => 4,
    }
}

pub fn ordered_sessions(snapshot: &Snapshot) -> Vec<&Session> {
    let mut sessions: Vec<&Session> = snapshot
        .sessions
        .iter()
        .filter(|session| session.state != SessionState::Ended)
        .collect();
    sessions.sort_by(|a, b| (group(a), &a.session_id).cmp(&(group(b), &b.session_id)));
    sessions
}

pub fn alerting_count(snapshot: Option<&Snapshot>) -> usize {
    snapshot.map_or(0, |snapshot| {
        snapshot
            .sessions
            .iter()
            .filter(|session| session.alerting)
            .count()
    })
}

impl PillModel {
    pub fn from_snapshot(snapshot: Option<&Snapshot>) -> PillModel {
        let Some(snapshot) = snapshot else {
            return PillModel::default();
        };
        let sessions = ordered_sessions(snapshot);
        PillModel {
            marks: sessions
                .iter()
                .take(MAX_MARKS)
                .filter_map(|session| {
                    mark_kind(session.state).map(|kind| Mark {
                        kind,
                        snoozed: session.attention && !session.alerting,
                    })
                })
                .collect(),
            overflow: sessions.len().saturating_sub(MAX_MARKS),
            alerting: alerting_count(Some(snapshot)),
        }
    }

    pub fn animates(&self, reduced_motion: bool) -> bool {
        !reduced_motion
            && self.marks.iter().any(|mark| {
                mark.kind == MarkKind::Working
                    || (mark.kind == MarkKind::NeedsInput && !mark.snoozed)
            })
    }
}

pub fn tray_state(snapshot: Option<&Snapshot>) -> TrayState {
    let sessions = snapshot.map_or(&[][..], |snapshot| &snapshot.sessions[..]);
    let alerting = alerting_count(snapshot);
    let any = |state: SessionState| sessions.iter().any(|session| session.state == state);
    if alerting > 0 {
        TrayState::Alerting(alerting)
    } else if any(SessionState::Error) {
        TrayState::Error
    } else if any(SessionState::Working) {
        TrayState::Working
    } else {
        TrayState::Quiet
    }
}

pub fn tab_tone(snapshot: Option<&Snapshot>) -> TabTone {
    let sessions = snapshot.map_or(&[][..], |snapshot| &snapshot.sessions[..]);
    let alerting_as = |kinds: &[MarkKind]| {
        sessions.iter().any(|session| {
            session.alerting && mark_kind(session.state).is_some_and(|kind| kinds.contains(&kind))
        })
    };
    if alerting_as(&[MarkKind::NeedsInput, MarkKind::Stalled]) {
        TabTone::NeedsYou
    } else if alerting_as(&[MarkKind::Error]) {
        TabTone::Failed
    } else if alerting_as(&[MarkKind::Done]) {
        TabTone::Finished
    } else if sessions
        .iter()
        .any(|session| session.state == SessionState::Working)
    {
        TabTone::Working
    } else {
        TabTone::Dormant
    }
}

pub fn claude_active(snapshot: Option<&Snapshot>) -> bool {
    snapshot.is_some_and(|snapshot| {
        snapshot.sessions.iter().any(|session| {
            session.alerting
                || matches!(
                    mark_kind(session.state),
                    Some(
                        MarkKind::Starting
                            | MarkKind::Working
                            | MarkKind::NeedsInput
                            | MarkKind::Stalled
                    )
                )
        })
    })
}

pub fn overlay_wanted(snapshot: Option<&Snapshot>, visibility: Visibility) -> bool {
    match visibility {
        Visibility::Always => true,
        Visibility::Active => claude_active(snapshot),
        Visibility::Attention => alerting_count(snapshot) > 0,
        Visibility::Never => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pin {
    Open,
    Keep,
    Close,
    StayHidden,
}

pub fn pin_decision(was_active: bool, active: bool, enabled: bool, dismissed: bool) -> (Pin, bool) {
    match (enabled, was_active, active, dismissed) {
        (false, ..) => (Pin::Close, false),
        (true, true, true, true) => (Pin::StayHidden, true),
        (true, _, true, _) => (Pin::Open, false),
        (true, true, false, _) => (Pin::Close, false),
        (true, false, false, _) => (Pin::Keep, false),
    }
}

impl Presence {
    pub fn update(&mut self, needed: bool, now: Instant) -> Option<Duration> {
        *self = match (*self, needed) {
            (_, true) => Presence::Shown,
            (Presence::Shown, false) => Presence::Lingering(now),
            (Presence::Lingering(since), false) if now.duration_since(since) >= LINGER => {
                Presence::Hidden
            }
            (unchanged, false) => unchanged,
        };
        match *self {
            Presence::Lingering(since) => Some(LINGER.saturating_sub(now.duration_since(since))),
            _ => None,
        }
    }

    pub fn shown(self) -> bool {
        self != Presence::Hidden
    }
}

pub fn in_use(snapshot: Option<&Snapshot>, panel_open: bool) -> bool {
    panel_open
        || snapshot.is_some_and(|snapshot| {
            !snapshot.pending.is_empty()
                || snapshot
                    .sessions
                    .iter()
                    .any(|session| session.state != SessionState::Ended)
        })
}

impl Unused {
    pub fn observe(&mut self, in_use: bool, now: Instant) {
        if in_use {
            self.since = None;
        } else if self.since.is_none() {
            self.since = Some(now);
        }
    }

    pub fn quit_in(&self, minutes: u64, now: Instant) -> Option<Duration> {
        let since = self.since.filter(|_| minutes > 0)?;
        Some((since + Duration::from_secs(minutes * 60)).saturating_duration_since(now))
    }
}

fn peek_name(session: &Session) -> String {
    let name = [&session.title, &session.listed_name, &session.project]
        .into_iter()
        .flatten()
        .map(|name| name.trim())
        .find(|name| !name.is_empty())
        .map_or_else(
            || session.session_id.chars().take(8).collect(),
            str::to_string,
        );
    if name.chars().count() > PEEK_NAME_CHARS {
        format!(
            "{}…",
            name.chars()
                .take(PEEK_NAME_CHARS)
                .collect::<String>()
                .trim_end()
        )
    } else {
        name
    }
}

fn alert_tone(state: SessionState) -> TabTone {
    match state {
        SessionState::Done => TabTone::Finished,
        SessionState::Error => TabTone::Failed,
        _ => TabTone::NeedsYou,
    }
}

pub fn peek_for(sessions: &[&Session]) -> Option<Peek> {
    let alerting: Vec<(&Session, &str)> = sessions
        .iter()
        .filter_map(|session| verb(session.state).map(|verb| (*session, verb)))
        .collect();
    match alerting.as_slice() {
        [] => None,
        [(session, verb)] => Some(Peek {
            text: format!("{} {verb}", peek_name(session)),
            tone: alert_tone(session.state),
        }),
        several
            if several
                .iter()
                .all(|(session, _)| session.state == SessionState::Done) =>
        {
            Some(Peek {
                text: format!("{} sessions finished", several.len()),
                tone: TabTone::Finished,
            })
        }
        several => Some(Peek {
            text: format!("{} sessions need you", several.len()),
            tone: TabTone::NeedsYou,
        }),
    }
}

impl PeekBatch {
    pub fn add(&mut self, gained: &[String], now: Instant) -> Vec<String> {
        self.gains
            .retain(|(at, id)| now.duration_since(*at) <= PEEK_BATCH && !gained.contains(id));
        self.gains.extend(gained.iter().map(|id| (now, id.clone())));
        self.gains.iter().map(|(_, id)| id.clone()).collect()
    }
}

impl Unseen {
    pub fn observe(&mut self, snapshot: Option<&Snapshot>, now: Instant) -> Vec<String> {
        let alerting: BTreeSet<String> = snapshot.map_or_else(BTreeSet::new, |snapshot| {
            snapshot
                .sessions
                .iter()
                .filter(|session| session.alerting)
                .map(|session| session.session_id.clone())
                .collect()
        });
        self.ids.retain(|id| alerting.contains(id));
        let before = self.ids.len();
        let gained: Vec<String> = alerting.difference(&self.alerting).cloned().collect();
        self.ids.extend(gained.iter().cloned());
        if self.ids.is_empty() {
            self.since = None;
        } else if self.ids.len() > before {
            self.since = Some((
                self.since
                    .filter(|_| before > 0)
                    .map_or(now, |(first, _)| first),
                now,
            ));
        }
        self.alerting = alerting;
        gained
    }

    pub fn clear(&mut self) {
        self.ids.clear();
        self.since = None;
    }

    pub fn since(&self) -> Option<(Instant, Instant)> {
        self.since
    }
}

pub fn tooltip(snapshot: Option<&Snapshot>) -> String {
    let Some(snapshot) = snapshot else {
        return "Beacon: not connected".to_string();
    };
    let live = ordered_sessions(snapshot);
    let working = live
        .iter()
        .filter(|session| session.state == SessionState::Working)
        .count();
    let parts: Vec<String> = [
        (alerting_count(Some(snapshot)), "need you"),
        (working, "working"),
    ]
    .into_iter()
    .filter(|(count, _)| *count > 0)
    .map(|(count, label)| format!("{count} {label}"))
    .collect();
    match (parts.is_empty(), live.len()) {
        (false, _) => format!("Beacon: {}", parts.join(", ")),
        (true, 0) => "Beacon: no sessions".to_string(),
        (true, 1) => "Beacon: 1 session, all quiet".to_string(),
        (true, count) => format!("Beacon: {count} sessions, all quiet"),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn session(id: &str, state: SessionState) -> Session {
        Session {
            state,
            attention: state.needs_attention(),
            alerting: state.needs_attention(),
            project: Some(format!("project-{id}")),
            ..Session::new(id, 0)
        }
    }

    pub fn snapshot(sessions: Vec<Session>) -> Snapshot {
        Snapshot {
            sessions,
            pending: Vec::new(),
            resolved: Vec::new(),
            snoozed_until_ms: None,
            account_usage: None,
            config: beacon_core::Config::default().live(),
        }
    }

    fn snoozed(mut session: Session) -> Session {
        session.alerting = false;
        session.snoozed_until_ms = Some(1);
        session
    }

    #[test]
    fn sessions_are_ordered_alerting_snoozed_working_starting_idle_then_by_id() {
        let snapshot = snapshot(vec![
            session("i", SessionState::Idle),
            session("s", SessionState::Starting),
            session("w2", SessionState::Working),
            snoozed(session("z", SessionState::NeedsAnswer)),
            session("w1", SessionState::Working),
            session("b", SessionState::Done),
            session("e", SessionState::Ended),
            session("a", SessionState::NeedsPermission),
        ]);
        let ids: Vec<&str> = ordered_sessions(&snapshot)
            .iter()
            .map(|session| session.session_id.as_str())
            .collect();
        assert_eq!(ids, ["a", "b", "z", "w1", "w2", "s", "i"]);
    }

    #[test]
    fn the_model_caps_marks_at_ten_and_counts_the_rest() {
        let sessions = (0..12)
            .map(|index| session(&format!("{index:02}"), SessionState::Working))
            .collect();
        let model = PillModel::from_snapshot(Some(&snapshot(sessions)));
        assert_eq!(model.marks.len(), 10);
        assert_eq!(model.overflow, 2);
        assert_eq!(model.alerting, 0);
    }

    #[test]
    fn snoozed_attention_marks_are_flagged_and_do_not_animate() {
        let model = PillModel::from_snapshot(Some(&snapshot(vec![snoozed(session(
            "a",
            SessionState::NeedsPermission,
        ))])));
        assert_eq!(
            model.marks,
            [Mark {
                kind: MarkKind::NeedsInput,
                snoozed: true
            }]
        );
        assert!(!model.animates(false));
        let alerting = PillModel::from_snapshot(Some(&snapshot(vec![session(
            "a",
            SessionState::NeedsAnswer,
        )])));
        assert!(alerting.animates(false));
        assert!(!alerting.animates(true));
    }

    #[test]
    fn no_snapshot_is_the_empty_model() {
        assert_eq!(PillModel::from_snapshot(None), PillModel::default());
    }

    #[test]
    fn tray_state_prefers_alerting_then_error_then_working() {
        let state = |sessions: Vec<Session>| tray_state(Some(&snapshot(sessions)));
        assert_eq!(tray_state(None), TrayState::Quiet);
        assert_eq!(state(vec![]), TrayState::Quiet);
        assert_eq!(
            state(vec![session("a", SessionState::Idle)]),
            TrayState::Quiet
        );
        assert_eq!(
            state(vec![
                session("a", SessionState::Idle),
                session("b", SessionState::Working)
            ]),
            TrayState::Working
        );
        assert_eq!(
            state(vec![
                session("a", SessionState::Working),
                snoozed(session("b", SessionState::Error))
            ]),
            TrayState::Error
        );
        assert_eq!(
            state(vec![
                session("a", SessionState::NeedsPermission),
                session("b", SessionState::Done),
                session("c", SessionState::Error),
            ]),
            TrayState::Alerting(3)
        );
    }

    #[test]
    fn the_tab_tone_prefers_needs_you_then_failed_then_finished_then_working() {
        let tone = |sessions: Vec<Session>| tab_tone(Some(&snapshot(sessions)));
        assert_eq!(tab_tone(None), TabTone::Dormant);
        assert_eq!(
            tone(vec![session("a", SessionState::Idle)]),
            TabTone::Dormant
        );
        assert_eq!(
            tone(vec![
                session("a", SessionState::Idle),
                session("b", SessionState::Working)
            ]),
            TabTone::Working
        );
        assert_eq!(
            tone(vec![
                session("a", SessionState::Working),
                snoozed(session("b", SessionState::Error))
            ]),
            TabTone::Working
        );
        assert_eq!(
            tone(vec![
                session("a", SessionState::Working),
                session("b", SessionState::Done)
            ]),
            TabTone::Finished
        );
        assert_eq!(
            tone(vec![
                session("a", SessionState::Done),
                session("b", SessionState::Error)
            ]),
            TabTone::Failed
        );
        assert_eq!(
            tone(vec![
                session("a", SessionState::Error),
                session("b", SessionState::Stalled)
            ]),
            TabTone::NeedsYou
        );
        assert_eq!(
            tone(vec![
                session("a", SessionState::Done),
                session("b", SessionState::NeedsPlanApproval)
            ]),
            TabTone::NeedsYou
        );
    }

    #[test]
    fn unseen_gains_new_alerts_clears_on_open_and_drops_quiet_sessions() {
        let ids = |unseen: &Unseen| unseen.ids.iter().cloned().collect::<Vec<_>>();
        let (start, later) = (
            Instant::now(),
            Instant::now() + std::time::Duration::from_secs(5),
        );
        let mut unseen = Unseen::default();
        unseen.observe(
            Some(&snapshot(vec![session("w", SessionState::Working)])),
            start,
        );
        assert!(ids(&unseen).is_empty());
        assert_eq!(unseen.since(), None);
        let alerting_a = snapshot(vec![
            session("a", SessionState::NeedsPermission),
            session("w", SessionState::Working),
        ]);
        unseen.observe(Some(&alerting_a), start);
        assert_eq!(ids(&unseen), ["a"]);
        assert_eq!(unseen.since(), Some((start, start)));
        unseen.clear();
        assert!(ids(&unseen).is_empty());
        assert_eq!(unseen.since(), None);
        unseen.observe(Some(&alerting_a), later);
        assert!(ids(&unseen).is_empty(), "a stays alerting and was seen");
        let alerting_both = snapshot(vec![
            session("a", SessionState::NeedsPermission),
            session("b", SessionState::Error),
        ]);
        unseen.observe(Some(&alerting_both), later);
        assert_eq!(ids(&unseen), ["b"]);
        assert_eq!(unseen.since(), Some((later, later)));
        let latest = later + std::time::Duration::from_secs(5);
        let alerting_three = snapshot(vec![
            session("a", SessionState::NeedsPermission),
            session("b", SessionState::Error),
            session("c", SessionState::Done),
        ]);
        unseen.observe(Some(&alerting_three), latest);
        assert_eq!(ids(&unseen), ["b", "c"]);
        assert_eq!(unseen.since(), Some((later, latest)));
        unseen.observe(Some(&alerting_a), later);
        assert!(ids(&unseen).is_empty());
        assert_eq!(unseen.since(), None);
        unseen.observe(Some(&alerting_both), start);
        assert_eq!(ids(&unseen), ["b"]);
        unseen.observe(None, start);
        assert!(ids(&unseen).is_empty());
    }

    #[test]
    fn the_overlay_is_wanted_by_visibility_and_session_states() {
        use Visibility::*;
        let wanted = |visibility, sessions: Vec<Session>| {
            overlay_wanted(Some(&snapshot(sessions)), visibility)
        };
        for visibility in [Always, Active, Attention, Never] {
            assert_eq!(overlay_wanted(None, visibility), visibility == Always);
            assert_eq!(wanted(visibility, vec![]), visibility == Always);
        }
        for state in [
            SessionState::Starting,
            SessionState::Working,
            SessionState::NeedsPermission,
            SessionState::NeedsAnswer,
            SessionState::NeedsPlanApproval,
            SessionState::NeedsMcpInput,
            SessionState::Stalled,
        ] {
            assert!(wanted(Active, vec![session("a", state)]), "{state:?}");
        }
        for state in [SessionState::Idle, SessionState::Ended] {
            assert!(!wanted(Active, vec![session("a", state)]), "{state:?}");
        }
        assert!(wanted(Active, vec![session("a", SessionState::Done)]));
        assert!(!wanted(
            Active,
            vec![snoozed(session("a", SessionState::Done))]
        ));
        assert!(!wanted(
            Attention,
            vec![session("a", SessionState::Working)]
        ));
        assert!(wanted(Attention, vec![session("a", SessionState::Error)]));
        assert!(!wanted(
            Attention,
            vec![snoozed(session("a", SessionState::NeedsPermission))]
        ));
        assert!(!wanted(
            Never,
            vec![session("a", SessionState::NeedsPermission)]
        ));
        assert!(wanted(Always, vec![session("a", SessionState::Idle)]));
    }

    #[test]
    fn claude_is_active_exactly_when_the_active_visibility_shows_the_overlay() {
        assert!(!claude_active(None));
        for state in [
            SessionState::Starting,
            SessionState::Working,
            SessionState::NeedsPermission,
            SessionState::NeedsAnswer,
            SessionState::NeedsPlanApproval,
            SessionState::NeedsMcpInput,
            SessionState::Stalled,
            SessionState::Done,
            SessionState::Error,
            SessionState::Idle,
            SessionState::Ended,
        ] {
            for sessions in [
                vec![session("a", state)],
                vec![snoozed(session("a", state))],
            ] {
                let snapshot = snapshot(sessions);
                assert_eq!(
                    claude_active(Some(&snapshot)),
                    overlay_wanted(Some(&snapshot), Visibility::Active),
                    "{state:?}"
                );
            }
        }
    }

    #[test]
    fn the_pinned_panel_follows_activity_and_a_dismissal_lasts_until_claude_rests() {
        use Pin::*;
        let table = [
            ((false, false, false, false), (Close, false)),
            ((false, false, true, false), (Close, false)),
            ((false, true, false, false), (Close, false)),
            ((false, true, true, false), (Close, false)),
            ((false, false, false, true), (Close, false)),
            ((false, false, true, true), (Close, false)),
            ((false, true, false, true), (Close, false)),
            ((false, true, true, true), (Close, false)),
            ((true, false, false, false), (Keep, false)),
            ((true, false, false, true), (Keep, false)),
            ((true, false, true, false), (Open, false)),
            ((true, false, true, true), (Open, false)),
            ((true, true, true, false), (Open, false)),
            ((true, true, true, true), (StayHidden, true)),
            ((true, true, false, false), (Close, false)),
            ((true, true, false, true), (Close, false)),
        ];
        for ((enabled, was_active, active, dismissed), expected) in table {
            assert_eq!(
                pin_decision(was_active, active, enabled, dismissed),
                expected,
                "enabled {enabled}, was active {was_active}, active {active}, dismissed {dismissed}"
            );
        }
    }

    #[test]
    fn the_overlay_lingers_five_seconds_after_it_stops_being_needed() {
        let start = Instant::now();
        let at = |seconds: f64| start + Duration::from_secs_f64(seconds);
        let mut presence = Presence::default();
        assert!(!presence.shown());
        assert_eq!(presence.update(false, at(0.0)), None);
        assert!(!presence.shown());
        assert_eq!(presence.update(true, at(1.0)), None);
        assert!(presence.shown());
        assert_eq!(presence.update(false, at(2.0)), Some(LINGER));
        assert_eq!(
            presence.update(false, at(4.0)),
            Some(Duration::from_secs(3))
        );
        assert!(presence.shown());
        assert_eq!(presence.update(true, at(5.0)), None);
        assert_eq!(presence.update(false, at(6.0)), Some(LINGER));
        assert_eq!(presence.update(false, at(11.0)), None);
        assert!(!presence.shown());
    }

    #[test]
    fn the_app_quits_after_the_chosen_minutes_without_sessions_panel_or_requests() {
        let start = Instant::now();
        let ended = snapshot(vec![session("a", SessionState::Ended)]);
        let mut waiting = snapshot(vec![]);
        waiting.pending.push(
            serde_json::from_value(serde_json::json!({
                "id": 1, "session_id": "a", "kind": "tool", "tool_name": "Bash",
                "tool_input": {}, "permission_suggestions": null, "created_ms": 0
            }))
            .unwrap(),
        );
        assert!(!in_use(None, false));
        assert!(!in_use(Some(&ended), false));
        assert!(in_use(Some(&ended), true));
        assert!(in_use(Some(&waiting), false));
        assert!(in_use(
            Some(&snapshot(vec![session("a", SessionState::Idle)])),
            false
        ));
        let mut unused = Unused::default();
        unused.observe(true, start);
        assert_eq!(unused.quit_in(15, start), None);
        unused.observe(false, start + Duration::from_secs(60));
        unused.observe(false, start + Duration::from_secs(120));
        assert_eq!(
            unused.quit_in(15, start + Duration::from_secs(120)),
            Some(Duration::from_secs(14 * 60))
        );
        assert_eq!(
            unused.quit_in(5, start + Duration::from_secs(3600)),
            Some(Duration::ZERO)
        );
        assert_eq!(unused.quit_in(0, start + Duration::from_secs(3600)), None);
        unused.observe(true, start + Duration::from_secs(200));
        assert_eq!(unused.quit_in(15, start + Duration::from_secs(3600)), None);
    }

    fn titled(id: &str, state: SessionState, title: Option<&str>) -> Session {
        Session {
            title: title.map(str::to_string),
            ..session(id, state)
        }
    }

    #[test]
    fn a_peek_names_one_session_and_its_reason() {
        let peek = |session: Session| peek_for(&[&session]).map(|peek| (peek.text, peek.tone));
        assert_eq!(
            peek(titled(
                "a",
                SessionState::NeedsPermission,
                Some("Fix login")
            )),
            Some(("Fix login needs permission".to_string(), TabTone::NeedsYou))
        );
        assert_eq!(
            peek(session("a", SessionState::NeedsAnswer)).unwrap().0,
            "project-a has a question"
        );
        let listed = Session {
            listed_name: Some("listed".to_string()),
            ..session("a", SessionState::NeedsPlanApproval)
        };
        assert_eq!(peek(listed).unwrap().0, "listed wants plan approval");
        assert_eq!(
            peek(session("a", SessionState::NeedsMcpInput)).unwrap().0,
            "project-a needs input"
        );
        assert_eq!(
            peek(session("a", SessionState::Error)),
            Some(("project-a hit an API error".to_string(), TabTone::Failed))
        );
        assert_eq!(
            peek(session("a", SessionState::Stalled)).unwrap().0,
            "project-a may be stalled"
        );
        assert_eq!(
            peek(session("a", SessionState::Done)),
            Some(("project-a finished".to_string(), TabTone::Finished))
        );
        assert_eq!(peek(session("a", SessionState::Working)), None);
        let nameless = Session {
            project: None,
            ..session("0123456789", SessionState::Done)
        };
        assert_eq!(peek(nameless).unwrap().0, "01234567 finished");
    }

    #[test]
    fn a_long_title_is_cut_to_twenty_eight_characters_with_an_ellipsis() {
        let long = titled(
            "a",
            SessionState::NeedsPermission,
            Some("Refactor the authentication middleware"),
        );
        assert_eq!(
            peek_for(&[&long]).unwrap().text,
            "Refactor the authentication… needs permission"
        );
        let exact = titled("a", SessionState::Done, Some(&"x".repeat(28)));
        assert_eq!(
            peek_for(&[&exact]).unwrap().text,
            format!("{} finished", "x".repeat(28))
        );
    }

    #[test]
    fn several_sessions_need_you_or_finished_together() {
        let (a, b, c) = (
            session("a", SessionState::Done),
            session("b", SessionState::Done),
            session("c", SessionState::NeedsPermission),
        );
        let finished = peek_for(&[&a, &b]).unwrap();
        assert_eq!(finished.text, "2 sessions finished");
        assert_eq!(finished.tone, TabTone::Finished);
        let mixed = peek_for(&[&a, &b, &c]).unwrap();
        assert_eq!(mixed.text, "3 sessions need you");
        assert_eq!(mixed.tone, TabTone::NeedsYou);
        assert_eq!(peek_for(&[]), None);
    }

    #[test]
    fn gains_within_two_seconds_are_peeked_together() {
        let start = Instant::now();
        let ids = |names: &[&str]| {
            names
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>()
        };
        let mut batch = PeekBatch::default();
        assert_eq!(batch.add(&ids(&["a"]), start), ids(&["a"]));
        assert_eq!(
            batch.add(&ids(&["b", "c"]), start + Duration::from_millis(1500)),
            ids(&["a", "b", "c"])
        );
        assert_eq!(
            batch.add(&ids(&["a"]), start + Duration::from_millis(3000)),
            ids(&["b", "c", "a"])
        );
        assert_eq!(
            batch.add(&ids(&["d"]), start + Duration::from_millis(6000)),
            ids(&["d"])
        );
    }

    #[test]
    fn tooltips_summarise_alerting_and_working() {
        assert_eq!(tooltip(None), "Beacon: not connected");
        assert_eq!(tooltip(Some(&snapshot(vec![]))), "Beacon: no sessions");
        assert_eq!(
            tooltip(Some(&snapshot(vec![
                session("a", SessionState::NeedsPermission),
                session("b", SessionState::Done),
                session("c", SessionState::Working),
            ]))),
            "Beacon: 2 need you, 1 working"
        );
        assert_eq!(
            tooltip(Some(&snapshot(vec![session("a", SessionState::Idle)]))),
            "Beacon: 1 session, all quiet"
        );
    }
}

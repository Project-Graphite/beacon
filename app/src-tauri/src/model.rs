use beacon_core::{Session, SessionState, Snapshot};

pub const MAX_MARKS: usize = 10;

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

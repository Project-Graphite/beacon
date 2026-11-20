use beacon_core::{Session, SessionState, Snapshot};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    NeedsYou,
    Finished,
}

impl Sound {
    pub fn system_alias(self) -> &'static str {
        match self {
            Sound::NeedsYou => "SystemExclamation",
            Sound::Finished => "SystemAsterisk",
        }
    }
}

fn needs_you(session: &Session) -> bool {
    session.alerting
        && (session.state.needs_input()
            || matches!(session.state, SessionState::Stalled | SessionState::Error))
}

fn finished(session: &Session) -> bool {
    session.alerting && session.state == SessionState::Done
}

pub fn sound_for(previous: Option<&Snapshot>, current: &Snapshot, enabled: bool) -> Option<Sound> {
    let previous = previous.filter(|_| enabled)?;
    let needing_you = |snapshot: &Snapshot| {
        snapshot
            .sessions
            .iter()
            .filter(|session| needs_you(session))
            .count()
    };
    if needing_you(current) > needing_you(previous) {
        return Some(Sound::NeedsYou);
    }
    let finished_before: HashSet<&str> = previous
        .sessions
        .iter()
        .filter(|session| finished(session))
        .map(|session| session.session_id.as_str())
        .collect();
    current
        .sessions
        .iter()
        .any(|session| finished(session) && !finished_before.contains(session.session_id.as_str()))
        .then_some(Sound::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::{session, snapshot};

    fn played(before: Vec<Session>, after: Vec<Session>) -> Option<Sound> {
        sound_for(Some(&snapshot(before)), &snapshot(after), true)
    }

    #[test]
    fn a_rising_count_of_sessions_that_need_you_plays_needs_you() {
        for state in [
            SessionState::NeedsPermission,
            SessionState::NeedsAnswer,
            SessionState::NeedsPlanApproval,
            SessionState::NeedsMcpInput,
            SessionState::Stalled,
            SessionState::Error,
        ] {
            assert_eq!(
                played(
                    vec![session("a", SessionState::Working)],
                    vec![session("a", state)]
                ),
                Some(Sound::NeedsYou),
                "{state:?}"
            );
        }
        assert_eq!(
            played(
                vec![session("a", SessionState::NeedsPermission)],
                vec![session("a", SessionState::NeedsAnswer)]
            ),
            None
        );
        assert_eq!(
            played(
                vec![
                    session("a", SessionState::NeedsPermission),
                    session("b", SessionState::Error)
                ],
                vec![session("a", SessionState::NeedsPermission)]
            ),
            None
        );
    }

    #[test]
    fn a_session_newly_finished_plays_finished_and_needs_you_wins() {
        assert_eq!(
            played(
                vec![session("a", SessionState::Working)],
                vec![session("a", SessionState::Done)]
            ),
            Some(Sound::Finished)
        );
        assert_eq!(
            played(
                vec![session("a", SessionState::Done)],
                vec![
                    session("a", SessionState::Done),
                    session("b", SessionState::Idle)
                ]
            ),
            None
        );
        assert_eq!(
            played(
                vec![
                    session("a", SessionState::Working),
                    session("b", SessionState::Working)
                ],
                vec![
                    session("a", SessionState::Done),
                    session("b", SessionState::NeedsAnswer)
                ]
            ),
            Some(Sound::NeedsYou)
        );
    }

    #[test]
    fn nothing_plays_on_the_first_snapshot_when_disabled_or_for_quiet_sessions() {
        let after = snapshot(vec![
            session("a", SessionState::Done),
            session("b", SessionState::NeedsPermission),
        ]);
        let before = snapshot(vec![]);
        assert_eq!(sound_for(None, &after, true), None);
        assert_eq!(sound_for(Some(&before), &after, false), None);
        let mut snoozed = session("a", SessionState::NeedsPermission);
        snoozed.alerting = false;
        snoozed.snoozed_until_ms = Some(1);
        let mut dismissed = session("b", SessionState::Stalled);
        dismissed.alerting = false;
        dismissed.dismissed = true;
        let mut quiet_done = session("c", SessionState::Done);
        quiet_done.alerting = false;
        assert_eq!(played(vec![], vec![snoozed, dismissed, quiet_done]), None);
    }
}

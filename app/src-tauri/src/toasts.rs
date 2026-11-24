use crate::platform::registry;
use beacon_core::session::display_title;
use beacon_core::{SessionState, Snapshot};
use std::collections::HashMap;
use std::io;
use std::path::Path;

const BODY_CHARS: usize = 120;

pub fn register_identity(home: &Path, identifier: &str) -> io::Result<()> {
    let icon = home.join("icon.png");
    std::fs::create_dir_all(home)?;
    std::fs::write(&icon, include_bytes!("../icons/128x128.png"))?;
    let key = format!(r"Software\Classes\AppUserModelId\{identifier}");
    registry::write_string(&key, "DisplayName", "Beacon")?;
    registry::write_string(&key, "IconUri", &icon.to_string_lossy())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone)]
struct Candidate {
    session_id: String,
    name: String,
    state: SessionState,
    activity: Option<String>,
}

#[derive(Default)]
pub struct Toaster {
    seen: Option<HashMap<String, (SessionState, bool)>>,
    batch: Vec<Candidate>,
}

pub fn verb(state: SessionState) -> Option<&'static str> {
    match state {
        SessionState::NeedsPermission => Some("needs permission"),
        SessionState::NeedsAnswer => Some("has a question"),
        SessionState::NeedsPlanApproval => Some("wants plan approval"),
        SessionState::NeedsMcpInput => Some("needs input"),
        SessionState::Error => Some("hit an API error"),
        SessionState::Stalled => Some("may be stalled"),
        SessionState::Done => Some("finished"),
        _ => None,
    }
}

impl Toaster {
    pub fn observe(&mut self, snapshot: &Snapshot) -> bool {
        let current: HashMap<String, (SessionState, bool)> = snapshot
            .sessions
            .iter()
            .map(|session| {
                (
                    session.session_id.clone(),
                    (session.state, session.alerting),
                )
            })
            .collect();
        let Some(seen) = self.seen.replace(current) else {
            return false;
        };
        let was_empty = self.batch.is_empty();
        for session in &snapshot.sessions {
            let entered = session.alerting
                && verb(session.state).is_some()
                && seen.get(&session.session_id) != Some(&(session.state, true));
            if entered {
                self.batch
                    .retain(|candidate| candidate.session_id != session.session_id);
                self.batch.push(Candidate {
                    session_id: session.session_id.clone(),
                    name: display_title(session),
                    state: session.state,
                    activity: session.activity.clone(),
                });
            }
        }
        was_empty && !self.batch.is_empty()
    }

    pub fn drop_finished(&mut self) {
        self.batch
            .retain(|candidate| candidate.state != SessionState::Done);
    }

    pub fn flush_unless_focused(&mut self, focused: Option<&str>) -> Option<Toast> {
        self.batch
            .retain(|candidate| Some(candidate.session_id.as_str()) != focused);
        self.flush()
    }

    pub fn flush(&mut self) -> Option<Toast> {
        let batch = std::mem::take(&mut self.batch);
        match batch.as_slice() {
            [] => None,
            [single] => Some(Toast {
                title: format!("{} {}", single.name, verb(single.state).unwrap_or_default()),
                body: single
                    .activity
                    .as_deref()
                    .unwrap_or_default()
                    .chars()
                    .take(BODY_CHARS)
                    .collect(),
            }),
            several => Some(Toast {
                title: if several
                    .iter()
                    .all(|candidate| candidate.state == SessionState::Done)
                {
                    format!("{} sessions finished", several.len())
                } else {
                    format!("{} sessions need you", several.len())
                },
                body: several
                    .iter()
                    .map(|candidate| candidate.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::{session, snapshot};
    use beacon_core::Session;

    fn with_activity(mut session: Session, activity: &str) -> Session {
        session.activity = Some(activity.to_string());
        session
    }

    fn primed(sessions: Vec<Session>) -> Toaster {
        let mut toaster = Toaster::default();
        assert!(!toaster.observe(&snapshot(sessions)));
        toaster
    }

    #[test]
    fn the_first_snapshot_is_a_baseline() {
        let mut toaster = primed(vec![session("a", SessionState::NeedsPermission)]);
        assert_eq!(toaster.flush(), None);
        assert!(!toaster.observe(&snapshot(vec![session("a", SessionState::NeedsPermission)])));
        assert_eq!(toaster.flush(), None);
    }

    #[test]
    fn one_candidate_names_the_project_and_the_reason() {
        let cases = [
            (SessionState::NeedsPermission, "needs permission"),
            (SessionState::NeedsAnswer, "has a question"),
            (SessionState::NeedsPlanApproval, "wants plan approval"),
            (SessionState::NeedsMcpInput, "needs input"),
            (SessionState::Error, "hit an API error"),
            (SessionState::Stalled, "may be stalled"),
            (SessionState::Done, "finished"),
        ];
        for (state, reason) in cases {
            let mut toaster = primed(vec![session("a", SessionState::Working)]);
            assert!(toaster.observe(&snapshot(vec![with_activity(
                session("a", state),
                "Running npm test"
            )])));
            assert_eq!(
                toaster.flush(),
                Some(Toast {
                    title: format!("project-a {reason}"),
                    body: "Running npm test".to_string(),
                })
            );
        }
    }

    #[test]
    fn the_body_is_cut_to_one_hundred_and_twenty_characters() {
        let mut toaster = primed(vec![]);
        toaster.observe(&snapshot(vec![with_activity(
            session("a", SessionState::NeedsPermission),
            &"é".repeat(500),
        )]));
        assert_eq!(toaster.flush().unwrap().body.chars().count(), 120);
    }

    #[test]
    fn several_candidates_become_one_toast() {
        let mut toaster = primed(vec![]);
        assert!(toaster.observe(&snapshot(vec![session("a", SessionState::NeedsPermission)])));
        assert!(!toaster.observe(&snapshot(vec![
            session("a", SessionState::NeedsPermission),
            session("b", SessionState::Done),
            session("c", SessionState::Error),
        ])));
        assert_eq!(
            toaster.flush(),
            Some(Toast {
                title: "3 sessions need you".to_string(),
                body: "project-a, project-b, project-c".to_string(),
            })
        );
        assert_eq!(toaster.flush(), None);
    }

    #[test]
    fn several_finished_sessions_say_finished() {
        let mut toaster = primed(vec![
            session("a", SessionState::Working),
            session("b", SessionState::Working),
        ]);
        toaster.observe(&snapshot(vec![
            session("a", SessionState::Done),
            session("b", SessionState::Done),
        ]));
        assert_eq!(toaster.flush().unwrap().title, "2 sessions finished");
    }

    #[test]
    fn a_session_counts_once_with_its_latest_state() {
        let mut toaster = primed(vec![session("a", SessionState::Working)]);
        toaster.observe(&snapshot(vec![session("a", SessionState::NeedsPermission)]));
        toaster.observe(&snapshot(vec![session("a", SessionState::Done)]));
        assert_eq!(toaster.flush().unwrap().title, "project-a finished");
    }

    #[test]
    fn snoozed_idle_and_working_sessions_produce_nothing() {
        let mut snoozed = session("a", SessionState::NeedsPermission);
        snoozed.alerting = false;
        let mut toaster = primed(vec![]);
        assert!(!toaster.observe(&snapshot(vec![
            snoozed,
            session("b", SessionState::Working),
            session("c", SessionState::Idle),
            session("d", SessionState::Starting),
        ])));
        assert_eq!(toaster.flush(), None);
    }

    #[test]
    fn the_session_whose_window_has_focus_gets_no_toast() {
        let mut toaster = primed(vec![
            session("a", SessionState::Working),
            session("b", SessionState::Working),
        ]);
        toaster.observe(&snapshot(vec![
            session("a", SessionState::Done),
            session("b", SessionState::NeedsPermission),
        ]));
        assert_eq!(
            toaster.flush_unless_focused(Some("a")),
            Some(Toast {
                title: "project-b needs permission".to_string(),
                body: String::new(),
            })
        );
        toaster.observe(&snapshot(vec![
            session("a", SessionState::NeedsAnswer),
            session("b", SessionState::NeedsPermission),
        ]));
        assert_eq!(toaster.flush_unless_focused(Some("a")), None);
        toaster.observe(&snapshot(vec![
            session("a", SessionState::Done),
            session("b", SessionState::Done),
        ]));
        assert_eq!(
            toaster.flush_unless_focused(None).unwrap().title,
            "2 sessions finished"
        );
    }

    #[test]
    fn finished_sessions_can_be_left_out() {
        let mut toaster = primed(vec![
            session("a", SessionState::Working),
            session("b", SessionState::Working),
        ]);
        toaster.observe(&snapshot(vec![
            session("a", SessionState::Done),
            session("b", SessionState::NeedsAnswer),
        ]));
        toaster.drop_finished();
        assert_eq!(toaster.flush().unwrap().title, "project-b has a question");
        toaster.observe(&snapshot(vec![
            session("a", SessionState::Done),
            session("b", SessionState::Done),
        ]));
        toaster.drop_finished();
        assert_eq!(toaster.flush(), None);
    }

    #[test]
    fn a_session_without_a_project_is_named_by_its_id() {
        let mut nameless = session("0123456789abcdef", SessionState::Done);
        nameless.project = None;
        let mut toaster = primed(vec![]);
        toaster.observe(&snapshot(vec![nameless]));
        assert_eq!(toaster.flush().unwrap().title, "01234567 finished");
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Starting,
    Working,
    NeedsPermission,
    NeedsAnswer,
    NeedsPlanApproval,
    NeedsMcpInput,
    Done,
    Error,
    Idle,
    Stalled,
    Ended,
}

impl SessionState {
    pub fn needs_input(self) -> bool {
        matches!(
            self,
            SessionState::NeedsPermission
                | SessionState::NeedsAnswer
                | SessionState::NeedsPlanApproval
                | SessionState::NeedsMcpInput
        )
    }

    pub fn needs_attention(self) -> bool {
        self.needs_input()
            || matches!(
                self,
                SessionState::Error | SessionState::Stalled | SessionState::Done
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartSource {
    Startup,
    Resume,
    Fork,
    Clear,
    Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionKind {
    Tool,
    Question,
    Plan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Busy,
    Idle,
    Waiting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolEvent {
    Tool,
    SubagentStarted,
    SubagentStopped,
    Task,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    SessionStarted(StartSource),
    PromptSubmitted,
    ToolActivity(ToolEvent),
    PermissionRequested(PermissionKind),
    PermissionResolved,
    ElicitationRequested,
    ElicitationResolved,
    Stopped { background_tasks: u32 },
    StopFailed,
    Interrupted,
    Acknowledged,
    SessionEnded,
    ProcessGone,
    StallTimeout,
    Reconciled(AgentStatus),
    CompactStarted,
    CompactFinished,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Overlays {
    pub compacting: bool,
    pub subagents: u32,
    pub background_tasks: u32,
}

pub fn transition(state: SessionState, input: Input) -> SessionState {
    use SessionState::*;
    match input {
        Input::SessionStarted(StartSource::Startup | StartSource::Resume | StartSource::Fork) => {
            Starting
        }
        Input::SessionStarted(StartSource::Clear) => Idle,
        Input::SessionStarted(StartSource::Compact) => state,
        Input::PromptSubmitted => Working,
        Input::ToolActivity(_) if state.needs_input() => state,
        Input::ToolActivity(_) => Working,
        Input::PermissionRequested(PermissionKind::Tool) => NeedsPermission,
        Input::PermissionRequested(PermissionKind::Question) => NeedsAnswer,
        Input::PermissionRequested(PermissionKind::Plan) => NeedsPlanApproval,
        Input::PermissionResolved => match state {
            NeedsPermission | NeedsAnswer | NeedsPlanApproval => Working,
            other => other,
        },
        Input::ElicitationRequested => NeedsMcpInput,
        Input::ElicitationResolved => match state {
            NeedsMcpInput => Working,
            other => other,
        },
        Input::Stopped {
            background_tasks: 0,
        } => Done,
        Input::Stopped { .. } => Working,
        Input::StopFailed => Error,
        Input::Interrupted => Idle,
        Input::Acknowledged => match state {
            Done | Error => Idle,
            Stalled => Working,
            other => other,
        },
        Input::SessionEnded | Input::ProcessGone => Ended,
        Input::StallTimeout => match state {
            Working => Stalled,
            other => other,
        },
        Input::Reconciled(AgentStatus::Busy) => match state {
            Idle | Done | Stalled | Starting => Working,
            needs if needs.needs_input() => Working,
            other => other,
        },
        Input::Reconciled(AgentStatus::Idle) => match state {
            Working | Stalled | Starting => Idle,
            needs if needs.needs_input() => Idle,
            other => other,
        },
        Input::Reconciled(AgentStatus::Waiting) => match state {
            Working | Starting | Idle | Stalled => NeedsPermission,
            other => other,
        },
        Input::CompactStarted | Input::CompactFinished => state,
    }
}

pub fn overlays_after(overlays: Overlays, input: Input) -> Overlays {
    match input {
        Input::CompactStarted => Overlays {
            compacting: true,
            ..overlays
        },
        Input::CompactFinished | Input::SessionStarted(StartSource::Compact) => Overlays {
            compacting: false,
            ..overlays
        },
        Input::ToolActivity(ToolEvent::SubagentStarted) => Overlays {
            subagents: overlays.subagents + 1,
            ..overlays
        },
        Input::ToolActivity(ToolEvent::SubagentStopped) => Overlays {
            subagents: overlays.subagents.saturating_sub(1),
            ..overlays
        },
        Input::Stopped { background_tasks } => Overlays {
            background_tasks,
            ..overlays
        },
        Input::PromptSubmitted => Overlays {
            background_tasks: 0,
            ..overlays
        },
        _ => overlays,
    }
}

#[cfg(test)]
mod tests {
    use super::AgentStatus::{Busy, Idle as StatusIdle, Waiting};
    use super::SessionState::*;
    use super::*;

    const STATES: [SessionState; 11] = [
        Starting,
        Working,
        NeedsPermission,
        NeedsAnswer,
        NeedsPlanApproval,
        NeedsMcpInput,
        Done,
        Error,
        Idle,
        Stalled,
        Ended,
    ];

    fn variant(input: Input) -> &'static str {
        match input {
            Input::SessionStarted(_) => "SessionStarted",
            Input::PromptSubmitted => "PromptSubmitted",
            Input::ToolActivity(_) => "ToolActivity",
            Input::PermissionRequested(_) => "PermissionRequested",
            Input::PermissionResolved => "PermissionResolved",
            Input::ElicitationRequested => "ElicitationRequested",
            Input::ElicitationResolved => "ElicitationResolved",
            Input::Stopped { .. } => "Stopped",
            Input::StopFailed => "StopFailed",
            Input::Interrupted => "Interrupted",
            Input::Acknowledged => "Acknowledged",
            Input::SessionEnded => "SessionEnded",
            Input::ProcessGone => "ProcessGone",
            Input::StallTimeout => "StallTimeout",
            Input::Reconciled(_) => "Reconciled",
            Input::CompactStarted => "CompactStarted",
            Input::CompactFinished => "CompactFinished",
        }
    }

    fn table() -> Vec<(Input, [SessionState; 11])> {
        let unchanged = STATES;
        let working_unless_needs = [
            Working,
            Working,
            NeedsPermission,
            NeedsAnswer,
            NeedsPlanApproval,
            NeedsMcpInput,
            Working,
            Working,
            Working,
            Working,
            Working,
        ];
        let permission_resolved = [
            Starting,
            Working,
            Working,
            Working,
            Working,
            NeedsMcpInput,
            Done,
            Error,
            Idle,
            Stalled,
            Ended,
        ];
        vec![
            (Input::SessionStarted(StartSource::Startup), [Starting; 11]),
            (Input::SessionStarted(StartSource::Resume), [Starting; 11]),
            (Input::SessionStarted(StartSource::Fork), [Starting; 11]),
            (Input::SessionStarted(StartSource::Clear), [Idle; 11]),
            (Input::SessionStarted(StartSource::Compact), unchanged),
            (Input::PromptSubmitted, [Working; 11]),
            (Input::ToolActivity(ToolEvent::Tool), working_unless_needs),
            (
                Input::ToolActivity(ToolEvent::SubagentStarted),
                working_unless_needs,
            ),
            (
                Input::ToolActivity(ToolEvent::SubagentStopped),
                working_unless_needs,
            ),
            (Input::ToolActivity(ToolEvent::Task), working_unless_needs),
            (
                Input::PermissionRequested(PermissionKind::Tool),
                [NeedsPermission; 11],
            ),
            (
                Input::PermissionRequested(PermissionKind::Question),
                [NeedsAnswer; 11],
            ),
            (
                Input::PermissionRequested(PermissionKind::Plan),
                [NeedsPlanApproval; 11],
            ),
            (Input::PermissionResolved, permission_resolved),
            (Input::ElicitationRequested, [NeedsMcpInput; 11]),
            (
                Input::ElicitationResolved,
                [
                    Starting,
                    Working,
                    NeedsPermission,
                    NeedsAnswer,
                    NeedsPlanApproval,
                    Working,
                    Done,
                    Error,
                    Idle,
                    Stalled,
                    Ended,
                ],
            ),
            (
                Input::Stopped {
                    background_tasks: 0,
                },
                [Done; 11],
            ),
            (
                Input::Stopped {
                    background_tasks: 2,
                },
                [Working; 11],
            ),
            (Input::StopFailed, [Error; 11]),
            (Input::Interrupted, [Idle; 11]),
            (
                Input::Acknowledged,
                [
                    Starting,
                    Working,
                    NeedsPermission,
                    NeedsAnswer,
                    NeedsPlanApproval,
                    NeedsMcpInput,
                    Idle,
                    Idle,
                    Idle,
                    Working,
                    Ended,
                ],
            ),
            (Input::SessionEnded, [Ended; 11]),
            (Input::ProcessGone, [Ended; 11]),
            (
                Input::StallTimeout,
                [
                    Starting,
                    Stalled,
                    NeedsPermission,
                    NeedsAnswer,
                    NeedsPlanApproval,
                    NeedsMcpInput,
                    Done,
                    Error,
                    Idle,
                    Stalled,
                    Ended,
                ],
            ),
            (
                Input::Reconciled(Busy),
                [
                    Working, Working, Working, Working, Working, Working, Working, Error, Working,
                    Working, Ended,
                ],
            ),
            (
                Input::Reconciled(StatusIdle),
                [
                    Idle, Idle, Idle, Idle, Idle, Idle, Done, Error, Idle, Idle, Ended,
                ],
            ),
            (
                Input::Reconciled(Waiting),
                [
                    NeedsPermission,
                    NeedsPermission,
                    NeedsPermission,
                    NeedsAnswer,
                    NeedsPlanApproval,
                    NeedsMcpInput,
                    Done,
                    Error,
                    NeedsPermission,
                    NeedsPermission,
                    Ended,
                ],
            ),
            (Input::CompactStarted, unchanged),
            (Input::CompactFinished, unchanged),
        ]
    }

    #[test]
    fn every_state_and_input_pair_matches_the_table() {
        for (input, expected_by_state) in table() {
            for (state, expected) in STATES.into_iter().zip(expected_by_state) {
                assert_eq!(transition(state, input), expected, "{state:?} + {input:?}");
            }
        }
    }

    #[test]
    fn the_table_covers_every_input_variant_and_parameter() {
        let rows = table();
        let mut variants: Vec<&str> = rows.iter().map(|(input, _)| variant(*input)).collect();
        variants.sort_unstable();
        variants.dedup();
        assert_eq!(variants.len(), 17);
        assert_eq!(rows.len(), 29);
    }

    #[test]
    fn attention_states_are_needs_error_stalled_and_done() {
        let attention: Vec<SessionState> = STATES
            .into_iter()
            .filter(|state| state.needs_attention())
            .collect();
        assert_eq!(
            attention,
            [
                NeedsPermission,
                NeedsAnswer,
                NeedsPlanApproval,
                NeedsMcpInput,
                Done,
                Error,
                Stalled
            ]
        );
    }

    #[test]
    fn compacting_overlay_follows_compaction_events() {
        let compacting = overlays_after(Overlays::default(), Input::CompactStarted);
        assert!(compacting.compacting);
        assert!(!overlays_after(compacting, Input::CompactFinished).compacting);
        assert!(
            !overlays_after(compacting, Input::SessionStarted(StartSource::Compact)).compacting
        );
        assert!(overlays_after(compacting, Input::PromptSubmitted).compacting);
    }

    #[test]
    fn subagent_overlay_counts_with_a_floor_of_zero() {
        let started = overlays_after(
            overlays_after(
                Overlays::default(),
                Input::ToolActivity(ToolEvent::SubagentStarted),
            ),
            Input::ToolActivity(ToolEvent::SubagentStarted),
        );
        assert_eq!(started.subagents, 2);
        let stopped = overlays_after(started, Input::ToolActivity(ToolEvent::SubagentStopped));
        assert_eq!(stopped.subagents, 1);
        let floor = overlays_after(
            Overlays::default(),
            Input::ToolActivity(ToolEvent::SubagentStopped),
        );
        assert_eq!(floor.subagents, 0);
    }

    #[test]
    fn background_tasks_overlay_comes_from_stop_and_clears_on_prompt() {
        let waiting = overlays_after(
            Overlays::default(),
            Input::Stopped {
                background_tasks: 3,
            },
        );
        assert_eq!(waiting.background_tasks, 3);
        assert_eq!(
            overlays_after(waiting, Input::ToolActivity(ToolEvent::Tool)).background_tasks,
            3
        );
        assert_eq!(
            overlays_after(waiting, Input::PromptSubmitted).background_tasks,
            0
        );
        assert_eq!(
            overlays_after(
                waiting,
                Input::Stopped {
                    background_tasks: 0
                }
            )
            .background_tasks,
            0
        );
    }
}

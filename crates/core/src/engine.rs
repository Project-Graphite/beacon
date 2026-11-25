use crate::account::AccountUsage;
use crate::config::Config;
use crate::control::{
    Decision, PendingDecision, Reply, Request, Resolution, ResolvedDecision, SessionDetail,
    Snapshot, TimelineEntry,
};
use crate::events::{self, Common, Event, Meta, ToolUse};
use crate::focus::Host;
use crate::lister::ListedSession;
use crate::log::{Logger, now_ms};
use crate::platform::{ProcessWatch, process_alive, process_image_path};
use crate::preview::input_preview;
use crate::progress::{LABEL_CHARS, StepKind, step_kind};
use crate::prompts::{PromptPath, UNDELIVERED_REASON, followup, stop_output};
use crate::redact::redact;
use crate::session::{
    McpRequest, Session, activity_line, display_title, surface_from_image, title_line,
};
use crate::state::{AgentStatus, Input, PermissionKind, SessionState, StartSource, ToolEvent};
use crate::usage::{SessionUsage, TranscriptTitles, UsageStatus};
use beacon_protocol::{
    DECISION_TIMEOUT_SECS, DOCTOR_EVENT, HookReply, STOP_WAITER_EVENT, is_waiting_event,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::io;
use std::path::Path;
use tokio::sync::{mpsc, oneshot, watch};

const RECENT_TOOL_USES: usize = 16;
const RESOLVED_KEPT: usize = 32;
const DOCTOR_REPLY: &[u8] = br#"{"beacon":"ok"}"#;
const ENDED_KEPT_MS: u64 = 2 * 60 * 60 * 1000;
const ENDED_KEPT: usize = 20;
const ENDED_IN_SNAPSHOT: usize = 5;
const TIMELINE_KEPT: usize = 60;
const LISTINGS_MISSED_BEFORE_ENDING: u32 = 2;
const LAST_MESSAGE_BYTES: usize = 64 * 1024;
const USAGE_EVENTS: [&str; 4] = ["Stop", "SubagentStop", "PostToolBatch", "SessionEnd"];
const LOCATION_EVENTS: [&str; 3] = ["SessionStart", "UserPromptSubmit", "Stop"];
const FOLLOWUP_DELIVERED: &str = "Follow-up delivered from Beacon";

pub type ProcessExit = (String, u32);

pub enum HookOutcome {
    Done,
    Reply(HookReply),
    Wait(u64, oneshot::Receiver<HookReply>),
    Waiter(u64, oneshot::Receiver<HookReply>),
}

struct Pending {
    view: PendingDecision,
    reply: oneshot::Sender<HookReply>,
}

pub struct PromptDelivery {
    session_id: String,
    prompt_id: u64,
}

struct StopWaiter {
    id: u64,
    reply: oneshot::Sender<HookReply>,
}

impl StopWaiter {
    fn release(self) {
        let _ = self.reply.send(HookReply::NoDecision);
    }
}

struct RecentToolUse {
    agent_id: Option<String>,
    tool_name: String,
    tool_input: Value,
    tool_use_id: String,
}

#[derive(Default)]
struct Runtime {
    recent_tool_uses: VecDeque<RecentToolUse>,
    watched_pid: Option<u32>,
    watch: Option<ProcessWatch>,
    transcript_len: Option<u64>,
    quiet_since_ms: u64,
    timeline: VecDeque<TimelineEntry>,
    last_message: Option<String>,
    image_read: bool,
    titled_by_hook: bool,
    transcript_sought: bool,
    missed_listings: u32,
}

enum Followup {
    Nothing,
    ToolUsed(ToolUse),
    ToolRan(Option<String>),
    BatchFinished(Vec<String>),
    PromptDelivered(u64, String),
    PermissionAsked {
        tool_name: String,
        tool_input: Value,
        permission_suggestions: Value,
    },
}

#[derive(Serialize, Deserialize)]
struct PersistedState {
    sessions: Vec<Session>,
    #[serde(default)]
    snoozed_until_ms: Option<u64>,
}

pub struct Engine {
    config: Config,
    pub log: Logger,
    sessions: BTreeMap<String, Session>,
    runtime: HashMap<String, Runtime>,
    pending: BTreeMap<u64, Pending>,
    resolved: VecDeque<ResolvedDecision>,
    next_id: u64,
    next_prompt_id: u64,
    changes: watch::Sender<u64>,
    process_exits: mpsc::UnboundedSender<ProcessExit>,
    snoozed_until_ms: Option<u64>,
    account_usage: Option<AccountUsage>,
    usage_wanted: BTreeMap<String, String>,
    plans_wanted: Vec<u64>,
    transcripts_wanted: Vec<String>,
    waiters: HashMap<String, StopWaiter>,
    next_waiter_id: u64,
    reconcile_wanted: bool,
    delivery: Option<PromptDelivery>,
}

impl Engine {
    pub fn new(
        config: Config,
        log: Logger,
        sessions: Vec<Session>,
    ) -> (Engine, mpsc::UnboundedReceiver<ProcessExit>) {
        let (process_exits, exits) = mpsc::unbounded_channel();
        let next_prompt_id = sessions
            .iter()
            .flat_map(|session| &session.prompts)
            .map(|prompt| prompt.id + 1)
            .max()
            .unwrap_or(1);
        let mut engine = Engine {
            config,
            log,
            sessions: sessions
                .into_iter()
                .map(|session| (session.session_id.clone(), session))
                .collect(),
            runtime: HashMap::new(),
            pending: BTreeMap::new(),
            resolved: VecDeque::new(),
            next_id: now_ms(),
            next_prompt_id,
            changes: watch::channel(0).0,
            process_exits,
            snoozed_until_ms: None,
            account_usage: None,
            usage_wanted: BTreeMap::new(),
            plans_wanted: Vec::new(),
            transcripts_wanted: Vec::new(),
            waiters: HashMap::new(),
            next_waiter_id: 1,
            reconcile_wanted: false,
            delivery: None,
        };
        engine.prune_ended(now_ms());
        engine.settle_snoozes(now_ms());
        engine.settle_sessions();
        engine.usage_wanted = engine
            .sessions
            .values()
            .filter_map(|session| {
                Some((session.session_id.clone(), session.transcript_path.clone()?))
            })
            .collect();
        let live: Vec<ProcessExit> = engine
            .sessions
            .values()
            .filter(|session| session.state != SessionState::Ended)
            .filter_map(|session| Some((session.session_id.clone(), session.claude_pid?)))
            .collect();
        for (session_id, pid) in live {
            engine.watch_process(&session_id, pid);
        }
        (engine, exits)
    }

    pub fn load_sessions(bytes: &[u8]) -> io::Result<Vec<Session>> {
        Ok(serde_json::from_slice::<PersistedState>(bytes)?.sessions)
    }

    pub fn load_global_snooze(bytes: &[u8]) -> io::Result<Option<u64>> {
        Ok(serde_json::from_slice::<PersistedState>(bytes)?.snoozed_until_ms)
    }

    pub fn persisted_state(&self) -> io::Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(&PersistedState {
            sessions: self.sessions.values().cloned().collect(),
            snoozed_until_ms: self.snoozed_until_ms,
        })?)
    }

    pub fn watch_changes(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    pub fn needs_user(&self) -> BTreeSet<String> {
        self.pending
            .keys()
            .map(|id| format!("decision {id}"))
            .chain(
                self.sessions
                    .values()
                    .filter(|session| session.alerting)
                    .map(|session| format!("session {}", session.session_id)),
            )
            .collect()
    }

    pub fn live_claude_pids(&self) -> Vec<u32> {
        self.sessions
            .values()
            .filter(|session| session.state != SessionState::Ended)
            .filter_map(|session| {
                let pid = session.claude_pid?;
                process_alive(pid, session.claude_pid_created).then_some(pid)
            })
            .collect()
    }

    pub fn has_live_sessions(&self) -> bool {
        self.sessions
            .values()
            .any(|session| session.state != SessionState::Ended)
    }

    pub fn snapshot(&self) -> Snapshot {
        let ended = self.ended_newest_first();
        let shown_ended: HashSet<&str> = ended
            .iter()
            .filter(|session| !session.dismissed)
            .take(ENDED_IN_SNAPSHOT)
            .map(|session| session.session_id.as_str())
            .collect();
        Snapshot {
            sessions: self
                .sessions
                .values()
                .filter(|session| {
                    session.state != SessionState::Ended
                        || shown_ended.contains(session.session_id.as_str())
                })
                .map(in_snapshot)
                .collect(),
            ended_count: ended.iter().filter(|session| !session.dismissed).count(),
            pending: self
                .pending
                .values()
                .map(|pending| pending.view.clone())
                .collect(),
            resolved: self.resolved.iter().cloned().collect(),
            snoozed_until_ms: self.snoozed_until_ms,
            account_usage: self.account_usage.clone(),
            config: self.config.live(),
        }
    }

    fn ended_newest_first(&self) -> Vec<&Session> {
        let mut ended: Vec<&Session> = self
            .sessions
            .values()
            .filter(|session| session.state == SessionState::Ended)
            .collect();
        ended.sort_by_key(|session| std::cmp::Reverse(ended_at(session)));
        ended
    }

    pub fn ended(&self) -> Vec<Session> {
        self.ended_newest_first().into_iter().map(shown).collect()
    }

    pub fn set_config(
        &mut self,
        home: &Path,
        stall_minutes: Option<u64>,
        answer_unattended: Option<bool>,
        clear_finished_minutes: Option<u64>,
    ) -> Result<(), String> {
        self.config.change(
            home,
            stall_minutes,
            answer_unattended,
            clear_finished_minutes,
        )?;
        self.log.info(format_args!(
            "configuration changed: {:?}",
            self.config.live()
        ));
        self.changed();
        Ok(())
    }

    pub fn control(&mut self, request: Request) -> Reply {
        let result = match request {
            Request::Snapshot | Request::Subscribe => return Reply::with_snapshot(self.snapshot()),
            Request::Reconcile => {
                self.reconcile_wanted = true;
                self.changed();
                return Reply::ok();
            }
            Request::Ended => {
                return Reply {
                    ended: Some(self.ended()),
                    ..Reply::ok()
                };
            }
            Request::Detail { session_id } => {
                return self
                    .detail(&session_id)
                    .map_or_else(Reply::failed, |detail| Reply {
                        detail: Some(detail),
                        ..Reply::ok()
                    });
            }
            Request::Decide { id, decision } => self.decide(id, decision),
            Request::Acknowledge { session_id } => self.acknowledge(&session_id, now_ms()),
            Request::Dismiss { session_id } => self.dismiss(&session_id, now_ms()),
            Request::Undismiss { session_id } => self.undismiss(&session_id),
            Request::Snooze { minutes: 0, .. } => {
                Err("a snooze needs at least one minute".to_string())
            }
            Request::Snooze {
                session_id,
                minutes,
            } => self.snooze(
                session_id.as_deref(),
                Some(now_ms().saturating_add(minutes.saturating_mul(60_000))),
            ),
            Request::Unsnooze { session_id } => self.snooze(session_id.as_deref(), None),
            Request::QueuePrompt { session_id, text } => {
                return self.queue_prompt(&session_id, &text, now_ms()).map_or_else(
                    Reply::failed,
                    |path| Reply {
                        prompt_path: Some(path),
                        ..Reply::ok()
                    },
                );
            }
            Request::CancelPrompt { id } => self.cancel_prompt(id),
            Request::RefreshUsage | Request::SetConfig { .. } | Request::Shutdown => Ok(()),
        };
        result.map_or_else(Reply::failed, |()| Reply::ok())
    }

    pub fn snooze(
        &mut self,
        session_id: Option<&str>,
        until_ms: Option<u64>,
    ) -> Result<(), String> {
        match session_id {
            None => self.snoozed_until_ms = until_ms,
            Some(session_id) => {
                self.sessions
                    .get_mut(session_id)
                    .ok_or_else(|| format!("no session {session_id}"))?
                    .snoozed_until_ms = until_ms;
            }
        }
        self.changed();
        Ok(())
    }

    fn queue_prompt(
        &mut self,
        session_id: &str,
        text: &str,
        now: u64,
    ) -> Result<PromptPath, String> {
        let prompt_id = self.next_prompt_id;
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| format!("no session {session_id}"))?;
        let path = session.queue_prompt(prompt_id, text, now)?;
        self.next_prompt_id += 1;
        if path == PromptPath::Wake
            && let Some(waiter) = self.waiters.remove(session_id)
            && waiter
                .reply
                .send(HookReply::Rewake(followup(text).into_bytes()))
                .is_err()
        {
            session.fail_delivered_prompt(prompt_id, UNDELIVERED_REASON);
            self.changed();
            return Err(format!(
                "session {session_id} is no longer waiting for a prompt; the prompt was not delivered"
            ));
        }
        self.log.info(format_args!(
            "prompt {prompt_id} for session {session_id}: {path:?}"
        ));
        self.changed();
        Ok(path)
    }

    pub fn take_delivery(&mut self) -> Option<PromptDelivery> {
        self.delivery.take()
    }

    pub fn prompt_undelivered(&mut self, delivery: PromptDelivery) {
        let PromptDelivery {
            session_id,
            prompt_id,
        } = delivery;
        let Some(session) = self.sessions.get_mut(&session_id) else {
            return;
        };
        if !session.fail_delivered_prompt(prompt_id, UNDELIVERED_REASON) {
            return;
        }
        self.log.warn(format_args!(
            "prompt {prompt_id} did not reach the stop hook of session {session_id}"
        ));
        let now = now_ms();
        session.apply(
            Input::Stopped {
                background_tasks: 0,
            },
            now,
        );
        session.add_step(StepKind::Notice, "Follow-up could not be delivered", now);
        self.record(&session_id, "Stop", "Follow-up could not be delivered", now);
        self.changed();
    }

    fn cancel_prompt(&mut self, id: u64) -> Result<(), String> {
        self.sessions
            .values_mut()
            .find_map(|session| session.cancel_prompt(id))
            .ok_or_else(|| format!("no prompt {id}"))??;
        self.changed();
        Ok(())
    }

    pub fn next_snooze_expiry(&self) -> Option<u64> {
        self.sessions
            .values()
            .filter_map(|session| session.snoozed_until_ms)
            .chain(self.snoozed_until_ms)
            .min()
    }

    pub fn expire_snoozes(&mut self, now: u64) {
        if self
            .next_snooze_expiry()
            .is_some_and(|expiry| expiry <= now)
        {
            self.changed_at(now);
        }
    }

    fn settle_snoozes(&mut self, now: u64) {
        let active = |until: Option<u64>| until.filter(|until| *until > now);
        self.snoozed_until_ms = active(self.snoozed_until_ms);
        let globally_snoozed = self.snoozed_until_ms.is_some();
        for session in self.sessions.values_mut() {
            session.snoozed_until_ms = active(session.snoozed_until_ms);
            session.alerting = session.attention
                && session.snoozed_until_ms.is_none()
                && !globally_snoozed
                && !session.dismissed;
        }
    }

    pub fn detail(&self, session_id: &str) -> Result<SessionDetail, String> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| format!("no session {session_id}"))?;
        let runtime = self.runtime.get(session_id);
        Ok(SessionDetail {
            session_id: session_id.to_string(),
            timeline: runtime
                .map(|runtime| runtime.timeline.iter().cloned().collect())
                .unwrap_or_default(),
            last_message: runtime.and_then(|runtime| runtime.last_message.clone()),
            steps: session.steps.iter().cloned().collect(),
            tasks: session.tasks.clone(),
        })
    }

    pub fn has_session(&self, session_id: &str) -> bool {
        self.sessions.contains_key(session_id)
    }

    pub fn take_usage_wanted(&mut self) -> BTreeMap<String, String> {
        std::mem::take(&mut self.usage_wanted)
    }

    pub fn take_plans_wanted(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.plans_wanted)
    }

    pub fn take_reconcile_wanted(&mut self) -> bool {
        std::mem::take(&mut self.reconcile_wanted)
    }

    pub fn take_transcripts_wanted(&mut self) -> Vec<String> {
        std::mem::take(&mut self.transcripts_wanted)
    }

    pub fn adopt_transcript(&mut self, session_id: &str, transcript: String) {
        let Some(session) = self
            .sessions
            .get_mut(session_id)
            .filter(|session| session.transcript_path.is_none())
        else {
            return;
        };
        session.transcript_path = Some(transcript.clone());
        self.usage_wanted.insert(session_id.to_string(), transcript);
        self.changed();
    }

    pub fn set_titles(&mut self, session_id: &str, titles: TranscriptTitles) {
        let titled_by_hook = self
            .runtime
            .get(session_id)
            .is_some_and(|runtime| runtime.titled_by_hook);
        let Some(session) = self.sessions.get_mut(session_id) else {
            return;
        };
        if let Some(ai_title) = titles.ai_title {
            session.ai_title = Some(title_line(&ai_title));
        }
        if let Some(last_prompt) = titles.last_prompt {
            session.last_prompt = Some(title_line(&last_prompt));
        }
        if let Some(custom_title) = titles.custom_title.filter(|_| !titled_by_hook) {
            session.title = Some(title_line(&custom_title));
        }
        self.changed();
    }

    pub fn waiter_gone(&mut self, id: u64) {
        self.waiters.retain(|_, waiter| waiter.id != id);
        self.changed();
    }

    fn register_waiter(&mut self, session_id: String) -> HookOutcome {
        let (reply, receiver) = oneshot::channel();
        let id = self.next_waiter_id;
        self.next_waiter_id += 1;
        self.reconcile_wanted |= !self.sessions.contains_key(&session_id);
        if let Some(older) = self
            .waiters
            .insert(session_id.clone(), StopWaiter { id, reply })
        {
            older.release();
        }
        self.changed();
        self.log_event(STOP_WAITER_EVENT, &session_id);
        HookOutcome::Waiter(id, receiver)
    }

    fn settle_sessions(&mut self) {
        let sessions = &self.sessions;
        for (_, waiter) in self.waiters.extract_if(|session_id, _| {
            sessions
                .get(session_id)
                .is_some_and(|session| session.state == SessionState::Ended)
        }) {
            waiter.release();
        }
        for session in self.sessions.values_mut() {
            session.can_wake = self.waiters.contains_key(&session.session_id)
                && matches!(
                    session.state,
                    SessionState::Idle | SessionState::Done | SessionState::Error
                );
            session.display_title = display_title(session);
        }
    }

    fn log_event(&self, event: &str, session_id: &str) {
        let session = self.sessions.get(session_id);
        self.log.info(format_args!(
            "{event} {} entrypoint {} state {}",
            session_id.chars().take(8).collect::<String>(),
            session
                .and_then(|session| session.entrypoint.as_deref())
                .unwrap_or("-"),
            session.map_or_else(|| "-".to_string(), |session| format!("{:?}", session.state))
        ));
    }

    pub fn plan_source(&self, id: u64) -> Option<(String, Option<String>)> {
        let view = &self.pending.get(&id)?.view;
        if view.plan.is_some() {
            return None;
        }
        let transcript = self
            .sessions
            .get(&view.session_id)?
            .transcript_path
            .clone()?;
        Some((transcript, view.tool_use_id.clone()))
    }

    pub fn set_plan(&mut self, id: u64, plan: String) {
        if let Some(pending) = self.pending.get_mut(&id) {
            pending.view.plan = Some(plan);
            self.changed();
        }
    }

    pub fn set_usage(&mut self, session_id: &str, usage: SessionUsage) {
        let Some(session) = self.sessions.get_mut(session_id) else {
            return;
        };
        if usage.status == UsageStatus::Unrecognized
            && session.usage.as_ref().map(|usage| usage.status) != Some(UsageStatus::Unrecognized)
        {
            self.log.warn(format_args!(
                "the transcript of session {session_id} has assistant lines without a recognised usage object"
            ));
        }
        session.usage = Some(usage);
        self.changed();
    }

    pub fn offer_account_usage(&mut self, usage: AccountUsage) {
        if self
            .account_usage
            .as_ref()
            .is_some_and(|current| current.fetched_ms > usage.fetched_ms || *current == usage)
        {
            return;
        }
        self.account_usage = Some(usage);
        self.changed();
    }

    pub fn handle_hook(&mut self, meta: &[u8], payload: &[u8]) -> HookOutcome {
        let meta: Meta = match serde_json::from_slice(meta) {
            Ok(meta) => meta,
            Err(error) => {
                self.log
                    .warn("dropping a hook request with unreadable meta");
                self.log.debug(format_args!("unreadable meta: {error}"));
                return HookOutcome::Done;
            }
        };
        if meta.event == DOCTOR_EVENT {
            return HookOutcome::Reply(HookReply::Stdout(DOCTOR_REPLY.to_vec()));
        }
        let outcome = match events::parse(payload) {
            Ok(_) if meta.event == STOP_WAITER_EVENT && meta.unattended() => {
                Some(HookOutcome::Reply(HookReply::NoDecision))
            }
            Ok(parsed) if meta.event == STOP_WAITER_EVENT => {
                Some(self.register_waiter(parsed.common.session_id))
            }
            Ok(parsed) => {
                let event_name = parsed.common.hook_event_name.clone();
                let session_id = parsed.common.session_id.clone();
                let outcome = self.apply_event(&meta, parsed.common, parsed.event);
                self.changed();
                self.log_event(&event_name, &session_id);
                outcome
            }
            Err(rejected) => {
                self.log.warn(format_args!(
                    "dropping a {} hook payload: {}",
                    meta.event, rejected.reason
                ));
                if let Some(detail) = rejected.detail {
                    self.log
                        .debug(format_args!("{} payload detail: {detail}", meta.event));
                }
                None
            }
        };
        outcome.unwrap_or(if is_waiting_event(&meta.event) {
            HookOutcome::Reply(HookReply::NoDecision)
        } else {
            HookOutcome::Done
        })
    }

    fn apply_event(&mut self, meta: &Meta, common: Common, event: Event) -> Option<HookOutcome> {
        let now = now_ms();
        let session_id = common.session_id;
        if matches!(
            event,
            Event::UserPromptSubmit { .. } | Event::Stop { .. } | Event::SessionEnd { .. }
        ) {
            self.expire_pending(&session_id);
        }
        self.reconcile_wanted |= !self.sessions.contains_key(&session_id);
        let session = self
            .sessions
            .entry(session_id.clone())
            .or_insert_with(|| Session::new(&session_id, now));
        session.limited = false;
        session.last_event_ms = now;
        session.event_count += 1;
        session.acknowledge_prompts();
        if let Some(cwd) = &common.cwd
            && (session.cwd.as_ref() != Some(cwd)
                || LOCATION_EVENTS.contains(&common.hook_event_name.as_str()))
        {
            session.set_location(cwd, meta.env("CLAUDE_PROJECT_DIR"));
        }
        let transcript_changed =
            common.transcript_path.is_some() && common.transcript_path != session.transcript_path;
        if transcript_changed {
            session.transcript_path = common.transcript_path;
        }
        let reads_usage =
            transcript_changed || USAGE_EVENTS.contains(&common.hook_event_name.as_str());
        if common.permission_mode.is_some() {
            session.permission_mode = common.permission_mode.clone();
        }
        if let Some(entrypoint) = meta.env("CLAUDE_CODE_ENTRYPOINT") {
            session.entrypoint = Some(entrypoint.to_string());
        }
        if let Some(attended) = meta.env("CLAUDE_CODE_SESSION_ATTENDED") {
            session.attended = Some(attended != "0");
        }
        let pid = meta
            .env("CLAUDE_PID")
            .and_then(|pid| pid.parse::<u32>().ok());
        if let Some(pid) = pid {
            session.set_claude_pid(pid);
        }
        if session.host.is_none() {
            session.host = Host::from_env(&meta.env);
        }
        let mut label = redact(&timeline_label(
            &common.hook_event_name,
            &event,
            session.cwd.as_deref(),
        ));
        let mut kind = step_kind(
            &common.hook_event_name,
            match &event {
                Event::PreToolUse(tool_use)
                | Event::PostToolUse(tool_use)
                | Event::PostToolUseFailure(tool_use) => Some(tool_use.tool_name.as_str()),
                Event::PermissionRequest { tool_name, .. } => Some(tool_name.as_str()),
                _ => None,
            },
        );
        let tool_step = matches!(
            event,
            Event::PreToolUse(_) | Event::PostToolUse(_) | Event::PostToolUseFailure(_)
        );
        if matches!(
            event,
            Event::SessionStart {
                session_title: Some(_),
                ..
            } | Event::UserPromptSubmit {
                session_title: Some(_),
                ..
            }
        ) {
            self.runtime
                .entry(session_id.clone())
                .or_default()
                .titled_by_hook = true;
        }
        let (input, followup) = match event {
            Event::SessionStart {
                source,
                model,
                session_title,
            } => {
                session.model = model.or(session.model.take());
                session.title = session_title.or(session.title.take());
                session.dismissed = false;
                if source.as_deref() == Some("clear") {
                    session.clear_tasks();
                }
                (
                    start_source(source.as_deref()).map(Input::SessionStarted),
                    Followup::Nothing,
                )
            }
            Event::UserPromptSubmit {
                session_title,
                prompt,
            } => {
                if let Some(waiter) = self.waiters.remove(&session_id) {
                    waiter.release();
                }
                if let Some(prompt) = prompt {
                    session.last_prompt = Some(title_line(&prompt));
                }
                session.title = session_title.or(session.title.take());
                session.turn_started_ms = Some(now);
                session.dismissed = false;
                (Some(Input::PromptSubmitted), Followup::Nothing)
            }
            Event::PreToolUse(tool_use) => {
                session.activity = Some(label.clone());
                session.start_step(&tool_use, kind, &label, now);
                (
                    Some(Input::ToolActivity(ToolEvent::Tool)),
                    Followup::ToolUsed(tool_use),
                )
            }
            Event::PostToolUse(tool_use) | Event::PostToolUseFailure(tool_use) => {
                session.activity = Some(label.clone());
                session.finish_step(
                    &tool_use,
                    kind,
                    &label,
                    common.hook_event_name == "PostToolUseFailure",
                    now,
                );
                (
                    Some(Input::ToolActivity(ToolEvent::Tool)),
                    Followup::ToolRan(tool_use.tool_use_id),
                )
            }
            Event::PostToolBatch { tool_calls } => (
                Some(Input::ToolActivity(ToolEvent::Tool)),
                Followup::BatchFinished(
                    tool_calls
                        .into_iter()
                        .filter_map(|call| call.tool_use_id)
                        .collect(),
                ),
            ),
            Event::SubagentStart { .. } => (
                Some(Input::ToolActivity(ToolEvent::SubagentStarted)),
                Followup::Nothing,
            ),
            Event::SubagentStop => (
                Some(Input::ToolActivity(ToolEvent::SubagentStopped)),
                Followup::Nothing,
            ),
            Event::TaskCreated {
                task_id,
                task_subject,
            } => {
                session.create_task(
                    task_id.unwrap_or_default(),
                    task_subject.as_deref().unwrap_or_default(),
                );
                (
                    Some(Input::ToolActivity(ToolEvent::Task)),
                    Followup::Nothing,
                )
            }
            Event::TaskCompleted {
                task_id,
                task_subject,
            } => {
                session.complete_task(
                    task_id.unwrap_or_default(),
                    task_subject.as_deref().unwrap_or_default(),
                );
                (
                    Some(Input::ToolActivity(ToolEvent::Task)),
                    Followup::Nothing,
                )
            }
            Event::PermissionRequest {
                tool_name,
                tool_input,
                permission_suggestions,
            } if meta.event == "PermissionRequest" => {
                session.dismissed = false;
                (
                    None,
                    Followup::PermissionAsked {
                        tool_name,
                        tool_input: tool_input.unwrap_or(Value::Null),
                        permission_suggestions: permission_suggestions.unwrap_or(Value::Null),
                    },
                )
            }
            Event::Stop {
                last_assistant_message,
                background_tasks,
            } => {
                if let Some(mut message) = last_assistant_message.as_deref().map(redact) {
                    session.set_preview(&message);
                    message.truncate(message.floor_char_boundary(LAST_MESSAGE_BYTES));
                    self.runtime
                        .entry(session_id.clone())
                        .or_default()
                        .last_message = Some(message);
                }
                match (meta.event == "Stop")
                    .then(|| session.deliver_next_prompt())
                    .flatten()
                {
                    Some((prompt_id, text)) => {
                        label = FOLLOWUP_DELIVERED.to_string();
                        kind = StepKind::Prompt;
                        (
                            Some(Input::ToolActivity(ToolEvent::Tool)),
                            Followup::PromptDelivered(prompt_id, text),
                        )
                    }
                    None => {
                        let background_tasks =
                            background_tasks.map_or(0, |tasks| tasks.len() as u32);
                        (Some(Input::Stopped { background_tasks }), Followup::Nothing)
                    }
                }
            }
            Event::StopFailure { error } => {
                session.last_error = error.as_deref().map(redact);
                (Some(Input::StopFailed), Followup::Nothing)
            }
            Event::SessionEnd { .. } => (Some(Input::SessionEnded), Followup::Nothing),
            Event::Notification { notification_type } => (
                match notification_type.as_deref() {
                    Some("elicitation_dialog") => Some(Input::ElicitationRequested),
                    Some("elicitation_response") => Some(Input::ElicitationResolved),
                    _ => None,
                },
                Followup::Nothing,
            ),
            Event::Elicitation {
                mcp_server_name,
                message,
            } => {
                session.dismissed = false;
                session.mcp_request = Some(McpRequest {
                    server: mcp_server_name.unwrap_or_default(),
                    message: message.as_deref().map(redact).unwrap_or_default(),
                });
                (Some(Input::ElicitationRequested), Followup::Nothing)
            }
            Event::ElicitationResult => (Some(Input::ElicitationResolved), Followup::Nothing),
            Event::CwdChanged { new_cwd } => {
                if let Some(new_cwd) = &new_cwd {
                    session.set_location(new_cwd, meta.env("CLAUDE_PROJECT_DIR"));
                }
                (None, Followup::Nothing)
            }
            Event::PostModelSwitch { to_model } => {
                session.model = to_model.or(session.model.take());
                (None, Followup::Nothing)
            }
            Event::PreCompact => (Some(Input::CompactStarted), Followup::Nothing),
            Event::PostCompact => (Some(Input::CompactFinished), Followup::Nothing),
            Event::PermissionRequest { .. } | Event::Other => (None, Followup::Nothing),
        };
        if let Some(input) = input {
            session.apply(input, now);
        }
        if !tool_step {
            session.add_step(kind, &label, now);
        }
        if reads_usage && let Some(path) = &session.transcript_path {
            self.usage_wanted.insert(session_id.clone(), path.clone());
        }
        self.record(&session_id, &common.hook_event_name, &label, now);
        if let Some(pid) = pid {
            self.watch_process(&session_id, pid);
        }
        match followup {
            Followup::Nothing | Followup::ToolRan(None) => None,
            Followup::PromptDelivered(prompt_id, text) => {
                self.log.info(format_args!(
                    "delivering a queued prompt to session {session_id}"
                ));
                self.delivery = Some(PromptDelivery {
                    session_id,
                    prompt_id,
                });
                Some(HookOutcome::Reply(HookReply::Stdout(stop_output(&text))))
            }
            Followup::ToolUsed(tool_use) => {
                self.note_tool_use(&session_id, common.agent_id, tool_use);
                None
            }
            Followup::ToolRan(Some(tool_use_id)) => {
                self.resolve_elsewhere(&session_id, &tool_use_id, Resolution::AllowedInSession);
                None
            }
            Followup::BatchFinished(tool_use_ids) => {
                for tool_use_id in tool_use_ids {
                    self.resolve_elsewhere(&session_id, &tool_use_id, Resolution::DeniedInSession);
                }
                None
            }
            Followup::PermissionAsked {
                tool_name,
                tool_input,
                permission_suggestions,
            } => Some(self.ask_permission(
                meta,
                PendingDecision {
                    id: 0,
                    kind: permission_kind(&tool_name),
                    preview: input_preview(&tool_name, &tool_input),
                    session_id,
                    agent_id: common.agent_id,
                    tool_name,
                    tool_input,
                    permission_suggestions,
                    created_ms: now,
                    tool_use_id: None,
                    cwd: common.cwd,
                    permission_mode: common.permission_mode,
                    agent_type: common.agent_type,
                    expires_ms: now + DECISION_TIMEOUT_SECS * 1000,
                    plan: None,
                },
            )),
        }
    }

    fn ask_permission(&mut self, meta: &Meta, mut view: PendingDecision) -> HookOutcome {
        if meta.unattended() && !self.config.answer_unattended {
            self.log.info(format_args!(
                "not holding {} for unattended session {}",
                view.tool_name, view.session_id
            ));
            return HookOutcome::Reply(HookReply::NoDecision);
        }
        view.id = self.next_id;
        self.next_id += 1;
        view.tool_use_id = self.claim_recent_tool_use(&view);
        if view.kind == PermissionKind::Plan {
            view.plan = view
                .tool_input
                .get("plan")
                .and_then(Value::as_str)
                .filter(|plan| !plan.is_empty())
                .map(str::to_string);
            if view.plan.is_none() {
                self.plans_wanted.push(view.id);
            }
        }
        self.log.info(format_args!(
            "decision {} pending: {} in session {}",
            view.id, view.tool_name, view.session_id
        ));
        let (id, kind, session_id) = (view.id, view.kind, view.session_id.clone());
        let (reply, receiver) = oneshot::channel();
        self.pending.insert(id, Pending { view, reply });
        self.apply(&session_id, Input::PermissionRequested(kind));
        HookOutcome::Wait(id, receiver)
    }

    fn claim_recent_tool_use(&mut self, view: &PendingDecision) -> Option<String> {
        let recent = &mut self.runtime.get_mut(&view.session_id)?.recent_tool_uses;
        let index = recent.iter().position(|tool_use| {
            tool_use.agent_id == view.agent_id
                && tool_use.tool_name == view.tool_name
                && tool_use.tool_input == view.tool_input
        })?;
        recent.remove(index).map(|tool_use| tool_use.tool_use_id)
    }

    fn note_tool_use(&mut self, session_id: &str, agent_id: Option<String>, tool_use: ToolUse) {
        let Some(tool_use_id) = tool_use.tool_use_id else {
            return;
        };
        let tool_input = tool_use.tool_input.unwrap_or(Value::Null);
        let unjoined = self.pending.values_mut().find(|pending| {
            pending.view.session_id == session_id
                && pending.view.tool_use_id.is_none()
                && pending.view.agent_id == agent_id
                && pending.view.tool_name == tool_use.tool_name
                && pending.view.tool_input == tool_input
        });
        if let Some(pending) = unjoined {
            pending.view.tool_use_id = Some(tool_use_id);
            return;
        }
        let recent = &mut self
            .runtime
            .entry(session_id.to_string())
            .or_default()
            .recent_tool_uses;
        if recent.len() == RECENT_TOOL_USES {
            recent.pop_front();
        }
        recent.push_back(RecentToolUse {
            agent_id,
            tool_name: tool_use.tool_name,
            tool_input,
            tool_use_id,
        });
    }

    fn resolve_elsewhere(&mut self, session_id: &str, tool_use_id: &str, resolution: Resolution) {
        if let Some(runtime) = self.runtime.get_mut(session_id) {
            runtime
                .recent_tool_uses
                .retain(|tool_use| tool_use.tool_use_id != tool_use_id);
        }
        let joined = self.pending.iter().find(|(_, pending)| {
            pending.view.session_id == session_id
                && pending.view.tool_use_id.as_deref() == Some(tool_use_id)
        });
        if let Some(id) = joined.map(|(id, _)| *id) {
            self.finish(id, resolution, Some(HookReply::NoDecision), None);
        }
    }

    pub fn decide(&mut self, id: u64, decision: Decision) -> Result<(), String> {
        let pending = self
            .pending
            .get(&id)
            .ok_or_else(|| format!("no pending decision {id}"))?;
        if matches!(decision, Decision::Answer { .. })
            && pending.view.kind != PermissionKind::Question
        {
            return Err(format!("decision {id} is not a question"));
        }
        if pending.view.kind == PermissionKind::Question
            && matches!(
                decision,
                Decision::Allow | Decision::AllowWithInput { .. } | Decision::AllowAlways { .. }
            )
        {
            return Err(format!("decision {id} is a question and needs an answer"));
        }
        let output = hook_output(&decision, &pending.view.tool_input);
        let session_id = pending.view.session_id.clone();
        if !self.finish(
            id,
            Resolution::Decided,
            Some(HookReply::Stdout(output)),
            Some(&decision),
        ) {
            self.changed();
            return Err(format!("decision {id} expired before it was delivered"));
        }
        if matches!(decision, Decision::DenyAndStop { .. }) {
            self.apply(&session_id, Input::Interrupted);
        }
        self.changed();
        Ok(())
    }

    fn expire_pending(&mut self, session_id: &str) {
        let stale: Vec<u64> = self
            .pending
            .iter()
            .filter(|(_, pending)| pending.view.session_id == session_id)
            .map(|(id, _)| *id)
            .collect();
        for id in stale {
            self.finish(id, Resolution::Expired, Some(HookReply::NoDecision), None);
        }
    }

    pub fn helper_gone(&mut self, id: u64) {
        self.finish(id, Resolution::Expired, None, None);
        self.changed();
    }

    fn finish(
        &mut self,
        id: u64,
        resolution: Resolution,
        reply: Option<HookReply>,
        decision: Option<&Decision>,
    ) -> bool {
        let Some(pending) = self.pending.remove(&id) else {
            return false;
        };
        let delivered = reply.is_some_and(|reply| pending.reply.send(reply).is_ok());
        let resolution = match (resolution, delivered) {
            (Resolution::Decided, false) => Resolution::Expired,
            (resolution, _) => resolution,
        };
        self.log
            .info(format_args!("decision {id} resolved: {resolution:?}"));
        let label = match resolution {
            Resolution::Decided
                if matches!(
                    decision,
                    Some(Decision::Deny { .. } | Decision::DenyAndStop { .. })
                ) =>
            {
                "Denied"
            }
            Resolution::Decided => "Allowed",
            Resolution::AllowedInSession => "Allowed in session",
            Resolution::DeniedInSession => "Denied in session",
            Resolution::Expired => "Request expired",
        };
        self.record(
            &pending.view.session_id,
            "PermissionResolved",
            label,
            now_ms(),
        );
        if let Some(session) = self.sessions.get_mut(&pending.view.session_id) {
            session.add_step(
                step_kind("PermissionResolved", Some(&pending.view.tool_name)),
                label,
                now_ms(),
            );
        }
        if self.resolved.len() == RESOLVED_KEPT {
            self.resolved.pop_front();
        }
        let session_id = pending.view.session_id;
        self.resolved.push_back(ResolvedDecision {
            id,
            session_id: session_id.clone(),
            tool_name: pending.view.tool_name,
            resolution,
            resolved_ms: now_ms(),
        });
        if !self
            .pending
            .values()
            .any(|other| other.view.session_id == session_id)
        {
            self.apply(&session_id, Input::PermissionResolved);
        }
        delivered
    }

    pub fn acknowledge(&mut self, session_id: &str, now: u64) -> Result<(), String> {
        let Some(session) = self.sessions.get(session_id) else {
            return Err(format!("no session {session_id}"));
        };
        if session.state == SessionState::Stalled {
            self.runtime
                .entry(session_id.to_string())
                .or_default()
                .quiet_since_ms = now;
        }
        self.apply(session_id, Input::Acknowledged);
        self.changed();
        Ok(())
    }

    pub fn dismiss(&mut self, session_id: &str, now: u64) -> Result<(), String> {
        if self
            .pending
            .values()
            .any(|pending| pending.view.session_id == session_id)
        {
            return Err(format!(
                "session {session_id} is waiting for a decision; answer it before hiding the session"
            ));
        }
        self.sessions
            .get_mut(session_id)
            .ok_or_else(|| format!("no session {session_id}"))?
            .dismissed = true;
        self.acknowledge(session_id, now)
    }

    pub fn undismiss(&mut self, session_id: &str) -> Result<(), String> {
        self.sessions
            .get_mut(session_id)
            .ok_or_else(|| format!("no session {session_id}"))?
            .dismissed = false;
        self.changed();
        Ok(())
    }

    pub fn process_exited(&mut self, session_id: &str, pid: u32) {
        if self
            .sessions
            .get(session_id)
            .is_some_and(|session| session.claude_pid == Some(pid))
        {
            self.log.info(format_args!(
                "process {pid} of session {session_id} is gone"
            ));
            self.expire_pending(session_id);
            self.apply(session_id, Input::ProcessGone);
            self.reconcile_wanted = true;
            self.changed();
        }
    }

    pub fn reconcile(&mut self, listed: Vec<ListedSession>, listing_started_ms: u64) {
        let now = now_ms();
        let listed_ids: HashSet<String> = listed
            .iter()
            .map(|entry| entry.session_id.clone())
            .collect();
        let mut watches = Vec::new();
        for entry in listed {
            let session = self
                .sessions
                .entry(entry.session_id.clone())
                .or_insert_with(|| Session {
                    limited: true,
                    ..Session::new(&entry.session_id, now)
                });
            if session.limited {
                if let Some(cwd) = &entry.cwd {
                    session.set_location(cwd, None);
                }
                if let Some(pid) = entry.pid {
                    session.set_claude_pid(pid);
                }
            } else if let (None, Some(pid)) = (session.claude_pid, entry.pid) {
                session.set_claude_pid(pid);
            }
            if session.entrypoint.is_none()
                && let Some(pid) = session.claude_pid
            {
                let runtime = self.runtime.entry(entry.session_id.clone()).or_default();
                if !runtime.image_read {
                    runtime.image_read = true;
                    if let Some((entrypoint, editor)) = process_image_path(pid)
                        .ok()
                        .as_deref()
                        .and_then(surface_from_image)
                    {
                        session.entrypoint = Some(entrypoint.to_string());
                        session.editor = editor.map(str::to_string);
                    }
                }
            }
            session.listed_name = entry.name;
            match &session.transcript_path {
                None => {
                    let runtime = self.runtime.entry(entry.session_id.clone()).or_default();
                    if !runtime.transcript_sought {
                        runtime.transcript_sought = true;
                        self.transcripts_wanted.push(entry.session_id.clone());
                    }
                }
                Some(transcript) if session.limited => {
                    self.usage_wanted
                        .insert(entry.session_id.clone(), transcript.clone());
                }
                Some(_) => {}
            }
            if let Some(status) = agent_status(entry.status.as_deref())
                && (session.event_count == 0 || session.last_event_ms <= listing_started_ms)
            {
                session.apply(Input::Reconciled(status), now);
            }
            if let Some(pid) = session
                .claude_pid
                .filter(|_| session.state != SessionState::Ended)
            {
                watches.push((entry.session_id, pid));
            }
        }
        for (session_id, pid) in watches {
            self.watch_process(&session_id, pid);
        }
        for session_id in self.sessions.keys() {
            let runtime = self.runtime.entry(session_id.clone()).or_default();
            runtime.missed_listings = if listed_ids.contains(session_id) {
                0
            } else {
                runtime.missed_listings + 1
            };
        }
        let gone: Vec<String> = self
            .sessions
            .values()
            .filter(|session| {
                session.state != SessionState::Ended
                    && !listed_ids.contains(&session.session_id)
                    && (!session.limited
                        || self.runtime[&session.session_id].missed_listings
                            >= LISTINGS_MISSED_BEFORE_ENDING)
                    && !session
                        .claude_pid
                        .is_some_and(|pid| process_alive(pid, session.claude_pid_created))
            })
            .map(|session| session.session_id.clone())
            .collect();
        for session_id in gone {
            self.log.info(format_args!(
                "session {session_id} is no longer listed and its process is gone"
            ));
            self.expire_pending(&session_id);
            self.apply(&session_id, Input::ProcessGone);
        }
        let sessions = &self.sessions;
        for (_, waiter) in self
            .waiters
            .extract_if(|session_id, _| !sessions.contains_key(session_id))
        {
            waiter.release();
        }
        self.changed();
    }

    pub fn check_stalls(&mut self, now: u64) {
        let stall_ms = self.config.stall_minutes.saturating_mul(60_000);
        let mut stalled = Vec::new();
        for session in self
            .sessions
            .values()
            .filter(|session| session.state == SessionState::Working)
        {
            let runtime = self.runtime.entry(session.session_id.clone()).or_default();
            let transcript_len = session
                .transcript_path
                .as_ref()
                .and_then(|path| std::fs::metadata(path).ok())
                .map(|metadata| metadata.len());
            if transcript_len != runtime.transcript_len {
                runtime.transcript_len = transcript_len;
                runtime.quiet_since_ms = now;
            } else if now.saturating_sub(runtime.quiet_since_ms.max(session.last_event_ms))
                >= stall_ms
            {
                stalled.push(session.session_id.clone());
            }
        }
        for session_id in &stalled {
            self.log.info(format_args!("session {session_id} stalled"));
            self.apply(session_id, Input::StallTimeout);
        }
        let clear_ms = self.config.clear_finished_minutes.saturating_mul(60_000);
        let finished: Vec<String> = self
            .sessions
            .values()
            .filter(|session| {
                clear_ms > 0
                    && session.state == SessionState::Done
                    && now.saturating_sub(session.turn_ended_ms.unwrap_or(session.last_event_ms))
                        >= clear_ms
            })
            .map(|session| session.session_id.clone())
            .collect();
        for session_id in &finished {
            self.log.info(format_args!(
                "session {session_id} acknowledged after being finished for {} minutes",
                self.config.clear_finished_minutes
            ));
            self.apply(session_id, Input::Acknowledged);
        }
        if self.prune_ended(now) || !stalled.is_empty() || !finished.is_empty() {
            self.changed();
        }
    }

    fn prune_ended(&mut self, now: u64) -> bool {
        let expired: Vec<String> = self
            .ended_newest_first()
            .into_iter()
            .enumerate()
            .filter(|(index, session)| {
                *index >= ENDED_KEPT || now.saturating_sub(ended_at(session)) > ENDED_KEPT_MS
            })
            .map(|(_, session)| session.session_id.clone())
            .collect();
        for session_id in &expired {
            self.sessions.remove(session_id);
            self.runtime.remove(session_id);
        }
        !expired.is_empty()
    }

    fn watch_process(&mut self, session_id: &str, pid: u32) {
        let runtime = self.runtime.entry(session_id.to_string()).or_default();
        if runtime.watched_pid == Some(pid) {
            return;
        }
        if let Some(session) = self.sessions.get_mut(session_id)
            && !session.keep_claude_pid(pid)
        {
            self.log.info(format_args!(
                "process {pid} of session {session_id} was reused by another process and is dropped"
            ));
            return;
        }
        runtime.watched_pid = Some(pid);
        runtime.watch = None;
        let exits = self.process_exits.clone();
        let watched_session = session_id.to_string();
        let on_exit = Box::new(move || {
            let _ = exits.send((watched_session.clone(), pid));
        });
        match ProcessWatch::start(pid, on_exit) {
            Ok(Some(watch)) => runtime.watch = Some(watch),
            Ok(None) => {
                let _ = self.process_exits.send((session_id.to_string(), pid));
            }
            Err(error) => self.log.warn(format_args!(
                "cannot watch process {pid} of session {session_id}: {error}"
            )),
        }
    }

    fn apply(&mut self, session_id: &str, input: Input) {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.apply(input, now_ms());
        }
    }

    fn record(&mut self, session_id: &str, event: &str, label: &str, now: u64) {
        let timeline = &mut self
            .runtime
            .entry(session_id.to_string())
            .or_default()
            .timeline;
        if timeline.len() == TIMELINE_KEPT {
            timeline.pop_front();
        }
        timeline.push_back(TimelineEntry {
            t_ms: now,
            event: event.to_string(),
            label: redact(label).chars().take(LABEL_CHARS).collect(),
        });
    }

    fn changed(&mut self) {
        self.changed_at(now_ms());
    }

    fn changed_at(&mut self, now: u64) {
        self.prune_ended(now);
        self.settle_snoozes(now);
        self.settle_sessions();
        self.changes.send_modify(|version| *version += 1);
    }
}

fn ended_at(session: &Session) -> u64 {
    session.ended_ms.unwrap_or(session.last_event_ms)
}

fn shown(session: &Session) -> Session {
    Session {
        steps: VecDeque::new(),
        tasks: Vec::new(),
        ..session.without_prompt_texts()
    }
}

fn in_snapshot(session: &Session) -> Session {
    match session.state {
        SessionState::Ended => Session {
            last_message_preview: None,
            ai_title: None,
            last_prompt: None,
            ..shown(session)
        },
        _ => shown(session),
    }
}

fn timeline_label(event_name: &str, event: &Event, cwd: Option<&str>) -> String {
    let described = |text: &str, detail: &Option<String>, shown: fn(&str, &str) -> String| {
        detail
            .as_deref()
            .map_or_else(|| text.to_string(), |detail| shown(text, detail))
    };
    let colon = |text: &str, detail: &str| format!("{text}: {detail}");
    let parenthesised = |text: &str, detail: &str| format!("{text} ({detail})");
    match event {
        Event::PreToolUse(tool_use)
        | Event::PostToolUse(tool_use)
        | Event::PostToolUseFailure(tool_use) => {
            activity_line(&tool_use.tool_name, tool_use.tool_input.as_ref(), cwd)
        }
        Event::UserPromptSubmit { .. } => "Prompt submitted".to_string(),
        Event::PermissionRequest { tool_name, .. } => format!("Permission requested: {tool_name}"),
        Event::Stop { .. } => "Turn finished".to_string(),
        Event::StopFailure { error } => described("API error", error, colon),
        Event::PreCompact => "Compacting".to_string(),
        Event::SessionStart { source, .. } => described("Session started", source, parenthesised),
        Event::SessionEnd { reason } => described("Session ended", reason, parenthesised),
        Event::SubagentStart { agent_type } => described("Subagent started", agent_type, colon),
        Event::TaskCreated { task_subject, .. } => described("Task added", task_subject, colon),
        Event::TaskCompleted { task_subject, .. } => described("Task done", task_subject, colon),
        _ => event_name.to_string(),
    }
}

fn start_source(source: Option<&str>) -> Option<StartSource> {
    match source? {
        "startup" => Some(StartSource::Startup),
        "resume" => Some(StartSource::Resume),
        "fork" => Some(StartSource::Fork),
        "clear" => Some(StartSource::Clear),
        "compact" => Some(StartSource::Compact),
        _ => None,
    }
}

fn agent_status(status: Option<&str>) -> Option<AgentStatus> {
    match status? {
        "busy" => Some(AgentStatus::Busy),
        "idle" => Some(AgentStatus::Idle),
        "waiting" => Some(AgentStatus::Waiting),
        _ => None,
    }
}

fn permission_kind(tool_name: &str) -> PermissionKind {
    match tool_name {
        "AskUserQuestion" => PermissionKind::Question,
        "ExitPlanMode" => PermissionKind::Plan,
        _ => PermissionKind::Tool,
    }
}

pub fn hook_output(decision: &Decision, tool_input: &Value) -> Vec<u8> {
    let decision = match decision {
        Decision::Allow => json!({"behavior": "allow"}),
        Decision::AllowWithInput { input } => json!({"behavior": "allow", "updatedInput": input}),
        Decision::AllowAlways { permissions } => {
            json!({"behavior": "allow", "updatedPermissions": permissions})
        }
        Decision::Answer { answers } => json!({
            "behavior": "allow",
            "updatedInput": {
                "questions": tool_input.get("questions").cloned().unwrap_or(Value::Null),
                "answers": answers,
            },
        }),
        Decision::Deny { message } => deny(message.as_deref(), false),
        Decision::DenyAndStop { message } => deny(message.as_deref(), true),
    };
    json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": decision}})
        .to_string()
        .into_bytes()
}

fn deny(message: Option<&str>, interrupt: bool) -> Value {
    let mut decision = json!({"behavior": "deny"});
    if let Some(message) = message {
        decision["message"] = json!(message);
    }
    if interrupt {
        decision["interrupt"] = json!(true);
    }
    decision
}

#[cfg(test)]
mod tests;

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { openedFrom, type OpenedPayload } from './lib/panel.ts';
import type { AddRules } from './lib/rules.ts';
import { withDefaults, type WireSession } from './lib/sessions.ts';
import { commandMissing, uiSettings } from './lib/settings.ts';

export type SessionState =
  | 'starting'
  | 'working'
  | 'needs_permission'
  | 'needs_answer'
  | 'needs_plan_approval'
  | 'needs_mcp_input'
  | 'done'
  | 'error'
  | 'idle'
  | 'stalled'
  | 'ended';

export type PermissionKind = 'tool' | 'question' | 'plan';

export type StepKind = 'prompt' | 'read' | 'edit' | 'run' | 'search' | 'web' | 'agent' | 'task' | 'mcp' | 'permission' | 'question' | 'plan' | 'notice' | 'finish' | 'error';
export type StepStatus = 'running' | 'done' | 'failed' | 'stopped';
export interface Task { id: string; subject: string; done: boolean; }
export interface CurrentStep { kind: StepKind; label: string; }
export interface Step { id: number; t_ms: number; kind: StepKind; label: string; status: StepStatus; duration_ms: number | null; }

export interface Overlays {
  compacting: boolean;
  subagents: number;
  background_tasks: number;
}

export interface SessionUsage {
  input_tokens: number;
  output_tokens: number;
  cache_creation_tokens: number;
  cache_read_tokens: number;
  context_tokens: number;
  subagent_tokens: number;
  reported_cost_usd: number | null;
  model: string | null;
  updated_ms: number;
  status: 'ok' | 'no_data' | 'unrecognized';
}

export interface Session {
  session_id: string;
  state: SessionState;
  attention: boolean;
  overlays: Overlays;
  cwd: string | null;
  project: string | null;
  branch: string | null;
  transcript_path: string | null;
  entrypoint: string | null;
  claude_pid: number | null;
  attended: boolean | null;
  permission_mode: string | null;
  model: string | null;
  title: string | null;
  listed_name: string | null;
  activity: string | null;
  turn_started_ms: number | null;
  turn_ended_ms: number | null;
  last_event_ms: number;
  event_count: number;
  last_message_preview: string | null;
  last_error: string | null;
  tasks_created: number;
  tasks_completed: number;
  limited: boolean;
  snoozed_until_ms: number | null;
  alerting: boolean;
  editor: string | null;
  usage: SessionUsage | null;
  mcp_request: McpRequest | null;
  host?: SessionHost | null;
  prompts: Prompt[];
  task_current: string | null;
  step_current: CurrentStep | null;
  command_current: string | null;
  dismissed: boolean;
  ai_title?: string | null;
  last_prompt?: string | null;
  display_title?: string;
  can_wake?: boolean;
}

export type PromptStatus = 'queued' | 'delivered' | 'acknowledged' | 'failed';

export interface Prompt {
  id: number;
  preview: string;
  created_ms: number;
  status: PromptStatus;
  reason: string | null;
}

export type PromptPath = 'at_turn_end' | 'wake' | 'prefill' | 'copy_and_jump';

export interface WindowPrompt {
  path: PromptPath;
  outcome: JumpOutcome;
}

export interface SessionHost {
  term_program: string | null;
  wt_session: string | null;
  wt_profile_id: string | null;
  vscode_pid: string | null;
  tmux: string | null;
  tmux_pane: string | null;
  kitty_window_id: string | null;
  wezterm_pane: string | null;
  wsl_distro_name: string | null;
}

export interface McpRequest {
  server: string;
  message: string;
}

export type TargetKind =
  | { type: 'editor'; scheme: string }
  | { type: 'desktop' }
  | { type: 'windows_terminal' }
  | { type: 'console' }
  | { type: 'other' };

export interface JumpOutcome {
  result: 'focused' | 'flashed' | 'no_window';
  kind: TargetKind | null;
  ambiguous: boolean;
  link_opened: boolean;
}

export interface PendingDecision {
  id: number;
  session_id: string;
  agent_id: string | null;
  tool_name: string;
  tool_input: unknown;
  permission_suggestions: unknown;
  kind: PermissionKind;
  created_ms: number;
  tool_use_id: string | null;
  preview: string;
  cwd: string | null;
  permission_mode: string | null;
  agent_type: string | null;
  expires_ms: number;
  plan: string | null;
}

export interface ResolvedDecision {
  id: number;
  session_id: string;
  tool_name: string;
  resolution: 'decided' | 'allowed_in_session' | 'denied_in_session' | 'expired';
  resolved_ms: number;
}

export interface AccountUsage {
  five_hour_percent: number | null;
  five_hour_resets_at: string | null;
  seven_day_percent: number | null;
  seven_day_resets_at: string | null;
  fetched_ms: number | null;
  source: 'cache' | 'probe';
}

export interface LiveConfig {
  stall_minutes: number;
  answer_unattended: boolean;
  clear_finished_minutes: number;
}

export interface Snapshot {
  sessions: Session[];
  ended_count: number;
  pending: PendingDecision[];
  resolved: ResolvedDecision[];
  snoozed_until_ms: number | null;
  account_usage: AccountUsage | null;
  config: LiveConfig;
}

export interface TimelineEntry {
  t_ms: number;
  event: string;
  label: string;
}

export interface SessionDetail {
  session_id: string;
  timeline: TimelineEntry[];
  last_message: string | null;
  steps: Step[];
  tasks: Task[];
}

export type Decision =
  | { type: 'allow' }
  | { type: 'allow_with_input'; input: unknown }
  | { type: 'allow_always'; permissions: AddRules[] }
  | { type: 'answer'; answers: Record<string, string> }
  | { type: 'deny'; message?: string }
  | { type: 'deny_and_stop'; message?: string };

export type PanelMode = 'hover' | 'sticky' | 'pinned';

export type PanelEdge = 'left' | 'right' | 'top' | 'bottom';

export interface Opened {
  mode: PanelMode;
  focused: boolean;
  edge: PanelEdge | null;
}

export type HooksState = 'installed' | 'not_installed' | 'needs_repair';

export type HooksAction = 'install' | 'uninstall';

export interface HooksStatus {
  state: HooksState;
  settings_path: string;
  problems: string[];
  warnings: string[];
}

export type OverlayVisibility = 'always' | 'active' | 'attention' | 'never';

export type OverlaySize = 'small' | 'medium' | 'large';

export type AttentionStyle = 'colour' | 'pulse' | 'pulse_peek';

export type StartMode = 'login' | 'claude' | 'manual';

export interface UiSettings {
  hotkey: string;
  toasts_enabled: boolean;
  toast_on_finished: boolean;
  sounds_enabled: boolean;
  visibility: OverlayVisibility;
  reveal_on_hover: boolean;
  size: OverlaySize;
  idle_opacity: number;
  attention_style: AttentionStyle;
  attention_repeat_seconds: number;
  peek_seconds: number;
  start_mode: StartMode;
  quit_when_idle_minutes: number;
  pin_while_active: boolean;
  open_on_hover: boolean;
}

export type OverlayStyle = 'tab' | 'pill';

export type OverlayCorner = 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right';

export type OverlayPosition = PanelEdge | OverlayCorner;

export interface OverlayDisplay {
  id: string;
  name: string;
  width: number;
  height: number;
  primary: boolean;
  target: boolean;
  avoided: boolean;
}

export interface Overlay {
  style: OverlayStyle;
  position: OverlayPosition | null;
  displays: OverlayDisplay[];
}

export interface AppInfo {
  version: string;
  home: string;
}

type WireSnapshot = Omit<Snapshot, 'sessions'> & { sessions: WireSession[] };
const snapshotFrom = (snapshot: WireSnapshot): Snapshot => ({ ...snapshot, sessions: snapshot.sessions.map(withDefaults) });

interface InitialData {
  snapshot: WireSnapshot | null;
  ui: Partial<UiSettings>;
}

export interface OpenedEvent {
  opened: Opened;
  snapshot: Snapshot | null;
  ui: UiSettings;
}

const initial = (window as unknown as { __BEACON_INITIAL__: InitialData }).__BEACON_INITIAL__;
export const initialSnapshot = initial.snapshot && snapshotFrom(initial.snapshot);
export const initialUi = uiSettings(initial.ui);
const details = new Map<string, SessionDetail>();
export const getDetail = (sessionId: string) =>
  invoke<SessionDetail>('get_detail', { sessionId }).then((detail) => {
    details.set(sessionId, detail);
    return detail;
  });
export const getEnded = () => invoke<WireSession[]>('get_ended').then((ended) => ended.map(withDefaults));
export const cachedDetail = (sessionId: string) => details.get(sessionId) ?? null;
export const acknowledge = (sessionId: string) => invoke<void>('acknowledge', { sessionId });
export const dismiss = (sessionId: string) => invoke<void>('dismiss', { sessionId });
export const decide = (id: number, decision: Decision) => invoke<void>('decide', { id, decision });
export const snooze = (sessionId: string | null, minutes: number) =>
  invoke<void>('snooze', { sessionId, minutes });
export const unsnooze = (sessionId: string | null) => invoke<void>('unsnooze', { sessionId });
export const refreshUsage = () => invoke<void>('refresh_usage');
export const closePanel = () => invoke<void>('close_panel');
export const setPanelHeight = (height: number) => invoke<void>('set_panel_height', { height });
export const jump = (sessionId: string) => invoke<JumpOutcome>('jump', { sessionId });
export const queuePrompt = (sessionId: string, text: string) => invoke<void>('queue_prompt', { sessionId, text });
export const cancelPrompt = (id: number) => invoke<void>('cancel_prompt', { id });
export const sendPromptToWindow = (sessionId: string, text: string) =>
  invoke<WindowPrompt>('send_prompt_to_window', { sessionId, text });
export const setConfig = ({ stall_minutes, answer_unattended, clear_finished_minutes }: Partial<LiveConfig>) =>
  invoke<void>('set_config', {
    stallMinutes: stall_minutes,
    answerUnattended: answer_unattended,
    clearFinishedMinutes: clear_finished_minutes,
  });
let lastHooksStatus: HooksStatus | null = null;
export const hooksStatus = () => invoke<HooksStatus>('hooks_status').then((status) => (lastHooksStatus = status));
export const cachedHooksStatus = () => lastHooksStatus;
export const hooksPlan = (action: HooksAction) => invoke<string>('hooks_plan', { action });
export const hooksApply = (action: HooksAction) => invoke<HooksStatus>('hooks_apply', { action });
export const setUiSettings = (change: Partial<UiSettings>) =>
  invoke<Partial<UiSettings>>('set_ui_settings', { change }).then(uiSettings);
export const previewAttention = () =>
  invoke<void>('preview_attention').catch((error) => {
    if (!commandMissing(error)) throw error;
  });
export const openLogsFolder = () => invoke<void>('open_logs_folder');
let lastAppInfo: AppInfo | null = null;
export const appInfo = () => invoke<AppInfo>('app_info').then((info) => (lastAppInfo = info));
export const cachedAppInfo = () => lastAppInfo;
let lastOverlay: Overlay | null = null;
export const getOverlay = () => invoke<Overlay>('get_overlay').then((overlay) => (lastOverlay = overlay));
export const cachedOverlay = () => lastOverlay;
export const setStyle = (style: OverlayStyle) => invoke<void>('set_style', { style });
export const setDisplay = (id: string) => invoke<void>('set_display', { id });
export const setDisplayAvoided = (id: string, avoided: boolean) => invoke<void>('set_display_avoided', { id, avoided });
export const setOverlayPosition = (position: OverlayPosition) => invoke<void>('set_overlay_position', { position });
export const quitApp = () => invoke<void>('quit_app');

export const onSnapshot = (handler: (snapshot: Snapshot | null) => void): Promise<UnlistenFn> =>
  listen<WireSnapshot | null>('beacon://snapshot', (event) => handler(event.payload && snapshotFrom(event.payload)));

export const onOpened = (handler: (opened: OpenedEvent) => void): Promise<UnlistenFn> =>
  listen<OpenedPayload & InitialData>('beacon://opened', ({ payload }) =>
    handler({ opened: openedFrom(payload), snapshot: payload.snapshot && snapshotFrom(payload.snapshot), ui: uiSettings(payload.ui) }),
  );

export const onClosing = (handler: () => void): Promise<UnlistenFn> => listen('beacon://closing', handler);

export const onSettingsChanged = (handler: () => void): Promise<UnlistenFn> =>
  listen('beacon://settings-changed', handler);

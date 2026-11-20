import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AddRules } from './lib/rules.ts';

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
}

export type PromptStatus = 'queued' | 'delivered' | 'acknowledged' | 'failed';

export interface Prompt {
  id: number;
  preview: string;
  created_ms: number;
  status: PromptStatus;
  reason: string | null;
}

export type PromptPath = 'at_turn_end' | 'prefill' | 'copy_and_jump';

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
}

export interface Snapshot {
  sessions: Session[];
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
}

export type Decision =
  | { type: 'allow' }
  | { type: 'allow_with_input'; input: unknown }
  | { type: 'allow_always'; permissions: AddRules[] }
  | { type: 'answer'; answers: Record<string, string> }
  | { type: 'deny'; message?: string }
  | { type: 'deny_and_stop'; message?: string };

export type PanelMode = 'hover' | 'sticky' | 'pinned';

export interface Opened {
  mode: PanelMode;
  focused: boolean;
}

export type HooksState = 'installed' | 'not_installed' | 'needs_repair';

export type HooksAction = 'install' | 'uninstall';

export interface HooksStatus {
  state: HooksState;
  settings_path: string;
  problems: string[];
  warnings: string[];
}

export interface UiSettings {
  hotkey: string;
  toasts_enabled: boolean;
  toast_on_finished: boolean;
}

export interface AppInfo {
  version: string;
  home: string;
}

export const getSnapshot = () => invoke<Snapshot>('get_snapshot');
export const getDetail = (sessionId: string) => invoke<SessionDetail>('get_detail', { sessionId });
export const acknowledge = (sessionId: string) => invoke<void>('acknowledge', { sessionId });
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
export const setConfig = ({ stall_minutes, answer_unattended }: Partial<LiveConfig>) =>
  invoke<void>('set_config', { stallMinutes: stall_minutes, answerUnattended: answer_unattended });
export const hooksStatus = () => invoke<HooksStatus>('hooks_status');
export const hooksPlan = (action: HooksAction) => invoke<string>('hooks_plan', { action });
export const hooksApply = (action: HooksAction) => invoke<HooksStatus>('hooks_apply', { action });
export const getUiSettings = () => invoke<UiSettings>('get_ui_settings');
export const setUiSettings = (change: Partial<UiSettings>) => invoke<UiSettings>('set_ui_settings', { change });
export const getLoginStart = () => invoke<boolean>('get_login_start');
export const setLoginStart = (enabled: boolean) => invoke<void>('set_login_start', { enabled });
export const openLogsFolder = () => invoke<void>('open_logs_folder');
export const appInfo = () => invoke<AppInfo>('app_info');

export const onSnapshot = (handler: (snapshot: Snapshot | null) => void): Promise<UnlistenFn> =>
  listen<Snapshot | null>('beacon://snapshot', (event) => handler(event.payload));

export const onOpened = (handler: (opened: Opened) => void): Promise<UnlistenFn> =>
  listen<Opened>('beacon://opened', (event) => handler(event.payload));

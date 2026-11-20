import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

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

export interface Snapshot {
  sessions: Session[];
  pending: PendingDecision[];
  resolved: ResolvedDecision[];
  snoozed_until_ms: number | null;
  account_usage: AccountUsage | null;
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

export type PanelMode = 'hover' | 'sticky' | 'pinned';

export interface Opened {
  mode: PanelMode;
  focused: boolean;
}

export const getSnapshot = () => invoke<Snapshot>('get_snapshot');
export const getDetail = (sessionId: string) => invoke<SessionDetail>('get_detail', { sessionId });
export const acknowledge = (sessionId: string) => invoke<void>('acknowledge', { sessionId });
export const snooze = (sessionId: string | null, minutes: number) =>
  invoke<void>('snooze', { sessionId, minutes });
export const unsnooze = (sessionId: string | null) => invoke<void>('unsnooze', { sessionId });
export const refreshUsage = () => invoke<void>('refresh_usage');
export const closePanel = () => invoke<void>('close_panel');
export const setPanelHeight = (height: number) => invoke<void>('set_panel_height', { height });
export const jump = (sessionId: string) => invoke<JumpOutcome>('jump', { sessionId });

export const onSnapshot = (handler: (snapshot: Snapshot | null) => void): Promise<UnlistenFn> =>
  listen<Snapshot | null>('beacon://snapshot', (event) => handler(event.payload));

export const onOpened = (handler: (opened: Opened) => void): Promise<UnlistenFn> =>
  listen<Opened>('beacon://opened', (event) => handler(event.payload));

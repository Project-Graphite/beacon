import type { Session, SessionState } from '../beacon';
import { formatDuration, formatTokens, shortModel } from './format.ts';
import { queuedPrompts } from './prompts.ts';

export type Tone = 'wip' | 'live' | 'danger' | 'muted';

export const TONE_CLASSES: Record<Tone, string> = {
  wip: 'text-wip',
  live: 'text-live',
  danger: 'text-danger',
  muted: 'text-muted',
};
export type SurfaceIcon = 'terminal' | 'headless' | 'editor' | 'desktop' | 'unknown';

export interface SessionGroup {
  label: string;
  sessions: Session[];
}

export interface MetaItem {
  text: string;
  title?: string;
}

export interface SummaryPart {
  kind: 'alerting' | 'working' | 'request';
  count: number;
  text: string;
}

export interface QueuePlace {
  position: number;
  total: number;
}

export type RowLine =
  | { kind: 'tasks'; count: string; text: string | null }
  | { kind: 'step'; text: string }
  | { kind: 'state'; text: string; tone: Tone };

export const cardId = (sessionId: string) => `session-${sessionId}`;

const SESSION_DEFAULTS = {
  attention: false,
  cwd: null,
  project: null,
  branch: null,
  transcript_path: null,
  entrypoint: null,
  claude_pid: null,
  attended: null,
  permission_mode: null,
  model: null,
  title: null,
  listed_name: null,
  activity: null,
  turn_started_ms: null,
  turn_ended_ms: null,
  last_message_preview: null,
  last_error: null,
  limited: false,
  snoozed_until_ms: null,
  alerting: false,
  editor: null,
  usage: null,
  mcp_request: null,
  host: null,
  prompts: [],
  task_current: null,
  step_current: null,
  command_current: null,
  dismissed: false,
  ai_title: null,
  last_prompt: null,
  can_wake: false,
} satisfies Partial<Session>;

export type WireSession = Omit<Session, keyof typeof SESSION_DEFAULTS> & Partial<Session>;

export const withDefaults = (session: WireSession): Session => ({ ...SESSION_DEFAULTS, ...session });

const GROUP_LABELS =['Needs you', 'Snoozed', 'Working', 'Idle'];

const STATE_PHRASES: Record<SessionState, { text: string; tone: Tone }> = {
  needs_permission: { text: 'needs permission', tone: 'wip' },
  needs_answer: { text: 'has a question', tone: 'wip' },
  needs_plan_approval: { text: 'wants plan approval', tone: 'wip' },
  needs_mcp_input: { text: 'needs input', tone: 'wip' },
  stalled: { text: 'may be stalled', tone: 'wip' },
  done: { text: 'finished', tone: 'live' },
  error: { text: 'API error', tone: 'danger' },
  working: { text: 'working', tone: 'muted' },
  starting: { text: 'starting', tone: 'muted' },
  idle: { text: 'idle', tone: 'muted' },
  ended: { text: 'ended', tone: 'muted' },
};

const groupIndex = (session: Session) => {
  if (session.alerting) return 0;
  if (session.attention) return 1;
  if (session.state === 'working' || session.state === 'starting') return 2;
  return 3;
};

const newestFirst = (sessions: Session[]) =>
  [...sessions].sort((a, b) => b.last_event_ms - a.last_event_ms);

export function groupSessions(sessions: Session[]): { groups: SessionGroup[]; ended: Session[] } {
  const listed = sessions.filter((session) => !session.dismissed);
  const live = newestFirst(listed.filter((session) => session.state !== 'ended'));
  return {
    groups: GROUP_LABELS.map((label, index) => ({
      label,
      sessions: live.filter((session) => groupIndex(session) === index),
    })).filter((group) => group.sessions.length > 0),
    ended: newestFirst(listed.filter((session) => session.state === 'ended')),
  };
}

export function crossedOff(sessions: Session[]): Session[] {
  return newestFirst(sessions.filter((session) => session.dismissed));
}

export function sessionTitle(session: Session): string {
  return (
    session.display_title ?? session.title ?? session.listed_name ?? session.project ?? session.session_id.slice(0, 8)
  );
}

export function taskProgress(session: Session): number {
  return session.tasks_created === 0 ? 0 : session.tasks_completed / session.tasks_created;
}

export function findSession(sessions: Session[], ended: Session[], sessionId: string | null): Session | undefined {
  const matches = (session: Session) => session.session_id === sessionId;
  return (
    sessions.find((session) => matches(session) && session.state !== 'ended') ?? ended.find(matches) ?? sessions.find(matches)
  );
}

export function sessionTabs(listOrder: Session[], currentId: string): Session[] {
  return listOrder.filter(
    (session) => session.session_id === currentId || (session.state !== 'ended' && !session.dismissed),
  );
}

export function stepTab(tabs: Session[], currentId: string, step: 1 | -1): string | null {
  if (tabs.length === 0) return null;
  const index = tabs.findIndex((session) => session.session_id === currentId);
  return tabs[(index + step + tabs.length) % tabs.length].session_id;
}

export function summaryParts(sessions: Session[], queue: QueuePlace | null = null): SummaryPart[] {
  const alerting = sessions.filter((session) => session.alerting).length;
  const working = sessions.filter((session) => session.state === 'working').length;
  return [
    { kind: 'alerting' as const, count: alerting, text: alerting === 1 ? 'needs you' : 'need you' },
    { kind: 'working' as const, count: working, text: 'working' },
    { kind: 'request' as const, count: queue && queue.total > 1 ? queue.position : 0, text: `of ${queue?.total} requests` },
  ].filter((part) => part.count > 0);
}

export function summary(sessions: Session[], queue: QueuePlace | null = null): string {
  return summaryParts(sessions, queue).map(({ count, text }) => `${count} ${text}`).join(' · ') || 'All quiet';
}

export function rowLine(session: Session): RowLine {
  if (needsInput(session)) return { kind: 'state', ...statePhrase(session) };
  if (session.tasks_created > 0) {
    return { kind: 'tasks', count: `${session.tasks_completed}/${session.tasks_created}`, text: session.task_current };
  }
  if (session.step_current) return { kind: 'step', text: session.step_current.label };
  return { kind: 'state', ...statePhrase(session) };
}

export function surface(session: Session): { icon: SurfaceIcon; label: string } {
  switch (session.entrypoint) {
    case 'cli':
      return { icon: 'terminal', label: 'terminal' };
    case 'sdk-cli':
      return { icon: 'headless', label: 'headless' };
    case 'claude-vscode':
      return { icon: 'editor', label: session.editor ?? 'VS Code' };
    case 'claude-desktop':
      return { icon: 'desktop', label: 'desktop' };
    case null:
      return { icon: 'unknown', label: 'unknown' };
    default:
      return { icon: 'unknown', label: session.entrypoint };
  }
}

export function statePhrase(session: Session): { text: string; tone: Tone } {
  const phrase = STATE_PHRASES[session.state];
  return session.state === 'error' && session.last_error
    ? { ...phrase, text: `${phrase.text}: ${session.last_error}` }
    : phrase;
}

export function canAcknowledge(session: Session): boolean {
  return session.state === 'done' || session.state === 'error' || session.state === 'stalled';
}

export function needsInput(session: Session): boolean {
  return session.state.startsWith('needs_');
}

export function canSnooze(session: Session): boolean {
  return session.alerting && needsInput(session);
}

export function turnRunning(session: Session): boolean {
  return session.turn_started_ms !== null && session.turn_ended_ms === null;
}

export function turnTime(session: Session, now: number): string | null {
  return session.turn_started_ms === null
    ? null
    : formatDuration((session.turn_ended_ms ?? now) - session.turn_started_ms);
}

export function metaItems(session: Session): MetaItem[] {
  const model = session.model ?? session.usage?.model;
  const { subagents, background_tasks, compacting } = session.overlays;
  const contextTokens = session.usage?.context_tokens ?? 0;
  const queued = queuedPrompts(session);
  return [
    queued > 0 && { text: `${queued} queued` },
    model && { text: shortModel(model) },
    session.permission_mode && session.permission_mode !== 'default' && { text: session.permission_mode },
    subagents > 0 && { text: `sub ${subagents}` },
    background_tasks > 0 && { text: `bg ${background_tasks}` },
    compacting && { text: 'compacting' },
    contextTokens > 0 && { text: `ctx ${formatTokens(contextTokens)}` },
    session.limited && {
      text: 'limited',
      title: 'Known only from claude agents; no hook has fired for this session yet',
    },
  ].filter((item): item is MetaItem => Boolean(item));
}

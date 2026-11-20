import type { Session, SessionState } from '../beacon';
import { formatDuration, formatTokens, shortModel } from './format.ts';

export type Tone = 'wip' | 'live' | 'danger' | 'muted';
export type SurfaceIcon = 'terminal' | 'headless' | 'editor' | 'desktop' | 'unknown';

export interface SessionGroup {
  label: string;
  sessions: Session[];
}

export interface MetaItem {
  text: string;
  title?: string;
}

export const cardId = (sessionId: string) => `session-${sessionId}`;

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
  const live = newestFirst(sessions.filter((session) => session.state !== 'ended'));
  return {
    groups: GROUP_LABELS.map((label, index) => ({
      label,
      sessions: live.filter((session) => groupIndex(session) === index),
    })).filter((group) => group.sessions.length > 0),
    ended: newestFirst(sessions.filter((session) => session.state === 'ended')),
  };
}

export function summary(sessions: Session[]): string {
  const alerting = sessions.filter((session) => session.alerting).length;
  const working = sessions.filter((session) => session.state === 'working').length;
  const parts = [
    alerting > 0 && `${alerting} ${alerting === 1 ? 'needs' : 'need'} you`,
    working > 0 && `${working} working`,
  ].filter(Boolean);
  return parts.length > 0 ? parts.join(' · ') : 'All quiet';
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
  return [
    model && { text: shortModel(model) },
    session.permission_mode && session.permission_mode !== 'default' && { text: session.permission_mode },
    session.tasks_created > 0 && { text: `tasks ${session.tasks_completed}/${session.tasks_created}` },
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

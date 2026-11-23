import type { PendingDecision, ResolvedDecision } from '../beacon';

export interface ResolutionNotice {
  id: number;
  sessionId: string;
  text: string;
}

const NOTICE_TEXTS: Partial<Record<ResolvedDecision['resolution'], string>> = {
  allowed_in_session: 'Allowed in the session',
  denied_in_session: 'Denied in the session',
  expired: 'Request expired: answer in the session window',
};

export const requestId = (id: number) => `request-${id}`;

export function requestQueue(pending: PendingDecision[]): PendingDecision[] {
  return [...pending].sort((a, b) => a.created_ms - b.created_ms || a.id - b.id);
}

export function currentRequest(queue: PendingDecision[], chosenId: number | null): PendingDecision | null {
  return queue.find((request) => request.id === chosenId) ?? queue[0] ?? null;
}

export function requestsOf(queue: PendingDecision[], sessionId: string): PendingDecision[] {
  return queue.filter((request) => request.session_id === sessionId);
}

export function keyTarget(queue: PendingDecision[], chosenId: number | null, sessionId: string | null): PendingDecision | null {
  return currentRequest(sessionId === null ? queue : requestsOf(queue, sessionId), chosenId);
}

export function stepRequest(queue: PendingDecision[], chosenId: number | null, step: 1 | -1): number | null {
  if (queue.length === 0) return null;
  const index = queue.findIndex((request) => request.id === currentRequest(queue, chosenId)?.id);
  return queue[(index + step + queue.length) % queue.length].id;
}

export function firstRequestOf(queue: PendingDecision[], sessionId: string): number | null {
  return queue.find((request) => request.session_id === sessionId)?.id ?? null;
}

export function resolutionNotices(
  previousIds: number[],
  pending: PendingDecision[],
  resolved: ResolvedDecision[],
): ResolutionNotice[] {
  return previousIds
    .filter((id) => !pending.some((request) => request.id === id))
    .flatMap((id) => {
      const entry = resolved.find((each) => each.id === id);
      const text = entry && NOTICE_TEXTS[entry.resolution];
      return entry && text ? [{ id, sessionId: entry.session_id, text }] : [];
    });
}

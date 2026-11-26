import type { PendingDecision, Session } from '../beacon';
import { promptPath } from './prompts.ts';
import { firstRequestOf, keyTarget, requestsOf, stepRequest } from './queue.ts';
import { canAcknowledge, canSnooze, stepTab } from './sessions.ts';

export type FocusKind = 'text' | 'choice' | 'button' | 'body' | 'other';
export type RequestKey = 'a' | 'd' | 'm';

export interface KeyPress {
  key: string;
  shift: boolean;
  modified: boolean;
  repeat: boolean;
  focus: FocusKind;
  inRequest: boolean;
  settled: boolean;
  targetView: 'absent' | 'hidden' | 'shown';
}

export interface KeyFocusState {
  openSession: Session | null;
  selectedId: string | null;
  visible: Session[];
  queue: PendingDecision[];
  chosenRequestId: number | null;
}

export interface KeyState extends KeyFocusState {
  settingsOpen: boolean;
  opening: boolean;
  tabs: Session[];
}

export type KeyAction =
  | { type: 'blur' }
  | { type: 'close-settings' }
  | { type: 'answer'; id: number; key: RequestKey }
  | { type: 'reveal'; id: number }
  | { type: 'open'; sessionId: string }
  | { type: 'prompt'; sessionId: string }
  | { type: 'select'; sessionId: string | null; requestId: number | null }
  | { type: 'back' }
  | { type: 'close-panel' }
  | { type: 'acknowledge' | 'snooze' | 'jump' | 'hide'; sessionId: string };

export interface KeyResult {
  prevent: boolean;
  action: KeyAction | null;
}

const result = (prevent: boolean, action: KeyAction | null = null): KeyResult => ({ prevent, action });

export function keyFocus({ openSession, selectedId, visible, queue, chosenRequestId }: KeyFocusState) {
  const selected = openSession ?? visible.find((session) => session.session_id === selectedId) ?? null;
  return { selected, target: keyTarget(queue, chosenRequestId, selected?.session_id ?? null) };
}

export function decideKey(press: KeyPress, state: KeyState): KeyResult {
  const key = press.key.toLowerCase();
  if (press.repeat && press.focus !== 'text' && !key.startsWith('arrow')) return result(key === 'enter' || key === ' ');
  if (key === 'escape' && press.focus === 'text') return result(false, { type: 'blur' });
  if (state.settingsOpen) return result(false, key === 'escape' ? { type: 'close-settings' } : null);
  if (press.modified || press.focus === 'text') return result(false);
  const { openSession, queue } = state;
  const { selected, target } = keyFocus(state);
  if (target && (key === 'a' || key === 'd' || key === 'm')) {
    if (!press.settled) return result(true);
    if (press.targetView === 'absent') return result(true, { type: 'open', sessionId: target.session_id });
    return result(true, press.targetView === 'shown' ? { type: 'answer', id: target.id, key } : { type: 'reveal', id: target.id });
  }
  switch (key) {
    case 'tab': {
      const tabQueue = openSession ? requestsOf(queue, openSession.session_id) : queue;
      if (tabQueue.length < 2 || (press.focus !== 'body' && !press.inRequest)) return result(false);
      const step = press.shift ? -1 : 1;
      const next = target
        ? tabQueue.find((request) => request.id === stepRequest(tabQueue, target.id, step))
        : step === 1
          ? tabQueue[0]
          : tabQueue.at(-1);
      return result(true, next ? { type: 'select', sessionId: openSession ? null : next.session_id, requestId: next.id } : null);
    }
    case 'arrowdown':
    case 'arrowup': {
      if (openSession) return result(false);
      const step = key === 'arrowdown' ? 1 : -1;
      const index = state.visible.findIndex((session) => session.session_id === state.selectedId);
      const next = index === -1 ? (step === 1 ? state.visible[0] : state.visible.at(-1)) : state.visible[index + step];
      return result(true, next ? { type: 'select', sessionId: next.session_id, requestId: firstRequestOf(queue, next.session_id) } : null);
    }
    case 'enter':
      return result(false, !openSession && selected && press.focus !== 'button' ? { type: 'open', sessionId: selected.session_id } : null);
    case 'arrowleft':
    case 'backspace':
      return openSession && press.focus !== 'choice' ? result(true, { type: 'back' }) : result(false);
    case '[':
    case ']': {
      const next = openSession && stepTab(state.tabs, openSession.session_id, key === ']' ? 1 : -1);
      return result(false, next ? { type: 'open', sessionId: next } : null);
    }
    case 'a':
      return result(false, selected && canAcknowledge(selected) ? { type: 'acknowledge', sessionId: selected.session_id } : null);
    case 's':
      return result(false, selected && canSnooze(selected) ? { type: 'snooze', sessionId: selected.session_id } : null);
    case 'j': {
      const jumpTarget = selected ? (selected.state !== 'ended' ? selected.session_id : null) : target?.session_id;
      return result(false, jumpTarget ? { type: 'jump', sessionId: jumpTarget } : null);
    }
    case 'x':
      return result(
        false,
        selected && requestsOf(queue, selected.session_id).length === 0 ? { type: 'hide', sessionId: selected.session_id } : null,
      );
    case '/':
      return selected && promptPath(selected) ? result(true, { type: 'prompt', sessionId: selected.session_id }) : result(false);
    case 'escape':
      return result(false, openSession || state.opening ? { type: 'back' } : { type: 'close-panel' });
  }
  return result(false);
}

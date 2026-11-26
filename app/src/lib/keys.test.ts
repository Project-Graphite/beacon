import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { PendingDecision, Session, SessionState } from '../beacon';
import { decideKey, keyFocus, type KeyPress, type KeyState } from './keys.ts';
import { requestQueue } from './queue.ts';
import { withDefaults } from './sessions.ts';

const session = (session_id: string, state: SessionState = 'idle', fields: Partial<Session> = {}): Session =>
  withDefaults({
    session_id,
    state,
    overlays: { compacting: false, subagents: 0, background_tasks: 0 },
    last_event_ms: 0,
    event_count: 0,
    tasks_created: 0,
    tasks_completed: 0,
    ...fields,
  });

const request = (id: number, session_id: string): PendingDecision => ({
  id,
  session_id,
  agent_id: null,
  tool_name: 'Bash',
  tool_input: {},
  permission_suggestions: null,
  kind: 'tool',
  created_ms: id,
  tool_use_id: null,
  preview: '',
  cwd: null,
  permission_mode: null,
  agent_type: null,
  expires_ms: id + 3_600_000,
  plan: null,
});

const asking = session('a', 'needs_permission', { alerting: true, attention: true });
const done = session('b', 'done');
const idle = session('c', 'idle', { can_wake: true });
const ended = session('d', 'ended');

const state = (fields: Partial<KeyState> = {}): KeyState => ({
  openSession: null,
  selectedId: null,
  visible: [asking, done, idle],
  queue: requestQueue([request(1, 'a'), request(2, 'a'), request(3, 'b')]),
  chosenRequestId: null,
  settingsOpen: false,
  opening: false,
  tabs: [],
  ...fields,
});

const press = (key: string, fields: Partial<KeyPress> = {}): KeyPress => ({
  key,
  shift: false,
  modified: false,
  repeat: false,
  focus: 'body',
  inRequest: false,
  settled: true,
  targetView: 'shown',
  ...fields,
});

test('a held key does nothing, and a held Enter or Space is swallowed', () => {
  assert.deepEqual(decideKey(press('a', { repeat: true }), state()), { prevent: false, action: null });
  assert.deepEqual(decideKey(press('Enter', { repeat: true }), state({ selectedId: 'a' })), { prevent: true, action: null });
  assert.deepEqual(decideKey(press(' ', { repeat: true }), state()), { prevent: true, action: null });
  assert.deepEqual(decideKey(press('ArrowDown', { repeat: true }), state()).action, { type: 'select', sessionId: 'a', requestId: 1 });
});

test('keys typed into a text field stay there, and Escape leaves the field', () => {
  for (const key of ['a', 'd', 'm', 's', 'j', 'x', '/', 'Tab', 'Enter', 'ArrowDown', 'Backspace']) {
    assert.deepEqual(decideKey(press(key, { focus: 'text' }), state({ selectedId: 'a' })), { prevent: false, action: null }, key);
  }
  assert.deepEqual(decideKey(press('Escape', { focus: 'text' }), state()).action, { type: 'blur' });
  assert.deepEqual(decideKey(press('a', { focus: 'text', repeat: true }), state({ selectedId: 'a' })).action, null);
});

test('modified keys are left to the system', () => {
  for (const key of ['a', 'Tab', 'Escape', 'j']) {
    assert.deepEqual(decideKey(press(key, { modified: true }), state({ selectedId: 'a' })), { prevent: false, action: null }, key);
  }
});

test('in the settings only Escape does something, and it closes them', () => {
  assert.deepEqual(decideKey(press('Escape'), state({ settingsOpen: true })).action, { type: 'close-settings' });
  for (const key of ['a', 'Tab', 'ArrowDown', 'j']) {
    assert.equal(decideKey(press(key), state({ settingsOpen: true, selectedId: 'a' })).action, null, key);
  }
});

test('the key target is the chosen request of the selected or open session, else the oldest', () => {
  assert.equal(keyFocus(state()).target?.id, 1);
  assert.equal(keyFocus(state({ chosenRequestId: 3 })).target?.id, 3);
  assert.equal(keyFocus(state({ selectedId: 'b' })).target?.id, 3);
  assert.equal(keyFocus(state({ selectedId: 'a', chosenRequestId: 3 })).target?.id, 1);
  assert.equal(keyFocus(state({ selectedId: 'a', chosenRequestId: 2 })).target?.id, 2);
  assert.equal(keyFocus(state({ selectedId: 'c' })).target, null);
  assert.equal(keyFocus(state({ openSession: done, selectedId: 'a' })).target?.id, 3);
  assert.equal(keyFocus(state({ selectedId: 'gone' })).target?.id, 1);
});

test('A, D and M act on the key target', () => {
  for (const key of ['a', 'd', 'm']) {
    assert.deepEqual(decideKey(press(key), state({ selectedId: 'b' })), {
      prevent: true,
      action: { type: 'answer', id: 3, key },
    });
  }
  assert.deepEqual(decideKey(press('m'), state({ openSession: asking, chosenRequestId: 2 })).action, {
    type: 'answer',
    id: 2,
    key: 'm',
  });
});

test('A, D and M on a request scrolled out of view only bring it into view', () => {
  for (const key of ['a', 'd', 'm']) {
    assert.deepEqual(decideKey(press(key, { targetView: 'hidden' }), state({ selectedId: 'b' })), {
      prevent: true,
      action: { type: 'reveal', id: 3 },
    });
  }
});

test('A, D and M on a request that is not on screen open its session', () => {
  assert.deepEqual(decideKey(press('a', { targetView: 'absent' }), state({ selectedId: 'b' })), {
    prevent: true,
    action: { type: 'open', sessionId: 'b' },
  });
});

test('A, D and M are ignored until the key target has settled', () => {
  for (const targetView of ['shown', 'hidden', 'absent'] as const) {
    for (const key of ['a', 'd', 'm']) {
      assert.deepEqual(decideKey(press(key, { settled: false, targetView }), state({ selectedId: 'b' })), { prevent: true, action: null });
    }
  }
  assert.deepEqual(decideKey(press('Tab', { settled: false }), state()).action, { type: 'select', sessionId: 'a', requestId: 2 });
  assert.deepEqual(decideKey(press('a', { settled: false }), state({ selectedId: 'b', queue: [] })).action, {
    type: 'acknowledge',
    sessionId: 'b',
  });
});

test('Caps Lock and Shift do not change what a letter does', () => {
  assert.deepEqual(decideKey(press('A'), state({ selectedId: 'b' })).action, { type: 'answer', id: 3, key: 'a' });
  const quiet = state({ selectedId: 'b', queue: [] });
  assert.deepEqual(decideKey(press('A'), quiet).action, { type: 'acknowledge', sessionId: 'b' });
  assert.deepEqual(decideKey(press('J'), quiet).action, { type: 'jump', sessionId: 'b' });
  assert.deepEqual(decideKey(press('X'), quiet).action, { type: 'hide', sessionId: 'b' });
  assert.deepEqual(decideKey(press('S'), state({ selectedId: 'a', queue: [] })).action, { type: 'snooze', sessionId: 'a' });
  assert.deepEqual(decideKey(press('TAB'), state()).action, { type: 'select', sessionId: 'a', requestId: 2 });
});

test('Tab moves the key target through the requests, from the one the keys act on', () => {
  assert.deepEqual(decideKey(press('Tab'), state({ selectedId: 'b' })), {
    prevent: true,
    action: { type: 'select', sessionId: 'a', requestId: 1 },
  });
  assert.deepEqual(decideKey(press('Tab', { shift: true }), state({ selectedId: 'b' })).action, {
    type: 'select',
    sessionId: 'a',
    requestId: 2,
  });
  assert.deepEqual(decideKey(press('Tab'), state({ selectedId: 'a', chosenRequestId: 3 })).action, {
    type: 'select',
    sessionId: 'a',
    requestId: 2,
  });
  assert.deepEqual(decideKey(press('Tab'), state({ selectedId: 'c' })).action, { type: 'select', sessionId: 'a', requestId: 1 });
  assert.deepEqual(decideKey(press('Tab', { shift: true }), state({ selectedId: 'c' })).action, {
    type: 'select',
    sessionId: 'b',
    requestId: 3,
  });
  assert.deepEqual(decideKey(press('Tab'), state({ openSession: asking, chosenRequestId: 1 })).action, {
    type: 'select',
    sessionId: null,
    requestId: 2,
  });
});

test('Tab keeps its native behaviour unless two or more requests wait and focus is on the page or in a request', () => {
  const native = { prevent: false, action: null };
  assert.deepEqual(decideKey(press('Tab'), state({ queue: [request(1, 'a')] })), native);
  assert.deepEqual(decideKey(press('Tab'), state({ queue: [] })), native);
  assert.deepEqual(decideKey(press('Tab'), state({ openSession: done })), native);
  assert.deepEqual(decideKey(press('Tab', { focus: 'button' }), state()), native);
  assert.deepEqual(decideKey(press('Tab', { focus: 'other' }), state()), native);
  assert.deepEqual(decideKey(press('Tab', { focus: 'choice' }), state()), native);
  assert.equal(decideKey(press('Tab', { focus: 'button', inRequest: true }), state()).prevent, true);
  assert.equal(decideKey(press('Tab', { focus: 'choice', inRequest: true }), state()).prevent, true);
  assert.equal(decideKey(press('Tab'), state()).prevent, true);
});

test('the arrows select the next or previous session in the list and its first request', () => {
  assert.deepEqual(decideKey(press('ArrowDown'), state()), { prevent: true, action: { type: 'select', sessionId: 'a', requestId: 1 } });
  assert.deepEqual(decideKey(press('ArrowUp'), state()).action, { type: 'select', sessionId: 'c', requestId: null });
  assert.deepEqual(decideKey(press('ArrowDown'), state({ selectedId: 'a' })).action, { type: 'select', sessionId: 'b', requestId: 3 });
  assert.deepEqual(decideKey(press('ArrowDown'), state({ selectedId: 'c' })), { prevent: true, action: null });
  assert.deepEqual(decideKey(press('ArrowDown'), state({ openSession: asking })), { prevent: false, action: null });
});

test('Enter opens the selected session unless a button has focus', () => {
  assert.deepEqual(decideKey(press('Enter'), state({ selectedId: 'b' })), { prevent: false, action: { type: 'open', sessionId: 'b' } });
  assert.equal(decideKey(press('Enter', { focus: 'button' }), state({ selectedId: 'b' })).action, null);
  assert.equal(decideKey(press('Enter'), state()).action, null);
  assert.equal(decideKey(press('Enter'), state({ openSession: done })).action, null);
});

test('a, s, j, x and / act on the selected session in the list', () => {
  const quiet = state({ queue: [] });
  assert.deepEqual(decideKey(press('a'), { ...quiet, selectedId: 'b' }).action, { type: 'acknowledge', sessionId: 'b' });
  assert.equal(decideKey(press('a'), { ...quiet, selectedId: 'c' }).action, null);
  assert.deepEqual(decideKey(press('s'), { ...quiet, selectedId: 'a' }).action, { type: 'snooze', sessionId: 'a' });
  assert.equal(decideKey(press('s'), { ...quiet, selectedId: 'b' }).action, null);
  assert.deepEqual(decideKey(press('j'), { ...quiet, selectedId: 'c' }).action, { type: 'jump', sessionId: 'c' });
  assert.deepEqual(decideKey(press('x'), { ...quiet, selectedId: 'c' }).action, { type: 'hide', sessionId: 'c' });
  assert.equal(decideKey(press('x'), state({ selectedId: 'b' })).action, null);
  assert.deepEqual(decideKey(press('/'), { ...quiet, selectedId: 'c' }), { prevent: true, action: { type: 'prompt', sessionId: 'c' } });
});

test('j jumps to the session of the key target when nothing is selected, and never to an ended one', () => {
  assert.deepEqual(decideKey(press('j'), state({ chosenRequestId: 3 })).action, { type: 'jump', sessionId: 'b' });
  assert.equal(decideKey(press('j'), state({ queue: [] })).action, null);
  assert.equal(decideKey(press('j'), state({ visible: [ended], selectedId: 'd', queue: [] })).action, null);
});

test('x does not hide a session with a request waiting', () => {
  assert.equal(decideKey(press('x'), state({ selectedId: 'a', queue: [request(1, 'a')] })).action, null);
  assert.equal(decideKey(press('x'), state({ openSession: asking, queue: [request(5, 'a')] })).action, null);
  assert.deepEqual(decideKey(press('x'), state({ openSession: asking, queue: [request(5, 'b')] })).action, { type: 'hide', sessionId: 'a' });
});

test('in a session, Escape, the left arrow and Backspace go back, and [ and ] switch sessions', () => {
  const inside = state({ openSession: done, tabs: [asking, done, idle], queue: [] });
  assert.deepEqual(decideKey(press('Escape'), inside), { prevent: false, action: { type: 'back' } });
  assert.deepEqual(decideKey(press('ArrowLeft'), inside), { prevent: true, action: { type: 'back' } });
  assert.deepEqual(decideKey(press('Backspace'), inside), { prevent: true, action: { type: 'back' } });
  assert.deepEqual(decideKey(press('ArrowLeft', { focus: 'choice' }), inside), { prevent: false, action: null });
  assert.deepEqual(decideKey(press(']'), inside).action, { type: 'open', sessionId: 'c' });
  assert.deepEqual(decideKey(press('['), inside).action, { type: 'open', sessionId: 'a' });
  assert.deepEqual(decideKey(press('j'), inside).action, { type: 'jump', sessionId: 'b' });
  assert.deepEqual(decideKey(press('x'), inside).action, { type: 'hide', sessionId: 'b' });
  assert.deepEqual(decideKey(press('/'), inside).action, { type: 'prompt', sessionId: 'b' });
});

test('Escape in the list closes the panel, or cancels a session that is still opening', () => {
  assert.deepEqual(decideKey(press('Escape'), state()).action, { type: 'close-panel' });
  assert.deepEqual(decideKey(press('Escape'), state({ opening: true })).action, { type: 'back' });
  assert.equal(decideKey(press('ArrowLeft'), state()).action, null);
  assert.equal(decideKey(press(']'), state()).action, null);
});

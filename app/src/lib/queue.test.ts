import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { PendingDecision, ResolvedDecision } from '../beacon';
import { currentRequest, firstRequestOf, keyTarget, requestQueue, resolutionNotices, stepRequest } from './queue.ts';

const request = (id: number, created_ms: number, session_id = 's'): PendingDecision => ({
  id,
  session_id,
  agent_id: null,
  tool_name: 'Bash',
  tool_input: {},
  permission_suggestions: null,
  kind: 'tool',
  created_ms,
  tool_use_id: null,
  preview: '',
  cwd: null,
  permission_mode: null,
  agent_type: null,
  expires_ms: created_ms + 3_600_000,
  plan: null,
});

const ids = (queue: PendingDecision[]) => queue.map((each) => each.id);

test('the queue is in arrival order, then by id', () => {
  const queue = requestQueue([request(4, 300), request(2, 100), request(3, 100), request(1, 200)]);
  assert.deepEqual(ids(queue), [2, 3, 1, 4]);
});

test('the oldest request is current until another is chosen and still pending', () => {
  const queue = requestQueue([request(1, 100), request(2, 200), request(3, 300)]);
  assert.equal(currentRequest(queue, null)?.id, 1);
  assert.equal(currentRequest(queue, 2)?.id, 2);
  assert.equal(currentRequest(queue, 99)?.id, 1);
  assert.equal(currentRequest([], 2), null);
});

test('moving the current request wraps around both ways', () => {
  const queue = requestQueue([request(1, 100), request(2, 200), request(3, 300)]);
  assert.equal(stepRequest(queue, null, 1), 2);
  assert.equal(stepRequest(queue, 2, 1), 3);
  assert.equal(stepRequest(queue, 3, 1), 1);
  assert.equal(stepRequest(queue, 1, -1), 3);
  assert.equal(stepRequest(queue, 99, -1), 3);
  assert.equal(stepRequest([], null, 1), null);
});

test('selecting a card picks its first request', () => {
  const queue = requestQueue([request(1, 100, 'a'), request(2, 200, 'b'), request(3, 50, 'b')]);
  assert.equal(firstRequestOf(queue, 'b'), 3);
  assert.equal(firstRequestOf(queue, 'a'), 1);
  assert.equal(firstRequestOf(queue, 'c'), null);
});

test('keys target only the requests of the open or selected session', () => {
  const queue = requestQueue([request(1, 100, 'a'), request(2, 200, 'b'), request(3, 300, 'b')]);
  assert.equal(keyTarget(queue, null, null)?.id, 1);
  assert.equal(keyTarget(queue, null, 'b')?.id, 2);
  assert.equal(keyTarget(queue, 3, 'b')?.id, 3);
  assert.equal(keyTarget(queue, 1, 'b')?.id, 2);
  assert.equal(keyTarget(queue, null, 'c'), null);
});

test('requests answered elsewhere leave a notice, Beacon decisions do not', () => {
  const resolved: ResolvedDecision[] = [
    { id: 1, session_id: 'a', tool_name: 'Bash', resolution: 'allowed_in_session', resolved_ms: 1 },
    { id: 2, session_id: 'b', tool_name: 'Bash', resolution: 'denied_in_session', resolved_ms: 1 },
    { id: 3, session_id: 'c', tool_name: 'Bash', resolution: 'expired', resolved_ms: 1 },
    { id: 4, session_id: 'd', tool_name: 'Bash', resolution: 'decided', resolved_ms: 1 },
  ];
  assert.deepEqual(resolutionNotices([1, 2, 3, 4, 5, 6], [request(6, 1)], resolved), [
    { id: 1, sessionId: 'a', text: 'Allowed in the session' },
    { id: 2, sessionId: 'b', text: 'Denied in the session' },
    { id: 3, sessionId: 'c', text: 'Request expired: answer in the session window' },
  ]);
  assert.deepEqual(resolutionNotices([], [], resolved), []);
});

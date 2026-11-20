import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { Session } from '../beacon';
import {
  canAcknowledge,
  canSnooze,
  groupSessions,
  metaItems,
  needsInput,
  statePhrase,
  summary,
  surface,
  turnRunning,
  turnTime,
} from './sessions.ts';

const session = (fields: Partial<Session>): Session => ({
  session_id: 's',
  state: 'idle',
  attention: false,
  overlays: { compacting: false, subagents: 0, background_tasks: 0 },
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
  last_event_ms: 0,
  event_count: 0,
  last_message_preview: null,
  last_error: null,
  tasks_created: 0,
  tasks_completed: 0,
  limited: false,
  snoozed_until_ms: null,
  alerting: false,
  editor: null,
  usage: null,
  mcp_request: null,
  prompts: [],
  ...fields,
});

const ids = (sessions: Session[]) => sessions.map((each) => each.session_id);

test('sessions group as needs you, snoozed, working, idle, newest first, with ended apart', () => {
  const { groups, ended } = groupSessions([
    session({ session_id: 'idle', state: 'idle', last_event_ms: 9 }),
    session({ session_id: 'old-ask', state: 'needs_permission', attention: true, alerting: true, last_event_ms: 1 }),
    session({ session_id: 'done', state: 'done', attention: true, alerting: true, last_event_ms: 5 }),
    session({ session_id: 'snoozed', state: 'needs_answer', attention: true, last_event_ms: 3 }),
    session({ session_id: 'start', state: 'starting', last_event_ms: 8 }),
    session({ session_id: 'work', state: 'working', last_event_ms: 4 }),
    session({ session_id: 'gone', state: 'ended', last_event_ms: 2 }),
    session({ session_id: 'gone-later', state: 'ended', last_event_ms: 7 }),
  ]);
  assert.deepEqual(
    groups.map((group) => [group.label, ids(group.sessions)]),
    [
      ['Needs you', ['done', 'old-ask']],
      ['Snoozed', ['snoozed']],
      ['Working', ['start', 'work']],
      ['Idle', ['idle']],
    ],
  );
  assert.deepEqual(ids(ended), ['gone-later', 'gone']);
  assert.deepEqual(groupSessions([session({ state: 'working' })]).groups.map((group) => group.label), ['Working']);
  assert.deepEqual(groupSessions([]), { groups: [], ended: [] });
});

test('the summary counts sessions needing you and working, or says all quiet', () => {
  const alerting = session({ state: 'done', attention: true, alerting: true });
  const working = session({ state: 'working' });
  assert.equal(summary([alerting, alerting, working, working, working]), '2 need you · 3 working');
  assert.equal(summary([alerting, session({})]), '1 needs you');
  assert.equal(summary([working]), '1 working');
  assert.equal(summary([session({}), session({ state: 'needs_answer', attention: true })]), 'All quiet');
  assert.equal(summary([]), 'All quiet');
});

test('surfaces map entrypoints to labels and icons', () => {
  assert.deepEqual(surface(session({ entrypoint: 'cli' })), { icon: 'terminal', label: 'terminal' });
  assert.deepEqual(surface(session({ entrypoint: 'sdk-cli' })), { icon: 'headless', label: 'headless' });
  assert.deepEqual(surface(session({ entrypoint: 'claude-vscode' })), { icon: 'editor', label: 'VS Code' });
  assert.deepEqual(surface(session({ entrypoint: 'claude-vscode', editor: 'Cursor' })), {
    icon: 'editor',
    label: 'Cursor',
  });
  assert.deepEqual(surface(session({ entrypoint: 'claude-desktop' })), { icon: 'desktop', label: 'desktop' });
  assert.deepEqual(surface(session({ entrypoint: 'mystery' })), { icon: 'unknown', label: 'mystery' });
  assert.deepEqual(surface(session({})), { icon: 'unknown', label: 'unknown' });
});

test('state phrases carry their tone and the error name', () => {
  const phrase = (fields: Partial<Session>) => statePhrase(session(fields));
  assert.deepEqual(phrase({ state: 'needs_permission' }), { text: 'needs permission', tone: 'wip' });
  assert.deepEqual(phrase({ state: 'needs_answer' }), { text: 'has a question', tone: 'wip' });
  assert.deepEqual(phrase({ state: 'needs_plan_approval' }), { text: 'wants plan approval', tone: 'wip' });
  assert.deepEqual(phrase({ state: 'needs_mcp_input' }), { text: 'needs input', tone: 'wip' });
  assert.deepEqual(phrase({ state: 'stalled' }), { text: 'may be stalled', tone: 'wip' });
  assert.deepEqual(phrase({ state: 'done' }), { text: 'finished', tone: 'live' });
  assert.deepEqual(phrase({ state: 'error', last_error: 'rate_limit' }), { text: 'API error: rate_limit', tone: 'danger' });
  assert.deepEqual(phrase({ state: 'error' }), { text: 'API error', tone: 'danger' });
  for (const state of ['working', 'starting', 'idle', 'ended'] as const) {
    assert.deepEqual(phrase({ state }), { text: state, tone: 'muted' });
  }
});

test('done, error and stalled are acknowledged; only alerting needs states are snoozed', () => {
  const states = [
    'starting',
    'working',
    'needs_permission',
    'needs_answer',
    'needs_plan_approval',
    'needs_mcp_input',
    'done',
    'error',
    'idle',
    'stalled',
    'ended',
  ] as const;
  const where = (check: (each: Session) => boolean, alerting: boolean) =>
    states.filter((state) => check(session({ state, attention: alerting, alerting })));
  assert.deepEqual(where(canAcknowledge, true), ['done', 'error', 'stalled']);
  assert.deepEqual(where(canAcknowledge, false), ['done', 'error', 'stalled']);
  assert.deepEqual(where(needsInput, false), ['needs_permission', 'needs_answer', 'needs_plan_approval', 'needs_mcp_input']);
  assert.deepEqual(where(canSnooze, true), ['needs_permission', 'needs_answer', 'needs_plan_approval', 'needs_mcp_input']);
  assert.deepEqual(where(canSnooze, false), []);
});

test('turn times run live, freeze when the turn ends and are absent without a turn', () => {
  const now = 1_000_000;
  const running = session({ turn_started_ms: now - 252_000 });
  assert.equal(turnTime(running, now), '4m12s');
  assert.equal(turnRunning(running), true);
  const finished = session({ turn_started_ms: now - 600_000, turn_ended_ms: now - 588_000 });
  assert.equal(turnTime(finished, now), '12s');
  assert.equal(turnRunning(finished), false);
  assert.equal(turnTime(session({}), now), null);
  assert.equal(turnRunning(session({})), false);
});

test('meta items list only what is present', () => {
  assert.deepEqual(metaItems(session({})), []);
  assert.deepEqual(
    metaItems(
      session({
        model: 'claude-opus-5-5',
        permission_mode: 'acceptEdits',
        tasks_created: 7,
        tasks_completed: 3,
        overlays: { compacting: true, subagents: 2, background_tasks: 1 },
        limited: true,
        usage: {
          input_tokens: 0,
          output_tokens: 0,
          cache_creation_tokens: 0,
          cache_read_tokens: 0,
          context_tokens: 182_000,
          subagent_tokens: 0,
          reported_cost_usd: null,
          model: null,
          updated_ms: 0,
          status: 'ok',
        },
      }),
    ),
    [
      { text: 'opus-5-5' },
      { text: 'acceptEdits' },
      { text: 'tasks 3/7' },
      { text: 'sub 2' },
      { text: 'bg 1' },
      { text: 'compacting' },
      { text: 'ctx 182k' },
      { text: 'limited', title: 'Known only from claude agents; no hook has fired for this session yet' },
    ],
  );
  assert.deepEqual(metaItems(session({ permission_mode: 'default' })), []);
});

test('a session with queued prompts says how many in its meta line', () => {
  const prompt = (id: number, status: 'queued' | 'delivered') => ({ id, preview: '', created_ms: 0, status, reason: null });
  assert.deepEqual(metaItems(session({ prompts: [prompt(1, 'delivered'), prompt(2, 'queued')] })), [{ text: '1 queued' }]);
});
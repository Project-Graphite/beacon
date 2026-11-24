import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { Prompt, Session, SessionState } from '../beacon';
import { promptPath, promptStatus, queuedPrompts, recentPrompts } from './prompts.ts';

const session = (state: SessionState, entrypoint: string | null = 'cli', prompts: Prompt[] = []) =>
  ({ session_id: 's', state, entrypoint, prompts }) as Session;

const prompt = (id: number, status: Prompt['status'], reason: string | null = null): Prompt => ({
  id,
  preview: `prompt ${id}`,
  created_ms: 1000 + id,
  status,
  reason,
});

test('a running turn queues, an idle VS Code session prefills, any other idle session copies and jumps', () => {
  const paths = (entrypoint: string | null) =>
    (['starting', 'working', 'needs_permission', 'needs_answer', 'needs_plan_approval', 'needs_mcp_input', 'stalled', 'idle', 'done', 'error', 'ended'] as const).map(
      (state) => promptPath(session(state, entrypoint))?.path ?? null,
    );
  const running = Array(7).fill('at_turn_end');
  assert.deepEqual(paths('claude-vscode'), [...running, 'prefill', 'prefill', 'prefill', null]);
  assert.deepEqual(paths('cli'), [...running, 'copy_and_jump', 'copy_and_jump', 'copy_and_jump', null]);
  assert.deepEqual(paths(null), [...running, 'copy_and_jump', 'copy_and_jump', 'copy_and_jump', null]);
  assert.deepEqual(paths('claude-desktop').slice(7), ['copy_and_jump', 'copy_and_jump', 'copy_and_jump', null]);
});

test('an idle, finished or failed session with a stop waiter is woken, whatever its surface', () => {
  const wakeable = (state: SessionState, entrypoint: string | null) =>
    promptPath({ ...session(state, entrypoint), can_wake: true })?.path ?? null;
  for (const entrypoint of ['claude-vscode', 'cli', null]) {
    assert.deepEqual(
      (['working', 'idle', 'done', 'error', 'ended'] as const).map((state) => wakeable(state, entrypoint)),
      ['at_turn_end', 'wake', 'wake', 'wake', null],
    );
  }
});

test('each path states what will happen', () => {
  assert.equal(promptPath(session('working'))?.label, 'Sends when the current turn finishes.');
  assert.equal(
    promptPath({ ...session('idle', 'claude-vscode'), can_wake: true })?.label,
    'Sends now. Claude sees it as a follow-up from Beacon, not as a typed message.',
  );
  assert.equal(
    promptPath(session('idle', 'claude-vscode'))?.label,
    'Opens the session in VS Code with the text in its prompt box and on your clipboard. If VS Code says the session is already open, paste it.',
  );
  assert.equal(promptPath(session('done'))?.label, "Copies the text and jumps to the session's window. Paste it there.");
});

test('recent prompts list newest first without reordering the source', () => {
  const prompts = [prompt(3, 'acknowledged'), prompt(5, 'queued'), prompt(4, 'delivered'), prompt(1, 'failed', 'x')];
  assert.deepEqual(
    recentPrompts(prompts).map((each) => each.id),
    [5, 4, 3, 1],
  );
  assert.deepEqual(
    prompts.map((each) => each.id),
    [3, 5, 4, 1],
  );
});

test('a status reads as its name, with the reason after a failure', () => {
  assert.equal(promptStatus(prompt(1, 'queued')), 'queued');
  assert.equal(promptStatus(prompt(2, 'acknowledged')), 'acknowledged');
  assert.equal(
    promptStatus(prompt(3, 'failed', 'The session ended before the turn finished.')),
    'failed: The session ended before the turn finished.',
  );
});

test('only queued prompts count as queued', () => {
  assert.equal(queuedPrompts(session('working', 'cli', [prompt(1, 'queued'), prompt(2, 'delivered'), prompt(3, 'queued')])), 2);
  assert.equal(queuedPrompts(session('working')), 0);
});

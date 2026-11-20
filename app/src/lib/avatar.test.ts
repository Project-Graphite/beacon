import assert from 'node:assert/strict';
import { test } from 'node:test';
import { AVATAR_HUES, avatarFace, avatarHue, blinkDelay } from './avatar.ts';

test('an avatar hue is stable for a session and spreads across all six hues', () => {
  const id = '4c2c3535-edec-4bb7-90b7-38049912db27';
  assert.equal(avatarHue(id), avatarHue(id));
  assert.ok(AVATAR_HUES.includes(avatarHue(id)));
  const counts = new Map<string, number>();
  for (let index = 0; index < 600; index++) {
    const hue = avatarHue(`session-${index}-${index * 7919}`);
    counts.set(hue, (counts.get(hue) ?? 0) + 1);
  }
  assert.deepEqual([...counts.keys()].sort(), [...AVATAR_HUES].sort());
  for (const count of counts.values()) assert.ok(count > 60, `uneven spread: ${[...counts.values()]}`);
});

test('blink offsets are stable, negative and differ between sessions', () => {
  assert.equal(blinkDelay('a'), blinkDelay('a'));
  const delays = ['a', 'b', 'c', 'd', 'e'].map(blinkDelay);
  assert.ok(delays.every((delay) => delay <= 0 && delay > -4200));
  assert.ok(new Set(delays).size > 1);
});

test('the face follows the state', () => {
  const faces = {
    starting: 'blank',
    working: 'working',
    needs_permission: 'alarmed',
    needs_answer: 'alarmed',
    needs_plan_approval: 'alarmed',
    needs_mcp_input: 'alarmed',
    done: 'happy',
    error: 'crossed',
    idle: 'sleepy',
    stalled: 'sideways',
    ended: 'closed',
  } as const;
  for (const [state, face] of Object.entries(faces)) {
    assert.equal(avatarFace(state as keyof typeof faces), face, state);
  }
});

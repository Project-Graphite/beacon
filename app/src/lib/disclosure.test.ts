import assert from 'node:assert/strict';
import { test } from 'node:test';
import { toggleOpen } from './disclosure.ts';

test('each session keeps its own More disclosure, closed until toggled', () => {
  const opened = toggleOpen([], 'a');
  assert.deepEqual(opened, ['a']);
  assert.equal(opened.includes('b'), false);
  assert.deepEqual(toggleOpen(opened, 'b'), ['a', 'b']);
  assert.deepEqual(toggleOpen(['a', 'b'], 'a'), ['b']);
  const kept: readonly string[] = ['a'];
  toggleOpen(kept, 'a');
  assert.deepEqual(kept, ['a']);
});

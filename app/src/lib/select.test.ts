import assert from 'node:assert/strict';
import { test } from 'node:test';
import { menuOpensUp, selectKey } from './select.ts';

const closed = { open: false, active: 0 };

test('a closed select opens on the arrows, Enter or Space at the selected choice and ignores other keys', () => {
  for (const key of ['ArrowDown', 'ArrowUp', 'Enter', ' ']) {
    assert.deepEqual(selectKey(closed, key, 4, 2), { open: true, active: 2, choose: false }, key);
  }
  assert.deepEqual(selectKey(closed, 'ArrowDown', 4, -1), { open: true, active: 0, choose: false });
  for (const key of ['Escape', 'Tab', 'Home', 'a']) assert.equal(selectKey(closed, key, 4, 2), null, key);
  assert.equal(selectKey(closed, 'ArrowDown', 0, -1), null);
});

test('an open select moves with wrapping arrows, Home and End, chooses with Enter or Space and closes on Escape or Tab', () => {
  const at = (active: number) => ({ open: true, active });
  assert.deepEqual(selectKey(at(1), 'ArrowDown', 4, 1), { open: true, active: 2, choose: false });
  assert.deepEqual(selectKey(at(3), 'ArrowDown', 4, 1), { open: true, active: 0, choose: false });
  assert.deepEqual(selectKey(at(0), 'ArrowUp', 4, 1), { open: true, active: 3, choose: false });
  assert.deepEqual(selectKey(at(2), 'Home', 4, 1), { open: true, active: 0, choose: false });
  assert.deepEqual(selectKey(at(0), 'End', 4, 1), { open: true, active: 3, choose: false });
  assert.deepEqual(selectKey(at(2), 'Enter', 4, 1), { open: true, active: 2, choose: true });
  assert.deepEqual(selectKey(at(2), ' ', 4, 1), { open: true, active: 2, choose: true });
  assert.deepEqual(selectKey(at(2), 'Escape', 4, 1), { open: false, active: 2, choose: false });
  assert.deepEqual(selectKey(at(2), 'Tab', 4, 1), { open: false, active: 2, choose: false });
  assert.equal(selectKey(at(2), 'x', 4, 1), null);
});

test('the menu opens downwards unless it only fits above the trigger', () => {
  assert.equal(menuOpensUp({ top: 40, bottom: 64 }, 120, 400, 4), false);
  assert.equal(menuOpensUp({ top: 340, bottom: 364 }, 120, 400, 4), true);
  assert.equal(menuOpensUp({ top: 60, bottom: 84 }, 120, 150, 4), false);
});

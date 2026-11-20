import assert from 'node:assert/strict';
import { test } from 'node:test';
import { entryOffset, openedFrom } from './panel.ts';

test('an opened event without an edge reads as no edge, and only hover and sticky modes exist', () => {
  assert.deepEqual(openedFrom({ mode: 'hover', focused: false }), { mode: 'hover', focused: false, edge: null });
  assert.deepEqual(openedFrom({ mode: 'sticky', focused: true, edge: 'bottom' }), {
    mode: 'sticky',
    focused: true,
    edge: 'bottom',
  });
  assert.deepEqual(openedFrom({ mode: 'sticky', focused: true, edge: null }).edge, null);
});

test('the panel slides 14 px in from its edge, and from the top without one', () => {
  assert.deepEqual(entryOffset('left'), { x: -14, y: 0 });
  assert.deepEqual(entryOffset('right'), { x: 14, y: 0 });
  assert.deepEqual(entryOffset('bottom'), { x: 0, y: 14 });
  assert.deepEqual(entryOffset('top'), { x: 0, y: -14 });
  assert.deepEqual(entryOffset(null), { x: 0, y: -14 });
});

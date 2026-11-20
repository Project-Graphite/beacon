import assert from 'node:assert/strict';
import { test } from 'node:test';
import { flipDelta, listChanges, rollDirection, staggerDelay } from './motion.ts';

test('the first eight rows stagger 20 ms apart and later rows do not animate', () => {
  assert.deepEqual(
    [0, 1, 2, 7, 8, 30].map(staggerDelay),
    [0, 20, 40, 140, null, null],
  );
});

test('a flip delta inverts the move and ignores sub-pixel noise', () => {
  assert.deepEqual(flipDelta({ x: 0, y: 120 }, { x: 0, y: 40 }), { x: 0, y: 80 });
  assert.deepEqual(flipDelta({ x: 10, y: 0 }, { x: 0, y: 33 }), { x: 10, y: -33 });
  assert.equal(flipDelta({ x: 0, y: 40.2 }, { x: 0, y: 40 }), null);
  assert.equal(flipDelta({ x: 0, y: 40 }, { x: 0, y: 40 }), null);
});

test('list changes sort rows into entered, moved and left in document order', () => {
  const at = (id: string, y: number) => [id, { id, offset: { x: 0, y } }] as const;
  const before = new Map([at('a', 0), at('b', 33), at('c', 66), at('gone', 99)]);
  const after = new Map([at('c', 0), at('a', 33), at('b', 66), at('new', 99), at('newer', 132)]);
  const changes = listChanges(before, after);
  assert.deepEqual(
    changes.entered.map((item) => item.id),
    ['new', 'newer'],
  );
  assert.deepEqual(
    changes.moved.map(({ item, delta }) => [item.id, delta.y]),
    [
      ['c', 66],
      ['a', -33],
      ['b', -33],
    ],
  );
  assert.deepEqual(
    changes.left.map((item) => item.id),
    ['gone'],
  );
  assert.deepEqual(listChanges(after, after), { entered: [], moved: [], left: [] });
});

test('numbers roll up when they grow, down when they shrink, and not at all otherwise', () => {
  assert.equal(rollDirection(1, 2), 'up');
  assert.equal(rollDirection(3, 1), 'down');
  assert.equal(rollDirection(2, 2), null);
  assert.equal(rollDirection(null, 4), null);
});

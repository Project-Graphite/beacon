import assert from 'node:assert/strict';
import { test } from 'node:test';
import { reuseEqual, sameJson, sameProps } from './reuse.ts';

test('equal JSON values compare equal however they were parsed', () => {
  assert.ok(sameJson({ a: [1, { b: null }], c: 'x' }, JSON.parse('{"a":[1,{"b":null}],"c":"x"}')));
  assert.ok(!sameJson({ a: 1 }, { a: 1, b: 2 }));
  assert.ok(!sameJson({ a: [1, 2] }, { a: [1, 3] }));
  assert.ok(!sameJson([], {}));
  assert.ok(!sameJson({ a: null }, { b: null }));
  assert.ok(!sameJson(null, {}));
});

test('unchanged items keep their previous objects and an unchanged list keeps its array', () => {
  const previous = [
    { id: 'a', state: 'working', steps: [1] },
    { id: 'b', state: 'idle', steps: [] },
  ];
  const next = JSON.parse(JSON.stringify(previous));
  assert.equal(reuseEqual(previous, next, (item) => item.id), previous);
  next[0].steps = [1, 2];
  const merged = reuseEqual(previous, next, (item) => item.id);
  assert.notEqual(merged, previous);
  assert.notEqual(merged[0], previous[0]);
  assert.equal(merged[1], previous[1]);
  const reordered = reuseEqual(previous, [next[1], previous[0]], (item) => item.id);
  assert.deepEqual(reordered, [previous[1], previous[0]]);
  assert.notEqual(reordered, previous);
  assert.deepEqual(reuseEqual(previous, [{ id: 'c', state: 'idle', steps: [] }], (item) => item.id), [
    { id: 'c', state: 'idle', steps: [] },
  ]);
});

test('props are the same when every value is identical or an array of identical items', () => {
  const session = { id: 'a' };
  const request = { id: 1 };
  const open = () => undefined;
  assert.ok(sameProps({ session, pending: [request], now: 1, open }, { session, pending: [request], now: 1, open }));
  assert.ok(!sameProps({ session, pending: [request] }, { session, pending: [{ id: 1 }] }));
  assert.ok(!sameProps({ session, selected: false }, { session, selected: true }));
  assert.ok(!sameProps({ session }, { session, extra: 1 }));
  assert.ok(!sameProps({ pending: [] }, { pending: [request] }));
});

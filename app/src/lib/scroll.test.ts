import assert from 'node:assert/strict';
import { test } from 'node:test';
import { thumbFraction } from './scroll.ts';

test('the thumb fills the track while everything fits', () => {
  assert.equal(thumbFraction(400, 400), 1);
  assert.equal(thumbFraction(400, 120), 1);
});

test('the thumb is the visible fraction of the list', () => {
  assert.equal(thumbFraction(400, 800), 0.5);
  assert.equal(thumbFraction(406, 1624), 0.25);
});

test('the thumb never gets shorter than 24 px of its track, which is inset 3 px at each end', () => {
  assert.equal(thumbFraction(406, 40600), 24 / 400);
  assert.equal(thumbFraction(30, 31), 1);
});

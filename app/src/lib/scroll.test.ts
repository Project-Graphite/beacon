import assert from 'node:assert/strict';
import { test } from 'node:test';
import { fullyShown, thumbFraction } from './scroll.ts';

test('an item is shown when it lies inside the view, give or take a pixel, or fills it', () => {
  const view = { top: 100, bottom: 400 };
  assert.equal(fullyShown({ top: 120, bottom: 200 }, view), true);
  assert.equal(fullyShown({ top: 99.5, bottom: 400.6 }, view), true);
  assert.equal(fullyShown({ top: 50, bottom: 600 }, view), true);
  assert.equal(fullyShown({ top: 380, bottom: 460 }, view), false);
  assert.equal(fullyShown({ top: 60, bottom: 140 }, view), false);
  assert.equal(fullyShown({ top: 500, bottom: 560 }, view), false);
});

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

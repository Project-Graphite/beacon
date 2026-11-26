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

test('an item too tall for the view is shown once it reaches within the scroll inset of both edges', () => {
  const view = { top: 100, bottom: 600 };
  assert.equal(fullyShown({ top: 116, bottom: 900 }, view, 28), true);
  assert.equal(fullyShown({ top: -200, bottom: 584 }, view, 28), true);
  assert.equal(fullyShown({ top: 128.6, bottom: 900 }, view, 28), true);
  assert.equal(fullyShown({ top: -200, bottom: 571.5 }, view, 28), true);
  assert.equal(fullyShown({ top: 116, bottom: 900 }, view), false);
  assert.equal(fullyShown({ top: 140, bottom: 900 }, view, 28), false);
  assert.equal(fullyShown({ top: -200, bottom: 560 }, view, 28), false);
});

test('an item that fits the view is not shown while part of it is scrolled away, whatever the inset', () => {
  const view = { top: 100, bottom: 600 };
  assert.equal(fullyShown({ top: 60, bottom: 400 }, view, 28), false);
  assert.equal(fullyShown({ top: 300, bottom: 640 }, view, 28), false);
  assert.equal(fullyShown({ top: 128, bottom: 572 }, view, 28), true);
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

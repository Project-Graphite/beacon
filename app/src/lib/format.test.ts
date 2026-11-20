import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  clockTime,
  formatDuration,
  formatStepDuration,
  formatTokens,
  resetTime,
  shortModel,
} from './format.ts';

test('durations switch units at a minute, an hour and a day', () => {
  assert.equal(formatDuration(0), '0s');
  assert.equal(formatDuration(12_900), '12s');
  assert.equal(formatDuration(252_000), '4m12s');
  assert.equal(formatDuration(65_000), '1m05s');
  assert.equal(formatDuration(7_500_000), '2h05m');
  assert.equal(formatDuration(2 * 86_400_000 + 3_600_000), '2d01h');
  assert.equal(formatDuration(-5), '0s');
});

test('token counts use k and M suffixes', () => {
  assert.equal(formatTokens(0), '0');
  assert.equal(formatTokens(999), '999');
  assert.equal(formatTokens(1000), '1k');
  assert.equal(formatTokens(1200), '1.2k');
  assert.equal(formatTokens(9_960), '10k');
  assert.equal(formatTokens(40_000), '40k');
  assert.equal(formatTokens(182_345), '182k');
  assert.equal(formatTokens(999_600), '1M');
  assert.equal(formatTokens(1_234_567), '1.2M');
  assert.equal(formatTokens(25_400_000), '25M');
});

test('clock times are local and zero padded', () => {
  const afternoon = new Date(2026, 10, 20, 14, 32, 5).getTime();
  assert.equal(clockTime(afternoon), '14:32');
});

test('step durations show tenths under ten seconds, then the turn duration format, and nothing while unknown', () => {
  assert.equal(formatStepDuration(null), '');
  assert.equal(formatStepDuration(0), '0.0s');
  assert.equal(formatStepDuration(437), '0.4s');
  assert.equal(formatStepDuration(9_990), '9.9s');
  assert.equal(formatStepDuration(12_400), '12s');
  assert.equal(formatStepDuration(125_000), '2m05s');
});

test('reset times show the weekday only when not today', () => {
  const now = new Date(2026, 10, 20, 10, 0).getTime();
  assert.equal(resetTime(new Date(2026, 10, 20, 14, 30).toISOString(), now), '14:30');
  assert.equal(resetTime(new Date(2026, 10, 26, 9, 0).toISOString(), now), 'Thu 09:00');
  assert.equal(resetTime('soon', now), 'soon');
});

test('model names drop a leading claude-', () => {
  assert.equal(shortModel('claude-opus-5-5'), 'opus-5-5');
  assert.equal(shortModel('sonnet'), 'sonnet');
});

import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { AccountUsage } from '../beacon';
import { usageLabel, usageTitle } from './account.ts';

const now = new Date(2026, 10, 20, 10, 0).getTime();

const usage: AccountUsage = {
  five_hour_percent: 47.4,
  five_hour_resets_at: new Date(2026, 10, 20, 14, 30).toISOString(),
  seven_day_percent: 11.5,
  seven_day_resets_at: new Date(2026, 10, 26, 9, 0).toISOString(),
  fetched_ms: now - 192_000,
  source: 'cache',
};

test('the label shows both windows rounded and skips a missing one', () => {
  assert.equal(usageLabel(usage), '5h 47% · 7d 12%');
  assert.equal(usageLabel({ ...usage, seven_day_percent: null }), '5h 47%');
});

test('the title says how old the reading is, when windows reset and what refresh costs', () => {
  assert.equal(
    usageTitle(usage, now, null),
    'Read 3m12s ago\n5h window resets 14:30\n7d window resets Thu 09:00\nRefresh (spends a small Haiku request)',
  );
  assert.equal(
    usageTitle({ ...usage, fetched_ms: null, five_hour_resets_at: null, seven_day_resets_at: null }, now, 'no claude'),
    'Reading time unknown\nRefresh failed: no claude\nRefresh (spends a small Haiku request)',
  );
});

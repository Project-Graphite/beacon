import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { Step } from '../beacon';
import { recentSteps, reservedDetailHeight } from './steps.ts';

const steps = (count: number): Step[] =>
  Array.from({ length: count }, (_, index) => ({
    id: index + 1,
    t_ms: index,
    kind: 'read',
    label: `Reading ${index + 1}`,
    status: 'done',
    duration_ms: null,
  }));

test('the most recent six steps show, oldest first, with the rest kept as earlier', () => {
  const ids = (list: Step[]) => list.map((step) => step.id);
  assert.deepEqual(recentSteps([]), { earlier: [], recent: [] });
  assert.deepEqual(ids(recentSteps(steps(5)).recent), [1, 2, 3, 4, 5]);
  assert.deepEqual(recentSteps(steps(6)).earlier, []);
  const split = recentSteps(steps(20));
  assert.deepEqual(ids(split.earlier), [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
  assert.deepEqual(ids(split.recent), [15, 16, 17, 18, 19, 20]);
});

test('the detail reserves the plan and the recent steps before its data arrives', () => {
  assert.equal(reservedDetailHeight({ tasks_created: 0, event_count: 0 }), 0);
  assert.equal(reservedDetailHeight({ tasks_created: 6, event_count: 0 }), 20 + 6 * 19 - 2);
  assert.equal(reservedDetailHeight({ tasks_created: 0, event_count: 3 }), 20 + 3 * 20);
  assert.equal(reservedDetailHeight({ tasks_created: 2, event_count: 40 }), 20 + 2 * 19 - 2 + 12 + 20 + 6 * 20 + 16);
});

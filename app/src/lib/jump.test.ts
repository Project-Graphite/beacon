import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { JumpOutcome } from '../beacon';
import { jumpReport } from './jump.ts';

const outcome = (fields: Partial<JumpOutcome>): JumpOutcome => ({
  result: 'focused',
  kind: { type: 'console' },
  ambiguous: false,
  link_opened: false,
  ...fields,
});

test('a plain focus succeeds silently', () => {
  assert.deepEqual(jumpReport(outcome({})), { notice: null, succeeded: true });
  assert.deepEqual(jumpReport(outcome({ link_opened: true }), true), { notice: null, succeeded: true });
});

test('a flash, a missing window or an ambiguous match is reported and is no success', () => {
  assert.deepEqual(jumpReport(outcome({ result: 'flashed' })), {
    notice: 'Windows blocked the focus change; the taskbar button is flashing.',
    succeeded: false,
  });
  assert.deepEqual(jumpReport(outcome({ result: 'no_window', kind: null })), {
    notice: 'No window found for this session.',
    succeeded: false,
  });
  assert.deepEqual(jumpReport(outcome({ ambiguous: true })), {
    notice: 'Several windows matched; this may be the wrong one.',
    succeeded: false,
  });
  assert.equal(
    jumpReport(outcome({ result: 'flashed', ambiguous: true })).notice,
    'Windows blocked the focus change; the taskbar button is flashing. Several windows matched; this may be the wrong one.',
  );
});

test('a prefill succeeds only when the session link opened', () => {
  assert.deepEqual(jumpReport(outcome({}), true), {
    notice: 'The window is focused but the session did not open with the text; it is still in the box.',
    succeeded: false,
  });
});

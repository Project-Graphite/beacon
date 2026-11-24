import type { JumpOutcome } from '../beacon';

const FLASHED = 'Windows blocked the focus change; the taskbar button is flashing.';
const NO_WINDOW = 'No window found for this session.';
const NOT_PREFILLED = 'The window is focused but the session did not open with the text; it is still in the box.';
const ON_CLIPBOARD = 'The text is on your clipboard. If VS Code says the session is already open, paste it into the session.';
const AMBIGUOUS = 'Several windows matched; this may be the wrong one.';

export interface JumpReport {
  notice: string | null;
  succeeded: boolean;
}

export function jumpReport(outcome: JumpOutcome, prefill = false): JumpReport {
  const notice = [
    outcome.result === 'flashed' && FLASHED,
    outcome.result === 'no_window' && NO_WINDOW,
    prefill && outcome.result === 'focused' && !outcome.link_opened && NOT_PREFILLED,
    outcome.ambiguous && AMBIGUOUS,
  ]
    .filter(Boolean)
    .join(' ');
  return { notice: notice || null, succeeded: notice === '' };
}

export function prefillReport(outcome: JumpOutcome): JumpReport {
  return { notice: [jumpReport(outcome, true).notice, ON_CLIPBOARD].filter(Boolean).join(' '), succeeded: false };
}

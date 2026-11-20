import { acknowledge, snooze, unsnooze, type Session } from '../beacon';
import { clockTime } from '../lib/format.ts';
import { canAcknowledge, canSnooze, needsInput } from '../lib/sessions.ts';
import { OutlineButton } from './OutlineButton';

interface CardActionsProps {
  session: Session;
  globalSnoozedUntil: number | null;
  run: (action: Promise<unknown>) => void;
}

export function CardActions({ session, globalSnoozedUntil, run }: CardActionsProps) {
  const snoozedUntil =
    needsInput(session) && session.attention && !session.alerting
      ? (session.snoozed_until_ms ?? globalSnoozedUntil)
      : null;
  if (!canAcknowledge(session) && !canSnooze(session) && snoozedUntil === null) return null;
  return (
    <div className="mt-1.5 flex items-center justify-end gap-2">
      {canAcknowledge(session) && (
        <OutlineButton onClick={() => run(acknowledge(session.session_id))}>Acknowledge</OutlineButton>
      )}
      {canSnooze(session) &&
        [5, 15].map((minutes) => (
          <OutlineButton
            key={minutes}
            title={`Snooze this session for ${minutes} minutes`}
            onClick={() => run(snooze(session.session_id, minutes))}
          >
            Snooze {minutes}
          </OutlineButton>
        ))}
      {snoozedUntil !== null && (
        <>
          <span className="text-micro text-muted">Snoozed until {clockTime(snoozedUntil)}</span>
          <OutlineButton
            title={session.snoozed_until_ms === null ? 'Clear the snooze for all sessions' : 'Clear the snooze'}
            onClick={() => run(unsnooze(session.snoozed_until_ms === null ? null : session.session_id))}
          >
            Clear
          </OutlineButton>
        </>
      )}
    </div>
  );
}

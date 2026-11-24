import type { Session } from '../beacon';
import { useNow } from '../hooks/useNow.ts';
import { turnRunning, turnTime } from '../lib/sessions.ts';

export function TurnTime({ session }: { session: Session }) {
  const time = turnTime(session, useNow(session.state !== 'ended' && turnRunning(session)));
  return time && <span className="shrink-0 text-micro text-faint tabular-nums">{time}</span>;
}

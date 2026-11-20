import { useState, type MouseEvent } from 'react';
import type { PendingDecision, Session } from '../beacon';
import { plainText } from '../lib/markdown.ts';
import { cardId, metaItems, statePhrase, surface, turnTime, type Tone } from '../lib/sessions.ts';
import { CardActions } from './CardActions';
import { Icon } from './Icon';
import { PendingRequest } from './PendingRequest';
import { SessionDetails } from './SessionDetails';
import { StateMark } from './StateMark';

const TONE_CLASSES: Record<Tone, string> = {
  wip: 'text-wip',
  live: 'text-live',
  danger: 'text-danger',
  muted: 'text-muted',
};

interface SessionCardProps {
  session: Session;
  pending: PendingDecision[];
  now: number;
  selected: boolean;
  expanded: boolean;
  globalSnoozedUntil: number | null;
  onActivate: () => void;
  run: (action: Promise<unknown>) => void;
}

export function SessionCard({
  session,
  pending,
  now,
  selected,
  expanded,
  globalSnoozedUntil,
  onActivate,
  run,
}: SessionCardProps) {
  const [seen, setSeen] = useState(expanded);
  if (expanded && !seen) setSeen(true);
  const { icon, label } = surface(session);
  const phrase = statePhrase(session);
  const name = session.title ?? session.listed_name;
  const time = turnTime(session, now);
  const meta = metaItems(session);
  const activate = (event: MouseEvent) => {
    if (!(event.target instanceof Element && event.target.closest('button'))) onActivate();
  };

  return (
    <li
      id={cardId(session.session_id)}
      aria-current={selected || undefined}
      className={`relative ${selected ? 'before:absolute before:inset-y-0 before:left-0 before:z-10 before:w-0.5 before:bg-ink' : ''}`}
    >
      <div
        onClick={activate}
        className={`grid grid-cols-[16px_minmax(0,1fr)] gap-x-2.5 px-3 py-2.5 transition-colors duration-150 ease-settle hover:bg-surface ${selected ? 'bg-surface' : ''}`}
      >
        <div className="pt-[1.5px]">
          <StateMark state={session.state} snoozed={session.attention && !session.alerting} />
        </div>
        <div className="min-w-0">
          <div className="flex items-baseline gap-2">
            <span className="min-w-0 truncate font-semibold text-ink">
              {session.project ?? session.session_id.slice(0, 8)}
            </span>
            {session.branch && (
              <span className="min-w-0 shrink-[2] truncate font-mono text-micro text-muted">{session.branch}</span>
            )}
            {time && (
              <span className="ml-auto shrink-0 pl-1 font-mono text-micro text-muted tabular-nums">{time}</span>
            )}
          </div>
          <div className="flex min-w-0 items-center gap-1.5 text-secondary text-muted">
            <Icon name={icon} className="shrink-0" />
            <span className="shrink-0">{label}</span>
            <span aria-hidden="true">·</span>
            <span className={`min-w-0 truncate ${TONE_CLASSES[phrase.tone]}`}>{phrase.text}</span>
            {name && (
              <>
                <span aria-hidden="true">·</span>
                <span className="min-w-0 shrink-[2] truncate">{name}</span>
              </>
            )}
          </div>
          {session.activity && (
            <p className="truncate text-secondary text-muted">
              {session.activity.startsWith('Running ') ? (
                <>
                  Running <span className="font-mono">{session.activity.slice('Running '.length)}</span>
                </>
              ) : (
                session.activity
              )}
            </p>
          )}
          {pending.map((each) => (
            <PendingRequest key={each.id} pending={each} now={now} />
          ))}
          {meta.length > 0 && (
            <p className="mt-1 truncate font-mono text-micro text-faint">
              {meta.map((item, index) => (
                <span key={item.text} title={item.title}>
                  {index > 0 && ' · '}
                  {item.text}
                </span>
              ))}
            </p>
          )}
          {session.last_message_preview && (
            <p className="mt-1 line-clamp-2 text-secondary text-muted [overflow-wrap:anywhere]">
              {plainText(session.last_message_preview)}
            </p>
          )}
          <CardActions session={session} globalSnoozedUntil={globalSnoozedUntil} run={run} />
        </div>
      </div>
      {seen && (
        <div
          inert={!expanded}
          className={`grid transition-[grid-template-rows] duration-150 ease-settle starting:grid-rows-[0fr] ${expanded ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]'}`}
        >
          <div className="min-h-0 overflow-hidden">
            <SessionDetails session={session} expanded={expanded} />
          </div>
        </div>
      )}
    </li>
  );
}

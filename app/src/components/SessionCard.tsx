import { useState, type MouseEvent } from 'react';
import { acknowledge, type PendingDecision, type Session } from '../beacon';
import { plainText } from '../lib/markdown.ts';
import {
  canAcknowledge,
  cardId,
  metaItems,
  rowPhrase,
  statePhrase,
  surface,
  turnTime,
  type Tone,
} from '../lib/sessions.ts';
import type { JumpReport } from '../lib/jump.ts';
import type { ResolutionNotice } from '../lib/queue.ts';
import { CardActions } from './CardActions';
import { Collapse } from './Collapse';
import { Icon } from './Icon';
import { RequestCard } from './RequestCard';
import { RequestRow } from './RequestRow';
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
  notices: ResolutionNotice[];
  currentId: number | null;
  queueSize: number;
  keys: Map<number, (key: string) => void>;
  now: number;
  selected: boolean;
  expanded: boolean;
  globalSnoozedUntil: number | null;
  onActivate: () => void;
  onExpand: () => void;
  onJump: () => void;
  onJumpReport: (report: JumpReport) => void;
  run: (action: Promise<unknown>) => void;
}

export function SessionCard({
  session,
  pending,
  notices,
  currentId,
  queueSize,
  keys,
  now,
  selected,
  expanded,
  globalSnoozedUntil,
  onActivate,
  onExpand,
  onJump,
  onJumpReport,
  run,
}: SessionCardProps) {
  const [seen, setSeen] = useState(expanded);
  if (expanded && !seen) setSeen(true);
  const { icon, label } = surface(session);
  const phrase = statePhrase(session);
  const row = rowPhrase(session);
  const name = session.title ?? session.listed_name;
  const time = turnTime(session, now);
  const meta = [...(session.branch ? [{ text: session.branch, title: 'Branch' }] : []), ...metaItems(session)];
  const activate = (event: MouseEvent) => {
    if (!(event.target instanceof Element && event.target.closest('button'))) onActivate();
  };

  return (
    <li
      id={cardId(session.session_id)}
      data-motion={session.session_id}
      aria-current={selected || undefined}
      className={`relative ${selected ? 'before:absolute before:inset-y-0 before:left-0 before:z-10 before:w-0.5 before:bg-ink' : ''}`}
    >
      <div
        onClick={activate}
        className={`relative transition-colors duration-120 ease-settle hover:bg-surface ${selected ? 'bg-surface' : ''}`}
      >
        <span data-flash aria-hidden="true" className="pointer-events-none absolute inset-0 bg-ink/8 opacity-0" />
        <div className="flex h-8 items-center gap-2 px-2.5">
          <StateMark state={session.state} snoozed={session.attention && !session.alerting} />
          <span className="max-w-1/2 shrink-0 truncate font-semibold text-ink">
            {session.project ?? session.session_id.slice(0, 8)}
          </span>
          <span className={`min-w-0 flex-1 truncate text-secondary ${row.kind === 'state' ? TONE_CLASSES[row.tone] : 'text-muted'}`}>
            {row.kind === 'command' ? (
              <>
                Running <span className="font-mono">{row.text}</span>
              </>
            ) : (
              row.text
            )}
          </span>
          <span title={label} className="shrink-0 text-faint">
            <Icon name={icon} className="size-3" />
          </span>
          {canAcknowledge(session) && (
            <button
              type="button"
              aria-label="Acknowledge"
              title="Acknowledge (A)"
              onClick={() => run(acknowledge(session.session_id))}
              className="press -my-1 grid size-[22px] shrink-0 place-items-center rounded-field text-muted hover:bg-line-soft hover:text-ink"
            >
              <Icon name="check" className="size-3.5" />
            </button>
          )}
          {time && <span className="shrink-0 font-mono text-micro text-faint tabular-nums">{time}</span>}
        </div>
        <Collapse open={!expanded}>
          {pending.map((request) => (
            <RequestRow
              key={request.id}
              request={request}
              marked={queueSize > 1 && request.id === currentId}
              currentId={currentId}
              keys={expanded ? null : keys}
              onExpand={onExpand}
            />
          ))}
        </Collapse>
        {notices.map((notice) => (
          <p key={notice.id} className="origin-top-left animate-appear pr-2.5 pb-1.5 pl-8 text-micro text-muted">
            {notice.text}
          </p>
        ))}
      </div>
      {seen && (
        <Collapse open={expanded} appear>
          <div className="pr-2.5 pb-2.5 pl-8">
            <div className="flex min-w-0 items-center gap-1.5 text-secondary text-muted">
              <Icon name={icon} className="size-3 shrink-0" />
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
            {pending.map((request) => (
              <RequestCard
                key={request.id}
                request={request}
                now={now}
                marked={queueSize > 1 && request.id === currentId}
                currentId={currentId}
                keys={expanded ? keys : null}
                onJump={onJump}
              />
            ))}
            {session.state === 'needs_mcp_input' && (
              <div className="mt-2 rounded-field border border-line p-2">
                <p className="text-secondary text-ink">
                  {session.mcp_request?.server || 'An MCP server'} is asking for input
                </p>
                {session.mcp_request?.message && (
                  <p className="mt-0.5 text-secondary text-muted [overflow-wrap:anywhere]">{session.mcp_request.message}</p>
                )}
                <p className="mt-1 text-micro text-muted">Answer in the session window: Beacon does not answer MCP forms.</p>
              </div>
            )}
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
            <CardActions session={session} globalSnoozedUntil={globalSnoozedUntil} onJump={onJump} run={run} />
          </div>
          <SessionDetails session={session} expanded={expanded} onJumpReport={onJumpReport} />
        </Collapse>
      )}
    </li>
  );
}

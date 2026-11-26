import { memo, type MouseEvent } from 'react';
import type { PendingDecision, Session } from '../beacon';
import type { ResolutionNotice } from '../lib/queue.ts';
import { sameProps } from '../lib/reuse.ts';
import { cardId, needsInput, rowLine, sessionTitle, taskProgress, TONE_CLASSES } from '../lib/sessions.ts';
import { Avatar } from './Avatar';
import { ProgressBar } from './ProgressBar';
import { RequestRow } from './RequestRow';
import { SessionActions } from './SessionActions';
import { StepLabel } from './StepLabel';
import { TextButton } from './TextButton';
import { TurnTime } from './TurnTime';

interface SessionRowProps {
  session: Session;
  pending: PendingDecision[];
  notices: ResolutionNotice[];
  currentId: number | null;
  queueSize: number;
  keys: Map<number, (key: string) => void> | null;
  selected: boolean;
  crossedOff: boolean;
  onOpen: (sessionId: string) => Promise<void>;
  onJump: (sessionId: string) => void;
  onCrossOff: (sessionId: string) => void;
  onRestore: (sessionId: string) => void;
}

export const SessionRow = memo(function SessionRow({
  session,
  pending,
  notices,
  currentId,
  queueSize,
  keys,
  selected,
  crossedOff,
  onOpen,
  onJump,
  onCrossOff,
  onRestore,
}: SessionRowProps) {
  const line = rowLine(session);
  const alert = needsInput(session);
  const openThis = () => onOpen(session.session_id);
  const open = (event: MouseEvent) => {
    if (!crossedOff && !(event.target instanceof Element && event.target.closest('button'))) void openThis();
  };

  return (
    <li
      id={cardId(session.session_id)}
      data-motion={session.session_id}
      aria-current={selected || undefined}
      className={`relative ${selected ? 'before:absolute before:inset-y-3 before:left-0 before:z-10 before:w-[3px] before:rounded-r-full before:bg-ink' : ''}`}
    >
      <div
        onClick={open}
        className={`relative transition-colors duration-150 ease-settle ${crossedOff ? 'opacity-45' : alert ? `hover:bg-wip-tint/[0.11] ${selected ? 'bg-wip-tint/[0.11]' : 'bg-wip-tint/[0.07]'}` : 'hover:bg-surface'} ${selected && !alert ? 'bg-surface' : ''}`}
      >
        <span data-flash aria-hidden="true" className="pointer-events-none absolute inset-0 bg-ink/8 opacity-0" />
        <div className="relative flex h-[52px] items-center gap-3 pr-2.5 pl-4">
          <Avatar session={session} size="row" />
          <div className="min-w-0 flex-1">
            <p className="flex items-baseline gap-2">
              <span className="min-w-0 flex-1 truncate font-medium text-ink">{sessionTitle(session)}</span>
              <TurnTime session={session} />
            </p>
            <p className={`truncate text-secondary ${line.kind === 'state' ? TONE_CLASSES[line.tone] : 'text-muted'}`}>
              {line.kind === 'tasks' ? (
                <>
                  <span className="tabular-nums">{line.count}</span> {line.text}
                </>
              ) : line.kind === 'step' ? (
                <StepLabel label={line.text} />
              ) : (
                line.text
              )}
            </p>
          </div>
          {crossedOff ? (
            <TextButton title="Show it in the list again" onClick={() => onRestore(session.session_id)}>
              Show
            </TextButton>
          ) : (
            <SessionActions
              requestPending={pending.length > 0}
              onJump={() => onJump(session.session_id)}
              onCrossOff={() => onCrossOff(session.session_id)}
            />
          )}
          {session.tasks_created > 0 && (
            <ProgressBar fraction={taskProgress(session)} alert={alert} className="absolute inset-x-0 bottom-0 h-0.5" />
          )}
        </div>
        {pending.map((request) => (
          <RequestRow
            key={request.id}
            request={request}
            marked={queueSize > 1 && request.id === currentId}
            currentId={currentId}
            keys={keys}
            onOpen={openThis}
          />
        ))}
        {notices.map((notice) => (
          <p key={notice.id} role="status" className="origin-top-left animate-appear pr-4 pb-2.5 pl-[56px] text-secondary text-muted">
            {notice.text}
          </p>
        ))}
      </div>
    </li>
  );
}, sameProps);

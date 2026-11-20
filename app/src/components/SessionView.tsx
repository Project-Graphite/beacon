import { useEffect, useState } from 'react';
import { getDetail, type PendingDecision, type Session, type SessionDetail } from '../beacon';
import type { JumpReport } from '../lib/jump.ts';
import type { ResolutionNotice } from '../lib/queue.ts';
import {
  metaItems,
  needsInput,
  sessionTitle,
  statePhrase,
  surface,
  taskProgress,
  TONE_CLASSES,
} from '../lib/sessions.ts';
import { ActivityList } from './ActivityList';
import { Avatar } from './Avatar';
import { CardActions } from './CardActions';
import { CommandBlock } from './CommandBlock';
import { IconButton } from './IconButton';
import { PlanList } from './PlanList';
import { ProgressBar } from './ProgressBar';
import { RequestCard } from './RequestCard';
import { SessionActions } from './SessionActions';
import { SessionDetails } from './SessionDetails';
import { SessionTabs } from './SessionTabs';

interface SessionViewProps {
  session: Session;
  tabs: Session[];
  pending: PendingDecision[];
  notices: ResolutionNotice[];
  currentId: number | null;
  queueSize: number;
  keys: Map<number, (key: string) => void> | null;
  now: number;
  globalSnoozedUntil: number | null;
  onBack: () => void;
  onSwitch: (sessionId: string) => void;
  onJump: () => void;
  onCrossOff: () => void;
  onJumpReport: (report: JumpReport) => void;
  run: (action: Promise<unknown>) => void;
}

export function SessionView({
  session,
  tabs,
  pending,
  notices,
  currentId,
  queueSize,
  keys,
  now,
  globalSnoozedUntil,
  onBack,
  onSwitch,
  onJump,
  onCrossOff,
  onJumpReport,
  run,
}: SessionViewProps) {
  const [detail, setDetail] = useState<SessionDetail | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    getDetail(session.session_id).then(
      (loaded) => {
        if (!current) return;
        setDetail(loaded);
        setFailure(null);
      },
      (error) => current && setFailure(String(error)),
    );
    return () => {
      current = false;
    };
  }, [session.session_id, session.event_count]);

  const phrase = statePhrase(session);
  const meta = metaItems(session);
  const facts = [
    session.project && <span key="project">{session.project}</span>,
    session.branch && (
      <span key="branch" title="Branch" className="font-mono">
        {session.branch}
      </span>
    ),
    <span key="surface">{surface(session).label}</span>,
    <span key="state" className={TONE_CLASSES[phrase.tone]}>
      {phrase.text}
    </span>,
  ].filter(Boolean);

  return (
    <div>
      <div className="flex h-8 items-center gap-1 border-b border-line-soft px-1.5">
        <IconButton icon="chevron" iconClassName="rotate-180" title="Back to sessions" onClick={onBack} />
        <SessionTabs tabs={tabs} currentId={session.session_id} onSwitch={onSwitch} />
        <SessionActions requestPending={pending.length > 0} onJump={onJump} onCrossOff={onCrossOff} />
      </div>
      <div className="space-y-3 px-2.5 pt-2.5 pb-3">
        <header className="space-y-2">
          <div className="flex items-center gap-3">
            <Avatar session={session} size="header" />
            <div className="min-w-0 flex-1">
              <h2 className="truncate text-[15px] leading-snug font-semibold text-ink">{sessionTitle(session)}</h2>
              <p className="truncate text-secondary text-muted">
                {facts.map((fact, index) => (
                  <span key={index}>
                    {index > 0 && ' · '}
                    {fact}
                  </span>
                ))}
              </p>
            </div>
          </div>
          {session.tasks_created > 0 && (
            <div>
              <p className="flex items-baseline gap-1.5 text-secondary">
                <span className="shrink-0 font-mono text-muted tabular-nums">
                  {session.tasks_completed}/{session.tasks_created}
                </span>
                <span className="min-w-0 truncate text-ink">{session.task_current}</span>
              </p>
              <ProgressBar
                fraction={taskProgress(session)}
                alert={needsInput(session)}
                className="mt-1 h-[3px] rounded-full"
              />
            </div>
          )}
          {meta.length > 0 && (
            <p className="truncate font-mono text-micro text-faint">
              {meta.map((item, index) => (
                <span key={item.text} title={item.title}>
                  {index > 0 && ' · '}
                  {item.text}
                </span>
              ))}
            </p>
          )}
          <CardActions session={session} globalSnoozedUntil={globalSnoozedUntil} run={run} />
          {notices.map((notice) => (
            <p key={notice.id} className="origin-top-left animate-appear text-micro text-muted">
              {notice.text}
            </p>
          ))}
        </header>
        {pending.map((request) => (
          <RequestCard
            key={request.id}
            request={request}
            now={now}
            marked={queueSize > 1 && request.id === currentId}
            currentId={currentId}
            keys={keys}
            onJump={onJump}
          />
        ))}
        {session.state === 'needs_mcp_input' && (
          <div className="rounded-field border border-line p-2">
            <p className="text-secondary text-ink">{session.mcp_request?.server || 'An MCP server'} is asking for input</p>
            {session.mcp_request?.message && (
              <p className="mt-0.5 text-secondary text-muted [overflow-wrap:anywhere]">{session.mcp_request.message}</p>
            )}
            <p className="mt-1 text-micro text-muted">Answer in the session window: Beacon does not answer MCP forms.</p>
          </div>
        )}
        {session.command_current && (
          <section>
            <h3 className="text-micro text-faint">Terminal</h3>
            <CommandBlock command={session.command_current} live />
          </section>
        )}
        {session.tasks.length > 0 && <PlanList tasks={session.tasks} />}
        {failure && <p className="text-micro text-danger">Cannot load details: {failure}</p>}
        {detail && detail.steps.length > 0 && <ActivityList steps={detail.steps} />}
        <SessionDetails session={session} detail={detail} onJumpReport={onJumpReport} />
      </div>
    </div>
  );
}

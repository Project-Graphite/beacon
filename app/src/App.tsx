import { useCallback, useEffect, useRef, useState } from 'react';
import {
  acknowledge,
  closePanel,
  getSnapshot,
  jump,
  onOpened,
  onSnapshot,
  setPanelHeight,
  snooze,
  type Opened,
  type Session,
  type Snapshot,
} from './beacon';
import { EmptyState } from './components/EmptyState';
import { Header } from './components/Header';
import { SessionCard } from './components/SessionCard';
import { SessionList } from './components/SessionList';
import { jumpReport, type JumpReport } from './lib/jump.ts';
import { promptBoxId, promptPath } from './lib/prompts.ts';
import {
  currentRequest,
  firstRequestOf,
  requestId,
  requestQueue,
  resolutionNotices,
  stepRequest,
  type ResolutionNotice,
} from './lib/queue.ts';
import { canAcknowledge, canSnooze, cardId, groupSessions, turnRunning } from './lib/sessions.ts';

const NOTICE_MS = 4000;

const isTextField = (target: EventTarget | null) =>
  target instanceof HTMLTextAreaElement ||
  target instanceof HTMLSelectElement ||
  (target instanceof HTMLInputElement && target.type !== 'radio' && target.type !== 'checkbox');

export function App() {
  const [snapshot, setSnapshot] = useState<Snapshot | null | undefined>(undefined);
  const [opened, setOpened] = useState<Opened | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [now, setNow] = useState(Date.now);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [endedOpen, setEndedOpen] = useState(false);
  const [listening, setListening] = useState(false);
  const [chosenRequestId, setChosenRequestId] = useState<number | null>(null);
  const [notices, setNotices] = useState<ResolutionNotice[]>([]);
  const panel = useRef<HTMLElement>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const pendingIds = useRef<number[]>([]);
  const requestKeys = useRef(new Map<number, (key: string) => void>());
  const nextJumpNoticeId = useRef(-1);

  const run = useCallback((action: Promise<unknown>) => {
    action.then(
      () => setFailure(null),
      (error) => setFailure(String(error)),
    );
  }, []);

  const showNotices = useCallback((fresh: ResolutionNotice[]) => {
    setNotices((current) => [...current, ...fresh]);
    window.setTimeout(() => setNotices((current) => current.filter((notice) => !fresh.includes(notice))), NOTICE_MS);
  }, []);

  useEffect(() => {
    const receive = (next: Snapshot | null) => {
      if (next) {
        const fresh = resolutionNotices(pendingIds.current, next.pending, next.resolved);
        pendingIds.current = next.pending.map((request) => request.id);
        if (fresh.length > 0) showNotices(fresh);
      }
      setSnapshot(next);
      setNow(Date.now());
    };
    const subscriptions = Promise.all([onSnapshot(receive), onOpened(setOpened)]);
    run(
      subscriptions.then(() => {
        setListening(true);
        return getSnapshot().then(receive);
      }),
    );
    return () => void subscriptions.then((unlisteners) => unlisteners.forEach((unlisten) => unlisten()));
  }, [run, showNotices]);

  useEffect(() => {
    const [section, list, inner] = [panel.current, scroller.current, content.current];
    if (!listening || !section || !list || !inner) return;
    const observer = new ResizeObserver(() =>
      run(setPanelHeight(section.offsetHeight - list.clientHeight + inner.offsetHeight)),
    );
    observer.observe(inner);
    return () => observer.disconnect();
  }, [listening, run]);

  const sessions = snapshot?.sessions ?? [];
  const { groups, ended } = groupSessions(sessions);
  const visible = [...groups.flatMap((group) => group.sessions), ...(endedOpen ? ended : [])];
  const queue = requestQueue(snapshot?.pending ?? []);
  const current = currentRequest(queue, chosenRequestId);
  const ticking = queue.length > 0 || sessions.some((session) => session.state !== 'ended' && turnRunning(session));

  useEffect(() => {
    if (!ticking) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [ticking]);

  const firstVisibleId = visible[0]?.session_id ?? null;
  useEffect(() => {
    if (opened?.focused) setSelectedId((selected) => selected ?? firstVisibleId);
  }, [opened?.focused, firstVisibleId]);

  useEffect(() => {
    if (selectedId) document.getElementById(cardId(selectedId))?.scrollIntoView({ block: 'nearest' });
  }, [selectedId]);

  useEffect(() => {
    if (chosenRequestId !== null) document.getElementById(requestId(chosenRequestId))?.scrollIntoView({ block: 'nearest' });
  }, [chosenRequestId]);

  const toggle = (sessionId: string) => setExpandedId((expanded) => (expanded === sessionId ? null : sessionId));

  const reportJump = (sessionId: string, { notice, succeeded }: JumpReport) => {
    if (notice) showNotices([{ id: nextJumpNoticeId.current--, sessionId, text: notice }]);
    if (succeeded) run(closePanel());
  };

  const jumpTo = (sessionId: string) =>
    run(jump(sessionId).then((outcome) => reportJump(sessionId, jumpReport(outcome))));

  useEffect(() => {
    const index = visible.findIndex((session) => session.session_id === selectedId);
    const selected: Session | undefined = visible[index];
    const handleKey = (event: KeyboardEvent) => {
      if (event.ctrlKey || event.altKey || event.metaKey || isTextField(event.target)) return;
      const key = event.key.toLowerCase();
      if (current && (key === 'a' || key === 'd' || key === 'm')) {
        event.preventDefault();
        requestKeys.current.get(current.id)?.(key);
        return;
      }
      switch (event.key) {
        case 'Tab':
          if (queue.length > 0) {
            event.preventDefault();
            setChosenRequestId(stepRequest(queue, chosenRequestId, event.shiftKey ? -1 : 1));
          }
          break;
        case 'ArrowDown':
        case 'ArrowUp': {
          event.preventDefault();
          const step = event.key === 'ArrowDown' ? 1 : -1;
          const next = index === -1 ? (step === 1 ? visible[0] : visible.at(-1)) : visible[index + step];
          if (next) {
            setSelectedId(next.session_id);
            const firstRequest = firstRequestOf(queue, next.session_id);
            if (firstRequest !== null) setChosenRequestId(firstRequest);
          }
          break;
        }
        case 'Enter':
          if (selected && !(event.target instanceof HTMLButtonElement)) toggle(selected.session_id);
          break;
        case 'a':
          if (selected && canAcknowledge(selected)) run(acknowledge(selected.session_id));
          break;
        case 's':
          if (selected && canSnooze(selected)) run(snooze(selected.session_id, 5));
          break;
        case 'j': {
          const target = current?.session_id ?? (selected && selected.state !== 'ended' ? selected.session_id : null);
          if (target) jumpTo(target);
          break;
        }
        case '/':
          if (selected && promptPath(selected)) {
            event.preventDefault();
            setExpandedId(selected.session_id);
            window.setTimeout(() => document.getElementById(promptBoxId(selected.session_id))?.focus());
          }
          break;
        case 'Escape':
          if (expandedId !== null) setExpandedId(null);
          else run(closePanel());
          break;
      }
    };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  });

  return (
    <section
      ref={panel}
      className="flex h-screen flex-col overflow-hidden rounded-panel border border-line bg-paper font-sans text-body text-ink"
    >
      <Header
        sessions={sessions}
        accountUsage={snapshot?.account_usage ?? null}
        snoozedUntil={snapshot?.snoozed_until_ms ?? null}
        pinned={opened?.mode === 'pinned'}
        queue={current && queue.length > 1 ? { position: queue.indexOf(current) + 1, total: queue.length } : null}
        run={run}
      />
      <div ref={scroller} className="scroll-thin min-h-0 flex-1 overflow-y-auto">
        <div ref={content}>
          {failure && (
            <p title={failure} className="truncate border-b border-line-soft px-3 py-1.5 text-micro text-danger">
              {failure}
            </p>
          )}
          {snapshot === null && (
            <EmptyState title="Not connected to the Beacon core." detail="Sessions appear once it is running." />
          )}
          {snapshot?.sessions.length === 0 && (
            <EmptyState title="No Claude sessions running." detail="Sessions appear here when Claude Code starts." />
          )}
          <SessionList
            groups={groups}
            ended={ended}
            endedOpen={endedOpen}
            onToggleEnded={() => setEndedOpen(!endedOpen)}
            renderCard={(session) => (
              <SessionCard
                key={session.session_id}
                session={session}
                pending={queue.filter((request) => request.session_id === session.session_id)}
                notices={notices.filter((notice) => notice.sessionId === session.session_id)}
                currentId={current?.id ?? null}
                queueSize={queue.length}
                keys={requestKeys.current}
                now={now}
                selected={session.session_id === selectedId}
                expanded={session.session_id === expandedId}
                globalSnoozedUntil={snapshot?.snoozed_until_ms ?? null}
                onActivate={() => {
                  setSelectedId(session.session_id);
                  toggle(session.session_id);
                }}
                onJump={() => jumpTo(session.session_id)}
                onJumpReport={(report) => reportJump(session.session_id, report)}
                run={run}
              />
            )}
          />
        </div>
      </div>
    </section>
  );
}

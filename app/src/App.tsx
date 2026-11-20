import { useCallback, useEffect, useRef, useState } from 'react';
import {
  acknowledge,
  closePanel,
  getSnapshot,
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
import { canAcknowledge, canSnooze, cardId, groupSessions, turnRunning } from './lib/sessions.ts';

export function App() {
  const [snapshot, setSnapshot] = useState<Snapshot | null | undefined>(undefined);
  const [opened, setOpened] = useState<Opened | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [now, setNow] = useState(Date.now);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [endedOpen, setEndedOpen] = useState(false);
  const [listening, setListening] = useState(false);
  const panel = useRef<HTMLElement>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const content = useRef<HTMLDivElement>(null);

  const run = useCallback((action: Promise<unknown>) => {
    action.then(
      () => setFailure(null),
      (error) => setFailure(String(error)),
    );
  }, []);

  useEffect(() => {
    const receive = (next: Snapshot | null) => {
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
  }, [run]);

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
  const ticking = sessions.some((session) => session.state !== 'ended' && turnRunning(session));

  useEffect(() => {
    if (!ticking) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [ticking]);

  const firstVisibleId = visible[0]?.session_id ?? null;
  useEffect(() => {
    if (opened?.focused) setSelectedId((current) => current ?? firstVisibleId);
  }, [opened?.focused, firstVisibleId]);

  useEffect(() => {
    if (selectedId) document.getElementById(cardId(selectedId))?.scrollIntoView({ block: 'nearest' });
  }, [selectedId]);

  const toggle = (sessionId: string) => setExpandedId((current) => (current === sessionId ? null : sessionId));

  useEffect(() => {
    const index = visible.findIndex((session) => session.session_id === selectedId);
    const selected: Session | undefined = visible[index];
    const handleKey = (event: KeyboardEvent) => {
      if (event.ctrlKey || event.altKey || event.metaKey) return;
      switch (event.key) {
        case 'ArrowDown':
        case 'ArrowUp': {
          event.preventDefault();
          const step = event.key === 'ArrowDown' ? 1 : -1;
          const next = index === -1 ? (step === 1 ? visible[0] : visible.at(-1)) : visible[index + step];
          if (next) setSelectedId(next.session_id);
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
                pending={snapshot?.pending.filter((each) => each.session_id === session.session_id) ?? []}
                now={now}
                selected={session.session_id === selectedId}
                expanded={session.session_id === expandedId}
                globalSnoozedUntil={snapshot?.snoozed_until_ms ?? null}
                onActivate={() => {
                  setSelectedId(session.session_id);
                  toggle(session.session_id);
                }}
                run={run}
              />
            )}
          />
        </div>
      </div>
    </section>
  );
}

import { useCallback, useEffect, useRef, useState, type CSSProperties } from 'react';
import {
  acknowledge,
  closePanel,
  dismiss,
  getSnapshot,
  getUiSettings,
  jump,
  onClosing,
  onOpened,
  onSnapshot,
  setUiSettings,
  snooze,
  type Opened,
  type Session,
  type Snapshot,
  type UiSettings,
} from './beacon';
import { EmptyState } from './components/EmptyState';
import { Header } from './components/Header';
import { SessionList } from './components/SessionList';
import { SessionRow } from './components/SessionRow';
import { SessionView } from './components/SessionView';
import { SettingsView } from './components/SettingsView';
import { ViewStack } from './components/ViewStack';
import { useListMotion } from './hooks/useListMotion.ts';
import { PanelFit, usePanelHeight } from './hooks/usePanelHeight.ts';
import { jumpReport, type JumpReport } from './lib/jump.ts';
import { entryOffset } from './lib/panel.ts';
import { promptBoxId, promptPath } from './lib/prompts.ts';
import { thumbFraction } from './lib/scroll.ts';
import {
  currentRequest,
  firstRequestOf,
  requestId,
  requestQueue,
  resolutionNotices,
  stepRequest,
  type ResolutionNotice,
} from './lib/queue.ts';
import {
  canAcknowledge,
  canSnooze,
  cardId,
  crossedOff,
  groupSessions,
  sessionTabs,
  stepTab,
  turnRunning,
} from './lib/sessions.ts';
import { flashRow } from './motion.ts';

const NOTICE_MS = 4000;
const SCROLL_IDLE_MS = 800;
const DETAIL_PREFIX = 'detail:';

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
  const [openId, setOpenId] = useState<string | null>(null);
  const [endedOpen, setEndedOpen] = useState(false);
  const [crossedOpen, setCrossedOpen] = useState(false);
  const [listening, setListening] = useState(false);
  const [chosenRequestId, setChosenRequestId] = useState<number | null>(null);
  const [notices, setNotices] = useState<ResolutionNotice[]>([]);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [ui, setUi] = useState<UiSettings | null>(null);
  const [closing, setClosing] = useState(false);
  const [entrance, setEntrance] = useState(0);
  const panel = useRef<HTMLElement>(null);
  const scrollArea = useRef<HTMLDivElement>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const ghosts = useRef<HTMLDivElement>(null);
  const shown = useRef(false);
  const pendingIds = useRef<number[]>([]);
  const requestKeys = useRef(new Map<number, (key: string) => void>());
  const nextJumpNoticeId = useRef(-1);

  const run = useCallback((action: Promise<unknown>) => {
    action.then(
      () => setFailure(null),
      (error) => setFailure(String(error)),
    );
  }, []);

  const fit = usePanelHeight(panel, scroller, content, listening, run);

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
    const open = (next: Opened) => {
      if (!shown.current) setEntrance((count) => count + 1);
      shown.current = true;
      setClosing(false);
      setOpened(next);
    };
    const close = () => {
      shown.current = false;
      setClosing(true);
    };
    const subscriptions = Promise.all([onSnapshot(receive), onOpened(open), onClosing(close)]);
    run(
      subscriptions.then(() => {
        setListening(true);
        return getSnapshot().then(receive);
      }),
    );
    run(getUiSettings().then(setUi));
    return () => void subscriptions.then((unlisteners) => unlisteners.forEach((unlisten) => unlisten()));
  }, [run, showNotices]);

  useEffect(() => {
    const [area, list, inner] = [scrollArea.current, scroller.current, content.current];
    if (!area || !list || !inner) return;
    const observer = new ResizeObserver(() => {
      area.style.setProperty('--thumb', String(thumbFraction(list.clientHeight, list.scrollHeight)));
      area.toggleAttribute('data-overflowing', list.scrollHeight > list.clientHeight);
    });
    let idle = 0;
    const startScrolling = () => {
      window.clearTimeout(idle);
      area.toggleAttribute('data-scrolling', true);
    };
    const stopScrolling = () => {
      idle = window.setTimeout(() => area.toggleAttribute('data-scrolling', false), SCROLL_IDLE_MS);
    };
    observer.observe(list);
    observer.observe(inner);
    list.addEventListener('scroll', startScrolling);
    list.addEventListener('scrollend', stopScrolling);
    return () => {
      observer.disconnect();
      window.clearTimeout(idle);
      list.removeEventListener('scroll', startScrolling);
      list.removeEventListener('scrollend', stopScrolling);
    };
  }, []);

  const sessions = snapshot?.sessions ?? [];
  const { groups, ended } = groupSessions(sessions);
  const crossed = crossedOff(sessions);
  const listOrder = [...groups.flatMap((group) => group.sessions), ...ended];
  const visible = [...groups.flatMap((group) => group.sessions), ...(endedOpen ? ended : [])];
  const openSession = sessions.find((session) => session.session_id === openId);
  const queue = requestQueue(snapshot?.pending ?? []);
  const current = currentRequest(queue, chosenRequestId);
  const ticking = queue.length > 0 || sessions.some((session) => session.state !== 'ended' && turnRunning(session));
  const pendingOf = (sessionId: string) => queue.filter((request) => request.session_id === sessionId);
  const view = settingsOpen
    ? { key: 'settings', depth: 2 }
    : openSession
      ? { key: `${DETAIL_PREFIX}${openSession.session_id}`, depth: 1 }
      : { key: 'list', depth: 0 };

  useEffect(() => {
    const syncHidden = () => document.documentElement.toggleAttribute('data-hidden', document.hidden);
    document.addEventListener('visibilitychange', syncHidden);
    return () => document.removeEventListener('visibilitychange', syncHidden);
  }, []);

  useListMotion(
    content,
    ghosts,
    view.key === 'list'
      ? [
          ...groups.map((group) => `${group.label}:${group.sessions.map((session) => session.session_id)}`),
          `ended:${ended.map((session) => session.session_id)}`,
          `crossed:${crossed.map((session) => session.session_id)}`,
          queue.map((request) => request.id),
        ].join('|')
      : null,
    entrance,
  );

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

  const openDetail = (sessionId: string) => {
    setSelectedId(sessionId);
    setOpenId(sessionId);
  };

  const crossOff = (sessionId: string) =>
    run(dismiss(sessionId).then(() => setOpenId((open) => (open === sessionId ? null : open))));

  const reportJump = (sessionId: string, { notice, succeeded }: JumpReport) => {
    if (notice) showNotices([{ id: nextJumpNoticeId.current--, sessionId, text: notice }]);
    if (succeeded) run(closePanel());
  };

  const jumpTo = (sessionId: string) => {
    flashRow(sessionId);
    run(jump(sessionId).then((outcome) => reportJump(sessionId, jumpReport(outcome))));
  };

  const changeUi = (change: Partial<UiSettings>) =>
    setUiSettings(change).then((saved) => {
      setUi(saved);
      return saved;
    });

  const focusPrompt = (sessionId: string) =>
    window.setTimeout(() => document.getElementById(promptBoxId(sessionId))?.focus({ preventScroll: !openSession }));

  useEffect(() => {
    const index = visible.findIndex((session) => session.session_id === selectedId);
    const selected: Session | undefined = openSession ?? visible[index];
    const handleKey = (event: KeyboardEvent) => {
      if (settingsOpen) {
        if (event.key === 'Escape') setSettingsOpen(false);
        return;
      }
      if (event.ctrlKey || event.altKey || event.metaKey || isTextField(event.target)) return;
      const key = event.key.toLowerCase();
      if (current && (key === 'a' || key === 'd' || key === 'm')) {
        event.preventDefault();
        const act = requestKeys.current.get(current.id);
        if (act) act(key);
        else {
          openDetail(current.session_id);
          window.setTimeout(() => requestKeys.current.get(current.id)?.(key));
        }
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
          if (openSession) break;
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
          if (!openSession && selected && !(event.target instanceof HTMLButtonElement)) openDetail(selected.session_id);
          break;
        case 'ArrowLeft':
        case 'Backspace':
          if (openSession) {
            event.preventDefault();
            setOpenId(null);
          }
          break;
        case '[':
        case ']': {
          const next = openSession && stepTab(sessionTabs(listOrder, openSession.session_id), openSession.session_id, event.key === ']' ? 1 : -1);
          if (next) openDetail(next);
          break;
        }
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
        case 'x':
          if (selected && pendingOf(selected.session_id).length === 0) crossOff(selected.session_id);
          break;
        case '/':
          if (selected && promptPath(selected)) {
            event.preventDefault();
            openDetail(selected.session_id);
            focusPrompt(selected.session_id);
          }
          break;
        case 'Escape':
          if (openSession) setOpenId(null);
          else run(closePanel());
          break;
      }
    };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  });

  const renderView = (key: string, active: boolean) => {
    const keys = active ? requestKeys.current : null;
    if (key === 'settings') return <SettingsView config={snapshot?.config ?? null} ui={ui} onChangeUi={changeUi} />;
    if (key.startsWith(DETAIL_PREFIX)) {
      const session = sessions.find((each) => each.session_id === key.slice(DETAIL_PREFIX.length));
      return (
        session && (
          <SessionView
            session={session}
            tabs={sessionTabs(listOrder, session.session_id)}
            pending={pendingOf(session.session_id)}
            notices={notices.filter((notice) => notice.sessionId === session.session_id)}
            currentId={current?.id ?? null}
            queueSize={queue.length}
            keys={keys}
            now={now}
            globalSnoozedUntil={snapshot?.snoozed_until_ms ?? null}
            onBack={() => setOpenId(null)}
            onSwitch={openDetail}
            onJump={() => jumpTo(session.session_id)}
            onCrossOff={() => crossOff(session.session_id)}
            onJumpReport={(report) => reportJump(session.session_id, report)}
            run={run}
          />
        )
      );
    }
    const renderRow = (session: Session, crossedOffRow: boolean) => (
      <SessionRow
        key={session.session_id}
        session={session}
        pending={pendingOf(session.session_id)}
        notices={notices.filter((notice) => notice.sessionId === session.session_id)}
        currentId={current?.id ?? null}
        queueSize={queue.length}
        keys={crossedOffRow ? null : keys}
        now={now}
        selected={session.session_id === selectedId}
        crossedOff={crossedOffRow}
        onOpen={() => openDetail(session.session_id)}
        onJump={() => jumpTo(session.session_id)}
        onCrossOff={() => crossOff(session.session_id)}
      />
    );
    return (
      <>
        {snapshot === null && <EmptyState title="Not connected to the Beacon core." detail="Sessions appear once it is running." />}
        {snapshot?.sessions.length === 0 && (
          <EmptyState title="No Claude sessions running." detail="Sessions appear here when Claude Code starts." />
        )}
        <SessionList
          groups={groups}
          ended={ended}
          crossed={crossed}
          endedOpen={endedOpen}
          crossedOpen={crossedOpen}
          onToggleEnded={() => setEndedOpen(!endedOpen)}
          onToggleCrossed={() => setCrossedOpen(!crossedOpen)}
          renderRow={(session) => renderRow(session, false)}
          renderCrossedRow={(session) => renderRow(session, true)}
        />
      </>
    );
  };

  const offset = entryOffset(opened?.edge ?? null);

  return (
    <PanelFit value={fit}>
      <section
        ref={panel}
        style={{ '--enter-x': `${offset.x}px`, '--enter-y': `${offset.y}px` } as CSSProperties}
        className={`flex h-screen flex-col overflow-hidden rounded-panel border border-line bg-paper font-sans text-body text-ink ${opened === null ? 'opacity-0' : closing ? 'animate-panel-out' : 'animate-panel-in'}`}
      >
        <Header
          sessions={sessions.filter((session) => !session.dismissed)}
          accountUsage={snapshot?.account_usage ?? null}
          snoozedUntil={snapshot?.snoozed_until_ms ?? null}
          queue={current && queue.length > 1 ? { position: queue.indexOf(current) + 1, total: queue.length } : null}
          soundsEnabled={ui?.sounds_enabled ?? null}
          settingsOpen={settingsOpen}
          onToggleSounds={() => ui && run(changeUi({ sounds_enabled: !ui.sounds_enabled }))}
          onToggleSettings={() => setSettingsOpen(!settingsOpen)}
          run={run}
        />
        <div ref={scrollArea} className="scroll-area relative flex min-h-0 flex-1 flex-col">
          <div ref={scroller} className="scroll-list min-h-0 flex-1 scroll-py-4 overflow-x-hidden overflow-y-auto">
            <div ref={content} className="relative">
              {failure && (
                <p title={failure} className="truncate border-b border-line-soft px-2.5 py-1.5 text-micro text-danger">
                  {failure}
                </p>
              )}
              <ViewStack view={view} scroller={scroller} render={renderView} />
              <div ref={ghosts} aria-hidden="true" className="pointer-events-none absolute inset-0" />
            </div>
          </div>
          <div aria-hidden="true" className="scroll-thumb" />
        </div>
      </section>
    </PanelFit>
  );
}

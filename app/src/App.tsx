import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties } from 'react';
import { flushSync } from 'react-dom';
import {
  acknowledge,
  appInfo,
  cachedAppInfo,
  cachedDetail,
  cachedHooksStatus,
  cachedOverlay,
  closePanel,
  dismiss,
  getDetail,
  getEnded,
  getOverlay,
  hooksStatus,
  initialSnapshot,
  initialUi,
  jump,
  onClosing,
  onOpened,
  onOpenSettings,
  onSnapshot,
  setUiSettings,
  snooze,
  undismiss,
  type Opened,
  type OpenedEvent,
  type Session,
  type Snapshot,
  type UiSettings,
} from './beacon';
import { EmptyState } from './components/EmptyState';
import { Header } from './components/Header';
import { SessionList } from './components/SessionList';
import { SessionRow } from './components/SessionRow';
import { SessionView } from './components/SessionView';
import { SettingsView, type SettingsPart } from './components/SettingsView';
import { TextButton } from './components/TextButton';
import { ViewStack } from './components/ViewStack';
import { useListMotion } from './hooks/useListMotion.ts';
import { PanelFit, usePanelHeight } from './hooks/usePanelHeight.ts';
import { toggleOpen } from './lib/disclosure.ts';
import { errorText } from './lib/format.ts';
import { jumpReport, type JumpReport } from './lib/jump.ts';
import { entryOffset } from './lib/panel.ts';
import { promptBoxId, promptPath } from './lib/prompts.ts';
import { thumbFraction } from './lib/scroll.ts';
import {
  currentRequest,
  firstRequestOf,
  keyTarget,
  requestId,
  requestQueue,
  requestsOf,
  resolutionNotices,
  stepRequest,
  type ResolutionNotice,
} from './lib/queue.ts';
import { reuseEqual } from './lib/reuse.ts';
import {
  canAcknowledge,
  canSnooze,
  cardId,
  crossedOff,
  findSession,
  groupSessions,
  sessionTabs,
  sessionTitle,
  stepTab,
} from './lib/sessions.ts';
import { flashRow } from './motion.ts';

const NOTICE_MS = 4000;
const HIDDEN_NOTICE_MS = 6000;
const SCROLL_IDLE_MS = 800;
const PRELOAD_WAIT_MS = 150;
const DETAIL_PREFIX = 'detail:';
const NO_SESSIONS: Session[] = [];

const whenLoaded = (loading: Promise<unknown>) =>
  Promise.race([loading.catch(() => null), new Promise((ready) => window.setTimeout(ready, PRELOAD_WAIT_MS))]);

const isTextField = (target: EventTarget | null): target is HTMLElement =>
  target instanceof HTMLTextAreaElement ||
  target instanceof HTMLSelectElement ||
  (target instanceof HTMLInputElement && target.type !== 'radio' && target.type !== 'checkbox');

export function App() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(initialSnapshot);
  const [opened, setOpened] = useState<Opened | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [openId, setOpenId] = useState<string | null>(null);
  const [endedOpen, setEndedOpen] = useState(false);
  const [endedList, setEndedList] = useState<Session[] | null>(null);
  const [crossedOpen, setCrossedOpen] = useState(false);
  const [listening, setListening] = useState(false);
  const [chosenRequestId, setChosenRequestId] = useState<number | null>(null);
  const [notices, setNotices] = useState<ResolutionNotice[]>([]);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [ui, setUi] = useState<UiSettings>(initialUi);
  const [closing, setClosing] = useState(false);
  const [entrance, setEntrance] = useState(0);
  const [viewReset, setViewReset] = useState(0);
  const [moreOpen, setMoreOpen] = useState<string[]>([]);
  const [settingsParts, setSettingsParts] = useState<SettingsPart[]>([]);
  const [hidden, setHidden] = useState<{ sessionId: string; title: string } | null>(null);
  const hiddenTimer = useRef(0);
  const panel = useRef<HTMLElement>(null);
  const scrollArea = useRef<HTMLDivElement>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const ghosts = useRef<HTMLDivElement>(null);
  const shown = useRef(false);
  const pendingIds = useRef<number[]>([]);
  const requestKeys = useRef(new Map<number, (key: string) => void>());
  const nextJumpNoticeId = useRef(-1);
  const opening = useRef<string | null>(null);
  const shownView = useRef('list');

  const fail = useCallback((error: unknown) => setFailure(errorText(error)), []);

  const run = useCallback(
    (action: Promise<unknown>) => {
      action.then(() => setFailure(null), fail);
    },
    [fail],
  );

  const fit = usePanelHeight(panel, scroller, content, listening, fail);

  const showNotices = useCallback((fresh: ResolutionNotice[]) => {
    setNotices((current) => [...current, ...fresh]);
    window.setTimeout(() => setNotices((current) => current.filter((notice) => !fresh.includes(notice))), NOTICE_MS);
  }, []);

  useEffect(() => {
    const receive = (next: Snapshot | null) => {
      if (next) {
        const fresh = shown.current ? resolutionNotices(pendingIds.current, next.pending, next.resolved) : [];
        pendingIds.current = next.pending.map((request) => request.id);
        if (fresh.length > 0) showNotices(fresh);
      }
      setSnapshot((previous) =>
        previous && next
          ? {
              ...next,
              sessions: reuseEqual(previous.sessions, next.sessions, (session) => session.session_id),
              pending: reuseEqual(previous.pending, next.pending, (request) => request.id),
            }
          : next,
      );
    };
    const open = ({ opened: next, snapshot: latest, ui: settings }: OpenedEvent) => {
      receive(latest);
      setUi(settings);
      if (!shown.current) {
        opening.current = null;
        setOpenId(null);
        setSettingsOpen(false);
        if (shownView.current !== 'list') setViewReset((count) => count + 1);
        setEntrance((count) => count + 1);
      }
      shown.current = true;
      setClosing(false);
      setOpened(next);
    };
    const close = () => {
      shown.current = false;
      setClosing(true);
    };
    const subscriptions = Promise.all([onSnapshot(receive), onOpened(open), onClosing(close)]);
    run(subscriptions.then(() => setListening(true)));
    return () => {
      void subscriptions.then((unlisteners) => unlisteners.forEach((unlisten) => unlisten()));
    };
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

  const sessions = snapshot?.sessions ?? NO_SESSIONS;
  const { groups, ended: recentlyEnded } = useMemo(() => groupSessions(sessions), [sessions]);
  const ended = useMemo(() => (endedList ?? NO_SESSIONS).filter((session) => !session.dismissed), [endedList]);
  const listed = useMemo(() => sessions.filter((session) => !session.dismissed), [sessions]);
  const endedKey = `${snapshot?.ended_count ?? 0}:${recentlyEnded.map((session) => session.session_id)}`;
  const crossed = useMemo(() => crossedOff(sessions), [sessions]);
  const listOrder = [...groups.flatMap((group) => group.sessions), ...ended];
  const visible = [...groups.flatMap((group) => group.sessions), ...(endedOpen ? ended : [])];
  const openSession = findSession(sessions, ended, openId);
  const queue = requestQueue(snapshot?.pending ?? []);
  const current = currentRequest(queue, chosenRequestId);
  const pendingOf = (sessionId: string) => requestsOf(queue, sessionId);
  const view = settingsOpen
    ? { key: 'settings', depth: 2 }
    : openSession
      ? { key: `${DETAIL_PREFIX}${openSession.session_id}`, depth: 1 }
      : { key: 'list', depth: 0 };

  useEffect(() => {
    shownView.current = view.key;
  }, [view.key]);

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
    if (endedOpen) getEnded().then(setEndedList, fail);
  }, [endedOpen, endedKey, fail]);

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
    opening.current = sessionId;
    return (cachedDetail(sessionId) ? Promise.resolve() : whenLoaded(getDetail(sessionId))).then(() => {
      if (opening.current !== sessionId) return;
      opening.current = null;
      flushSync(() => setOpenId(sessionId));
    });
  };

  const openSettings = () =>
    void (cachedHooksStatus() && cachedAppInfo() && cachedOverlay()
      ? Promise.resolve()
      : whenLoaded(Promise.all([hooksStatus(), appInfo(), getOverlay()]))
    ).then(() => flushSync(() => setSettingsOpen(true)));

  const toggleSettings = () => (settingsOpen ? setSettingsOpen(false) : openSettings());

  const closeDetail = () => {
    opening.current = null;
    setOpenId(null);
  };

  const crossOff = (sessionId: string) => {
    const session = findSession(sessions, ended, sessionId);
    run(
      dismiss(sessionId).then(() => {
        if (opening.current === sessionId) opening.current = null;
        setOpenId((open) => (open === sessionId ? null : open));
        window.clearTimeout(hiddenTimer.current);
        setHidden({ sessionId, title: session ? sessionTitle(session) : sessionId.slice(0, 8) });
        hiddenTimer.current = window.setTimeout(() => setHidden(null), HIDDEN_NOTICE_MS);
        if (endedOpen) return getEnded().then(setEndedList);
      }),
    );
  };

  const undoHide = (sessionId: string) => {
    window.clearTimeout(hiddenTimer.current);
    setHidden(null);
    run(undismiss(sessionId));
  };

  const toggleEnded = () => {
    if (endedOpen) setEndedOpen(false);
    else
      run(
        getEnded().then((list) => {
          setEndedList(list);
          setEndedOpen(true);
        }),
      );
  };

  const reportJump = (sessionId: string, { notice, succeeded }: JumpReport) => {
    if (notice) showNotices([{ id: nextJumpNoticeId.current--, sessionId, text: notice }]);
    if (succeeded) run(closePanel());
  };

  const jumpTo = (sessionId: string) => {
    flashRow(sessionId);
    run(jump(sessionId).then((outcome) => reportJump(sessionId, jumpReport(outcome))));
  };

  const rowActions = useRef({ openDetail, jumpTo, crossOff, openSettings });
  useLayoutEffect(() => {
    rowActions.current = { openDetail, jumpTo, crossOff, openSettings };
  });
  const openRow = useCallback((sessionId: string) => rowActions.current.openDetail(sessionId), []);
  const jumpRow = useCallback((sessionId: string) => rowActions.current.jumpTo(sessionId), []);
  const crossOffRow = useCallback((sessionId: string) => rowActions.current.crossOff(sessionId), []);

  useEffect(() => {
    const subscription = onOpenSettings(() => rowActions.current.openSettings());
    return () => void subscription.then((unlisten) => unlisten());
  }, []);

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
    const tabQueue = openSession ? pendingOf(openSession.session_id) : queue;
    const handleKey = (event: KeyboardEvent) => {
      if (event.repeat && !isTextField(event.target) && !event.key.startsWith('Arrow')) {
        if (event.key === 'Enter' || event.key === ' ') event.preventDefault();
        return;
      }
      if (settingsOpen) {
        if (event.key === 'Escape') setSettingsOpen(false);
        return;
      }
      if (event.key === 'Escape' && isTextField(event.target)) {
        event.target.blur();
        return;
      }
      if (event.ctrlKey || event.altKey || event.metaKey || isTextField(event.target)) return;
      const key = event.key.toLowerCase();
      const target = keyTarget(queue, chosenRequestId, selected?.session_id ?? null);
      if (target && (key === 'a' || key === 'd' || key === 'm')) {
        event.preventDefault();
        const act = requestKeys.current.get(target.id);
        if (act) act(key);
        else void openDetail(target.session_id);
        return;
      }
      switch (event.key) {
        case 'Tab': {
          const nextId = stepRequest(tabQueue, chosenRequestId, event.shiftKey ? -1 : 1);
          const next = tabQueue.find((request) => request.id === nextId);
          if (next) {
            event.preventDefault();
            setChosenRequestId(next.id);
            if (!openSession) setSelectedId(next.session_id);
          }
          break;
        }
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
          if (!openSession && selected && !(event.target instanceof HTMLButtonElement)) void openDetail(selected.session_id);
          break;
        case 'ArrowLeft':
        case 'Backspace':
          if (openSession && !(event.target instanceof HTMLInputElement)) {
            event.preventDefault();
            closeDetail();
          }
          break;
        case '[':
        case ']': {
          const next = openSession && stepTab(sessionTabs(listOrder, openSession.session_id), openSession.session_id, event.key === ']' ? 1 : -1);
          if (next) void openDetail(next);
          break;
        }
        case 'a':
          if (selected && canAcknowledge(selected)) run(acknowledge(selected.session_id));
          break;
        case 's':
          if (selected && canSnooze(selected)) run(snooze(selected.session_id, 5));
          break;
        case 'j': {
          const jumpTarget = selected ? (selected.state !== 'ended' ? selected.session_id : null) : current?.session_id;
          if (jumpTarget) jumpTo(jumpTarget);
          break;
        }
        case 'x':
          if (selected && pendingOf(selected.session_id).length === 0) crossOff(selected.session_id);
          break;
        case '/':
          if (selected && promptPath(selected)) {
            event.preventDefault();
            void openDetail(selected.session_id).then(() => focusPrompt(selected.session_id));
          }
          break;
        case 'Escape':
          if (openSession || opening.current) closeDetail();
          else run(closePanel());
          break;
      }
    };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  });

  const renderView = (key: string, active: boolean) => {
    const keys = active ? requestKeys.current : null;
    if (key === 'settings')
      return (
        <SettingsView
          config={snapshot?.config ?? null}
          ui={ui}
          openParts={settingsParts}
          onToggle={(part) => setSettingsParts((open) => toggleOpen(open, part))}
          onChangeUi={changeUi}
        />
      );
    if (key.startsWith(DETAIL_PREFIX)) {
      const session = findSession(sessions, ended, key.slice(DETAIL_PREFIX.length));
      return (
        session && (
          <SessionView
            session={session}
            tabs={sessionTabs(listOrder, session.session_id)}
            pending={pendingOf(session.session_id)}
            notices={notices.filter((notice) => notice.sessionId === session.session_id)}
            currentId={keyTarget(queue, chosenRequestId, session.session_id)?.id ?? null}
            queueSize={pendingOf(session.session_id).length}
            keys={keys}
            globalSnoozedUntil={snapshot?.snoozed_until_ms ?? null}
            moreOpen={moreOpen.includes(session.session_id)}
            onToggleMore={() => setMoreOpen((open) => toggleOpen(open, session.session_id))}
            onBack={closeDetail}
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
        selected={session.session_id === selectedId}
        crossedOff={crossedOffRow}
        onOpen={openRow}
        onJump={jumpRow}
        onCrossOff={crossOffRow}
      />
    );
    return (
      <>
        {snapshot === null && <EmptyState title="Not connected to the Beacon core" detail="Sessions appear once it is running" />}
        {snapshot?.sessions.length === 0 && (
          <EmptyState title="No Claude sessions running" detail="Sessions appear here when Claude Code starts" />
        )}
        <SessionList
          groups={groups}
          ended={ended}
          endedCount={snapshot?.ended_count ?? 0}
          crossed={crossed}
          endedOpen={endedOpen}
          crossedOpen={crossedOpen}
          onToggleEnded={toggleEnded}
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
        className={`flex h-screen flex-col overflow-hidden rounded-panel border border-line bg-paper font-sans text-body text-ink shadow-[inset_0_1px_0_var(--color-sheen)] ${opened === null ? 'opacity-0' : closing ? 'animate-panel-out' : 'animate-panel-in'}`}
      >
        <Header
          sessions={listed}
          accountUsage={snapshot?.account_usage ?? null}
          snoozedUntil={snapshot?.snoozed_until_ms ?? null}
          queue={current && queue.length > 1 ? { position: queue.indexOf(current) + 1, total: queue.length } : null}
          soundsEnabled={ui.sounds_enabled}
          settingsOpen={settingsOpen}
          pinned={opened?.mode === 'pinned'}
          onUnpin={() => run(closePanel())}
          onToggleSounds={() => run(changeUi({ sounds_enabled: !ui.sounds_enabled }))}
          onToggleSettings={toggleSettings}
          run={run}
        />
        <div ref={scrollArea} className="scroll-area relative flex min-h-0 flex-1 flex-col">
          <div ref={scroller} className="scroll-list min-h-0 flex-1 scroll-py-4 overflow-x-hidden overflow-y-auto">
            <div ref={content} className="relative">
              {failure && (
                <p title={failure} className="origin-top animate-appear truncate border-b border-line-soft px-4 py-2 text-secondary text-danger">
                  {failure}
                </p>
              )}
              {hidden && (
                <div
                  role="status"
                  className="flex origin-top animate-appear items-center gap-3 border-b border-line-soft bg-surface px-4 py-2 text-secondary"
                >
                  <p className="min-w-0 flex-1 truncate text-muted">
                    <span className="text-ink">{hidden.title}</span> is hidden until its next prompt
                  </p>
                  <TextButton onClick={() => undoHide(hidden.sessionId)}>Undo</TextButton>
                </div>
              )}
              <ViewStack key={viewReset} view={view} scroller={scroller} render={renderView} />
              <div ref={ghosts} aria-hidden="true" className="pointer-events-none absolute inset-0" />
            </div>
          </div>
          <div aria-hidden="true" className="scroll-thumb" />
        </div>
      </section>
    </PanelFit>
  );
}

import { Fragment } from 'react';
import type { AccountUsage, Session } from '../beacon';
import { summary, summaryParts, type QueuePlace } from '../lib/sessions.ts';
import { Icon } from './Icon';
import { RollingNumber } from './RollingNumber';
import { SnoozeMenu } from './SnoozeMenu';
import { UsageButton } from './UsageButton';

interface HeaderProps {
  sessions: Session[];
  accountUsage: AccountUsage | null;
  snoozedUntil: number | null;
  queue: QueuePlace | null;
  soundsEnabled: boolean;
  settingsOpen: boolean;
  pinned: boolean;
  onUnpin: () => void;
  onToggleSounds: () => void;
  onToggleSettings: () => void;
  run: (action: Promise<unknown>) => void;
}

export function Header({
  sessions,
  accountUsage,
  snoozedUntil,
  queue,
  soundsEnabled,
  settingsOpen,
  pinned,
  onUnpin,
  onToggleSounds,
  onToggleSettings,
  run,
}: HeaderProps) {
  const parts = summaryParts(sessions, queue);
  return (
    <header className="relative z-20 flex h-11 shrink-0 items-center gap-2 border-b border-line-soft pr-2 pl-4">
      <p
        aria-label={summary(sessions, queue)}
        title={queue && queue.total > 1 ? 'Tab and Shift+Tab move between requests' : undefined}
        className="min-w-0 truncate text-muted"
      >
        {parts.length === 0
          ? 'All quiet'
          : parts.map((part, index) => (
              <Fragment key={part.kind}>
                {index > 0 && <span className="text-faint"> · </span>}
                <span className={part.kind === 'request' ? undefined : 'text-ink'}>
                  <RollingNumber value={part.count} /> {part.text}
                </span>
              </Fragment>
            ))}
      </p>
      <div className="ml-auto flex shrink-0 items-center gap-1">
        {accountUsage && <UsageButton usage={accountUsage} />}
        {pinned && (
          <button
            type="button"
            aria-label="Hide until Claude is next active"
            title="Pinned while Claude is active. Click to hide until Claude is next active"
            onClick={onUnpin}
            className="press grid size-7 place-items-center rounded-[7px] text-ink hover:bg-surface"
          >
            <Icon name="pin" />
          </button>
        )}
        <SnoozeMenu snoozedUntil={snoozedUntil} soundsEnabled={soundsEnabled} onToggleSounds={onToggleSounds} run={run} />
        <button
          type="button"
          aria-label="Settings"
          aria-pressed={settingsOpen}
          title={settingsOpen ? 'Back to sessions' : 'Settings'}
          onClick={onToggleSettings}
          className={`press grid size-7 place-items-center rounded-[7px] transition-[color,background-color,rotate] duration-200 ease-settle hover:bg-surface hover:text-ink ${settingsOpen ? 'rotate-45 ' : ''}${settingsOpen ? 'bg-surface text-ink' : 'text-muted'}`}
        >
          <Icon name="gear" />
        </button>
      </div>
    </header>
  );
}

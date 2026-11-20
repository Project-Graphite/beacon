import { Fragment } from 'react';
import type { AccountUsage, Session } from '../beacon';
import { summary, summaryParts } from '../lib/sessions.ts';
import { Icon } from './Icon';
import { RollingNumber } from './RollingNumber';
import { SnoozeMenu } from './SnoozeMenu';
import { UsageButton } from './UsageButton';

interface HeaderProps {
  sessions: Session[];
  accountUsage: AccountUsage | null;
  snoozedUntil: number | null;
  queue: { position: number; total: number } | null;
  soundsEnabled: boolean | null;
  settingsOpen: boolean;
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
  onToggleSounds,
  onToggleSettings,
  run,
}: HeaderProps) {
  const parts = summaryParts(sessions);
  return (
    <header className="relative z-20 flex h-8 shrink-0 items-center gap-1.5 border-b border-line-soft px-2.5">
      <p aria-label={summary(sessions)} className="min-w-0 truncate text-secondary text-muted">
        {parts.length === 0
          ? 'All quiet'
          : parts.map((part, index) => (
              <Fragment key={part.kind}>
                {index > 0 && <span className="text-faint"> · </span>}
                <span className="text-ink">
                  <RollingNumber value={part.count} /> {part.text}
                </span>
              </Fragment>
            ))}
      </p>
      <div className="ml-auto flex shrink-0 items-center gap-0.5">
        {queue && (
          <span
            title="Tab and Shift+Tab move between requests"
            className="mr-1 flex items-center gap-1.5 font-mono text-micro text-muted tabular-nums"
          >
            <span aria-hidden="true" className="size-1.5 rounded-full bg-wip" />
            <RollingNumber value={queue.position} /> of <RollingNumber value={queue.total} />
          </span>
        )}
        {accountUsage && <UsageButton usage={accountUsage} />}
        <button
          type="button"
          aria-label="Sounds"
          aria-pressed={soundsEnabled ?? false}
          disabled={soundsEnabled === null}
          title="Play a sound when a session needs you or finishes"
          onClick={onToggleSounds}
          className={`press grid size-6 place-items-center rounded-field hover:bg-surface hover:text-ink ${soundsEnabled ? 'text-ink' : 'text-muted'}`}
        >
          <Icon name={soundsEnabled ? 'sound-on' : 'sound-off'} />
        </button>
        <SnoozeMenu snoozedUntil={snoozedUntil} run={run} />
        <button
          type="button"
          aria-label="Settings"
          aria-pressed={settingsOpen}
          title={settingsOpen ? 'Back to sessions' : 'Settings'}
          onClick={onToggleSettings}
          className={`press grid size-6 place-items-center rounded-field hover:bg-surface hover:text-ink ${settingsOpen ? 'bg-surface text-ink' : 'text-muted'}`}
        >
          <Icon name="gear" />
        </button>
      </div>
    </header>
  );
}

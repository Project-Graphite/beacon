import type { AccountUsage, Session } from '../beacon';
import { summary } from '../lib/sessions.ts';
import { Icon } from './Icon';
import { SnoozeMenu } from './SnoozeMenu';
import { UsageButton } from './UsageButton';

interface HeaderProps {
  sessions: Session[];
  accountUsage: AccountUsage | null;
  snoozedUntil: number | null;
  pinned: boolean;
  queue: { position: number; total: number } | null;
  settingsOpen: boolean;
  onToggleSettings: () => void;
  run: (action: Promise<unknown>) => void;
}

export function Header({
  sessions,
  accountUsage,
  snoozedUntil,
  pinned,
  queue,
  settingsOpen,
  onToggleSettings,
  run,
}: HeaderProps) {
  return (
    <header className="relative z-20 flex h-10 shrink-0 items-center gap-2 border-b border-line-soft px-3">
      <h1 className="shrink-0 font-semibold text-ink">Beacon</h1>
      <p className="min-w-0 truncate text-secondary text-muted">{summary(sessions)}</p>
      <div className="ml-auto flex shrink-0 items-center gap-1">
        {queue && (
          <span
            title="Tab and Shift+Tab move between requests"
            className="mr-1 flex items-center gap-1.5 font-mono text-micro text-muted tabular-nums"
          >
            <span aria-hidden="true" className="size-1.5 rounded-full bg-wip" />
            {queue.position} of {queue.total}
          </span>
        )}
        {accountUsage && <UsageButton usage={accountUsage} />}
        <SnoozeMenu snoozedUntil={snoozedUntil} run={run} />
        {pinned && (
          <span
            role="img"
            aria-label="Stays open while a session needs you"
            title="Stays open while a session needs you"
            className="grid size-[26px] place-items-center text-wip"
          >
            <Icon name="pin" />
          </span>
        )}
        <button
          type="button"
          aria-label="Settings"
          aria-pressed={settingsOpen}
          title={settingsOpen ? 'Back to sessions' : 'Settings'}
          onClick={onToggleSettings}
          className={`grid size-[26px] place-items-center rounded-field transition-colors duration-150 ease-settle hover:bg-surface hover:text-ink ${settingsOpen ? 'bg-surface text-ink' : 'text-muted'}`}
        >
          <Icon name="gear" />
        </button>
      </div>
    </header>
  );
}

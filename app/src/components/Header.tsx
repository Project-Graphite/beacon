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
  run: (action: Promise<unknown>) => void;
}

export function Header({ sessions, accountUsage, snoozedUntil, pinned, run }: HeaderProps) {
  return (
    <header className="relative z-20 flex h-10 shrink-0 items-center gap-2 border-b border-line-soft px-3">
      <h1 className="shrink-0 font-semibold text-ink">Beacon</h1>
      <p className="min-w-0 truncate text-secondary text-muted">{summary(sessions)}</p>
      <div className="ml-auto flex shrink-0 items-center gap-1">
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
      </div>
    </header>
  );
}

import { useEffect, useState } from 'react';
import { refreshUsage, type AccountUsage } from '../beacon';
import { errorText } from '../lib/format.ts';
import { usageLabel, usageTitle } from '../lib/account.ts';

const FAILURE_SHOWN_MS = 5000;

export function UsageButton({ usage }: { usage: AccountUsage }) {
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const [hoveredAt, setHoveredAt] = useState(Date.now);

  useEffect(() => {
    if (failure === null) return;
    const timer = window.setTimeout(() => setFailure(null), FAILURE_SHOWN_MS);
    return () => window.clearTimeout(timer);
  }, [failure]);

  const label = usageLabel(usage);
  if (label === '') return null;

  const refresh = () => {
    setBusy(true);
    setFailure(null);
    refreshUsage()
      .catch((error) => setFailure(errorText(error)))
      .finally(() => setBusy(false));
  };

  return (
    <button
      type="button"
      disabled={busy}
      aria-busy={busy}
      title={usageTitle(usage, hoveredAt, failure)}
      onPointerEnter={() => setHoveredAt(Date.now())}
      onClick={refresh}
      className="press flex h-6 shrink-0 items-center gap-1.5 rounded-field px-1.5 font-mono text-micro text-muted tabular-nums hover:bg-surface hover:text-ink disabled:animate-pulse disabled:cursor-progress disabled:text-faint disabled:hover:bg-transparent"
    >
      {failure !== null && <span className="size-1.5 rounded-full bg-danger" />}
      {label}
    </button>
  );
}

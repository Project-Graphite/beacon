import type { PendingDecision } from '../beacon';
import { formatDuration } from '../lib/format.ts';

export function PendingRequest({ pending, now }: { pending: PendingDecision; now: number }) {
  return (
    <div className="mt-2 rounded-field border border-line bg-surface p-2">
      <div className="flex items-baseline gap-2">
        <span className="min-w-0 truncate font-semibold text-ink">{pending.tool_name}</span>
        <span className="ml-auto shrink-0 font-mono text-micro text-muted tabular-nums">
          {formatDuration(now - pending.created_ms)}
        </span>
      </div>
      <p className="mt-0.5 line-clamp-2 font-mono text-secondary text-ink [overflow-wrap:anywhere]">{pending.preview}</p>
      <p className="mt-1 text-micro text-muted">Answer in the session window</p>
    </div>
  );
}

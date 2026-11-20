import type { AccountUsage } from '../beacon';
import { formatDuration, resetTime } from './format.ts';

const windows = (usage: AccountUsage) => [
  { name: '5h', percent: usage.five_hour_percent, resetsAt: usage.five_hour_resets_at },
  { name: '7d', percent: usage.seven_day_percent, resetsAt: usage.seven_day_resets_at },
];

export function usageLabel(usage: AccountUsage): string {
  return windows(usage)
    .filter((window) => window.percent !== null)
    .map((window) => `${window.name} ${Math.round(window.percent ?? 0)}%`)
    .join(' · ');
}

export function usageTitle(usage: AccountUsage, now: number, failure: string | null): string {
  return [
    usage.fetched_ms === null
      ? 'Reading time unknown'
      : `Read ${formatDuration(now - usage.fetched_ms)} ago`,
    ...windows(usage)
      .filter((window) => window.resetsAt !== null)
      .map((window) => `${window.name} window resets ${resetTime(window.resetsAt ?? '', now)}`),
    failure && `Refresh failed: ${failure}`,
    'Refresh (spends a small Haiku request)',
  ]
    .filter(Boolean)
    .join('\n');
}

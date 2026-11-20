const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

const pad = (value: number) => String(value).padStart(2, '0');

export function formatDuration(ms: number): string {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m${pad(seconds % 60)}s`;
  if (seconds < 86_400) return `${Math.floor(seconds / 3600)}h${pad(Math.floor(seconds / 60) % 60)}m`;
  return `${Math.floor(seconds / 86_400)}d${pad(Math.floor(seconds / 3600) % 24)}h`;
}

export function formatStepDuration(ms: number | null): string {
  if (ms === null) return '';
  return ms < 10_000 ? `${(Math.floor(ms / 100) / 10).toFixed(1)}s` : formatDuration(ms);
}

export function formatTokens(count: number): string {
  const scaled = (value: number, suffix: string) =>
    `${value < 9.95 ? String(Number(value.toFixed(1))) : Math.round(value)}${suffix}`;
  if (count < 1000) return String(count);
  if (count < 999_500) return scaled(count / 1000, 'k');
  return scaled(count / 1_000_000, 'M');
}

export function clockTime(ms: number): string {
  const date = new Date(ms);
  return `${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

export function resetTime(iso: string, now: number): string {
  const ms = Date.parse(iso);
  if (Number.isNaN(ms)) return iso;
  const date = new Date(ms);
  return date.toDateString() === new Date(now).toDateString()
    ? clockTime(ms)
    : `${WEEKDAYS[date.getDay()]} ${clockTime(ms)}`;
}

export function shortModel(model: string): string {
  return model.replace(/^claude-/, '');
}

import type { PendingDecision } from '../beacon';
import { formatDuration } from './format.ts';
import { shortenPath } from './paths.ts';

const META_CHARS = 52;
const MIN_PATH_CHARS = 16;
const EXPIRY_SHOWN_MS = 15 * 60 * 1000;
const ARGUMENT_CHARS = 200;

export function requestMeta(request: PendingDecision, now: number): string[] {
  const timeLeft = request.expires_ms - now;
  const rest = [
    request.permission_mode && request.permission_mode !== 'default' ? request.permission_mode : null,
    request.agent_id !== null ? `from subagent${request.agent_type ? ` ${request.agent_type}` : ''}` : null,
    formatDuration(now - request.created_ms),
    timeLeft < EXPIRY_SHOWN_MS ? `expires in ${Math.max(1, Math.ceil(timeLeft / 60_000))}m` : null,
  ].filter((item): item is string => item !== null);
  if (request.cwd === null) return rest;
  const used = rest.reduce((total, item) => total + item.length + 3, 0);
  return [shortenPath(request.cwd, Math.max(MIN_PATH_CHARS, META_CHARS - used)), ...rest];
}

export function mcpTool(toolName: string): { server: string; tool: string } | null {
  const match = /^mcp__(.+?)__(.+)$/.exec(toolName);
  return match ? { server: match[1], tool: match[2] } : null;
}

export function argumentEntries(toolInput: unknown): [string, string][] {
  const entries =
    toolInput !== null && typeof toolInput === 'object' && !Array.isArray(toolInput)
      ? Object.entries(toolInput)
      : [['input', toolInput] as const];
  return entries.map(([key, value]) => {
    const shown = JSON.stringify(value) ?? String(value);
    return [key, shown.length > ARGUMENT_CHARS ? `${shown.slice(0, ARGUMENT_CHARS - 1)}…` : shown];
  });
}

export function splitUrl(url: string): { head: string; host: string; tail: string } {
  const match = /^([a-z][a-z0-9+.-]*:\/\/(?:[^@/?#]*@)?)([^/?#:]*)(.*)$/is.exec(url);
  return match ? { head: match[1], host: match[2], tail: match[3] } : { head: '', host: '', tail: url };
}

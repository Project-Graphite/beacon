import type { SessionState } from '../beacon';

export type AvatarFace = 'working' | 'alarmed' | 'happy' | 'crossed' | 'sideways' | 'sleepy' | 'blank' | 'closed';

export const AVATAR_HUES = ['#8f9cf7', '#f08cc0', '#5bbcf0', '#bd95f2', '#33c9bf', '#e08fdc'];

const BLINK_PERIOD_MS = 4200;

function idHash(sessionId: string): number {
  let hash = 0x811c9dc5;
  for (const char of sessionId) hash = Math.imul(hash ^ char.charCodeAt(0), 0x01000193);
  return hash >>> 0;
}

export function avatarHue(sessionId: string): string {
  return AVATAR_HUES[idHash(sessionId) % AVATAR_HUES.length];
}

export function blinkDelay(sessionId: string): number {
  return -((idHash(sessionId) >>> 8) % BLINK_PERIOD_MS);
}

export function avatarFace(state: SessionState): AvatarFace {
  switch (state) {
    case 'working':
      return 'working';
    case 'needs_permission':
    case 'needs_answer':
    case 'needs_plan_approval':
    case 'needs_mcp_input':
      return 'alarmed';
    case 'done':
      return 'happy';
    case 'error':
      return 'crossed';
    case 'stalled':
      return 'sideways';
    case 'idle':
      return 'sleepy';
    case 'starting':
      return 'blank';
    case 'ended':
      return 'closed';
  }
}

import type { Step } from '../beacon';

const RECENT_STEPS = 6;

export function recentSteps(steps: Step[]): { earlier: Step[]; recent: Step[] } {
  const split = Math.max(0, steps.length - RECENT_STEPS);
  return { earlier: steps.slice(0, split), recent: steps.slice(split) };
}

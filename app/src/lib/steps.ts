import type { Session, Step } from '../beacon';

const RECENT_STEPS = 6;
const HEADING_PX = 20;
const BLOCK_GAP_PX = 12;
const TASK_ROW_PX = 19;
const STEP_ROW_PX = 20;
const EARLIER_BUTTON_PX = 16;

export function recentSteps(steps: Step[]): { earlier: Step[]; recent: Step[] } {
  const split = Math.max(0, steps.length - RECENT_STEPS);
  return { earlier: steps.slice(0, split), recent: steps.slice(split) };
}

export function reservedDetailHeight({ tasks_created, event_count }: Pick<Session, 'tasks_created' | 'event_count'>): number {
  const steps = Math.min(event_count, RECENT_STEPS);
  const blocks = [
    tasks_created > 0 ? HEADING_PX + tasks_created * TASK_ROW_PX - 2 : 0,
    steps > 0 ? HEADING_PX + steps * STEP_ROW_PX + (event_count > RECENT_STEPS ? EARLIER_BUTTON_PX : 0) : 0,
  ].filter((height) => height > 0);
  return blocks.reduce((total, height) => total + height, 0) + Math.max(0, blocks.length - 1) * BLOCK_GAP_PX;
}

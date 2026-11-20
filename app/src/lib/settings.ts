import type { HooksAction, HooksState } from '../beacon';
import type { DiffLine } from './diff.ts';

export const HOOKS_STATES: Record<HooksState, { label: string; dot: string }> = {
  installed: { label: 'Installed', dot: 'bg-live' },
  not_installed: { label: 'Not installed', dot: 'bg-muted' },
  needs_repair: { label: 'Needs repair', dot: 'bg-wip' },
};

export function hooksButtons(state: HooksState): { label: string; action: HooksAction }[] {
  return [
    ...(state === 'installed'
      ? []
      : [{ label: state === 'needs_repair' ? 'Repair' : 'Install', action: 'install' as const }]),
    ...(state === 'not_installed' ? [] : [{ label: 'Uninstall', action: 'uninstall' as const }]),
  ];
}

const DIFF_MARKS: Record<string, DiffLine['kind']> = { ' ': 'context', '+': 'added', '-': 'removed' };

export function splitPlan(plan: string): { notes: string[]; diff: DiffLine[] } {
  const notes: string[] = [];
  const diff: DiffLine[] = [];
  let inDiff = false;
  for (const line of plan.replace(/\n$/, '').split('\n')) {
    if (line.startsWith('--- ') && !inDiff) {
      inDiff = true;
    } else if (inDiff && line.startsWith('+++ ')) {
      continue;
    } else if (inDiff && line.startsWith('@@')) {
      diff.push({ kind: 'context', text: line });
    } else if (inDiff && line[0] in DIFF_MARKS) {
      diff.push({ kind: DIFF_MARKS[line[0]], text: line.slice(1) });
    } else if (!line.startsWith('Nothing was changed.') && !line.startsWith('Warning: ')) {
      inDiff = false;
      notes.push(line);
    }
  }
  return { notes, diff };
}

export function stallMinutes(text: string): number | null {
  const trimmed = text.trim();
  if (!/^\d+$/.test(trimmed)) return null;
  const minutes = Number(trimmed);
  return minutes >= 1 && minutes <= 240 ? minutes : null;
}

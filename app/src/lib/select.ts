export interface SelectState {
  open: boolean;
  active: number;
}

export interface SelectStep extends SelectState {
  choose: boolean;
}

const OPENING_KEYS = ['ArrowDown', 'ArrowUp', 'Enter', ' '];

export function selectKey(state: SelectState, key: string, count: number, selected: number): SelectStep | null {
  if (!state.open) {
    return OPENING_KEYS.includes(key) && count > 0 ? { open: true, active: Math.max(selected, 0), choose: false } : null;
  }
  const move = (active: number) => ({ open: true, active, choose: false });
  switch (key) {
    case 'ArrowDown':
      return move((state.active + 1) % count);
    case 'ArrowUp':
      return move((state.active - 1 + count) % count);
    case 'Home':
      return move(0);
    case 'End':
      return move(count - 1);
    case 'Enter':
    case ' ':
      return { ...state, choose: true };
    case 'Escape':
    case 'Tab':
      return { ...state, open: false, choose: false };
    default:
      return null;
  }
}

export function menuOpensUp(trigger: { top: number; bottom: number }, menuHeight: number, viewHeight: number, gap: number) {
  return trigger.bottom + gap + menuHeight > viewHeight && trigger.top - gap - menuHeight >= 0;
}

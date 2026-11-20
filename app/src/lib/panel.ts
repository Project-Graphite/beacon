import type { Opened, PanelEdge } from '../beacon';

const ENTRY_PX = 14;

export type OpenedPayload = Omit<Opened, 'edge'> & { edge?: PanelEdge | null };

export function openedFrom({ mode, focused, edge }: OpenedPayload): Opened {
  return { mode, focused, edge: edge ?? null };
}

export function entryOffset(edge: PanelEdge | null): { x: number; y: number } {
  switch (edge) {
    case 'left':
      return { x: -ENTRY_PX, y: 0 };
    case 'right':
      return { x: ENTRY_PX, y: 0 };
    case 'bottom':
      return { x: 0, y: ENTRY_PX };
    default:
      return { x: 0, y: -ENTRY_PX };
  }
}

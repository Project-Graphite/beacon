const STAGGER_MS = 20;
const STAGGERED_ROWS = 8;

export interface Offset {
  x: number;
  y: number;
}

export interface ListChanges<T> {
  entered: T[];
  moved: { item: T; delta: Offset }[];
  left: T[];
}

export function staggerDelay(index: number): number | null {
  return index < STAGGERED_ROWS ? index * STAGGER_MS : null;
}

export function flipDelta(before: Offset, after: Offset): Offset | null {
  const delta = { x: before.x - after.x, y: before.y - after.y };
  return Math.abs(delta.x) < 0.5 && Math.abs(delta.y) < 0.5 ? null : delta;
}

export function listChanges<T extends { offset: Offset }>(before: Map<string, T>, after: Map<string, T>): ListChanges<T> {
  return {
    entered: [...after].filter(([id]) => !before.has(id)).map(([, item]) => item),
    moved: [...after].flatMap(([id, item]) => {
      const previous = before.get(id);
      const delta = previous && flipDelta(previous.offset, item.offset);
      return delta ? [{ item, delta }] : [];
    }),
    left: [...before].filter(([id]) => !after.has(id)).map(([, item]) => item),
  };
}

export function rollDirection(previous: number | null, next: number): 'up' | 'down' | null {
  if (previous === null || previous === next) return null;
  return next > previous ? 'up' : 'down';
}

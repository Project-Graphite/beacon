export function sameJson(left: unknown, right: unknown): boolean {
  if (left === right) return true;
  if (typeof left !== 'object' || typeof right !== 'object' || left === null || right === null) return false;
  if (Array.isArray(left) !== Array.isArray(right)) return false;
  const leftKeys = Object.keys(left);
  const rightRecord = right as Record<string, unknown>;
  return (
    leftKeys.length === Object.keys(right).length &&
    leftKeys.every((key) => key in rightRecord && sameJson((left as Record<string, unknown>)[key], rightRecord[key]))
  );
}

export function reuseEqual<T>(previous: T[], next: T[], id: (item: T) => string | number): T[] {
  const known = new Map(previous.map((item) => [id(item), item]));
  const merged = next.map((item) => {
    const before = known.get(id(item));
    return before !== undefined && sameJson(before, item) ? before : item;
  });
  return merged.length === previous.length && merged.every((item, index) => item === previous[index]) ? previous : merged;
}

export function sameProps(left: object, right: object): boolean {
  const leftRecord = left as Record<string, unknown>;
  const rightRecord = right as Record<string, unknown>;
  const keys = Object.keys(leftRecord);
  return (
    keys.length === Object.keys(rightRecord).length &&
    keys.every((key) => {
      const [before, after] = [leftRecord[key], rightRecord[key]];
      return (
        before === after ||
        (Array.isArray(before) &&
          Array.isArray(after) &&
          before.length === after.length &&
          before.every((item, index) => item === after[index]))
      );
    })
  );
}
